#include "browser_core/shortcut_key.h"

namespace browser_core {

std::string ShortcutKeyName(const std::string& typed,
                            int windows_key_code,
                            const HeldKeys& held,
                            bool mac) {
  // Chrome's VKEYs for letters and digits are their ASCII capitals and digits.
  const bool letter = windows_key_code >= 'A' && windows_key_code <= 'Z';
  const bool digit = windows_key_code >= '0' && windows_key_code <= '9';
  const bool by_place =
      (held.alt && (letter || digit)) || (mac && held.control && held.shift && !held.meta && digit);
  return by_place ? std::string(1, static_cast<char>(windows_key_code)) : typed;
}

}  // namespace browser_core
