#ifndef BROWSER_CORE_ARCHIVE_H_
#define BROWSER_CORE_ARCHIVE_H_

#include <optional>
#include <string>
#include <vector>

#include "base/time/time.h"
#include "base/uuid.h"
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
  // The node of the Space it closed in, which restoring it goes back to, as Arc's does, while
  // that's still one of the user's Spaces. Invalid for a window's own Space, which has none, and
  // for one archived before the archive kept it.
  base::Uuid space;
  // Whether the user closed it, which ⇧⌘T brings back, rather than auto-archive's timer, whose tabs
  // the Library alone brings back (#4408).
  bool reopens = false;
  // The directory of its Space's profile, whose windows' ⇧⌘T bring it back.
  std::string profile;
};

// All of it, newest first.
const std::vector<ArchivedTab>& Archived();

// Puts back what the last launch saved, as the browser starts. Then archive_changed.
void LoadArchive(std::vector<ArchivedTab> archived);

// Archives |tab|, a Today tab closing at |url| in the Space with the node |space| of the profile
// in the directory |profile|, or with |little| a Little ilo window's tab, and with |reopens| one
// the user closed. Then archive_changed.
void Archive(bc_id tab,
             const GURL& url,
             const std::string& title,
             const base::Uuid& space,
             bool little,
             bool reopens,
             const std::string& profile);
// Whether the archive keeps the closed tab |tab|, or a Delete or Clear its Undo can put back does:
// its history is kept as long (ReopenClosedTab).
bool ArchiveHolds(bc_id tab);
// The closed tabs whose history the archive keeps of those the user closed, by their |tab|: the
// newest 25 (closed_rule::WithHistory), which the session file keeps with their rows. The rest
// come back at their page.
std::vector<bc_id> ArchiveWithHistory();

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
