#ifndef BROWSER_CORE_SNOOZE_H_
#define BROWSER_CORE_SNOOZE_H_

#include "base/time/time.h"
#include "browser_core/public/browser_core.h"

namespace browser_core {

// Snoozed tabs (#2350): a Today tab closed into the archive until a time, which then comes back by
// itself at the top of its own Space's Today tabs, unloaded. One timer, on the wall clock so it
// keeps time through sleep, waits for the snoozed tab due first, and none runs while nothing is
// snoozed. It starts once the browser's startup is done, so a tab that fell due while the browser
// was closed comes back after the window is on screen.

// Closes |tab| into the archive snoozed until |until|, as bc_tab_close closes it. False, and it
// stays open, for one that isn't a Today tab of the user's own Spaces, or a time that has passed.
bool SnoozeTab(bc_id tab, base::Time until);

// The snooze |tab|'s close is to archive it with, taken, or null for none. A page that kept its
// tab open with "Leave site?" keeps its snooze for the tab's next close.
base::Time TakeSnooze(bc_id tab);

// The user closed |tab| themselves: no snooze waits for its close.
void ForgetSnooze(bc_id tab);

// Waits for the snoozed tab due first again, from a task of its own, as the archive changes.
void ScheduleSnoozes();

// The Spaces are read and the browser has started: the timer starts once Chrome's startup is
// done.
void SnoozesStarted();

}  // namespace browser_core

#endif  // BROWSER_CORE_SNOOZE_H_
