#include "browser_core/agent_space_rule.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::agent_space_rule {
namespace {

constexpr Ask kAsks[] = {Ask::kPaste, Ask::kFilePicker};

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

}  // namespace
}  // namespace browser_core::agent_space_rule
