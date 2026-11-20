#include "browser_core/overlay_lend.h"

#include <utility>

namespace browser_core {

void OverlayLend::Take(uint64_t selected) {
  lent_ = true;
  under_ = selected;
  ui_asked_ = false;
}

void OverlayLend::UIAsked() {
  ui_asked_ = lent_;
}

void OverlayLend::PageAsked() {
  ui_asked_ = false;
}

OverlayLend::Goes OverlayLend::LetGo(uint64_t selected,
                                     bool page_shown,
                                     base::FunctionRef<bool(uint64_t)> open) {
  lent_ = false;
  const uint64_t under = std::exchange(under_, 0);
  if (std::exchange(ui_asked_, false)) {
    return Goes::kToUI;
  }
  if (under != selected && page_shown && (!under || open(under))) {
    return Goes::kToPage;
  }
  return Goes::kBack;
}

}  // namespace browser_core
