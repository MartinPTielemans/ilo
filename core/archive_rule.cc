#include "browser_core/archive_rule.h"

namespace browser_core::archive_rule {

std::optional<base::TimeDelta> FromPref(double seconds, base::TimeDelta fallback) {
  if (seconds == kNever) {
    return std::nullopt;
  }
  return seconds > 0 ? base::Seconds(seconds) : fallback;
}

std::optional<base::Time> DueAt(base::Time last_seen, std::optional<base::TimeDelta> after) {
  if (!after) {
    return std::nullopt;
  }
  return last_seen + *after;
}

}  // namespace browser_core::archive_rule
