#include "browser_core/shortcut_key.h"

#include <map>
#include <string>
#include <utility>
#include <vector>

#include "base/base_paths.h"
#include "base/files/file_path.h"
#include "base/files/file_util.h"
#include "base/path_service.h"
#include "base/strings/string_split.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

constexpr HeldKeys kShift = {.shift = true};
constexpr HeldKeys kControlShift = {.shift = true, .control = true};
constexpr HeldKeys kAltShift = {.shift = true, .alt = true};
constexpr HeldKeys kControlAltShift = {.shift = true, .control = true, .alt = true};
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
    EXPECT_EQ(ShortcutKeyName(typed, '2', "", kControlShift, /*mac=*/true), "2") << typed;
  }
  EXPECT_EQ(ShortcutKeyName("=", '0', "", kControlShift, /*mac=*/true), "0");
}

// Off the Mac, Ctrl is ⌘, whose keys go by the Latin layout's key instead (latin_key), and ⌃ is
// Alt, which already names the key by its place.
TEST(ShortcutKeyTest, ControlShiftDigitOffTheMacIsWhatItTyped) {
  EXPECT_EQ(ShortcutKeyName("\"", '2', "", kControlShift, /*mac=*/false), "\"");
  EXPECT_EQ(ShortcutKeyName("\"", '2', "", kAltShift, /*mac=*/false), "2");
}

// Keys that type, or that hold ⌘ too, are what the layout types, as before.
TEST(ShortcutKeyTest, OtherKeysAreWhatTheyType) {
  EXPECT_EQ(ShortcutKeyName("\"", '2', "", kShift, /*mac=*/true), "\"");
  EXPECT_EQ(ShortcutKeyName("\"", '2', "", kControlShiftCommand, /*mac=*/true), "\"");
  EXPECT_EQ(ShortcutKeyName("2", '2', "", {}, /*mac=*/true), "2");
  EXPECT_EQ(ShortcutKeyName("A", 'A', "", kControlShift, /*mac=*/true), "A");
  EXPECT_EQ(ShortcutKeyName("Å", kBracketKey, "", kControlShift, /*mac=*/true), "Å");
  EXPECT_EQ(ShortcutKeyName("Process", kProcessKey, "", {}, /*mac=*/true), "Process");
  EXPECT_EQ(ShortcutKeyName("Process", kProcessKey, "", kShift, /*mac=*/false), "Process");
}

// With ⌥ held, a letter's or digit's key is named by its place, as ⌥I types "ˆ".
TEST(ShortcutKeyTest, OptionLetterOrDigitIsNamedByItsPlace) {
  EXPECT_EQ(ShortcutKeyName("ˆ", 'I', "", kOption, /*mac=*/true), "I");
  EXPECT_EQ(ShortcutKeyName("¡", '1', "", kOption, /*mac=*/true), "1");
  EXPECT_EQ(ShortcutKeyName("“", kBracketKey, "", kOption, /*mac=*/true), "“");
}

// The split keys are held as GPUI's patches hold them in the UI: ⌃⇧ without ⌘ on the Mac, ⇧ with
// one of Ctrl or Alt on Windows, where both are AltGr, and ⇧ with either on Linux.
TEST(ShortcutKeyTest, SplitKeysAreHeldAsTheUiHoldsThem) {
  EXPECT_TRUE(SplitKeysHeld(kControlShift, KeysPlatform::kMac));
  EXPECT_TRUE(SplitKeysHeld(kControlAltShift, KeysPlatform::kMac));
  EXPECT_FALSE(SplitKeysHeld(kControlShiftCommand, KeysPlatform::kMac));
  EXPECT_FALSE(SplitKeysHeld(kAltShift, KeysPlatform::kMac));

  EXPECT_TRUE(SplitKeysHeld(kControlShift, KeysPlatform::kWindows));
  EXPECT_TRUE(SplitKeysHeld(kAltShift, KeysPlatform::kWindows));
  EXPECT_FALSE(SplitKeysHeld(kControlAltShift, KeysPlatform::kWindows));
  EXPECT_FALSE(SplitKeysHeld(kControlShiftCommand, KeysPlatform::kWindows));

  EXPECT_TRUE(SplitKeysHeld(kControlShift, KeysPlatform::kLinux));
  EXPECT_TRUE(SplitKeysHeld(kAltShift, KeysPlatform::kLinux));
  EXPECT_TRUE(SplitKeysHeld(kControlAltShift, KeysPlatform::kLinux));

  for (KeysPlatform platform :
       {KeysPlatform::kMac, KeysPlatform::kWindows, KeysPlatform::kLinux}) {
    EXPECT_FALSE(SplitKeysHeld(kShift, platform));
    EXPECT_FALSE(SplitKeysHeld({.control = true}, platform));
    EXPECT_FALSE(SplitKeysHeld(kOption, platform));
  }
}

// A line of core/split_keys_layouts.txt: a layout's base characters by the web's code for each
// key's place, and the US place of the key each split key runs on, or "none".
struct TableLayout {
  std::string name;
  LayoutKeys keys;
  std::vector<std::pair<char, std::string>> runs_on;
};

// The web's code for a US place the table names.
std::string CodeAt(const std::string& place) {
  static const std::map<std::string, std::string> kCodes = {
      {"[", "BracketLeft"}, {"]", "BracketRight"}, {"-", "Minus"},
      {"\\", "Backslash"},  {"/", "Slash"},        {"6", "Digit6"},
  };
  auto code = kCodes.find(place);
  return code == kCodes.end() ? std::string() : code->second;
}

std::vector<TableLayout> ReadTable() {
  base::FilePath root;
  EXPECT_TRUE(base::PathService::Get(base::DIR_SRC_TEST_DATA_ROOT, &root));
  std::string text;
  EXPECT_TRUE(base::ReadFileToString(
      root.AppendASCII("browser_core").AppendASCII("split_keys_layouts.txt"), &text));
  std::vector<TableLayout> layouts;
  for (const std::string& line :
       base::SplitString(text, "\n", base::TRIM_WHITESPACE, base::SPLIT_WANT_NONEMPTY)) {
    if (line.starts_with("#")) {
      continue;
    }
    std::vector<std::string> words =
        base::SplitString(line, " ", base::TRIM_WHITESPACE, base::SPLIT_WANT_NONEMPTY);
    TableLayout layout{.name = words[0]};
    std::vector<std::pair<std::string, std::string>> keys;
    std::vector<std::string> targets;
    for (size_t i = 1; i < words.size(); i++) {
      const size_t equals = words[i].find('=', 1);
      if (equals == std::string::npos) {
        targets.push_back(words[i]);
      } else {
        const std::string code = CodeAt(words[i].substr(0, equals));
        EXPECT_FALSE(code.empty()) << line;
        keys.emplace_back(code, words[i].substr(equals + 1));
      }
    }
    EXPECT_EQ(targets.size(), 3u) << line;
    layout.keys = LayoutKeys(std::move(keys));
    for (size_t i = 0; i < targets.size() && i < 3; i++) {
      layout.runs_on.emplace_back("[]-"[i], targets[i]);
    }
    layouts.push_back(std::move(layout));
  }
  return layouts;
}

// Each layout in the table the app's tests read names a page's key as the UI does: the split key
// runs on the key whose base character it is, else on the key at its US place, unless that types
// another split key. A key that's a digit's, as French's 6 key typing -, is the split key's first.
TEST(ShortcutKeyTest, SplitKeysRunOnTheKeysTheLayoutsTableSays) {
  const std::vector<TableLayout> table = ReadTable();
  ASSERT_GE(table.size(), 6u);
  for (const TableLayout& line : table) {
    const SplitLayout layout(line.keys);
    for (const auto& [split, place] : line.runs_on) {
      const std::string key(1, split);
      for (const auto& [code, base] : line.keys) {
        const bool runs_here = place != "none" && code == CodeAt(place);
        EXPECT_EQ(SplitKeyName(code, layout) == key, runs_here)
            << line.name << " " << key << " at " << code;
      }
      if (place == "none") {
        continue;
      }
      // Named so ahead of a digit's place and what the key types with ⇧, on the Mac and off it.
      const std::string named = SplitKeyName(CodeAt(place), layout);
      const int windows_key_code = place == "6" ? '6' : kBracketKey;
      EXPECT_EQ(ShortcutKeyName("&", windows_key_code, named, kControlShift, /*mac=*/true), key)
          << line.name;
      EXPECT_EQ(ShortcutKeyName("&", windows_key_code, named, kAltShift, /*mac=*/false), key)
          << line.name;
    }
  }
}

// Only the three keys change: a layout's other keys, and every key on a layout that couldn't be
// read, are named as before.
TEST(ShortcutKeyTest, OnlySplitKeysAreNamedByTheirLayout) {
  const SplitLayout danish(
      LayoutKeys({{"BracketLeft", "å"}, {"Slash", "-"}, {"KeyA", "a"}, {"Digit2", "2"}}));
  EXPECT_EQ(SplitKeyName("KeyA", danish), "");
  EXPECT_EQ(SplitKeyName("Digit2", danish), "");
  EXPECT_EQ(ShortcutKeyName("\"", '2', SplitKeyName("Digit2", danish), kControlShift, true), "2");
  const SplitLayout unread;
  EXPECT_EQ(unread.by_place, "");
  for (const char* code : {"BracketLeft", "BracketRight", "Minus"}) {
    EXPECT_EQ(SplitKeyName(code, unread), "") << code;
  }
}

// On Wayland and Windows the layout is compared with the one its table was made for: a change of
// layout between two presses is seen on the second, and no press reads it again otherwise.
TEST(ShortcutKeyTest, ALayoutChangedBetweenTwoPressesIsSeenOnTheSecond) {
  const LayoutKeys us({{"BracketLeft", "["}, {"BracketRight", "]"}, {"Minus", "-"}});
  const LayoutKeys danish({{"BracketLeft", "å"}, {"BracketRight", "¨"}, {"Slash", "-"}});
  SplitLayoutCache layouts;
  int reads = 0;
  auto read = [&](const LayoutKeys& keys) {
    return [&reads, &keys] {
      reads++;
      return keys;
    };
  };
  EXPECT_EQ(SplitKeyName("Slash", layouts.For("us", read(us))), "");
  EXPECT_EQ(SplitKeyName("Slash", layouts.For("us", read(danish))), "");
  EXPECT_EQ(reads, 1);
  EXPECT_EQ(SplitKeyName("Slash", layouts.For("da", read(danish))), "-");
  EXPECT_EQ(SplitKeyName("BracketLeft", layouts.For("da", read(us))), "[");
  EXPECT_EQ(reads, 2);
  EXPECT_EQ(SplitKeyName("Slash", layouts.For("us", read(us))), "");
  EXPECT_EQ(reads, 3);
}

}  // namespace
}  // namespace browser_core
