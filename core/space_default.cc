#include "browser_core/space_default.h"

namespace browser_core {

bool GivesWay(const FirstSpace& space) {
  return !space.named && space.pinned_rows == 0 && space.favorites == 0;
}

}  // namespace browser_core
