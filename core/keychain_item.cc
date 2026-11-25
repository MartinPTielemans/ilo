#include "browser_core/keychain_item.h"

namespace browser_core {

std::optional<KeychainItem> KeychainItemFor(
    base::FunctionRef<std::string(std::string_view)> plist) {
  std::string name = plist("CrProductDirName");
  if (name.empty()) {
    return std::nullopt;
  }
  return KeychainItem{.service = name + " Safe Storage", .account = name};
}

}  // namespace browser_core
