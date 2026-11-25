#include "browser_core/handed_history.h"

namespace browser_core::handed_history {

void Bound::Set(const std::vector<int>& entries, int current) {
  users_ = std::set<int>(entries.begin(), entries.end());
  users_.erase(current);
}

bool Bound::MayGoTo(int entry, bool held, bool from_page) const {
  return (held && !from_page) || !users_.contains(entry);
}

}  // namespace browser_core::handed_history
