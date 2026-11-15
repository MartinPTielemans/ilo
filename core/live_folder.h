#ifndef BROWSER_CORE_LIVE_FOLDER_H_
#define BROWSER_CORE_LIVE_FOLDER_H_

#include "browser_core/public/browser_core.h"

namespace browser_core {

// Asks GitHub for the Live Folder's lists on its Space's profile's own session, and answers with
// folder_live_answered once both lists have come, a task later at the soonest (#2979). Does
// nothing for a plain folder, one in an agent's Space or a window's own, or one whose fetch is
// under way. Never asks again on its own: when to ask is the app's.
void FetchLiveFolder(bc_id folder);

// Drops the folder's fetch under way, which answers nothing: it was made a plain folder.
void DropLiveFolderFetch(bc_id folder);

}  // namespace browser_core

#endif  // BROWSER_CORE_LIVE_FOLDER_H_
