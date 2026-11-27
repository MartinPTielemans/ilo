#include "browser_core/agent_tabs_close.h"

#include <algorithm>
#include <map>
#include <memory>
#include <optional>
#include <vector>

#include "base/functional/bind.h"
#include "base/no_destructor.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "base/timer/wall_clock_timer.h"
#include "browser_core/agent_close.h"
#include "browser_core/agent_session.h"
#include "browser_core/agent_tabs_close_rule.h"
#include "browser_core/handed.h"
#include "browser_core/host.h"
#include "browser_core/session.h"
#include "browser_core/space.h"
#include "browser_core/window.h"
#include "chrome/browser/lifetime/browser_shutdown.h"

namespace browser_core {
namespace {

namespace rule = agent_tabs_close_rule;

struct Thread {
  rule::Clock clock;
  base::WallClockTimer timer;
};

// By agent id. WallClockTimer doesn't move, so each is held by pointer.
std::map<std::string, std::unique_ptr<Thread>>& Threads() {
  static base::NoDestructor<std::map<std::string, std::unique_ptr<Thread>>> threads;
  return *threads;
}

std::optional<base::TimeDelta> After() {
  return rule::After(UserSettings().values.agent_tabs_close_after);
}

rule::Tab FactsOf(const Space& space, bc_id tab) {
  return {
      .handed = handed::Of(tab) != nullptr,
      .held = IsHeld(tab),
      .pinned = space.IsPinned(tab),
      .shown = Window::Showing(tab) || Window::Picturing(tab),
  };
}

// The tabs of |agent|'s Spaces, one a profile (#4376), with what the rule asks of each.
std::vector<std::pair<bc_id, rule::Tab>> TabsOf(const std::string& agent) {
  std::vector<std::pair<bc_id, rule::Tab>> tabs;
  for (const Space* space : Space::All()) {
    if (!space->agent() || space->agent()->id != agent) {
      continue;
    }
    // Pinned ones never close, so the Today tabs are all it looks at.
    for (bc_id tab : space->today()) {
      tabs.emplace_back(tab, FactsOf(*space, tab));
    }
  }
  return tabs;
}

// Closes |tab| for the thread, its page asked as an agent's close asks it: one that asks to stay
// stays. The activity log hears of it as the agent's, by the browser.
void Close(const std::string& agent, bc_id tab) {
  if (Host().agent_activity) {
    const bc_agent_activity entry = {
        .agent = agent.c_str(),
        .tab = tab,
        .action = "close_idle_tab",
        .refused = "",
        .failure = BC_AGENT_FAILURE_NONE,
    };
    Host().agent_activity(Host().context, &entry);
  }
  AgentCloseTab(tab, /*discard=*/false);
}

void CloseDue(const std::string& agent) {
  if (browser_shutdown::IsTryingToQuit()) {
    return;
  }
  auto it = Threads().find(agent);
  if (it == Threads().end() || !it->second->clock.Due(base::Time::Now(), After())) {
    return;
  }
  for (const auto& [tab, facts] : TabsOf(agent)) {
    if (rule::ClosesNow(facts)) {
      Close(agent, tab);
    }
  }
  // Shown ones close as they're hidden (ThreadTabHidden); nothing is armed until it acts again.
}

void Arm(const std::string& agent, Thread& thread) {
  thread.timer.Stop();
  const std::optional<base::Time> due = thread.clock.DueAt(After());
  if (!due) {
    return;
  }
  const std::vector<std::pair<bc_id, rule::Tab>> tabs = TabsOf(agent);
  if (std::ranges::none_of(tabs, [](const auto& tab) { return rule::Own(tab.second); })) {
    return;
  }
  thread.timer.Start(FROM_HERE, *due, base::BindOnce(&CloseDue, agent));
}

void CloseIfDue(bc_id tab) {
  const Space* space = Space::Holding(tab);
  if (browser_shutdown::IsTryingToQuit() || !space || !space->agent()) {
    return;
  }
  const std::string agent = space->agent()->id;
  auto it = Threads().find(agent);
  if (it == Threads().end() || !it->second->clock.Due(base::Time::Now(), After())) {
    return;
  }
  if (rule::ClosesNow(FactsOf(*space, tab))) {
    Close(agent, tab);
  }
}

}  // namespace

void ThreadWorking(const std::string& agent, bool working) {
  std::unique_ptr<Thread>& thread = Threads()[agent];
  if (!thread) {
    thread = std::make_unique<Thread>();
  }
  thread->clock.SetWorking(working, base::Time::Now());
  Arm(agent, *thread);
}

void ThreadActed(const std::string& agent) {
  auto it = Threads().find(agent);
  if (it == Threads().end()) {
    return;
  }
  it->second->clock.Acted(base::Time::Now());
  // Posted: the call that acted may be opening the tab it acts in, which the Space holds after.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](const std::string& agent) {
                       auto it = Threads().find(agent);
                       if (it != Threads().end()) {
                         Arm(agent, *it->second);
                       }
                     },
                     agent));
}

void ThreadTabsRearm() {
  for (auto& [agent, thread] : Threads()) {
    Arm(agent, *thread);
  }
}

void ThreadTabHidden(bc_id tab) {
  if (Threads().empty()) {
    return;
  }
  // Posted: the window is still changing what it shows, and may show it again in the same task.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&CloseIfDue, tab));
}

}  // namespace browser_core
