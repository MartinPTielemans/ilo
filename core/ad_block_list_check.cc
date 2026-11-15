#include "browser_core/ad_block_list_check.h"

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

bool Takes(std::string_view downloaded, std::optional<std::string_view> last_good) {
  const std::string_view start = base::TrimWhitespaceASCII(downloaded, base::TRIM_LEADING);
  if (start.starts_with('<') || downloaded.find('\0') != std::string_view::npos ||
      !base::IsStringUTF8(downloaded)) {
    return false;
  }
  const size_t rules = Rules(downloaded);
  return rules > 0 && (!last_good || rules * 2 >= Rules(*last_good));
}

}  // namespace browser_core::ad_block_list_check
