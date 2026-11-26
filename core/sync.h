// The browser's account (public/browser_core.h, bc_sync_*): it signs in to an ilo server, keeps the
// tokens and the account's secret, holds the relay connection, pairs devices, and has Chromium's
// sync use all of it for the start profile, through the hooks engine/patches/ours/
// chromium-sync-hooks.patch adds, in place of Google's sign-in and FCM.

#ifndef BROWSER_CORE_SYNC_H_
#define BROWSER_CORE_SYNC_H_

#include <array>
#include <cstdint>
#include <optional>
#include <string>
#include <string_view>

#include "base/functional/callback_forward.h"
#include "url/gurl.h"

struct bc_sync_activity;

namespace content {
class WebContents;
}

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
// bc_sync_sign_in, or bc_sync_sign_in_google with |google|.
void SignIn(bool google);
void Cancel();
void SignOut();
std::string RecoveryWords();
bool EnterRecoveryWords(const std::string& words);
void StartOver();
void RequestPairing();
void AnswerPairing(const std::string& id, bool add);
void ReadDevices();
void RemoveDevice(const std::string& id);
void ReadGoogle();
void RemoveGoogle();
void ReadActivity(bc_sync_activity* activity);
void Choose(int what, bool on);
void DeleteAccount();

// Whether |contents| is the sign-in window's page, which gets none of the user's saved passwords
// and offers to save none: the account's password, or Google's, stays the page's alone.
bool InSignInWindow(content::WebContents* contents);

// For share links (share.cc). Nothing of these is logged, as nothing of a share is.
// The account's server, or an empty URL while signed out.
GURL ShareServer();
// A key for |info| derived from the account's secret, or none while signed out or without it.
std::optional<std::array<uint8_t, 32>> ShareKey(std::string_view info);
// Asks the account's server, as the account, |method| |path| with the JSON |json| if it isn't
// empty. |done| gets the answer's status, 0 if there was none or the user is signed out, and body.
using ShareAnswered = base::OnceCallback<void(int status, std::string body)>;
void AskForShares(std::string method, std::string path, std::string json, ShareAnswered done);

}  // namespace browser_core::sync

#endif  // BROWSER_CORE_SYNC_H_
