#ifndef BROWSER_CORE_NOTIFICATION_COUNT_H_
#define BROWSER_CORE_NOTIFICATION_COUNT_H_

#include <set>
#include <string>

namespace browser_core {

// The notifications a page has posted that the user hasn't seen, as its favorite's badge counts
// them (bc_tab_state's notifications): one a notification the page posted while its tab wasn't
// shown, cleared as a window shows the tab again. A notification Chromium holds for two displays
// (NotificationDisplayQueue re-displays a queued one as a blocker lifts) counts once, by its id.
// Apart from content so browser_core_unittests tests the rule alone.
class NotificationCount {
 public:
  // The count the app's badge draws.
  int count() const { return count_; }
  // The page posted the notification |id| while its tab wasn't shown. False if |id| was already
  // counted, so its caller reports its state only when the count changed.
  bool Posted(const std::string& id);
  // The window shows the tab again, so the user has seen them all. False if none were counted, so
  // its caller reports nothing.
  bool Shown();

 private:
  int count_ = 0;
  // The ids of the notifications counted, so one displayed twice counts once.
  std::set<std::string> ids_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_NOTIFICATION_COUNT_H_
