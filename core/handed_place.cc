#include "browser_core/handed_place.h"

namespace browser_core::handed_place {

size_t Among(const std::vector<Row>& today) {
  size_t at = 0;
  for (const Row& row : today) {
    if (row.before) {
      ++at;
    }
  }
  while (at < today.size() && today[at].joined_above) {
    ++at;
  }
  return at;
}

size_t InSplit(const std::vector<bool>& was_before) {
  size_t at = 0;
  for (size_t pane = 0; pane < was_before.size(); ++pane) {
    if (was_before[pane]) {
      at = pane + 1;
    }
  }
  return at;
}

}  // namespace browser_core::handed_place
