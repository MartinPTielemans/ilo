#include "browser_core/archive.h"

#include <algorithm>
#include <utility>

#include "base/functional/bind.h"
#include "base/no_destructor.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/host.h"
#include "browser_core/session.h"
#include "browser_core/snooze.h"

namespace browser_core {

namespace {

// Ours: Arc's limits aren't known.
constexpr base::TimeDelta kKeepFor = base::Days(30);
constexpr size_t kMaxArchived = 500;

std::vector<ArchivedTab>& Tabs() {
  static base::NoDestructor<std::vector<ArchivedTab>> tabs;
  return *tabs;
}

// What one Delete or Clear took out, under its own id, each tab with the place it had, in that
// order.
struct Removal {
  bc_id id = 0;
  std::vector<std::pair<size_t, ArchivedTab>> tabs;
};

// Newest last, as many as the closed tabs.
constexpr size_t kMaxRemovals = 25;

std::vector<Removal>& Removals() {
  static base::NoDestructor<std::vector<Removal>> removals;
  return *removals;
}

bool g_report_posted = false;

void Report() {
  g_report_posted = false;
  if (!Host().archive_changed) {
    return;
  }
  std::vector<bc_archived_tab> tabs;
  tabs.reserve(Tabs().size());
  for (const ArchivedTab& tab : Tabs()) {
    tabs.push_back({.id = tab.id,
                    .url = tab.url.spec().c_str(),
                    .title = tab.title.c_str(),
                    .archived = tab.archived.InMillisecondsFSinceUnixEpochIgnoringNull(),
                    .little = tab.little,
                    .snoozed_until = tab.snoozed_until.InMillisecondsFSinceUnixEpoch()});
  }
  Host().archive_changed(Host().context, tabs.data(), tabs.size());
}

// Tells the app once, after the call that changed it has returned.
void PostReport() {
  if (g_report_posted) {
    return;
  }
  g_report_posted = true;
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(FROM_HERE, base::BindOnce(&Report));
}

// Saves it with the Spaces, tells the app, and has the snoozed tab due first waited for.
void Changed() {
  SessionChanged();
  PostReport();
  ScheduleSnoozes();
}

// Drops what's older than 30 days, then the oldest over 500. A snoozed tab stays till it's back.
void Prune() {
  const base::Time oldest = base::Time::Now() - kKeepFor;
  std::erase_if(Tabs(), [oldest](const ArchivedTab& tab) {
    return tab.snoozed_until.is_null() && tab.archived < oldest;
  });
  for (size_t index = Tabs().size(); Tabs().size() > kMaxArchived && index-- > 0;) {
    if (Tabs()[index].snoozed_until.is_null()) {
      Tabs().erase(Tabs().begin() + index);
    }
  }
}

}  // namespace

const std::vector<ArchivedTab>& Archived() {
  return Tabs();
}

void LoadArchive(std::vector<ArchivedTab> archived) {
  Tabs() = std::move(archived);
  for (ArchivedTab& tab : Tabs()) {
    tab.id = NewId();
  }
  Prune();
  if (!Tabs().empty()) {
    PostReport();
    ScheduleSnoozes();
  }
}

void Archive(bc_id tab,
             const GURL& url,
             const std::string& title,
             const base::Uuid& space,
             bool little,
             base::Time snoozed_until) {
  Tabs().insert(Tabs().begin(), {.id = NewId(),
                                 .url = url,
                                 .title = title,
                                 .archived = base::Time::Now(),
                                 .tab = tab,
                                 .little = little,
                                 .space = space,
                                 .snoozed_until = snoozed_until});
  Prune();
  Changed();
}

bool SnoozeArchived(bc_id id, base::Time until) {
  auto it = std::ranges::find(Tabs(), id, &ArchivedTab::id);
  if (it == Tabs().end()) {
    return false;
  }
  // One that stays is a tab closed now, its 30 days counted from here, so a long snooze that
  // ends without its Space isn't dropped at once.
  if (until.is_null() && !it->snoozed_until.is_null()) {
    it->archived = base::Time::Now();
  }
  it->snoozed_until = until;
  Changed();
  return true;
}

void Unarchive(bc_id tab) {
  if (std::erase_if(Tabs(), [tab](const ArchivedTab& archived) { return archived.tab == tab; })) {
    Changed();
  }
}

std::optional<ArchivedTab> TakeArchived(bc_id id) {
  auto it = std::ranges::find(Tabs(), id, &ArchivedTab::id);
  if (it == Tabs().end()) {
    return std::nullopt;
  }
  ArchivedTab tab = std::move(*it);
  Tabs().erase(it);
  Changed();
  return tab;
}

void ClearArchive() {
  if (!Tabs().empty()) {
    Tabs().clear();
    Changed();
  }
}

// Keeps |removal| for UndoRemoved, and tells the app. Returns its id.
static bc_id Keep(Removal removal) {
  removal.id = NewId();
  const bc_id id = removal.id;
  Removals().push_back(std::move(removal));
  if (Removals().size() > kMaxRemovals) {
    Removals().erase(Removals().begin());
  }
  Changed();
  return id;
}

bc_id RemoveArchived(bc_id id) {
  auto it = std::ranges::find(Tabs(), id, &ArchivedTab::id);
  if (it == Tabs().end()) {
    return 0;
  }
  Removal removal;
  removal.tabs.emplace_back(static_cast<size_t>(it - Tabs().begin()), std::move(*it));
  Tabs().erase(it);
  return Keep(std::move(removal));
}

bc_id RemoveAllArchived() {
  if (Tabs().empty()) {
    return 0;
  }
  Removal removal;
  for (size_t index = 0; index < Tabs().size(); ++index) {
    removal.tabs.emplace_back(index, std::move(Tabs()[index]));
  }
  Tabs().clear();
  return Keep(std::move(removal));
}

size_t UndoRemoved(bc_id removal_id) {
  auto found = std::ranges::find(Removals(), removal_id, &Removal::id);
  if (found == Removals().end()) {
    return 0;
  }
  Removal removal = std::move(*found);
  Removals().erase(found);
  const size_t count = removal.tabs.size();
  // In order of place, so each lands where it was.
  for (auto& [index, tab] : removal.tabs) {
    Tabs().insert(Tabs().begin() + std::min(index, Tabs().size()), std::move(tab));
  }
  Changed();
  return count;
}

}  // namespace browser_core
