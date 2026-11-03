#include "browser_core/tab.h"

#include <algorithm>
#include <cstdio>
#include <map>
#include <string>
#include <utility>

#include "base/auto_reset.h"
#include "base/barrier_callback.h"
#include "base/functional/bind.h"
#include "base/functional/callback_helpers.h"
#include "base/memory/ref_counted_memory.h"
#include "base/memory/ptr_util.h"
#include "base/no_destructor.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "browser_core/favorites.h"
#include "browser_core/held_keys.h"
#include "browser_core/host.h"
#include "browser_core/link_target.h"
#include "browser_core/password_offer.h"
#include "browser_core/platform/keyboard.h"
#include "browser_core/script_dialog.h"
#include "browser_core/session.h"
#include "browser_core/developer_mode.h"
#include "browser_core/drm_request.h"
#include "browser_core/site_controls.h"
#include "browser_core/space.h"
#include "browser_core/system_engine.h"
#include "browser_core/window.h"
#include "build/build_config.h"
#include "chrome/browser/favicon/favicon_service_factory.h"
#include "chrome/browser/history/history_service_factory.h"
#include "chrome/browser/lifetime/browser_shutdown.h"
#include "chrome/browser/media/webrtc/media_capture_devices_dispatcher.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/ui/browser_window/public/browser_window_interface.h"
#include "chrome/browser/ui/navigator/browser_navigator.h"
#include "chrome/browser/ui/navigator/browser_navigator_params.h"
#include "chrome/browser/ui/recently_audible_helper.h"
#include "chrome/browser/ui/tab_sharing/tab_sharing_infobar_delegate.h"
#include "chrome/browser/ui/tabs/tab_muted_utils.h"
#include "chrome/browser/ui/tabs/tab_strip_model.h"
#include "components/error_page/content/browser/net_error_auto_reloader.h"
#include "components/favicon/content/content_favicon_driver.h"
#include "components/favicon/core/favicon_service.h"
#include "components/favicon_base/favicon_types.h"
#include "components/history/core/browser/history_service.h"
#include "components/history/core/browser/history_types.h"
#include "components/infobars/content/content_infobar_manager.h"
#include "components/infobars/core/infobar.h"
#include "components/input/native_web_keyboard_event.h"
#include "components/keyed_service/core/service_access_type.h"
#include "components/tabs/public/tab_interface.h"
#include "content/public/browser/favicon_status.h"
#include "content/public/browser/media_session.h"
#include "content/public/browser/navigation_controller.h"
#include "content/public/browser/navigation_entry.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/reload_type.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/render_process_host.h"
#include "content/public/browser/render_widget_host.h"
#include "content/public/browser/render_widget_host_view.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_user_data.h"
#include "content/public/browser/web_contents_view_delegate.h"
#include "content/public/common/drop_data.h"
#include "content/public/common/result_codes.h"
#include "net/base/registry_controlled_domains/registry_controlled_domain.h"
#include "third_party/blink/public/common/input/web_input_event.h"
#include "third_party/blink/public/common/input/web_mouse_event.h"
#include "ui/base/page_transition_types.h"
#include "ui/base/window_open_disposition.h"
#include "ui/events/keycodes/dom/dom_key.h"
#include "ui/events/keycodes/dom/keycode_converter.h"
#include "ui/events/keycodes/keyboard_codes.h"
#include "ui/gfx/codec/png_codec.h"
#include "ui/gfx/image/image_skia.h"
#include "ui/gfx/image/image_skia_rep.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

// 16 points at 2×, as tab_favicon_changed's.
constexpr int kFaviconPixels = 32;

// A page ListHistory lists, until its favicon is found.
struct ListedPage {
  int offset;
  GURL url;
  std::string spec;
  std::string title;
  scoped_refptr<base::RefCountedMemory> favicon;
};

// The favicon found for the listed page at |at|.
struct ListedFavicon {
  size_t at;
  favicon_base::FaviconRawBitmapResult result;
};

void ReportHistory(bc_id tab,
                   bool forward,
                   std::vector<ListedPage> pages,
                   std::vector<ListedFavicon> favicons) {
  if (!Host().tab_history) {
    return;
  }
  for (const ListedFavicon& favicon : favicons) {
    if (favicon.result.is_valid()) {
      pages[favicon.at].favicon = favicon.result.bitmap_data;
    }
  }
  std::vector<bc_history_entry> entries;
  for (const ListedPage& page : pages) {
    entries.push_back({page.offset, page.spec.c_str(), page.title.c_str(),
                       page.favicon ? page.favicon->data() : nullptr,
                       page.favicon ? page.favicon->size() : 0});
  }
  Host().tab_history(Host().context, tab, forward, entries.data(), entries.size());
}

// Set once StopHearingCaptures has run, after which no tab starts hearing them.
bool& CapturesStopped() {
  static bool stopped = false;
  return stopped;
}

std::map<bc_id, std::unique_ptr<Tab>>& Tabs() {
  static base::NoDestructor<std::map<bc_id, std::unique_ptr<Tab>>> tabs;
  return *tabs;
}

// The line check.sh times a page's full screen from, on the uptime clock the app's and the host's
// lines read.
void LogFullScreenAsked(bool entered) {
  fprintf(stderr, "browser: full screen %s asked at %.1f ms\n", entered ? "enter" : "exit",
          base::TimeTicks::Now().since_origin().InMillisecondsF());
}

// Whether a renderer that went with |status| died, rather than ending as asked, as it does when
// the core closes a page on purpose.
bool Died(base::TerminationStatus status) {
  switch (status) {
    case base::TERMINATION_STATUS_ABNORMAL_TERMINATION:
    case base::TERMINATION_STATUS_PROCESS_WAS_KILLED:
    case base::TERMINATION_STATUS_PROCESS_CRASHED:
    case base::TERMINATION_STATUS_LAUNCH_FAILED:
    case base::TERMINATION_STATUS_OOM:
    case base::TERMINATION_STATUS_EVICTED_FOR_MEMORY:
      return true;
    default:
      return false;
  }
}

// The tab Wake is giving a page, which takes the first page asked for its id meanwhile: the core
// may hear of the page as Chrome makes it, before Wake does.
Tab* g_waking = nullptr;

// Ties a page to its tab, which outlives it once unloaded.
class TabLink : public content::WebContentsUserData<TabLink> {
 public:
  bc_id id() const { return id_; }

 private:
  friend class content::WebContentsUserData<TabLink>;

  TabLink(content::WebContents* contents, bc_id id)
      : content::WebContentsUserData<TabLink>(*contents), id_(id) {}

  const bc_id id_;

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

WEB_CONTENTS_USER_DATA_KEY_IMPL(TabLink);

// Forgets |id|, and deletes its tab once the caller, which may be the tab, has returned.
void Forget(bc_id id) {
  auto it = Tabs().find(id);
  if (it == Tabs().end()) {
    return;
  }
  std::unique_ptr<Tab> tab = std::move(it->second);
  Tabs().erase(it);
  base::SingleThreadTaskRunner::GetCurrentDefault()->DeleteSoon(FROM_HERE, std::move(tab));
}

// Chrome's tab sharing infobar on |contents|, which every tab has while a page shares a tab, if
// it offers Share this tab instead: the app's controls stand for the infobar, which never shows.
TabSharingInfoBarDelegate* ShareInsteadInfoBar(content::WebContents* contents) {
  auto* manager = contents ? infobars::ContentInfoBarManager::FromWebContents(contents) : nullptr;
  if (!manager) {
    return nullptr;
  }
  for (infobars::InfoBar* infobar : manager->infobars()) {
    if (infobar->delegate()->GetIdentifier() !=
        infobars::InfoBarDelegate::TAB_SHARING_INFOBAR_DELEGATE) {
      continue;
    }
    auto* sharing = static_cast<TabSharingInfoBarDelegate*>(infobar->delegate());
    constexpr auto kInstead = TabSharingInfoBarDelegate::kShareThisTabInstead;
    return (sharing->GetButtons() & kInstead) && sharing->IsButtonEnabled(kInstead) ? sharing
                                                                                    : nullptr;
  }
  return nullptr;
}

}  // namespace

// static
bc_id Tab::IdOf(content::WebContents* contents) {
  if (const TabLink* link = TabLink::FromWebContents(contents)) {
    return link->id();
  }
  if (g_waking && !g_waking->web_contents()) {
    g_waking->Attach(contents);
    return g_waking->id_;
  }
  const bc_id id = NewId();
  Tab* tab = (Tabs()[id] = base::WrapUnique(
                  new Tab(id, Profile::FromBrowserContext(contents->GetBrowserContext()))))
                 .get();
  tab->Attach(contents);
  return id;
}

// static
Tab* Tab::FromWebContents(content::WebContents* contents) {
  const TabLink* link = contents ? TabLink::FromWebContents(contents) : nullptr;
  return link ? Get(link->id()) : nullptr;
}

// static
Tab* Tab::Get(bc_id id) {
  auto it = Tabs().find(id);
  return it == Tabs().end() ? nullptr : it->second.get();
}

// static
content::WebContents* Tab::FromId(bc_id id) {
  const Tab* tab = Get(id);
  return tab ? tab->web_contents() : nullptr;
}

// static
void Tab::LinkTargetChanged(BrowserWindowInterface* browser,
                            content::WebContents* source,
                            const GURL& url) {
  // Other Browsers, such as a DevTools window's, aren't the app's.
  if (Window::ForBrowser(browser)) {
    Get(IdOf(source))->ReportLinkTarget(url);
  }
}

// static
bool Tab::LinkDragged(content::WebContents* contents,
                      content::PageLinkDrag phase,
                      const content::DropData& data,
                      const gfx::PointF& point) {
  Tab* tab = FromWebContents(contents);
  if (!tab || !Host().tab_link_dragged) {
    return false;
  }
  if (phase == content::PageLinkDrag::kLeft) {
    Host().tab_link_dragged(Host().context, tab->id_, BC_LINK_DRAG_LEFT, "", 0, 0);
    return false;
  }
  // Only a web page's link opens in a pane: one that isn't is left to the page.
  if (data.url_infos.empty() || !data.url_infos.front().url.SchemeIsHTTPOrHTTPS()) {
    return false;
  }
  const int dragged =
      phase == content::PageLinkDrag::kDrop ? BC_LINK_DRAG_DROP : BC_LINK_DRAG_OVER;
  return Host().tab_link_dragged(Host().context, tab->id_, dragged,
                                 data.url_infos.front().url.spec().c_str(), point.x(), point.y());
}

// static
std::vector<Tab*> Tab::All() {
  std::vector<Tab*> all;
  for (auto& [id, tab] : Tabs()) {
    all.push_back(tab.get());
  }
  return all;
}

// static
bc_id Tab::Restore(Profile* profile, const SavedTab& saved) {
  if (!profile || saved.history.empty()) {
    return 0;
  }
  const bc_id id = NewId();
  Tab* tab = (Tabs()[id] = base::WrapUnique(new Tab(id, profile))).get();
  // Only what the page would keep: the rest of |saved| is its place, which the caller sees to.
  SavedTab& unloaded = tab->unloaded_.emplace();
  unloaded.history = saved.history;
  unloaded.current = std::clamp(saved.current, 0, static_cast<int>(saved.history.size()) - 1);
  unloaded.muted = saved.muted;
  tab->name_ = saved.name;
  tab->icon_ = saved.icon;
  if (!saved.last_seen.is_null()) {
    tab->last_seen_ = saved.last_seen;
  }
  // check.sh waits for this line, an error since our builds print no warnings.
  LOG(ERROR) << "browser: tab " << id << " without its page";
  tab->StateChanged();
  // Posted, as the state is, so the app hears of it once its call has returned.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&Tab::ReportTitle, tab->weak_factory_.GetWeakPtr()));
  tab->FindFavicon(tab->Url());
  return id;
}

Tab::Tab(bc_id id, Profile* profile) : id_(id), profile_(profile->GetWeakPtr()) {}

Tab::~Tab() = default;

void Tab::Attach(content::WebContents* contents) {
  Observe(contents);
  TabLink::CreateForWebContents(contents, id_);
  // Chrome's tab helpers make the driver as the tab is made, before anything asks for its id.
  if (auto* driver = favicon::ContentFaviconDriver::FromWebContents(contents)) {
    favicon_observation_.Observe(driver);
  }
  // It keeps a tab audible 2 s after its sound stops, so the speaker doesn't flicker between
  // sounds.
  if (auto* audible = RecentlyAudibleHelper::FromWebContents(contents)) {
    audible_subscription_ = audible->RegisterRecentlyAudibleChangedCallback(
        base::IgnoreArgs<bool>(base::BindRepeating(&Tab::ReportStateSoon, base::Unretained(this))));
  }
  // Its page's camera, microphone and sharing, as Chrome's tab strip marks them. Not once quitting,
  // as a page's popup can still come after StopHearingCaptures.
  if (!capture_observation_.IsObserving() && !CapturesStopped() &&
      !browser_shutdown::HasShutdownStarted()) {
    capture_observation_.Observe(
        MediaCaptureDevicesDispatcher::GetInstance()->GetMediaStreamCaptureIndicator().get());
  }
  // Another page's session is heard from once it plays.
  media_observer_.reset();
  // Chrome's tab sharing infobar comes and goes on it as a page starts and stops sharing a tab.
  infobar_observation_.Reset();
  if (auto* infobars = infobars::ContentInfoBarManager::FromWebContents(contents)) {
    infobar_observation_.Observe(infobars);
  }
  // Its page's alert(), confirm() and prompt() ask with the app's dialog.
  ScriptDialogs::Attach(contents);
  // A tab is often made with its first page already on its way, before anything observes it, so
  // the app hears it's loading now rather than once the page commits.
  ReportStateSoon();
}

// static
std::vector<content::WebContents*> Tab::Pages() {
  std::vector<content::WebContents*> pages;
  for (const auto& [id, tab] : Tabs()) {
    if (content::WebContents* contents = tab->web_contents()) {
      pages.push_back(contents);
    }
  }
  return pages;
}

// static
void Tab::StopHearingCaptures() {
  CapturesStopped() = true;
  for (const auto& [id, tab] : Tabs()) {
    tab->capture_observation_.Reset();
  }
}

// static
bool Tab::Loaded(bc_id id) {
  content::WebContents* contents = FromId(id);
  if (!contents) {
    return false;
  }
  const Tab* tab = FromWebContents(contents);
  return tab->system_page_ || (!contents->GetController().NeedsReload() && !tab->waiting_);
}

// static
GURL Tab::PageUrl(content::WebContents* contents) {
  if (const Tab* tab = FromWebContents(contents); tab && tab->system_page_) {
    return tab->system_page_->url();
  }
  if (const Tab* tab = FromWebContents(contents); tab && tab->waiting_) {
    return tab->waiting_->url;
  }
  // A tab starts with an entry for no page, until its first page commits.
  content::NavigationEntry* entry = contents->GetController().GetLastCommittedEntry();
  return entry && !entry->IsInitialEntry() ? entry->GetURL() : contents->GetVisibleURL();
}

// static
gfx::NativeView Tab::PageViewOf(content::WebContents* contents) {
  const Tab* tab = FromWebContents(contents);
  return tab && tab->system_page_ ? tab->system_page_->view() : contents->GetNativeView();
}

// static
void Tab::FocusPage(content::WebContents* contents) {
  if (Tab* tab = FromWebContents(contents); tab && tab->system_page_) {
    tab->system_page_->Focus();
  } else {
    contents->Focus();
  }
}

// static
bool Tab::AskAppAboutKey(bool (*callback)(void*, bc_id, const bc_key*),
                         bc_id tab,
                         const input::NativeWebKeyboardEvent& event,
                         bool editing) {
  if (!callback || event.GetType() != blink::WebInputEvent::Type::kRawKeyDown ||
      event.skip_if_unhandled) {
    return false;
  }
  uint32_t modifiers = 0;
  const std::string name = KeyName(event, &modifiers);
  // ⌘'s shortcuts, which are Ctrl's off the Mac, by the Latin layout's key, chosen by the app. Read
  // only with it held, as every key the page gets comes here.
  const int command =
      BUILDFLAG(IS_MAC) ? blink::WebInputEvent::kMetaKey : blink::WebInputEvent::kControlKey;
  const std::string latin =
      (event.GetModifiers() & command) ? platform::LatinKey(event) : std::string();
  const bc_key key = {name.c_str(), modifiers, editing, latin.c_str()};
  // A shortcut that opens a field of the app's holds the page's keys after it (bc_hold_keys).
  auto asking = held_keys::AskingAbout(tab, event);
  const bool used = callback(Host().context, tab, &key);
  // A key the page didn't keep opens a field the keys after it are held for, or nothing does.
  if (callback == Host().tab_key_unhandled) {
    held_keys::Answered(tab, event, used);
  }
  return used;
}

// static
std::string Tab::KeyName(const input::NativeWebKeyboardEvent& event, uint32_t* modifiers) {
  std::string name = ui::KeycodeConverter::DomKeyToKeyString(ui::DomKey(event.dom_key));
  const int held = event.GetModifiers();
  // With ⌥ held, a Mac's key is the character ⌥ types with it, such as ⌥I's "ˆ", which no shortcut
  // names. A letter's or digit's shortcut is its key's, as Chrome's are.
  const int code = event.windows_key_code;
  if ((held & blink::WebInputEvent::kAltKey) &&
      ((code >= ui::VKEY_A && code <= ui::VKEY_Z) || (code >= ui::VKEY_0 && code <= ui::VKEY_9))) {
    name = std::string(1, static_cast<char>(code));
  }
  *modifiers = 0;
  for (auto [web, flag] : {std::pair{blink::WebInputEvent::kShiftKey, BC_MODIFIER_SHIFT},
                           std::pair{blink::WebInputEvent::kControlKey, BC_MODIFIER_CONTROL},
                           std::pair{blink::WebInputEvent::kAltKey, BC_MODIFIER_ALT},
                           std::pair{blink::WebInputEvent::kMetaKey, BC_MODIFIER_META}}) {
    if (held & web) {
      *modifiers |= flag;
    }
  }
  return name;
}

GURL Tab::Url() const {
  if (web_contents()) {
    return PageUrl(web_contents());
  }
  return unloaded_ ? unloaded_->history[unloaded_->current].virtual_url() : GURL();
}

SavedTab Tab::Save() const {
  SavedTab saved = web_contents() ? SaveTab(*web_contents()) : unloaded_.value_or(SavedTab());
  saved.name = name_;
  saved.icon = icon_;
  saved.muted = muted();
  return saved;
}

void Tab::Unload() {
  content::WebContents* contents = web_contents();
  tabs::TabInterface* in = contents ? tabs::TabInterface::MaybeGetFromContents(contents) : nullptr;
  // One opened behind and not yet shown has no page to let go of, and would lose the load it waits
  // for.
  if (!in || waiting_) {
    return;
  }
  TabStripModel* tabs = in->GetBrowserWindowInterface()->GetTabStripModel();
  // Quitting or closing its window, the page closes with the tab, which the session keeps from it.
  if (browser_shutdown::IsTryingToQuit() || tabs->closing_all()) {
    return;
  }
  // A system page's URL is its history's last entry, so it loads again once shown, in Chromium,
  // which hands a site the engine is kept for back to it.
  SavedTab saved = SaveTab(*contents);
  if (saved.history.empty()) {
    return;
  }
  unloaded_.emplace();
  unloaded_->history = std::move(saved.history);
  unloaded_->current = saved.current;
  unloaded_->muted = muted();
  if (system_page_) {
    DropSystemPage();
  }
  // The next page starts with nothing wrong, and on Chrome's white.
  crashed_ = false;
  ClearHang();
  reload_after_kill_.reset();
  focus_when_ready_ = false;
  net_error_ = 0;
  shown_on_.reset();
  {
    base::AutoReset<bool> unloading(&unloading_, true);
    tabs->DetachAndDeleteWebContentsAt(tabs->GetIndexOfWebContents(contents));
  }
  LOG(ERROR) << "browser: tab " << id_ << " without its page";
  StateChanged();
}

content::WebContents* Tab::Wake(Window& window) {
  if (web_contents() || !unloaded_ || !profile()) {
    return web_contents();
  }
  content::WebContents* contents = nullptr;
  {
    base::AutoReset<Tab*> waking(&g_waking, this);
    contents = window.RestoreTab(profile(), unloaded_->history, unloaded_->current);
  }
  if (!contents) {
    return nullptr;
  }
  if (!web_contents()) {
    Attach(contents);
  }
  DCHECK_EQ(web_contents(), contents);
  const bool muted = std::exchange(unloaded_, std::nullopt)->muted;
  if (muted) {
    SetMuted(true);
  }
  LOG(ERROR) << "browser: tab " << id_ << " has its page";
  Restored();
  return contents;
}

void Tab::Close() {
  if (web_contents()) {
    return;
  }
  ReportClosed();
  Forget(id_);
}

void Tab::ReportClosed() {
  Window::OnTabClosing(id_);
  if (Host().tab_closed) {
    Host().tab_closed(Host().context, id_);
  }
}

void Tab::Restored() {
  StateChanged();
  // Posted, as the state is, so the app hears of it once its call has returned.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&Tab::ReportTitle, weak_factory_.GetWeakPtr()));
  // Pages don't load until shown, and Chrome finds a favicon once a page commits.
  if (auto* driver = favicon::ContentFaviconDriver::FromWebContents(web_contents())) {
    driver->FetchFavicon(web_contents()->GetController().GetLastCommittedEntry()->GetURL(),
                         /*is_same_document=*/false);
  }
}

void Tab::LoadWhenShown(content::NavigationController::LoadURLParams load) {
  waiting_ = std::make_unique<content::NavigationController::LoadURLParams>(std::move(load));
  StateChanged();
  FindFavicon(waiting_->url);
}

void Tab::FindFavicon(const GURL& url) {
  favicon::FaviconService* service =
      profile() ? FaviconServiceFactory::GetForProfile(profile(), ServiceAccessType::EXPLICIT_ACCESS)
                : nullptr;
  if (!service) {
    return;
  }
  service->GetRawFaviconForPageURL(
      url, {favicon_base::IconType::kFavicon}, kFaviconPixels,
      /*fallback_to_host=*/true,
      base::BindOnce(
          [](base::WeakPtr<Tab> tab, const favicon_base::FaviconRawBitmapResult& result) {
            // Once the page loads, its favicon comes from Chrome.
            if (tab && !Loaded(tab->id_) && result.is_valid() && Host().tab_favicon_changed) {
              Host().tab_favicon_changed(Host().context, tab->id_, result.bitmap_data->data(),
                                         result.bitmap_data->size());
            }
          },
          weak_factory_.GetWeakPtr()),
      &favicon_tracker_);
}

void Tab::LoadWaiting() {
  if (!waiting_) {
    return;
  }
  const std::unique_ptr<content::NavigationController::LoadURLParams> load = std::move(waiting_);
  web_contents()->GetController().LoadURLWithParams(*load);
  StateChanged();
}

void Tab::SetMuted(bool muted) {
  if (!web_contents()) {
    if (unloaded_) {
      unloaded_->muted = muted;
    }
    ReportStateSoon();
    return;
  }
  // The reason extensions read, as for Chrome's speaker button. Chromium's page keeps it for when
  // the tab goes back there.
  SetTabAudioMuted(web_contents(), muted, TabMutedReason::kAudioIndicator, std::string());
  if (system_page_) {
    system_page_->SetMuted(muted);
  }
}

bool Tab::UseSystemEngine(const GURL& url) {
  if (!url.SchemeIsHTTPOrHTTPS() || !system_engine::Allowed(id_)) {
    return false;
  }
  if (system_page_) {
    system_page_->Load(url);
    return true;
  }
  Profile* profile = Profile::FromBrowserContext(web_contents()->GetBrowserContext());
  std::unique_ptr<platform::SystemPage> page =
      platform::SystemPage::Create(system_engine::StoreOf(profile), url, this);
  if (!page) {
    return false;
  }
  waiting_.reset();
  system_engine::StoreMade(profile);
  // The tab has its page before Chromium's goes, so the renderer ending reads as the switch.
  system_page_ = std::move(page);
  if (web_contents()->IsAudioMuted()) {
    system_page_->SetMuted(true);
  }
  // Chromium's page goes, keeping its history, as Unload has it go. A tab whose first page this
  // is has none.
  content::NavigationController& history = web_contents()->GetController();
  content::NavigationEntry* entry = history.GetLastCommittedEntry();
  if (entry && !entry->IsInitialEntry() && !history.NeedsReload()) {
    web_contents()->Discard(base::DoNothing());
  }
  favicon_ = gfx::Image();
  // The note offered this, and its question is answered.
  ForgetMediaUnsupported(web_contents());
  EngineChanged();
  // The user's session with the site comes too, before the page's first load (decision 1).
  system_engine::CopyToSystemEngine(
      profile, url,
      base::BindOnce(
          [](base::WeakPtr<Tab> tab, int page, platform::StorageByOrigin storage) {
            if (tab && tab->system_page_ && tab->system_pages_ == page) {
              tab->system_page_->Start(std::move(storage));
            }
          },
          weak_factory_.GetWeakPtr(), ++system_pages_));
  return true;
}

void Tab::UseChromium(const GURL& url) {
  if (!system_page_) {
    return;
  }
  HandBack(url, [&] {
    if (url.is_valid()) {
      web_contents()->GetController().LoadURL(url, content::Referrer(), ui::PAGE_TRANSITION_LINK,
                                              std::string());
    }
  });
}

void Tab::Load(const GURL& url, base::FunctionRef<void()> load) {
  if (!system_page_) {
    load();
    return;
  }
  Profile* profile = Profile::FromBrowserContext(web_contents()->GetBrowserContext());
  if (url.SchemeIsHTTPOrHTTPS() &&
      (net::registry_controlled_domains::SameDomainOrHost(
           url, system_page_->url(), net::registry_controlled_domains::INCLUDE_PRIVATE_REGISTRIES) ||
       system_engine::Remembered(profile, url))) {
    system_page_->Load(url);
    return;
  }
  HandBack(url, load);
}

bool Tab::CanGoBack() {
  content::NavigationController& history = web_contents()->GetController();
  if (!system_page_) {
    return history.CanGoBack();
  }
  if (system_page_->can_go_back()) {
    return true;
  }
  // From the system page's first, back to Chromium's page, or past it if it's at a site the engine
  // is kept for, which would come straight back.
  content::NavigationEntry* entry = history.GetLastCommittedEntry();
  if (!entry || entry->IsInitialEntry()) {
    return false;
  }
  Profile* profile = Profile::FromBrowserContext(web_contents()->GetBrowserContext());
  return !system_engine::Remembered(profile, entry->GetURL()) || history.CanGoBack();
}

bool Tab::CanGoForward() {
  return system_page_ ? system_page_->can_go_forward()
                      : web_contents()->GetController().CanGoForward();
}

void Tab::GoBack() {
  content::NavigationController& history = web_contents()->GetController();
  if (system_page_ && system_page_->can_go_back()) {
    system_page_->GoBack();
    return;
  }
  if (!CanGoBack()) {
    return;
  }
  if (!system_page_) {
    history.GoBack();
    return;
  }
  Profile* profile = Profile::FromBrowserContext(web_contents()->GetBrowserContext());
  const bool past = system_engine::Remembered(profile, history.GetLastCommittedEntry()->GetURL());
  HandBack(history.GetEntryAtOffset(past ? -1 : 0)->GetURL(), [&] {
    if (past) {
      history.GoBack();
    } else {
      history.SetNeedsReload();
      history.LoadIfNecessary();
    }
  });
}

void Tab::GoForward() {
  if (!CanGoForward()) {
    return;
  }
  if (system_page_) {
    system_page_->GoForward();
  } else {
    web_contents()->GetController().GoForward();
  }
}

void Tab::ListHistory(bool forward) {
  std::vector<ListedPage> pages;
  if (!system_page_) {
    const SavedTab saved = Save();
    const int step = forward ? 1 : -1;
    for (int i = saved.current + step; i >= 0 && i < static_cast<int>(saved.history.size());
         i += step) {
      const sessions::SerializedNavigationEntry& entry = saved.history[i];
      pages.push_back({i - saved.current, entry.virtual_url(), entry.virtual_url().spec(),
                       base::UTF16ToUTF8(entry.title()), nullptr});
    }
  }
  favicon::FaviconService* service =
      profile() ? FaviconServiceFactory::GetForProfile(profile(), ServiceAccessType::EXPLICIT_ACCESS)
                : nullptr;
  if (pages.empty() || !service) {
    // Posted, as the service's answers are, so the app hears of it once its call has returned.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&ReportHistory, id_, forward, std::move(pages),
                                  std::vector<ListedFavicon>()));
    return;
  }
  std::vector<GURL> urls;
  for (const ListedPage& page : pages) {
    urls.push_back(page.url);
  }
  // One report once every page's favicon is in. A tab that closes first cancels them, and none.
  base::RepeatingCallback<void(ListedFavicon)> found = base::BarrierCallback<ListedFavicon>(
      urls.size(), base::BindOnce(&ReportHistory, id_, forward, std::move(pages)));
  for (size_t at = 0; at < urls.size(); ++at) {
    service->GetRawFaviconForPageURL(
        urls[at], {favicon_base::IconType::kFavicon}, kFaviconPixels, /*fallback_to_host=*/true,
        base::BindOnce(
            [](base::RepeatingCallback<void(ListedFavicon)> found, size_t at,
               const favicon_base::FaviconRawBitmapResult& result) { found.Run({at, result}); },
            found, at),
        &favicon_tracker_);
  }
}

void Tab::GoToOffset(int offset) {
  if (system_page_ || !web_contents() || offset == 0) {
    return;
  }
  content::NavigationController& history = web_contents()->GetController();
  if (history.CanGoToOffset(offset)) {
    history.GoToOffset(offset);
  }
}

void Tab::Stop() {
  if (system_page_) {
    system_page_->Stop();
  } else {
    web_contents()->Stop();
  }
}

void Tab::DropSystemPage() {
  system_page_.reset();
  ForgetOutputRestricted();
  favicon_ = gfx::Image();
  system_page_favicon_.clear();
  system_page_title_.reset();
  EngineChanged();
}

void Tab::HandBack(const GURL& url, base::FunctionRef<void()> load) {
  // check.sh reads this line.
  fprintf(stderr, "browser: tab %llu back to Chromium for %s\n",
          static_cast<unsigned long long>(id_), url.spec().c_str());
  // Chromium's page starts its load before its view is shown again. Shown with none under way, it
  // reloads the page it kept, which at a site kept for the system's engine hands the tab straight
  // back there in place of the load.
  system_page_.reset();
  load();
  DropSystemPage();
}

void Tab::EngineChanged() {
  // The window shows the other engine's view in the pane.
  Window::PageViewChanged(id_);
  SiteControls::PageChanged(id_);
  StateChanged();
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&Tab::ReportTitle, weak_factory_.GetWeakPtr()));
}

void Tab::FetchSystemPageFavicon() {
  favicon::FaviconService* service = FaviconServiceFactory::GetForProfile(
      Profile::FromBrowserContext(web_contents()->GetBrowserContext()),
      ServiceAccessType::EXPLICIT_ACCESS);
  if (!service || !system_page_) {
    return;
  }
  // WebKit keeps its pages' favicons to itself, so the tab shows the one Chromium knows for the
  // page, else for its site, else none.
  service->GetRawFaviconForPageURL(
      system_page_->url(), {favicon_base::IconType::kFavicon}, kFaviconPixels,
      /*fallback_to_host=*/true,
      base::BindOnce(
          [](base::WeakPtr<Tab> tab, const favicon_base::FaviconRawBitmapResult& result) {
            if (!tab || !tab->system_page_ || !Host().tab_favicon_changed) {
              return;
            }
            std::vector<uint8_t> png;
            if (result.is_valid()) {
              png.assign(result.bitmap_data->data(),
                         result.bitmap_data->data() + result.bitmap_data->size());
            }
            if (png == tab->system_page_favicon_) {
              return;
            }
            tab->system_page_favicon_ = png;
            Host().tab_favicon_changed(Host().context, tab->id_, png.empty() ? nullptr : png.data(),
                                       png.size());
          },
          weak_factory_.GetWeakPtr()),
      &favicon_tracker_);
}

void Tab::SystemPageChanged() {
  StateChanged();
}

void Tab::SystemPageCommitted() {
  if (!system_page_) {
    return;
  }
  const GURL url = system_page_->url();
  Profile* profile = Profile::FromBrowserContext(web_contents()->GetBrowserContext());
  // In the profile's history, as Chromium's pages are, but never a private window's.
  if (!profile->IsOffTheRecord() && url.SchemeIsHTTPOrHTTPS()) {
    if (history::HistoryService* history = HistoryServiceFactory::GetForProfile(
            profile, ServiceAccessType::IMPLICIT_ACCESS)) {
      history->AddPage(url, base::Time::Now(), history::SOURCE_BROWSED);
    }
  }
  FetchSystemPageFavicon();
  SiteControls::PageChanged(id_);
  // Chromium tells each new document's title. WebKit's can replace the empty one the commit set
  // before the next report, and when it's the last page's, it would go untold.
  system_page_title_.reset();
  ForgetOutputRestricted(url);
  StateChanged();
}

void Tab::SystemPageLoaded() {
  // As DidFinishLoad reports Chromium's page.
  if (system_page_ && Host().tab_loaded) {
    Host().tab_loaded(Host().context, id_, system_page_->url().spec().c_str());
  }
}

void Tab::SystemPageLeft(const GURL& url) {
  if (!system_page_) {
    return;
  }
  Profile* profile = Profile::FromBrowserContext(web_contents()->GetBrowserContext());
  if (system_engine::Remembered(profile, url)) {
    system_page_->Load(url);
  } else {
    UseChromium(url);
  }
}

void Tab::SystemPageOpen(const GURL& url) {
  tabs::TabInterface* tab = tabs::TabInterface::MaybeGetFromContents(web_contents());
  if (!tab) {
    return;
  }
  // A tab of Chromium's, which hands a site the engine is kept for back to it.
  NavigateParams params(tab->GetBrowserWindowInterface(), url, ui::PAGE_TRANSITION_LINK);
  params.disposition = WindowOpenDisposition::NEW_FOREGROUND_TAB;
  params.source_contents = web_contents();
  Navigate(&params);
}

bool Tab::SystemPageKey(const input::NativeWebKeyboardEvent& event,
                        bool before_page,
                        bool editing) {
  return AskAppAboutKey(before_page ? Host().tab_key_before_page : Host().tab_key_unhandled, id_,
                        event, editing);
}

void Tab::SystemPageClicked() {
  Window::OnPageClicked(id_);
}

void Tab::SystemPageOutputRestricted() {
  // Once a site, as pages ask for keys again at every play.
  if (!system_page_ || output_restricted_at_.is_valid() || !Host().tab_media_unsupported) {
    return;
  }
  output_restricted_at_ = system_page_->url();
  // Logged, as no check can make a display's connection fall short.
  fprintf(stderr, "browser: tab %llu output restricted\n", static_cast<unsigned long long>(id_));
  Host().tab_media_unsupported(Host().context, id_, "output-restricted");
}

void Tab::ForgetOutputRestricted(const GURL& url) {
  if (!output_restricted_at_.is_valid() ||
      net::registry_controlled_domains::SameDomainOrHost(
          url, output_restricted_at_,
          net::registry_controlled_domains::INCLUDE_PRIVATE_REGISTRIES)) {
    return;
  }
  output_restricted_at_ = GURL();
  if (Host().tab_media_unsupported) {
    Host().tab_media_unsupported(Host().context, id_, "");
  }
}

bool Tab::muted() const {
  return web_contents() ? web_contents()->IsAudioMuted() : unloaded_ && unloaded_->muted;
}

void Tab::DidStartNavigation(content::NavigationHandle* navigation) {
  if (navigation->IsInPrimaryMainFrame()) {
    PasswordOffer::WatchKey(web_contents());
    StateChanged();
    // The page under the pointer is going; the new one tells where the pointer is on it. A link
    // that scrolls the same page keeps it.
    if (!navigation->IsSameDocument()) {
      ReportLinkTarget(GURL());
    }
  }
}

void Tab::DidFinishNavigation(content::NavigationHandle* navigation) {
  if (!navigation->IsInPrimaryMainFrame()) {
    return;
  }
  if (navigation->HasCommitted() && !navigation->IsSameDocument()) {
    net_error_ = navigation->IsErrorPage() ? navigation->GetNetErrorCode() : 0;
    ShowOnPageColor();
    ClearHang();
    if (std::exchange(crashed_, false)) {
      LayOutSoon();
    }
  }
  StateChanged();
}

void Tab::DidStartLoading() {
  StateChanged();
}

void Tab::DidStopLoading() {
  StateChanged();
}

void Tab::NavigationEntriesDeleted() {
  StateChanged();
}

void Tab::StateChanged() {
  SessionChanged();
  ReportStateSoon();
}

void Tab::ReportStateSoon() {
  if (state_report_posted_) {
    return;
  }
  state_report_posted_ = true;
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&Tab::ReportState, weak_factory_.GetWeakPtr()));
}

void Tab::ReportState() {
  state_report_posted_ = false;
  if (!Host().tab_state_changed) {
    return;
  }
  const Space* space = Space::Holding(id_);
  const GURL pinned_url = space ? space->PinnedUrl(id_) : Favorites::Url(id_);
  content::WebContents* contents = web_contents();
  if (!contents) {
    // Its history says where it is and where it can go. Without a page, it has nothing wrong.
    const std::string url = Url().spec();
    const int count = unloaded_ ? static_cast<int>(unloaded_->history.size()) : 0;
    const int current = unloaded_ ? unloaded_->current : 0;
    const bc_tab_state state = {
        .url = url.c_str(),
        .can_go_back = current > 0,
        .can_go_forward = current + 1 < count,
        .loading = false,
        .unloaded = true,
        .away_from_pinned_url = pinned_url.is_valid() && Url() != pinned_url,
        .audible = false,
        .muted = muted(),
        .icon = icon_.c_str(),
        .trouble = BC_PAGE_FINE,
        .net_error = 0,
        .system_engine = false,
        .media_title = "",
        .media_artist = "",
        .developer_mode = developer_mode::On(profile(), Url()),
    };
    Host().tab_state_changed(Host().context, id_, &state);
    return;
  }
  content::NavigationController& history = contents->GetController();
  platform::SystemPage* page = system_page_.get();
  // The visible URL is the one Chrome's location bar shows: one a page starts loading by itself
  // shows only once it commits, so a page can't put another site's URL over its own. One waiting
  // to load is the link the user opened. WebKit's shows the page's own, as Safari's does.
  const std::string url =
      (page ? page->url() : waiting_ ? waiting_->url : contents->GetVisibleURL()).spec();
  const RecentlyAudibleHelper* audible = RecentlyAudibleHelper::FromWebContents(contents);
  // The system's engine shows its page's own markers, and Chromium's capture none of it.
  const scoped_refptr<MediaStreamCaptureIndicator> capture =
      MediaCaptureDevicesDispatcher::GetInstance()->GetMediaStreamCaptureIndicator();
  const bool sharing = capture->IsCapturingTab(contents) ||
                       capture->IsCapturingWindow(contents) ||
                       capture->IsCapturingDisplay(contents);
  // Chrome's media controls show a session they can control, as the sidebar's player does.
  content::MediaSession* session = page ? nullptr : content::MediaSession::GetIfExists(contents);
  const media_session::mojom::MediaSessionInfoPtr media =
      session ? session->GetMediaSessionInfoSync() : nullptr;
  const bool controllable = media && media->is_controllable;
  const std::string media_title =
      controllable ? base::UTF16ToUTF8(session->GetMediaSessionMetadata().title) : "";
  const std::string media_artist =
      controllable ? base::UTF16ToUTF8(session->GetMediaSessionMetadata().artist) : "";
  const bc_tab_state state = {
      .url = url.c_str(),
      .can_go_back = CanGoBack(),
      .can_go_forward = CanGoForward(),
      .loading = page ? page->loading() : contents->ShouldShowLoadingUI(),
      .unloaded = !page && (history.NeedsReload() || waiting_),
      .away_from_pinned_url = pinned_url.is_valid() && PageUrl(contents) != pinned_url,
      .audible = page ? page->audible() : audible && audible->WasRecentlyAudible(),
      .muted = page ? page->muted() : contents->IsAudioMuted(),
      .icon = icon_.c_str(),
      .trouble = Trouble(),
      .net_error = Trouble() == BC_PAGE_NETWORK_ERROR ? net_error_ : 0,
      .system_engine = page != nullptr,
      .camera = !page && capture->IsCapturingVideo(contents),
      .microphone = !page && capture->IsCapturingAudio(contents),
      .sharing = !page && sharing,
      .shared = !page && capture->IsBeingMirrored(contents),
      .share_instead = !page && ShareInsteadInfoBar(contents),
      .media = controllable,
      .media_playing = controllable && media->playback_state ==
                                           media_session::mojom::MediaPlaybackState::kPlaying,
      .media_title = media_title.c_str(),
      .media_artist = media_artist.c_str(),
      .media_next = controllable &&
                    std::ranges::contains(session->GetMediaSessionActionsSync(),
                                          media_session::mojom::MediaSessionAction::kNextTrack),
      // Chrome offers the action while the page floats too, where the player has nothing to do.
      .media_picture_in_picture =
          controllable &&
          media->picture_in_picture_state !=
              media_session::mojom::MediaPictureInPictureState::kInPictureInPicture &&
          std::ranges::contains(session->GetMediaSessionActionsSync(),
                                media_session::mojom::MediaSessionAction::kEnterPictureInPicture),
      .developer_mode = developer_mode::On(profile(), GURL(url)),
  };
  Host().tab_state_changed(Host().context, id_, &state);
  // The system page's comes from FetchSystemPageFavicon.
  if (page) {
    // Its title changes as Chromium's page's does, which TitleWasSet reports, and is told once a
    // document at least.
    if (Title() != system_page_title_) {
      system_page_title_ = Title();
      ReportTitle();
    }
    return;
  }
  ReportFavicon(state.loading);
}

void Tab::ReportFavicon(bool loading) {
  if (!Host().tab_favicon_changed) {
    return;
  }
  content::NavigationEntry* entry = web_contents()->GetController().GetLastCommittedEntry();
  gfx::Image favicon;
  if (entry && entry->GetFavicon().valid) {
    favicon = entry->GetFavicon().image;
  } else if (loading || web_contents()->GetController().NeedsReload()) {
    // The new page's favicon comes once it has committed, from the history database or the page,
    // so the old one stays rather than flashing none, as it does for a page given back and not yet
    // loaded.
    return;
  }
  if (favicon.AsImageSkia().BackedBySameObjectAs(favicon_.AsImageSkia())) {
    return;
  }
  favicon_ = favicon;
  std::optional<std::vector<uint8_t>> png;
  if (!favicon.IsEmpty()) {
    png = gfx::PNGCodec::EncodeBGRASkBitmap(favicon.AsImageSkia().GetRepresentation(2).GetBitmap(),
                                            /*discard_transparency=*/false);
  }
  Host().tab_favicon_changed(Host().context, id_, png ? png->data() : nullptr,
                             png ? png->size() : 0);
}

void Tab::OnFaviconUpdated(favicon::FaviconDriver* driver,
                           NotificationIconType type,
                           const GURL& icon_url,
                           bool icon_url_changed,
                           const gfx::Image& image) {
  if (type == NON_TOUCH_16_DIP) {
    StateChanged();
  }
}

void Tab::DidFinishLoad(content::RenderFrameHost* frame, const GURL& url) {
  // An unloaded page's empty document finishes loading at the page's URL, which isn't the page.
  if (frame->IsInPrimaryMainFrame() && !web_contents()->WasDiscarded() && Host().tab_loaded) {
    Host().tab_loaded(Host().context, id_, url.spec().c_str());
  }
}

void Tab::SetPageColor(SkColor color) {
  page_color_ = color;
  content::RenderWidgetHostView* view =
      web_contents() ? web_contents()->GetRenderWidgetHostView() : nullptr;
  if (view) {
    view->SetBackgroundColor(color);
  }
  ShowOnPageColor();
}

void Tab::ShowOnPageColor() {
  // The core's error page has no background of its own (//browser_core/error_page.cc), so it sits
  // on the colour the pane shows before a page draws, as the app's cards sit on theirs. So does the
  // empty document a tab shows until its first page commits, as one a page opened waits for an
  // answer, which off the Mac draws white over the pane. Every other page keeps Chrome's white.
  const content::NavigationEntry* entry =
      web_contents() ? web_contents()->GetController().GetLastCommittedEntry() : nullptr;
  const bool empty = !entry || entry->IsInitialEntry();
  const std::optional<SkColor> color = net_error_ || empty ? page_color_ : std::nullopt;
  if (web_contents() && color != shown_on_) {
    shown_on_ = color;
    web_contents()->SetPageBaseBackgroundColor(color);
  }
}

void Tab::RenderFrameHostChanged(content::RenderFrameHost* old_host,
                                 content::RenderFrameHost* new_host) {
  // A page from another site comes in a view of its own, white until its first frame.
  if (page_color_ && new_host == web_contents()->GetPrimaryMainFrame() && new_host->GetView()) {
    new_host->GetView()->SetBackgroundColor(*page_color_);
  }
}

void Tab::RenderViewReady() {
  if (page_color_) {
    SetPageColor(*page_color_);
  }
  // A page that died has a renderer again, loading it: its pane shows it again.
  if (std::exchange(crashed_, false)) {
    LayOutSoon();
    StateChanged();
  }
  // As Chrome's reload focuses the page, as the user asked for it. The view the keyboard was in
  // went with the renderer, and with no view focused no key reaches the host's tab_key_before_page.
  if (std::exchange(focus_when_ready_, false)) {
    Window::PageViewChanged(id_);
  } else {
    // Aura doesn't focus a new renderer's view either, as for a tab unloaded or killed that loads
    // again once it's shown: the keyboard stays in the view that holds it, for no page.
    Window::PageCommittedInNewView(id_);
  }
}

void Tab::TitleWasSet(content::NavigationEntry* entry) {
  // Chromium's page waits under the system page, whose title is the tab's.
  if (system_page_) {
    return;
  }
  SessionChanged();
  ReportTitle();
}

void Tab::Rename(const std::string& name) {
  name_ = name;
  SessionChanged();
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&Tab::ReportTitle, weak_factory_.GetWeakPtr()));
}

void Tab::SetIcon(const std::string& icon) {
  icon_ = icon;
  StateChanged();
}

std::string Tab::Title() {
  if (!name_.empty()) {
    return name_;
  }
  if (web_contents()) {
    return base::UTF16ToUTF8(system_page_ ? system_page_->title() : web_contents()->GetTitle());
  }
  if (!unloaded_) {
    return std::string();
  }
  // As Chrome titles a page without one.
  const sessions::SerializedNavigationEntry& entry = unloaded_->history[unloaded_->current];
  return entry.title().empty() ? entry.virtual_url().spec() : base::UTF16ToUTF8(entry.title());
}

void Tab::ReportTitle() {
  if (Host().tab_title_changed) {
    Host().tab_title_changed(Host().context, id_, Title().c_str());
  }
}

void Tab::DidGetUserInteraction(const blink::WebInputEvent& event) {
  if (event.GetType() == blink::WebInputEvent::Type::kMouseDown ||
      event.GetType() == blink::WebInputEvent::Type::kRawKeyDown) {
    const int command =
        BUILDFLAG(IS_MAC) ? blink::WebInputEvent::kMetaKey : blink::WebInputEvent::kControlKey;
    last_press_held_command_ = event.GetModifiers() & command;
  }
  // A click, not the keyboard coming to the page: the core moves the keyboard between the pages of
  // a split view itself, and focus reports posted from those moves arrive after the next one.
  if (event.GetType() == blink::WebInputEvent::Type::kMouseDown) {
    last_click_ = static_cast<const blink::WebMouseEvent&>(event).PositionInWidget();
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Window::OnPageClicked, id_));
  }
}

void Tab::ReportLinkTarget(const GURL& url) {
  // Only as it changes: nothing is told while the pointer is on no link and none was shown.
  if (url.is_empty() && !link_target_) {
    return;
  }
  link_target_ = !url.is_empty();
  if (!Host().tab_link_target) {
    return;
  }
  const LinkTarget target = FormatLinkTarget(url);
  Host().tab_link_target(Host().context, id_, target.text.c_str(), target.domain_begin,
                         target.domain_end);
}

void Tab::EnteredFullScreen() {
  if (fullscreen_) {
    return;
  }
  fullscreen_ = true;
  LogFullScreenAsked(true);
  // At once, as only the page's request takes it full screen: Chrome has asked the window to go
  // with it, in a task it posted, and the app lays the page out at the window's new size only if
  // it hears first.
  if (Host().tab_fullscreen_changed) {
    Host().tab_fullscreen_changed(Host().context, id_, true);
  }
}

void Tab::DidToggleFullscreenModeForTab(bool entered_fullscreen, bool will_cause_resize) {
  // Once each way: a page leaving full screen itself is reported as Chrome ends its full screen
  // and again as the page's request returns.
  if (entered_fullscreen) {
    EnteredFullScreen();
    return;
  }
  if (!fullscreen_) {
    return;
  }
  fullscreen_ = false;
  LogFullScreenAsked(false);
  // Posted, as the state is: Chrome ends a page's full screen within the app's call that switches
  // or closes its tab. By id, so the app hears it once the tab has gone too. Still before the
  // window leaves full screen, which Chrome posts after.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](bc_id id) {
                       if (Host().tab_fullscreen_changed) {
                         Host().tab_fullscreen_changed(Host().context, id, false);
                       }
                     },
                     id_));
}

void Tab::DidUpdateAudioMutingState(bool muted) {
  ReportStateSoon();
}

void Tab::OnVisibilityChanged(content::Visibility visibility) {
  // As Chrome loads a restored tab once it's shown. Posted: the page view is being shown now.
  if (visibility == content::Visibility::VISIBLE && waiting_) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Tab::LoadWaiting, weak_factory_.GetWeakPtr()));
  }
}

// The page's renderer went away, crashed, killed or ended on purpose. check.sh waits for this line,
// an error since our builds print no warnings.
void Tab::PrimaryMainFrameRenderProcessGone(base::TerminationStatus status) {
  // Let go, not lost: the tab unloaded its page, or shows it in the system's engine now. Unloading
  // deletes the page rather than ending its renderer, so Chrome says nothing here then.
  if (unloading_ || system_page_ || web_contents()->WasDiscarded()) {
    return;
  }
  LOG(ERROR) << "browser: tab " << id_ << " page gone, status " << static_cast<int>(status);
  ClearHang();
  if (reload_after_kill_) {
    focus_when_ready_ = true;
    // Posted: Chrome is still telling the page's observers it went.
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Tab::Reload, weak_factory_.GetWeakPtr(),
                                  *std::exchange(reload_after_kill_, std::nullopt)));
  } else if (Died(status) && !browser_shutdown::HasShutdownStarted()) {
    if (Window::Showing(id_)) {
      crashed_ = true;
      LayOutSoon();
    } else {
      // Behind, it has nothing to show the user, so it's unloaded, as Chrome does with a page it
      // ended for memory, and loads again once it's shown.
      web_contents()->GetController().SetNeedsReload();
    }
  }
  StateChanged();
}

// static
void Tab::PageHung(content::WebContents* contents,
                   content::RenderWidgetHost* widget,
                   base::RepeatingClosure restart_hang_monitor) {
  // A page in none of the core's tabs, such as DevTools', waits as it would after Chrome's Wait.
  Tab* tab = FromWebContents(contents);
  if (!tab) {
    return;
  }
  tab->restart_hang_monitor_ = std::move(restart_hang_monitor);
  tab->hung_process_ = widget->GetProcess()->GetDeprecatedID();
  // check.sh waits for this line.
  LOG(ERROR) << "browser: tab " << tab->id_ << " page hung";
  tab->ReportStateSoon();
}

// static
void Tab::GameRunning(content::WebContents* contents, bool running) {
  if (Tab* tab = FromWebContents(contents)) {
    // check.sh waits for these lines.
    LOG(ERROR) << "browser: tab " << tab->id_ << " game " << (running ? "running" : "over");
  }
  if (auto* reloader = error_page::NetErrorAutoReloader::FromWebContents(contents)) {
    reloader->SetHeld(running);
  }
}

void Tab::OnRendererResponsive(content::RenderProcessHost* process) {
  if (hung_process_ && process->GetDeprecatedID() == hung_process_) {
    ClearHang();
    ReportStateSoon();
  }
}

void Tab::Wait() {
  if (!restart_hang_monitor_) {
    return;
  }
  base::RepeatingClosure restart = std::move(restart_hang_monitor_);
  ClearHang();
  restart.Run();
  ReportStateSoon();
}

void Tab::Reload(bool bypass_cache) {
  if (system_page_) {
    system_page_->Reload(bypass_cache);
    return;
  }
  content::RenderProcessHost* hung =
      hung_process_ ? content::RenderProcessHost::FromID(hung_process_) : nullptr;
  ClearHang();
  if (hung && hung->IsInitializedAndNotDead()) {
    const bool page = hung == web_contents()->GetPrimaryMainFrame()->GetProcess();
    // As Chrome's Exit page: a reload would wait on the renderer that hung.
    hung->Shutdown(content::RESULT_CODE_HUNG);
    // The page reloads once its renderer has gone. A frame's alone ends without it.
    if (page) {
      reload_after_kill_ = bypass_cache;
      ReportStateSoon();
      return;
    }
  }
  // As Chrome's, it asks before sending a form again.
  web_contents()->GetController().Reload(
      bypass_cache ? content::ReloadType::BYPASSING_CACHE : content::ReloadType::NORMAL,
      /*check_for_repost=*/true);
}

void Tab::ClearHang() {
  restart_hang_monitor_.Reset();
  hung_process_ = 0;
}

void Tab::LeftWindow() {
  if (!crashed_) {
    return;
  }
  // Posted: the window letting go of the tab still lists it, and another may be taking it.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](base::WeakPtr<Tab> tab) {
                       if (tab && tab->crashed_ && tab->web_contents() &&
                           !Window::Showing(tab->id_)) {
                         tab->crashed_ = false;
                         tab->web_contents()->GetController().SetNeedsReload();
                         tab->StateChanged();
                       }
                     },
                     weak_factory_.GetWeakPtr()));
}

void Tab::LayOutSoon() {
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(&Window::LayOutShowing, id_));
}

int Tab::Trouble() const {
  if (crashed_) {
    return BC_PAGE_CRASHED;
  }
  // An unloaded page has no renderer to hang and no error page to show.
  if (!web_contents() || waiting_ || web_contents()->GetController().NeedsReload()) {
    return BC_PAGE_FINE;
  }
  if (hung_process_) {
    return BC_PAGE_HUNG;
  }
  return net_error_ ? BC_PAGE_NETWORK_ERROR : BC_PAGE_FINE;
}

void Tab::OnIsCapturingVideoChanged(content::WebContents* contents, bool) {
  CaptureChanged(contents);
}

void Tab::OnIsCapturingAudioChanged(content::WebContents* contents, bool) {
  CaptureChanged(contents);
}

void Tab::OnIsBeingMirroredChanged(content::WebContents* contents, bool) {
  CaptureChanged(contents);
}

void Tab::OnIsCapturingTabChanged(content::WebContents* contents, bool) {
  CaptureChanged(contents);
}

void Tab::OnIsCapturingWindowChanged(content::WebContents* contents, bool) {
  CaptureChanged(contents);
}

void Tab::OnIsCapturingDisplayChanged(content::WebContents* contents, bool) {
  CaptureChanged(contents);
}

void Tab::ShareInstead() {
  if (TabSharingInfoBarDelegate* sharing = ShareInsteadInfoBar(web_contents())) {
    sharing->ShareThisTabInstead();
  }
}

void Tab::Media(int action) {
  content::MediaSession* session =
      web_contents() ? content::MediaSession::GetIfExists(web_contents()) : nullptr;
  if (!session) {
    return;
  }
  // As the user asked from the browser's UI, as Chrome's media controls do.
  constexpr auto kUI = media_session::mojom::MediaSession::SuspendType::kUI;
  switch (action) {
    case BC_MEDIA_PLAY:
      session->Resume(kUI);
      break;
    case BC_MEDIA_PAUSE:
      session->Suspend(kUI);
      break;
    case BC_MEDIA_NEXT:
      session->NextTrack();
      break;
    case BC_MEDIA_PICTURE_IN_PICTURE:
      // The page's own handler if it has one, else its one video, as Chrome's media controls do.
      session->EnterPictureInPicture();
      break;
  }
}

void Tab::MediaStartedPlaying(const MediaPlayerInfo&, const content::MediaPlayerId&) {
  if (!media_observer_.is_bound()) {
    content::MediaSession::Get(web_contents())
        ->AddObserver(media_observer_.BindNewPipeAndPassRemote());
  }
}

void Tab::MediaSessionInfoChanged(media_session::mojom::MediaSessionInfoPtr) {
  ReportStateSoon();
}

void Tab::MediaSessionMetadataChanged(const std::optional<media_session::MediaMetadata>&) {
  ReportStateSoon();
}

void Tab::MediaSessionActionsChanged(const std::vector<media_session::mojom::MediaSessionAction>&) {
  ReportStateSoon();
}

void Tab::OnInfoBarAdded(infobars::InfoBar*) {
  ReportStateSoon();
}

void Tab::OnInfoBarRemoved(infobars::InfoBar*, bool) {
  ReportStateSoon();
}

void Tab::OnInfoBarReplaced(infobars::InfoBar*, infobars::InfoBar*) {
  ReportStateSoon();
}

void Tab::OnManagerWillBeDestroyed(infobars::InfoBarManager*) {
  infobar_observation_.Reset();
}

void Tab::CaptureChanged(content::WebContents* contents) {
  // Every tab hears every page's.
  if (contents && contents == web_contents()) {
    ReportStateSoon();
  }
}

void Tab::WebContentsDestroyed() {
  // The driver is the page's too, and may go before the tab does, as may its infobars.
  favicon_observation_.Reset();
  infobar_observation_.Reset();
  media_observer_.reset();
  audible_subscription_ = {};
  system_page_.reset();
  // Unloaded, it stays without its page.
  if (unloading_) {
    Observe(nullptr);
    return;
  }
  // Closed, as at a quit or with its window. Its page is still there, so the session kept as the
  // first tab closes saves this one from it too.
  ReportClosed();
  // What it posted, such as a report of its renderer ending, would tell the app of a closed tab.
  weak_factory_.InvalidateWeakPtrs();
  Observe(nullptr);
  Forget(id_);
}

}  // namespace browser_core
