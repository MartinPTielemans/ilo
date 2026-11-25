#include "browser_core/handed_history.h"

#include <cstddef>
#include <utility>
#include <vector>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::handed_history {
namespace {

// A tab's history as Chromium keeps it: its entries by id, and the one it's on. Each road below
// names the entry it would take the tab to, as NavigationControllerImpl::GoToIndex is given it,
// and goes there only if the bound lets it.
class Tab {
 public:
  Tab(std::vector<int> entries, size_t on) : entries_(std::move(entries)), on_(on) {}

  int on() const { return entries_[on_]; }

  // bc_tab_give_to_agent, Hand Back after the user held the tab, and a page made again.
  void Bind() { bound_.Set(entries_, on()); }
  void TakeOver() { held_ = true; }
  void HandBack() {
    held_ = false;
    Bind();
  }

  // A link followed or an address opened: a new entry after the one the tab is on, and those
  // ahead of it are gone.
  void Open(int entry) {
    entries_.resize(on_ + 1);
    entries_.push_back(entry);
    ++on_;
  }

  // The agent's back and forward tools, and the app's own back and forward:
  // NavigationController::GoBack and GoForward.
  bool Back() { return Step(-1); }
  bool Forward() { return Step(1); }
  // The page's history.back(), history.forward() and history.go(n):
  // NavigationControllerImpl::GoToOffsetFromRenderer.
  bool HistoryGo(int offset) { return Step(offset); }
  // The page's navigation.back() and navigation.forward(), which the renderer makes a
  // traverseTo() of the entry next to its own, and navigation.traverseTo(key), which names any
  // entry of the tab: NavigationControllerImpl::NavigateToNavigationApiKey.
  bool NavigationBack() { return Step(-1); }
  bool NavigationForward() { return Step(1); }
  bool TraverseTo(int entry) {
    for (size_t i = 0; i < entries_.size(); ++i) {
      if (entries_[i] == entry) {
        return GoTo(i);
      }
    }
    return false;
  }

 private:
  bool Step(int offset) {
    const int index = static_cast<int>(on_) + offset;
    return index >= 0 && index < static_cast<int>(entries_.size()) &&
           GoTo(static_cast<size_t>(index));
  }

  bool GoTo(size_t index) {
    if (!bound_.MayGoTo(entries_[index], held_)) {
      return false;
    }
    on_ = index;
    return true;
  }

  std::vector<int> entries_;
  size_t on_;
  bool held_ = false;
  Bound bound_;
};

// The user's tab went 1, 2, 3 and back to 2, where they handed it over: 1 is behind, 3 ahead.
Tab Handed() {
  Tab tab({1, 2, 3}, 1);
  tab.Bind();
  return tab;
}

TEST(HandedHistory, TheAgentsBackStopsAtTheEntryTheTabWasHandedOn) {
  Tab tab = Handed();
  EXPECT_FALSE(tab.Back());
  EXPECT_EQ(tab.on(), 2);
}

TEST(HandedHistory, TheAgentsForwardDoesntReachWhatTheUserHadAhead) {
  Tab tab = Handed();
  EXPECT_FALSE(tab.Forward());
  EXPECT_EQ(tab.on(), 2);
}

TEST(HandedHistory, ThePagesHistoryBackAndForwardStopThere) {
  Tab tab = Handed();
  EXPECT_FALSE(tab.HistoryGo(-1));
  EXPECT_FALSE(tab.HistoryGo(1));
  EXPECT_EQ(tab.on(), 2);
}

TEST(HandedHistory, ThePagesHistoryGoDoesntJumpBehindTheEntryTheTabWasHandedOn) {
  Tab tab = Handed();
  tab.Open(10);
  tab.Open(11);
  // Over the hand-over entry, onto the user's.
  EXPECT_FALSE(tab.HistoryGo(-3));
  EXPECT_EQ(tab.on(), 11);
  // Onto the hand-over entry itself.
  EXPECT_TRUE(tab.HistoryGo(-2));
  EXPECT_EQ(tab.on(), 2);
  EXPECT_FALSE(tab.HistoryGo(-1));
}

TEST(HandedHistory, ThePagesNavigationBackAndForwardStopThere) {
  Tab tab = Handed();
  EXPECT_FALSE(tab.NavigationBack());
  EXPECT_FALSE(tab.NavigationForward());
  EXPECT_EQ(tab.on(), 2);
}

TEST(HandedHistory, ThePagesTraverseToReachesNoEntryOfTheUsers) {
  Tab tab = Handed();
  tab.Open(10);
  // 3 was cut off by the thread's own page: there's no such entry to name any more.
  EXPECT_FALSE(tab.TraverseTo(3));
  EXPECT_FALSE(tab.TraverseTo(1));
  EXPECT_EQ(tab.on(), 10);
  EXPECT_TRUE(tab.TraverseTo(2));
}

TEST(HandedHistory, TraverseToDoesntReachWhatTheUserHadAhead) {
  Tab tab = Handed();
  EXPECT_FALSE(tab.TraverseTo(3));
  EXPECT_EQ(tab.on(), 2);
}

TEST(HandedHistory, TheThreadGoesBackAndForthAmongItsOwnEntriesByEveryRoad) {
  Tab tab = Handed();
  tab.Open(10);
  tab.Open(11);
  EXPECT_TRUE(tab.Back());
  EXPECT_EQ(tab.on(), 10);
  EXPECT_TRUE(tab.Forward());
  EXPECT_TRUE(tab.HistoryGo(-1));
  EXPECT_TRUE(tab.HistoryGo(1));
  EXPECT_TRUE(tab.NavigationBack());
  EXPECT_TRUE(tab.NavigationForward());
  EXPECT_TRUE(tab.TraverseTo(10));
  EXPECT_EQ(tab.on(), 10);
  // And back onto the entry it was handed on, from where it may come forward again.
  EXPECT_TRUE(tab.Back());
  EXPECT_EQ(tab.on(), 2);
  EXPECT_TRUE(tab.Forward());
  EXPECT_EQ(tab.on(), 10);
}

TEST(HandedHistory, AnEntryMadeInThePlaceOfOneOfTheUsersIsTheThreads) {
  Tab tab = Handed();
  // The thread's page where the user's 3 was.
  tab.Open(10);
  EXPECT_TRUE(tab.Back());
  EXPECT_TRUE(tab.Forward());
  EXPECT_EQ(tab.on(), 10);
}

TEST(HandedHistory, WhileTheUserHoldsTheTabTheirBackGoesBehindTheEntryItWasHandedOn) {
  Tab tab = Handed();
  tab.Open(10);
  tab.TakeOver();
  EXPECT_TRUE(tab.Back());
  EXPECT_TRUE(tab.Back());
  EXPECT_EQ(tab.on(), 1);
  EXPECT_TRUE(tab.HistoryGo(2));
  EXPECT_TRUE(tab.TraverseTo(1));
  EXPECT_EQ(tab.on(), 1);
}

TEST(HandedHistory, AtHandBackTheBoundMovesToTheEntryTheTabIsOn) {
  Tab tab = Handed();
  tab.Open(10);
  tab.TakeOver();
  ASSERT_TRUE(tab.TraverseTo(1));
  tab.HandBack();
  // On 1 now, with 2 and 10 ahead: what's ahead is what the user left there, the thread's old
  // entry too.
  EXPECT_FALSE(tab.Forward());
  EXPECT_FALSE(tab.HistoryGo(2));
  EXPECT_FALSE(tab.TraverseTo(2));
  EXPECT_FALSE(tab.TraverseTo(10));
  EXPECT_EQ(tab.on(), 1);
  tab.Open(20);
  EXPECT_TRUE(tab.Back());
  EXPECT_EQ(tab.on(), 1);
  EXPECT_FALSE(tab.Back());
}

TEST(HandedHistory, AtHandBackAnEntryTheUserMadeWhileHoldingIsTheirsToo) {
  Tab tab = Handed();
  tab.TakeOver();
  tab.Open(30);
  tab.Open(31);
  tab.HandBack();
  EXPECT_FALSE(tab.Back());
  EXPECT_FALSE(tab.HistoryGo(-2));
  EXPECT_FALSE(tab.TraverseTo(2));
  EXPECT_EQ(tab.on(), 31);
}

TEST(HandedHistory, APageMadeAgainIsBoundAtTheEntryItComesBackOn) {
  // An unloaded tab's page comes back with its entries under new ids.
  Tab tab({41, 42, 43}, 1);
  tab.Bind();
  EXPECT_FALSE(tab.Back());
  EXPECT_FALSE(tab.Forward());
  tab.Open(50);
  EXPECT_TRUE(tab.Back());
  EXPECT_EQ(tab.on(), 42);
}

TEST(HandedHistory, ATabHandedOnItsOnlyEntryHasNothingBehindIt) {
  Tab tab({7}, 0);
  tab.Bind();
  EXPECT_FALSE(tab.Back());
  EXPECT_FALSE(tab.HistoryGo(-1));
  tab.Open(8);
  EXPECT_TRUE(tab.Back());
  EXPECT_FALSE(tab.Back());
}

}  // namespace
}  // namespace browser_core::handed_history
