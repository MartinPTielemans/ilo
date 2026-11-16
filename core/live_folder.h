#ifndef BROWSER_CORE_LIVE_FOLDER_H_
#define BROWSER_CORE_LIVE_FOLDER_H_

#include "base/functional/callback.h"
#include "browser_core/live_folder_rule.h"
#include "browser_core/public/browser_core.h"

class Profile;

namespace browser_core {

// Answers |done| with |source|'s lists, a BC_LIVE_ source's, on |profile|'s own session, never
// inside this call. One fetch serves all who ask for a profile's source, so the site is asked at
// most once each live_folder::kFloor, however many ask: inside that, |done| gets what it last
// gave. A |refresh|, the user's own ask, fetches inside the floor too, but joins a fetch under way.
// The caller checks who asks and that |profile| is theirs, the user's own and loaded; the rows are
// that profile's alone, and nothing of them is logged. A Live Folder asks with FetchLiveFolder,
// and a pinned tab's card with this (#3074).
void FetchSourceLists(Profile* profile,
                      int source,
                      bool refresh,
                      base::OnceCallback<void(const live_folder::Kept&)> done);

// Asks for the Live Folder's lists on its Space's profile's own session (FetchSourceLists), and
// answers with folder_live_answered, a task later at the soonest (#2979). Does nothing for a plain
// folder, one in an agent's Space or a window's own, or one waiting on an answer already. Never
// asks again on its own: when to ask is the app's, and how often the core's.
void FetchLiveFolder(bc_id folder, bool refresh);

// The folder was made plain: it's answered nothing for what it asked.
void DropLiveFolderFetch(bc_id folder);

}  // namespace browser_core

#endif  // BROWSER_CORE_LIVE_FOLDER_H_
