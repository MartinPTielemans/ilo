#include "browser_core/typing_keys.h"

#include <algorithm>
#include <initializer_list>
#include <iterator>
#include <map>
#include <set>
#include <utility>

#include "base/strings/strcat.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/string_util.h"
#include "base/strings/utf_string_conversions.h"
#include "base/third_party/icu/icu_utf.h"

namespace browser_core {

namespace {

// A date or time value's parts by Blink's names for its segments: year, month, day, week, hour,
// minute, second and millisecond.
using Parts = std::map<std::string_view, int>;

// The text of a value, read from its start.
class Reader {
 public:
  explicit Reader(std::string_view text) : text_(text) {}

  // From |fewest| to |most| digits, as a number from |low| to |high|.
  bool Number(size_t fewest, size_t most, int low, int high, int& value) {
    size_t count = 0;
    int read = 0;
    while (count < most && at_ + count < text_.size() && base::IsAsciiDigit(text_[at_ + count])) {
      read = read * 10 + (text_[at_ + count] - '0');
      ++count;
    }
    if (count < fewest || read < low || read > high) {
      return false;
    }
    at_ += count;
    value = read;
    return true;
  }

  bool Literal(char character) {
    if (at_ < text_.size() && text_[at_] == character) {
      ++at_;
      return true;
    }
    return false;
  }

  bool Done() const { return at_ == text_.size(); }

 private:
  std::string_view text_;
  size_t at_ = 0;
};

bool Leap(int year) {
  return (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
}

// HTML's year has four digits or more, and Blink's fields stop at 275760.
bool ReadYear(Reader& reader, Parts& parts) {
  return reader.Number(4, 6, 1, 275760, parts["year"]);
}

bool ReadMonth(Reader& reader, Parts& parts) {
  return ReadYear(reader, parts) && reader.Literal('-') &&
         reader.Number(2, 2, 1, 12, parts["month"]);
}

bool ReadDate(Reader& reader, Parts& parts) {
  static constexpr int kDays[] = {31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
  if (!ReadMonth(reader, parts) || !reader.Literal('-')) {
    return false;
  }
  const int month = parts["month"];
  const int days = kDays[month - 1] + (month == 2 && Leap(parts["year"]) ? 1 : 0);
  return reader.Number(2, 2, 1, days, parts["day"]);
}

// A year has a 53rd week when it starts on a Thursday, or on a Wednesday in a leap year.
bool ReadWeek(Reader& reader, Parts& parts) {
  if (!ReadYear(reader, parts) || !reader.Literal('-') || !reader.Literal('W')) {
    return false;
  }
  const int before = parts["year"] - 1;
  const int first_day = (before + before / 4 - before / 100 + before / 400 + 1) % 7;
  const bool long_year = first_day == 4 || (first_day == 3 && Leap(parts["year"]));
  return reader.Number(2, 2, 1, long_year ? 53 : 52, parts["week"]);
}

// Hours and minutes, and seconds and their fraction where the value has them.
bool ReadTime(Reader& reader, Parts& parts) {
  if (!reader.Number(2, 2, 0, 23, parts["hour"]) || !reader.Literal(':') ||
      !reader.Number(2, 2, 0, 59, parts["minute"])) {
    return false;
  }
  if (reader.Done()) {
    return true;
  }
  if (!reader.Literal(':') || !reader.Number(2, 2, 0, 59, parts["second"])) {
    return false;
  }
  if (reader.Done()) {
    return true;
  }
  if (!reader.Literal('.')) {
    return false;
  }
  // A fraction of one to three digits, in milliseconds.
  for (size_t digits = 3; digits >= 1; --digits) {
    Reader fraction = reader;
    int value = 0;
    if (fraction.Number(digits, digits, 0, 999, value) && fraction.Done()) {
      parts["millisecond"] = value * (digits == 1 ? 100 : digits == 2 ? 10 : 1);
      reader = fraction;
      return true;
    }
  }
  return false;
}

// The parts of |text|, when it's exactly |type|'s value form.
std::optional<Parts> ValueParts(std::string_view type, std::string_view text) {
  Reader reader(text);
  Parts parts;
  bool read = false;
  if (type == "date") {
    read = ReadDate(reader, parts);
  } else if (type == "time") {
    read = ReadTime(reader, parts);
  } else if (type == "datetime-local") {
    read = ReadDate(reader, parts) && reader.Literal('T') && ReadTime(reader, parts);
  } else if (type == "month") {
    read = ReadMonth(reader, parts);
  } else if (type == "week") {
    read = ReadWeek(reader, parts);
  }
  if (!read || !reader.Done()) {
    return std::nullopt;
  }
  return parts;
}

}  // namespace

bool IsDateValueForm(std::string_view text) {
  return std::ranges::any_of(
      std::initializer_list<std::string_view>{"date", "time", "datetime-local", "month", "week"},
      [text](std::string_view type) { return ValueParts(type, text).has_value(); });
}

std::optional<std::vector<std::vector<base::DictValue>>> DateFieldKeys(
    std::string_view type,
    std::string_view text,
    const std::vector<std::string>& segments) {
  const std::optional<Parts> parts = ValueParts(type, text);
  if (!parts) {
    return std::nullopt;
  }
  static constexpr std::string_view kPrefix = "-webkit-datetime-edit-";
  static constexpr std::string_view kSuffix = "-field";
  // Each part Blink shows, with the digits it's typed as: a year has four at least.
  using Width = std::pair<std::string_view, size_t>;
  static constexpr Width kWidths[] = {
      {"year", 4},   {"month", 2},  {"day", 2},         {"week", 2}, {"hour", 2},
      {"minute", 2}, {"second", 2}, {"millisecond", 3}, {"ampm", 0}};
  std::vector<Width> named;
  std::set<std::string_view> seen;
  for (const std::string& segment : segments) {
    std::string_view name = segment;
    if (!name.starts_with(kPrefix) || !name.ends_with(kSuffix)) {
      return std::nullopt;
    }
    name = name.substr(kPrefix.size(), name.size() - kPrefix.size() - kSuffix.size());
    const auto known = std::ranges::find(kWidths, name, &Width::first);
    if (known == std::end(kWidths) || !seen.insert(name).second) {
      return std::nullopt;
    }
    named.push_back(*known);
  }
  // Each of the value's parts in a segment of its own, and AM or PM only with an hour.
  const bool twelve = seen.contains("ampm");
  if ((twelve && !parts->contains("hour")) || named.size() != parts->size() + (twelve ? 1 : 0)) {
    return std::nullopt;
  }
  const auto key = [](std::string_view name, int code) {
    const base::DictValue event = base::DictValue()
                                      .Set("key", std::string(name))
                                      .Set("code", std::string(name))
                                      .Set("windowsVirtualKeyCode", code);
    std::vector<base::DictValue> keys;
    keys.push_back(event.Clone().Set("type", "rawKeyDown"));
    keys.push_back(event.Clone().Set("type", "keyUp"));
    return keys;
  };
  std::vector<std::vector<base::DictValue>> typed;
  for (const auto& [name, width] : named) {
    if (name == "ampm") {
      std::vector<base::DictValue> keys = key("Backspace", 8);
      for (int up = parts->at("hour") < 12 ? 1 : 2; up > 0; --up) {
        std::ranges::move(key("ArrowUp", 38), std::back_inserter(keys));
      }
      typed.push_back(std::move(keys));
      continue;
    }
    const auto part = parts->find(name);
    if (part == parts->end()) {
      return std::nullopt;
    }
    int value = part->second;
    if (name == "hour" && twelve) {
      value = value % 12 == 0 ? 12 : value % 12;
    }
    std::u16string digits = base::NumberToString16(value);
    if (digits.size() < width) {
      digits.insert(0, width - digits.size(), u'0');
    }
    typed.push_back(TypingKeys(digits));
  }
  return typed;
}

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
