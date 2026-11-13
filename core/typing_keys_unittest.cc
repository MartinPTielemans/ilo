#include "browser_core/typing_keys.h"

#include <string>
#include <vector>

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

}  // namespace
}  // namespace browser_core
