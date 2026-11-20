// Where the keyboard goes as an overlay of the app's lets it go, apart so browser_core_unittests
// tests the rule alone (#3587).

#ifndef BROWSER_CORE_OVERLAY_LEND_H_
#define BROWSER_CORE_OVERLAY_LEND_H_

#include <cstdint>

#include "base/functional/function_ref.h"

namespace browser_core {

// One window's lend of its keyboard to an overlay, as the + menu or the command bar.
class OverlayLend {
 public:
  enum class Goes {
    kToUI,    // The UI asked for it while the overlay had it, and keeps it.
    kToPage,  // The page shown takes it, as the overlay showed another tab.
    kBack,    // The page or UI that lent it has it back.
  };

  // The overlay took the keyboard with |selected| the window's selected tab.
  void Take(uint64_t selected);

  // The UI asked for the keyboard (bc_window_ui_takes_keyboard), as for a field the overlay's
  // chosen command shows.
  void UIAsked();

  // The overlay let the keyboard go with |selected| selected, |page_shown| saying whether the
  // window shows a page. The UI keeps it when it asked for it since the overlay took it, even with
  // the let-go after the ask, as a Wayland overlay's comes a compositor roundtrip after its hide
  // (#3572). Otherwise the page shown takes it when the overlay showed another tab, as the command
  // bar does going to a page, unless the tab selected as it took it closed, |open| saying which
  // are, as a Space the dialog deleted: what shows then comes of that.
  Goes LetGo(uint64_t selected, bool page_shown, base::FunctionRef<bool(uint64_t)> open);

 private:
  uint64_t under_ = 0;
  bool ui_asked_ = false;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_OVERLAY_LEND_H_
