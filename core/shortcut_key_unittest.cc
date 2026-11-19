#include "browser_core/shortcut_key.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

constexpr HeldKeys kShift = {.shift = true};
constexpr HeldKeys kControlShift = {.shift = true, .control = true};
constexpr HeldKeys kControlShiftCommand = {.shift = true, .control = true, .meta = true};
constexpr HeldKeys kOption = {.alt = true};
// Chrome's VKEY_OEM_4, the US layout's [ key, which is å on Danish.
constexpr int kBracketKey = 0xDB;
// Chrome's VKEY_PROCESSKEY, a key an input method takes, as a Japanese composition's are.
constexpr int kProcessKey = 0xE5;

// ⌃⇧2, Go to Pane 2, on a Mac page is named 2 whatever ⇧2 types: " on Danish and German, @ on
// US, as GPUI names it from the UI (#3502).
TEST(ShortcutKeyTest, ControlShiftDigitIsNamedByItsPlaceOnTheMac) {
  for (const char* typed : {"\"", "@"}) {
    EXPECT_EQ(ShortcutKeyName(typed, '2', kControlShift, /*mac=*/true), "2") << typed;
  }
  EXPECT_EQ(ShortcutKeyName("=", '0', kControlShift, /*mac=*/true), "0");
}

// Off the Mac, Ctrl is ⌘, whose keys go by the Latin layout's key instead (latin_key), and ⌃ is
// Alt, which already names the key by its place.
TEST(ShortcutKeyTest, ControlShiftDigitOffTheMacIsWhatItTyped) {
  EXPECT_EQ(ShortcutKeyName("\"", '2', kControlShift, /*mac=*/false), "\"");
  EXPECT_EQ(ShortcutKeyName("\"", '2', {.shift = true, .alt = true}, /*mac=*/false), "2");
}

// Keys that type, or that hold ⌘ too, are what the layout types, as before.
TEST(ShortcutKeyTest, OtherKeysAreWhatTheyType) {
  EXPECT_EQ(ShortcutKeyName("\"", '2', kShift, /*mac=*/true), "\"");
  EXPECT_EQ(ShortcutKeyName("\"", '2', kControlShiftCommand, /*mac=*/true), "\"");
  EXPECT_EQ(ShortcutKeyName("2", '2', {}, /*mac=*/true), "2");
  EXPECT_EQ(ShortcutKeyName("A", 'A', kControlShift, /*mac=*/true), "A");
  EXPECT_EQ(ShortcutKeyName("Å", kBracketKey, kControlShift, /*mac=*/true), "Å");
  EXPECT_EQ(ShortcutKeyName("Process", kProcessKey, {}, /*mac=*/true), "Process");
  EXPECT_EQ(ShortcutKeyName("Process", kProcessKey, kShift, /*mac=*/false), "Process");
}

// With ⌥ held, a letter's or digit's key is named by its place, as ⌥I types "ˆ".
TEST(ShortcutKeyTest, OptionLetterOrDigitIsNamedByItsPlace) {
  EXPECT_EQ(ShortcutKeyName("ˆ", 'I', kOption, /*mac=*/true), "I");
  EXPECT_EQ(ShortcutKeyName("¡", '1', kOption, /*mac=*/true), "1");
  EXPECT_EQ(ShortcutKeyName("“", kBracketKey, kOption, /*mac=*/true), "“");
}

}  // namespace
}  // namespace browser_core
