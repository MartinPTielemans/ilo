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

// The app tells the hand-back's entry from a step's by its action alone, so the two differ.
TEST(SecureConnectionsRuleTest, TheHandBacksEntryHasAnActionOfItsOwn) {
  EXPECT_STREQ(kHandedBackAction, "handed_back_page");
  EXPECT_STRNE(kHandedBackAction, kEntryAction);
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
