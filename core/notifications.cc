// The unseen notifications a page posts, as its favorite's badge counts them: the core hears of
// each notification a page's document posted as Chromium displays it (NotificationDisplayService's
// observer), counts it on the tab holding the document if no window shows it, and clears the count
// as a window shows the tab again (Tab::Shown). A notification a worker posted has no document and
// counts for no tab, as Chrome's click opens none either. Not an agent's: its Space's tabs are the
// agent's.

#include "browser_core/notifications.h"

#include <string>

#include "base/memory/raw_ptr.h"
#include "base/supports_user_data.h"
#include "browser_core/agent_autofill.h"
#include "browser_core/session.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "chrome/browser/notifications/notification_common.h"
#include "chrome/browser/notifications/notification_display_service.h"
#include "chrome/browser/notifications/notification_display_service_factory.h"
#include "chrome/browser/profiles/profile.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

// Lives as long as the profile, watching its notifications: a notification's document can be held
// by the profile's own tabs alone, so each profile hears its own.
class Notifications : public base::SupportsUserData::Data,
                     public NotificationDisplayService::Observer {
 public:
  static constexpr char kKey[] = "browser_core::Notifications";

  Notifications(Profile* profile, NotificationDisplayService* service)
      : profile_(profile), service_(service) {
    service_->AddObserver(this);
  }

  ~Notifications() override {
    // The service tells of its end while it goes (OnNotificationDisplayServiceDestroyed), and the
    // profile tears its services down before its data, so this runs only while it lives.
    if (service_) {
      service_->RemoveObserver(this);
    }
  }

  // NotificationDisplayService::Observer:
  // The notification is displayed: counted on the tab holding the page that posted it, if no
  // window shows it. Chromium runs this before the notification reaches the desktop.
  void OnNotificationDisplayed(const message_center::Notification& notification,
                               const NotificationCommon::Metadata* const metadata) override {
    if (!UserSettings().favorite_notification_badges) {
      return;
    }
    // Only a page's own: an extension's, a download's, a call's and the rest are the browser's,
    // not a favorite's, and a worker's has no document to hold it.
    if (!metadata || metadata->type != NotificationHandler::Type::WEB_NON_PERSISTENT) {
      return;
    }
    const NonPersistentNotificationMetadata* page =
        NonPersistentNotificationMetadata::From(metadata);
    const GURL document = page ? page->document_url : GURL();
    if (!document.is_valid() || !document.SchemeIsHTTPOrHTTPS()) {
      return;
    }
    // The profile's tab whose page is at the document, if the page is in one. A page that posts a
    // notification and closes leaves none, as its notification does in Chrome once its document is
    // gone. An agent's tab is the agent's, and its page's notifications are its own to read there,
    // not the user's unseen ones.
    Tab* tab = Tab::HoldingDocument(document, profile_);
    if (!tab || IsAgentTab(tab->web_contents()) || Window::Showing(tab->id())) {
      return;
    }
    // The notification's id, as Chromium holds it for the document's page: the tab counts it once,
    // as Chromium can display one notification twice (NotificationDisplayQueue).
    tab->NotificationPosted(notification.id());
  }
  // A notification closed clears none of the count, which is of the ones unseen: the user closing
  // one without showing the tab has seen it, and the count reaches 0 as the tab is shown anyway.
  void OnNotificationClosed(const std::string& notification_id) override {}
  // The service is gone, so it watches nothing: leaves it, as the interface asks, and forgets it.
  void OnNotificationDisplayServiceDestroyed(NotificationDisplayService* service) override {
    service->RemoveObserver(this);
    service_ = nullptr;
  }

 private:
  const raw_ptr<Profile> profile_;
  raw_ptr<NotificationDisplayService> service_;
};

}  // namespace

void ObserveNotifications(Profile* profile) {
  if (!profile || profile->GetUserData(Notifications::kKey)) {
    return;
  }
  NotificationDisplayService* service =
      NotificationDisplayServiceFactory::GetForProfile(profile);
  if (!service) {
    return;
  }
  profile->SetUserData(
      Notifications::kKey, std::make_unique<Notifications>(profile, service));
}

void ClearFavoriteNotificationCounts() {
  for (Tab* tab : Tab::All()) {
    tab->Shown();
  }
}

}  // namespace browser_core
