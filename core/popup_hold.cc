#include "browser_core/popup_hold.h"

#include <utility>

namespace browser_core {

bool HoldsPopup(const PopupOpen& open) {
  return open.page_made && open.in_front && !open.agent && open.judged && open.filtering;
}

PopupHold::PopupHold() = default;
PopupHold::~PopupHold() = default;

void PopupHold::Hold() {
  held_ = true;
}

void PopupHold::Asking() {
  asking_ = true;
}

void PopupHold::Decided(bool closes) {
  if (answered_) {
    return;
  }
  answered_ = true;
  closes_ = closes;
  if (closes_) {
    report_.Reset();
  } else if (report_) {
    std::move(report_).Run();
  }
}

void PopupHold::Ended() {
  Decided(/*closes=*/false);
}

void PopupHold::OpenerGone() {
  if (!asking_) {
    Ended();
  }
}

bool PopupHold::Report(base::OnceClosure report) {
  if (!held_) {
    return false;
  }
  if (!answered_) {
    report_ = std::move(report);
    return true;
  }
  return closes_;
}

}  // namespace browser_core
