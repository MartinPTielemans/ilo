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
struct FakeTab {
  void Change(std::optional<uint32_t> argb) { Apply(background.Changed(argb, shown, now)); }
  void Cover(bool covered) { Apply(background.Covered(covered, shown, now)); }

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
      waited.push_back(*step.wait);
    }
  }

  // The page's background changes each frame for this long: a fade.
  void Fade(base::TimeDelta time) {
    for (base::TimeDelta at; at < time; at += kFrame) {
      Change(0xff000000 | (++faded & 0xffffff));
      Pass(kFrame);
    }
  }

  TabBackground background;
  bool shown = true;
  base::TimeTicks now = base::TimeTicks() + base::Seconds(100);
  std::optional<base::TimeTicks> due;
  int waits = 0;
  // How long each wait asked for was, in order.
  std::vector<base::TimeDelta> waited;
  uint32_t faded = 0;
  std::vector<std::optional<uint32_t>> told;
};

TEST(TabBackgroundTest, KeepsThePagesColourAndTellsAShownTabAtOnce) {
  FakeTab tab;
  EXPECT_FALSE(tab.background.color());
  tab.Change(kWhite);
  EXPECT_EQ(tab.background.color(), 0xffffffu);
  ASSERT_EQ(tab.told.size(), 1u);
  EXPECT_EQ(tab.told[0], 0xffffffu);
  EXPECT_FALSE(tab.background.waiting());
  EXPECT_FALSE(tab.due);
}

TEST(TabBackgroundTest, AChangeEqualToTheColourKeptIsDropped) {
  FakeTab tab;
  tab.Change(kInk);
  tab.Pass(base::Seconds(1));
  tab.Change(kInk);
  EXPECT_EQ(tab.told.size(), 1u);
  EXPECT_FALSE(tab.due);
}

// Transparent, half-transparent and none reported are all "none", and the same to each other.
TEST(TabBackgroundTest, AColourThatIsntOpaqueIsNone) {
  FakeTab tab;
  tab.Change(kInk);
  tab.Pass(base::Seconds(1));
  tab.Change(0x8016140f);
  EXPECT_FALSE(tab.background.color());
  ASSERT_EQ(tab.told.size(), 2u);
  EXPECT_FALSE(tab.told[1]);
  tab.Pass(base::Seconds(1));
  tab.Change(0x00000000);
  tab.Change(std::nullopt);
  EXPECT_EQ(tab.told.size(), 2u);
}

// A tab no window shows keeps the colour for the switch, tells nothing and waits for nothing.
TEST(TabBackgroundTest, ATabNotShownKeepsTheColourAndTellsNothing) {
  FakeTab tab;
  tab.shown = false;
  for (int frame = 0; frame < 60; ++frame) {
    tab.Change(0xff000000 | frame);
    tab.Pass(kFrame);
  }
  tab.Pass(base::Seconds(1));
  EXPECT_TRUE(tab.told.empty());
  EXPECT_EQ(tab.waits, 0);
  EXPECT_FALSE(tab.background.waiting());
  // The switch reads the colour it settled on.
  EXPECT_EQ(tab.background.color(), 59u);
}

// A fade: the first frame's colour at once, the last once it has stopped, none between.
TEST(TabBackgroundTest, ABurstTellsTheFirstAndTheSettledColour) {
  FakeTab tab;
  for (int frame = 0; frame < 30; ++frame) {
    tab.Change(0xff000000 | frame);
    tab.Pass(kFrame);
  }
  ASSERT_EQ(tab.told.size(), 1u);
  EXPECT_EQ(tab.told[0], 0u);
  EXPECT_TRUE(tab.background.waiting());
  tab.Pass(TabBackground::kSettle);
  ASSERT_EQ(tab.told.size(), 2u);
  EXPECT_EQ(tab.told[1], 29u);
  // Nothing is left armed once the changes stop.
  EXPECT_FALSE(tab.background.waiting());
  EXPECT_FALSE(tab.due);
  tab.Pass(base::Seconds(10));
  EXPECT_EQ(tab.told.size(), 2u);
}

// A change of theme that fades: the colour it ends on is told a settle time after its last frame,
// however many waits the fade outlasted.
TEST(TabBackgroundTest, AFadeIsToldASettleTimeAfterItsEnd) {
  for (base::TimeDelta fade : {base::Milliseconds(200), base::Milliseconds(400)}) {
    FakeTab tab;
    tab.Fade(fade);
    ASSERT_EQ(tab.told.size(), 1u);
    // The last frame was one frame ago.
    tab.Pass(TabBackground::kSettle - kFrame - base::Milliseconds(1));
    EXPECT_EQ(tab.told.size(), 1u) << fade;
    tab.Pass(base::Milliseconds(1));
    ASSERT_EQ(tab.told.size(), 2u) << fade;
    EXPECT_EQ(tab.told[1], tab.faded);
    EXPECT_FALSE(tab.due);
  }
}

// A page that never stops changing: the first colour and nothing more, on one wait at a time. For
// about a second the waits are the settle time's, then each is longer than the one before until
// the longest, which goes on.
TEST(TabBackgroundTest, AnEndlessChangeWaitsLongerEachTimeAndTellsOnce) {
  FakeTab tab;
  tab.Fade(base::Seconds(10));
  EXPECT_EQ(tab.told.size(), 1u);
  // Ten seconds of frames: 11 waits asked for, one in two seconds from the eighth on, not 40 and
  // not 625.
  ASSERT_EQ(tab.waited.size(), 11u);
  EXPECT_EQ(tab.waited[0], TabBackground::kSettle);
  for (size_t i = 1; i < 5; ++i) {
    // What's left of the settle time since the last frame.
    EXPECT_LT(tab.waited[i], TabBackground::kSettle) << i;
    EXPECT_GE(tab.waited[i], TabBackground::kSettle - kFrame) << i;
  }
  EXPECT_EQ(std::vector<base::TimeDelta>(tab.waited.begin() + 5, tab.waited.end()),
            (std::vector<base::TimeDelta>{base::Milliseconds(500), base::Seconds(1),
                                          base::Seconds(2), base::Seconds(2), base::Seconds(2),
                                          base::Seconds(2)}));
  EXPECT_TRUE(tab.background.waiting());
}

// Once a run of changes has settled and been told, the next run's waits start short again.
TEST(TabBackgroundTest, TheWaitsStartShortAgainOnceTheColourSettles) {
  FakeTab tab;
  // Long enough that the wait under way is the longest.
  tab.Fade(base::Seconds(4));
  ASSERT_EQ(tab.waited.back(), TabBackground::kWaits.back());
  ASSERT_EQ(tab.told.size(), 1u);
  tab.Pass(TabBackground::kWaits.back());
  ASSERT_EQ(tab.told.size(), 2u);
  EXPECT_FALSE(tab.background.waiting());
  EXPECT_FALSE(tab.due);
  tab.waited.clear();
  tab.Fade(base::Milliseconds(100));
  ASSERT_EQ(tab.waited.size(), 1u);
  EXPECT_EQ(tab.waited[0], TabBackground::kSettle);
}

// One change on the heels of another, as a page that sets its theme in two steps: the second is
// told a settle time after it, on the first wait alone.
TEST(TabBackgroundTest, ASecondChangeIsToldASettleTimeAfterIt) {
  FakeTab tab;
  tab.Change(kWhite);
  tab.Pass(kFrame);
  tab.Change(kInk);
  ASSERT_EQ(tab.told.size(), 1u);
  tab.Pass(TabBackground::kSettle - base::Milliseconds(1));
  EXPECT_EQ(tab.told.size(), 1u);
  tab.Pass(base::Milliseconds(1));
  ASSERT_EQ(tab.told.size(), 2u);
  EXPECT_EQ(tab.told[1], 0x16140fu);
  EXPECT_EQ(tab.waited, (std::vector<base::TimeDelta>{TabBackground::kSettle}));
  EXPECT_FALSE(tab.due);
}

// A page whose colour is still, or changes now and then, asks for no wait at all.
TEST(TabBackgroundTest, AStillPageAsksForNoWait) {
  FakeTab tab;
  tab.Change(kWhite);
  tab.Pass(base::Seconds(10));
  tab.Change(kInk);
  tab.Pass(base::Seconds(10));
  tab.Change(kWhite);
  EXPECT_EQ(tab.told.size(), 3u);
  EXPECT_EQ(tab.waits, 0);
  EXPECT_FALSE(tab.background.waiting());
  EXPECT_FALSE(tab.due);
}

// A change long after the last is told at once again.
TEST(TabBackgroundTest, AChangeAfterTheSettleTimeIsToldAtOnce) {
  FakeTab tab;
  tab.Change(kWhite);
  tab.Pass(TabBackground::kSettle);
  tab.Change(kInk);
  EXPECT_EQ(tab.told.size(), 2u);
  EXPECT_FALSE(tab.due);
}

// Hidden mid-fade: the wait ends at its time with nothing told and none after it, and the UI reads
// the colour when it shows the tab again.
TEST(TabBackgroundTest, AWaitEndsOnceTheTabIsntShown) {
  FakeTab tab;
  tab.Change(kWhite);
  tab.Pass(kFrame);
  tab.Change(kInk);
  ASSERT_TRUE(tab.background.waiting());
  tab.shown = false;
  tab.Change(kWhite);
  tab.Pass(base::Seconds(1));
  EXPECT_FALSE(tab.background.waiting());
  EXPECT_FALSE(tab.due);
  EXPECT_EQ(tab.waits, 1);
  EXPECT_EQ(tab.told.size(), 1u);
  EXPECT_EQ(tab.background.color(), 0xffffffu);
}

// Hidden while the longest wait is under way: shown again, its next run of changes starts with
// the short one.
TEST(TabBackgroundTest, TheWaitsStartShortAgainAfterTheTabWasHidden) {
  FakeTab tab;
  tab.Fade(base::Seconds(4));
  ASSERT_EQ(tab.waited.back(), TabBackground::kWaits.back());
  tab.shown = false;
  tab.Pass(TabBackground::kWaits.back());
  ASSERT_FALSE(tab.background.waiting());
  tab.shown = true;
  tab.waited.clear();
  tab.Fade(base::Milliseconds(100));
  ASSERT_EQ(tab.waited.size(), 1u);
  EXPECT_EQ(tab.waited[0], TabBackground::kSettle);
}

// The tab goes to another page mid-fade: the colour waiting is told then, since a new page of the
// same colour reports no change.
TEST(TabBackgroundTest, LeavingThePageTellsAWaitingChange) {
  FakeTab tab;
  tab.Change(kWhite);
  tab.Pass(kFrame);
  tab.Change(kInk);
  ASSERT_EQ(tab.told.size(), 1u);
  TabBackground::Step left = tab.background.LeftPage(/*shown=*/true);
  EXPECT_TRUE(left.tell);
  EXPECT_FALSE(left.wait);
  EXPECT_FALSE(tab.background.waiting());
  EXPECT_EQ(tab.background.color(), 0x16140fu);
  // With nothing waiting, leaving tells nothing: the new page keeps the colour until its own.
  EXPECT_FALSE(tab.background.LeftPage(/*shown=*/true).tell);
}

// Left while the longest wait is under way: the new page's first run of changes starts with the
// short one.
TEST(TabBackgroundTest, TheWaitsStartShortAgainAfterThePageIsLeft) {
  FakeTab tab;
  tab.Fade(base::Seconds(4));
  ASSERT_EQ(tab.waited.back(), TabBackground::kWaits.back());
  // The tab stops its timer as it leaves the page.
  tab.due.reset();
  tab.Apply(tab.background.LeftPage(/*shown=*/true));
  ASSERT_FALSE(tab.background.waiting());
  tab.waited.clear();
  tab.Pass(base::Seconds(1));
  tab.Fade(base::Milliseconds(100));
  ASSERT_EQ(tab.waited.size(), 1u);
  EXPECT_EQ(tab.waited[0], TabBackground::kSettle);
}

// A page that died: the pane shows the core's card, so the colour is none until the page is back,
// and then the one it had, as a page that loads again in the same colour reports no change.
TEST(TabBackgroundTest, ACoveredPageIsNoneAndHasItsColourAgainAfter) {
  FakeTab tab;
  tab.Change(kInk);
  tab.Pass(base::Seconds(1));
  tab.Cover(true);
  EXPECT_FALSE(tab.background.color());
  ASSERT_EQ(tab.told.size(), 2u);
  EXPECT_FALSE(tab.told[1]);
  // Covered already: nothing more.
  tab.Cover(true);
  EXPECT_EQ(tab.told.size(), 2u);
  tab.Pass(base::Seconds(1));
  tab.Cover(false);
  ASSERT_EQ(tab.told.size(), 3u);
  EXPECT_EQ(tab.told[2], 0x16140fu);
  EXPECT_FALSE(tab.due);
}

// What the page reports while it's covered is held and not told: the pane doesn't show it.
TEST(TabBackgroundTest, AColourReportedWhileCoveredWaitsForThePage) {
  FakeTab tab;
  tab.Change(kInk);
  tab.Pass(base::Seconds(1));
  tab.Cover(true);
  tab.Pass(base::Seconds(1));
  tab.Change(kWhite);
  EXPECT_FALSE(tab.background.color());
  EXPECT_EQ(tab.told.size(), 2u);
  EXPECT_EQ(tab.waits, 0);
  tab.Cover(false);
  ASSERT_EQ(tab.told.size(), 3u);
  EXPECT_EQ(tab.told[2], 0xffffffu);
}

// A tab no window shows, covered and back: the switch reads none, then the page's colour.
TEST(TabBackgroundTest, ACoveredTabNotShownTellsNothing) {
  FakeTab tab;
  tab.shown = false;
  tab.Change(kInk);
  tab.Cover(true);
  EXPECT_FALSE(tab.background.color());
  tab.Cover(false);
  EXPECT_EQ(tab.background.color(), 0x16140fu);
  EXPECT_TRUE(tab.told.empty());
  EXPECT_EQ(tab.waits, 0);
}

}  // namespace
}  // namespace browser_core
