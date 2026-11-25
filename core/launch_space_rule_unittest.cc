#include "browser_core/launch_space_rule.h"

#include <string>
#include <utility>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

LaunchSpace Users(std::string profile) {
  return {.agents = false, .profile = std::move(profile)};
}

LaunchSpace Agents(std::string profile) {
  return {.agents = true, .profile = std::move(profile)};
}

TEST(LaunchSpaceRule, TheUsersSpaceItShowedIsShownAgain) {
  EXPECT_EQ(SpaceShownAtLaunch({Users("a"), Users("a"), Agents("a")}, 1), 1u);
}

TEST(LaunchSpaceRule, AnAgentsSpaceGivesWayToTheUsersOfItsProfile) {
  // The user's first Space is another profile's: the one shown is the user's in the agent's.
  EXPECT_EQ(SpaceShownAtLaunch({Users("work"), Users("home"), Agents("home")}, 2), 1u);
}

TEST(LaunchSpaceRule, AnAgentsSpaceInAProfileWithNoneOfTheUsersGivesWayToTheUsersFirst) {
  EXPECT_EQ(SpaceShownAtLaunch({Agents("home"), Users("work"), Users("work")}, 0), 1u);
}

TEST(LaunchSpaceRule, WithNoneSavedTheUsersFirstIsShownPastAnAgents) {
  EXPECT_EQ(SpaceShownAtLaunch({Agents("a"), Users("a")}, std::nullopt), 1u);
  // A saved place past the Spaces put back is as none.
  EXPECT_EQ(SpaceShownAtLaunch({Agents("a"), Users("a")}, 5), 1u);
}

TEST(LaunchSpaceRule, OnlyWithNoneOfTheUsersIsAnAgentsTheAnswer) {
  EXPECT_EQ(SpaceShownAtLaunch({Agents("a"), Agents("a")}, 1), 1u);
  EXPECT_EQ(SpaceShownAtLaunch({Agents("a"), Agents("a")}, std::nullopt), 0u);
}

TEST(LaunchSpaceRule, NoSpacesShowsNone) {
  EXPECT_EQ(SpaceShownAtLaunch({}, std::nullopt), std::nullopt);
  EXPECT_EQ(SpaceShownAtLaunch({}, 0), std::nullopt);
}

}  // namespace
}  // namespace browser_core
