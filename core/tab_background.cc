#include "browser_core/tab_background.h"

#include <algorithm>
#include <utility>

namespace browser_core {

TabBackground::Step TabBackground::Changed(std::optional<uint32_t> argb,
                                           bool shown,
                                           base::TimeTicks now) {
  page_ = argb && *argb >> 24 == 0xff ? std::optional(*argb & 0xffffff) : std::nullopt;
  return Keep(shown, now);
}

TabBackground::Step TabBackground::Covered(bool covered, bool shown, base::TimeTicks now) {
  covered_ = covered;
  return Keep(shown, now);
}

TabBackground::Step TabBackground::Keep(bool shown, base::TimeTicks now) {
  const std::optional<uint32_t> color = covered_ ? std::nullopt : page_;
  if (color == color_) {
    return {};
  }
  const bool changing = !changed_at_.is_null() && now - changed_at_ < kSettle;
  color_ = color;
  changed_at_ = now;
  if (!shown || waiting_) {
    return {};
  }
  if (!changing) {
    return {.tell = true};
  }
  waiting_ = true;
  return {.wait = kWaits[wait_]};
}

TabBackground::Step TabBackground::Settled(bool shown, base::TimeTicks now) {
  if (!waiting_) {
    return {};
  }
  if (!shown) {
    waiting_ = false;
    wait_ = 0;
    return {};
  }
  // Still changing: the next wait. A settle time's is only what's left of it since the last
  // change, so the colour a fade ends on is told a settle time after it.
  if (now - changed_at_ < kSettle) {
    wait_ = std::min(wait_ + 1, kWaits.size() - 1);
    return {.wait = kWaits[wait_] == kSettle ? kSettle - (now - changed_at_) : kWaits[wait_]};
  }
  waiting_ = false;
  wait_ = 0;
  return {.tell = true};
}

TabBackground::Step TabBackground::LeftPage(bool shown) {
  wait_ = 0;
  return {.tell = std::exchange(waiting_, false) && shown};
}

}  // namespace browser_core
