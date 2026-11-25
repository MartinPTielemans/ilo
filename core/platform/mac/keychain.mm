#include "browser_core/platform/keychain.h"

#import <Foundation/Foundation.h>
#import <Security/Security.h>

#include <optional>
#include <string>
#include <string_view>
#include <utility>

#include "base/apple/bridging.h"
#include "base/apple/foundation_util.h"
#include "base/apple/scoped_cftyperef.h"
#include "base/base64.h"
#include "base/files/file_util.h"
#include "base/functional/bind.h"
#include "base/functional/callback.h"
#include "base/json/json_reader.h"
#include "base/no_destructor.h"
#include "base/strings/sys_string_conversions.h"
#include "base/task/thread_pool.h"
#include "base/time/time.h"
#include "base/values.h"
#include "browser_core/keychain_item.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/webauthn/webauthn_pref_names.h"
#include "chrome/common/chrome_constants.h"
#include "components/os_crypt/common/keychain_password_mac.h"
#include "components/prefs/pref_service.h"
#include "device/fido/mac/authenticator_config.h"
#include "device/fido/mac/credential_store.h"

namespace browser_core::platform {

namespace {

// On the thread pool: deletes the passkeys in |group| that |secret| says are the profile's. With
// no secret, the profile wasn't loaded, and its own preferences have it if it ever made one.
void DeletePasskeysBlocking(std::string group, std::string secret, base::FilePath path) {
  if (secret.empty()) {
    std::string json;
    if (!base::ReadFileToString(path.Append(chrome::kPreferencesFilename), &json)) {
      return;
    }
    std::optional<base::DictValue> prefs = base::JSONReader::ReadDict(json, base::JSON_PARSE_RFC);
    const std::string* saved =
        prefs ? prefs->FindStringByDottedPath(
                    webauthn::pref_names::kWebAuthnTouchIdMetadataSecretPrefName)
              : nullptr;
    if (!saved) {
      return;
    }
    secret = *saved;
  }
  // Saved in base64, as Chrome's delegate keeps it.
  std::string decoded;
  if (!base::Base64Decode(secret, &decoded) || decoded.empty()) {
    return;
  }
  device::fido::mac::TouchIdCredentialStore store(
      {.keychain_access_group = std::move(group), .metadata_secret = std::move(decoded)});
  store.DeleteCredentialsSync(base::Time(), base::Time::Max());
}

}  // namespace

void NameKeychainItem() {
  // The file's own values: -objectForInfoDictionaryKey: answers with a strings file's where
  // InfoPlist.strings names the key, as it may the name the user sees.
  NSDictionary* plist = NSBundle.mainBundle.infoDictionary;
  std::optional<KeychainItem> item = KeychainItemFor([&](std::string_view key) {
    return base::SysNSStringToUTF8(
        base::apple::ObjCCast<NSString>(plist[base::SysUTF8ToNSString(key)]));
  });
  if (!item) {
    return;
  }
  KeychainPassword::GetServiceName() = item->service;
  KeychainPassword::GetAccountName() = item->account;
}

std::string PasskeyKeychainGroup() {
  // The signature doesn't change while the app runs.
  static const base::NoDestructor<std::string> group([] {
    base::apple::ScopedCFTypeRef<SecTaskRef> task(SecTaskCreateFromSelf(kCFAllocatorDefault));
    if (!task) {
      return std::string();
    }
    base::apple::ScopedCFTypeRef<CFTypeRef> value(
        SecTaskCopyValueForEntitlement(task.get(), CFSTR("keychain-access-groups"), nullptr));
    NSArray* groups = base::apple::CFToNSPtrCast(base::apple::CFCast<CFArrayRef>(value.get()));
    for (id entry in groups) {
      if ([entry isKindOfClass:[NSString class]] && [entry hasSuffix:@".webauthn"]) {
        return base::SysNSStringToUTF8(entry);
      }
    }
    return std::string();
  }());
  return *group;
}

void DeletePasskeys(Profile* loaded, const base::FilePath& path, base::OnceClosure done) {
  // Without the group, Chromium's authenticator was never available, so there are none.
  std::string group = PasskeyKeychainGroup();
  if (group.empty()) {
    std::move(done).Run();
    return;
  }
  std::string secret =
      loaded ? loaded->GetPrefs()->GetString(
                   webauthn::pref_names::kWebAuthnTouchIdMetadataSecretPrefName)
             : std::string();
  base::ThreadPool::PostTaskAndReply(
      FROM_HERE, {base::MayBlock(), base::TaskPriority::USER_VISIBLE},
      base::BindOnce(&DeletePasskeysBlocking, std::move(group), std::move(secret), path),
      std::move(done));
}

}  // namespace browser_core::platform
