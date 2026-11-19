// The key a page's key is named by for the app's shortcuts (tab.cc), apart so
// browser_core_unittests tests it alone.

#ifndef BROWSER_CORE_SHORTCUT_KEY_H_
#define BROWSER_CORE_SHORTCUT_KEY_H_

#include <string>

namespace browser_core {

// The keys held with a key.
struct HeldKeys {
  bool shift = false;
  bool control = false;
  bool alt = false;
  bool meta = false;
};

// The name the app's shortcuts know a key by: |typed|, the web's name for what the layout types
// with it, unless a shortcut names the key by its place, its |windows_key_code| (Chrome's VKEY),
// as Chrome's do. A letter's or digit's key is named by its place with ⌥ held (Alt off the Mac),
// whose character, such as ⌥I's "ˆ", no shortcut names. On the Mac (|mac|), a digit's key is too
// with ⌃ and ⇧ held and no ⌘, which type no text, as GPUI names it from the UI: ⌃⇧2 is 2 where
// ⇧2 types " (Danish, German).
std::string ShortcutKeyName(const std::string& typed,
                            int windows_key_code,
                            const HeldKeys& held,
                            bool mac);

}  // namespace browser_core

#endif  // BROWSER_CORE_SHORTCUT_KEY_H_
