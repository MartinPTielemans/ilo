#include "browser_core/notification_count.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

// One notification a page posted while its tab wasn't shown counts, so its favorite's badge draws
// it (bc_tab_notifications), and the window showing the tab clears the count, as its notifications
// are seen.
TEST(NotificationCountTest, APostedNotificationCountsUntilTheTabIsShown) {
  NotificationCount count;
  EXPECT_EQ(count.count(), 0);
  EXPECT_TRUE(count.Posted("one"));
  EXPECT_EQ(count.count(), 1);
  EXPECT_TRUE(count.Shown());
  EXPECT_EQ(count.count(), 0);
}

// A notification Chromium displays twice (NotificationDisplayQueue re-displays a queued one as a
// blocker lifts) counts once, by its id.
TEST(NotificationCountTest, ANotificationDisplayedTwiceCountsOnce) {
  NotificationCount count;
  EXPECT_TRUE(count.Posted("one"));
  EXPECT_FALSE(count.Posted("one"));
  EXPECT_EQ(count.count(), 1);
}

// Two notifications count two, and the same id after the count is cleared, as a window showing the
// tab does, counts again: the user has seen the first, so its second display is unseen again.
TEST(NotificationCountTest, TheSameIdCountsAgainOnceSeen) {
  NotificationCount count;
  EXPECT_TRUE(count.Posted("one"));
  EXPECT_TRUE(count.Posted("two"));
  EXPECT_EQ(count.count(), 2);
  EXPECT_TRUE(count.Shown());
  EXPECT_TRUE(count.Posted("one"));
  EXPECT_EQ(count.count(), 1);
}

// A window showing a tab whose page posted nothing reports nothing: the app draws no badge either
// way, so the core tells it nothing (Tab::Shown's early return).
TEST(NotificationCountTest, ShownWithNothingCountedChangesNothing) {
  NotificationCount count;
  EXPECT_FALSE(count.Shown());
  EXPECT_EQ(count.count(), 0);
}

}  // namespace
}  // namespace browser_core
