#include "browser_core/typing_keys.h"

#include <utility>

#include "base/strings/strcat.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "base/third_party/icu/icu_utf.h"

namespace browser_core {

std::vector<base::DictValue> TypingKeys(const std::u16string& text) {
  // The text's characters, a surrogate pair one.
  std::vector<std::u16string> characters;
  for (size_t at = 0; at < text.size();) {
    const size_t size =
        CBU16_IS_LEAD(text[at]) && at + 1 < text.size() && CBU16_IS_TRAIL(text[at + 1]) ? 2 : 1;
    characters.push_back(text.substr(at, size));
    at += size;
  }
  std::vector<base::DictValue> keys;
  const auto inserted = [&keys](const std::u16string& piece) {
    const std::string utf8 = base::UTF16ToUTF8(piece);
    keys.push_back(base::DictValue()
                       .Set("type", "char")
                       .Set("text", utf8)
                       .Set("unmodifiedText", utf8)
                       .Set("key", "Unidentified"));
  };
  for (size_t at = 0; at < characters.size();) {
    const std::u16string& character = characters[at];
    // A run of line breaks, with the character before it or, at the text's start, after it.
    const bool printable = character.size() == 2 || character[0] >= u' ';
    const bool before = printable && at + 1 < characters.size() && characters[at + 1] == u"\n";
    if (character == u"\n" || before) {
      size_t end = at;
      while (end < characters.size() && (end == at || characters[end] == u"\n")) {
        ++end;
      }
      if (character == u"\n" && end < characters.size()) {
        ++end;
      }
      // Pieces of 2 or 3 units, each more than a lone control character.
      std::vector<std::u16string> pieces;
      for (size_t each = at; each < end; ++each) {
        if (pieces.empty() || pieces.back().size() + characters[each].size() > 3) {
          pieces.emplace_back();
        }
        pieces.back() += characters[each];
      }
      if (pieces.size() > 1 && pieces.back().size() == 1) {
        pieces.back().insert(0, 1, pieces[pieces.size() - 2].back());
        pieces[pieces.size() - 2].pop_back();
      }
      for (const std::u16string& piece : pieces) {
        if (piece == u"\n") {
          // The text is a line break alone, which only Enter's command puts in.
          keys.push_back(base::DictValue()
                             .Set("type", "char")
                             .Set("key", "Enter")
                             .Set("code", "Enter")
                             .Set("windowsVirtualKeyCode", 13)
                             .Set("text", "\r")
                             .Set("unmodifiedText", "\r"));
        } else {
          inserted(piece);
        }
      }
      at = end;
      continue;
    }
    ++at;
    // A tab goes in by its keypress's command; another control character not at all, as it didn't
    // reach the field from a keyboard either.
    if (character == u"\t") {
      inserted(character);
      continue;
    }
    if (character.size() == 1 && character[0] < u' ') {
      continue;
    }
    const std::string utf8 = base::UTF16ToUTF8(character);
    base::DictValue key =
        base::DictValue().Set("key", utf8).Set("text", utf8).Set("unmodifiedText", utf8);
    // A letter, a digit and the space bar have the key codes a page's handlers look for.
    const char16_t only = character.size() == 1 ? character[0] : 0;
    if (base::IsAsciiAlpha(only)) {
      const char upper = static_cast<char>(base::ToUpperASCII(only));
      key.Set("code", base::StrCat({"Key", std::string(1, upper)}));
      key.Set("windowsVirtualKeyCode", upper);
      if (base::IsAsciiUpper(only)) {
        key.Set("modifiers", 8);
      }
    } else if (base::IsAsciiDigit(only)) {
      key.Set("code", base::StrCat({"Digit", std::string(1, static_cast<char>(only))}));
      key.Set("windowsVirtualKeyCode", static_cast<int>(only));
    } else if (only == u' ') {
      key.Set("code", "Space");
      key.Set("windowsVirtualKeyCode", 32);
    }
    keys.push_back(key.Clone().Set("type", "keyDown"));
    key.Set("type", "keyUp");
    keys.push_back(std::move(key));
  }
  return keys;
}

}  // namespace browser_core
