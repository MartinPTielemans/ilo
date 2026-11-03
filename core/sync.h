// The browser's account (public/browser_core.h, bc_sync_*): it signs in to an ilo server, keeps the
// tokens and the account's secret, holds the relay connection, pairs devices, and has Chromium's
// sync use all of it for the start profile, through the hooks engine/patches/ours/
// chromium-sync-hooks.patch adds, in place of Google's sign-in and FCM.

#ifndef BROWSER_CORE_SYNC_H_
#define BROWSER_CORE_SYNC_H_

#include <string>

namespace browser_core::sync {

// Sets the sync hooks, before any profile is made.
void Install();

// Reads the account the last run kept, once the browser has started, then sync_changed.
void Start();

// One of the browser's windows became active: a pairing request waiting for this device to answer
// gets its key now (sync_pairing_asked).
void WindowActivated();

// The C API's calls, as public/browser_core.h says.
bool SetServer(const std::string& server);
void SignIn();
void Cancel();
void SignOut();
std::string RecoveryWords();
bool EnterRecoveryWords(const std::string& words);
void StartOver();
void RequestPairing();
void AnswerPairing(const std::string& id, bool add);
void ReadDevices();
void RemoveDevice(const std::string& id);

}  // namespace browser_core::sync

#endif  // BROWSER_CORE_SYNC_H_
