#ifndef BROWSER_CORE_HISTORY_DELETIONS_H_
#define BROWSER_CORE_HISTORY_DELETIONS_H_

class Profile;

namespace browser_core {

// Tells the app's history_deleted whenever the user deletes pages from |profile|'s history, from
// Clear browsing data, the History page or the command bar, so it forgets what it learned of them.
void ObserveHistoryDeletions(Profile* profile);

}  // namespace browser_core

#endif  // BROWSER_CORE_HISTORY_DELETIONS_H_
