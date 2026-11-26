#include "browser_core/sync_sign_in.h"

#include <map>
#include <optional>
#include <string>
#include <string_view>

#include "base/json/json_reader.h"
#include "net/base/url_util.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::sync_sign_in {
namespace {

constexpr char kServer[] = "https://sync.example.org/";
constexpr char kRedirect[] = "https://sync.example.org/signed-in";

std::map<std::string, std::string> Query(const GURL& url) {
  std::map<std::string, std::string> query;
  for (net::QueryIterator it(url); !it.IsAtEnd(); it.Advance()) {
    EXPECT_FALSE(query.contains(std::string(it.GetKey()))) << it.GetKey() << " twice";
    query[std::string(it.GetKey())] = it.GetUnescapedValue();
  }
  return query;
}

bool Offers(std::string_view json) {
  std::optional<base::DictValue> discovery = base::JSONReader::ReadDict(json, base::JSON_PARSE_RFC);
  EXPECT_TRUE(discovery) << json;
  return discovery && OffersGoogle(*discovery);
}

TEST(SyncSignInTest, TheSignInGoesToTheServersAuthorizeForTheBrowser) {
  const GURL url = AuthorizeUrl(GURL(kServer), GURL(kRedirect), "state-1", "challenge-1", false);
  EXPECT_EQ(url.DeprecatedGetOriginAsURL(), GURL(kServer));
  EXPECT_EQ(url.path(), "/authorize");
  EXPECT_EQ(Query(url), (std::map<std::string, std::string>{
                            {"response_type", "code"},
                            {"client_id", "ilo"},
                            {"redirect_uri", "https://sync.example.org/signed-in"},
                            {"scope", "openid email offline_access"},
                            {"state", "state-1"},
                            {"code_challenge", "challenge-1"},
                            {"code_challenge_method", "S256"},
                        }));
}

TEST(SyncSignInTest, AGoogleSignInIsTheSameSignInStraightOnToGoogle) {
  std::map<std::string, std::string> plain =
      Query(AuthorizeUrl(GURL(kServer), GURL(kRedirect), "state-1", "challenge-1", false));
  std::map<std::string, std::string> google =
      Query(AuthorizeUrl(GURL(kServer), GURL(kRedirect), "state-1", "challenge-1", true));
  EXPECT_EQ(google["provider"], "google");
  google.erase("provider");
  EXPECT_EQ(google, plain);
}

TEST(SyncSignInTest, TheStateAndRedirectAreEscaped) {
  const GURL url =
      AuthorizeUrl(GURL("http://127.0.0.1:8080"), GURL("http://127.0.0.1:8080/signed-in?a=b&c=d"),
                   "a&b=c", "x", true);
  std::map<std::string, std::string> query = Query(url);
  EXPECT_EQ(query["state"], "a&b=c");
  EXPECT_EQ(query["redirect_uri"], "http://127.0.0.1:8080/signed-in?a=b&c=d");
  EXPECT_EQ(query["provider"], "google");
}

TEST(SyncSignInTest, OnlyAServerThatSaysSoOffersGoogle) {
  EXPECT_TRUE(Offers(R"({"issuer": "https://sync.example.org", "ilo_google": true})"));
  EXPECT_FALSE(Offers(R"({"issuer": "https://sync.example.org"})"));
  EXPECT_FALSE(Offers(R"({"ilo_google": false})"));
  EXPECT_FALSE(Offers(R"({"ilo_google": "true"})"));
  EXPECT_FALSE(Offers(R"({"ilo_google": 1})"));
  EXPECT_FALSE(Offers(R"({"ilo_google": null})"));
}

TEST(SyncSignInTest, OnlyTheServerSayingSoRemovesGoogle) {
  EXPECT_TRUE(GoogleRemoved(204));
  // Another device removed it first.
  EXPECT_TRUE(GoogleRemoved(404));
  // No answer: the network, or the server's gone.
  EXPECT_FALSE(GoogleRemoved(0));
  EXPECT_FALSE(GoogleRemoved(200));
  EXPECT_FALSE(GoogleRemoved(401));
  EXPECT_FALSE(GoogleRemoved(403));
  EXPECT_FALSE(GoogleRemoved(500));
  EXPECT_FALSE(GoogleRemoved(503));
}

}  // namespace
}  // namespace browser_core::sync_sign_in
