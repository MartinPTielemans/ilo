#ifndef BROWSER_CORE_TAB_RECENT_H_
#define BROWSER_CORE_TAB_RECENT_H_

#include "browser_core/public/browser_core.h"

namespace browser_core {

// Asks what's recent on the site of a pinned tab or a favorite, for its hover card, on its
// profile's own session with the Live Folder's fetch (FetchSourceLists), and answers with
// tab_recent_answered, a task later at the soonest (#3074). Does nothing for a tab that asks
// nothing (tab_recent::SourceOf) or one waiting on an answer already. Never asks again on its own.
void FetchTabRecent(bc_id tab);

}  // namespace browser_core

#endif  // BROWSER_CORE_TAB_RECENT_H_
