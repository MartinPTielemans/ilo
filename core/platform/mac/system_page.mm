#include "browser_core/platform/system_page.h"

#import <AppKit/AppKit.h>
#import <WebKit/WebKit.h>
#include <dlfcn.h>
#include <objc/message.h>
#include <stdio.h>

#include <memory>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "base/apple/owned_objc.h"
#include "base/command_line.h"
#include "base/functional/bind.h"
#include "base/functional/callback.h"
#include "base/logging.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "base/strings/sys_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "components/input/native_web_keyboard_event.h"
#include "media/base/media_switches.h"
#include "net/base/apple/url_conversions.h"
#include "net/base/registry_controlled_domains/registry_controlled_domain.h"
#include "net/cookies/canonical_cookie.h"
#include "net/cookies/cookie_constants.h"
#include "url/gurl.h"
#include "url/origin.h"

// The core never links WebKit, and starts none of it, no view, store or WebKit process, before the
// first system page, or before it clears or deletes a store a profile made in an earlier run. Its
// code is mapped at launch all the same: system frameworks Chromium's framework links, such as
// AuthenticationServices, SafariServices and Quartz, link it. Its classes are found by name and its
// constants by symbol, so the core adds no link of its own.

namespace browser_core::platform {
class SystemPageMac;
}

namespace {

constexpr char kWebKitPath[] = "/System/Library/Frameworks/WebKit.framework/WebKit";
void* g_webkit = nullptr;

// Loads WebKit, once. Only the calls below that need it do, each saying why.
bool LoadWebKit(const char* why) {
  if (!g_webkit) {
    // Whether something else mapped it first, such as a system framework of Chromium's.
    const bool mapped = dlopen(kWebKitPath, RTLD_LAZY | RTLD_LOCAL | RTLD_NOLOAD) != nullptr;
    g_webkit = dlopen(kWebKitPath, RTLD_LAZY | RTLD_LOCAL);
    // check.sh reads this line.
    fprintf(stderr, "browser: system engine loaded for %s%s\n", why,
            mapped ? ", already mapped" : "");
  }
  return g_webkit != nullptr;
}

// WebKit, once a call above has loaded it.
void* WebKit() {
  return g_webkit;
}

Class WebKitClass(NSString* name) {
  return WebKit() ? NSClassFromString(name) : nil;
}

NSString* WebKitConstant(const char* name) {
  void* symbol = WebKit() ? dlsym(WebKit(), name) : nullptr;
  return symbol ? *static_cast<NSString* __unsafe_unretained*>(symbol) : nil;
}

// The stores the pages use, by id, which one profile's pages share. A private session's keeps
// nothing on disk, and goes once it's released.
NSMutableDictionary<NSString*, WKWebsiteDataStore*>* Stores() {
  static NSMutableDictionary<NSString*, WKWebsiteDataStore*>* const stores =
      [NSMutableDictionary dictionary];
  return stores;
}

WKWebsiteDataStore* StoreFor(const browser_core::platform::SystemStore& store) {
  NSString* key = base::SysUTF8ToNSString(store.id);
  if (WKWebsiteDataStore* made = Stores()[key]) {
    return made;
  }
  WKWebsiteDataStore* made = nil;
  id stores = WebKitClass(@"WKWebsiteDataStore");
  if (!store.persistent) {
    made = [stores nonPersistentDataStore];
  } else if (@available(macOS 14, *)) {
    NSUUID* uuid = [[NSUUID alloc] initWithUUIDString:key];
    made = uuid ? [stores dataStoreForIdentifier:uuid] : nil;
  }
  if (made) {
    Stores()[key] = made;
  }
  return made;
}

// Safari's application name, which the page's user agent ends with. Spotify serves WKWebView's
// own user agent, which has none, its phone page, and Safari's its web player.
NSString* SafariName() {
  static NSString* const name = [] {
    NSDictionary* info =
        [NSDictionary dictionaryWithContentsOfFile:@"/Applications/Safari.app/Contents/Info.plist"];
    NSString* version = info[@"CFBundleShortVersionString"] ?: @"26.0";
    return [NSString stringWithFormat:@"Version/%@ Safari/605.1.15", version];
  }();
  return name;
}

// The world the browser's own scripts run in, which the page's can't see or reach. localStorage is
// the origin's, the same in each world.
WKContentWorld* BrowserWorld() {
  return [WebKitClass(@"WKContentWorld") worldWithName:@"ilo"];
}

// The page's own world, for what only it sees.
WKContentWorld* PageWorld() {
  return [WebKitClass(@"WKContentWorld") pageWorld];
}

// Whether |action| is a server's redirect of a navigation already under way. WebKit's own, not
// public; without it, a redirect is taken for the navigation it continues.
bool IsRedirect(WKNavigationAction* action) {
  SEL redirect = NSSelectorFromString(@"_isRedirect");
  return [action respondsToSelector:redirect] &&
         reinterpret_cast<BOOL (*)(id, SEL)>(objc_msgSend)(action, redirect);
}

// |cookie| as WebKit's store takes it, or nil for one Foundation can't hold.
NSHTTPCookie* WebKitCookie(const net::CanonicalCookie& cookie) {
  NSString* name = base::SysUTF8ToNSString(cookie.Name());
  NSString* value = base::SysUTF8ToNSString(cookie.Value());
  // With a leading dot for a domain cookie; without, it goes to its host alone, as Chromium's
  // host cookie does.
  NSString* domain = base::SysUTF8ToNSString(cookie.Domain());
  NSString* path = base::SysUTF8ToNSString(cookie.Path());
  if (!name || !value || !domain || !path) {
    return nil;
  }
  NSMutableDictionary<NSHTTPCookiePropertyKey, id>* properties = [@{
    NSHTTPCookieName : name,
    NSHTTPCookieValue : value,
    NSHTTPCookieDomain : domain,
    NSHTTPCookiePath : path,
  } mutableCopy];
  if (cookie.SecureAttribute()) {
    properties[NSHTTPCookieSecure] = @"TRUE";
  }
  // CFNetwork's attribute, which NSHTTPCookie reads into isHTTPOnly: Foundation names no key.
  if (cookie.IsHttpOnly()) {
    properties[@"HttpOnly"] = @"TRUE";
  }
  if (cookie.IsPersistent()) {
    properties[NSHTTPCookieExpires] = cookie.ExpiryDate().ToNSDate();
  } else {
    properties[NSHTTPCookieDiscard] = @"TRUE";
  }
  if (cookie.SameSite() == net::CookieSameSite::STRICT_MODE) {
    properties[NSHTTPCookieSameSitePolicy] = NSHTTPCookieSameSiteStrict;
  } else if (cookie.SameSite() == net::CookieSameSite::LAX_MODE) {
    properties[NSHTTPCookieSameSitePolicy] = NSHTTPCookieSameSiteLax;
  }
  return [NSHTTPCookie cookieWithProperties:properties];
}

// |cookie| as Chromium's store takes it, or null for one it wouldn't.
std::unique_ptr<net::CanonicalCookie> ChromiumCookie(NSHTTPCookie* cookie) {
  const base::Time now = base::Time::Now();
  const base::Time expiry =
      cookie.expiresDate ? base::Time::FromNSDate(cookie.expiresDate) : base::Time();
  if (!expiry.is_null() && expiry <= now) {
    return nullptr;
  }
  const std::string domain = base::SysNSStringToUTF8(cookie.domain);
  const bool host_only = !domain.starts_with('.');
  const GURL url(std::string(cookie.secure ? "https://" : "http://") +
                 (host_only ? domain : domain.substr(1)) + "/");
  // Foundation has no word for SameSite=None, which Chromium takes only from a secure cookie.
  net::CookieSameSite same_site =
      cookie.secure ? net::CookieSameSite::NO_RESTRICTION : net::CookieSameSite::UNSPECIFIED;
  if ([cookie.sameSitePolicy isEqualToString:NSHTTPCookieSameSiteStrict]) {
    same_site = net::CookieSameSite::STRICT_MODE;
  } else if ([cookie.sameSitePolicy isEqualToString:NSHTTPCookieSameSiteLax]) {
    same_site = net::CookieSameSite::LAX_MODE;
  }
  return net::CanonicalCookie::CreateSanitizedCookie(
      url, base::SysNSStringToUTF8(cookie.name), base::SysNSStringToUTF8(cookie.value),
      host_only ? std::string() : domain, base::SysNSStringToUTF8(cookie.path), now, expiry, now,
      cookie.secure, cookie.HTTPOnly, same_site, net::COOKIE_PRIORITY_DEFAULT,
      /*partition_key=*/std::nullopt, /*status=*/nullptr);
}

// A script for the first load of a page, in the browser's world at each document's start, that
// sets the items |storage| has for the document's origin and no other's. Arrays, not an object, so
// no key the page used can name a prototype. Nil if Foundation can't write a string as JSON.
NSString* StorageScript(const browser_core::platform::StorageByOrigin& storage) {
  NSMutableArray* origins = [NSMutableArray array];
  for (const auto& [origin, items] : storage) {
    NSMutableArray* pairs = [NSMutableArray array];
    for (const auto& [key, value] : items) {
      [pairs addObject:@[ base::SysUTF16ToNSString(key), base::SysUTF16ToNSString(value) ]];
    }
    [origins addObject:@[ base::SysUTF8ToNSString(origin.Serialize()), pairs ]];
  }
  if (![NSJSONSerialization isValidJSONObject:origins]) {
    return nil;
  }
  NSData* json = [NSJSONSerialization dataWithJSONObject:origins options:0 error:nil];
  if (!json) {
    return nil;
  }
  return [NSString
      stringWithFormat:@"(() => {\n"
                        "  const items = new Map(%@).get(location.origin) || [];\n"
                        "  for (const [key, value] of items) {\n"
                        "    try { localStorage.setItem(key, value); } catch (e) {}\n"
                        "  }\n"
                        "})();",
                       [[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding]];
}

// Reads the localStorage of the document it runs in, as [origin, [[key, value], …]].
NSString* const kReadStorage =
    @"(() => {\n"
     "  const items = [];\n"
     "  for (let i = 0; i < localStorage.length; i++) {\n"
     "    const key = localStorage.key(i);\n"
     "    items.push([key, localStorage.getItem(key)]);\n"
     "  }\n"
     "  return [location.origin, items];\n"
     "})()";

// What the page's world posts to once its protected video is kept off the screen.
NSString* const kOutputRestricted = @"outputRestricted";

// Watches the protected-media sessions a document makes, in the page's world, as no other sees
// them, and posts once a key's status is "output-restricted": WebKit's word for a display
// connection whose protection (HDCP) falls short, which its player shows as black. It takes the
// handler at the document's start, before the page's scripts, which can post to it all the same.
// A Proxy, so the page's createSession still reads as WebKit's own.
NSString* const kWatchOutput =
    @"(() => {\n"
     "  const handler = window.webkit?.messageHandlers?.outputRestricted;\n"
     "  const keys = window.MediaKeys?.prototype;\n"
     "  if (!handler || !keys) return;\n"
     "  let told = false;\n"
     "  keys.createSession = new Proxy(keys.createSession, {\n"
     "    apply(create, mediaKeys, args) {\n"
     "      const session = Reflect.apply(create, mediaKeys, args);\n"
     "      session.addEventListener('keystatuseschange', () => {\n"
     "        session.keyStatuses.forEach((status) => {\n"
     "          if (status === 'output-restricted' && !told) {\n"
     "            told = true;\n"
     "            handler.postMessage(null);\n"
     "          }\n"
     "        });\n"
     "      });\n"
     "      return session;\n"
     "    },\n"
     "  });\n"
     "})();";

// How long a page has to answer for its localStorage, before its tab goes back to Chromium
// without it.
constexpr int64_t kStorageWaitSeconds = 2;

// The WebKit page's own keys, which KVO reports.
NSArray<NSString*>* ObservedKeys(WKWebView* web) {
  NSMutableArray<NSString*>* keys = [@[
    @"URL", @"title", @"loading", @"canGoBack", @"canGoForward", @"hasOnlySecureContent"
  ] mutableCopy];
  // WebKit's own, not public: whether the page plays sound, as Safari's speaker shows.
  if ([web respondsToSelector:NSSelectorFromString(@"_isPlayingAudio")]) {
    [keys addObject:@"_isPlayingAudio"];
  }
  return keys;
}

// |image| as a PNG at its pixels' size, which WebKit's snapshot draws at the display's scale, in
// sRGB, as the app reads a PNG's pixels and as the core gives Chromium's pages.
std::optional<std::vector<uint8_t>> SrgbPng(NSImage* image) {
  CGImageRef pixels = [image CGImageForProposedRect:nullptr context:nil hints:nil];
  if (!pixels) {
    return std::nullopt;
  }
  NSBitmapImageRep* bitmap = [[NSBitmapImageRep alloc] initWithCGImage:pixels];
  bitmap = [bitmap bitmapImageRepByConvertingToColorSpace:NSColorSpace.sRGBColorSpace
                                          renderingIntent:NSColorRenderingIntentDefault] ?: bitmap;
  NSData* png = [bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
  if (!png.length) {
    return std::nullopt;
  }
  const uint8_t* bytes = static_cast<const uint8_t*>(png.bytes);
  return std::vector<uint8_t>(bytes, bytes + png.length);
}

}  // namespace

// The view the window shows in the tab's pane, holding the WebKit page. A page full screen leaves
// it for WebKit's own window and comes back, while this stays in the core's window. AppKit offers
// it every key equivalent in the window before the page has it, as Chromium's page hands the
// browser's shortcuts over (AGENTS.md › Keyboard with the page focused).
@interface BrowserCoreSystemPageView : NSView {
 @public
  browser_core::platform::SystemPageMac* _owner;
}
@end

// WebKit's delegates, which tell the page's C++ side.
@interface BrowserCoreWebKitDelegate
    : NSObject <WKNavigationDelegate, WKUIDelegate, WKScriptMessageHandler> {
 @public
  browser_core::platform::SystemPageMac* _owner;
}
@end

namespace browser_core::platform {

class SystemPageMac : public SystemPage {
 public:
  SystemPageMac(WKWebsiteDataStore* store, const GURL& url, Delegate* delegate)
      : delegate_(delegate), first_url_(url) {
    WKWebViewConfiguration* config = [[WebKitClass(@"WKWebViewConfiguration") alloc] init];
    config.websiteDataStore = store;
    config.applicationNameForUserAgent = SafariName();
    // A video's full screen button takes it full screen in WebKit's own window.
    config.preferences.elementFullscreenEnabled = YES;
    // No site-safety lookups with Apple's service, as Chromium's pages make none (#2322).
    config.preferences.fraudulentWebsiteWarningEnabled = NO;
    web_ = [[WebKitClass(@"WKWebView") alloc] initWithFrame:NSZeroRect configuration:config];
    web_.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    web_.allowsMagnification = YES;
    if (@available(macOS 13.3, *)) {
      // The user's: Safari's Develop menu inspects it, as DevTools can't.
      web_.inspectable = YES;
    }
    // As Chromium's pages are, for the checks.
    if (base::CommandLine::ForCurrentProcess()->HasSwitch(switches::kMuteAudio)) {
      SetMuted(true);
    }
    delegates_ = [[BrowserCoreWebKitDelegate alloc] init];
    delegates_->_owner = this;
    web_.navigationDelegate = delegates_;
    web_.UIDelegate = delegates_;
    [web_.configuration.userContentController addScriptMessageHandler:delegates_
                                                          contentWorld:PageWorld()
                                                                  name:kOutputRestricted];
    WatchOutput();
    view_ = [[BrowserCoreSystemPageView alloc] initWithFrame:NSZeroRect];
    view_->_owner = this;
    web_.frame = view_.bounds;
    [view_ addSubview:web_];
    for (NSString* key in ObservedKeys(web_)) {
      [web_ addObserver:delegates_ forKeyPath:key options:0 context:nullptr];
    }
  }

  ~SystemPageMac() override {
    for (NSString* key in ObservedKeys(web_)) {
      [web_ removeObserver:delegates_ forKeyPath:key];
    }
    web_.navigationDelegate = nil;
    web_.UIDelegate = nil;
    [web_.configuration.userContentController
        removeScriptMessageHandlerForName:kOutputRestricted
                             contentWorld:PageWorld()];
    delegates_->_owner = nullptr;
    view_->_owner = nullptr;
    // Out of full screen and Picture in Picture, whose windows would keep the page.
    [web_ closeAllMediaPresentationsWithCompletionHandler:^{
    }];
    [web_ stopLoading];
    [web_ removeFromSuperview];
    [view_ removeFromSuperview];
    // Let go on the next turn: the tab can go inside a call the page made, such as ⌘W passed on
    // from a key equivalent, with these still on the stack.
    WKWebView* web = web_;
    BrowserCoreSystemPageView* view = view_;
    BrowserCoreWebKitDelegate* delegates = delegates_;
    dispatch_async(dispatch_get_main_queue(), ^{
      [web self];
      [view self];
      [delegates self];
    });
  }

  // SystemPage:
  gfx::NativeView view() override { return gfx::NativeView(view_); }

  void Focus() override {
    NSWindow* window = view_.window;
    if (window && web_.window == window) {
      [window makeFirstResponder:web_];
    }
  }

  void Start(StorageByOrigin storage) override {
    if (started_) {
      return;
    }
    started_ = true;
    if (NSString* source = storage.empty() ? nil : StorageScript(storage)) {
      WKUserScript* script = [[WebKitClass(@"WKUserScript") alloc]
            initWithSource:source
             injectionTime:WKUserScriptInjectionTimeAtDocumentStart
          forMainFrameOnly:NO
            inContentWorld:BrowserWorld()];
      [web_.configuration.userContentController addUserScript:script];
      seeding_ = true;
    }
    Load(first_url_);
  }

  void GetStorage(base::OnceCallback<void(url::Origin, StorageItems)> done) override {
    // Whichever comes first, the page's answer or the wait's end, answers.
    __block base::OnceCallback<void(url::Origin, StorageItems)> then = std::move(done);
    [web_ evaluateJavaScript:kReadStorage
                     inFrame:nil
              inContentWorld:BrowserWorld()
           completionHandler:^(id result, NSError* error) {
             if (!then) {
               return;
             }
             NSArray* answer = [result isKindOfClass:[NSArray class]] ? result : nil;
             NSString* origin = answer.count == 2 && [answer[0] isKindOfClass:[NSString class]]
                                    ? answer[0]
                                    : nil;
             NSArray* pairs = origin && [answer[1] isKindOfClass:[NSArray class]] ? answer[1] : nil;
             StorageItems items;
             for (NSArray* pair in pairs) {
               if ([pair isKindOfClass:[NSArray class]] && pair.count == 2 &&
                   [pair[0] isKindOfClass:[NSString class]] &&
                   [pair[1] isKindOfClass:[NSString class]]) {
                 items[base::SysNSStringToUTF16(pair[0])] = base::SysNSStringToUTF16(pair[1]);
               }
             }
             std::move(then).Run(
                 url::Origin::Create(GURL(base::SysNSStringToUTF8(origin ?: @""))),
                 std::move(items));
           }];
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, kStorageWaitSeconds * NSEC_PER_SEC),
                   dispatch_get_main_queue(), ^{
                     if (then) {
                       std::move(then).Run(url::Origin(), StorageItems());
                     }
                   });
  }

  void Snapshot(base::OnceCallback<void(std::optional<std::vector<uint8_t>>)> done) override {
    __block base::OnceCallback<void(std::optional<std::vector<uint8_t>>)> then = std::move(done);
    [web_ takeSnapshotWithConfiguration:nil
                      completionHandler:^(NSImage* image, NSError* error) {
                        std::move(then).Run(image ? SrgbPng(image) : std::nullopt);
                      }];
  }

  // Until Start, the URL it will load, which WebKit reports once its load starts.
  GURL url() override { return web_.URL ? net::GURLWithNSURL(web_.URL) : first_url_; }
  std::u16string title() override { return base::SysNSStringToUTF16(web_.title ?: @""); }
  bool loading() override { return !started_ || web_.loading; }
  bool can_go_back() override { return web_.canGoBack; }
  bool can_go_forward() override { return web_.canGoForward; }
  bool secure() override {
    return [web_.URL.scheme isEqualToString:@"https"] && web_.hasOnlySecureContent;
  }
  bool audible() override {
    SEL playing = NSSelectorFromString(@"_isPlayingAudio");
    return [web_ respondsToSelector:playing] &&
           reinterpret_cast<BOOL (*)(id, SEL)>(objc_msgSend)(web_, playing);
  }
  bool muted() override { return muted_; }

  void Load(const GURL& url) override {
    if (!started_) {
      first_url_ = url;
      return;
    }
    if (NSURL* page = net::NSURLWithGURL(url)) {
      [web_ loadRequest:[NSURLRequest requestWithURL:page]];
    }
  }
  void GoBack() override { [web_ goBack]; }
  void GoForward() override { [web_ goForward]; }
  void Reload(bool bypass_cache) override {
    if (bypass_cache) {
      [web_ reloadFromOrigin];
    } else {
      [web_ reload];
    }
  }
  void Stop() override { [web_ stopLoading]; }

  void SetMuted(bool muted) override {
    // WebKit's own, not public: _WKMediaAudioMuted mutes the whole page, Web Audio included, as
    // Chrome's speaker button does.
    SEL mute = NSSelectorFromString(@"_setPageMuted:");
    if (![web_ respondsToSelector:mute]) {
      return;
    }
    reinterpret_cast<void (*)(id, SEL, NSUInteger)>(objc_msgSend)(web_, mute, muted ? 1 : 0);
    muted_ = muted;
    delegate_->SystemPageChanged();
  }

  // From the views and delegates above.
  void Changed() { delegate_->SystemPageChanged(); }
  void Committed() { Soon(base::BindOnce(&Delegate::SystemPageCommitted)); }
  void Loaded() {
    Seeded();
    Soon(base::BindOnce(&Delegate::SystemPageLoaded));
  }
  // The first load ended, past its commit: the storage it took is set, and later loads take none.
  void Seeded() {
    if (seeding_) {
      seeding_ = false;
      [web_.configuration.userContentController removeAllUserScripts];
      WatchOutput();
    }
  }
  void OutputRestricted() { Soon(base::BindOnce(&Delegate::SystemPageOutputRestricted)); }
  void Open(const GURL& url) {
    Soon(base::BindOnce([](const GURL& url, Delegate* delegate) { delegate->SystemPageOpen(url); },
                        url));
  }
  // As Chromium's page's click is: the window is part way through the event.
  void ClickedSoon() { Soon(base::BindOnce(&Delegate::SystemPageClicked)); }

  // Whether the page starts loading |url| in its main frame, not as a redirect: a link the user
  // followed to a page of its own site does, and the tab takes another site's. Any other
  // navigation, a form's, a script's or the tab's own, loads where it goes.
  bool Navigates(const GURL& url, bool link) {
    const GURL from = net::GURLWithNSURL(web_.URL);
    link_from_ = link ? from : GURL();
    if (!link || SameSite(url, from)) {
      return true;
    }
    Leave(url);
    return false;
  }

  // Whether the page shows the main frame's answer from |url|, where its redirects ended. A link
  // the user followed on the page's site that ended at another site leaves for it, as a link
  // there would; one through other sites and back, as a sign-in goes, stays.
  bool Lands(const GURL& url) {
    const GURL from = std::exchange(link_from_, GURL());
    if (!from.is_valid() || !url.SchemeIsHTTPOrHTTPS() || SameSite(url, from)) {
      return true;
    }
    Leave(url);
    return false;
  }

  bool Key(NSEvent* event, bool before_page) {
    const input::NativeWebKeyboardEvent key{base::apple::OwnedNSEvent(event)};
    // WebKit gives the window's input context only to a page whose focused element takes text,
    // so input methods leave the rest alone.
    return delegate_->SystemPageKey(key, before_page, web_.inputContext != nil);
  }

  bool Focused() {
    NSResponder* responder = view_.window.firstResponder;
    return [responder isKindOfClass:[NSView class]] &&
           [static_cast<NSView*>(responder) isDescendantOf:web_];
  }

  // The content process went: crashed, or killed for memory. WebKit shows nothing until a load.
  void Gone() {
    // An error, as Chromium's tab's is.
    LOG(ERROR) << "browser: system page gone at " << url().spec();
    [web_ reload];
  }

 private:
  static bool SameSite(const GURL& a, const GURL& b) {
    return net::registry_controlled_domains::SameDomainOrHost(
        a, b, net::registry_controlled_domains::INCLUDE_PRIVATE_REGISTRIES);
  }

  // Has every document watch its protected video, from its start.
  void WatchOutput() {
    WKUserScript* script = [[WebKitClass(@"WKUserScript") alloc]
          initWithSource:kWatchOutput
           injectionTime:WKUserScriptInjectionTimeAtDocumentStart
        forMainFrameOnly:NO
          inContentWorld:PageWorld()];
    [web_.configuration.userContentController addUserScript:script];
  }

  void Leave(const GURL& url) {
    Soon(base::BindOnce([](const GURL& url, Delegate* delegate) { delegate->SystemPageLeft(url); },
                        url));
  }

  // Tells the tab from a task of its own: WebKit is part way through its call, and the tab may let
  // the page go.
  void Soon(base::OnceCallback<void(Delegate*)> call) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(
                       [](base::WeakPtr<SystemPageMac> page,
                          base::OnceCallback<void(Delegate*)> call) {
                         if (page) {
                           std::move(call).Run(page->delegate_.get());
                         }
                       },
                       weak_factory_.GetWeakPtr(), std::move(call)));
  }

  const raw_ptr<Delegate> delegate_;
  WKWebView* __strong web_;
  BrowserCoreSystemPageView* __strong view_;
  BrowserCoreWebKitDelegate* __strong delegates_;
  // What the first load loads, once Start has the page's storage in place.
  GURL first_url_;
  bool started_ = false;
  // The first load's storage script is in place.
  bool seeding_ = false;
  // The page a link the user followed in the main frame left, until its answer comes.
  GURL link_from_;
  bool muted_ = false;
  base::WeakPtrFactory<SystemPageMac> weak_factory_{this};
};

}  // namespace browser_core::platform

@implementation BrowserCoreSystemPageView {
  // The key equivalent last passed to the page, which WebKit sends round again if the page
  // doesn't keep it. Known by when it was typed and what it is: the app may hold it while a bar
  // opens and post it again, and AppKit hands a posted event back as a new one.
  BOOL _sent;
  NSTimeInterval _sentTime;
  unsigned short _sentKey;
  NSEventModifierFlags _sentFlags;
  NSTimeInterval _lastClick;
}

- (BOOL)isFlipped {
  return YES;
}

- (BOOL)performKeyEquivalent:(NSEvent*)event {
  if (!_owner || event.type != NSEventTypeKeyDown || !_owner->Focused()) {
    return [super performKeyEquivalent:event];
  }
  const NSEventModifierFlags flags =
      event.modifierFlags & NSEventModifierFlagDeviceIndependentFlagsMask;
  const bool unkept = _sent && event.timestamp == _sentTime && event.keyCode == _sentKey &&
                      flags == _sentFlags;
  _sent = NO;
  if (_owner->Key(event, /*before_page=*/!unkept)) {
    return YES;
  }
  // The page didn't keep it, and the app didn't use it: the menus get it next.
  if (unkept) {
    return NO;
  }
  _sent = YES;
  _sentTime = event.timestamp;
  _sentKey = event.keyCode;
  _sentFlags = flags;
  return [super performKeyEquivalent:event];
}

// A click in the page, for the split view's focus. The page's view takes the event itself, so it's
// seen as the window finds the view it goes to.
- (NSView*)hitTest:(NSPoint)point {
  NSView* hit = [super hitTest:point];
  NSEvent* event = NSApp.currentEvent;
  if (_owner && hit && event.type == NSEventTypeLeftMouseDown && event.window == self.window &&
      event.timestamp != _lastClick) {
    _lastClick = event.timestamp;
    _owner->ClickedSoon();
  }
  return hit;
}

@end

@implementation BrowserCoreWebKitDelegate

- (void)observeValueForKeyPath:(NSString*)keyPath
                      ofObject:(id)object
                        change:(NSDictionary*)change
                       context:(void*)context {
  if (_owner) {
    _owner->Changed();
  }
}

- (void)webView:(WKWebView*)web
    decidePolicyForNavigationAction:(WKNavigationAction*)action
                    decisionHandler:(void (^)(WKNavigationActionPolicy))decide {
  const GURL url = net::GURLWithNSURL(action.request.URL);
  // Frames load as they do. A new window's link comes to createWebView below.
  if (!_owner || !action.targetFrame.mainFrame) {
    decide(WKNavigationActionPolicyAllow);
    return;
  }
  // No downloads in this engine, nor other apps' schemes, such as mailto: the page goes nowhere.
  if (action.shouldPerformDownload ||
      !(url.SchemeIsHTTPOrHTTPS() || url.SchemeIs("about") || url.SchemeIs("blob"))) {
    LOG(WARNING) << "browser: system page refused " << url.possibly_invalid_spec();
    decide(WKNavigationActionPolicyCancel);
    return;
  }
  // A server's redirect goes where it goes, and its answer says where the navigation ended.
  if (IsRedirect(action)) {
    decide(WKNavigationActionPolicyAllow);
    return;
  }
  // A link the user followed to another site can leave the engine. A form and the page's own
  // script stay in it: sign-ins go through other sites.
  if (!_owner->Navigates(url, action.navigationType == WKNavigationTypeLinkActivated)) {
    decide(WKNavigationActionPolicyCancel);
    return;
  }
  decide(WKNavigationActionPolicyAllow);
}

- (void)webView:(WKWebView*)web
    decidePolicyForNavigationResponse:(WKNavigationResponse*)response
                      decisionHandler:(void (^)(WKNavigationResponsePolicy))decide {
  if (_owner && response.forMainFrame &&
      !_owner->Lands(net::GURLWithNSURL(response.response.URL))) {
    decide(WKNavigationResponsePolicyCancel);
    return;
  }
  // What the page can't show would be a download, which this engine doesn't do.
  if (response.forMainFrame && !response.canShowMIMEType) {
    LOG(WARNING) << "browser: system page refused a download of "
                 << net::GURLWithNSURL(response.response.URL).possibly_invalid_spec();
    decide(WKNavigationResponsePolicyCancel);
    return;
  }
  decide(WKNavigationResponsePolicyAllow);
}

- (void)webView:(WKWebView*)web didCommitNavigation:(WKNavigation*)navigation {
  if (_owner) {
    _owner->Committed();
  }
}

// The main frame's document and what it loads have finished loading, as Chromium's DidFinishLoad
// reports a page's: not a same-document navigation, nor a load that failed.
- (void)webView:(WKWebView*)web didFinishNavigation:(WKNavigation*)navigation {
  if (_owner) {
    _owner->Loaded();
  }
}

// A load that failed after its commit ends the first load too.
- (void)webView:(WKWebView*)web
    didFailNavigation:(WKNavigation*)navigation
            withError:(NSError*)error {
  if (_owner) {
    _owner->Seeded();
  }
}

- (void)webViewWebContentProcessDidTerminate:(WKWebView*)web {
  if (_owner) {
    _owner->Gone();
  }
}

// A new window opens as a tab of the browser's, at its URL: the page's opener is lost, and a
// site the engine is kept for comes back to it there.
- (WKWebView*)webView:(WKWebView*)web
    createWebViewWithConfiguration:(WKWebViewConfiguration*)config
               forNavigationAction:(WKNavigationAction*)action
                    windowFeatures:(WKWindowFeatures*)features {
  const GURL url = net::GURLWithNSURL(action.request.URL);
  if (_owner && url.SchemeIsHTTPOrHTTPS()) {
    _owner->Open(url);
  }
  return nil;
}

- (void)userContentController:(WKUserContentController*)controller
      didReceiveScriptMessage:(WKScriptMessage*)message {
  if (_owner && [message.name isEqualToString:kOutputRestricted]) {
    _owner->OutputRestricted();
  }
}

// The camera and the microphone are refused: this engine has no question to ask the user.
- (void)webView:(WKWebView*)web
    requestMediaCapturePermissionForOrigin:(WKSecurityOrigin*)origin
                          initiatedByFrame:(WKFrameInfo*)frame
                                      type:(WKMediaCaptureType)type
                           decisionHandler:(void (^)(WKPermissionDecision))decide {
  decide(WKPermissionDecisionDeny);
}

// Nor does it pick files for the page.
- (void)webView:(WKWebView*)web
    runOpenPanelWithParameters:(WKOpenPanelParameters*)parameters
              initiatedByFrame:(WKFrameInfo*)frame
             completionHandler:(void (^)(NSArray<NSURL*>*))done {
  done(nil);
}

@end

namespace browser_core::platform {

// static
bool SystemPage::Available() {
  if (@available(macOS 14, *)) {
    return true;
  }
  return false;
}

// static
std::unique_ptr<SystemPage> SystemPage::Create(const SystemStore& store,
                                               const GURL& url,
                                               Delegate* delegate) {
  if (!Available() || !LoadWebKit("a page") || !WebKitClass(@"WKWebView")) {
    return nullptr;
  }
  WKWebsiteDataStore* data = StoreFor(store);
  return data ? std::make_unique<SystemPageMac>(data, url, delegate) : nullptr;
}

// static
void SystemPage::DeleteStore(const std::string& id) {
  NSString* key = base::SysUTF8ToNSString(id);
  NSUUID* uuid = [[NSUUID alloc] initWithUUIDString:key];
  if (!uuid || !Available() || !LoadWebKit("deleting a profile's store")) {
    return;
  }
  // Ours lets go first: WebKit won't delete a store in use.
  [Stores() removeObjectForKey:key];
  if (@available(macOS 14, *)) {
    [WebKitClass(@"WKWebsiteDataStore")
        removeDataStoreForIdentifier:uuid
                   completionHandler:^(NSError* error) {
                     if (error) {
                       VLOG(1) << "browser: system store " << base::SysNSStringToUTF8(key)
                               << " not deleted: "
                               << base::SysNSStringToUTF8(error.localizedDescription);
                     }
                   }];
  }
}

// static
void SystemPage::ReleaseStore(const std::string& id) {
  [Stores() removeObjectForKey:base::SysUTF8ToNSString(id)];
}

// static
void SystemPage::ClearSite(const SystemStore& store,
                           const std::string& domain,
                           SiteData what,
                           base::OnceClosure done) {
  WKWebsiteDataStore* data =
      Available() && LoadWebKit("clearing a site") ? StoreFor(store) : nil;
  NSSet<NSString*>* types = nil;
  switch (what) {
    case SiteData::kCookies:
      types = [NSSet setWithObjects:WebKitConstant("WKWebsiteDataTypeCookies"), nil];
      break;
    case SiteData::kCache:
      types = [NSSet setWithObjects:WebKitConstant("WKWebsiteDataTypeDiskCache"),
                                    WebKitConstant("WKWebsiteDataTypeMemoryCache"), nil];
      break;
    case SiteData::kAll:
      types = [WebKitClass(@"WKWebsiteDataStore") allWebsiteDataTypes];
      break;
  }
  if (!data || types.count == 0) {
    std::move(done).Run();
    return;
  }
  __block base::OnceClosure then = std::move(done);
  NSString* site = base::SysUTF8ToNSString(domain);
  [data fetchDataRecordsOfTypes:types
              completionHandler:^(NSArray<WKWebsiteDataRecord*>* records) {
                // WebKit names a record by its site's registrable domain, as the core's filter
                // takes it.
                NSMutableArray<WKWebsiteDataRecord*>* of_site = [NSMutableArray array];
                for (WKWebsiteDataRecord* record in records) {
                  if ([record.displayName isEqualToString:site]) {
                    [of_site addObject:record];
                  }
                }
                [data removeDataOfTypes:types
                         forDataRecords:of_site
                      completionHandler:^{
                        std::move(then).Run();
                      }];
              }];
}

// static
void SystemPage::SetCookies(const SystemStore& store,
                            const std::vector<net::CanonicalCookie>& cookies,
                            base::OnceCallback<void(size_t)> done) {
  WKWebsiteDataStore* data =
      Available() && !cookies.empty() && LoadWebKit("copying cookies") ? StoreFor(store) : nil;
  NSMutableArray<NSHTTPCookie*>* made = [NSMutableArray array];
  for (const net::CanonicalCookie& cookie : cookies) {
    if (NSHTTPCookie* each = WebKitCookie(cookie)) {
      [made addObject:each];
    }
  }
  if (!data || made.count == 0) {
    std::move(done).Run(0);
    return;
  }
  __block base::OnceCallback<void(size_t)> then = std::move(done);
  __block NSUInteger left = made.count;
  for (NSHTTPCookie* cookie in made) {
    [data.httpCookieStore setCookie:cookie
                  completionHandler:^{
                    if (--left == 0) {
                      std::move(then).Run(made.count);
                    }
                  }];
  }
}

// static
void SystemPage::GetCookies(const SystemStore& store,
                            const std::string& site,
                            base::OnceCallback<void(std::vector<net::CanonicalCookie>)> done) {
  WKWebsiteDataStore* data = Available() && LoadWebKit("copying cookies") ? StoreFor(store) : nil;
  if (!data) {
    std::move(done).Run({});
    return;
  }
  __block base::OnceCallback<void(std::vector<net::CanonicalCookie>)> then = std::move(done);
  // A copy: a block captures a reference as the reference, and the caller's string is gone by the
  // time the store answers.
  const std::string of = site;
  [data.httpCookieStore getAllCookies:^(NSArray<NSHTTPCookie*>* all) {
    std::vector<net::CanonicalCookie> of_site;
    size_t matched = 0;
    for (NSHTTPCookie* cookie in all) {
      std::string domain = base::SysNSStringToUTF8(cookie.domain);
      if (!HostOfSite(domain.starts_with('.') ? domain.substr(1) : domain, of)) {
        continue;
      }
      ++matched;
      if (std::unique_ptr<net::CanonicalCookie> made = ChromiumCookie(cookie)) {
        of_site.push_back(std::move(*made));
      }
    }
    // Counts only: never a name or a value. With the copy back's own line, it says which step
    // dropped a cookie.
    fprintf(stderr, "browser: system store has %lu cookies, %zu of %s, %zu Chromium takes\n",
            static_cast<unsigned long>(all.count), matched, of.c_str(), of_site.size());
    std::move(then).Run(std::move(of_site));
  }];
}

}  // namespace browser_core::platform
