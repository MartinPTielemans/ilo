#include "browser_core/handed_give_rule.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::handed_give_rule {
namespace {

// A Today tab of the user's at a web page, given to an agent's Space of its profile.
Tab Fit() {
  return {.users = true, .to_agent = true, .same_profile = true, .web_page = true};
}

TEST(HandedGiveRuleTest, ATodayTabAtAWebPageIsGiven) {
  EXPECT_TRUE(MayGive(Fit()));
}

// An agent's own tab, a private window's or another window's own, and a favorite, in no Space.
TEST(HandedGiveRuleTest, OnlyATabOfTheUsersListedSpaces) {
  Tab tab = Fit();
  tab.users = false;
  EXPECT_FALSE(MayGive(tab));
}

TEST(HandedGiveRuleTest, OnlyIntoAnAgentsSpaceOfTheSameProfile) {
  Tab tab = Fit();
  tab.to_agent = false;
  EXPECT_FALSE(MayGive(tab));
  tab = Fit();
  tab.same_profile = false;
  EXPECT_FALSE(MayGive(tab));
}

TEST(HandedGiveRuleTest, APinnedTabIsRefused) {
  Tab tab = Fit();
  tab.pinned = true;
  EXPECT_FALSE(MayGive(tab));
}

TEST(HandedGiveRuleTest, ATabInASplitViewIsRefused) {
  Tab tab = Fit();
  tab.split = true;
  EXPECT_FALSE(MayGive(tab));
}

// A new tab's page, a file, an extension's page, or a page in the system's engine.
TEST(HandedGiveRuleTest, APageThatIsntHttpOrHttpsIsRefused) {
  Tab tab = Fit();
  tab.web_page = false;
  EXPECT_FALSE(MayGive(tab));
}

TEST(HandedGiveRuleTest, ATabUsingTheCameraIsRefused) {
  Tab tab = Fit();
  tab.camera = true;
  EXPECT_FALSE(MayGive(tab));
}

TEST(HandedGiveRuleTest, ATabUsingTheMicrophoneIsRefused) {
  Tab tab = Fit();
  tab.microphone = true;
  EXPECT_FALSE(MayGive(tab));
}

TEST(HandedGiveRuleTest, ATabUsingTheScreenIsRefused) {
  Tab tab = Fit();
  tab.screen = true;
  EXPECT_FALSE(MayGive(tab));
}

// The answer would come once the tab is the agent's.
TEST(HandedGiveRuleTest, ATabWhosePageAsksForAPermissionIsRefused) {
  Tab tab = Fit();
  tab.permission_asked = true;
  EXPECT_FALSE(MayGive(tab));
}

// An alert, a confirm, a prompt or the question whether to leave the page.
TEST(HandedGiveRuleTest, ATabWithAPagesDialogUpIsRefused) {
  Tab tab = Fit();
  tab.dialog = true;
  EXPECT_FALSE(MayGive(tab));
}

// Filled in already or not: the form's button would sign the agent in.
TEST(HandedGiveRuleTest, ATabWhoseSignInFormHasASavedLoginIsRefused) {
  Tab tab = Fit();
  tab.saved_login = true;
  EXPECT_FALSE(MayGive(tab));
}

}  // namespace
}  // namespace browser_core::handed_give_rule
