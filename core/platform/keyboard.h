// What the core reads of the keyboard's layout, per platform.

#ifndef BROWSER_CORE_PLATFORM_KEYBOARD_H_
#define BROWSER_CORE_PLATFORM_KEYBOARD_H_

#include <memory>
#include <string>

#include "build/build_config.h"

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

#if BUILDFLAG(IS_MAC)
// The key AppKit is handing out that views gave a widget as |character| alone, or null. A key that
// AppKit's key bindings make text, as they can one with ⌃, comes to a views dialog with no text
// field as its character, made without the key's modifiers, and its press never comes
// (BridgedContentView's insertText:).
std::unique_ptr<ui::KeyEvent> KeyBehindCharacter(const ui::KeyEvent& character);
#endif

}  // namespace browser_core::platform

#endif  // BROWSER_CORE_PLATFORM_KEYBOARD_H_
