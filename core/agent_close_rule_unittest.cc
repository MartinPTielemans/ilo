#include "browser_core/agent_close_rule.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

using Answer = AgentCloseAsk::Answer;
using Does = AgentArchive::Does;

// A handed tab whose page may hold unsaved changes stays handed, its page never asked, so no
// "Leave site?" shows and nothing moves: only discard archives it.
TEST(AgentArchiveRuleTest, APageThatMayAskStaysHandedUnlessDiscarded) {
  EXPECT_EQ((AgentArchive{.handed = true, .asks = true}).What(), Does::kAsks);
  EXPECT_EQ((AgentArchive{.handed = true, .asks = true, .discard = true}).What(), Does::kArchives);
  EXPECT_EQ((AgentArchive{.handed = true}).What(), Does::kArchives);
}

// Only a tab the user handed this agent, and doesn't hold, is its to archive, and a pinned one or
// a favorite only the user closes, whatever discard says.
TEST(AgentArchiveRuleTest, OnlyAHandedTabTheUserDoesntHold) {
  EXPECT_EQ((AgentArchive{.discard = true}).What(), Does::kRefused);
  EXPECT_EQ((AgentArchive{.handed = true, .held = true, .discard = true}).What(), Does::kRefused);
  EXPECT_EQ((AgentArchive{.handed = true, .pinned = true, .discard = true}).What(), Does::kPinned);
}

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
    AgentCloseAsk::Step closing = ask.Closing(/*users=*/false);
    EXPECT_TRUE(closing.close);
    EXPECT_FALSE(closing.answer);
    EXPECT_EQ(ask.PageGone(/*tab_stays=*/false).answer, Answer::kClosed);
  }
}

// A tab the user took over or pinned while its page was asked is theirs: a crash or the deadline
// leaves it open, and the agent hears it stays.
TEST(AgentCloseRuleTest, ACrashOrTheDeadlineLeavesATabThatBecameTheUsers) {
  for (bool crashed : {true, false}) {
    AgentCloseAsk ask;
    ASSERT_TRUE((crashed ? ask.RendererGone() : ask.TimedOut()).close);
    AgentCloseAsk::Step closing = ask.Closing(/*users=*/true);
    EXPECT_FALSE(closing.close);
    EXPECT_EQ(closing.answer, Answer::kStays);
    EXPECT_FALSE(ask.PageGone(/*tab_stays=*/false).answer);
  }
}

// An agreed page whose unload handlers hang is closed at the deadline too.
TEST(AgentCloseRuleTest, AnAgreedCloseThatHangsClosesAtTheDeadline) {
  AgentCloseAsk ask;
  ask.PageAnswered(/*proceed=*/true);
  EXPECT_TRUE(ask.TimedOut().close);
  EXPECT_TRUE(ask.Closing(/*users=*/false).close);
}

// Once the page agreed, Chromium is closing the tab, so a tab that became the user's meanwhile goes
// too, at the deadline if its unload handlers hang.
TEST(AgentCloseRuleTest, AnAgreedCloseGoesOnThoughTheTabBecameTheUsers) {
  AgentCloseAsk ask;
  ask.PageAnswered(/*proceed=*/true);
  ASSERT_TRUE(ask.TimedOut().close);
  EXPECT_TRUE(ask.Closing(/*users=*/true).close);
  EXPECT_EQ(ask.PageGone(/*tab_stays=*/false).answer, Answer::kClosed);
}

// A close the window didn't carry out, as of a tab between windows, is answered at once: it stays.
TEST(AgentCloseRuleTest, ACloseThatDidntHappenIsAnsweredStays) {
  AgentCloseAsk ask;
  ASSERT_TRUE(ask.TimedOut().close);
  ASSERT_TRUE(ask.Closing(/*users=*/false).close);
  EXPECT_EQ(ask.StillThere().answer, Answer::kStays);
  EXPECT_FALSE(ask.PageGone(/*tab_stays=*/false).answer);
}

// The page answered while the core's close waited its turn: the answer stands.
TEST(AgentCloseRuleTest, AnAnswerBeforeTheCoresCloseStands) {
  AgentCloseAsk ask;
  ASSERT_TRUE(ask.TimedOut().close);
  ASSERT_EQ(ask.PageAnswered(/*proceed=*/false).answer, Answer::kAsks);
  AgentCloseAsk::Step closing = ask.Closing(/*users=*/false);
  EXPECT_FALSE(closing.close);
  EXPECT_FALSE(closing.answer);
}

}  // namespace
}  // namespace browser_core
