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

// The documents of a tab's page, as their UKM source ids: the page an address was typed over, and
// the one the tab went to.
constexpr int64_t kPageBefore = 7;
constexpr int64_t kPageAfter = 8;

// ⌘L's key-down reached the page before, which left it to the browser to open the command bar,
// and the page typed in it asks as it loads, a second later: the note, as for any page that does,
// not a move (clear.sh's drm.html, #3593).
TEST(ProtectedMoveRuleTest, APressOnThePageBeforeGetsTheNote) {
  ProtectedAsk ask = Ask();
  DocumentPresses presses;
  presses.Users(kPageBefore, ask.now - base::Seconds(1), std::nullopt);
  presses.Fill(kPageAfter, ask);
  EXPECT_EQ(AnswerProtectedAsk(ask), Answer::kNote);
  // The same press moves the tab had the page it reached asked.
  presses.Fill(kPageBefore, ask);
  EXPECT_EQ(AnswerProtectedAsk(ask), Answer::kMove);
}

// So too an agent's: it never blocks the user's press on the page that asks.
TEST(ProtectedMoveRuleTest, AnAgentsInputOnThePageBeforeDoesntCount) {
  ProtectedAsk ask = Ask();
  DocumentPresses presses;
  presses.Users(kPageAfter, ask.now - base::Seconds(2), std::nullopt);
  presses.Agents(kPageBefore, ask.now - base::Seconds(1));
  presses.Fill(kPageAfter, ask);
  EXPECT_EQ(AnswerProtectedAsk(ask), Answer::kMove);
}

TEST(ProtectedMoveRuleTest, AKeyTheBrowserRanGivesThePageNoPress) {
  const base::TimeTicks key = base::TimeTicks() + base::Minutes(59);
  ProtectedAsk ask = Ask();
  DocumentPresses presses;
  // A key the page left that the browser ran, such as ⌘L, on the page that asks.
  presses.Users(kPageAfter, ask.now - base::Seconds(1), key);
  presses.TakenBack(key);
  presses.Fill(kPageAfter, ask);
  EXPECT_EQ(AnswerProtectedAsk(ask), Answer::kNote);
  // A click before the key stands, as the key took nothing from it.
  presses.Users(kPageAfter, ask.now - base::Seconds(2), std::nullopt);
  presses.Users(kPageAfter, ask.now - base::Seconds(1), key);
  presses.TakenBack(key);
  presses.Fill(kPageAfter, ask);
  EXPECT_EQ(ask.users_input, ask.now - base::Seconds(2));
  EXPECT_EQ(AnswerProtectedAsk(ask), Answer::kMove);
  // A key the page kept, such as a game's space, is the user's press on it.
  presses.Users(kPageAfter, ask.now - base::Seconds(1), key);
  presses.Fill(kPageAfter, ask);
  EXPECT_EQ(ask.users_input, ask.now - base::Seconds(1));
  // And a click after a key is the user's whatever the browser did with the key.
  presses.Users(kPageAfter, ask.now - base::Milliseconds(500), std::nullopt);
  presses.TakenBack(key);
  presses.Fill(kPageAfter, ask);
  EXPECT_EQ(ask.users_input, ask.now - base::Milliseconds(500));
}

// ⌘L ⌘L, the second sent before the page left the first: each is taken back, and the click before
// them is the press that counts. Keeping one press before the newest key left the first ⌘L counted.
TEST(ProtectedMoveRuleTest, KeysSentBeforeThePageLeftTheFirstAreAllTakenBack) {
  const base::TimeTicks first = base::TimeTicks() + base::Minutes(58);
  const base::TimeTicks second = base::TimeTicks() + base::Minutes(59);
  ProtectedAsk ask = Ask();
  DocumentPresses presses;
  presses.Users(kPageAfter, ask.now - base::Seconds(3), std::nullopt);
  presses.Users(kPageAfter, ask.now - base::Seconds(2), first);
  presses.Users(kPageAfter, ask.now - base::Seconds(1), second);
  presses.TakenBack(first);
  presses.TakenBack(second);
  presses.Fill(kPageAfter, ask);
  EXPECT_EQ(ask.users_input, ask.now - base::Seconds(3));
  // With no click before them, nothing is left: the note, not a move.
  presses = DocumentPresses();
  presses.Users(kPageAfter, ask.now - base::Seconds(2), first);
  presses.Users(kPageAfter, ask.now - base::Seconds(1), second);
  presses.TakenBack(second);
  presses.TakenBack(first);
  presses.Fill(kPageAfter, ask);
  EXPECT_EQ(AnswerProtectedAsk(ask), Answer::kNote);
}

}  // namespace
}  // namespace browser_core
