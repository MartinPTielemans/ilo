#include "browser_core/tab_background.h"

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
  return {.wait = kSettle};
}

TabBackground::Step TabBackground::Settled(bool shown, base::TimeTicks now) {
  if (!waiting_) {
    return {};
  }
  if (!shown) {
    waiting_ = false;
    return {};
  }
  // Still changing: the same wait goes on for what's left of the last change's time.
  if (now - changed_at_ < kSettle) {
    return {.wait = kSettle - (now - changed_at_)};
  }
  waiting_ = false;
  return {.tell = true};
}

TabBackground::Step TabBackground::LeftPage(bool shown) {
  return {.tell = std::exchange(waiting_, false) && shown};
}

}  // namespace browser_core
