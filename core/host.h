#ifndef BROWSER_CORE_HOST_H_
#define BROWSER_CORE_HOST_H_

#include "browser_core/public/browser_core.h"

namespace browser_core {

// The app's callbacks, from bc_main.
const bc_host& Host();

// A window's or tab's id, unique among both, and among those of earlier launches once the session
// is read (StartIdsAfter).
bc_id NewId();

// The last id NewId handed out, which the session keeps.
bc_id LastId();

// The last launch's session kept |last|: the ids from here on start past it, and past those that
// launch may have handed out after it saved, so an id never names a second tab across launches.
void StartIdsAfter(bc_id last);

// Whether |id| is below every id this launch handed out once the session was read: one an earlier
// launch handed out, if it names nothing now.
bool FromEarlierLaunch(bc_id id);

}  // namespace browser_core

#endif  // BROWSER_CORE_HOST_H_
