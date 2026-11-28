#include "browser_core/agent_tabs_close_rule.h"

#include <limits>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::agent_tabs_close_rule {
namespace {

const base::Time kStart = base::Time::FromSecondsSinceUnixEpoch(1'800'000'000);
const std::optional<base::TimeDelta> kFifteen = base::Minutes(15);

TEST(AgentTabsCloseRuleTest, TheDefaultIsFifteenMinutesAndNeverIsNone) {
  EXPECT_EQ(After(kDefault), base::Minutes(15));
  EXPECT_FALSE(After(0));
}

TEST(AgentTabsCloseRuleTest, ATimeIsTakenAsThePopUpsNearest) {
  EXPECT_EQ(Choice(900), 900);
  EXPECT_EQ(Choice(400), 300);
  EXPECT_EQ(Choice(100), 0);
  EXPECT_EQ(Choice(-1), 0);
  EXPECT_EQ(Choice(100000), 7200);
  EXPECT_EQ(Choice(std::numeric_limits<double>::quiet_NaN()), kDefault);
}

// A thread nothing has happened in has no time, so no timer is armed for it.
TEST(AgentTabsCloseRuleTest, NoActivityNoTime) {
  Clock clock;
  EXPECT_FALSE(clock.DueAt(kFifteen));
}

TEST(AgentTabsCloseRuleTest, ItCountsFromTheLastActivity) {
  Clock clock;
  clock.Acted(kStart);
  EXPECT_EQ(clock.DueAt(kFifteen), kStart + base::Minutes(15));
  clock.Acted(kStart + base::Minutes(10));
  EXPECT_EQ(clock.DueAt(kFifteen), kStart + base::Minutes(25));
  EXPECT_FALSE(clock.Due(kStart + base::Minutes(24), kFifteen));
  EXPECT_TRUE(clock.Due(kStart + base::Minutes(25), kFifteen));
}

// A run that thinks for longer than the time without touching a page keeps its tabs, and the
// count starts again when it ends.
TEST(AgentTabsCloseRuleTest, NothingCountsWhileARunIsUnderWay) {
  Clock clock;
  clock.Acted(kStart);
  clock.SetWorking(true, kStart);
  EXPECT_FALSE(clock.DueAt(kFifteen));
  EXPECT_FALSE(clock.Due(kStart + base::Hours(3), kFifteen));
  clock.SetWorking(false, kStart + base::Hours(3));
  EXPECT_EQ(clock.DueAt(kFifteen), kStart + base::Hours(3) + base::Minutes(15));
}

// The app names its idle threads at launch, and their tabs count from then.
TEST(AgentTabsCloseRuleTest, AThreadNamedIdleCountsFromThen) {
  Clock clock;
  clock.SetWorking(false, kStart);
  EXPECT_EQ(clock.DueAt(kFifteen), kStart + base::Minutes(15));
  // Named again, it doesn't start over.
  clock.SetWorking(false, kStart + base::Minutes(5));
  EXPECT_EQ(clock.DueAt(kFifteen), kStart + base::Minutes(15));
}

TEST(AgentTabsCloseRuleTest, NeverIsNeverDue) {
  Clock clock;
  clock.Acted(kStart);
  EXPECT_FALSE(clock.DueAt(After(0)));
  EXPECT_FALSE(clock.Due(kStart + base::Days(30), After(0)));
}

// A change of the setting gives a time from the same last activity, sooner or later, and one
// already past is due at once.
TEST(AgentTabsCloseRuleTest, AChangedSettingRearmsFromTheLastActivity) {
  Clock clock;
  clock.Acted(kStart);
  EXPECT_EQ(clock.DueAt(After(3600)), kStart + base::Hours(1));
  EXPECT_EQ(clock.DueAt(After(300)), kStart + base::Minutes(5));
  EXPECT_TRUE(clock.Due(kStart + base::Minutes(10), After(300)));
}

TEST(AgentTabsCloseRuleTest, OnlyTheThreadsOwnTabsClose) {
  EXPECT_TRUE(ClosesNow({}));
  EXPECT_FALSE(Own({.handed = true}));
  EXPECT_FALSE(Own({.held = true}));
  EXPECT_FALSE(Own({.pinned = true}));
  EXPECT_FALSE(ClosesNow({.handed = true}));
  EXPECT_FALSE(ClosesNow({.held = true}));
  EXPECT_FALSE(ClosesNow({.pinned = true}));
}

// One a window shows waits until it's hidden, then closes if no activity came since.
TEST(AgentTabsCloseRuleTest, AShownTabClosesOnceHidden) {
  EXPECT_TRUE(Own({.shown = true}));
  EXPECT_FALSE(ClosesNow({.shown = true}));
  Clock clock;
  clock.Acted(kStart);
  const base::Time hidden = kStart + base::Minutes(20);
  EXPECT_TRUE(clock.Due(hidden, kFifteen) && ClosesNow({.shown = false}));
  clock.Acted(kStart + base::Minutes(18));
  EXPECT_FALSE(clock.Due(hidden, kFifteen));
}

}  // namespace
}  // namespace browser_core::agent_tabs_close_rule
