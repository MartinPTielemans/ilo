#ifndef BROWSER_CORE_ARCHIVE_H_
#define BROWSER_CORE_ARCHIVE_H_

#include <optional>
#include <string>
#include <vector>

#include "base/time/time.h"
#include "browser_core/public/browser_core.h"
#include "url/gurl.h"

namespace browser_core {

// The archive: the Today tabs that closed, as Arc keeps them for its Library, newest first. It's
// saved with the Spaces, and keeps a tab for 30 days, up to 500 of them.
struct ArchivedTab {
  // Its own id, for the app.
  bc_id id = 0;
  GURL url;
  std::string title;
  base::Time archived;
  // The tab it was, while it can still come back with its history, else 0.
  bc_id tab = 0;
  // Whether it closed in a Little ilo window, as the Library's filter picks them out (Arc).
  bool little = false;
};

// All of it, newest first.
const std::vector<ArchivedTab>& Archived();

// Puts back what the last launch saved, as the browser starts. Then archive_changed.
void LoadArchive(std::vector<ArchivedTab> archived);

// Archives |tab|, a Today tab closing at |url|, or with |little| a Little ilo window's tab. Then
// archive_changed.
void Archive(bc_id tab, const GURL& url, const std::string& title, bool little = false);

// Takes |tab|, a closed tab that has come back, out of the archive. Then archive_changed.
void Unarchive(bc_id tab);

// Takes the archived tab |id| out of the archive and returns it. Then archive_changed.
std::optional<ArchivedTab> TakeArchived(bc_id id);

// Empties it. Then archive_changed.
void ClearArchive();

// As TakeArchived without returning it, and ClearArchive, for the user's Delete and Clear in the
// Library, which UndoRemoved brings back. Then archive_changed. Return the removal's id, or 0 if
// there was nothing to remove. The last 25 are kept, whichever window made them.
bc_id RemoveArchived(bc_id id);
bc_id RemoveAllArchived();

// Puts back what the removal |removal| took out, each tab with its id at the place it had, or the
// end if the archive is shorter now. Then archive_changed. Returns how many, 0 if it is gone.
size_t UndoRemoved(bc_id removal);

}  // namespace browser_core

#endif  // BROWSER_CORE_ARCHIVE_H_
