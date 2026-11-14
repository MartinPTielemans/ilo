#ifndef BROWSER_CORE_POPUP_HOLD_H_
#define BROWSER_CORE_POPUP_HOLD_H_

#include "base/functional/callback.h"

namespace browser_core {

// A tab a page opens waits for the native ad blocker's pop-up filters before the app lists it,
// so one they close is never listed or shown (#2806). It waits only for their answer about its
// first address, not for its page. Apart from content so browser_core_unittests tests it alone.

// How a new tab came.
struct PopupOpen {
  // Content made it for a page: a link with a target, or window.open(). A tab the user opens
  // (a modified or middle click, Open Link in New Tab, the command bar) Chrome makes itself.
  bool page_made = false;
  // It comes in front, as a page's own tab or pop-up does. A page's tab behind, or in a window of
  // its own, comes from a modified click: the user's.
  bool in_front = false;
  // The page that opened it is in an agent's Space. Its tabs are reported as before, so the
  // agent's answer is as before.
  bool agent = false;
  // Its first address is one the filters judge: http(s), not about:blank or none.
  bool judged = false;
  // The native ad blocker blocks, and not on the opener's site or the tab's.
  bool filtering = false;
};

// Whether the tab |open| says waits for the pop-up filters.
bool HoldsPopup(const PopupOpen& open);

// One tab's report to the app and the filters' answer about its first address, which come in
// either order: content can start a window.open() page's navigation before it shows the window.
class PopupHold {
 public:
  PopupHold();
  PopupHold(const PopupHold&) = delete;
  PopupHold& operator=(const PopupHold&) = delete;
  ~PopupHold();

  // The tab is one HoldsPopup holds, as its tab strip gets it.
  void Hold();
  // The filters were asked about its first address.
  void Asking();
  // Their answer: the page loads or shows its warning, or the tab |closes|.
  void Decided(bool closes);
  // No answer will come: its first navigation ended or never asked, or its page is gone.
  void Ended();
  // The page that opened it went. Before the filters were asked, they won't be.
  void OpenerGone();

  // The tab's report, |report|: false if it goes now, which the caller does, else true, as this
  // keeps it to run at the answer, or drops it for a held tab the filters close. One not held
  // goes now whatever they answer, and closes after, as before.
  bool Report(base::OnceClosure report);

  bool holding() const { return held_ && !answered_; }

 private:
  bool held_ = false;
  bool asking_ = false;
  bool answered_ = false;
  bool closes_ = false;
  base::OnceClosure report_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_POPUP_HOLD_H_
