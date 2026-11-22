#include "browser_core/platform/keyboard.h"

#include <xkbcommon/xkbcommon.h>

#include <map>
#include <memory>
#include <utility>
#include <vector>

#include "base/no_destructor.h"
#include "base/strings/strcat.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversion_utils.h"
#include "browser_core/platform/linux/wayland.h"
#include "components/input/native_web_keyboard_event.h"
#include "third_party/blink/public/common/input/web_input_event.h"
#include "ui/events/event.h"
#include "ui/events/event_constants.h"
#include "ui/events/event_utils.h"
#include "ui/events/keycodes/dom/dom_code.h"
#include "ui/events/keycodes/dom/dom_keyboard_layout.h"
#include "ui/events/keycodes/dom/keycode_converter.h"
#include "ui/events/keycodes/keyboard_code_conversion.h"
#include "ui/events/keycodes/scoped_xkb.h"
#include "ui/events/ozone/layout/keyboard_layout_engine_manager.h"
#include "ui/events/ozone/layout/xkb/xkb_keyboard_layout_engine.h"
#include "ui/gfx/x/connection.h"
#include "ui/gfx/x/event.h"
#include "ui/gfx/x/event_observer.h"
#include "ui/gfx/x/xkb.h"
#include "ui/gfx/x/xproto.h"

namespace browser_core::platform {

namespace {

// Ozone's layout engine on Wayland keeps the compositor's keymap and the layout typing in its XKB
// state, which it doesn't expose, and its Lookup reads only the layout typing.
struct WaylandEngine : ui::XkbKeyboardLayoutEngine {
  static xkb_state* State() {
    auto* engine = static_cast<ui::XkbKeyboardLayoutEngine*>(
        ui::KeyboardLayoutEngineManager::GetKeyboardLayoutEngine());
    return engine ? (engine->*&WaylandEngine::xkb_state_).get() : nullptr;
  }
};

// The XKB layouts the user switches between, as "us,il" has two: how many, and the one typing.
// On Wayland they're the engine's keymap's, looked up in a state of our own, as the engine's is
// the one it types with; on X11 the server's, through the X connection, where the key's event
// carries the group typing.
struct Layouts {
  uint32_t count = 0;
  uint32_t current = 0;
  std::unique_ptr<xkb_state, ui::XkbStateDeleter> wayland;

  explicit operator bool() const { return count; }

  // The character |keycode| types on |layout|, with ⇧ or without.
  uint32_t Typed(uint32_t keycode, uint32_t layout, bool shift) const {
    if (wayland) {
      xkb_keymap* keymap = xkb_state_get_keymap(wayland.get());
      const xkb_mod_mask_t mods =
          shift ? 1u << xkb_keymap_mod_get_index(keymap, XKB_MOD_NAME_SHIFT) : 0;
      xkb_state_update_mask(wayland.get(), mods, 0, 0, 0, 0, layout);
      return xkb_state_key_get_utf32(wayland.get(), keycode);
    }
    // The group is a state's bits 13 and 14.
    const uint32_t state =
        (layout << 13) | (shift ? static_cast<uint32_t>(x11::KeyButMask::Shift) : 0);
    return xkb_keysym_to_utf32(
        x11::Connection::Get()->KeycodeToKeysym(static_cast<x11::KeyCode>(keycode), state));
  }
};

Layouts WaylandLayouts() {
  xkb_state* engine = WaylandEngine::State();
  if (!engine) {
    return {};
  }
  xkb_keymap* keymap = xkb_state_get_keymap(engine);
  // Every lookup the engine makes sets the layout typing as its state's.
  return {xkb_keymap_num_layouts(keymap),
          xkb_state_serialize_layout(engine, XKB_STATE_LAYOUT_EFFECTIVE),
          std::unique_ptr<xkb_state, ui::XkbStateDeleter>(xkb_state_new(keymap))};
}

Layouts X11Layouts(const ui::Event* event) {
  const ui::Event::Properties* properties = event ? event->properties() : nullptr;
  if (!properties) {
    return {};
  }
  auto group = properties->find(ui::kPropertyKeyboardGroup);
  if (group == properties->end() || group->second.empty()) {
    return {};
  }
  // X has four groups, wrapped onto those the keymap has.
  return {4, group->second[0], nullptr};
}

// The keys |layout| types without ⇧, by the web's code for each key's place, as
// GenerateDomKeyboardLayoutMap gives them elsewhere: on X11 its engine is a stub's, the US layout.
LayoutKeys KeysOf(const Layouts& layouts, uint32_t layout) {
  std::vector<std::pair<std::string, std::string>> keys;
  for (const ui::DomCode code : ui::kWritingSystemKeyDomCodes) {
    // A dead key types nothing until the next, as GPUI's patch reads a key's level-0 character.
    const uint32_t typed =
        layouts.Typed(ui::KeycodeConverter::DomCodeToXkbKeycode(code), layout, false);
    if (typed > ' ' && typed != 0x7f) {
      std::string character;
      base::WriteUnicodeCharacter(typed, &character);
      keys.emplace_back(ui::KeycodeConverter::DomCodeToCodeString(code), std::move(character));
    }
  }
  return LayoutKeys(std::move(keys));
}

// The split keys' layouts, made again when the layout changed.
struct SplitLayouts {
  // Wayland's, for the engine's keymap, the layout typing on it and that layout's name, as xkb
  // gives a new keymap a freed one's address. No notice of the compositor's keymap reaches the
  // core, so they're compared when a split key asks.
  SplitLayoutCache wayland;
  // X11's, for each group a key's event carries, until the server says its keymap changed.
  std::map<uint32_t, SplitLayout> x11;
  bool hearing = false;
};

SplitLayouts& Split() {
  static base::NoDestructor<SplitLayouts> layouts;
  return *layouts;
}

// The server's keymap changed: the events after which x11::Connection reads it again.
struct X11LayoutNotices : x11::EventObserver {
  void OnEvent(const x11::Event& event) override {
    const auto* mapping = event.As<x11::MappingNotifyEvent>();
    if ((mapping && (mapping->request == x11::Mapping::Keyboard ||
                     mapping->request == x11::Mapping::Modifier)) ||
        event.As<x11::Xkb::NewKeyboardNotifyEvent>()) {
      Split().x11.clear();
    }
  }
};

X11LayoutNotices& X11Notices() {
  static base::NoDestructor<X11LayoutNotices> notices;
  return *notices;
}

const SplitLayout& WaylandSplitLayout() {
  static const SplitLayout kNone;
  xkb_state* engine = WaylandEngine::State();
  if (!engine) {
    return kNone;
  }
  xkb_keymap* keymap = xkb_state_get_keymap(engine);
  const uint32_t layout = xkb_state_serialize_layout(engine, XKB_STATE_LAYOUT_EFFECTIVE);
  const char* name = xkb_keymap_layout_get_name(keymap, layout);
  const std::string id = base::StrCat({base::NumberToString(reinterpret_cast<uintptr_t>(keymap)),
                                       " ", base::NumberToString(layout), " ", name ? name : ""});
  return Split().wayland.For(id, [&] { return KeysOf(WaylandLayouts(), layout); });
}

const SplitLayout& X11SplitLayout(const ui::Event* event) {
  static const SplitLayout kNone;
  const Layouts layouts = X11Layouts(event);
  if (!layouts) {
    return kNone;
  }
  SplitLayouts& split = Split();
  // Heard from the first key that asks, once; the server tells only when its keymap changes.
  if (!split.hearing) {
    x11::Connection::Get()->AddEventObserver(&X11Notices());
    split.hearing = true;
  }
  auto made = split.x11.find(layouts.current);
  if (made == split.x11.end()) {
    made = split.x11.emplace(layouts.current, SplitLayout(KeysOf(layouts, layouts.current))).first;
  }
  return made->second;
}

}  // namespace

const SplitLayout& SplitLayoutOf(const input::NativeWebKeyboardEvent& event) {
  return OnWayland() ? WaylandSplitLayout() : X11SplitLayout(event.os_event);
}

void StopHearingLayouts() {
  SplitLayouts& split = Split();
  if (split.hearing) {
    x11::Connection::Get()->RemoveEventObserver(&X11Notices());
    split.hearing = false;
  }
}

std::string LatinKey(const input::NativeWebKeyboardEvent& event) {
  const auto code = static_cast<ui::DomCode>(event.dom_code);
  const bool shift = event.GetModifiers() & blink::WebInputEvent::kShiftKey;
  const Layouts layouts = OnWayland() ? WaylandLayouts() : X11Layouts(event.os_event);
  if (!layouts) {
    return std::string();
  }
  // A layout is Latin when a key in the letters' places types a Latin letter. AZERTY and Dvorak
  // are, and keep their own characters: AZERTY's Ctrl on the W key is Ctrl+Z.
  auto latin = [&](uint32_t layout) {
    for (auto letter = static_cast<uint32_t>(ui::DomCode::US_A);
         letter <= static_cast<uint32_t>(ui::DomCode::US_Z); ++letter) {
      const uint32_t keycode =
          ui::KeycodeConverter::DomCodeToXkbKeycode(static_cast<ui::DomCode>(letter));
      if (base::IsAsciiAlpha(layouts.Typed(keycode, layout, false))) {
        return true;
      }
    }
    return false;
  };
  if (latin(layouts.current)) {
    return std::string();
  }
  // The first Latin layout the user switches to, else the key's place on the US layout.
  uint32_t typed = ui::DomCodeToUsLayoutCharacter(code, shift ? ui::EF_SHIFT_DOWN : ui::EF_NONE);
  for (uint32_t layout = 0; layout < layouts.count; ++layout) {
    if (latin(layout)) {
      typed = layouts.Typed(ui::KeycodeConverter::DomCodeToXkbKeycode(code), layout, shift);
      break;
    }
  }
  if (typed <= ' ' || typed >= 0x7f) {
    return std::string();
  }
  return std::string(1, static_cast<char>(typed));
}

}  // namespace browser_core::platform
