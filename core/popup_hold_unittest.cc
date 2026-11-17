#include "browser_core/popup_hold.h"

#include "base/test/mock_callback.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

// A page's link with a target, or its window.open(), to an http(s) address, with the native ad
// blocker on.
PopupOpen PagesPopup() {
  return {.page_made = true, .in_front = true, .agent = false, .judged = true, .filtering = true};
}

TEST(PopupHoldTest, HoldsAPagesPopupTheFiltersJudge) {
  EXPECT_TRUE(HoldsPopup(PagesPopup()));
}

// ⌘-click, a middle click, Open Link in New Tab and the command bar: Chrome makes the tab, not
// content for a page.
TEST(PopupHoldTest, NeverHoldsATabTheUserOpened) {
  PopupOpen open = PagesPopup();
  open.page_made = false;
  EXPECT_FALSE(HoldsPopup(open));
}

// A page's window.open() run from a click with ⌘ or ⇧ held opens behind or in a window: the user's.
TEST(PopupHoldTest, NeverHoldsATabAModifiedClickPutsBehindOrInAWindow) {
  PopupOpen open = PagesPopup();
  open.in_front = false;
  EXPECT_FALSE(HoldsPopup(open));
}

// window.open() with no address or about:blank is listed at once, as before.
TEST(PopupHoldTest, NeverHoldsATabWhoseFirstAddressTheFiltersDontJudge) {
  PopupOpen open = PagesPopup();
  open.judged = false;
  EXPECT_FALSE(HoldsPopup(open));
}

// uBlock Origin, native off in Settings, or the opener's or the tab's site turned off.
TEST(PopupHoldTest, NeverHoldsWhileTheNativeBlockerDoesntFilter) {
  PopupOpen open = PagesPopup();
  open.filtering = false;
  EXPECT_FALSE(HoldsPopup(open));
}

// An agent's page's pop-up is reported as before, which the agent's answer names.
TEST(PopupHoldTest, NeverHoldsAnAgentsPagesPopup) {
  PopupOpen open = PagesPopup();
  open.agent = true;
  EXPECT_FALSE(HoldsPopup(open));
}

// Reported at once, and closed after if the filters say so, as before: an agent's answer names it.
TEST(PopupHoldTest, ATabNotHeldIsReportedAtOnceWhateverTheFiltersSay) {
  PopupHold before;
  base::MockOnceClosure report;
  EXPECT_CALL(report, Run).Times(0);
  EXPECT_FALSE(before.Report(report.Get()));

  PopupHold closed;
  closed.Asking();
  closed.Decided(/*closes=*/true);
  EXPECT_FALSE(closed.Report(report.Get()));
}

// The hold ends at the filters' answer about the first address, before the page's request goes.
TEST(PopupHoldTest, APopupTheFiltersLetThroughIsReportedAtTheirAnswer) {
  PopupHold hold;
  hold.Hold();
  hold.Asking();
  base::MockOnceClosure report;
  EXPECT_CALL(report, Run).Times(0);
  EXPECT_TRUE(hold.Report(report.Get()));
  ::testing::Mock::VerifyAndClearExpectations(&report);
  EXPECT_CALL(report, Run);
  hold.Decided(/*closes=*/false);
  EXPECT_FALSE(hold.holding());
}

TEST(PopupHoldTest, APopupTheFiltersCloseIsNeverReported) {
  PopupHold hold;
  hold.Hold();
  hold.Asking();
  base::MockOnceClosure report;
  EXPECT_CALL(report, Run).Times(0);
  EXPECT_TRUE(hold.Report(report.Get()));
  hold.Decided(/*closes=*/true);
  hold.Ended();
}

// content can start a window.open() page's navigation before it shows the window, so the answer
// can come before the tab strip has the tab.
TEST(PopupHoldTest, AnAnswerBeforeTheTabStripHasTheTabStands) {
  PopupHold through;
  through.Asking();
  through.Decided(/*closes=*/false);
  through.Hold();
  base::MockOnceClosure report;
  EXPECT_CALL(report, Run).Times(0);
  EXPECT_FALSE(through.Report(report.Get()));

  PopupHold closed;
  closed.Asking();
  closed.Decided(/*closes=*/true);
  closed.Hold();
  EXPECT_TRUE(closed.Report(report.Get()));
}

// Its first navigation ends with no request, as for a page the opener stops or sends to
// about:blank first, or its page goes: listed, never left held.
TEST(PopupHoldTest, APopupThatNeverMakesARequestIsReported) {
  PopupHold hold;
  hold.Hold();
  base::MockOnceClosure report;
  EXPECT_TRUE(hold.Report(report.Get()));
  EXPECT_CALL(report, Run);
  hold.Ended();
  EXPECT_FALSE(hold.holding());
}

// The opener goes before the filters were asked, so they never will be: listed.
TEST(PopupHoldTest, APopupWhoseOpenerGoesBeforeTheFiltersAreAskedIsReported) {
  PopupHold hold;
  hold.Hold();
  base::MockOnceClosure report;
  EXPECT_TRUE(hold.Report(report.Get()));
  EXPECT_CALL(report, Run);
  hold.OpenerGone();
  EXPECT_FALSE(hold.holding());
}

// The opener goes while the filters are asked, whose answer, about the opener's page as it was,
// still comes: listed or dropped by it.
TEST(PopupHoldTest, APopupWhoseOpenerGoesWhileTheFiltersAreAskedGetsTheirAnswer) {
  PopupHold through;
  through.Hold();
  through.Asking();
  base::MockOnceClosure listed;
  EXPECT_TRUE(through.Report(listed.Get()));
  EXPECT_CALL(listed, Run).Times(0);
  through.OpenerGone();
  EXPECT_TRUE(through.holding());
  ::testing::Mock::VerifyAndClearExpectations(&listed);
  EXPECT_CALL(listed, Run);
  through.Decided(/*closes=*/false);

  PopupHold closed;
  closed.Hold();
  closed.Asking();
  base::MockOnceClosure dropped;
  EXPECT_CALL(dropped, Run).Times(0);
  EXPECT_TRUE(closed.Report(dropped.Get()));
  closed.OpenerGone();
  closed.Decided(/*closes=*/true);
  EXPECT_FALSE(closed.holding());
}

}  // namespace
}  // namespace browser_core
