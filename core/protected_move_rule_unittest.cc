#include "browser_core/protected_move_rule.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

using Answer = ProtectedAnswer;

// A page of the user's tab, at its own site, with nothing to lose, asking at |now|.
ProtectedAsk Ask() {
  ProtectedAsk ask;
  ask.now = base::TimeTicks() + base::Hours(1);
  ask.allowed = true;
  ask.own_site = true;
  return ask;
}

TEST(ProtectedMoveRuleTest, TheUsersPressMovesTheTab) {
  ProtectedAsk ask = Ask();
  ask.users_input = ask.now - base::Seconds(1);
  EXPECT_EQ(AnswerProtectedAsk(ask), Answer::kMove);
}

// A page that asks as it loads, with no press before it, gets the note.
TEST(ProtectedMoveRuleTest, AnAskWithNoPressGetsTheNote) {
  EXPECT_EQ(AnswerProtectedAsk(Ask()), Answer::kNote);
}

TEST(ProtectedMoveRuleTest, AnAskLongAfterThePressGetsTheNote) {
  ProtectedAsk ask = Ask();
  ask.users_input = ask.now - kPressLifespan - base::Milliseconds(1);
  EXPECT_EQ(AnswerProtectedAsk(ask), Answer::kNote);
}

// An agent's press, then the page asking 2 s later, as the user watches or after they took the tab
// over: the press wasn't theirs, so the tab stays and the note shows.
TEST(ProtectedMoveRuleTest, AnAgentsPressThenALateAskGetsTheNote) {
  ProtectedAsk ask = Ask();
  ask.agents_input = ask.now - base::Seconds(2);
  EXPECT_EQ(AnswerProtectedAsk(ask), Answer::kNote);
}

// The user pressed, then an agent did, before the page asked: the ask may be the agent's.
TEST(ProtectedMoveRuleTest, AnAgentsPressAfterTheUsersGetsTheNote) {
  ProtectedAsk ask = Ask();
  ask.users_input = ask.now - base::Seconds(3);
  ask.agents_input = ask.now - base::Seconds(1);
  EXPECT_EQ(AnswerProtectedAsk(ask), Answer::kNote);
}

// The user's press after the agent's is theirs.
TEST(ProtectedMoveRuleTest, TheUsersPressAfterAnAgentsMoves) {
  ProtectedAsk ask = Ask();
  ask.agents_input = ask.now - base::Seconds(3);
  ask.users_input = ask.now - base::Seconds(1);
  EXPECT_EQ(AnswerProtectedAsk(ask), Answer::kMove);
}

// A reload in the other engine would lose what the page holds, so the note asks first.
TEST(ProtectedMoveRuleTest, APageThatWouldLoseSomethingAsksFirst) {
  for (int each = 0; each < 3; ++each) {
    ProtectedAsk ask = Ask();
    ask.users_input = ask.now - base::Seconds(1);
    ask.typed = each == 0;
    ask.asks_before_leaving = each == 1;
    ask.capturing = each == 2;
    EXPECT_EQ(AnswerProtectedAsk(ask), each == 2 ? Answer::kAskCapturing : Answer::kAskTyped);
  }
}

// Without the user's press, a page with typing in it gets today's note, not the question.
TEST(ProtectedMoveRuleTest, WithoutAPressAPageWithTypingGetsTheNote) {
  ProtectedAsk ask = Ask();
  ask.typed = true;
  EXPECT_EQ(AnswerProtectedAsk(ask), Answer::kNote);
}

// Never an agent's tab or one with the engine off, a preview, a site the user keeps in Chromium,
// nor for another site's frame, such as an ad's.
TEST(ProtectedMoveRuleTest, NoMoveWhereTheTabOrSiteStays) {
  for (int each = 0; each < 4; ++each) {
    ProtectedAsk ask = Ask();
    ask.users_input = ask.now - base::Seconds(1);
    ask.allowed = each != 0;
    ask.preview = each == 1;
    ask.kept_in_chromium = each == 2;
    ask.own_site = each != 3;
    EXPECT_EQ(AnswerProtectedAsk(ask), Answer::kNote) << each;
  }
}

}  // namespace
}  // namespace browser_core
