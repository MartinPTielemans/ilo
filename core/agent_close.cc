#include "browser_core/agent_close.h"

#include <map>
#include <memory>

#include "base/functional/bind.h"
#include "base/logging.h"
#include "base/no_destructor.h"
#include "base/process/kill.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "browser_core/agent_close_rule.h"
#include "browser_core/agent_session.h"
#include "browser_core/handed.h"
#include "browser_core/host.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_observer.h"

namespace browser_core {

namespace {

// Chromium gives a page's beforeunload 500 ms, then its unload handlers 500 ms, before it treats it
// as hung, but not while DevTools is attached, as the agent's own session is. This is the most an
// agent's close waits on a page.
constexpr base::TimeDelta kAnswerWithin = base::Seconds(2);

int AnswerOf(AgentCloseAsk::Answer answer) {
  switch (answer) {
    case AgentCloseAsk::Answer::kClosed:
      return BC_AGENT_CLOSED;
    case AgentCloseAsk::Answer::kAsks:
      return BC_AGENT_CLOSE_ASKS;
    case AgentCloseAsk::Answer::kStays:
      return BC_AGENT_CLOSE_REFUSED;
  }
}

void Answer(bc_id tab, int answer) {
  // check.sh reads this line.
  LOG(ERROR) << "browser: agent close " << tab << " answered " << answer;
  if (Host().agent_close_answered) {
    Host().agent_close_answered(Host().context, tab, answer);
  }
}

// One tab whose page an agent's close is asking, until it's answered.
class AgentClose : public content::WebContentsObserver {
 public:
  AgentClose(bc_id tab, content::WebContents* contents);
  AgentClose(const AgentClose&) = delete;
  AgentClose& operator=(const AgentClose&) = delete;
  ~AgentClose() override = default;

 private:
  void Do(AgentCloseAsk::Step step);
  // The core's own close, after a crash or the deadline: it looks again at whose the tab is.
  static void CloseNow(bc_id tab);

  // content::WebContentsObserver:
  void BeforeUnloadFired(bool proceed) override;
  void PrimaryPageChanged(content::Page& page) override;
  void PrimaryMainFrameRenderProcessGone(base::TerminationStatus status) override;
  void WebContentsDestroyed() override;

  const bc_id tab_;
  AgentCloseAsk ask_;
  base::OneShotTimer deadline_;
};

std::map<bc_id, std::unique_ptr<AgentClose>>& Asking() {
  static base::NoDestructor<std::map<bc_id, std::unique_ptr<AgentClose>>> asking;
  return *asking;
}

AgentClose::AgentClose(bc_id tab, content::WebContents* contents)
    : content::WebContentsObserver(contents), tab_(tab) {
  deadline_.Start(FROM_HERE, kAnswerWithin,
                  base::BindOnce([](AgentClose* close) { close->Do(close->ask_.TimedOut()); },
                                 base::Unretained(this)));
}

void AgentClose::Do(AgentCloseAsk::Step step) {
  // Each posted: content is telling its observers, and this may be inside the agent's call, which
  // the answer never is.
  if (step.close) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&AgentClose::CloseNow, tab_));
  }
  if (!step.answer) {
    return;
  }
  const bc_id tab = tab_;
  const int answer = AnswerOf(*step.answer);
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(FROM_HERE,
                                                              base::BindOnce(&Answer, tab, answer));
  // Deletes this.
  Asking().erase(tab);
}

// static
void AgentClose::CloseNow(bc_id tab) {
  auto asking = Asking().find(tab);
  if (asking == Asking().end()) {
    return;
  }
  AgentClose* close = asking->second.get();
  const Space* space = Space::Holding(tab);
  const bool users = IsHeld(tab) || !space || !space->agent() || space->IsPinned(tab);
  AgentCloseAsk::Step step = close->ask_.Closing(users);
  if (!step.close) {
    close->Do(step);
    return;
  }
  LOG(ERROR) << "browser: agent close " << tab << " closes without its handlers";
  // Its page going answers, and deletes |close|, within the call.
  Window::CloseTab(tab, /*discard=*/true);
  asking = Asking().find(tab);
  if (asking != Asking().end()) {
    close = asking->second.get();
    close->Do(close->ask_.StillThere());
  }
}

void AgentClose::BeforeUnloadFired(bool proceed) {
  Do(ask_.PageAnswered(proceed));
}

void AgentClose::PrimaryPageChanged(content::Page& page) {
  Do(ask_.Navigated());
}

void AgentClose::PrimaryMainFrameRenderProcessGone(base::TerminationStatus status) {
  Do(ask_.RendererGone());
}

void AgentClose::WebContentsDestroyed() {
  const Tab* tab = Tab::Get(tab_);
  Do(ask_.PageGone(/*tab_stays=*/tab && tab->unloading()));
}

}  // namespace

int AgentCloseTab(bc_id tab, bool discard) {
  if (Asking().contains(tab) && !discard) {
    return BC_AGENT_CLOSE_PENDING;
  }
  const Tab* closing = Tab::Get(tab);
  content::WebContents* contents = closing ? closing->web_contents() : nullptr;
  if (discard || !contents || !closing->AsksBeforeLeaving()) {
    // Without its unload handlers: no frame has a beforeunload handler, or the agent said to lose
    // what it holds, and the tab is gone before the agent hears it closed (#1447, #1449). Its going
    // answers a close that was asking.
    Window::CloseTab(tab, /*discard=*/true);
    // A tab being dragged between windows stays.
    return Tab::FromId(tab) ? BC_AGENT_CLOSE_REFUSED : BC_AGENT_CLOSED;
  }
  LOG(ERROR) << "browser: agent close " << tab << " asks its page";
  Asking()[tab] = std::make_unique<AgentClose>(tab, contents);
  // Cancels a navigation it has under way, as a user's close does.
  contents->DispatchBeforeUnload(/*auto_cancel=*/true);
  return BC_AGENT_CLOSE_PENDING;
}

int AgentArchiveTab(const std::string& agent, bc_id tab, bool discard) {
  const Tab* closing = Tab::Get(tab);
  const handed::Mark* mark = handed::Of(tab);
  const AgentArchive archive{
      .handed = closing && mark && Space::AgentMayAct(agent, tab),
      .held = IsHeld(tab),
      .pinned = mark && (mark->pin || mark->tile),
      .asks = closing && closing->AsksBeforeLeaving(),
      .discard = discard,
  };
  switch (archive.What()) {
    case AgentArchive::Does::kRefused:
      return BC_AGENT_CLOSE_REFUSED;
    case AgentArchive::Does::kPinned:
      return BC_AGENT_CLOSE_PINNED;
    case AgentArchive::Does::kAsks:
      return BC_AGENT_CLOSE_ASKS;
    case AgentArchive::Does::kArchives:
      break;
  }
  // No window goes with it, so none the user looks at shows another Space for it. Where it can't
  // go back, as when the Space it left is gone, it stays handed, for the app to give back.
  const bool returned = handed::Return(tab, /*follows=*/false);
  // As the user's close, into the archive from their Space (KeepClosed), but without its
  // handlers, which would ask with the dialog.
  if (returned) {
    Window::CloseTab(tab, /*discard=*/true);
  }
  switch (AgentArchive::After(returned, returned && !Tab::FromId(tab))) {
    case AgentArchive::Came::kStaysHanded:
      return BC_AGENT_CLOSE_REFUSED;
    // Not a refusal: the tab moved, and the agent's log says so.
    case AgentArchive::Came::kReturned:
      return BC_AGENT_CLOSE_RETURNED;
    case AgentArchive::Came::kArchived:
      return BC_AGENT_CLOSED;
  }
}

}  // namespace browser_core
