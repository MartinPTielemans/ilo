// ilo's settings that follow the account, as Chromium's sync type ILO_SETTING, for the profile the
// browser's account syncs (sync.h), while the user chose to sync them (BC_SYNC_SETTINGS). Which
// follow and which stay with the computer is docs/ux-spec.md's Sync. Each is an entity of its own
// by its name, so two devices changing two settings at once both keep theirs.

#ifndef BROWSER_CORE_SYNC_SETTINGS_H_
#define BROWSER_CORE_SYNC_SETTINGS_H_

#include <memory>

class Profile;

namespace syncer {
class DataTypeController;
}

namespace browser_core::sync_settings {

// ILO_SETTING's controller for |profile|'s sync.
std::unique_ptr<syncer::DataTypeController> Controller(Profile* profile);

// The session has read the settings, so what came from the account before goes in now, and what
// the device has can go up.
void Read();

// Something the session keeps changed (SessionChanged), perhaps a setting.
void Changed();

}  // namespace browser_core::sync_settings

#endif  // BROWSER_CORE_SYNC_SETTINGS_H_
