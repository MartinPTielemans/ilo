// The Spaces, with their pinned tabs and folders, as Chromium's sync type ILO_SPACE, for the
// profile the browser's account syncs (sync.h). Each Space, pinned tab and folder is an entity by its
// node's id (space.h); Today tabs stay on the device.

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

// Something the session keeps changed (SessionChanged), perhaps a Space, a pinned tab or a folder.
void Changed();

}  // namespace browser_core::sync_spaces

#endif  // BROWSER_CORE_SYNC_SPACES_H_
