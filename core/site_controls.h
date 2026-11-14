#ifndef BROWSER_CORE_SITE_CONTROLS_H_
#define BROWSER_CORE_SITE_CONTROLS_H_

#include <memory>
#include <string>

#include "base/memory/weak_ptr.h"
#include "base/scoped_observation.h"
#include "browser_core/public/browser_core.h"
#include "chrome/browser/extensions/extension_action_dispatcher.h"
#include "components/page_info/page_info.h"
#include "components/page_info/page_info_ui.h"
#include "content/public/browser/web_contents_observer.h"
#include "url/gurl.h"

class ChromePageInfoUiDelegate;
class Profile;

namespace browser_core {

// The site of the tab's page as the app's Site Controls show it, in place of Chrome's page info
// bubble (tab_site_controls): of the permissions Chrome's page info lists, those that differ for the
// site; its connection; whether it plays a video that can go Picture in Picture; and whether the ad
// blocker runs on it.
// One tab's at a time, while the app's panel shows, and meanwhile its window tells of every
// turned-on extension's action.
class SiteControls : public PageInfoUI,
                     public content::WebContentsObserver,
                     public extensions::ExtensionActionDispatcher::Observer {
 public:
  // See bc_tab_site_controls and the calls after it.
  static void Open(bc_id tab);
  static void Close();
  static void SetPermission(bc_id tab, int type, int value);
  static void Reset(bc_id tab);
  static void RemoveDevice(bc_id tab, size_t index);
  static void SetAdBlocking(bc_id tab, bool on);
  static void PictureInPicture(bc_id tab);
  static void ShowCertificate(bc_id tab);
  // Reads |tab|'s site again if the panel shows it: its page went from one engine to the other, or
  // the system's engine committed a page, which Chromium doesn't see.
  static void PageChanged(bc_id tab);
  // Reports the site's logins again if the panel shows a page of |profile|: they were read or
  // changed.
  static void LoginsChanged(Profile* profile);

  SiteControls(const SiteControls&) = delete;
  SiteControls& operator=(const SiteControls&) = delete;
  ~SiteControls() override;

 private:
  explicit SiteControls(content::WebContents* contents);

  // Reads the site of the page the tab is at now.
  void Read();

  // PageInfoUI:
  void SetPermissionInfo(const PermissionInfoList& permission_info_list,
                         ChosenObjectInfoList chosen_object_info_list) override;
  void SetIdentityInfo(const IdentityInfo& identity_info) override;

  // content::WebContentsObserver:
  void PrimaryPageChanged(content::Page& page) override;
  void MediaStartedPlaying(const MediaPlayerInfo& info, const content::MediaPlayerId& id) override;
  void MediaStoppedPlaying(const MediaPlayerInfo& info,
                           const content::MediaPlayerId& id,
                           MediaStoppedReason reason) override;
  void MediaPictureInPictureChanged(bool is_picture_in_picture) override;
  void WebContentsDestroyed() override;

  // extensions::ExtensionActionDispatcher::Observer, for the ad blocker's count, which its action's
  // badge holds:
  void OnExtensionActionUpdated(extensions::ExtensionAction* action,
                                content::WebContents* contents,
                                content::BrowserContext* context) override;
  void OnShuttingDown() override;

  // The browser's own ad blocker's count, which it sends as WatchNativeAdBlocked says.
  void NativeAdBlocked(int blocked);

  // Tells the app from a task of its own: PageInfo reports inside the app's calls that change a
  // permission, and a burst of changes is one call.
  void Changed();
  void Report();

  const bc_id tab_;
  // The window showing the tab as the panel opened, which tells of every extension's action while
  // the panel shows, as the panel lists them all.
  bc_id window_ = 0;
  std::unique_ptr<ChromePageInfoUiDelegate> ui_delegate_;
  // Null for a file or a browser page, which Chrome's page info shows no permissions for.
  std::unique_ptr<PageInfo> page_info_;
  PermissionInfoList permissions_;
  // The devices the site was given, as Chrome's page info lists them.
  ChosenObjectInfoList devices_;
  // The page's URL, Chromium's or the system page's.
  GURL url_;
  std::string site_;
  int connection_ = BC_CONNECTION_NOT_SECURE;
  std::u16string connection_summary_;
  // Whether the profile keeps the site for the system's engine, or else offers it.
  bool system_engine_ = false;
  bool system_engine_offered_ = false;
  bool reporting_ = false;
  // How many requests the browser's own ad blocker blocked on the page, -1 until it says.
  int native_blocked_ = -1;
  base::ScopedObservation<extensions::ExtensionActionDispatcher,
                          extensions::ExtensionActionDispatcher::Observer>
      actions_{this};
  base::WeakPtrFactory<SiteControls> weak_factory_{this};
};

}  // namespace browser_core

#endif  // BROWSER_CORE_SITE_CONTROLS_H_
