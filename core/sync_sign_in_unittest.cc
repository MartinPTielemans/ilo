#include "browser_core/sync_sign_in.h"

#include <map>
#include <optional>
#include <string>
#include <string_view>

#include "base/json/json_reader.h"
#include "browser_core/public/browser_core.h"
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
  const GURL url = AuthorizeUrl(GURL(kServer), GURL(kRedirect), "state-1", "challenge-1", Road::kPage);
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
      Query(AuthorizeUrl(GURL(kServer), GURL(kRedirect), "state-1", "challenge-1", Road::kPage));
  std::map<std::string, std::string> google =
      Query(AuthorizeUrl(GURL(kServer), GURL(kRedirect), "state-1", "challenge-1", Road::kGoogle));
  EXPECT_EQ(google["provider"], "google");
  google.erase("provider");
  EXPECT_EQ(google, plain);
}

TEST(SyncSignInTest, TheStateAndRedirectAreEscaped) {
  const GURL url =
      AuthorizeUrl(GURL("http://127.0.0.1:8080"), GURL("http://127.0.0.1:8080/signed-in?a=b&c=d"),
                   "a&b=c", "x", Road::kGoogle);
  std::map<std::string, std::string> query = Query(url);
  EXPECT_EQ(query["state"], "a&b=c");
  EXPECT_EQ(query["redirect_uri"], "http://127.0.0.1:8080/signed-in?a=b&c=d");
  EXPECT_EQ(query["provider"], "google");
}

TEST(SyncSignInTest, ThePasskeyRoadIsTheSameSignInOnThePasskeyStep) {
  std::map<std::string, std::string> plain =
      Query(AuthorizeUrl(GURL(kServer), GURL(kRedirect), "state-1", "challenge-1", Road::kPage));
  std::map<std::string, std::string> passkey =
      Query(AuthorizeUrl(GURL(kServer), GURL(kRedirect), "state-1", "challenge-1", Road::kPasskey));
  EXPECT_EQ(passkey["ilo_road"], "passkey");
  passkey.erase("ilo_road");
  EXPECT_EQ(passkey, plain);
}

TEST(SyncSignInTest, AServersOwnPageGetsTheTypedAddressAsItsHint) {
  std::map<std::string, std::string> query =
      Query(AuthorizeUrl(GURL(kServer), GURL(kRedirect), "s", "c", Road::kPage, "ada+1@example.org"));
  EXPECT_EQ(query["login_hint"], "ada+1@example.org");
  EXPECT_FALSE(
      Query(AuthorizeUrl(GURL(kServer), GURL(kRedirect), "s", "c", Road::kPage)).contains("login_hint"));
}

bool Native(std::string_view json) {
  std::optional<base::DictValue> discovery = base::JSONReader::ReadDict(json, base::JSON_PARSE_RFC);
  EXPECT_TRUE(discovery) << json;
  return discovery && SignsInNatively(*discovery);
}

TEST(SyncSignInTest, OnlyAServerThatSaysSoTakesTheCodeFromTheApp) {
  EXPECT_TRUE(Native(R"({"issuer": "https://sync.example.org", "ilo_native_sign_in": true})"));
  // An older ilo-server, or another OpenID provider.
  EXPECT_FALSE(Native(R"({"issuer": "https://accounts.example.com"})"));
  EXPECT_FALSE(Native(R"({"ilo_native_sign_in": "true"})"));
  EXPECT_FALSE(Native(R"({"ilo_native_sign_in": 1})"));
}

TEST(SyncSignInTest, ACodeIsSixDigitsWithSpacesLeftOut) {
  EXPECT_EQ(CleanCode("123456"), "123456");
  EXPECT_EQ(CleanCode("123 456"), "123456");
  EXPECT_EQ(CleanCode(" 1 2 3 4 5 6 "), "123456");
  EXPECT_EQ(CleanCode("12345"), std::nullopt);
  EXPECT_EQ(CleanCode("1234567"), std::nullopt);
  EXPECT_EQ(CleanCode("12345a"), std::nullopt);
  EXPECT_EQ(CleanCode("12345\""), std::nullopt);
  EXPECT_EQ(CleanCode(""), std::nullopt);
}

TEST(SyncSignInTest, TheRoadsBodiesAreTheJsonTheServerReads) {
  std::optional<base::DictValue> start = base::JSONReader::ReadDict(
      StartBody("challenge-1", "ada\"@example.org"), base::JSON_PARSE_RFC);
  ASSERT_TRUE(start);
  EXPECT_EQ(*start->FindString("client_id"), "ilo");
  EXPECT_EQ(*start->FindString("code_challenge"), "challenge-1");
  EXPECT_EQ(*start->FindString("code_challenge_method"), "S256");
  EXPECT_EQ(*start->FindString("email"), "ada\"@example.org");

  std::optional<base::DictValue> resend =
      base::JSONReader::ReadDict(ResendBody("id-1"), base::JSON_PARSE_RFC);
  ASSERT_TRUE(resend);
  EXPECT_EQ(*resend->FindString("sign_in"), "id-1");

  std::optional<base::DictValue> code = base::JSONReader::ReadDict(
      CodeBody("id-1", "123456", "verifier-1", "ilo on Mac (Mac15,3)"), base::JSON_PARSE_RFC);
  ASSERT_TRUE(code);
  EXPECT_EQ(*code->FindString("sign_in"), "id-1");
  EXPECT_EQ(*code->FindString("code"), "123456");
  EXPECT_EQ(*code->FindString("code_verifier"), "verifier-1");
  EXPECT_EQ(*code->FindString("device_name"), "ilo on Mac (Mac15,3)");
}

TEST(SyncSignInTest, EachRefusalIsTheProblemTheAppSays) {
  EXPECT_EQ(Problem("invalid_email"), BC_SYNC_PROBLEM_NOT_AN_ADDRESS);
  EXPECT_EQ(Problem("undeliverable"), BC_SYNC_PROBLEM_NO_MAIL);
  EXPECT_EQ(Problem("limit_address"), BC_SYNC_PROBLEM_CODE_LIMIT);
  EXPECT_EQ(Problem("limit_client"), BC_SYNC_PROBLEM_CODE_LIMIT);
  EXPECT_EQ(Problem("limit_server"), BC_SYNC_PROBLEM_CODE_LIMIT);
  EXPECT_EQ(Problem("no_more_codes"), BC_SYNC_PROBLEM_CODE_LIMIT);
  EXPECT_EQ(Problem("mail_failed"), BC_SYNC_PROBLEM_MAIL_FAILED);
  EXPECT_EQ(Problem("wrong_code"), BC_SYNC_PROBLEM_WRONG_CODE);
  EXPECT_EQ(Problem("too_many_tries"), BC_SYNC_PROBLEM_TOO_MANY_TRIES);
  EXPECT_EQ(Problem("expired"), BC_SYNC_PROBLEM_CODE_EXPIRED);
  EXPECT_EQ(Problem("used"), BC_SYNC_PROBLEM_CODE_EXPIRED);
  EXPECT_EQ(Problem("sign_in_expired"), BC_SYNC_PROBLEM_CODE_EXPIRED);
  EXPECT_EQ(Problem("invalid_grant"), BC_SYNC_PROBLEM_SERVER);
  EXPECT_EQ(Problem(""), BC_SYNC_PROBLEM_SERVER);
}

TEST(SyncSignInTest, ACodeRefusedKeepsTheCodeStepAndALostSignInEndsIt) {
  for (const char* kept : {"wrong_code", "too_many_tries", "expired", "used", "limit_address",
                           "limit_client", "limit_server", "no_more_codes", "mail_failed"}) {
    EXPECT_FALSE(EndsSignIn(kept)) << kept;
  }
  for (const char* ended : {"sign_in_expired", "invalid_grant", "invalid_request", ""}) {
    EXPECT_TRUE(EndsSignIn(ended)) << ended;
  }
}

TEST(SyncSignInTest, AWipedSecretIsEmpty) {
  std::string code = "123456";
  Wipe(code);
  EXPECT_TRUE(code.empty());
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
