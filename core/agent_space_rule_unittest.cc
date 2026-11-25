#include "browser_core/agent_space_rule.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::agent_space_rule {
namespace {

constexpr Ask kAsks[] = {Ask::kPaste, Ask::kFilePicker, Ask::kDevice, Ask::kLinkPreview};

TEST(AgentSpaceRuleTest, ATabOfTheUsersIsRefusedNothing) {
  for (Ask ask : kAsks) {
    EXPECT_FALSE(Refuses(ask, {.agents = false, .held = false}));
    EXPECT_FALSE(Refuses(ask, {.agents = false, .held = true}));
  }
}

TEST(AgentSpaceRuleTest, APageInAnAgentsSpaceIsRefusedEachOfThem) {
  for (Ask ask : kAsks) {
    EXPECT_TRUE(Refuses(ask, {.agents = true, .held = false}));
  }
}

// At a step the user does themselves, what they paste is theirs to paste.
TEST(AgentSpaceRuleTest, AHeldTabTakesTheUsersPaste) {
  EXPECT_FALSE(Refuses(Ask::kPaste, {.agents = true, .held = true}));
}

// A file picked while they hold the tab would stay the page's once the agent has it again.
TEST(AgentSpaceRuleTest, AHeldTabStillGetsNoFilePicker) {
  EXPECT_TRUE(Refuses(Ask::kFilePicker, {.agents = true, .held = true}));
}

// A device opened while they hold the tab would stay open once the agent has it again.
TEST(AgentSpaceRuleTest, AHeldTabStillGetsNoDevice) {
  EXPECT_TRUE(Refuses(Ask::kDevice, {.agents = true, .held = true}));
}

// A link the user clicks at a step they do themselves opens in a preview as in a tab of theirs.
TEST(AgentSpaceRuleTest, AHeldTabOpensTheUsersLinkPreview) {
  EXPECT_FALSE(Refuses(Ask::kLinkPreview, {.agents = true, .held = true}));
}

TEST(AgentSpaceRuleTest, ThePageMenuHasNoLinkPreviewRowInAnAgentsSpace) {
  EXPECT_FALSE(MenuHasLinkPreviewRow(/*web_link=*/true, {.agents = true, .held = false}));
}

TEST(AgentSpaceRuleTest, ThePageMenuHasTheLinkPreviewRowForAWebLinkWhereTheClickOpensOne) {
  EXPECT_TRUE(MenuHasLinkPreviewRow(/*web_link=*/true, {.agents = false, .held = false}));
  EXPECT_TRUE(MenuHasLinkPreviewRow(/*web_link=*/true, {.agents = true, .held = true}));
  EXPECT_FALSE(MenuHasLinkPreviewRow(/*web_link=*/false, {.agents = false, .held = false}));
}

TEST(AgentSpaceRuleTest, ATabMadeForASpaceIsListedThere) {
  // Wherever its opener is by then, and whichever Space the window shows.
  EXPECT_EQ(Listed::kInTheSpaceItWasMadeFor, ListedIn({.made_for_a_space = true,
                                                       .its_space_is_there = true,
                                                       .opener_in_users_space = true,
                                                       .window_shows_users_space = true,
                                                       .profile_has_users_space = true}));
}

TEST(AgentSpaceRuleTest, ATabWhoseSpaceIsGoneIsClosed) {
  EXPECT_EQ(Listed::kClosed, ListedIn({.made_for_a_space = true,
                                       .its_space_is_there = false,
                                       .opener_in_users_space = true,
                                       .window_shows_users_space = true,
                                       .profile_has_users_space = true}));
}

TEST(AgentSpaceRuleTest, ATabAUsersPageOpenedIsListedInItsOpenersSpace) {
  EXPECT_EQ(Listed::kInItsOpenersSpace, ListedIn({.opener_in_users_space = true,
                                                  .window_shows_users_space = false,
                                                  .profile_has_users_space = true}));
}

// One with no opener, or whose opener is in an agent's Space by now.
TEST(AgentSpaceRuleTest, ATabWithNoOpenerOfTheUsersIsListedInTheUsersSpaceTheWindowShows) {
  EXPECT_EQ(Listed::kInTheSpaceShown,
            ListedIn({.window_shows_users_space = true, .profile_has_users_space = true}));
}

// The window shows an agent's Space: the tab isn't the agent's for that.
TEST(AgentSpaceRuleTest, ATabOpenedWhileTheWindowShowsAnAgentsSpaceIsListedInTheUsersFirst) {
  EXPECT_EQ(Listed::kInTheUsersFirstSpace, ListedIn({.profile_has_users_space = true}));
}

TEST(AgentSpaceRuleTest, ATabWithNoSpaceOfTheUsersToGoToIsClosed) {
  EXPECT_EQ(Listed::kClosed, ListedIn({}));
}

}  // namespace
}  // namespace browser_core::agent_space_rule
