#include "browser_core/secure_connections_rule.h"

#include "base/feature_list.h"
#include "browser_core/public/browser_core.h"
#include "chrome/common/chrome_features.h"
#include "components/security_interstitials/core/features.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::secure_connections_rule {
namespace {

TEST(SecureConnectionsRuleTest, TheRowIsOnWhereverTheProfileWarns) {
  EXPECT_FALSE(IsOn(Mode::kOff));
  EXPECT_TRUE(IsOn(Mode::kBalanced));
  EXPECT_TRUE(IsOn(Mode::kStrict));
}

TEST(SecureConnectionsRuleTest, OnFromOffIsTheBalancedMode) {
  EXPECT_EQ(After(Mode::kOff, true), Mode::kBalanced);
}

// The row shows on for both, so turning it on changes neither: the stricter choice isn't lost to a
// click on a row that already read on.
TEST(SecureConnectionsRuleTest, OnWhileItWarnsAlreadyChangesNothing) {
  EXPECT_EQ(After(Mode::kBalanced, true), Mode::kBalanced);
  EXPECT_EQ(After(Mode::kStrict, true), Mode::kStrict);
}

TEST(SecureConnectionsRuleTest, OffIsOffFromEither) {
  EXPECT_EQ(After(Mode::kOff, false), Mode::kOff);
  EXPECT_EQ(After(Mode::kBalanced, false), Mode::kOff);
  EXPECT_EQ(After(Mode::kStrict, false), Mode::kOff);
}

TEST(SecureConnectionsRuleTest, ATabOfTheUsersHasTheWarning) {
  EXPECT_FALSE(NoWarning({.agents = false, .held = false}));
  EXPECT_FALSE(NoWarning({.agents = false, .held = true}));
}

// Its Continue to site would be nobody's to press but the agent's.
TEST(SecureConnectionsRuleTest, AnAgentsTabNeverHasTheWarning) {
  EXPECT_TRUE(NoWarning({.agents = true, .held = false}));
}

// At a step the user does themselves the warning is theirs to answer, as in a tab of their own.
TEST(SecureConnectionsRuleTest, ATabTheUserHoldsHasTheWarning) {
  EXPECT_FALSE(NoWarning({.agents = true, .held = true}));
}

TEST(SecureConnectionsRuleTest, ATabWithAPageBehindGoesBackToIt) {
  EXPECT_EQ(PutBackTo(/*has_a_page_behind=*/true), PutBack::kBack);
}

TEST(SecureConnectionsRuleTest, ATabWithNoPageBehindCloses) {
  EXPECT_EQ(PutBackTo(/*has_a_page_behind=*/false), PutBack::kClosed);
}

// The two codes the app knows the entry by: neither holds a word of the page's address.
TEST(SecureConnectionsRuleTest, TheEntryIsKnownByItsTwoCodes) {
  EXPECT_STREQ(kEntryAction, "open_page");
  EXPECT_STREQ(kEntryReason, "not_secure");
}

// A reader of the log tells the hand-back's entry from a step's by its action alone.
TEST(SecureConnectionsRuleTest, TheHandBacksEntryHasAnActionOfItsOwn) {
  EXPECT_STREQ(kHandedBackAction, "handed_back_page");
  EXPECT_STRNE(kHandedBackAction, kEntryAction);
}

// The app asks an open's or a navigate's wait right after the call, so the wait has its load.
TEST(SecureConnectionsRuleTest, TheWaitAfterAnOpenOrANavigateHasItsLoad) {
  Asked kept = {.load = 7, .opened = true};
  const Asked job = TakenBy(kept, /*waits=*/true);
  EXPECT_EQ(job.load, 7);
  EXPECT_TRUE(job.opened);
}

// So only one job is ever told of a load, and it is answered for it once.
TEST(SecureConnectionsRuleTest, ALoadIsOneJobsOnly) {
  Asked kept = {.load = 7};
  TakenBy(kept, /*waits=*/true);
  EXPECT_EQ(kept.load, 0);
  EXPECT_EQ(TakenBy(kept, /*waits=*/true).load, 0);
}

// A click that came first isn't the load's job, and the wait after it has no older load's id.
TEST(SecureConnectionsRuleTest, AJobThatIsntTheWaitDropsTheLoad) {
  Asked kept = {.load = 7};
  EXPECT_EQ(TakenBy(kept, /*waits=*/false).load, 0);
  EXPECT_EQ(kept.load, 0);
  EXPECT_EQ(TakenBy(kept, /*waits=*/true).load, 0);
}

TEST(SecureConnectionsRuleTest, TheJobThatAskedForTheStoppedLoadIsToldSo) {
  EXPECT_TRUE(AskedFor({.load = 7}, 7));
}

// A click whose page then went to http, a type, a wait: its page left under it, and it goes on.
TEST(SecureConnectionsRuleTest, AJobThatAskedForNoLoadIsNeverToldSo) {
  EXPECT_FALSE(AskedFor({}, 7));
}

// As a page that opened and then went on to http by itself: another navigation than the one asked.
TEST(SecureConnectionsRuleTest, AJobThatAskedForAnotherLoadIsNeverToldSo) {
  EXPECT_FALSE(AskedFor({.load = 7}, 8));
}

// A page left at a Hand Back was the user's, and no job's.
TEST(SecureConnectionsRuleTest, APageNoNavigationOfAJobsOpenedEndsNoJob) {
  EXPECT_FALSE(AskedFor({}, 0));
  EXPECT_FALSE(AskedFor({.load = 7}, 0));
}

// The two sentences the app has for a tab that couldn't open or couldn't go, and no third.
TEST(SecureConnectionsRuleTest, TheJobIsToldInTheAppsOwnTwoSentences) {
  EXPECT_STREQ(NotOpened(/*opened=*/true), "The tab couldn't open.");
  EXPECT_STREQ(NotOpened(/*opened=*/false), "The tab couldn't go there.");
}

// A click, a key, a typed Enter or a choice went, so an answer that reads as one that didn't would
// have the agent do it again.
TEST(SecureConnectionsRuleTest, AStepThatWentIsToldSoAsItsTabIsClosedAsNotSecure) {
  EXPECT_EQ(ToldAtClose(/*closed_as_not_secure=*/true, /*running=*/true, {.went = true}),
            AtClose::kWent);
}

// Its press or its choice never reached the page, as one still waiting for its element, a type
// still typing before its Enter, or a select that only opened its list: it didn't go.
TEST(SecureConnectionsRuleTest, AStepNotYetSentIsToldTheTabsEnd) {
  EXPECT_EQ(ToldAtClose(/*closed_as_not_secure=*/true, /*running=*/true, {.went = false}),
            AtClose::kTabEnded);
}

// A step is no wait: one that went is told so whatever load the tab's last open had.
TEST(SecureConnectionsRuleTest, AStepThatWentIsToldSoBeforeAnyWaitsAnswer) {
  const Running step = {.went = true, .waits = false, .asked = {.load = 7, .opened = true}};
  EXPECT_EQ(ToldAtClose(/*closed_as_not_secure=*/true, /*running=*/true, step), AtClose::kWent);
}

// The page it opened was there, and went on by itself while the open still waited for it.
TEST(SecureConnectionsRuleTest, AnOpensWaitIsToldItsPageWentOn) {
  const Running wait = {.waits = true, .asked = {.load = 7, .opened = true}};
  EXPECT_EQ(ToldAtClose(/*closed_as_not_secure=*/true, /*running=*/true, wait), AtClose::kWentOn);
}

// A navigate's tab has the page it left behind it and is put back; a wait asked later has no load.
TEST(SecureConnectionsRuleTest, AWaitThatIsntAnOpensIsToldTheTabsEnd) {
  const Running navigates = {.waits = true, .asked = {.load = 7, .opened = false}};
  EXPECT_EQ(ToldAtClose(/*closed_as_not_secure=*/true, /*running=*/true, navigates),
            AtClose::kTabEnded);
  const Running later = {.waits = true};
  EXPECT_EQ(ToldAtClose(/*closed_as_not_secure=*/true, /*running=*/true, later),
            AtClose::kTabEnded);
}

// A type with no Enter, a scroll, a read: there is no answer of their own for them.
TEST(SecureConnectionsRuleTest, EveryOtherJobRunningIsToldTheTabsEnd) {
  EXPECT_EQ(ToldAtClose(/*closed_as_not_secure=*/true, /*running=*/true, {}), AtClose::kTabEnded);
}

// It never began, whatever it was to be.
TEST(SecureConnectionsRuleTest, AJobStillQueuedIsToldTheTabsEnd) {
  const Running wait = {.waits = true, .asked = {.load = 7, .opened = true}};
  EXPECT_EQ(ToldAtClose(/*closed_as_not_secure=*/true, /*running=*/false, wait),
            AtClose::kTabEnded);
  EXPECT_EQ(ToldAtClose(/*closed_as_not_secure=*/true, /*running=*/false, {.went = true}),
            AtClose::kTabEnded);
}

// The user's close, the agent's own, a close of the core's that didn't go: none says a page went
// on to http, to a step that went or to an open's wait.
TEST(SecureConnectionsRuleTest, ATabThatClosesAnyOtherWayTellsEveryJobTheTabsEnd) {
  const Running wait = {.waits = true, .asked = {.load = 7, .opened = true}};
  EXPECT_EQ(ToldAtClose(/*closed_as_not_secure=*/false, /*running=*/true, {.went = true}),
            AtClose::kTabEnded);
  EXPECT_EQ(ToldAtClose(/*closed_as_not_secure=*/false, /*running=*/true, wait),
            AtClose::kTabEnded);
}

// The object the header promises the app, which reads it by its one key.
TEST(SecureConnectionsRuleTest, TheClicksAnswerIsTheHeadersObject) {
  EXPECT_STREQ(kClosedAnswer, R"({"closed":"not_secure"})");
}

// An entry about the page its tab shows names no site. Every other entry the core sends is built
// as this one, by its fields' names without the site, which leaves the site null.
TEST(SecureConnectionsRuleTest, AnEntryBuiltWithoutASiteNamesNone) {
  const bc_agent_activity entry = {
      .agent = "claude-code",
      .tab = 1,
      .action = "act",
      .refused = "",
      .failure = BC_AGENT_FAILURE_NONE,
  };
  EXPECT_EQ(entry.site, nullptr);
}

// What a profile that never chose has, which the header promises as on (bc_profile_secure_
// connections) and no pref of ours sets: the engine's own defaults. A new major that turns one
// off stops here, before a profile is quietly left without the warning. The first three make the
// balanced mode what such a profile has; the fourth has a private window warn too; and the last
// has the warning as a dialog of the browser's, where no page's input reaches Continue to site,
// and not as a page, whose button a thread's click would press.
TEST(SecureConnectionsRuleTest, AProfileThatNeverChoseWarnsByTheEnginesDefaults) {
  EXPECT_EQ(features::kHttpsUpgrades.default_state, base::FEATURE_ENABLED_BY_DEFAULT);
  EXPECT_EQ(features::kHttpsFirstBalancedMode.default_state, base::FEATURE_ENABLED_BY_DEFAULT);
  EXPECT_EQ(features::kHttpsFirstBalancedModeAutoEnable.default_state,
            base::FEATURE_ENABLED_BY_DEFAULT);
  EXPECT_EQ(features::kHttpsFirstModeIncognito.default_state, base::FEATURE_ENABLED_BY_DEFAULT);
  EXPECT_EQ(security_interstitials::features::kHttpsFirstDialogUi.default_state,
            base::FEATURE_ENABLED_BY_DEFAULT);
}

}  // namespace
}  // namespace browser_core::secure_connections_rule
