#ifndef BROWSER_CORE_NOTIFICATIONS_H_
#define BROWSER_CORE_NOTIFICATIONS_H_

class Profile;

namespace browser_core {

// Watches |profile|'s notifications, once, and counts the ones their pages posted while no window
// showed their tabs, if the setting has the badges on.
void ObserveNotifications(Profile* profile);

// The favorite notification badge setting turned off (bc_set_favorite_notification_badges): every
// tab's unseen count goes, so no badge draws, and none counts again until it's on.
void ClearFavoriteNotificationCounts();

}  // namespace browser_core

#endif  // BROWSER_CORE_NOTIFICATIONS_H_
