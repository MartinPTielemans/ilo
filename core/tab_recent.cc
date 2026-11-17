#include "browser_core/tab_recent.h"

#include <optional>
#include <set>
#include <string>
#include <vector>

#include "base/functional/bind.h"
#include "base/no_destructor.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "browser_core/favorites.h"
#include "browser_core/host.h"
#include "browser_core/live_folder.h"
#include "browser_core/live_folder_rule.h"
#include "browser_core/profiles.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "browser_core/tab_recent_rule.h"
#include "chrome/browser/profiles/profile.h"
#include "content/public/browser/web_contents.h"

namespace browser_core {

namespace {

static_assert(tab_recent::kPullRequests == BC_LIVE_PULL_REQUESTS);

// The tabs waiting on an answer, each once.
std::set<bc_id>& Waiting() {
  static base::NoDestructor<std::set<bc_id>> waiting;
  return *waiting;
}

// |id| as its card's ask reads it, or none once it's gone.
std::optional<tab_recent::Asker> AskerOf(bc_id id) {
  const Tab* tab = Tab::Get(id);
  if (!tab) {
    return std::nullopt;
  }
  const Space* space = Space::Holding(id);
  const bool favorite = Favorites::Has(id);
  tab_recent::Asker asker;
  asker.kept = favorite || (space && space->IsPinned(id));
  asker.users = favorite || (space && !space->agent() && !space->own());
  content::WebContents* contents = tab->web_contents();
  // Not a page it's only loading: an unloaded tab is at the page it committed last.
  asker.committed = contents ? contents->GetLastCommittedURL() : tab->Url();
  const std::string dir =
      favorite ? Favorites::ProfileOf(id) : (space ? space->profile() : std::string());
  // The profile its page is on, never another the Space's falls back to, and never off the record.
  Profile* profile = tab->profile();
  if (profile && !dir.empty() && !profile->IsOffTheRecord() && DirOf(profile) == dir) {
    asker.profile = dir;
  }
  return asker;
}

// Answers the card of |id|, which asked |source| with the profile |asked|, unless it no longer
// asks that. Nothing of the rows is logged, here or by the app.
void Deliver(bc_id id, int source, const std::string& asked, const live_folder::Kept& kept) {
  if (!Waiting().erase(id)) {
    return;
  }
  const std::optional<live_folder::Outcome> outcome =
      tab_recent::OutcomeFor(source, asked, AskerOf(id), kept);
  if (!outcome || !Host().tab_recent_answered) {
    return;
  }
  std::vector<bc_live_item> rows;
  rows.reserve(outcome->items.size());
  for (const live_folder::Item& item : outcome->items) {
    rows.push_back({.list = static_cast<int>(item.list),
                    .repo = item.repo.c_str(),
                    .number = item.number,
                    .title = item.title.c_str(),
                    .state = static_cast<int>(item.state)});
  }
  Host().tab_recent_answered(Host().context, id, static_cast<int>(outcome->kind),
                             kept.answered.InMillisecondsSinceUnixEpoch(), rows.data(),
                             rows.size());
}

}  // namespace

void FetchTabRecent(bc_id id) {
  const std::optional<tab_recent::Asker> asker = AskerOf(id);
  const int source = asker ? tab_recent::SourceOf(*asker) : 0;
  if (!source || !Waiting().insert(id).second) {
    return;
  }
  if (asker->profile.empty()) {
    // Its profile isn't loaded, or isn't its Space's: it fails a task later, as a fetch that went
    // would, and no lists is never an answer.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Deliver, id, source, std::string(),
                                  live_folder::Kept{.answered = base::Time::Now()}));
    return;
  }
  // The card has no Refresh of its own: only a Live Folder's is the user's ask past the floor.
  FetchSourceLists(Tab::Get(id)->profile(), source, /*refresh=*/false,
                   base::BindOnce(&Deliver, id, source, asker->profile));
}

}  // namespace browser_core
