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
  // NavigationController::GoBack and GoForward. The browser's, as the app's go to an entry of its
  // back button's list is (bc_tab_go_to_history, NavigationController::GoToOffset).
  bool Back() { return Step(-1, /*from_page=*/false); }
  bool Forward() { return Step(1, /*from_page=*/false); }
  bool GoToEntry(int offset) { return Step(offset, /*from_page=*/false); }
  // Whether the tab's back and forward are lit (bc_tab_state): only over an entry that step may
  // take it to.
  bool CanBack() const { return May(-1); }
  bool CanForward() const { return May(1); }
  // The page's history.back(), history.forward() and history.go(n):
  // NavigationControllerImpl::GoToOffsetFromRenderer, with the frame that asked.
  bool HistoryGo(int offset) { return Step(offset, /*from_page=*/true); }
  // The page's navigation.back() and navigation.forward(), which the renderer makes a
  // traverseTo() of the entry next to its own, and navigation.traverseTo(key), which names any
  // entry of the tab: NavigationControllerImpl::NavigateToNavigationApiKey, with the frame too.
  bool NavigationBack() { return Step(-1, /*from_page=*/true); }
  bool NavigationForward() { return Step(1, /*from_page=*/true); }
  bool TraverseTo(int entry) {
    for (size_t i = 0; i < entries_.size(); ++i) {
      if (entries_[i] == entry) {
        return GoTo(i, /*from_page=*/true);
      }
    }
    return false;
  }

 private:
  bool Has(int index) const { return index >= 0 && index < static_cast<int>(entries_.size()); }

  bool May(int offset) const {
    const int index = static_cast<int>(on_) + offset;
    return Has(index) && bound_.MayGoTo(entries_[index], held_, /*from_page=*/false);
  }

  bool Step(int offset, bool from_page) {
    const int index = static_cast<int>(on_) + offset;
    return Has(index) && GoTo(static_cast<size_t>(index), from_page);
  }

  bool GoTo(size_t index, bool from_page) {
    if (!bound_.MayGoTo(entries_[index], held_, from_page)) {
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
  EXPECT_TRUE(tab.Forward());
  EXPECT_TRUE(tab.GoToEntry(-1));
  EXPECT_EQ(tab.on(), 1);
}

// The agent can put the tab in the user's hands at will (hand_off), so a page isn't the user
// because they hold the tab: by each of its roads it stays where the thread's entries end.
TEST(HandedHistory, WhileTheUserHoldsTheTabThePagesHistoryBackStaysBound) {
  Tab tab = Handed();
  tab.TakeOver();
  EXPECT_FALSE(tab.HistoryGo(-1));
  EXPECT_FALSE(tab.HistoryGo(1));
  EXPECT_EQ(tab.on(), 2);
}

TEST(HandedHistory, WhileTheUserHoldsTheTabThePagesHistoryGoStaysBound) {
  Tab tab = Handed();
  tab.Open(10);
  tab.Open(11);
  tab.TakeOver();
  EXPECT_FALSE(tab.HistoryGo(-3));
  EXPECT_EQ(tab.on(), 11);
}

TEST(HandedHistory, WhileTheUserHoldsTheTabThePagesNavigationBackStaysBound) {
  Tab tab = Handed();
  tab.TakeOver();
  EXPECT_FALSE(tab.NavigationBack());
  EXPECT_FALSE(tab.NavigationForward());
  EXPECT_EQ(tab.on(), 2);
}

TEST(HandedHistory, WhileTheUserHoldsTheTabThePagesTraverseToStaysBound) {
  Tab tab = Handed();
  tab.Open(10);
  tab.TakeOver();
  EXPECT_FALSE(tab.TraverseTo(1));
  EXPECT_EQ(tab.on(), 10);
}

TEST(HandedHistory, WhileTheUserHoldsTheTabAPageGoesBackToAnEntryTheThreadMade) {
  Tab tab = Handed();
  tab.Open(10);
  tab.Open(11);
  tab.TakeOver();
  EXPECT_TRUE(tab.HistoryGo(-1));
  EXPECT_TRUE(tab.NavigationForward());
  EXPECT_TRUE(tab.TraverseTo(10));
  // And onto the entry the tab was handed on.
  EXPECT_TRUE(tab.HistoryGo(-1));
  EXPECT_EQ(tab.on(), 2);
}

// Where the user went by themselves is no more a page's to leave by its own road than before.
TEST(HandedHistory, APageTheUserWentBackToWhileHoldingGoesNoFurtherByItself) {
  Tab tab({1, 2, 3, 4}, 2);
  tab.Bind();
  tab.TakeOver();
  ASSERT_TRUE(tab.Back());
  EXPECT_EQ(tab.on(), 2);
  EXPECT_FALSE(tab.HistoryGo(-1));
  EXPECT_FALSE(tab.TraverseTo(4));
  EXPECT_EQ(tab.on(), 2);
}

TEST(HandedHistory, BackAndForwardAreLitOnlyOverAnEntryTheTabMayBeTakenTo) {
  Tab tab = Handed();
  EXPECT_FALSE(tab.CanBack());
  EXPECT_FALSE(tab.CanForward());
  tab.Open(10);
  EXPECT_TRUE(tab.CanBack());
  EXPECT_FALSE(tab.CanForward());
  ASSERT_TRUE(tab.Back());
  EXPECT_FALSE(tab.CanBack());
  EXPECT_TRUE(tab.CanForward());
}

TEST(HandedHistory, WhileTheUserHoldsTheTabBackAndForwardAreLitAsTheirs) {
  Tab tab = Handed();
  tab.TakeOver();
  EXPECT_TRUE(tab.CanBack());
  EXPECT_TRUE(tab.CanForward());
  tab.HandBack();
  EXPECT_FALSE(tab.CanBack());
  EXPECT_FALSE(tab.CanForward());
}

TEST(HandedHistory, AtHandBackTheBoundMovesToTheEntryTheTabIsOn) {
  Tab tab = Handed();
  tab.Open(10);
  tab.TakeOver();
  ASSERT_TRUE(tab.GoToEntry(-2));
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
