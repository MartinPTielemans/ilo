#include "browser_core/platform/keyboard.h"

#import <AppKit/AppKit.h>
#include <Carbon/Carbon.h>

#include "base/apple/foundation_util.h"
#include "base/apple/owned_objc.h"
#include "base/apple/scoped_cftyperef.h"
#include "components/input/native_web_keyboard_event.h"
#include "ui/events/event.h"
#include "ui/events/keycodes/keyboard_code_conversion_mac.h"

namespace browser_core::platform {

std::string LatinKey(const input::NativeWebKeyboardEvent& event) {
  NSEvent* key = event.os_event.Get();
  if (!key || key.type != NSEventTypeKeyDown) {
    return std::string();
  }
  base::apple::ScopedCFTypeRef<TISInputSourceRef> layout(TISCopyCurrentKeyboardLayoutInputSource());
  CFBooleanRef latin_layout =
      layout ? base::apple::CFCast<CFBooleanRef>(TISGetInputSourceProperty(
                   layout.get(), kTISPropertyInputSourceIsASCIICapable))
             : nullptr;
  if (!latin_layout || CFBooleanGetValue(latin_layout)) {
    return std::string();
  }
  // The Latin layout the user last typed with, which is what AppKit's menus match by.
  base::apple::ScopedCFTypeRef<TISInputSourceRef> latin(
      TISCopyCurrentASCIICapableKeyboardLayoutInputSource());
  if (!latin) {
    return std::string();
  }
  // ⇧ alone, as Chromium names a page's key by its character with ⇧: ⌘ and ⌥ would give the
  // layout's other characters.
  const UInt32 shift = (key.modifierFlags & NSEventModifierFlagShift) ? (shiftKey >> 8) & 0xFF : 0;
  UInt32 dead = 0;
  const UniChar typed = ui::TranslatedUnicodeCharFromKeyCode(
      latin.get(), key.keyCode, kUCKeyActionDown, shift, LMGetKbdType(), &dead);
  // Printable ASCII only: a Latin layout's own letters, such as "é", and its function keys, which
  // Chromium names by the key, come from the page's name.
  if (typed <= 0x20 || typed >= 0x7F) {
    return std::string();
  }
  return std::string(1, static_cast<char>(typed));
}

std::unique_ptr<ui::KeyEvent> KeyBehindCharacter(const ui::KeyEvent& character) {
  // views makes the character in AppKit's dispatch of the key, with no NSEvent of its own.
  NSEvent* key = NSApp.currentEvent;
  if (!character.is_char() || character.HasNativeEvent() || !key ||
      key.type != NSEventTypeKeyDown || key.characters.length != 1 ||
      [key.characters characterAtIndex:0] != character.GetCharacter()) {
    return nullptr;
  }
  return std::make_unique<ui::KeyEvent>(base::apple::OwnedNSEvent(key));
}

}  // namespace browser_core::platform
