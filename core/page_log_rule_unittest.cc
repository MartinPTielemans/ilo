#include "browser_core/page_log_rule.h"

#include <string>

#include "base/strings/string_util.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::page_log {
namespace {

Request Failing(const char* url, int status, int net_error = 0, const char* error = "") {
  return {
      .method = "GET", .url = GURL(url), .status = status, .net_error = net_error, .error = error};
}

TEST(PageLogRuleTest, TheStripListKeepsNoSecretInAnyCase) {
  for (const char* secret :
       {"Cookie", "cookie", "COOKIE", "Authorization", "authorization", "Proxy-Authorization",
        "proxy-authorization", "Set-Cookie", "set-cookie", "SET-COOKIE"}) {
    EXPECT_FALSE(Kept(secret)) << secret;
  }
  for (const char* header :
       {"Accept", "Content-Type", "X-Cookie-Note", "Cookie2", "WWW-Authenticate"}) {
    EXPECT_TRUE(Kept(header)) << header;
  }
}

TEST(PageLogRuleTest, NoSecretReachesTheAnswerFromARequestOrAResponse) {
  Log log;
  log.NewPage(true);
  log.Add(Failing("https://example.com/api", 500));
  log.Add("1",
          {.url = GURL("https://example.com/api"),
           .status = 500,
           .request_headers = {{"Cookie", "sid=a"},
                               {"authorization", "Bearer b"},
                               {"PROXY-AUTHORIZATION", "Basic c"},
                               {"Accept", "text/html"}},
           .response_headers = {
               {"Set-Cookie", "sid=d"}, {"set-cookie", "e=f"}, {"Content-Type", "text/plain"}}});
  log.Body("1", "went wrong", false);
  const std::string answer = base::ToLowerASCII(log.Json());
  for (const char* secret : {"cookie", "authorization", "sid=", "bearer", "basic c", "e=f"}) {
    EXPECT_EQ(answer.find(secret), std::string::npos) << secret << " in " << answer;
  }
  EXPECT_NE(answer.find("text/html"), std::string::npos) << answer;
  EXPECT_NE(answer.find("text/plain"), std::string::npos) << answer;
  EXPECT_NE(answer.find("went wrong"), std::string::npos) << answer;
}

TEST(PageLogRuleTest, TrimmedIsTheAddressWithoutQueryFragmentOrUser) {
  EXPECT_EQ(Trimmed(GURL("https://me:pw@example.com:8443/a/b?token=x#top")),
            "https://example.com:8443/a/b");
  EXPECT_EQ(Trimmed(GURL("http://example.com/missing?token=x")), "http://example.com/missing");
  EXPECT_EQ(Trimmed(GURL()), "");
}

TEST(PageLogRuleTest, AFailureIsANetworkErrorOrAStatusFrom400) {
  EXPECT_TRUE(Failed(-20, 0));
  EXPECT_TRUE(Failed(0, 404));
  EXPECT_TRUE(Failed(0, 500));
  EXPECT_FALSE(Failed(0, 200));
  EXPECT_FALSE(Failed(0, 301));
  EXPECT_FALSE(Failed(0, 0));
}

TEST(PageLogRuleTest, OutsideDeveloperModeItKeepsOnlyFailuresTrimmed) {
  Log log;
  log.NewPage(false);
  log.Add(Failing("https://example.com/fine?token=x", 200));
  log.Add(Failing("https://example.com/missing?token=x#y", 404));
  log.Add(Failing("https://ads.example/a.js?id=1", 0, -20, "net::ERR_BLOCKED_BY_CLIENT"));
  log.Add(Message{.level = "error", .text = "secret token", .source = "app.js", .line = 1});
  log.Add("1", {.url = GURL("https://example.com/missing?token=x#y"),
                .status = 404,
                .response_headers = {{"Server", "kept"}}});
  const std::string answer = log.Json();
  EXPECT_EQ(answer,
            "{\"developer_mode\":false,\"requests\":["
            "{\"method\":\"GET\",\"status\":404,\"url\":\"https://example.com/missing\"},"
            "{\"error\":\"net::ERR_BLOCKED_BY_CLIENT\",\"method\":\"GET\","
            "\"url\":\"https://ads.example/a.js\"}]}");
}

TEST(PageLogRuleTest, InDeveloperModeItKeepsTheConsoleAndRequestsWhole) {
  Log log;
  log.NewPage(true);
  log.Add(Failing("https://example.com/missing?token=x", 404));
  log.Add(Message{.level = "error", .text = "it broke", .source = "app.js", .line = 3});
  const std::string answer = log.Json();
  EXPECT_NE(answer.find("\"url\":\"https://example.com/missing?token=x\""), std::string::npos)
      << answer;
  EXPECT_NE(answer.find("\"text\":\"it broke\""), std::string::npos) << answer;
}

TEST(PageLogRuleTest, ThereIsNoSessionOutsideDeveloperMode) {
  Log log;
  log.NewPage(false);
  EXPECT_FALSE(log.WantsSession());
  log.NewPage(true);
  EXPECT_TRUE(log.WantsSession());
  log.Add(Message{.level = "info", .text = "hello"});
  log.Add(Failing("https://example.com/missing?token=x", 404));
  log.Add("1", {.url = GURL("https://example.com/missing?token=x"),
                .status = 404,
                .response_headers = {{"Server", "kept"}}});
  // The user turns it off for the site: the session goes, and with it all the answer may no longer
  // hold.
  log.SetDeveloperMode(false);
  EXPECT_FALSE(log.WantsSession());
  EXPECT_FALSE(log.HasDetail("1"));
  EXPECT_EQ(log.Json(),
            "{\"developer_mode\":false,\"requests\":["
            "{\"method\":\"GET\",\"status\":404,\"url\":\"https://example.com/missing\"}]}");
}

TEST(PageLogRuleTest, ANewPageEmptiesIt) {
  Log log;
  log.NewPage(true);
  log.Add(Failing("https://example.com/missing", 404));
  log.Add(Message{.level = "error", .text = "old page"});
  log.NewPage(true);
  EXPECT_EQ(log.Json(), "{\"console\":[],\"developer_mode\":true,\"requests\":[]}");
}

TEST(PageLogRuleTest, TheCapsDropTheOldestAndSaySo) {
  Log log;
  log.NewPage(true);
  for (size_t each = 0; each <= kMostRequests; ++each) {
    log.Add(Failing(("https://example.com/" + std::to_string(each)).c_str(), 404));
  }
  for (size_t each = 0; each <= kMostMessages; ++each) {
    log.Add(Message{.level = "info", .text = "m" + std::to_string(each)});
  }
  const std::string answer = log.Json();
  EXPECT_EQ(answer.find("\"url\":\"https://example.com/0\""), std::string::npos);
  EXPECT_NE(answer.find("\"url\":\"https://example.com/200\""), std::string::npos);
  EXPECT_EQ(answer.find("\"text\":\"m0\""), std::string::npos);
  EXPECT_NE(answer.find("\"text\":\"m500\""), std::string::npos);
  EXPECT_NE(answer.find("\"dropped\":{\"console\":1,\"requests\":1}"), std::string::npos) << answer;
}

TEST(PageLogRuleTest, ALongMessageOrBodyIsCut) {
  Log log;
  log.NewPage(true);
  log.Add(Message{.level = "info", .text = std::string(kMostMessageBytes + 10, 'a')});
  log.Add(Failing("https://example.com/big", 500));
  log.Add("1", {.url = GURL("https://example.com/big"), .status = 500});
  log.Body("1", std::string(kMostBodyBytes + 10, 'b'), false);
  const std::string answer = log.Json();
  EXPECT_NE(answer.find(std::string(kMostMessageBytes, 'a') + "\""), std::string::npos);
  EXPECT_EQ(answer.find(std::string(kMostMessageBytes + 1, 'a')), std::string::npos);
  EXPECT_NE(answer.find(std::string(kMostBodyBytes, 'b') + "\""), std::string::npos);
  EXPECT_EQ(answer.find(std::string(kMostBodyBytes + 1, 'b')), std::string::npos);
}

TEST(PageLogRuleTest, TheWholeLogStaysUnderItsSize) {
  Log log;
  log.NewPage(true);
  for (size_t each = 0; each < kMostRequests; ++each) {
    const std::string url = "https://example.com/" + std::to_string(each);
    log.Add(Failing(url.c_str(), 500));
    log.Add(std::to_string(each), {.url = GURL(url), .status = 500});
    log.Body(std::to_string(each), std::string(kMostBodyBytes, 'b'), false);
  }
  EXPECT_LT(log.Json().size(), kMostBytes + 64 * 1024);
  // The failed requests themselves outlast their bodies.
  EXPECT_NE(log.Json().find("\"url\":\"https://example.com/0\""), std::string::npos);
}

}  // namespace
}  // namespace browser_core::page_log
