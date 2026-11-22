#include "browser_core/platform/keyboard.h"

#include <windows.h>

#include "base/no_destructor.h"
#include "base/strings/string_number_conversions.h"
#include "components/input/native_web_keyboard_event.h"
#include "third_party/blink/public/common/input/web_input_event.h"
#include "ui/events/event.h"
#include "ui/events/keycodes/dom/dom_keyboard_layout_map.h"

namespace browser_core::platform {

std::string LatinKey(const input::NativeWebKeyboardEvent& event) {
  // Windows' layouts give a key that isn't Latin the virtual key of its place on the US layout,
  // such as VK_T for Russian's "е", and Chrome's shortcuts match by it.
  const int code = event.windows_key_code;
  const bool shift = event.GetModifiers() & blink::WebInputEvent::kShiftKey;
  if (code >= 'A' && code <= 'Z') {
    return std::string(1, static_cast<char>(shift ? code : code - 'A' + 'a'));
  }
  // The US layout's characters, unshifted and with ⇧, as Chromium names a page's key with ⇧.
  struct Key {
    int code;
    char typed;
    char shifted;
  };
  static constexpr Key kKeys[] = {
      {'0', '0', ')'},          {'1', '1', '!'},          {'2', '2', '@'},
      {'3', '3', '#'},          {'4', '4', '$'},          {'5', '5', '%'},
      {'6', '6', '^'},          {'7', '7', '&'},          {'8', '8', '*'},
      {'9', '9', '('},          {VK_OEM_1, ';', ':'},     {VK_OEM_PLUS, '=', '+'},
      {VK_OEM_COMMA, ',', '<'}, {VK_OEM_MINUS, '-', '_'}, {VK_OEM_PERIOD, '.', '>'},
      {VK_OEM_2, '/', '?'},     {VK_OEM_3, '`', '~'},     {VK_OEM_4, '[', '{'},
      {VK_OEM_5, '\\', '|'},    {VK_OEM_6, ']', '}'},     {VK_OEM_7, '\'', '"'},
  };
  for (const Key& key : kKeys) {
    if (key.code == code) {
      return std::string(1, shift ? key.shifted : key.typed);
    }
  }
  return std::string();
}

const SplitLayout& SplitLayoutOf(const input::NativeWebKeyboardEvent& event) {
  static base::NoDestructor<SplitLayoutCache> layouts;
  // A layout is a thread's on Windows: the one of the thread whose window has the key, which is
  // the UI thread this runs on, as GenerateDomKeyboardLayoutMap reads it. A key with no message
  // behind it, as an agent's, is the UI thread's.
  const ui::Event* key = event.os_event;
  const HWND window = key && key->HasNativeEvent() ? key->native_event().hwnd : nullptr;
  const HKL layout = GetKeyboardLayout(window ? GetWindowThreadProcessId(window, nullptr) : 0);
  return layouts->For(base::NumberToString(reinterpret_cast<uintptr_t>(layout)),
                      [] { return ui::GenerateDomKeyboardLayoutMap(); });
}

// Windows' layouts are compared when a split key asks, as no notice of theirs reaches the core.
void StopHearingLayouts() {}

}  // namespace browser_core::platform
