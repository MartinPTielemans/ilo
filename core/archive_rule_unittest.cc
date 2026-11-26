#include "browser_core/archive_rule.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::archive_rule {
namespace {

TEST(ArchiveRule, AProfilesOwnTimeElseTheDefault) {
  EXPECT_EQ(FromPref(3600, base::Hours(12)), base::Hours(1));
  EXPECT_EQ(FromPref(0, base::Hours(12)), base::Hours(12));
}

TEST(ArchiveRule, NeverArchivesNothingPastThirtyDays) {
  const base::Time now = base::Time::Now();
  const std::optional<base::TimeDelta> never = FromPref(kNever, base::Hours(12));
  EXPECT_EQ(never, std::nullopt);
  // Thirty days is the longest choice; a tab unseen past it, and long past, is never due.
  for (base::TimeDelta unseen : {base::Days(30), base::Days(31), base::Days(365)}) {
    EXPECT_EQ(DueAt(now - unseen, never), std::nullopt);
  }
  // The thirty days choice takes a tab unseen that long.
  const std::optional<base::Time> due = DueAt(now - base::Days(31), FromPref(30 * 86400., {}));
  ASSERT_TRUE(due);
  EXPECT_LT(*due, now);
}

TEST(ArchiveRule, AnAgentsSpaceTakesNoTab) {
  const base::Time now = base::Time::Now();
  EXPECT_EQ(InSpace(false, base::Hours(1)), base::Hours(1));
  // A tab handed to an agent, unseen however long in its Space, is never due there.
  const std::optional<base::TimeDelta> agents = InSpace(true, base::Hours(1));
  EXPECT_EQ(agents, std::nullopt);
  EXPECT_EQ(DueAt(now - base::Days(365), agents), std::nullopt);
}

TEST(ArchiveRule, ATabBackWaitsItsWholeTimeAgain) {
  // Last seen 20 hours ago, then handed and back now, which counts as seen
  // (handed::UsersAgain): it's due 12 hours from now, not 8 hours ago.
  const base::Time back = base::Time::Now();
  const std::optional<base::TimeDelta> after = InSpace(false, base::Hours(12));
  const std::optional<base::Time> due = DueAt(back, after);
  ASSERT_TRUE(due);
  EXPECT_EQ(*due, back + base::Hours(12));
  EXPECT_GT(*due, back);
  EXPECT_LT(*DueAt(back - base::Hours(20), after), back);
}

}  // namespace
}  // namespace browser_core::archive_rule
