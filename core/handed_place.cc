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

}  // namespace browser_core::handed_place
