// What the core reads of the keyboard's layout, per platform.

#ifndef BROWSER_CORE_PLATFORM_KEYBOARD_H_
#define BROWSER_CORE_PLATFORM_KEYBOARD_H_

#include <memory>
#include <string>

#include "browser_core/shortcut_key.h"
#include "build/build_config.h"

#if BUILDFLAG(IS_MAC)
#include "base/functional/callback.h"
#include "ui/gfx/native_ui_types.h"
#endif

namespace input {
struct NativeWebKeyboardEvent;
}
namespace ui {
class KeyEvent;
}

namespace browser_core::platform {

// |event|'s key as the user's Latin layout types it, with ⇧ if it's held, when the layout typing
// is one that isn't Latin, such as Hebrew, Arabic or Armenian: "t" for the key Hebrew types "א"
// with. The Mac's menus match ⌘ shortcuts by it, and Chromium's characters keep the layout's own,
// even its ASCII ones, as Hebrew's "/" on the Q key. Empty on a Latin layout, for a key that types
// nothing there, and on platforms without such layouts. On Windows it's the key's place on the US
// layout, on every layout, as Chrome's Ctrl shortcuts there match by the virtual key.
std::string LatinKey(const input::NativeWebKeyboardEvent& event);

// The layout typing |event|'s key, for naming the split view's keys (SplitKeyName). Read only while
// they're held (SplitKeysHeld), and made again only when the layout has changed since: on the Mac
// and X11 when they say so, and on Wayland and Windows when the layout's identity isn't the one it
// was made for. Every other key reads nothing.
const SplitLayout& SplitLayoutOf(const input::NativeWebKeyboardEvent& event);

// Stops hearing of the layout's changes, as the core stops.
void StopHearingLayouts();

#if BUILDFLAG(IS_MAC)
// The key AppKit is handing out that views gave a widget as |character| alone, or null. A key that
// AppKit's key bindings make text, as they can one with ⌃, comes to a views dialog with no text
// field as its character, made without the key's modifiers, and its press never comes
// (BridgedContentView's insertText:).
std::unique_ptr<ui::KeyEvent> KeyBehindCharacter(const ui::KeyEvent& character);

// Gives |unheard| each key going down with ⌃ and not ⌘ in |window| that AppKit's key bindings make
// a no-op and views never heard (Heard), once AppKit has dispatched it. The engine's content view
// takes the no-op as the key's handling where AppKit answers for noop:, as macOS 14 does for ⌃⇧1,
// so the key goes nowhere (#4469). One a menu took is heard.
class UnheardKeys {
 public:
  UnheardKeys(gfx::NativeWindow window,
              base::RepeatingCallback<void(const ui::KeyEvent&)> unheard);
  UnheardKeys(const UnheardKeys&) = delete;
  UnheardKeys& operator=(const UnheardKeys&) = delete;
  ~UnheardKeys();

  // views gave the window a key event in AppKit's dispatch of the key going down.
  void Heard();

  struct Watch;

 private:
  std::unique_ptr<Watch> watch_;
};
#endif

}  // namespace browser_core::platform

#endif  // BROWSER_CORE_PLATFORM_KEYBOARD_H_
