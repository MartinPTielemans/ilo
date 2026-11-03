#include <xkbcommon/xkbcommon.h>

#include <optional>

#include "testing/gtest/include/gtest/gtest.h"
#include "ui/events/keycodes/dom/dom_code.h"
#include "ui/events/ozone/layout/xkb/xkb_evdev_codes.h"
#include "ui/events/ozone/layout/xkb/xkb_keyboard_layout_engine.h"

namespace browser_core {
namespace {

// A Wayland seat's keyboard with no keymap, as fcitx5's virtual one can be, still sends its
// modifiers, which crashed the engine (#1138). Chromium's test for its fix,
// patches/upstream/chromium-xkb-no-state-guard.patch, doesn't apply to the pinned version, so
// this checks the same two calls.
TEST(XkbNoKeymapTest, NothingWithNoKeymap) {
  ui::XkbEvdevCodes codes;
  ui::XkbKeyboardLayoutEngine engine(codes);
  EXPECT_EQ(engine.UpdateModifiers(/*depressed=*/1, /*latched=*/0, /*locked=*/0, /*group=*/0), 0);
  EXPECT_EQ(engine.GetDomCodeByKeysym(XKB_KEY_a, std::nullopt), ui::DomCode::NONE);
}

}  // namespace
}  // namespace browser_core
