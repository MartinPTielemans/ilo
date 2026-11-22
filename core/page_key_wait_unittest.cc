#include "browser_core/page_key_wait.h"

#include <vector>

#include "base/time/time.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

constexpr int kL = 76;
constexpr int kF = 70;
constexpr int kCommand = 1 << 3;

base::TimeTicks At(int ms) {
  return base::TimeTicks() + base::Milliseconds(1000 + ms);
}

// ⌘L pressed in |tab| at |ms|.
PageKey CommandL(uint64_t tab, int ms) {
  return {.tab = tab, .time = At(ms), .code = kL, .modifiers = kCommand};
}

// The bound is past what a page answers in, and well inside the second a second press has.
TEST(PageKeyWaitTest, TheBoundIsSet) {
  EXPECT_GT(kPageKeyBound, base::TimeDelta());
  EXPECT_LT(kPageKeyBound, base::Seconds(1));
}

TEST(PageKeyWaitTest, AKeySentWaitsUntilItsBound) {
  PageKeyWait wait;
  EXPECT_FALSE(wait.NextDue());
  wait.Sent(CommandL(1, 0), At(0));
  EXPECT_TRUE(wait.waiting());
  EXPECT_TRUE(wait.Waits(CommandL(1, 0)));
  EXPECT_FALSE(wait.Waits(CommandL(1, 5)));
  EXPECT_TRUE(wait.HasKeysOf(1));
  EXPECT_FALSE(wait.HasKeysOf(2));
  EXPECT_EQ(wait.NextDue(), At(0) + kPageKeyBound);
  EXPECT_TRUE(wait.Due(At(0) + kPageKeyBound - base::Milliseconds(1)).empty());
  EXPECT_TRUE(wait.waiting());
}

// A page that keeps the key in time, as Docs keeps ⌘F, sees nothing change.
TEST(PageKeyWaitTest, AKeyKeptInTimeIsThePages) {
  PageKeyWait wait;
  wait.Sent(CommandL(1, 0), At(0));
  EXPECT_TRUE(wait.Answered(CommandL(1, 0), /*kept=*/true));
  EXPECT_FALSE(wait.waiting());
  EXPECT_FALSE(wait.HasKeysOf(1));
  EXPECT_FALSE(wait.NextDue());
  EXPECT_TRUE(wait.Due(At(0) + kPageKeyBound).empty());
}

TEST(PageKeyWaitTest, AKeyNotKeptInTimeRunsAsItAlwaysHas) {
  PageKeyWait wait;
  wait.Sent(CommandL(1, 0), At(0));
  EXPECT_TRUE(wait.Answered(CommandL(1, 0), /*kept=*/false));
  EXPECT_TRUE(wait.RunsUnkept(CommandL(1, 0)));
  EXPECT_FALSE(wait.waiting());
  EXPECT_TRUE(wait.Due(At(0) + kPageKeyBound).empty());
}

// A busy page's ⌘L opens the command bar at the bound, and its late answer doesn't run it again,
// which would close the bar.
TEST(PageKeyWaitTest, PastTheBoundTheKeyRunsAndTheLateAnswerIsDropped) {
  PageKeyWait wait;
  wait.Sent(CommandL(1, 0), At(0));
  EXPECT_EQ(wait.Due(At(0) + kPageKeyBound), std::vector<PageKey>{CommandL(1, 0)});
  EXPECT_FALSE(wait.waiting());
  EXPECT_FALSE(wait.NextDue());
  // Its answer is still to come.
  EXPECT_TRUE(wait.HasKeysOf(1));
  EXPECT_FALSE(wait.Answered(CommandL(1, 0), /*kept=*/false));
  EXPECT_FALSE(wait.HasKeysOf(1));
  // Chromium hands it back without its tab.
  EXPECT_FALSE(wait.RunsUnkept(CommandL(0, 0)));
  // Each later press is its own.
  EXPECT_TRUE(wait.RunsUnkept(CommandL(1, 5000)));
}

// The page's own handler has run, late; ilo's ran at the bound. Nothing more happens.
TEST(PageKeyWaitTest, AKeyKeptLateIsDroppedToo) {
  PageKeyWait wait;
  wait.Sent(CommandL(1, 0), At(0));
  wait.Due(At(0) + kPageKeyBound);
  EXPECT_FALSE(wait.Answered(CommandL(1, 0), /*kept=*/true));
  EXPECT_TRUE(wait.RunsUnkept(CommandL(1, 0)));
}

// Pressed again within a second, the key runs ilo's command before the page, and the first
// press's late answer doesn't run it a second time.
TEST(PageKeyWaitTest, ASecondPressSettlesTheFirstAndDropsItsLateAnswer) {
  PageKeyWait wait;
  wait.Sent(CommandL(1, 0), At(0));
  wait.TakenBefore(CommandL(1, 50));
  EXPECT_FALSE(wait.waiting());
  EXPECT_TRUE(wait.Due(At(0) + kPageKeyBound).empty());
  EXPECT_FALSE(wait.Answered(CommandL(1, 0), /*kept=*/false));
  EXPECT_FALSE(wait.RunsUnkept(CommandL(1, 0)));
}

// Another key, or the same key in another tab, waits on its own.
TEST(PageKeyWaitTest, ASecondPressSettlesOnlyItsOwnKeyInItsOwnTab) {
  PageKeyWait wait;
  const PageKey find{.tab = 1, .time = At(0), .code = kF, .modifiers = kCommand};
  wait.Sent(find, At(0));
  wait.Sent(CommandL(2, 0), At(0));
  wait.TakenBefore(CommandL(1, 50));
  EXPECT_EQ(wait.Due(At(0) + kPageKeyBound), (std::vector<PageKey>{find, CommandL(2, 0)}));
}

TEST(PageKeyWaitTest, ATabGoneWhileItsKeyWaitsRunsNothing) {
  PageKeyWait wait;
  wait.Sent(CommandL(1, 0), At(0));
  wait.TabGone(1);
  EXPECT_FALSE(wait.waiting());
  EXPECT_FALSE(wait.NextDue());
  EXPECT_TRUE(wait.Due(At(0) + kPageKeyBound).empty());
}

// A page gone after the bound ran its key leaves nothing behind for a later page in the tab.
TEST(PageKeyWaitTest, ATabGoneForgetsItsLateKeys) {
  PageKeyWait wait;
  wait.Sent(CommandL(1, 0), At(0));
  wait.Due(At(0) + kPageKeyBound);
  wait.TabGone(1);
  EXPECT_TRUE(wait.Answered(CommandL(1, 0), /*kept=*/false));
  EXPECT_TRUE(wait.RunsUnkept(CommandL(1, 0)));
}

TEST(PageKeyWaitTest, TwoTabsWaitAtOnceEachToItsOwnBound) {
  PageKeyWait wait;
  wait.Sent(CommandL(1, 0), At(0));
  wait.Sent(CommandL(2, 30), At(30));
  EXPECT_EQ(wait.NextDue(), At(0) + kPageKeyBound);
  EXPECT_EQ(wait.Due(At(0) + kPageKeyBound), std::vector<PageKey>{CommandL(1, 0)});
  EXPECT_EQ(wait.NextDue(), At(30) + kPageKeyBound);
  EXPECT_TRUE(wait.Answered(CommandL(2, 30), /*kept=*/false));
  EXPECT_TRUE(wait.RunsUnkept(CommandL(2, 30)));
  EXPECT_FALSE(wait.NextDue());
  EXPECT_FALSE(wait.Answered(CommandL(1, 0), /*kept=*/false));
  EXPECT_FALSE(wait.RunsUnkept(CommandL(1, 0)));
}

// A tab closing leaves the other's key waiting.
TEST(PageKeyWaitTest, ATabGoneLeavesTheOthersKeys) {
  PageKeyWait wait;
  wait.Sent(CommandL(1, 0), At(0));
  wait.Sent(CommandL(2, 0), At(0));
  wait.TabGone(1);
  EXPECT_EQ(wait.Due(At(0) + kPageKeyBound), std::vector<PageKey>{CommandL(2, 0)});
}

}  // namespace
}  // namespace browser_core
