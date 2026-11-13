#include "browser_core/idle_unload.h"

#include <optional>
#include <utility>
#include <vector>

#include "base/functional/callback.h"
#include "base/logging.h"
#include "base/memory/memory_pressure_level.h"
#include "base/memory/memory_pressure_listener.h"
#include "base/no_destructor.h"
#include "base/task/task_traits.h"
#include "browser_core/idle_check.h"
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

// The core as a check sees it.
class CoreHost : public IdleCheck::Host {
 public:
  // Best effort, so each task waits for the work queued before it, such as a switch's frames.
  void Post(base::OnceClosure task) override {
    content::GetUIThreadTaskRunner({base::TaskPriority::BEST_EFFORT})
        ->PostTask(FROM_HERE, std::move(task));
  }
  bool On() override {
    return UserSettings().unload_idle_tabs && !browser_shutdown::IsTryingToQuit();
  }
  std::vector<uint64_t> Tabs() override {
    std::vector<uint64_t> tabs;
    for (const Tab* tab : Tab::All()) {
      tabs.push_back(tab->id());
    }
    return tabs;
  }
  std::optional<base::Time> LastSeen(uint64_t id) override {
    const Tab* tab = Tab::Get(id);
    return tab ? std::make_optional(tab->last_seen()) : std::nullopt;
  }
  void Unload(uint64_t id) override {
    Tab* tab = Tab::Get(id);
    if (tab && MayUnload(*tab, DiscardReason::PROACTIVE)) {
      tab->Unload();
    }
  }
  void Checked() override {
    if (g_aged) {
      LOG(ERROR) << "browser: idle tabs checked";
    }
  }
};

IdleCheck& Check() {
  static base::NoDestructor<CoreHost> host;
  static base::NoDestructor<IdleCheck> check(*host, kUnloadAfter);
  return *check;
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
  Check().Ask();
}

void AgeTabsForTesting(base::TimeDelta by) {
  g_aged = true;
  for (Tab* tab : Tab::All()) {
    tab->set_last_seen(tab->last_seen() - by);
  }
}

}  // namespace browser_core
