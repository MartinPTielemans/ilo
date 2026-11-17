#include "browser_core/agent_close_rule.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

using Answer = AgentCloseAsk::Answer;

// A page whose handler doesn't ask is closed by Chromium after its unload handlers: the agent hears
// once the tab is gone, not as the page agrees.
TEST(AgentCloseRuleTest, APageThatDoesntAskIsAnsweredAsItsTabGoes) {
  AgentCloseAsk ask;
  AgentCloseAsk::Step agreed = ask.PageAnswered(/*proceed=*/true);
  EXPECT_FALSE(agreed.close);
  EXPECT_FALSE(agreed.answer);
  EXPECT_EQ(ask.PageGone(/*tab_stays=*/false).answer, Answer::kClosed);
}

TEST(AgentCloseRuleTest, APageThatAsksStaysOpen) {
  AgentCloseAsk ask;
  AgentCloseAsk::Step asked = ask.PageAnswered(/*proceed=*/false);
  EXPECT_FALSE(asked.close);
  EXPECT_EQ(asked.answer, Answer::kAsks);
}

// Whatever comes after the answer, the agent heard once: the user closing the tab it kept, a
// crash, the deadline.
TEST(AgentCloseRuleTest, AnsweredOnceWhateverComesAfter) {
  AgentCloseAsk ask;
  ASSERT_EQ(ask.PageAnswered(/*proceed=*/false).answer, Answer::kAsks);
  EXPECT_FALSE(ask.PageAnswered(/*proceed=*/true).answer);
  EXPECT_FALSE(ask.Navigated().answer);
  EXPECT_FALSE(ask.RendererGone().close);
  EXPECT_FALSE(ask.TimedOut().close);
  EXPECT_FALSE(ask.PageGone(/*tab_stays=*/false).answer);
}

// The user's close, its window's, the profile's or a quit, before the page answered.
TEST(AgentCloseRuleTest, ATabClosedMeanwhileIsClosed) {
  AgentCloseAsk ask;
  EXPECT_EQ(ask.PageGone(/*tab_stays=*/false).answer, Answer::kClosed);
}

// Unloaded, its page went and the tab stayed.
TEST(AgentCloseRuleTest, AnUnloadedTabStays) {
  AgentCloseAsk ask;
  EXPECT_EQ(ask.PageGone(/*tab_stays=*/true).answer, Answer::kStays);
}

TEST(AgentCloseRuleTest, ATabThatWentToAnotherPageStays) {
  AgentCloseAsk ask;
  EXPECT_EQ(ask.Navigated().answer, Answer::kStays);
}

// Once the page agreed, Chromium is closing it: a page committing then doesn't keep the tab.
TEST(AgentCloseRuleTest, AnAgreedCloseGoesOnPastANavigation) {
  AgentCloseAsk ask;
  ask.PageAnswered(/*proceed=*/true);
  EXPECT_FALSE(ask.Navigated().answer);
  EXPECT_EQ(ask.PageGone(/*tab_stays=*/false).answer, Answer::kClosed);
}

// Nothing on a crashed page can ask, and a page that doesn't answer in time is closed as Chromium
// closes a hung one: each closes the tab, whose going answers.
TEST(AgentCloseRuleTest, ACrashOrTheDeadlineClosesTheTab) {
  for (bool crashed : {true, false}) {
    AgentCloseAsk ask;
    AgentCloseAsk::Step step = crashed ? ask.RendererGone() : ask.TimedOut();
    EXPECT_TRUE(step.close);
    EXPECT_FALSE(step.answer);
    EXPECT_EQ(ask.PageGone(/*tab_stays=*/false).answer, Answer::kClosed);
  }
}

// An agreed page whose unload handlers hang is closed at the deadline too.
TEST(AgentCloseRuleTest, AnAgreedCloseThatHangsClosesAtTheDeadline) {
  AgentCloseAsk ask;
  ask.PageAnswered(/*proceed=*/true);
  EXPECT_TRUE(ask.TimedOut().close);
}

}  // namespace
}  // namespace browser_core
