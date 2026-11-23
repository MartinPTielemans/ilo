#include "browser_core/closing_way_back.h"

namespace browser_core {

ClosingWayBack::ClosingWayBack() = default;
ClosingWayBack::~ClosingWayBack() = default;

void ClosingWayBack::Closing(uint64_t tab) {
  closing_.try_emplace(tab);
}

bool ClosingWayBack::IsClosing(uint64_t tab) const {
  return closing_.contains(tab);
}

void ClosingWayBack::Left(uint64_t tab, uint64_t next) {
  // Only the first leaving counts: the Space forgot where its tabs came from then.
  if (auto closing = closing_.find(tab); closing != closing_.end() && !closing->second) {
    closing->second = next;
  }
}

void ClosingWayBack::Stayed(uint64_t tab) {
  closing_.erase(tab);
}

uint64_t ClosingWayBack::Closed(uint64_t tab, uint64_t next) {
  auto closing = closing_.find(tab);
  if (closing == closing_.end()) {
    return next;
  }
  const std::optional<uint64_t> kept = closing->second;
  closing_.erase(closing);
  return kept.value_or(next);
}

}  // namespace browser_core
