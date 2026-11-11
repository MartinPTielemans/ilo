#include "browser_core/idle_unload.h"

#include <vector>

#include "base/functional/bind.h"
#include "base/logging.h"
#include "base/memory/memory_pressure_level.h"
#include "base/memory/memory_pressure_listener.h"
#include "base/no_destructor.h"
#include "base/task/task_traits.h"
#include "browser_core/session.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "chrome/browser/devtools/devtools_window.h"
#include "chrome/browser/lifetime/browser_shutdown.h"
#include "chrome/browser/performance_manager/policies/cannot_discard_reason.h"
#include "chrome/browser/performance_manager/policies/discard_eligibility_policy.h"
#include "chrome/browser/profiles/profile.h"
#include "components/performance_manager/public/graph/page_node.h"
#include "components/performance_manager/public/performance_manager.h"
#include "content/public/browser/browser_task_traits.h"
#include "content/public/browser/browser_thread.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "third_party/blink/public/mojom/frame/sudden_termination_disabler_type.mojom-shared.h"

namespace browser_core {

namespace {

using performance_manager::PerformanceManager;
using performance_manager::policies::CannotDiscardReason;
using performance_manager::policies::DiscardEligibilityPolicy;
using DiscardReason = DiscardEligibilityPolicy::DiscardReason;

// The owner's call: long enough that the tabs the user works between keep their pages.
constexpr base::TimeDelta kUnloadAfter = base::Hours(2);

bool g_check_posted = false;
// Whether check.sh aged the tabs, and waits for each check's line.
bool g_aged = false;

// Whether |tab| may let go of its page now without the user losing anything: what they typed, a
// sound, a call, a page that asks before it's left, or one an agent may be working in out of sight.
bool MayUnload(const Tab& tab, DiscardReason reason) {
  content::WebContents* contents = tab.web_contents();
  const Space* space = Space::Holding(tab.id());
  // Not one a window shows, or has a picture of while another shows it, nor a private window's,
  // whose session ends with it anyway.
  if (!contents || Window::Showing(tab.id()) || Window::Picturing(tab.id()) || !tab.profile() ||
      tab.profile()->IsOffTheRecord() || (space && space->agent())) {
    return false;
  }
  // Chrome's checks can't see into the system's engine's page, which may be playing or hold what
  // the user typed.
  if (tab.system_page()) {
    return false;
  }
  // Nothing loaded, as a tab opened behind and not yet shown, or one Chrome unloaded in place: no
  // page to lose, and none for Chrome's checks.
  if (!Tab::Loaded(tab.id())) {
    return true;
  }
  if (DevToolsWindow::GetInstanceForInspectedWebContents(contents)) {
    return false;
  }
  // Its page would never get to ask.
  bool asks = false;
  contents->ForEachRenderFrameHost([&asks](content::RenderFrameHost* frame) {
    asks = asks || frame->GetSuddenTerminationDisablerState(
                       blink::mojom::SuddenTerminationDisablerType::kBeforeUnloadHandler);
  });
  if (asks) {
    return false;
  }
  // Chrome's checks for its Memory Saver: sound now or lately, capturing, picture-in-picture, typed
  // form text, a device connected, an extension keeping it, and for |reason| PROACTIVE a page that
  // shows notifications or works in the background.
  DiscardEligibilityPolicy* policy =
      DiscardEligibilityPolicy::GetFromGraph(PerformanceManager::GetGraph());
  base::WeakPtr<performance_manager::PageNode> page =
      PerformanceManager::GetPrimaryPageNodeForWebContents(contents);
  if (!policy || !page) {
    return false;
  }
  // A call is never let go, even one that takes no camera, microphone or screen and makes no sound,
  // such as one listening alone, which Chrome's checks would let go (#686).
  if (page->UsesWebRTC()) {
    return false;
  }
  std::vector<CannotDiscardReason> reasons;
  policy->CanDiscardWithCustomRecentVisibilityWindow(
      page.get(), reason, /*minimum_time_in_background=*/base::TimeDelta(), &reasons);
  // Each profile's Browser keeps an active tab no window need show. Whether one shows it is the
  // core's to say, above.
  std::erase(reasons, CannotDiscardReason::kActiveTab);
  return reasons.empty();
}

// The tabs idle when a check began that it's still to look at.
std::vector<bc_id>& Unchecked() {
  static base::NoDestructor<std::vector<bc_id>> unchecked;
  return *unchecked;
}

// One tab a task, so the frames and keys of the switch that asked run between them: a page's
// teardown is 5-6 ms of the main thread, and a user back after hours has every tab idle at once,
// which in one task held a switch 164 ms (#2387). Best effort, so each waits for the work queued
// before it, such as a switch's frames.
void UnloadNextIdle() {
  std::vector<bc_id>& unchecked = Unchecked();
  if (!unchecked.empty() && UserSettings().unload_idle_tabs &&
      !browser_shutdown::IsTryingToQuit()) {
    Tab* tab = Tab::Get(unchecked.back());
    unchecked.pop_back();
    // Unless it has been seen since.
    if (tab && base::Time::Now() - tab->last_seen() >= kUnloadAfter &&
        MayUnload(*tab, DiscardReason::PROACTIVE)) {
      tab->Unload();
    }
    content::GetUIThreadTaskRunner({base::TaskPriority::BEST_EFFORT})
        ->PostTask(FROM_HERE, base::BindOnce(&UnloadNextIdle));
    return;
  }
  unchecked.clear();
  g_check_posted = false;
  if (g_aged) {
    LOG(ERROR) << "browser: idle tabs checked";
  }
}

// Unloads the tab unseen longest, whatever its age, each time the system says memory is short:
// one a notification, as Chrome's urgent discarding does, and macOS notifies again while it lasts.
class PressureListener : public base::MemoryPressureListener {
 public:
  PressureListener() = default;

 private:
  void OnMemoryPressure(base::MemoryPressureLevel level) override {
    if (level == base::MEMORY_PRESSURE_LEVEL_NONE || !UserSettings().unload_idle_tabs ||
        browser_shutdown::IsTryingToQuit()) {
      return;
    }
    Tab* oldest = nullptr;
    for (Tab* tab : Tab::All()) {
      if ((!oldest || tab->last_seen() < oldest->last_seen()) &&
          MayUnload(*tab, DiscardReason::URGENT)) {
        oldest = tab;
      }
    }
    if (oldest) {
      oldest->Unload();
    }
  }

  // Base has no tag for the core, and adding one rebuilds most of Chromium. The tag only picks
  // which field trial may silence a listener, and our build runs none, so it's the nearest one's.
  base::MemoryPressureListenerRegistration registration_{
      base::MemoryPressureListenerTag::kBackgroundTabLoadingPolicy, this};
};

}  // namespace

void UnloadIdleTabsSoon() {
  // Made with the first tab shown, which is soon enough to hear of memory running short.
  static base::NoDestructor<PressureListener> listener;
  if (g_check_posted) {
    return;
  }
  g_check_posted = true;
  const base::Time now = base::Time::Now();
  for (const Tab* tab : Tab::All()) {
    if (now - tab->last_seen() >= kUnloadAfter) {
      Unchecked().push_back(tab->id());
    }
  }
  content::GetUIThreadTaskRunner({base::TaskPriority::BEST_EFFORT})
      ->PostTask(FROM_HERE, base::BindOnce(&UnloadNextIdle));
}

void AgeTabsForTesting(base::TimeDelta by) {
  g_aged = true;
  for (Tab* tab : Tab::All()) {
    tab->set_last_seen(tab->last_seen() - by);
  }
}

}  // namespace browser_core
