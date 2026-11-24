#include "browser_core/tab_background.h"

#include <optional>
#include <vector>

#include "base/time/time.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

constexpr uint32_t kWhite = 0xffffffff;
constexpr uint32_t kInk = 0xff16140f;
constexpr base::TimeDelta kFrame = base::Milliseconds(16);

// A tab as the core runs it: one wait at most, ended when its time comes, and the colours the UI
// was told.
struct Run {
  void Change(std::optional<uint32_t> argb) { Apply(background.Changed(argb, shown, now)); }

  // Time passes, and the wait ends when it's due.
  void Pass(base::TimeDelta time) {
    const base::TimeTicks end = now + time;
    while (due && *due <= end) {
      now = *due;
      due.reset();
      Apply(background.Settled(shown, now));
    }
    now = end;
  }

  void Apply(TabBackground::Step step) {
    if (step.tell) {
      told.push_back(background.color());
    }
    if (step.wait) {
      // A second wait while one is under way would be a second timer.
      EXPECT_FALSE(due);
      due = now + *step.wait;
      ++waits;
    }
  }

  TabBackground background;
  bool shown = true;
  base::TimeTicks now = base::TimeTicks() + base::Seconds(100);
  std::optional<base::TimeTicks> due;
  int waits = 0;
  std::vector<std::optional<uint32_t>> told;
};

TEST(TabBackgroundTest, KeepsThePagesColourAndTellsAShownTabAtOnce) {
  Run run;
  EXPECT_FALSE(run.background.color());
  run.Change(kWhite);
  EXPECT_EQ(run.background.color(), 0xffffffu);
  ASSERT_EQ(run.told.size(), 1u);
  EXPECT_EQ(run.told[0], 0xffffffu);
  EXPECT_FALSE(run.background.waiting());
  EXPECT_FALSE(run.due);
}

TEST(TabBackgroundTest, AChangeEqualToTheColourKeptIsDropped) {
  Run run;
  run.Change(kInk);
  run.Pass(base::Seconds(1));
  run.Change(kInk);
  EXPECT_EQ(run.told.size(), 1u);
  EXPECT_FALSE(run.due);
}

// Transparent, half-transparent and none reported are all "none", and the same to each other.
TEST(TabBackgroundTest, AColourThatIsntOpaqueIsNone) {
  Run run;
  run.Change(kInk);
  run.Pass(base::Seconds(1));
  run.Change(0x8016140f);
  EXPECT_FALSE(run.background.color());
  ASSERT_EQ(run.told.size(), 2u);
  EXPECT_FALSE(run.told[1]);
  run.Pass(base::Seconds(1));
  run.Change(0x00000000);
  run.Change(std::nullopt);
  EXPECT_EQ(run.told.size(), 2u);
}

// A tab no window shows keeps the colour for the switch, tells nothing and waits for nothing.
TEST(TabBackgroundTest, ATabNotShownKeepsTheColourAndTellsNothing) {
  Run run;
  run.shown = false;
  for (int frame = 0; frame < 60; ++frame) {
    run.Change(0xff000000 | frame);
    run.Pass(kFrame);
  }
  run.Pass(base::Seconds(1));
  EXPECT_TRUE(run.told.empty());
  EXPECT_EQ(run.waits, 0);
  EXPECT_FALSE(run.background.waiting());
  // The switch reads the colour it settled on.
  EXPECT_EQ(run.background.color(), 59u);
}

// A fade: the first frame's colour at once, the last once it has stopped, none between.
TEST(TabBackgroundTest, ABurstTellsTheFirstAndTheSettledColour) {
  Run run;
  for (int frame = 0; frame < 30; ++frame) {
    run.Change(0xff000000 | frame);
    run.Pass(kFrame);
  }
  ASSERT_EQ(run.told.size(), 1u);
  EXPECT_EQ(run.told[0], 0u);
  EXPECT_TRUE(run.background.waiting());
  run.Pass(TabBackground::kSettle);
  ASSERT_EQ(run.told.size(), 2u);
  EXPECT_EQ(run.told[1], 29u);
  // Nothing is left armed once the changes stop.
  EXPECT_FALSE(run.background.waiting());
  EXPECT_FALSE(run.due);
  run.Pass(base::Seconds(10));
  EXPECT_EQ(run.told.size(), 2u);
}

// A page that never stops changing: the first colour and nothing more, on one wait at a time.
TEST(TabBackgroundTest, AnEndlessRunTellsOnce) {
  Run run;
  for (int frame = 0; frame < 600; ++frame) {
    run.Change(0xff000000 | frame);
    run.Pass(kFrame);
  }
  EXPECT_EQ(run.told.size(), 1u);
  // The wait went on about once per settle time, not once per change.
  EXPECT_LE(run.waits, 2 * 600 * kFrame / TabBackground::kSettle);
}

// A change long after the last is told at once again.
TEST(TabBackgroundTest, AChangeAfterTheSettleTimeIsToldAtOnce) {
  Run run;
  run.Change(kWhite);
  run.Pass(TabBackground::kSettle);
  run.Change(kInk);
  EXPECT_EQ(run.told.size(), 2u);
  EXPECT_FALSE(run.due);
}

// Hidden mid-fade: the wait ends at its time with nothing told and none after it, and the UI reads
// the colour when it shows the tab again.
TEST(TabBackgroundTest, AWaitEndsOnceTheTabIsntShown) {
  Run run;
  run.Change(kWhite);
  run.Pass(kFrame);
  run.Change(kInk);
  ASSERT_TRUE(run.background.waiting());
  run.shown = false;
  run.Change(kWhite);
  run.Pass(base::Seconds(1));
  EXPECT_FALSE(run.background.waiting());
  EXPECT_FALSE(run.due);
  EXPECT_EQ(run.waits, 1);
  EXPECT_EQ(run.told.size(), 1u);
  EXPECT_EQ(run.background.color(), 0xffffffu);
}

// The tab goes to another page mid-fade: the colour waiting is told then, since a new page of the
// same colour reports no change.
TEST(TabBackgroundTest, LeavingThePageTellsAWaitingChange) {
  Run run;
  run.Change(kWhite);
  run.Pass(kFrame);
  run.Change(kInk);
  ASSERT_EQ(run.told.size(), 1u);
  TabBackground::Step left = run.background.LeftPage(/*shown=*/true);
  EXPECT_TRUE(left.tell);
  EXPECT_FALSE(left.wait);
  EXPECT_FALSE(run.background.waiting());
  EXPECT_EQ(run.background.color(), 0x16140fu);
  // With nothing waiting, leaving tells nothing: the new page keeps the colour until its own.
  EXPECT_FALSE(run.background.LeftPage(/*shown=*/true).tell);
}

}  // namespace
}  // namespace browser_core
