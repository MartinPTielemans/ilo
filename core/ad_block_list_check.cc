#include "browser_core/ad_block_list_check.h"

#include <algorithm>
#include <array>
#include <optional>
#include <string>

#include "base/strings/string_number_conversions.h"
#include "base/strings/string_util.h"

namespace browser_core::ad_block_list_check {

namespace {

// Whether |line|, trimmed, is a comment: `!` and `[Adblock Plus …]` in a filter list, and `#` in
// a hosts file, but not `##` and the other cosmetic filters a site may leave out before them.
bool IsComment(std::string_view line) {
  if (line.starts_with('!') || line.starts_with('[')) {
    return true;
  }
  if (!line.starts_with('#')) {
    return false;
  }
  return line.size() == 1 || std::string_view("#@?$%").find(line[1]) == std::string_view::npos;
}

// The number of |digits| digits at |text|'s |at|, or null if they aren't all there.
std::optional<int> Number(std::string_view text, size_t at, size_t digits) {
  int number = 0;
  if (at + digits > text.size() || !base::StringToInt(text.substr(at, digits), &number) ||
      !base::IsAsciiDigit(text[at])) {
    return std::nullopt;
  }
  return number;
}

// |text| as a time: ISO 8601 in UTC, such as URLhaus's `2026-11-19T00:10:18Z`, with or without
// its time of day, or an HTTP date, such as EasyList's `Wed, 18 Nov 2026 12:15:39 +0000`.
std::optional<base::Time> ReadTime(std::string_view text) {
  const std::optional<int> year = Number(text, 0, 4);
  if (year && text.size() >= 10 && text[4] == '-' && text[7] == '-') {
    base::Time::Exploded exploded = {};
    exploded.year = *year;
    std::optional<int> month = Number(text, 5, 2), day = Number(text, 8, 2), hour = 0, minute = 0,
                       second = 0;
    std::string_view rest = text.substr(10);
    if (rest.starts_with('T') || rest.starts_with(' ')) {
      hour = Number(rest, 1, 2);
      minute = rest.size() >= 6 && rest[3] == ':' ? Number(rest, 4, 2) : std::nullopt;
      rest = rest.substr(std::min<size_t>(rest.size(), 6));
      if (rest.starts_with(':')) {
        second = Number(rest, 1, 2);
        rest = rest.substr(std::min<size_t>(rest.size(), 3));
      }
    }
    base::Time time;
    if (!month || !day || !hour || !minute || !second || !(rest.empty() || rest == "Z")) {
      return std::nullopt;
    }
    exploded.month = *month;
    exploded.day_of_month = *day;
    exploded.hour = *hour;
    exploded.minute = *minute;
    exploded.second = *second;
    return base::Time::FromUTCExploded(exploded, &time) ? std::optional(time) : std::nullopt;
  }
  base::Time time;
  return base::Time::FromString(std::string(text).c_str(), &time) ? std::optional(time)
                                                                  : std::nullopt;
}

}  // namespace

bool PageLooks(base::TimeTicks last_look, base::TimeTicks now) {
  return !last_look.is_null() && now - last_look >= kLookEvery;
}

size_t Rules(std::string_view list) {
  size_t rules = 0;
  while (!list.empty()) {
    const size_t end = list.find('\n');
    const std::string_view line = base::TrimWhitespaceASCII(list.substr(0, end), base::TRIM_ALL);
    if (!line.empty() && !IsComment(line)) {
      ++rules;
    }
    list.remove_prefix(end == std::string_view::npos ? list.size() : end + 1);
  }
  return rules;
}

std::optional<base::Time> Updated(std::string_view list) {
  static constexpr std::array<std::string_view, 3> kNames = {
      "updated:", "last modified:", "last updated:"};
  while (!list.empty()) {
    const size_t end = list.find('\n');
    const std::string_view line = base::TrimWhitespaceASCII(list.substr(0, end), base::TRIM_ALL);
    list.remove_prefix(end == std::string_view::npos ? list.size() : end + 1);
    if (line.empty()) {
      continue;
    }
    if (!IsComment(line)) {
      return std::nullopt;
    }
    const std::string_view text = base::TrimWhitespaceASCII(line.substr(1), base::TRIM_LEADING);
    for (std::string_view name : kNames) {
      if (base::StartsWith(text, name, base::CompareCase::INSENSITIVE_ASCII)) {
        return ReadTime(base::TrimWhitespaceASCII(text.substr(name.size()), base::TRIM_ALL));
      }
    }
  }
  return std::nullopt;
}

bool Takes(std::string_view downloaded, std::optional<std::string_view> last_good) {
  const std::string_view start = base::TrimWhitespaceASCII(downloaded, base::TRIM_LEADING);
  if (start.starts_with('<') || downloaded.find('\0') != std::string_view::npos ||
      !base::IsStringUTF8(downloaded)) {
    return false;
  }
  const size_t rules = Rules(downloaded);
  if (rules == 0) {
    return false;
  }
  if (!last_good) {
    return true;
  }
  if (rules * 2 < Rules(*last_good)) {
    return false;
  }
  const std::optional<base::Time> updated = Updated(downloaded);
  const std::optional<base::Time> held = Updated(*last_good);
  return !updated || !held || *updated >= *held;
}

}  // namespace browser_core::ad_block_list_check
