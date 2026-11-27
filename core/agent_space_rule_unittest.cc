#include "browser_core/agent_space_rule.h"

#include <memory>
#include <vector>

#include "base/functional/bind.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::agent_space_rule {
namespace {

constexpr Ask kAsks[] = {Ask::kPaste,  Ask::kCopy,        Ask::kFilePicker,
                         Ask::kDevice, Ask::kLinkPreview, Ask::kPictureInPicture};

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

// And what they copy there is theirs to copy.
TEST(AgentSpaceRuleTest, AHeldTabTakesTheUsersCopy) {
  EXPECT_FALSE(Refuses(Ask::kCopy, {.agents = true, .held = true}));
}

// A file picked while they hold the tab would stay the page's once the agent has it again.
TEST(AgentSpaceRuleTest, AHeldTabStillGetsNoFilePicker) {
  EXPECT_TRUE(Refuses(Ask::kFilePicker, {.agents = true, .held = true}));
}

// A device opened while they hold the tab would stay open once the agent has it again.
TEST(AgentSpaceRuleTest, AHeldTabStillGetsNoDevice) {
  EXPECT_TRUE(Refuses(Ask::kDevice, {.agents = true, .held = true}));
}

// A window opened while they hold the tab would stay on their screen once the agent has it again.
TEST(AgentSpaceRuleTest, AHeldTabStillGetsNoPictureInPicture) {
  EXPECT_TRUE(Refuses(Ask::kPictureInPicture, {.agents = true, .held = true}));
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

TEST(AgentSpaceRuleTest, ATabWithNowhereElseGoesToASpaceOfTheUsersInItsProfile) {
  EXPECT_TRUE(TakesATabWithNowhereElse({.of_the_tabs_profile = true}));
}

TEST(AgentSpaceRuleTest, ATabWithNowhereElseNeverGoesToAnAgentsSpace) {
  EXPECT_FALSE(TakesATabWithNowhereElse({.agents = true, .of_the_tabs_profile = true}));
}

// A Little window's or a Blank Window's.
TEST(AgentSpaceRuleTest, ATabWithNowhereElseNeverGoesToAWindowsOwnSpace) {
  EXPECT_FALSE(TakesATabWithNowhereElse({.a_windows_own = true, .of_the_tabs_profile = true}));
}

TEST(AgentSpaceRuleTest, ATabWithNowhereElseNeverGoesToAnotherProfilesSpace) {
  EXPECT_FALSE(TakesATabWithNowhereElse({}));
}

// An agent has a Space in each profile it works in (#4376): a tab asked for in a profile goes to
// that profile's Space, never to another profile's, and another agent's Space is never its.
TEST(AgentSpaceRuleTest, AnAgentsTabGoesToItsSpaceInTheProfileAsked) {
  const std::vector<AgentSpace> spaces = {
      {.agent = "app", .profile = "Default"},
      {.agent = "other", .profile = "Profile 2"},
      {.agent = "app", .profile = "Profile 2"},
  };
  EXPECT_EQ(AgentSpaceIn(spaces, "app", "Default"), 0);
  EXPECT_EQ(AgentSpaceIn(spaces, "app", "Profile 2"), 2);
  EXPECT_EQ(AgentSpaceIn(spaces, "other", "Default"), -1);
  EXPECT_EQ(AgentSpaceIn(spaces, "app", "Profile 3"), -1);
  EXPECT_EQ(AgentSpaceIn(spaces, "", "Default"), -1);
}

// One Space a profile: the first made there is the agent's, so a second call finds it.
TEST(AgentSpaceRuleTest, TheFirstSpaceInAProfileIsTheAgents) {
  const std::vector<AgentSpace> spaces = {
      {.agent = "app", .profile = "Default"},
      {.agent = "app", .profile = "Default"},
  };
  EXPECT_EQ(AgentSpaceIn(spaces, "app", "Default"), 0);
}

// With no profile named, the agent's Space is found as before profiles were: the last made, which
// for a thread's agent is its one Space, in whatever profile it was made.
TEST(AgentSpaceRuleTest, NoProfileFindsTheLastMade) {
  const std::vector<AgentSpace> spaces = {
      {.agent = "task-1", .profile = "Profile 2"},
      {.agent = "app", .profile = "Default"},
  };
  EXPECT_EQ(AgentSpaceIn(spaces, "task-1", ""), 0);
  EXPECT_EQ(AgentSpaceIn(spaces, "app", ""), 1);
  EXPECT_EQ(AgentSpaceIn({}, "app", ""), -1);
}

// A Space's tab opens only in its own profile: never in the one Chrome gives while it hasn't
// loaded, whose sign-ins aren't the ones the user picked.
TEST(AgentSpaceRuleTest, ATabOpensOnlyInItsSpacesOwnProfile) {
  EXPECT_TRUE(OpensIn("Profile 2", "Profile 2"));
  EXPECT_FALSE(OpensIn("Profile 2", "Default"));
  EXPECT_FALSE(OpensIn("", ""));
}

// The pick goes to the profile that visited the site last (#4376).
TEST(AgentSpaceRuleTest, ThePickIsTheProfileThatVisitedTheSiteLast) {
  EXPECT_EQ(Picked({{.profile = 1, .order = 0, .loaded = true, .last_visit = 100},
                    {.profile = 2, .order = 1, .loaded = true, .last_visit = 300},
                    {.profile = 3, .order = 2, .loaded = true, .last_visit = 200}}),
            2u);
}

// A tie goes to the profile given first, whichever history answered first.
TEST(AgentSpaceRuleTest, ATieGoesToTheProfileGivenFirst) {
  EXPECT_EQ(Picked({{.profile = 3, .order = 1, .loaded = true, .last_visit = 100},
                    {.profile = 2, .order = 0, .loaded = true, .last_visit = 100}}),
            2u);
}

// No profile that visited the site is no pick, and the app's window decides.
TEST(AgentSpaceRuleTest, NoVisitIsNoPick) {
  EXPECT_EQ(Picked({}), 0u);
  EXPECT_EQ(Picked({{.profile = 1, .loaded = true, .last_visit = 0}}), 0u);
}

// A private session is never picked, however recent its visit, and neither is a profile whose
// history hasn't loaded: the pick never loads one to look.
TEST(AgentSpaceRuleTest, APrivateOrUnloadedProfileIsNeverPicked) {
  EXPECT_EQ(Picked({{.profile = 1, .order = 0, .loaded = true, .last_visit = 100},
                    {.profile = 2, .order = 1, .loaded = true, .private_session = true,
                     .last_visit = 900},
                    {.profile = 3, .order = 2, .loaded = false, .last_visit = 800}}),
            1u);
  EXPECT_EQ(Picked({{.profile = 2, .loaded = true, .private_session = true, .last_visit = 9}}),
            0u);
}

TEST(AgentSpaceRuleTest, APickReadsThePagesHostItsSiteAndTheSitesWwwHostOnce) {
  EXPECT_EQ(PickHosts("mail.example.com", "example.com"),
            (std::vector<std::string>{"mail.example.com", "example.com", "www.example.com"}));
  EXPECT_EQ(PickHosts("www.example.com", "example.com"),
            (std::vector<std::string>{"www.example.com", "example.com"}));
  EXPECT_EQ(PickHosts("localhost", "localhost"),
            (std::vector<std::string>{"localhost", "www.localhost"}));
  EXPECT_TRUE(PickHosts("", "").empty());
}

TEST(AgentSpaceRuleTest, AProfilesAnswerComesOnceWithTheVisitItRead) {
  std::vector<int64_t> heard;
  {
    PickAnswer answer(base::BindOnce([](std::vector<int64_t>* heard,
                                        int64_t last) { heard->push_back(last); },
                                     &heard));
    answer.Answer(42);
    answer.Answer(7);
  }
  EXPECT_EQ(heard, std::vector<int64_t>{42});
}

// A profile whose history shuts down mid-read drops its task unrun: the pick still ends.
TEST(AgentSpaceRuleTest, AReadDroppedUnansweredAnswersNone) {
  std::vector<int64_t> heard;
  auto answer = std::make_unique<PickAnswer>(base::BindOnce(
      [](std::vector<int64_t>* heard, int64_t last) { heard->push_back(last); }, &heard));
  answer.reset();
  EXPECT_EQ(heard, std::vector<int64_t>{0});
}

}  // namespace
}  // namespace browser_core::agent_space_rule
