// Which today's folder a synced Today tab goes in (#4252), apart from the bridge, so
// browser_core_unittests tests the rule alone.

#ifndef BROWSER_CORE_SYNC_TODAY_FOLDERS_H_
#define BROWSER_CORE_SYNC_TODAY_FOLDERS_H_

#include <map>
#include <string>

#include "browser_core/sync_spaces.pb.h"

namespace browser_core::sync_spaces {

// Before a Today tab's or today's folder's node id in its entity's id and client tag. ilo 0.1.13
// and older take a kind they don't know for SPACE, proto2's default, so they'd make a Space of
// each and write it back as one over every device's copy. Their Parse takes only a bare lowercase
// UUID, so they drop these instead (#2108).
inline constexpr char kTodayPrefix[] = "today:";

// The id of the today's folder |tab|, a Today tab's entity, goes in, among |known|, the account's
// entities by id: its today_folder's, if that's a today's folder of the same Space, else empty, so
// a tab whose folder is unknown, pinned or another Space's stays loose. Its parent, a pinned
// folder's field, never puts it in one.
std::string TodayFolderOf(const sync_spaces_pb::SpaceEntity& tab,
                          const std::map<std::string, sync_spaces_pb::SpaceEntity>& known);

// Writes |folder|, the node key of the today's folder holding |tab|, a Today tab's entity, or
// nothing for a tab in none, so an account without today's folders is written as before them and
// an older ilo stops syncing Spaces only once one exists.
void SetTodayFolder(sync_spaces_pb::SpaceEntity& tab, const std::string& folder);

}  // namespace browser_core::sync_spaces

#endif  // BROWSER_CORE_SYNC_TODAY_FOLDERS_H_
