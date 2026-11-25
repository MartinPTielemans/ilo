#include "browser_core/sync_today_folders.h"

namespace browser_core::sync_spaces {

std::string TodayFolderOf(const sync_spaces_pb::SpaceEntity& tab,
                          const std::map<std::string, sync_spaces_pb::SpaceEntity>& known) {
  using Entity = sync_spaces_pb::SpaceEntity;
  if (tab.kind() != Entity::TODAY || tab.parent().empty()) {
    return std::string();
  }
  const auto folder = known.find(kTodayPrefix + tab.parent());
  if (folder == known.end() || folder->second.kind() != Entity::TODAY_FOLDER ||
      folder->second.space() != tab.space()) {
    return std::string();
  }
  return folder->first;
}

}  // namespace browser_core::sync_spaces
