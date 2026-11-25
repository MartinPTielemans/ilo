// What the Keychain item that holds the profiles' encryption key is called on the Mac, apart so
// browser_core_unittests tests the rule alone.

#ifndef BROWSER_CORE_KEYCHAIN_ITEM_H_
#define BROWSER_CORE_KEYCHAIN_ITEM_H_

#include <optional>
#include <string>
#include <string_view>

#include "base/functional/function_ref.h"

namespace browser_core {

struct KeychainItem {
  std::string service;
  std::string account;
};

// The item's names for an app whose Info.plist |plist| answers for, a key's value as the file has
// it or empty for none: "<name> Safe Storage" kept for <name>, where <name> is CrProductDirName,
// the folder its profiles are in. The key belongs with what it opens, so a profile's cookies and
// passwords stay readable when the name the user sees, CFBundleName and CFBundleDisplayName,
// changes. Nothing without that key, which leaves the item Chromium's own.
std::optional<KeychainItem> KeychainItemFor(
    base::FunctionRef<std::string(std::string_view)> plist);

}  // namespace browser_core

#endif  // BROWSER_CORE_KEYCHAIN_ITEM_H_
