#include "browser_core/battery_saver.h"

#include <memory>
#include <optional>
#include <set>
#include <string>

#include "base/command_line.h"
#include "base/functional/callback.h"
#include "base/logging.h"
#include "base/no_destructor.h"
#include "base/power_monitor/battery_level_provider.h"
#include "base/power_monitor/battery_state_sampler.h"
#include "base/power_monitor/sampling_event_source.h"
#include "base/scoped_observation.h"
#include "base/strings/string_number_conversions.h"
#include "base/time/time.h"
#include "browser_core/host.h"
#include "browser_core/platform/battery.h"
#include "browser_core/tab.h"
#include "chrome/browser/browser_process.h"
#include "chrome/browser/performance_manager/public/user_tuning/battery_saver_mode_manager.h"
#include "components/performance_manager/public/graph/graph.h"
#include "components/performance_manager/public/graph/page_node.h"
#include "components/performance_manager/public/performance_manager.h"
#include "components/performance_manager/public/user_tuning/prefs.h"
#include "components/prefs/pref_service.h"
#include "content/public/browser/web_contents.h"

namespace browser_core {

namespace {

using performance_manager::user_tuning::BatterySaverModeManager;
namespace prefs = performance_manager::user_tuning::prefs;

constexpr char kBatteryForTesting[] = "battery-for-testing";

// The check's battery, and the sampler's call to read it again.
struct CheckBattery {
  int percent = 100;
  bool plugged = true;
  base::SamplingEventSource::SamplingEventCallback sample;
};

CheckBattery& Battery() {
  static base::NoDestructor<CheckBattery> battery;
  return *battery;
}

// Samples only when the check says the battery changed.
class CheckSamplingSource : public base::SamplingEventSource {
 public:
  bool Start(SamplingEventCallback callback) override {
    Battery().sample = std::move(callback);
    return true;
  }
  base::TimeDelta GetSampleInterval() override { return base::Minutes(1); }
};

class CheckBatteryProvider : public base::BatteryLevelProvider {
 public:
  void GetBatteryState(
      base::OnceCallback<void(const std::optional<BatteryState>&)> callback) override {
    BatteryState state;
    state.battery_count = 1;
    state.is_external_power_connected = Battery().plugged;
    state.current_capacity = static_cast<uint64_t>(Battery().percent);
    state.full_charged_capacity = 100;
    state.charge_unit = BatteryLevelUnit::kRelative;
    state.capture_time = base::TimeTicks::Now();
    std::move(callback).Run(state);
  }
};

BatterySaverModeManager* Manager() {
  return BatterySaverModeManager::HasInstance() ? BatterySaverModeManager::GetInstance() : nullptr;
}

void Tell(bool active) {
  const int percent = Manager() ? Manager()->SampledBatteryPercentage() : -1;
  // check.sh reads these lines.
  if (active) {
    LOG(ERROR) << "browser: battery saver on at " << percent << "%";
  } else {
    LOG(ERROR) << "browser: battery saver off";
  }
  if (Host().battery_saver_changed) {
    Host().battery_saver_changed(Host().context, active, percent);
  }
}

class BatterySaverWatcher : public BatterySaverModeManager::Observer {
 public:
  void Watch(BatterySaverModeManager* manager) {
    observation_.Observe(manager);
    // On already, as at a launch below 20%, before this watched.
    if (manager->IsBatterySaverActive()) {
      Tell(true);
    }
  }

 private:
  void OnBatterySaverActiveChanged(bool is_active) override { Tell(is_active); }
  void OnBatterySaverModeManagerDestroyed() override { observation_.Reset(); }

  base::ScopedObservation<BatterySaverModeManager, BatterySaverModeManager::Observer>
      observation_{this};
};

// Tells the app as Chrome freezes a tab's page, as Battery Saver does a busy one out of sight, and
// as it thaws, as it does once shown. The graph owns it, so it goes as the graph does.
class FrozenWatcher : public performance_manager::GraphOwned,
                      public performance_manager::PageNodeObserver {
 public:
  void OnPassedToGraph(performance_manager::Graph* graph) override {
    graph->AddPageNodeObserver(this);
  }
  void OnTakenFromGraph(performance_manager::Graph* graph) override {
    graph->RemovePageNodeObserver(this);
  }

 private:
  void OnPageLifecycleStateChanged(const performance_manager::PageNode* page) override {
    content::WebContents* contents = page->GetWebContents().get();
    Tab* tab = contents ? Tab::FromWebContents(contents) : nullptr;
    if (!tab) {
      return;
    }
    const bool frozen =
        page->GetLifecycleState() == performance_manager::mojom::LifecycleState::kFrozen;
    if (frozen == frozen_.contains(tab->id())) {
      return;
    }
    if (frozen) {
      frozen_.insert(tab->id());
    } else {
      frozen_.erase(tab->id());
    }
    // check.sh reads this line.
    LOG(ERROR) << "browser: tab " << tab->id() << (frozen ? " frozen" : " thawed");
    if (Host().tab_frozen) {
      Host().tab_frozen(Host().context, tab->id(), frozen);
    }
  }

  std::set<bc_id> frozen_;
};

}  // namespace

void InstallBatterySampler() {
  platform::Battery battery;
  if (base::CommandLine::ForCurrentProcess()->HasSwitch(kBatteryForTesting)) {
    battery = {std::make_unique<CheckSamplingSource>(), std::make_unique<CheckBatteryProvider>()};
  } else {
    battery = platform::OsBattery();
  }
  if (!battery.source) {
    return;
  }
  // Chrome makes its own only if there's none "for testing" (ChromeBrowserMainExtraParts-
  // PerformanceManager::PostCreateThreads), so this one takes its place with no engine patch.
  static base::NoDestructor<std::unique_ptr<base::BatteryStateSampler>> sampler(
      base::BatteryStateSampler::CreateInstanceForTesting(std::move(battery.source),
                                                          std::move(battery.provider)));
}

void WatchBatterySaver() {
  static base::NoDestructor<BatterySaverWatcher> watcher;
  if (BatterySaverModeManager* manager = Manager()) {
    watcher->Watch(manager);
  }
  if (performance_manager::Graph* graph = performance_manager::PerformanceManager::GetGraph()) {
    graph->PassToGraph(std::make_unique<FrozenWatcher>());
  }
}

bool BatterySaverActive() {
  return Manager() && Manager()->IsBatterySaverActive();
}

void TurnOffBatterySaverForNow() {
  if (Manager()) {
    Manager()->SetTemporaryBatterySaverDisabledForSession(true);
  }
}

bool BatterySaverEnabled() {
  return prefs::GetCurrentBatterySaverModeState(g_browser_process->local_state()) !=
         prefs::BatterySaverModeState::kDisabled;
}

void SetBatterySaverEnabled(bool enabled) {
  const prefs::BatterySaverModeState state = enabled
                                                 ? prefs::BatterySaverModeState::kEnabledBelowThreshold
                                                 : prefs::BatterySaverModeState::kDisabled;
  g_browser_process->local_state()->SetInteger(prefs::kBatterySaverModeState,
                                               static_cast<int>(state));
}

void SetBatteryForTesting(int percent, bool plugged) {
  CheckBattery& battery = Battery();
  battery.percent = percent;
  battery.plugged = plugged;
  if (battery.sample) {
    battery.sample.Run();
  }
}

}  // namespace browser_core
