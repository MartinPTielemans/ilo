#ifndef BROWSER_CORE_SYNC_OTHER_TABS_H_
#define BROWSER_CORE_SYNC_OTHER_TABS_H_

class Profile;

namespace browser_core {

// Tells the app of the tabs open on the account's other computers, from Chromium's open tabs sync,
// for the command bar (bc_host's other_tabs_changed, #1234): |profile|'s, the profile that syncs,
// now and as they change. Again for the same profile does nothing.
void WatchOtherTabs(Profile* profile);

// Tells the app again if the list changed for a reason of this computer's: the command bar leaves
// out the tabs in a Space here, so a tab sync puts in a Space leaves the list.
void RefreshOtherTabs();

}  // namespace browser_core

#endif  // BROWSER_CORE_SYNC_OTHER_TABS_H_
