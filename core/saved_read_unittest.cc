#include "browser_core/saved_read.h"

#include <atomic>
#include <memory>
#include <optional>
#include <string>
#include <vector>

#include "base/functional/bind.h"
#include "base/memory/scoped_refptr.h"
#include "base/test/bind.h"
#include "base/test/task_environment.h"
#include "base/test/test_future.h"
#include "browser_core/mojo_once_for_testing.h"
#include "mojo/public/cpp/bindings/pending_receiver.h"
#include "mojo/public/cpp/bindings/pending_remote.h"
#include "mojo/public/cpp/bindings/remote.h"
#include "net/base/isolation_info.h"
#include "net/base/load_flags.h"
#include "net/cookies/site_for_cookies.h"
#include "net/http/http_status_code.h"
#include "net/test/embedded_test_server/embedded_test_server.h"
#include "net/test/embedded_test_server/http_request.h"
#include "net/test/embedded_test_server/http_response.h"
#include "services/network/network_service.h"
#include "services/network/public/cpp/resource_request.h"
#include "services/network/public/cpp/weak_wrapper_shared_url_loader_factory.h"
#include "services/network/public/mojom/network_context.mojom.h"
#include "services/network/public/mojom/network_service.mojom.h"
#include "services/network/public/mojom/url_loader_factory.mojom.h"
#include "services/network/test/fake_test_cert_verifier_params_factory.h"
#include "services/network/test/test_network_context_client.h"
#include "services/network/test/test_url_loader_factory.h"
#include "testing/gmock/include/gmock/gmock.h"
#include "testing/gtest/include/gtest/gtest.h"
#include "url/gurl.h"
#include "url/origin.h"

namespace browser_core {
namespace {

using ::testing::HasSubstr;

constexpr char kClosed[] = "The profile closed before it answered.";

// What the read sends, on a loader factory that records each request.
class SavedReadRequestTest : public testing::Test {
 protected:
  static void SetUpTestSuite() { StartMojoOnceForTesting(); }

  void SetUp() override {
    factory_.SetInterceptor(base::BindLambdaForTesting(
        [this](const network::ResourceRequest& request) { sent_.push_back(request); }));
  }

  SavedReadAnswer Read(const GURL& url) {
    base::test::TestFuture<SavedReadAnswer> answer;
    reads_->Read(url, 1000, 1000, answer.GetCallback<const SavedReadAnswer&>());
    return answer.Take();
  }

  base::test::TaskEnvironment task_environment_;
  network::TestURLLoaderFactory factory_;
  std::vector<network::ResourceRequest> sent_;
  std::optional<SavedReads> reads_{std::in_place, factory_.GetSafeWeakWrapper()};
};

TEST_F(SavedReadRequestTest, TheReadIsOneGetAsTheSitesOwnPageWouldSendIt) {
  const GURL url("https://www.example.test/inbox?page=2");
  factory_.AddResponse(url.spec(), "the inbox");
  const SavedReadAnswer answer = Read(url);
  EXPECT_EQ(answer.status, 200);
  EXPECT_EQ(answer.url, url.spec());
  EXPECT_EQ(answer.body, "the inbox");
  EXPECT_FALSE(answer.cut);
  EXPECT_EQ(answer.error, "");

  ASSERT_EQ(sent_.size(), 1u);
  const network::ResourceRequest& sent = sent_[0];
  const url::Origin origin = url::Origin::Create(url);
  EXPECT_EQ(sent.url, url);
  EXPECT_EQ(sent.method, "GET");
  EXPECT_FALSE(sent.request_body);
  EXPECT_TRUE(sent.headers.IsEmpty()) << sent.headers.ToString();
  EXPECT_EQ(sent.credentials_mode, network::mojom::CredentialsMode::kInclude);
  EXPECT_EQ(sent.mode, network::mojom::RequestMode::kNoCors);
  EXPECT_TRUE(sent.do_not_prompt_for_login);
  EXPECT_EQ(sent.request_initiator, origin);
  EXPECT_TRUE(sent.site_for_cookies.IsEquivalent(net::SiteForCookies::FromOrigin(origin)));
  ASSERT_TRUE(sent.trusted_params);
  EXPECT_TRUE(sent.trusted_params->isolation_info.IsEqualForTesting(
      net::IsolationInfo::Create(net::IsolationInfo::RequestType::kOther, origin, origin,
                                 net::SiteForCookies::FromOrigin(origin))));
}

TEST_F(SavedReadRequestTest, NothingIsTakenFromTheCacheOrPutInIt) {
  const GURL url("https://www.example.test/");
  factory_.AddResponse(url.spec(), "page");
  Read(url);
  ASSERT_EQ(sent_.size(), 1u);
  EXPECT_TRUE(sent_[0].load_flags & net::LOAD_DISABLE_CACHE);
}

TEST_F(SavedReadRequestTest, ARefusalSendsNothingAndAnswersFromATaskOfItsOwn) {
  bool answered = false;
  SavedReadAnswer answer;
  reads_->Read(GURL("file:///etc/hosts"), 1000, 1000,
               base::BindLambdaForTesting([&](const SavedReadAnswer& each) {
                 answered = true;
                 answer = each;
               }));
  EXPECT_FALSE(answered);
  task_environment_.RunUntilIdle();
  EXPECT_TRUE(answered);
  EXPECT_EQ(answer.status, 0);
  EXPECT_NE(answer.error, "");
  EXPECT_TRUE(sent_.empty());
}

// The profile's going (profile_reads.cc's OnProfileWillBeDestroyed) ends its reads so.
TEST_F(SavedReadRequestTest, AReadUnderWayWhenItsProfileGoesEndsOnceAndHearsNothingAfter) {
  const GURL url("https://www.example.test/slow");
  std::vector<SavedReadAnswer> answers;
  reads_->Read(url, 1000, 1000, base::BindLambdaForTesting([&](const SavedReadAnswer& answer) {
                 answers.push_back(answer);
               }));
  task_environment_.RunUntilIdle();
  ASSERT_EQ(sent_.size(), 1u);
  ASSERT_EQ(reads_->Reading(), 1u);

  reads_->EndAll(kClosed);
  EXPECT_TRUE(answers.empty());
  task_environment_.RunUntilIdle();
  ASSERT_EQ(answers.size(), 1u);
  EXPECT_EQ(answers[0].error, kClosed);
  EXPECT_EQ(answers[0].status, 0);
  EXPECT_EQ(reads_->Reading(), 0u);

  // The site answering late reaches no one.
  factory_.AddResponse(url.spec(), "late");
  task_environment_.RunUntilIdle();
  EXPECT_EQ(answers.size(), 1u);

  // Nor once the profile's reads are gone.
  reads_.reset();
  task_environment_.RunUntilIdle();
  EXPECT_EQ(answers.size(), 1u);
}

// At the quit the reads go with no answer, as the app may have let go of what waits for them.
TEST_F(SavedReadRequestTest, AReadUnderWayWhenItsReadsGoIsNeverAnswered) {
  const GURL url("https://www.example.test/slow");
  bool answered = false;
  reads_->Read(url, 1000, 1000,
               base::BindLambdaForTesting([&](const SavedReadAnswer&) { answered = true; }));
  task_environment_.RunUntilIdle();
  ASSERT_EQ(sent_.size(), 1u);
  reads_.reset();
  factory_.AddResponse(url.spec(), "late");
  task_environment_.RunUntilIdle();
  EXPECT_FALSE(answered);
}

// The read on a network service and context of its own, against a site on this machine.
class SavedReadNetworkTest : public testing::Test {
 protected:
  static void SetUpTestSuite() { StartMojoOnceForTesting(); }

  SavedReadNetworkTest() : task_environment_(base::test::TaskEnvironment::MainThreadType::IO) {}

  void SetUp() override {
    mojo::Remote<network::mojom::NetworkService> service;
    network_service_ = network::NetworkService::Create(service.BindNewPipeAndPassReceiver());
    network::mojom::NetworkContextParamsPtr context_params =
        network::mojom::NetworkContextParams::New();
    context_params->cert_verifier_params =
        network::FakeTestCertVerifierParamsFactory::GetCertVerifierParams();
    service->CreateNetworkContext(context_.BindNewPipeAndPassReceiver(),
                                  std::move(context_params));
    mojo::PendingReceiver<network::mojom::URLLoaderNetworkServiceObserver> observer;
    network::mojom::NetworkServiceParamsPtr service_params =
        network::mojom::NetworkServiceParams::New();
    service_params->default_observer = observer.InitWithNewPipeAndPassRemote();
    service->SetParams(std::move(service_params));
    mojo::PendingRemote<network::mojom::NetworkContextClient> client;
    client_ = std::make_unique<network::TestNetworkContextClient>(
        client.InitWithNewPipeAndPassReceiver());
    context_->SetClient(std::move(client));
    network::mojom::URLLoaderFactoryParamsPtr params =
        network::mojom::URLLoaderFactoryParams::New();
    params->process_id = network::OriginatingProcessId::browser();
    params->is_orb_enabled = false;
    params->is_trusted = true;
    context_->CreateURLLoaderFactory(factory_.BindNewPipeAndPassReceiver(), std::move(params));

    // Another origin: the same host on another port.
    other_.RegisterRequestHandler(base::BindLambdaForTesting(
        [this](const net::test_server::HttpRequest&)
            -> std::unique_ptr<net::test_server::HttpResponse> {
          ++other_asked_;
          auto response = std::make_unique<net::test_server::BasicHttpResponse>();
          response->set_content("the other origin");
          return response;
        }));
    ASSERT_TRUE(other_.Start());
    site_.RegisterRequestHandler(
        base::BindRepeating(&SavedReadNetworkTest::Answer, base::Unretained(this)));
    ASSERT_TRUE(site_.Start());
    reads_.emplace(
        base::MakeRefCounted<network::WeakWrapperSharedURLLoaderFactory>(factory_.get()));
  }

  void TearDown() override { reads_.reset(); }

  std::unique_ptr<net::test_server::HttpResponse> Answer(
      const net::test_server::HttpRequest& request) {
    const std::string path(request.GetURL().path());
    auto response = std::make_unique<net::test_server::BasicHttpResponse>();
    response->set_content_type("text/plain");
    if (path == "/sign-in") {
      // A same-site Strict cookie and one in the site's own partition, as a session would set.
      response->AddCustomHeader("Set-Cookie", "strict=1; Secure; SameSite=Strict; Path=/");
      response->AddCustomHeader("Set-Cookie",
                                "parted=2; Secure; SameSite=None; Partitioned; Path=/");
      response->set_content("signed in");
    } else if (path == "/echo") {
      const auto cookie = request.headers.find("Cookie");
      response->set_content(cookie == request.headers.end() ? "" : cookie->second);
    } else if (path == "/moved") {
      response->set_code(net::HTTP_FOUND);
      response->AddCustomHeader("Location", "/echo");
    } else if (path == "/away") {
      response->set_code(net::HTTP_FOUND);
      response->AddCustomHeader("Location", other_.GetURL("/landing").spec());
    } else if (path == "/big") {
      response->set_content(std::string(2'000'000, 'a'));
    } else if (path == "/hang") {
      return std::make_unique<net::test_server::HungResponse>();
    } else {
      return nullptr;
    }
    return response;
  }

  SavedReadAnswer Read(const std::string& path,
                       size_t max_bytes = 1 << 20,
                       int64_t timeout_ms = 10'000) {
    base::test::TestFuture<SavedReadAnswer> answer;
    reads_->Read(site_.GetURL(path), max_bytes, timeout_ms,
                 answer.GetCallback<const SavedReadAnswer&>());
    return answer.Take();
  }

  base::test::TaskEnvironment task_environment_;
  std::unique_ptr<network::mojom::NetworkService> network_service_;
  std::unique_ptr<network::mojom::NetworkContextClient> client_;
  mojo::Remote<network::mojom::NetworkContext> context_;
  mojo::Remote<network::mojom::URLLoaderFactory> factory_;
  // HTTPS, as a Secure or Partitioned cookie needs, on the fake verifier.
  net::EmbeddedTestServer site_{net::EmbeddedTestServer::TYPE_HTTPS};
  net::EmbeddedTestServer other_{net::EmbeddedTestServer::TYPE_HTTPS};
  std::atomic<int> other_asked_{0};
  std::optional<SavedReads> reads_;
};

TEST_F(SavedReadNetworkTest, TheSessionsCookiesGoWithItAndThoseItSetsAreSaved) {
  const SavedReadAnswer signed_in = Read("/sign-in");
  ASSERT_EQ(signed_in.status, 200) << signed_in.error;
  const SavedReadAnswer echoed = Read("/echo");
  ASSERT_EQ(echoed.status, 200) << echoed.error;
  EXPECT_THAT(echoed.body, HasSubstr("strict=1"));
  EXPECT_THAT(echoed.body, HasSubstr("parted=2"));
}

TEST_F(SavedReadNetworkTest, ARedirectOnTheOriginIsFollowed) {
  const SavedReadAnswer answer = Read("/moved");
  EXPECT_EQ(answer.status, 200) << answer.error;
  EXPECT_EQ(answer.url, site_.GetURL("/echo").spec());
  EXPECT_EQ(answer.redirect, "");
}

TEST_F(SavedReadNetworkTest, ARedirectToAnotherOriginIsToldAndNotFollowed) {
  const SavedReadAnswer answer = Read("/away");
  EXPECT_EQ(answer.status, 302);
  EXPECT_EQ(answer.url, site_.GetURL("/away").spec());
  EXPECT_EQ(answer.redirect, other_.GetURL("/landing").spec());
  EXPECT_EQ(answer.body, "");
  EXPECT_EQ(answer.error, "");
  EXPECT_EQ(other_asked_.load(), 0);
}

TEST_F(SavedReadNetworkTest, ABodyPastItsSizeComesBackCut) {
  const SavedReadAnswer answer = Read("/big", 1000);
  EXPECT_EQ(answer.status, 200);
  EXPECT_EQ(answer.body, std::string(1000, 'a'));
  EXPECT_TRUE(answer.cut);
  EXPECT_EQ(answer.error, "");
}

TEST_F(SavedReadNetworkTest, ASiteThatNeverAnswersEndsAtItsTime) {
  const SavedReadAnswer answer = Read("/hang", 1000, 200);
  EXPECT_EQ(answer.status, 0);
  EXPECT_THAT(answer.error, HasSubstr("ERR_TIMED_OUT"));
  EXPECT_EQ(answer.body, "");
  EXPECT_FALSE(answer.cut);
}

}  // namespace
}  // namespace browser_core
