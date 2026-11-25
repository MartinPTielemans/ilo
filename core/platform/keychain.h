// The Keychain items the app keeps: the one that holds the profiles' encryption key, and the
// profiles' passkeys kept on this Mac.

#ifndef BROWSER_CORE_PLATFORM_KEYCHAIN_H_
#define BROWSER_CORE_PLATFORM_KEYCHAIN_H_

#include <string>

#include "base/files/file_path.h"
#include "base/functional/callback_forward.h"

class Profile;

namespace browser_core::platform {

// Names the item after the folder the app's profiles are in, as "<name> Safe Storage"
// (keychain_item.h), so the app keeps a key of its own rather than sharing Chromium's with every
// other Chromium on the Mac. Call it before anything reads the key.
void NameKeychainItem();

// The keychain access group the app is signed to keep its passkeys in: the entry of its
// keychain-access-groups entitlement that ends in ".webauthn", or empty for none, as in a dev
// build. Chromium's Touch ID authenticator keeps each profile's passkeys there
// (engine/patches/mac/chromium-core-touch-id-keychain-group.patch).
std::string PasskeyKeychainGroup();

// Deletes the passkeys this Mac keeps for the profile at |path|, |loaded| if it's loaded, then
// runs |done|. Chrome leaves them in the Keychain when it deletes a profile, where no profile can
// list, use or delete them, as only the deleted profile's secret tells them from another's.
void DeletePasskeys(Profile* loaded, const base::FilePath& path, base::OnceClosure done);

}  // namespace browser_core::platform

#endif  // BROWSER_CORE_PLATFORM_KEYCHAIN_H_
