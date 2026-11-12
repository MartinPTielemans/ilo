#include "browser_core/sidebar_backups.h"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <set>
#include <utility>

namespace browser_core {

namespace {

constexpr size_t kBackupsToday = 10;
constexpr int64_t kDailyBackupsFor = 10;
constexpr int64_t kWeeklyBackupsFor = 31;
constexpr int64_t kMonthlyBackupsFor = 365;

// The days since 1970 to the user's date of |time|. Local midnight is within 14 hours of UTC's.
int64_t LocalDay(base::Time time) {
  return std::lround((time.LocalMidnight() - base::Time::UnixEpoch()).InSecondsF() / 86400);
}

}  // namespace

std::vector<base::Time> BackupsKept(const std::vector<base::Time>& times, base::Time now) {
  const int64_t today = LocalDay(now);
  size_t from_today = 0;
  // By how often they're kept, then which day, week or month.
  std::set<std::pair<int, int64_t>> spans;
  std::vector<base::Time> kept;
  for (base::Time time : times) {
    const int64_t day = LocalDay(time);
    // One from a clock set back counts as today's.
    const int64_t ago = std::max<int64_t>(today - day, 0);
    bool keep = false;
    if (ago == 0) {
      keep = from_today++ < kBackupsToday;
    } else if (ago <= kDailyBackupsFor) {
      keep = spans.insert({0, day}).second;
    } else if (ago <= kWeeklyBackupsFor) {
      // Weeks from Monday: 1 January 1970 was a Thursday.
      keep = spans.insert({1, (day + 3) / 7}).second;
    } else if (ago <= kMonthlyBackupsFor) {
      base::Time::Exploded exploded;
      time.LocalExplode(&exploded);
      keep = spans.insert({2, exploded.year * 12 + exploded.month}).second;
    }
    if (keep) {
      kept.push_back(time);
    }
  }
  return kept;
}

}  // namespace browser_core
