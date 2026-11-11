#include "browser_core/snooze.h"

#include <algorithm>
#include <map>
#include <optional>
#include <vector>

#include "base/containers/adapters.h"
#include "base/functional/bind.h"
#include "base/logging.h"
#include "base/no_destructor.h"
#include "base/task/sequenced_task_runner.h"
#include "base/task/single_thread_task_runner.h"
#include "base/timer/wall_clock_timer.h"
#include "browser_core/archive.h"
#include "browser_core/favorites.h"
#include "browser_core/host.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "chrome/browser/after_startup_task_utils.h"
#include "chrome/browser/lifetime/browser_shutdown.h"

namespace browser_core {

namespace {

// By tab, the snoozes waiting for their tab's close.
std::map<bc_id, base::Time>& Pending() {
  static base::NoDestructor<std::map<bc_id, base::Time>> pending;
  return *pending;
}

base::WallClockTimer& Timer() {
  static base::NoDestructor<base::WallClockTimer> timer;
  return *timer;
}

bool g_started = false;
bool g_schedule_posted = false;

void Schedule();

void TellUnsnoozed(bc_id tab) {
  if (Host().tab_unsnoozed && Tab::Get(tab)) {
    Host().tab_unsnoozed(Host().context, tab);
  }
}

// Brings back each snoozed tab whose time has come.
void ComeBack() {
  if (browser_shutdown::IsTryingToQuit()) {
    return;
  }
  const base::Time now = base::Time::Now();
  std::vector<bc_id> due;
  for (const ArchivedTab& tab : Archived()) {
    if (!tab.snoozed_until.is_null() && tab.snoozed_until <= now) {
      due.push_back(tab.id);
    }
  }
  // Oldest first, so the one archived last ends up on top.
  for (bc_id id : base::Reversed(due)) {
    const bc_id tab = Window::Unsnooze(id);
    if (!tab) {
      // Its Space is gone: it stays in the archive as any closed tab.
      SnoozeArchived(id, base::Time());
      continue;
    }
    // check.sh reads this line, an error since our builds print no warnings.
    LOG(ERROR) << "browser: tab " << tab << " back from snooze";
    // Posted, after the Space's change, so the app has the tab when it hears.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&TellUnsnoozed, tab));
  }
  Schedule();
}

void Schedule() {
  g_schedule_posted = false;
  if (!g_started) {
    return;
  }
  std::optional<base::Time> next;
  for (const ArchivedTab& tab : Archived()) {
    if (!tab.snoozed_until.is_null()) {
      next = next ? std::min(*next, tab.snoozed_until) : tab.snoozed_until;
    }
  }
  if (next) {
    Timer().Start(FROM_HERE, *next, base::BindOnce(&ComeBack));
  } else {
    Timer().Stop();
  }
}

void Start() {
  g_started = true;
  Schedule();
}

}  // namespace

bool SnoozeTab(bc_id id, base::Time until) {
  Space* space = Space::Holding(id);
  // What auto-archive and Close Tab archive: not a pinned tab or favorite, which stay, nor a
  // private, Little ilo or Blank window's, which have no Space of the user's to come back to, nor
  // an agent's (#1408).
  if (!Tab::Get(id) || !space || space->IsPinned(id) || Favorites::Has(id) ||
      space->off_the_record() || space->little() || space->blank() || space->agent() ||
      until <= base::Time::Now()) {
    return false;
  }
  Pending()[id] = until;
  Window::CloseTab(id);
  return true;
}

base::Time TakeSnooze(bc_id tab) {
  auto it = Pending().find(tab);
  if (it == Pending().end()) {
    return base::Time();
  }
  const base::Time until = it->second;
  Pending().erase(it);
  return until;
}

void ForgetSnooze(bc_id tab) {
  Pending().erase(tab);
}

void ScheduleSnoozes() {
  if (g_schedule_posted || !g_started) {
    return;
  }
  g_schedule_posted = true;
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(FROM_HERE, base::BindOnce(&Schedule));
}

void SnoozesStarted() {
  // Chrome's startup is done once the window's first page has drawn, or soon after a launch with
  // none to draw.
  AfterStartupTaskUtils::PostTask(FROM_HERE, base::SequencedTaskRunner::GetCurrentDefault(),
                                  base::BindOnce(&Start));
}

}  // namespace browser_core
