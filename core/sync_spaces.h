// The Spaces, with their pinned tabs, folders and Today tabs, as Chromium's sync type ILO_SPACE,
// for the profile the browser's account syncs (sync.h). Each is an entity by its node's id
// (space.h). A Today tab from another device comes without its page, which loads only when it's
// clicked.

#ifndef BROWSER_CORE_SYNC_SPACES_H_
#define BROWSER_CORE_SYNC_SPACES_H_

#include <memory>

class Profile;

namespace syncer {
class DataTypeController;
}

namespace browser_core::sync_spaces {

// ILO_SPACE's controller for |profile|'s sync.
std::unique_ptr<syncer::DataTypeController> Controller(Profile* profile);

// Something the session keeps changed (SessionChanged), perhaps a Space, a tab in one or a folder.
void Changed();

}  // namespace browser_core::sync_spaces

#endif  // BROWSER_CORE_SYNC_SPACES_H_
