#include "browser_core/sync_spaces_handed.h"

namespace browser_core::sync_spaces {

void SendAsKnown(const std::string& key,
                 const std::map<std::string, sync_spaces_pb::SpaceEntity>& known,
                 std::map<std::string, sync_spaces_pb::SpaceEntity>& sent) {
  if (auto was = known.find(key); was != known.end()) {
    sent[key] = was->second;
  } else {
    sent.erase(key);
  }
}

}  // namespace browser_core::sync_spaces
