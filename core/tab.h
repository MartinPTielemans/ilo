#ifndef BROWSER_CORE_TAB_H_
#define BROWSER_CORE_TAB_H_

#include <memory>
#include <optional>
#include <string>
#include <vector>

#include "base/callback_list.h"
#include "base/functional/callback.h"
#include "base/functional/function_ref.h"
#include "base/memory/weak_ptr.h"
#include "base/scoped_observation.h"
#include "base/task/cancelable_task_tracker.h"
#include "base/time/time.h"
#include "browser_core/platform/system_page.h"
#include "browser_core/public/browser_core.h"
#include "browser_core/session.h"
#include "chrome/browser/media/webrtc/media_stream_capture_indicator.h"
#include "components/favicon/core/favicon_driver.h"
#include "components/favicon/core/favicon_driver_observer.h"
#include "components/infobars/core/infobar_manager.h"
#include "content/public/browser/navigation_controller.h"
#include "content/public/browser/web_contents_observer.h"
#include "mojo/public/cpp/bindings/receiver.h"
#include "services/media_session/public/mojom/media_session.mojom.h"
#include "third_party/skia/include/core/SkColor.h"
#include "ui/gfx/geometry/point_f.h"
#include "ui/gfx/image/image.h"
#include "ui/gfx/native_ui_types.h"
#include "url/gurl.h"

class BrowserWindowInterface;
class Profile;
namespace content {
class RenderWidgetHost;
struct DropData;
enum class PageLinkDrag;
}

namespace browser_core {

class Window;

// A tab's id, and what the app hears about it. A tab may have no page: one put back at launch and
// not yet shown, or one unloaded. It keeps its history then, and what its row shows, and gets its
// page back once a window shows it (Wake), as Chrome loads a restored tab once it's shown.
class Tab : public content::WebContentsObserver,
            public favicon::FaviconDriverObserver,
            public MediaStreamCaptureIndicator::Observer,
            public infobars::InfoBarManager::Observer,
            public media_session::mojom::MediaSessionObserver,
            public platform::SystemPage::Delegate {
 public:
  // |contents|'s id, given the first time it's asked for.
  static bc_id IdOf(content::WebContents* contents);
  // |contents|'s tab, or null if nothing asked for its id.
  static Tab* FromWebContents(content::WebContents* contents);
  // |id|'s tab, with its page or without, or null.
  static Tab* Get(bc_id id);
  // |id|'s page, or null, as for a tab without one.
  static content::WebContents* FromId(bc_id id);
  // Every tab, with its page or without.
  static std::vector<Tab*> All();
  // Whether |id| has its page loaded, which it hasn't without one, nor once unloaded in place by
  // Chrome, nor while opened behind and not yet shown.
  static bool Loaded(bc_id id);
  // The URL of the page |contents| is at, loaded or not, else of the one it's loading.
  static GURL PageUrl(content::WebContents* contents);
  // The hook for Chrome's status bubble (hooks.h): |source|'s page has the pointer on the link
  // |url|, or on none and it's empty. Tells the app, if |source| is a tab of one of the core's
  // windows.
  static void LinkTargetChanged(BrowserWindowInterface* browser,
                                content::WebContents* source,
                                const GURL& url);
  // The hook for a link dragged over a page (content::SetPageLinkDragHandler): tells the app of a
  // web link dragged over a tab's page, and answers whether the app takes it there.
  static bool LinkDragged(content::WebContents* contents,
                          content::PageLinkDrag phase,
                          const content::DropData& data,
                          const gfx::PointF& point);
  // Every tab's page, of the tabs that have one.
  static std::vector<content::WebContents*> Pages();
  // Stops every tab hearing Chrome's capture indicator, at a quit once the run loop is done: the
  // tabs outlive it, and it checks it has no one left to tell as it goes.
  static void StopHearingCaptures();
  // The view a window shows for |contents|'s page: its system page's, else its own.
  static gfx::NativeView PageViewOf(content::WebContents* contents);
  // Gives |contents|'s page the keyboard.
  static void FocusPage(content::WebContents* contents);
  // Asks the app, through |callback|, about a key going down in |tab|'s page, whose focused element
  // takes text if |editing|. Returns whether the app used it.
  static bool AskAppAboutKey(bool (*callback)(void*, bc_id, const bc_key*),
                             bc_id tab,
                             const input::NativeWebKeyboardEvent& event,
                             bool editing);
  // What the web names the key |event| is, as the app's shortcuts do, with its BC_MODIFIER_ flags
  // held in |modifiers|.
  static std::string KeyName(const input::NativeWebKeyboardEvent& event, uint32_t* modifiers);

  // A tab without its page, in |profile| and in no Space, as |saved| keeps it: its history, name,
  // icon, mute and when it was last seen. Tells the app its state, title and the favicon the
  // profile has for its URL. Returns its id, or 0 without history.
  static bc_id Restore(Profile* profile, const SavedTab& saved);

  Tab(const Tab&) = delete;
  Tab& operator=(const Tab&) = delete;
  ~Tab() override;

  bc_id id() const { return id_; }
  // Its profile, or null once that's gone.
  Profile* profile() const { return profile_.get(); }
  // The URL of the page it's at, loaded or not, else of the one it's loading.
  GURL Url() const;
  // Its history, with its name, icon and mute, as the session keeps it.
  SavedTab Save() const;

  // Lets go of the tab's page, keeping its history, name and icon, and its place: it's a tab without
  // its page (Wake). Chrome's tab strip deletes the page, so none of its unload handlers run.
  void Unload();
  // Whether Unload is deleting its page.
  bool unloading() const { return unloading_; }
  // For a tab without its page: gives it one in |window|'s Browser for its profile, at its history,
  // not shown, which loads once it is. Returns it, else null, and the tab stays without.
  content::WebContents* Wake(Window& window);
  // Closes a tab without its page, as one closes with its page: out of its Space or the favorites,
  // and the app hears it closed. Deletes it.
  void Close();

  // For a copy of a page with its history, as a duplicate: tells the app its page's URL, history
  // and title, and finds its favicon among the profile's, as the page doesn't load until shown.
  void Restored();

  // For a new tab opened behind, as by ⌘-click on a link: |load| waits until the tab is first
  // shown, and runs then as Chrome would have run it at once. Until then the tab has no page, and
  // the app hears of the URL and the site's favicon among the profile's.
  void LoadWhenShown(content::NavigationController::LoadURLParams load);
  // The load waiting for the tab to be shown, else null.
  const content::NavigationController::LoadURLParams* waiting() const { return waiting_.get(); }

  // Tells the app the tab's state from a task of its own, so a burst of changes is one call, and
  // none comes during the app's call that made them.
  void StateChanged();

  // The name the user gave the tab, which it shows for its title, or empty for its page's.
  const std::string& name() const { return name_; }
  // Names the tab and tells the app its title from a task of its own. Empty gives it back its
  // page's title.
  void Rename(const std::string& name);
  // For a copy: its name, which Restored tells the app.
  void set_name(const std::string& name) { name_ = name; }
  // Its name, else its page's title.
  std::string Title();
  // The emoji the user gave the tab to show in place of its page's favicon, or empty.
  const std::string& icon() const { return icon_; }
  // Gives the tab |icon|, and tells the app with its state. Empty gives it back its favicon.
  void SetIcon(const std::string& icon);
  // For a copy: its icon, which its first state report tells the app.
  void set_icon(const std::string& icon) { icon_ = icon; }

  // Mutes or unmutes the tab, as Chrome's speaker button does.
  void SetMuted(bool muted);
  // While another page shares a tab, has it share this tab's page instead, as Chrome's infobar's
  // Share this tab instead does.
  void ShareInstead();
  // Plays, pauses or skips the page's media, as bc_tab_media.
  void Media(int action);
  bool muted() const;

  // The page the system's engine shows in place of Chromium's (system_engine.h), else null. Its
  // WebContents stays, unloaded, with the history it had.
  platform::SystemPage* system_page() const { return system_page_.get(); }
  // Shows |url| in the system's engine, whose first load waits for the profile's session with the
  // site to come over from Chromium. Going back from its first page goes back to Chromium's.
  // False, doing nothing, if the tab may not.
  bool UseSystemEngine(const GURL& url);
  // Back to Chromium, which loads |url|, or if it's empty its page where it was.
  void UseChromium(const GURL& url = GURL());
  // Loads |url| as the user asked: in the tab's system page for the page's site or one the profile
  // keeps for it, else through |load|, which loads it in Chromium.
  void Load(const GURL& url, base::FunctionRef<void()> load);
  // Back, forward, reload and stop, as the app's buttons, in whichever engine shows the page.
  // Reload, as bc_tab_reload says, ends a hung page's renderer first.
  bool CanGoBack();
  bool CanGoForward();
  void GoBack();
  void GoForward();
  // The pages back from this one, or forward of it, as SavedTab keeps them, to the app's
  // tab_history with their favicons. None while a system page shows, whose history is WebKit's.
  void ListHistory(bool forward);
  // Goes to the page |offset| from this one, as ListHistory numbers them, if there's one there.
  void GoToOffset(int offset);
  void Reload(bool bypass_cache);
  void Stop();

  // The colour the tab's page shows until it draws its first frame, opaque. Each page view the tab
  // swaps in takes it too, which Chrome's views do on their own only off the Mac.
  void SetPageColor(SkColor color);

  // For g_browser_core_page_hung (//browser_core/hooks.h): |contents|'s page hasn't answered input
  // for Chrome's hang timeout. Its tab is BC_PAGE_HUNG until it answers, or until Wait or Reload.
  static void PageHung(content::WebContents* contents,
                       content::RenderWidgetHost* widget,
                       base::RepeatingClosure restart_hang_monitor);
  // For g_browser_core_offline_game (//browser_core/hooks.h): while a run of the offline page's
  // game is going, Chromium's auto-reload waits, and it loads the page once the run ends if the
  // network came back meanwhile, so a page coming back never throws a run away.
  static void GameRunning(content::WebContents* contents, bool running);
  // Gives a hung page Chrome's hang timeout again, as Chrome's Wait does.
  void Wait();
  // For a window that stopped showing the tab: a page whose renderer died is unloaded rather than
  // kept with nothing to show, so it loads again once it's shown.
  void LeftWindow();
  // Whether the page's renderer died while a window showed it, so its pane shows the app's card in
  // place of the page until it's loaded again.
  bool crashed() const { return crashed_; }

  // Where the user last pressed the mouse in the page, in points from the top-left of its view:
  // where a link it opens was clicked.
  gfx::PointF last_click() const { return last_click_; }
  // The page's last press, of the pointer or a key, held ⌘ (Ctrl off the Mac), which tells
  // ⌥⌘-click on a link from ⌥-click, as both ask the browser for a split view.
  bool last_press_held_command() const { return last_press_held_command_; }

  // When a window last stopped showing the tab, else when it opened, for the auto-archive and for
  // unloading tabs unseen a while.
  base::Time last_seen() const { return last_seen_; }
  void set_last_seen(base::Time time) { last_seen_ = time; }

  // Tells the app the page went full screen, once, as it does when the page's request returns
  // (DidToggleFullscreenModeForTab). On Windows the window resizes before that returns, and the
  // app lays the page out once, at the window's new size, only if it hears first.
  void EnteredFullScreen();

 private:
  Tab(bc_id id, Profile* profile);

  // Makes |contents| the tab's page.
  void Attach(content::WebContents* contents);
  // Tells the app the tab closed, as its page did or as Close closes it.
  void ReportClosed();
  // Finds the favicon the profile has for |url|, for a tab whose page hasn't loaded, as Chrome's
  // favicon driver finds one only for a page that has committed.
  void FindFavicon(const GURL& url);

  // content::WebContentsObserver:
  void DidStartNavigation(content::NavigationHandle* navigation) override;
  void DidFinishNavigation(content::NavigationHandle* navigation) override;
  void DidStartLoading() override;
  void DidStopLoading() override;
  void NavigationEntriesDeleted() override;
  void DidFinishLoad(content::RenderFrameHost* frame, const GURL& url) override;
  void RenderFrameHostChanged(content::RenderFrameHost* old_host,
                              content::RenderFrameHost* new_host) override;
  void RenderViewReady() override;
  void PrimaryMainFrameRenderProcessGone(base::TerminationStatus status) override;
  void OnRendererResponsive(content::RenderProcessHost* process) override;
  void TitleWasSet(content::NavigationEntry* entry) override;
  void DidToggleFullscreenModeForTab(bool entered_fullscreen, bool will_cause_resize) override;
  void DidUpdateAudioMutingState(bool muted) override;
  void DidGetUserInteraction(const blink::WebInputEvent& event) override;
  void OnVisibilityChanged(content::Visibility visibility) override;
  void MediaStartedPlaying(const MediaPlayerInfo& info, const content::MediaPlayerId& id) override;
  void WebContentsDestroyed() override;

  // platform::SystemPage::Delegate:
  void SystemPageChanged() override;
  void SystemPageCommitted() override;
  void SystemPageLoaded() override;
  void SystemPageLeft(const GURL& url) override;
  void SystemPageOpen(const GURL& url) override;
  bool SystemPageKey(const input::NativeWebKeyboardEvent& event,
                     bool before_page,
                     bool editing) override;
  void SystemPageClicked() override;
  void SystemPageOutputRestricted() override;

  // favicon::FaviconDriverObserver:
  void OnFaviconUpdated(favicon::FaviconDriver* driver,
                        NotificationIconType type,
                        const GURL& icon_url,
                        bool icon_url_changed,
                        const gfx::Image& image) override;

  // MediaStreamCaptureIndicator::Observer:
  void OnIsCapturingVideoChanged(content::WebContents* contents, bool capturing) override;
  void OnIsCapturingAudioChanged(content::WebContents* contents, bool capturing) override;
  void OnIsBeingMirroredChanged(content::WebContents* contents, bool mirrored) override;
  void OnIsCapturingTabChanged(content::WebContents* contents, bool capturing) override;
  void OnIsCapturingWindowChanged(content::WebContents* contents, bool capturing) override;
  void OnIsCapturingDisplayChanged(content::WebContents* contents, bool capturing) override;
  // Within those, for a change to this tab's page.
  void CaptureChanged(content::WebContents* contents);

  // infobars::InfoBarManager::Observer, for Chrome's tab sharing infobar, which ShareInstead acts
  // through and the app never shows:
  void OnInfoBarAdded(infobars::InfoBar* infobar) override;
  void OnInfoBarRemoved(infobars::InfoBar* infobar, bool animate) override;
  void OnInfoBarReplaced(infobars::InfoBar* old_infobar, infobars::InfoBar* new_infobar) override;
  void OnManagerWillBeDestroyed(infobars::InfoBarManager* manager) override;

  // media_session::mojom::MediaSessionObserver, for the media the sidebar's player shows:
  void MediaSessionInfoChanged(media_session::mojom::MediaSessionInfoPtr info) override;
  void MediaSessionMetadataChanged(
      const std::optional<media_session::MediaMetadata>& metadata) override;
  void MediaSessionActionsChanged(
      const std::vector<media_session::mojom::MediaSessionAction>& actions) override;
  void MediaSessionImagesChanged(
      const base::flat_map<media_session::mojom::MediaSessionImageType,
                           std::vector<media_session::MediaImage>>& images) override {}
  void MediaSessionPositionChanged(
      const std::optional<media_session::MediaPosition>& position) override {}

  // Tells the app the tab's state from a task of its own, as StateChanged does, for a change the
  // session doesn't keep.
  void ReportStateSoon();
  void ReportState();
  // Within ReportState, if the favicon the tab shows changed.
  void ReportFavicon(bool loading);
  void ReportTitle();
  void LoadWaiting();
  // Tells the app the link the pointer is on, if it's another than it was last told of.
  void ReportLinkTarget(const GURL& url);
  // A BC_PAGE_ value, for the app and agents.
  int Trouble() const;
  // Says the page is fine again if it hung, without telling the app.
  void ClearHang();
  // Lays out each window showing the tab, from a task of its own, so a crashed page's pane shows
  // the app's card, or the page again.
  void LayOutSoon();
  // Puts the core's error page on the pane's colour, and every other page back on Chrome's white.
  void ShowOnPageColor();
  // Drops the system page, and has the window show Chromium's view in its place.
  void DropSystemPage();
  // Takes back what the app was told of the system page's protected video, once it's gone or at
  // another site than |url|, an empty one for any.
  void ForgetOutputRestricted(const GURL& url = GURL());
  // Drops the system page for Chromium's, which |load| has load |url|.
  void HandBack(const GURL& url, base::FunctionRef<void()> load);
  // Tells the window, Site Controls and the app that the tab's page is now the other engine's.
  void EngineChanged();
  // Finds the favicon of the system page's URL among the profile's, as Chrome's driver can't.
  void FetchSystemPageFavicon();

  const bc_id id_;
  base::WeakPtr<Profile> profile_;
  // Without its page, its history and mute.
  std::optional<SavedTab> unloaded_;
  bool unloading_ = false;
  std::string name_;
  std::string icon_;
  base::Time last_seen_ = base::Time::Now();
  std::optional<SkColor> page_color_;
  bool state_report_posted_ = false;
  gfx::PointF last_click_;
  bool last_press_held_command_ = false;
  // Whether the app was last told the page is full screen.
  bool fullscreen_ = false;
  // Whether the app was last told the pointer is on a link.
  bool link_target_ = false;
  // The favicon the app was last told of.
  gfx::Image favicon_;
  base::ScopedObservation<favicon::FaviconDriver, favicon::FaviconDriverObserver>
      favicon_observation_{this};
  base::CallbackListSubscription audible_subscription_;
  base::ScopedObservation<MediaStreamCaptureIndicator, MediaStreamCaptureIndicator::Observer>
      capture_observation_{this};
  base::ScopedObservation<infobars::InfoBarManager, infobars::InfoBarManager::Observer>
      infobar_observation_{this};
  // Bound once the page first plays media, as only then has it a session to hear from.
  mojo::Receiver<media_session::mojom::MediaSessionObserver> media_observer_{this};
  std::unique_ptr<content::NavigationController::LoadURLParams> waiting_;
  bool crashed_ = false;
  // While the page is hung: Chrome's Wait, and the id of the renderer that hung, which Reload ends.
  base::RepeatingClosure restart_hang_monitor_;
  int hung_process_ = 0;
  // A reload waiting for the hung renderer Reload ended to go.
  std::optional<bool> reload_after_kill_;
  // That reload's page gets the keyboard back once its new renderer is ready.
  bool focus_when_ready_ = false;
  // Chromium's net error code while the page is its error page, else 0.
  int net_error_ = 0;
  // The page base colour the tab last gave its pages, if not Chrome's.
  std::optional<SkColor> shown_on_;
  std::unique_ptr<platform::SystemPage> system_page_;
  // How many system pages the tab has made, the last of them system_page_.
  int system_pages_ = 0;
  // The system page's favicon, PNG, and its title, which the app was last told of. None since a
  // document committed, so each document's title is told even when it's the one before's.
  std::vector<uint8_t> system_page_favicon_;
  std::optional<std::string> system_page_title_;
  // The system page the app was told its protected video is kept off the screen at, until the tab
  // goes to another site or back to Chromium.
  GURL output_restricted_at_;
  base::CancelableTaskTracker favicon_tracker_;
  base::WeakPtrFactory<Tab> weak_factory_{this};
};

}  // namespace browser_core

#endif  // BROWSER_CORE_TAB_H_
