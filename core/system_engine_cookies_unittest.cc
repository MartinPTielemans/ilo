#include "browser_core/system_engine_cookies.h"

#include <string>
#include <vector>

#include "base/functional/bind.h"
#include "base/test/task_environment.h"
#include "base/time/time.h"
#include "net/cookies/canonical_cookie.h"
#include "net/cookies/cookie_constants.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::system_engine {
namespace {

const base::Time kSignedIn = base::Time::FromSecondsSinceUnixEpoch(1'800'000'000);

// A cookie of example.com's, made at |creation| and last set at |last_update|, as the stores give
// them: the system's has no last update of its own, so GetCookies gives it the copy's time.
net::CanonicalCookie Cookie(const std::string& name,
                            const std::string& value,
                            base::Time creation,
                            base::Time last_update,
                            const std::string& path = "/") {
  return *net::CanonicalCookie::CreateUnsafeCookieForTesting(
      name, value, ".example.com", path, creation, base::Time(), creation, last_update,
      /*secure=*/true, /*httponly=*/true, net::CookieSameSite::LAX_MODE,
      net::COOKIE_PRIORITY_DEFAULT, net::CookieSourceType::kHTTP);
}

std::vector<std::string> Values(const std::vector<net::CanonicalCookie>& cookies) {
  std::vector<std::string> values;
  for (const net::CanonicalCookie& cookie : cookies) {
    values.push_back(cookie.Name() + "=" + cookie.Value());
  }
  return values;
}

// The user signed in in the system's engine, and an agent's page at the site, signed out, had the
// site set its own session in Chromium before then: the copy into the system's engine keeps the
// user's.
TEST(SystemEngineCookiesTest, AnOlderChromiumCookieLeavesTheUsersSignIn) {
  const std::vector<net::CanonicalCookie> chromium = {
      Cookie("session", "anonymous", kSignedIn - base::Hours(1), kSignedIn - base::Hours(1))};
  const std::vector<net::CanonicalCookie> system = {
      Cookie("session", "user", kSignedIn, kSignedIn + base::Days(1))};
  EXPECT_TRUE(Newer(chromium, Store::kChromium, system).empty());
}

// The site gave the agent's page a new session since, made long ago and set again later: Chromium's
// last update counts, and the newer session goes over the system's.
TEST(SystemEngineCookiesTest, ANewerChromiumCookieGoesOver) {
  const std::vector<net::CanonicalCookie> chromium = {
      Cookie("session", "renewed", kSignedIn - base::Days(30), kSignedIn + base::Hours(1))};
  const std::vector<net::CanonicalCookie> system = {
      Cookie("session", "user", kSignedIn, base::Time::Now())};
  EXPECT_EQ(Values(Newer(chromium, Store::kChromium, system)),
            std::vector<std::string>{"session=renewed"});
}

// Into an agent's page the system's session goes over an older one in Chromium, and the system's
// own time is its creation, not the time GetCookies read it.
TEST(SystemEngineCookiesTest, TheSystemsNewerSessionGoesIntoChromium) {
  const std::vector<net::CanonicalCookie> system = {
      Cookie("session", "user", kSignedIn, base::Time::Now())};
  const std::vector<net::CanonicalCookie> older = {
      Cookie("session", "anonymous", kSignedIn - base::Hours(1), kSignedIn - base::Hours(1))};
  const std::vector<net::CanonicalCookie> newer = {
      Cookie("session", "renewed", kSignedIn, kSignedIn + base::Hours(1))};
  EXPECT_EQ(Values(Newer(system, Store::kSystem, older)), std::vector<std::string>{"session=user"});
  EXPECT_TRUE(Newer(system, Store::kSystem, newer).empty());
}

// A cookie the other store has none of goes, and one of another name or path is another cookie.
TEST(SystemEngineCookiesTest, OnlyAnEquivalentCookieKeepsItsPlace) {
  const std::vector<net::CanonicalCookie> chromium = {
      Cookie("session", "anonymous", kSignedIn - base::Hours(1), kSignedIn - base::Hours(1)),
      Cookie("theme", "dark", kSignedIn - base::Hours(1), kSignedIn - base::Hours(1)),
      Cookie("session", "app", kSignedIn - base::Hours(1), kSignedIn - base::Hours(1), "/app")};
  const std::vector<net::CanonicalCookie> system = {
      Cookie("session", "user", kSignedIn, kSignedIn)};
  EXPECT_EQ(Values(Newer(chromium, Store::kChromium, system)),
            (std::vector<std::string>{"theme=dark", "session=app"}));
}

// Set at the same moment, the cookie already there stays: nothing says the other is newer.
TEST(SystemEngineCookiesTest, ATieKeepsTheCookieThere) {
  const std::vector<net::CanonicalCookie> chromium = {
      Cookie("session", "agent", kSignedIn, kSignedIn)};
  const std::vector<net::CanonicalCookie> system = {
      Cookie("session", "user", kSignedIn, kSignedIn)};
  EXPECT_TRUE(Newer(chromium, Store::kChromium, system).empty());
}

// The agent's open or navigate, and the give's load, which the app let through before they load.
constexpr Arrival kOpened = {.browser_initiated = true,
                             .history = false,
                             .started_at_site = true,
                             .at_site = false};

TEST(SystemEngineCookiesTest, AnOpenTheAppLetThroughTakesTheSession) {
  EXPECT_TRUE(CopiesSession(kOpened));
}

// A link, form or script in the agent's page: the fence judges it only once it is there, so a site
// off limits to agents gets no session from it.
TEST(SystemEngineCookiesTest, APagesLinkTakesNoSession) {
  Arrival link = kOpened;
  link.browser_initiated = false;
  EXPECT_FALSE(CopiesSession(link));
}

TEST(SystemEngineCookiesTest, BackForwardOrARestoreTakesNoSession) {
  Arrival back = kOpened;
  back.history = true;
  EXPECT_FALSE(CopiesSession(back));
}

TEST(SystemEngineCookiesTest, ARedirectFromAnotherSiteTakesNoSession) {
  Arrival redirected = kOpened;
  redirected.started_at_site = false;
  EXPECT_FALSE(CopiesSession(redirected));
}

// Within a visit the page has the session already: once a visit.
TEST(SystemEngineCookiesTest, ALoadWithinTheSiteCopiesNothingMore) {
  Arrival again = kOpened;
  again.at_site = true;
  EXPECT_FALSE(CopiesSession(again));
}

// A store that never answers: the navigation goes on, without the session, at kWait.
TEST(SystemEngineCookiesTest, NoAnswerGoesOnAtTheWait) {
  base::test::TaskEnvironment tasks(base::test::TaskEnvironment::TimeSource::MOCK_TIME);
  StoreWait wait;
  std::vector<bool> went;
  wait.Start(base::BindOnce(
      [](std::vector<bool>* went, bool answered) { went->push_back(answered); }, &went));
  tasks.FastForwardBy(StoreWait::kWait - base::Milliseconds(1));
  EXPECT_TRUE(went.empty());
  tasks.FastForwardBy(base::Milliseconds(1));
  EXPECT_EQ(went, std::vector<bool>{false});
  // The copy ending late changes nothing.
  wait.Answered();
  EXPECT_EQ(went, std::vector<bool>{false});
}

TEST(SystemEngineCookiesTest, AnAnswerGoesOnOnceAndEndsTheTimer) {
  base::test::TaskEnvironment tasks(base::test::TaskEnvironment::TimeSource::MOCK_TIME);
  StoreWait wait;
  std::vector<bool> went;
  wait.Start(base::BindOnce(
      [](std::vector<bool>* went, bool answered) { went->push_back(answered); }, &went));
  tasks.FastForwardBy(base::Seconds(1));
  wait.Answered();
  EXPECT_EQ(went, std::vector<bool>{true});
  tasks.FastForwardBy(StoreWait::kWait * 2);
  EXPECT_EQ(went, std::vector<bool>{true});
  EXPECT_EQ(tasks.GetPendingMainThreadTaskCount(), 0u);
}

}  // namespace
}  // namespace browser_core::system_engine
