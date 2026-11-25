#include "browser_core/typing_keys.h"

#include <initializer_list>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "base/strings/strcat.h"
#include "base/strings/utf_string_conversions.h"
#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

// Each event as its type and text, "keyDown a", and the text the page gets, in order.
std::vector<std::string> Events(const std::u16string& text) {
  std::vector<std::string> events;
  for (const base::DictValue& key : TypingKeys(text)) {
    const std::string* type = key.FindString("type");
    const std::string* typed = key.FindString("text");
    events.push_back((type ? *type : "") + " " + (typed ? *typed : ""));
  }
  return events;
}

// What a page's editing inserts: each keyDown's and char's text, as keyUp inserts none.
std::u16string Inserted(const std::u16string& text) {
  std::u16string inserted;
  for (const base::DictValue& key : TypingKeys(text)) {
    const std::string* type = key.FindString("type");
    const std::string* typed = key.FindString("text");
    if (type && typed && *type != "keyUp") {
      inserted += *typed == "\r" ? u"\n" : base::UTF8ToUTF16(*typed);
    }
  }
  return inserted;
}

TEST(TypingKeysTest, ACharacterIsAKeyDownAndUp) {
  EXPECT_EQ(Events(u"a1"), (std::vector<std::string>{"keyDown a", "keyUp a", "keyDown 1",
                                                     "keyUp 1"}));
  const std::vector<base::DictValue> keys = TypingKeys(u"A");
  EXPECT_EQ(*keys[0].FindString("code"), "KeyA");
  EXPECT_EQ(keys[0].FindInt("windowsVirtualKeyCode"), 65);
  EXPECT_EQ(keys[0].FindInt("modifiers"), 8);
  EXPECT_EQ(TypingKeys(u"7")[0].FindInt("windowsVirtualKeyCode"), 55);
}

// A line break is never Enter's key down, which sends a chat's message or submits a form: it goes
// in as text with the character beside it, in a keypress whose character isn't Enter's.
TEST(TypingKeysTest, ALineBreakIsTextWithItsNeighbour) {
  EXPECT_EQ(Events(u"a\nb"),
            (std::vector<std::string>{"char a\n", "keyDown b", "keyUp b"}));
  EXPECT_EQ(Events(u"\nb"), (std::vector<std::string>{"char \nb"}));
  EXPECT_EQ(Events(u"a\n"), (std::vector<std::string>{"char a\n"}));
  for (const std::u16string text :
       {u"line one\nline two\n\nline four", u"a\n\n\n\n\nb", u"\n\n\nx", u"😀\n\n", u"x\n\n\n"}) {
    EXPECT_EQ(Inserted(text), text);
    for (const base::DictValue& key : TypingKeys(text)) {
      // The other characters are their own keys; each line break is in a keypress of more than
      // a lone control character, which Blink doesn't insert, and at most 3 units.
      const std::string* typed = key.FindString("text");
      if (typed->find('\n') == std::string::npos) {
        continue;
      }
      EXPECT_EQ(*key.FindString("type"), "char") << *typed;
      EXPECT_GE(base::UTF8ToUTF16(*typed).size(), 2u) << *typed;
      EXPECT_LE(base::UTF8ToUTF16(*typed).size(), 3u) << *typed;
    }
  }
  // Only a text that's a line break alone has Enter's keypress, as nothing else puts it in.
  EXPECT_EQ(Events(u"\n"), (std::vector<std::string>{"char \r"}));
}

// A character outside the BMP is one key, never half a pair, and combining marks and CJK text go in
// exactly as given.
TEST(TypingKeysTest, TextThatIsntOneKeyArrivesExact) {
  for (const std::u16string text : {u"😀", u"👩‍👩‍👧", u"日本語のテキスト", u"é", u"漢字\n😀"}) {
    EXPECT_EQ(Inserted(text), text);
  }
  EXPECT_EQ(Events(u"😀"), (std::vector<std::string>{"keyDown 😀", "keyUp 😀"}));
}

TEST(TypingKeysTest, ATabIsItsKeypressAndOtherControlCharactersGo) {
  EXPECT_EQ(Events(u"a\tb"),
            (std::vector<std::string>{"keyDown a", "keyUp a", "char \t", "keyDown b", "keyUp b"}));
  EXPECT_EQ(Inserted(u"a\x01" u"b"), u"ab");
}

// The segments Blink shows, by their names alone: "month" for -webkit-datetime-edit-month-field.
std::vector<std::string> Segments(std::initializer_list<std::string_view> names) {
  std::vector<std::string> segments;
  for (std::string_view name : names) {
    segments.push_back(base::StrCat({"-webkit-datetime-edit-", name, "-field"}));
  }
  return segments;
}

using Pressed = std::vector<std::string>;

// What each segment is pressed, its text as the page gets it and a key with no text by name,
// "[ArrowUp]", or nothing when the text goes as keys alone. Each key goes down and comes up.
std::optional<Pressed> Typed(std::string_view type,
                             std::string_view text,
                             std::initializer_list<std::string_view> names) {
  std::optional<std::vector<std::vector<base::DictValue>>> keys =
      DateFieldKeys(type, text, Segments(names));
  if (!keys) {
    return std::nullopt;
  }
  Pressed pressed;
  for (const std::vector<base::DictValue>& segment : *keys) {
    std::string each;
    int down = 0;
    for (const base::DictValue& key : segment) {
      const std::string* kind = key.FindString("type");
      const std::string* typed = key.FindString("text");
      const std::string* name = key.FindString("key");
      if (kind && *kind == "keyUp") {
        --down;
      } else if (kind && *kind == "rawKeyDown" && name) {
        ++down;
        each += "[" + *name + "]";
      } else if (typed) {
        ++down;
        each += *typed;
      }
    }
    EXPECT_EQ(down, 0) << each;
    pressed.push_back(each);
  }
  return pressed;
}

// A date goes into its segments in the order the locale shows them, never as typed, each part
// with its width's digits (#4281).
TEST(DateFieldKeysTest, ADateGoesInTheOrderItsSegmentsShow) {
  EXPECT_EQ(Typed("date", "2026-11-25", {"month", "day", "year"}),
            (Pressed{"11", "25", "2026"}));
  EXPECT_EQ(Typed("date", "2026-11-25", {"day", "month", "year"}),
            (Pressed{"25", "11", "2026"}));
  EXPECT_EQ(Typed("date", "2026-02-05", {"year", "month", "day"}), (Pressed{"2026", "02", "05"}));
  EXPECT_EQ(Typed("date", "2024-02-29", {"month", "day", "year"}), (Pressed{"02", "29", "2024"}));
  EXPECT_EQ(Typed("date", "10000-01-01", {"month", "day", "year"}),
            (Pressed{"01", "01", "10000"}));
}

TEST(DateFieldKeysTest, ATimeGoesInItsSegmentsWithSecondsWhereTheFieldHasThem) {
  EXPECT_EQ(Typed("time", "14:30", {"hour", "minute"}), (Pressed{"14", "30"}));
  EXPECT_EQ(Typed("time", "00:05", {"hour", "minute"}), (Pressed{"00", "05"}));
  EXPECT_EQ(Typed("time", "14:30:05", {"hour", "minute", "second"}),
            (Pressed{"14", "30", "05"}));
  EXPECT_EQ(Typed("time", "14:30:05.25", {"hour", "minute", "second", "millisecond"}),
            (Pressed{"14", "30", "05", "250"}));
  EXPECT_EQ(Typed("time", "14:30:05.007", {"hour", "minute", "second", "millisecond"}),
            (Pressed{"14", "30", "05", "007"}));
}

// A 12-hour field's hour is 01 to 12. Its AM or PM, whose labels are the locale's, is emptied,
// then stepped up once to AM, the first, or twice to PM.
TEST(DateFieldKeysTest, ATwelveHourTimeTypesItsHourAndStepsToAmOrPm) {
  const std::string am = "[Backspace][ArrowUp]";
  const std::string pm = "[Backspace][ArrowUp][ArrowUp]";
  EXPECT_EQ(Typed("time", "14:30", {"hour", "minute", "ampm"}), (Pressed{"02", "30", pm}));
  EXPECT_EQ(Typed("time", "00:05", {"hour", "minute", "ampm"}), (Pressed{"12", "05", am}));
  EXPECT_EQ(Typed("time", "12:00", {"hour", "minute", "ampm"}), (Pressed{"12", "00", pm}));
  EXPECT_EQ(Typed("time", "11:59", {"hour", "minute", "ampm"}), (Pressed{"11", "59", am}));
  EXPECT_EQ(Typed("time", "09:15", {"ampm", "hour", "minute"}), (Pressed{am, "09", "15"}));
}

TEST(DateFieldKeysTest, ADateAndTimeAMonthAndAWeekGoInTheirSegments) {
  EXPECT_EQ(Typed("datetime-local", "2026-11-25T09:05",
                  {"month", "day", "year", "hour", "minute", "ampm"}),
            (Pressed{"11", "25", "2026", "09", "05", "[Backspace][ArrowUp]"}));
  EXPECT_EQ(Typed("datetime-local", "2026-11-25T21:05", {"day", "month", "year", "hour", "minute"}),
            (Pressed{"25", "11", "2026", "21", "05"}));
  EXPECT_EQ(Typed("month", "2026-11", {"month", "year"}), (Pressed{"11", "2026"}));
  EXPECT_EQ(Typed("week", "2026-W08", {"week", "year"}), (Pressed{"08", "2026"}));
  // 2026 starts on a Thursday, so it has a 53rd week, and 2025 doesn't.
  EXPECT_EQ(Typed("week", "2026-W53", {"week", "year"}), (Pressed{"53", "2026"}));
  EXPECT_EQ(Typed("week", "2025-W53", {"week", "year"}), std::nullopt);
}

// Text that isn't exactly the type's value form is typed as keys, as into any field.
TEST(DateFieldKeysTest, AWrongFormIsTypedAsKeys) {
  const std::initializer_list<std::string_view> date = {"month", "day", "year"};
  for (std::string_view text : {"11/25/2026", "25 November 2026", "2026-11-31", "2025-02-29",
                                "2026-2-5", "2026-11-25 ", "26-11-25", "14:30", ""}) {
    EXPECT_EQ(Typed("date", text, date), std::nullopt) << text;
  }
  for (std::string_view text : {"2:30 PM", "24:00", "14:60", "14:30:5", "14:30:05.1234"}) {
    EXPECT_EQ(Typed("time", text, {"hour", "minute", "second", "millisecond"}), std::nullopt)
        << text;
  }
  // A space for the T is HTML's too, but not the value the input gives back.
  EXPECT_EQ(Typed("datetime-local", "2026-11-25 09:05", {"month", "day", "year", "hour", "minute"}),
            std::nullopt);
  // Seconds the field has no segment for, or a segment the value has no seconds for.
  EXPECT_EQ(Typed("time", "14:30:05", {"hour", "minute"}), std::nullopt);
  EXPECT_EQ(Typed("time", "14:30", {"hour", "minute", "second"}), std::nullopt);
  // Another input's value form, or an input that isn't a date's.
  EXPECT_EQ(Typed("month", "2026-11-25", {"month", "year"}), std::nullopt);
  EXPECT_EQ(Typed("text", "2026-11-25", date), std::nullopt);
  EXPECT_EQ(Typed("", "2026-11-25", date), std::nullopt);
}

// Segments that can't be read as the value's parts leave the text to be typed as keys.
TEST(DateFieldKeysTest, AnOrderThatCantBeReadIsTypedAsKeys) {
  EXPECT_EQ(Typed("date", "2026-11-25", {}), std::nullopt);
  EXPECT_EQ(Typed("date", "2026-11-25", {"month", "day"}), std::nullopt);
  EXPECT_EQ(Typed("date", "2026-11-25", {"month", "day", "year", "week"}), std::nullopt);
  EXPECT_EQ(Typed("date", "2026-11-25", {"month", "day", "day"}), std::nullopt);
  EXPECT_EQ(Typed("date", "2026-11-25", {"month", "day", "era"}), std::nullopt);
  EXPECT_EQ(Typed("month", "2026-11", {"month", "year", "ampm"}), std::nullopt);
  EXPECT_FALSE(DateFieldKeys("date", "2026-11-25", {"month", "day", "year"}));
  EXPECT_FALSE(DateFieldKeys("date", "2026-11-25",
                             {"-webkit-datetime-edit-month-field",
                              "-webkit-datetime-edit-fields-wrapper",
                              "-webkit-datetime-edit-year-field"}));
}

TEST(DateFieldKeysTest, OnlyAValueFormIsWorthReadingTheFieldFor) {
  for (std::string_view text : {"2026-11-25", "14:30", "14:30:05.250", "2026-11-25T14:30",
                                "2026-11", "2026-W48"}) {
    EXPECT_TRUE(IsDateValueForm(text)) << text;
  }
  for (std::string_view text : {"Lisbon", "2026", "11/25/2026", "2026-11-25 14:30", "", "1430"}) {
    EXPECT_FALSE(IsDateValueForm(text)) << text;
  }
}

}  // namespace
}  // namespace browser_core
