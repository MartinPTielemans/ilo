// A page shown by the system's own web view in place of Chromium's: WebKit's on the Mac. A tab
// switches to one for a site whose protected video only the system's engine plays
// (docs/decisions.md, decision 1). The tab keeps its id, its Space and its place; only its pane's
// view and what it reports come from here.

#ifndef BROWSER_CORE_PLATFORM_SYSTEM_PAGE_H_
#define BROWSER_CORE_PLATFORM_SYSTEM_PAGE_H_

#include <cstdint>
#include <map>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "base/functional/callback_forward.h"
#include "browser_core/quit_stores_wait.h"
#include "net/cookies/canonical_cookie.h"
#include "ui/gfx/native_ui_types.h"
#include "url/gurl.h"
#include "url/origin.h"

namespace input {
struct NativeWebKeyboardEvent;
}

namespace browser_core::platform {

// What a site's data clearing deletes (ClearSiteAndReload, SystemPage::ClearSite).
enum class SiteData { kCookies, kCache, kAll };

// A page's media, as the sidebar's player shows it (bc_tab_state's media fields).
struct SystemMedia {
  // It played since its document committed, so the player has something to play again.
  bool has = false;
  bool playing = false;
  // Its navigator.mediaSession.metadata's, or empty without one.
  std::u16string title;
  std::u16string artist;
  // It named its track, so it's offered the previous and the next: a page that names its tracks
  // nearly always skips them, and nothing outside the page can tell which actions it handles.
  bool named = false;
};

// What the player does to a page's media.
enum class MediaAction { kPlay, kPause, kNext, kPrevious };

// Where a system page keeps its cookies and site data: one store a profile, apart from Chromium's
// and from Safari's. |id| is a UUID for a profile's, or for a private session's, whose store keeps
// nothing on disk and goes once ReleaseStore lets it go.
struct SystemStore {
  std::string id;
  bool persistent;
};

// An origin's localStorage, keys to values, as the page's script has them.
using StorageItems = std::map<std::u16string, std::u16string>;
using StorageByOrigin = std::map<url::Origin, StorageItems>;

// Whether |host|, an origin's or a cookie's domain without its dot, is |site| or a host under it,
// so a site's data moves with each of its subdomains'.
inline bool HostOfSite(std::string_view host, std::string_view site) {
  return host == site || (host.size() > site.size() && host.ends_with(site) &&
                          host[host.size() - site.size() - 1] == '.');
}

class SystemPage {
 public:
  // What the page tells its tab, on the main thread. SystemPageChanged can come inside the tab's
  // own calls, as WebKit's observers do; the rest come from tasks of their own.
  class Delegate {
   public:
    // Its URL, title, loading, history, sound or media changed.
    virtual void SystemPageChanged() = 0;
    // A new document committed in its main frame.
    virtual void SystemPageCommitted() = 0;
    // That document finished loading.
    virtual void SystemPageLoaded() = 0;
    // The user followed a link to |url|, on another site, in the page's main frame, or a link
    // to the page's own site whose redirects ended there. The page doesn't load it: the tab
    // loads it, here or in Chromium.
    virtual void SystemPageLeft(const GURL& url) = 0;
    // The page asked for |url| in a new window, as a link with a target or window.open do.
    virtual void SystemPageOpen(const GURL& url) = 0;
    // A key going down with the page focused, before the page sees it, or once the page didn't
    // keep it. |editing| if the page's focused element takes text. Returns whether the app used
    // it.
    virtual bool SystemPageKey(const input::NativeWebKeyboardEvent& event,
                               bool before_page,
                               bool editing) = 0;
    // Tab went past the page's last control, or ⇧Tab (|reverse|) before its first. Returns
    // whether the app took the keyboard; if not, the page has it again at its other end.
    virtual bool SystemPageFocusLeft(bool reverse) = 0;
    // A click in the page.
    virtual void SystemPageClicked() = 0;
    // The page's protected video is kept off the screen: the display connection's protection
    // (HDCP) isn't enough for a key it has. The page says so, so the page can say it falsely.
    virtual void SystemPageOutputRestricted() = 0;
    // The icon of the document that committed last, as its server sent it, from its own
    // <link rel=icon>, else its site's /favicon.ico. Empty once it's known to have none. Nothing
    // has decoded it.
    virtual void SystemPageIcon(const GURL& icon_url, std::vector<uint8_t> bytes) = 0;

   protected:
    virtual ~Delegate() = default;
  };

  // Whether the platform has a system engine that keeps a store per profile: macOS 14 or later.
  // Answers without loading it.
  static bool Available();
  // A page for |url| in |store|, with the system's engine loaded the first time. It loads nothing
  // until Start. Null where there's none.
  static std::unique_ptr<SystemPage> Create(const SystemStore& store,
                                            const GURL& url,
                                            Delegate* delegate);
  // Writes |cookies| into |store|, each with its domain, path, expiry, Secure, HttpOnly and
  // SameSite, then runs |done| with how many it took. Loads the system's engine, as DeleteStore.
  static void SetCookies(const SystemStore& store,
                         const std::vector<net::CanonicalCookie>& cookies,
                         base::OnceCallback<void(size_t)> done);
  // Reads |store|'s cookies of |site|, a registrable domain or a host, and of the hosts under it.
  // Loads the system's engine, as DeleteStore.
  static void GetCookies(const SystemStore& store,
                         const std::string& site,
                         base::OnceCallback<void(std::vector<net::CanonicalCookie>)> done);
  // Deletes |id|'s persistent store from the disk, as its profile goes, then runs |done| with
  // whether it went. Loads the system's engine: call it only for a store a page made.
  static void DeleteStore(const std::string& id, base::OnceCallback<void(bool deleted)> done);
  // Runs |done| with the ids of the app's persistent stores on disk, or none if it can't list
  // them. Loads the system's engine.
  static void ListStores(base::OnceCallback<void(std::vector<std::string>)> done);
  // Lets a private session's store go. Never loads the system's engine.
  static void ReleaseStore(const std::string& id);
  // Deletes |domain|'s |what| from |store|, then runs |done|. Loads the system's engine, as
  // DeleteStore.
  static void ClearSite(const SystemStore& store,
                        const std::string& domain,
                        SiteData what,
                        base::OnceClosure done);
  // Asks the system's engine to write what the session's stores keep to the disk now, then runs
  // |done| with how it answered: at once with kNone if the session made no store that keeps
  // anything. A quit asks after its last page has closed, as an item a page set less than a moment
  // before is otherwise lost with the engine's process (#3685). Never loads the system's engine.
  static void WriteStores(base::OnceCallback<void(StoresWritten)> done);

  SystemPage(const SystemPage&) = delete;
  SystemPage& operator=(const SystemPage&) = delete;
  virtual ~SystemPage() = default;

  // The view the window shows in the tab's pane, as it shows a WebContents's.
  virtual gfx::NativeView view() = 0;
  // Gives the page the keyboard.
  virtual void Focus() = 0;
  // Gives the page the keyboard at its first control, or its |last|, as Tab or ⇧Tab into it does.
  virtual void FocusEnd(bool last) = 0;

  // Starts the page's first load, which |storage| goes into: each origin's items are set before
  // the scripts of its first document in that load run. A Load before it changes what it loads.
  virtual void Start(StorageByOrigin storage) = 0;
  // Reads the localStorage of the origin the page shows, then runs |done| with it: nothing for a
  // page that has none, or doesn't answer within a moment.
  virtual void GetStorage(base::OnceCallback<void(url::Origin, StorageItems)> done) = 0;

  // Draws the page as its view shows it, then runs |done| with a PNG of it in sRGB at the
  // display's scale, or nothing if it couldn't. Protected video comes out black.
  virtual void Snapshot(base::OnceCallback<void(std::optional<std::vector<uint8_t>>)> done) = 0;

  virtual GURL url() = 0;
  virtual std::u16string title() = 0;
  virtual bool loading() = 0;
  virtual bool can_go_back() = 0;
  virtual bool can_go_forward() = 0;
  // Its connection is https and everything it loaded came over it.
  virtual bool secure() = 0;
  virtual bool audible() = 0;
  virtual bool muted() = 0;
  virtual SystemMedia media() = 0;

  virtual void Load(const GURL& url) = 0;
  virtual void GoBack() = 0;
  virtual void GoForward() = 0;
  virtual void Reload(bool bypass_cache) = 0;
  virtual void Stop() = 0;
  virtual void SetMuted(bool muted) = 0;
  // Through the page's own handlers where it has them, as the media keys go.
  virtual void Media(MediaAction action) = 0;

 protected:
  SystemPage() = default;
};

}  // namespace browser_core::platform

#endif  // BROWSER_CORE_PLATFORM_SYSTEM_PAGE_H_
