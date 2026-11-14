#include "browser_core/boosts.h"

#include <memory>
#include <optional>
#include <string>
#include <utility>

#include "base/functional/bind.h"
#include "base/json/string_escape.h"
#include "base/logging.h"
#include "base/no_destructor.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/utf_string_conversions.h"
#include "browser_core/arrange.h"
#include "browser_core/host.h"
#include "browser_core/profiles.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_keyed_service_factory.h"
#include "chrome/common/chrome_isolated_world_ids.h"
#include "components/keyed_service/core/keyed_service.h"
#include "components/pref_registry/pref_registry_syncable.h"
#include "components/prefs/pref_service.h"
#include "content/public/browser/host_zoom_map.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/render_widget_host_view.h"
#include "content/public/browser/web_contents.h"
#include "third_party/blink/public/common/input/web_input_event.h"
#include "third_party/blink/public/common/page/page_zoom.h"

namespace browser_core {

namespace {

using Type = blink::WebInputEvent::Type;

// Chrome's world for its own features, which ilo's Chrome doesn't otherwise run script in: the
// page's scripts can't see what runs there, though they share the document.
constexpr int kWorld = ISOLATED_WORLD_ID_CHROME_INTERNAL;

// The Boost's script, called with what to do. "css" adopts a style sheet with |css|, or drops it
// for none. "zap" outlines what's under the pointer, with a crosshair, until "unzap". "at" names
// what's at |x|, |y| in the viewport, in CSS pixels, by a selector from the nearest unique id or
// the body, by tag and place among its kind, or null for the body or nothing.
constexpr char16_t kScript[] =
    uR"js((function (op, css, x, y) {
  const boost = (globalThis.__iloBoost ??= {});
  const adopt = (sheet, on) => {
    const sheets = document.adoptedStyleSheets;
    const has = sheets.includes(sheet);
    if (on && !has) document.adoptedStyleSheets = [...sheets, sheet];
    if (!on && has) document.adoptedStyleSheets = sheets.filter((each) => each !== sheet);
  };
  const zappable = (element) =>
    element && element !== document.documentElement && element !== document.body &&
    element !== boost.zap?.outline;
  if (op === "css") {
    boost.sheet ??= new CSSStyleSheet();
    try {
      boost.sheet.replaceSync(css);
    } catch (error) {
      boost.sheet.replaceSync("");
    }
    adopt(boost.sheet, css !== "");
    return null;
  }
  if (op === "zap") {
    if (boost.zap) return null;
    const outline = document.createElement("div");
    outline.style.cssText =
      "all:initial;position:fixed;display:none;pointer-events:none;z-index:2147483647;" +
      "box-sizing:border-box;border:2px solid #289054;border-radius:4px;" +
      "background:rgba(40,144,84,.14)";
    const cursor = new CSSStyleSheet();
    cursor.replaceSync("*{cursor:crosshair!important}");
    const move = (event) => {
      const target = document.elementFromPoint(event.clientX, event.clientY);
      if (!zappable(target)) {
        outline.style.display = "none";
        return;
      }
      const box = target.getBoundingClientRect();
      Object.assign(outline.style, {
        display: "block",
        left: box.left + "px",
        top: box.top + "px",
        width: box.width + "px",
        height: box.height + "px",
      });
    };
    addEventListener("mousemove", move, {capture: true, passive: true});
    document.documentElement.append(outline);
    adopt(cursor, true);
    boost.zap = {outline, cursor, move};
    return null;
  }
  if (op === "unzap") {
    if (!boost.zap) return null;
    removeEventListener("mousemove", boost.zap.move, {capture: true});
    boost.zap.outline.remove();
    adopt(boost.zap.cursor, false);
    boost.zap = null;
    return null;
  }
  const target = document.elementFromPoint(x, y);
  if (!zappable(target)) return null;
  if (boost.zap) boost.zap.outline.style.display = "none";
  const parts = [];
  for (let node = target; node && node !== document.body; node = node.parentElement) {
    if (node.id && !/^\d|\d{4,}/.test(node.id)) {
      const id = "#" + CSS.escape(node.id);
      if (document.querySelectorAll(id).length === 1) {
        parts.unshift(id);
        return parts.join(" > ");
      }
    }
    const kind = CSS.escape(node.localName);
    const kin = [...(node.parentElement?.children ?? [])].filter(
      (each) => each.localName === node.localName);
    parts.unshift(kin.length > 1 ? `${kind}:nth-of-type(${kin.indexOf(node) + 1})` : kind);
  }
  parts.unshift("body");
  return parts.join(" > ");
}))js";

// The CSS of each profile's Boosts, by profile and site.
std::map<bc_id, std::map<std::string, std::string, std::less<>>>& Store() {
  static base::NoDestructor<std::map<bc_id, std::map<std::string, std::string, std::less<>>>> store;
  return *store;
}

// The profile whose Boosts |contents| takes: a private window's takes its original's.
bc_id ProfileIdOf(content::WebContents* contents) {
  Profile* profile = Profile::FromBrowserContext(contents->GetBrowserContext());
  return profile ? ProfileId(DirOf(profile->GetOriginalProfile())) : 0;
}

// The script the app read from each profile's Boost files, by profile and site. It runs only while
// the record says these bytes are the ones turned on.
std::map<bc_id, std::map<std::string, std::string, std::less<>>>& Scripts() {
  static base::NoDestructor<std::map<bc_id, std::map<std::string, std::string, std::less<>>>>
      scripts;
  return *scripts;
}

// Builds no service: Chrome registers a profile's prefs through its factories.
class PrefsFactory : public ProfileKeyedServiceFactory {
 public:
  static PrefsFactory* GetInstance() {
    static base::NoDestructor<PrefsFactory> instance;
    return instance.get();
  }

  PrefsFactory() : ProfileKeyedServiceFactory("BrowserCoreBoostScripts") {}

 private:
  std::unique_ptr<KeyedService> BuildServiceInstanceForBrowserContext(
      content::BrowserContext* context) const override {
    return nullptr;
  }

  void RegisterProfilePrefs(user_prefs::PrefRegistrySyncable* registry) override {
    boost_script::RegisterPrefs(registry);
  }
};

// Whether |frame| and every frame it's in show |site|: a script never runs in a frame another
// site's page holds, nor in one whose origin is opaque, as a sandboxed or srcdoc frame's is.
bool AllOf(content::RenderFrameHost* frame, std::string_view site) {
  for (; frame; frame = frame->GetParentOrOuterDocument()) {
    if (frame->GetLastCommittedOrigin().opaque() ||
        Boosts::SiteOf(frame->GetLastCommittedURL()) != site) {
      return false;
    }
  }
  return true;
}

std::string_view CssFor(bc_id profile, std::string_view site) {
  if (site.empty()) {
    return {};
  }
  auto boosts = Store().find(profile);
  if (boosts == Store().end()) {
    return {};
  }
  auto boost = boosts->second.find(site);
  return boost == boosts->second.end() ? std::string_view() : std::string_view(boost->second);
}

void Run(content::RenderFrameHost* frame,
         std::string_view op,
         std::string_view css,
         double x,
         double y,
         content::RenderFrameHost::JavaScriptResultCallback done) {
  if (!frame || !frame->IsRenderFrameLive()) {
    if (done) {
      std::move(done).Run(base::Value());
    }
    return;
  }
  const std::string call = "(" + base::GetQuotedJSONString(op) + "," +
                           base::GetQuotedJSONString(css) + "," + base::NumberToString(x) + "," +
                           base::NumberToString(y) + ")";
  frame->ExecuteJavaScriptInIsolatedWorld(std::u16string(kScript) + base::UTF8ToUTF16(call),
                                          std::move(done), kWorld);
}

// |event|'s point in the page's view: an out-of-process frame's widget has it in its own.
gfx::PointF InPage(content::RenderWidgetHost* widget, const blink::WebMouseEvent& event) {
  content::RenderWidgetHostView* view = widget->GetView();
  return view ? view->TransformPointToRootCoordSpaceF(event.PositionInWidget())
              : event.PositionInWidget();
}

}  // namespace

Boosts::Boosts(content::WebContents* contents, bc_id tab)
    : content::WebContentsObserver(contents),
      content::WebContentsUserData<Boosts>(*contents),
      tab_(tab) {}

Boosts::~Boosts() {
  Unhook();
}

// static
std::string Boosts::SiteOf(const GURL& url) {
  if (!url.SchemeIsHTTPOrHTTPS() || url.host().empty()) {
    return {};
  }
  std::string_view host = url.host();
  if (host.starts_with("www.") && host.size() > 4) {
    host.remove_prefix(4);
  }
  return std::string(host);
}

// static
void Boosts::Set(bc_id profile, std::string_view site, std::string_view css) {
  if (site.empty()) {
    return;
  }
  auto& boosts = Store()[profile];
  if (css.empty()) {
    boosts.erase(std::string(site));
  } else {
    boosts.insert_or_assign(std::string(site), std::string(css));
  }
  for (Tab* tab : Tab::All()) {
    content::WebContents* contents = tab->web_contents();
    Boosts* each = contents ? Boosts::FromWebContents(contents) : nullptr;
    if (!each || ProfileIdOf(contents) != profile) {
      continue;
    }
    contents->ForEachRenderFrameHost([&](content::RenderFrameHost* frame) {
      if (SiteOf(frame->GetLastCommittedURL()) == site) {
        each->Apply(frame);
      }
    });
  }
}

// static
boost_script::State Boosts::SetScript(bc_id profile,
                                      std::string_view site,
                                      std::optional<std::string_view> script) {
  auto& scripts = Scripts()[profile];
  // One too large never runs, so it isn't kept.
  if (!script || site.empty() || script->size() > boost_script::kMaxBytes) {
    scripts.erase(std::string(site));
  } else {
    scripts.insert_or_assign(std::string(site), std::string(*script));
  }
  return ScriptState(profile, site, script.value_or(std::string_view()));
}

// static
boost_script::State Boosts::ScriptState(bc_id profile,
                                        std::string_view site,
                                        std::string_view script) {
  if (script.size() > boost_script::kMaxBytes) {
    return boost_script::State::kTooLarge;
  }
  Profile* loaded = ProfileOrLoad(profile);
  return loaded ? boost_script::Check(*loaded->GetPrefs(), site, script)
                : boost_script::State::kNew;
}

// static
bool Boosts::TurnOnScript(bc_id profile, std::string_view site, std::string_view script) {
  Profile* loaded = ProfileOrLoad(profile);
  if (!loaded || !boost_script::TurnOn(*loaded->GetPrefs(), site, script)) {
    return false;
  }
  // What runs is what the user was shown, even if the file changed while the dialog was open.
  Scripts()[profile].insert_or_assign(std::string(site), std::string(script));
  LOG(ERROR) << "browser: boost script turned on " << site;
  return true;
}

// static
void Boosts::ForgetScript(bc_id profile, std::string_view site) {
  if (Profile* loaded = ProfileOrLoad(profile)) {
    boost_script::Forget(*loaded->GetPrefs(), site);
    LOG(ERROR) << "browser: boost script turned off " << site;
  }
}

// static
void Boosts::RegisterPrefs() {
  PrefsFactory::GetInstance();
}

void Boosts::RunScript(content::RenderFrameHost* frame) {
  content::WebContents* contents = web_contents();
  Profile* profile = Profile::FromBrowserContext(contents->GetBrowserContext());
  if (!profile || profile->IsOffTheRecord()) {
    return;
  }
  // An agent's tab acts in the user's sessions, never with the user's own code (#2297's 1.5).
  if (Space* space = Space::Holding(tab_); space && space->agent()) {
    return;
  }
  const std::string site = SiteOf(frame->GetLastCommittedURL());
  auto scripts = Scripts().find(ProfileIdOf(contents));
  if (site.empty() || scripts == Scripts().end()) {
    return;
  }
  auto script = scripts->second.find(site);
  if (script == scripts->second.end() || !AllOf(frame, site) ||
      boost_script::Check(*profile->GetPrefs(), site, script->second) != boost_script::State::kOn) {
    return;
  }
  // The bytes that were hashed, as UTF-8: an invalid sequence becomes U+FFFD, as a file would.
  frame->ExecuteJavaScriptInIsolatedWorld(base::UTF8ToUTF16(script->second),
                                          content::RenderFrameHost::JavaScriptResultCallback(),
                                          ISOLATED_WORLD_ID_BOOST_SCRIPT);
  LOG(ERROR) << "browser: tab " << tab_ << " boost script ran " << site;
}

void Boosts::Apply(content::RenderFrameHost* frame) {
  const std::string_view css =
      CssFor(ProfileIdOf(web_contents()), SiteOf(frame->GetLastCommittedURL()));
  Run(frame, "css", css, 0, 0, content::RenderFrameHost::JavaScriptResultCallback());
}

void Boosts::Zap(bool on) {
  if (on == zapping_) {
    return;
  }
  if (!on) {
    EndZap(false);
    return;
  }
  content::RenderFrameHost* page = web_contents()->GetPrimaryMainFrame();
  if (!page || SiteOf(page->GetLastCommittedURL()).empty()) {
    // Nothing to zap on a page that isn't http(s): the app hears it ended.
    EndZap(true);
    return;
  }
  // Arrange takes the page's presses too.
  if (Arrange* arrange = Arrange::FromWebContents(web_contents())) {
    arrange->End(/*tell=*/true);
  }
  zapping_ = true;
  web_contents()->ForEachRenderFrameHost(
      [this](content::RenderFrameHost* frame) { Hook(frame->GetRenderWidgetHost()); });
  Run(page, "zap", "", 0, 0, content::RenderFrameHost::JavaScriptResultCallback());
  LOG(ERROR) << "browser: tab " << tab_ << " zap on";
}

void Boosts::EndZap(bool tell) {
  const bool was = zapping_;
  zapping_ = false;
  Unhook();
  if (was) {
    Run(web_contents()->GetPrimaryMainFrame(), "unzap", "", 0, 0,
        content::RenderFrameHost::JavaScriptResultCallback());
    LOG(ERROR) << "browser: tab " << tab_ << " zap off";
  }
  if (tell && Host().tab_boost_zap_ended) {
    Host().tab_boost_zap_ended(Host().context, tab_);
  }
}

void Boosts::Hook(content::RenderWidgetHost* widget) {
  // A frame in its parent's process shares its widget.
  if (!widget || widgets_.IsObservingSource(widget)) {
    return;
  }
  widgets_.AddObservation(widget);
  auto callback =
      base::BindRepeating(&Boosts::OnMouse, base::Unretained(this), base::Unretained(widget));
  widget->AddMouseEventCallback(callback);
  hooked_.emplace(widget, std::move(callback));
}

void Boosts::Unhook() {
  for (auto& [widget, callback] : hooked_) {
    widget->RemoveMouseEventCallback(callback);
  }
  hooked_.clear();
  widgets_.RemoveAllObservations();
}

bool Boosts::OnMouse(content::RenderWidgetHost* widget, const blink::WebMouseEvent& event) {
  // A left press, its drag and its release are Zap's, so the page neither follows a link nor
  // starts a selection. Moves without it reach the page, which the outline follows.
  if (!zapping_ || event.button != blink::WebPointerProperties::Button::kLeft) {
    return false;
  }
  if (event.GetType() != Type::kMouseDown) {
    return event.GetType() == Type::kMouseUp || event.GetType() == Type::kMouseMove;
  }
  // The page's view has it in DIPs; the page lays out in CSS pixels, its zoom apart.
  const gfx::PointF at = InPage(widget, event);
  const double zoom =
      blink::ZoomLevelToZoomFactor(content::HostZoomMap::GetZoomLevel(web_contents()));
  Run(web_contents()->GetPrimaryMainFrame(), "at", "", at.x() / zoom, at.y() / zoom,
      base::BindOnce(&Boosts::Zapped, weak_factory_.GetWeakPtr()));
  return true;
}

void Boosts::Zapped(base::Value selector) {
  const std::string* text = selector.GetIfString();
  LOG(ERROR) << "browser: tab " << tab_ << " zapped " << (text ? *text : "nothing");
  if (Host().tab_boost_zapped) {
    Host().tab_boost_zapped(Host().context, tab_, text ? text->c_str() : nullptr);
  }
}

void Boosts::DidFinishNavigation(content::NavigationHandle* navigation) {
  if (!navigation->HasCommitted() || navigation->IsSameDocument() || navigation->IsErrorPage()) {
    return;
  }
  content::RenderFrameHost* frame = navigation->GetRenderFrameHost();
  // A site with no Boost has nothing to run, but a page from the back-forward cache keeps the sheet
  // it had, which may have changed since.
  if (frame &&
      (navigation->IsServedFromBackForwardCache() ||
       !CssFor(ProfileIdOf(web_contents()), SiteOf(frame->GetLastCommittedURL())).empty())) {
    Apply(frame);
  }
}

void Boosts::DOMContentLoaded(content::RenderFrameHost* frame) {
  RunScript(frame);
}

void Boosts::RenderFrameCreated(content::RenderFrameHost* frame) {
  if (zapping_) {
    Hook(frame->GetRenderWidgetHost());
  }
}

void Boosts::PrimaryPageChanged(content::Page& page) {
  // Zap was the last page's.
  if (zapping_) {
    EndZap(true);
  }
}

void Boosts::WebContentsDestroyed() {
  zapping_ = false;
  Unhook();
}

void Boosts::RenderWidgetHostDestroyed(content::RenderWidgetHost* widget) {
  hooked_.erase(widget);
  widgets_.RemoveObservation(widget);
}

WEB_CONTENTS_USER_DATA_KEY_IMPL(Boosts);

}  // namespace browser_core
