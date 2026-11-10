#include "browser_core/sync_spaces_newer.h"

namespace browser_core::sync_spaces {

bool WrittenByNewer(const sync_spaces_pb::SpaceEntity& entity) {
  return !entity.unknown_fields().empty();
}

}  // namespace browser_core::sync_spaces
