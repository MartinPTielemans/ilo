#include "browser_core/arrange_rule.h"

#include <string>
#include <vector>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::arrange_rule {
namespace {

TEST(ArrangeRule, AWebPageOfTheUsersOwnArranges) {
  EXPECT_TRUE(MayStart("example.com", /*off_the_record=*/false, /*in_agent_space=*/false));
}

TEST(ArrangeRule, ATabInAnAgentsSpaceNeverArranges) {
  // A task tab, and one the user took over that's still held there, alike.
  EXPECT_FALSE(MayStart("example.com", /*off_the_record=*/false, /*in_agent_space=*/true));
}

TEST(ArrangeRule, NorAPrivateWindowsTabOrAPageWithNoSite) {
  EXPECT_FALSE(MayStart("example.com", /*off_the_record=*/true, /*in_agent_space=*/false));
  EXPECT_FALSE(MayStart("", /*off_the_record=*/false, /*in_agent_space=*/false));
}

TEST(ArrangeRule, GivingATabToAnAgentEndsArrangeBeforeTheMove) {
  std::vector<std::string> done;
  const bool moved = GiveToAgent([&] { done.push_back("end arrange"); },
                                 [&] {
                                   done.push_back("move");
                                   return true;
                                 });
  EXPECT_TRUE(moved);
  EXPECT_EQ(done, (std::vector<std::string>{"end arrange", "move"}));
}

TEST(ArrangeRule, AMoveThatFailsIsSaidAndArrangeStaysEnded) {
  std::vector<std::string> done;
  const bool moved = GiveToAgent([&] { done.push_back("end arrange"); },
                                 [&] {
                                   done.push_back("move");
                                   return false;
                                 });
  EXPECT_FALSE(moved);
  EXPECT_EQ(done, (std::vector<std::string>{"end arrange", "move"}));
}

}  // namespace
}  // namespace browser_core::arrange_rule
