// The key a page's key is named by for the app's shortcuts (tab.cc), apart so
// browser_core_unittests tests it alone.

#ifndef BROWSER_CORE_SHORTCUT_KEY_H_
#define BROWSER_CORE_SHORTCUT_KEY_H_

#include <optional>
#include <string>

#include "base/containers/flat_map.h"
#include "base/functional/function_ref.h"

namespace browser_core {

// The keys held with a key.
struct HeldKeys {
  bool shift = false;
  bool control = false;
  bool alt = false;
  bool meta = false;
};

// The platforms whose keys the split keys are named on as their GPUI names them (SplitKeysHeld).
enum class KeysPlatform { kMac, kWindows, kLinux };

// The split view's keys, ⌃⇧[ Previous Pane, ⌃⇧] Next Pane and ⌃⇧- Close Split Pane, are held
// (#3509): ⌃ and ⇧ without ⌘ on the Mac, which type no text; ⇧ with exactly one of Ctrl or Alt on
// Windows, where both are AltGr; ⇧ with Ctrl or Alt on Linux, whose AltGr is neither. As GPUI's
// patches hold them in the UI, so a split key names a page's key as it does a UI's.
bool SplitKeysHeld(const HeldKeys& held, KeysPlatform platform);

// The base characters a layout's keys type, without ⇧ or AltGr, by the web's code for each key's
// place, such as "BracketLeft", as ui::GenerateDomKeyboardLayoutMap gives them.
using LayoutKeys = base::flat_map<std::string, std::string>;

// A layout's keys, with the split keys, [ ] and -, it names by their US place: those the layout
// has no key of its own for, unless its key at that place types another of them. Made when the
// layout changes, never for a key's press.
struct SplitLayout {
  SplitLayout() = default;
  explicit SplitLayout(LayoutKeys layout_keys);

  LayoutKeys keys;
  std::string by_place;
};

// The split keys' layout for the one whose identity is |id|, made by |read| only when |id| isn't
// the identity it was last made for. On Wayland and Windows, whose layout changes the core hears
// of no other way, compared while the split keys are held.
class SplitLayoutCache {
 public:
  const SplitLayout& For(const std::string& id, base::FunctionRef<LayoutKeys()> read);

 private:
  std::optional<std::string> id_;
  SplitLayout layout_;
};

// The split key the key at |code| is named by while they're held (SplitKeysHeld), or empty: its
// own base character on |layout| if that's [, ] or -, else the split key whose US place it's at,
// if |layout| names that one by its place (core/split_keys_layouts.txt). Empty on a layout that
// couldn't be read.
std::string SplitKeyName(const std::string& code, const SplitLayout& layout);

// The name the app's shortcuts know a key by: |split_key|, if the split keys named it
// (SplitKeyName), ahead of a digit's place, as French's - key is its 6 key. Else |typed|, the
// web's name for what the layout types with it, unless a shortcut names the key by its place, its
// |windows_key_code| (Chrome's VKEY), as Chrome's do. A letter's or digit's key is named by its
// place with ⌥ held (Alt off the Mac), whose character, such as ⌥I's "ˆ", no shortcut names. On
// the Mac (|mac|), a digit's key is too with ⌃ and ⇧ held and no ⌘, which type no text, as GPUI
// names it from the UI: ⌃⇧2 is 2 where ⇧2 types " (Danish, German).
std::string ShortcutKeyName(const std::string& typed,
                            int windows_key_code,
                            const std::string& split_key,
                            const HeldKeys& held,
                            bool mac);

}  // namespace browser_core

#endif  // BROWSER_CORE_SHORTCUT_KEY_H_
