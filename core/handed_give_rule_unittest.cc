#include "browser_core/handed_give_rule.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::handed_give_rule {
namespace {

// A tab of the user's at a web page, in Today, pinned or a favorite, given to an agent's Space of
// its profile.
Tab Fit() {
  return {.users = true, .to_agent = true, .same_profile = true, .web_page = true};
}

TEST(HandedGiveRuleTest, ATabAtAWebPageIsGiven) {
  EXPECT_TRUE(MayGive(Fit()));
}

// An agent's own tab, and a private window's or another window's own.
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

// A pinned tab in a folder that fills itself, or in a folder inside one.
TEST(HandedGiveRuleTest, ATabInALiveFolderIsRefused) {
  Tab tab = Fit();
  tab.live_folder = true;
  EXPECT_FALSE(MayGive(tab));
}

// Two or three favorites shown side by side are one tile.
TEST(HandedGiveRuleTest, AFavoriteInASplitViewOfFavoritesIsRefused) {
  Tab tab = Fit();
  tab.favorite_split = true;
  EXPECT_FALSE(MayGive(tab));
}

// The tab that keeps a favorite's tile while an agent has the favorite's own.
TEST(HandedGiveRuleTest, ATabKeepingAHandedFavoritesTileIsRefused) {
  Tab tab = Fit();
  tab.stands_in = true;
  EXPECT_FALSE(MayGive(tab));
}

// A new tab's page, a file, an extension's page, or a page in the system's engine.
TEST(HandedGiveRuleTest, APageThatIsntHttpOrHttpsIsRefused) {
  Tab tab = Fit();
  tab.web_page = false;
  EXPECT_FALSE(MayGive(tab));
}

// A call goes with its tab.
TEST(HandedGiveRuleTest, ATabUsingTheCameraIsGiven) {
  Tab tab = Fit();
  tab.camera = true;
  EXPECT_TRUE(MayGive(tab));
}

TEST(HandedGiveRuleTest, ATabUsingTheMicrophoneIsGiven) {
  Tab tab = Fit();
  tab.microphone = true;
  EXPECT_TRUE(MayGive(tab));
}

TEST(HandedGiveRuleTest, ATabUsingBothIsGiven) {
  Tab tab = Fit();
  tab.camera = true;
  tab.microphone = true;
  EXPECT_TRUE(MayGive(tab));
}

// A share with the call refuses it still.
TEST(HandedGiveRuleTest, ATabInACallThatSharesTheScreenIsRefused) {
  Tab tab = Fit();
  tab.camera = true;
  tab.microphone = true;
  tab.screen = true;
  EXPECT_FALSE(MayGive(tab));
}

// The screen, a window or another tab.
TEST(HandedGiveRuleTest, ATabSharingTheScreenIsRefused) {
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

// A device it has open, or a scan for one.
TEST(HandedGiveRuleTest, ATabUsingADeviceIsRefused) {
  Tab tab = Fit();
  tab.device = true;
  EXPECT_FALSE(MayGive(tab));
}

TEST(HandedGiveRuleTest, ATabHoldingTheUsersFilesIsRefused) {
  Tab tab = Fit();
  tab.files = true;
  EXPECT_FALSE(MayGive(tab));
}

}  // namespace
}  // namespace browser_core::handed_give_rule
