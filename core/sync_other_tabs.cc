#include "browser_core/sync_other_tabs.h"

#include <algorithm>
#include <set>
#include <string>
#include <utility>
#include <vector>

#include "base/callback_list.h"
#include "base/functional/bind.h"
#include "base/memory/raw_ptr.h"
#include "base/no_destructor.h"
#include "base/scoped_observation.h"
#include "browser_core/host.h"
#include "browser_core/other_tabs.h"
#include "browser_core/profiles.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_observer.h"
#include "chrome/browser/sync/device_info_sync_service_factory.h"
#include "chrome/browser/sync/session_sync_service_factory.h"
#include "components/sessions/core/session_types.h"
#include "components/sync_device_info/device_info.h"
#include "components/sync_device_info/device_info_sync_service.h"
#include "components/sync_device_info/device_info_tracker.h"
#include "components/sync_sessions/open_tabs_ui_delegate.h"
#include "components/sync_sessions/session_sync_service.h"
#include "components/sync_sessions/synced_session.h"

namespace browser_core {
namespace {

// As bc_sync_device's: only ilo's systems sync with an ilo account.
std::string OsName(syncer::DeviceInfo::OsType os) {
  switch (os) {
    case syncer::DeviceInfo::OsType::kMac:
      return "macOS";
    case syncer::DeviceInfo::OsType::kWindows:
      return "Windows";
    case syncer::DeviceInfo::OsType::kLinux:
      return "Linux";
    default:
      return std::string();
  }
}

// Reads the profile's other computers each time sync brings a change to their tabs or to the
// devices that name them, and tells the app when the list differs from the one it has.
class Watch : public syncer::DeviceInfoTracker::Observer, public ProfileObserver {
 public:
  static Watch& Get() {
    static base::NoDestructor<Watch> watch;
    return *watch;
  }

  void Start(Profile* profile) {
    if (profile == profile_) {
      return;
    }
    Stop();
    sync_sessions::SessionSyncService* sessions = SessionSyncServiceFactory::GetForProfile(profile);
    if (!sessions) {
      return;
    }
    profile_ = profile;
    profile_observation_.Observe(profile);
    sessions_ = sessions->SubscribeToForeignSessionsChanged(
        base::BindRepeating(&Watch::Report, base::Unretained(this)));
    if (syncer::DeviceInfoSyncService* devices =
            DeviceInfoSyncServiceFactory::GetForProfile(profile)) {
      devices_observation_.Observe(devices->GetDeviceInfoTracker());
    }
    Report();
  }

  // Tells the app of the tabs as they are now, if they changed: another computer's, or the Spaces'
  // here, which leave out those in them.
  void Report() {
    if (!profile_ || !Host().other_tabs_changed) {
      return;
    }
    // While sync is off, as once the profile signs out, Chromium has deleted the other computers'
    // sessions and has no delegate: none.
    std::vector<OtherComputer> computers;
    sync_sessions::SessionSyncService* service = SessionSyncServiceFactory::GetForProfile(profile_);
    sync_sessions::OpenTabsUIDelegate* open_tabs =
        service ? service->GetOpenTabsUIDelegate() : nullptr;
    std::vector<raw_ptr<const sync_sessions::SyncedSession, VectorExperimental>> sessions;
    if (open_tabs && open_tabs->GetAllForeignSessions(&sessions)) {
      // Every device's record: GetDeviceInfo takes only one with a Chrome version, which
      // TrimSyncCommit keeps on its device.
      std::vector<const syncer::DeviceInfo*> devices;
      if (const syncer::DeviceInfoTracker* tracker = devices_observation_.GetSource()) {
        devices = tracker->GetAllDeviceInfo();
      }
      for (const sync_sessions::SyncedSession* session : sessions) {
        OtherComputer computer{.name = session->GetSessionName(),
                               .seen = session->GetModifiedTime()};
        // A session's tag is its device's cache GUID, which names the device's own record.
        auto device =
            std::ranges::find(devices, session->GetSessionTag(), &syncer::DeviceInfo::guid);
        if (device != devices.end()) {
          computer.name = (*device)->client_name();
          computer.os = OsName((*device)->os_type());
        }
        open_tabs->GetForeignSessionTabs(session->GetSessionTag(), &computer.tabs);
        computers.push_back(std::move(computer));
      }
    }
    std::vector<OtherTab> tabs = OtherTabsOf(computers);
    // The Spaces' Today and pinned tabs sync as the Spaces' own (#2098), so another computer's are
    // in a Space here already, where the command bar lists them as this computer's. What it lists
    // as another computer's is what's outside the Spaces there: a Little ilo window's or a Blank
    // Window's tabs, and the favorites.
    std::set<std::string> in_spaces;
    for (Space* space : Space::All()) {
      for (const std::vector<bc_id>* rows : {&space->pinned_rows(), &space->today()}) {
        for (bc_id row : *rows) {
          if (const Tab* tab = Tab::Get(row)) {
            in_spaces.insert(tab->Url().spec());
          }
        }
      }
    }
    std::erase_if(tabs, [&](const OtherTab& tab) { return in_spaces.contains(tab.url); });
    if (reported_ && tabs == tabs_) {
      return;
    }
    reported_ = true;
    tabs_ = std::move(tabs);
    std::vector<bc_other_tab> listed;
    for (const OtherTab& tab : tabs_) {
      listed.push_back({.url = tab.url.c_str(),
                        .title = tab.title.c_str(),
                        .computer = tab.computer.c_str(),
                        .os = tab.os.c_str(),
                        .seen = tab.seen.InMillisecondsFSinceUnixEpochIgnoringNull()});
    }
    Host().other_tabs_changed(Host().context, ProfileId(DirOf(profile_)), listed.data(),
                              listed.size());
  }

 private:
  friend class base::NoDestructor<Watch>;
  Watch() = default;

  void Stop() {
    sessions_ = {};
    devices_observation_.Reset();
    profile_observation_.Reset();
    profile_ = nullptr;
  }

  // syncer::DeviceInfoTracker::Observer: a computer's name or system came after its tabs.
  void OnDeviceInfoChange() override { Report(); }

  // ProfileObserver: its sync services go with it.
  void OnProfileWillBeDestroyed(Profile* profile) override { Stop(); }

  raw_ptr<Profile> profile_ = nullptr;
  base::CallbackListSubscription sessions_;
  base::ScopedObservation<syncer::DeviceInfoTracker, syncer::DeviceInfoTracker::Observer>
      devices_observation_{this};
  base::ScopedObservation<Profile, ProfileObserver> profile_observation_{this};
  // What the app was last told, so a change that leaves the list as it was says nothing.
  bool reported_ = false;
  std::vector<OtherTab> tabs_;
};

}  // namespace

void WatchOtherTabs(Profile* profile) {
  Watch::Get().Start(profile);
}

void RefreshOtherTabs() {
  Watch::Get().Report();
}

}  // namespace browser_core
