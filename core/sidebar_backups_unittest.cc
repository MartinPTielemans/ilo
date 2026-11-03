#include "browser_core/sidebar_backups.h"

#include <algorithm>
#include <iterator>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

// Noon by the user's clock, so a day's daylight saving change never moves one to another date.
base::Time Noon(int year, int month, int day) {
  base::Time::Exploded exploded = {.year = year, .month = month, .day_of_month = day, .hour = 12};
  base::Time time;
  EXPECT_TRUE(base::Time::FromLocalExploded(exploded, &time));
  return time;
}

// Monday 15 June 2026.
base::Time Now() {
  return Noon(2026, 6, 15);
}

// One a day, newest first, from |from| days ago to |to| days ago.
std::vector<base::Time> Daily(int from, int to) {
  std::vector<base::Time> times;
  for (int ago = from; ago <= to; ++ago) {
    times.push_back(Now() - base::Days(ago));
  }
  return times;
}

TEST(SidebarBackupsTest, TenAreKeptFromToday) {
  std::vector<base::Time> times;
  for (int hour = 0; hour < 12; ++hour) {
    times.push_back(Now() - base::Hours(hour));
  }
  const std::vector<base::Time> kept = BackupsKept(times, Now());
  EXPECT_EQ(kept, std::vector<base::Time>(times.begin(), times.begin() + 10));
}

TEST(SidebarBackupsTest, TheNewestOfEachOfTheTenDaysBeforeIsKept) {
  std::vector<base::Time> times;
  for (int ago = 1; ago <= 10; ++ago) {
    times.push_back(Now() - base::Days(ago) + base::Hours(3));
    times.push_back(Now() - base::Days(ago));
  }
  std::vector<base::Time> newest;
  for (size_t i = 0; i < times.size(); i += 2) {
    newest.push_back(times[i]);
  }
  EXPECT_EQ(BackupsKept(times, Now()), newest);
}

// Weeks start on Monday: from 11 to 31 days back that's Thursday 4 June, then the Sundays 31, 24
// and 17 May, whose weeks' older days go.
TEST(SidebarBackupsTest, TheNewestOfEachWeekIsKeptToAMonthBack) {
  const std::vector<base::Time> expected = {Noon(2026, 6, 4), Noon(2026, 5, 31),
                                            Noon(2026, 5, 24), Noon(2026, 5, 17)};
  EXPECT_EQ(BackupsKept(Daily(11, 31), Now()), expected);
}

// From 32 days back to a year back, the newest of each month: 14 May, then the last of each month
// to June 2025. Older ones go.
TEST(SidebarBackupsTest, TheNewestOfEachMonthIsKeptToAYearBack) {
  const std::vector<base::Time> kept = BackupsKept(Daily(32, 400), Now());
  ASSERT_EQ(kept.size(), 12u);
  EXPECT_EQ(kept.front(), Noon(2026, 5, 14));
  EXPECT_EQ(kept.back(), Noon(2025, 6, 30));
  for (size_t i = 1; i < kept.size(); ++i) {
    base::Time::Exploded day, next;
    kept[i].LocalExplode(&day);
    (kept[i] + base::Days(1)).LocalExplode(&next);
    EXPECT_NE(day.month, next.month) << "not the last of its month: " << kept[i];
  }
}

// Each kind is kept from the whole list at once, as BackUp prunes it.
TEST(SidebarBackupsTest, TheScheduleKeepsEachKindTogether) {
  std::vector<base::Time> times = {Now(), Now() - base::Hours(2)};
  std::ranges::copy(Daily(1, 400), std::back_inserter(times));
  // 2 today, 10 days, 4 weeks and 12 months.
  EXPECT_EQ(BackupsKept(times, Now()).size(), 28u);
}

// One from a clock set back counts as today's rather than going.
TEST(SidebarBackupsTest, OneFromTheFutureCountsAsToday) {
  const std::vector<base::Time> times = {Now() + base::Days(3), Now()};
  EXPECT_EQ(BackupsKept(times, Now()), times);
}

}  // namespace
}  // namespace browser_core
