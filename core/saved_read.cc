#include "browser_core/saved_read.h"

#include <utility>

#include "base/functional/bind.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "browser_core/saved_read_rule.h"
#include "net/base/isolation_info.h"
#include "net/base/load_flags.h"
#include "net/base/net_errors.h"
#include "net/cookies/site_for_cookies.h"
#include "net/http/http_request_headers.h"
#include "net/http/http_response_headers.h"
#include "net/traffic_annotation/network_traffic_annotation.h"
#include "net/url_request/redirect_info.h"
#include "services/network/public/cpp/resource_request.h"
#include "services/network/public/cpp/shared_url_loader_factory.h"
#include "services/network/public/cpp/simple_url_loader.h"
#include "services/network/public/mojom/url_response_head.mojom.h"
#include "url/origin.h"

namespace browser_core {

namespace {

constexpr net::NetworkTrafficAnnotationTag kSavedRead =
    net::DefineNetworkTrafficAnnotation("browser_core_saved_read", R"(
      semantics {
        sender: "Saved reads"
        description:
          "One GET of an address the user saved for reading, on the profile's own session, as the "
          "site's own page would read it, with no tab open."
        trigger:
          "Only when the app asks: an agent the user connected uses a read the user saved. Never "
          "on a timer, never again after a failure."
        data: "The saved address alone, with the profile's cookies for its site."
        destination: WEBSITE
      }
      policy {
        cookies_allowed: YES
        cookies_store: "user"
        setting: "Settings › Agents: turn agents off, or remove the saved read."
        policy_exception_justification: "Only for a read the user saved."
      })");

void Tell(SavedReads::Done done, SavedReadAnswer answer) {
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(std::move(done), std::move(answer)));
}

}  // namespace

SavedReads::Under::Under() = default;
SavedReads::Under::Under(Under&&) = default;
SavedReads::Under& SavedReads::Under::operator=(Under&&) = default;
SavedReads::Under::~Under() = default;

SavedReads::SavedReads(scoped_refptr<network::SharedURLLoaderFactory> factory)
    : factory_(std::move(factory)) {}

// The loaders go with it, and with them every callback they had.
SavedReads::~SavedReads() = default;

void SavedReads::Read(const GURL& url, size_t max_bytes, int64_t timeout_ms, Done done) {
  const std::string refused =
      SavedReadRefusal(url, /*off_the_record=*/false, max_bytes, timeout_ms);
  if (!refused.empty()) {
    Tell(std::move(done), {.error = refused});
    return;
  }
  const url::Origin origin = url::Origin::Create(url);
  auto request = std::make_unique<network::ResourceRequest>();
  request->url = url;
  request->method = net::HttpRequestHeaders::kGetMethod;
  // As the site's own page reads it: its cookies, SameSite and partitioned ones included, the site
  // as the first party, and no sign-in asked of the user. No CORS mode, so a redirect to another
  // origin reaches Redirected, which tells it; the request tool's same-origin mode fails one
  // without its address.
  request->credentials_mode = network::mojom::CredentialsMode::kInclude;
  request->mode = network::mojom::RequestMode::kNoCors;
  request->destination = network::mojom::RequestDestination::kEmpty;
  request->do_not_prompt_for_login = true;
  // Neither read from the HTTP cache nor written to it, so no answer stays on disk.
  request->load_flags = net::LOAD_DISABLE_CACHE;
  request->request_initiator = origin;
  request->site_for_cookies = net::SiteForCookies::FromOrigin(origin);
  request->trusted_params = network::ResourceRequest::TrustedParams();
  request->trusted_params->isolation_info = net::IsolationInfo::Create(
      net::IsolationInfo::RequestType::kOther, origin, origin, request->site_for_cookies);
  const uint64_t read = ++last_;
  Under& under = reads_[read];
  under.asked = url;
  under.done = std::move(done);
  under.loader = network::SimpleURLLoader::Create(std::move(request), kSavedRead);
  under.loader->SetAllowHttpErrorResults(true);
  under.loader->SetAllowPartialResults(true);
  under.loader->SetTimeoutDuration(base::Milliseconds(SavedReadMilliseconds(timeout_ms)));
  under.loader->SetOnRedirectCallback(
      base::BindRepeating(&SavedReads::Redirected, weak_.GetWeakPtr(), read));
  under.loader->DownloadToString(factory_.get(),
                                 base::BindOnce(&SavedReads::Answered, weak_.GetWeakPtr(), read),
                                 SavedReadBytes(max_bytes));
}

void SavedReads::EndAll(const std::string& why) {
  std::map<uint64_t, Under> ending = std::move(reads_);
  reads_.clear();
  for (auto& [read, under] : ending) {
    under.loader.reset();
    Tell(std::move(under.done), {.error = why});
  }
}

void SavedReads::Redirected(uint64_t read,
                            const GURL& from,
                            const net::RedirectInfo& redirect,
                            const network::mojom::URLResponseHead& head,
                            std::vector<std::string>* removed_headers) {
  auto it = reads_.find(read);
  if (it == reads_.end() || SavedReadFollows(it->second.asked, redirect.new_url)) {
    return;
  }
  // Copied first: |from| and |redirect| may be the loader's own.
  SavedReadAnswer answer = {
      .status = redirect.status_code, .url = from.spec(), .redirect = redirect.new_url.spec()};
  // Ended before anything goes to the other origin: the loader is gone, which its callback
  // allows, so it follows nothing.
  Under under = std::move(it->second);
  reads_.erase(it);
  under.loader.reset();
  Tell(std::move(under.done), std::move(answer));
}

void SavedReads::Answered(uint64_t read, std::optional<std::string> body) {
  auto it = reads_.find(read);
  if (it == reads_.end()) {
    return;
  }
  Under under = std::move(it->second);
  reads_.erase(it);
  const network::SimpleURLLoader& loader = *under.loader;
  const network::mojom::URLResponseHead* head = loader.ResponseInfo();
  const int error = loader.NetError();
  SavedReadAnswer answer;
  answer.status = head && head->headers ? head->headers->response_code() : 0;
  answer.url = loader.GetFinalURL().spec();
  answer.type = head ? head->mime_type : std::string();
  // With partial results, a body past the limit fails so, with what came of it.
  answer.cut = error == net::ERR_INSUFFICIENT_RESOURCES && body;
  answer.body = std::move(body).value_or(std::string());
  if (error != net::OK && !answer.cut) {
    answer.error = (answer.status ? "The answer broke off: " : "No answer came: ") +
                   net::ErrorToShortString(error);
  }
  std::move(under.done).Run(answer);
}

}  // namespace browser_core
