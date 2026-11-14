#ifndef BROWSER_CORE_BOOSTS_H_
#define BROWSER_CORE_BOOSTS_H_

#include <map>
#include <string>
#include <string_view>

#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "base/scoped_multi_source_observation.h"
#include "browser_core/public/browser_core.h"
#include "content/public/browser/render_widget_host.h"
#include "content/public/browser/render_widget_host_observer.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/web_contents_user_data.h"
#include "third_party/blink/public/common/input/web_mouse_event.h"
#include "url/gurl.h"

class Profile;

namespace browser_core {

// Boosts (docs/ux-spec.md › Boosts): the CSS the user gives a site, which its pages and frames
// take as they commit, and at once while they're open, and Zap, which hides what the user clicks.
// The app keeps the Boosts, as plain files, and hands each profile's to the core. The CSS goes in
// as a constructed style sheet adopted from Chrome's internal isolated world, so the page's
// scripts can't reach the browser's code and its Content-Security-Policy, which governs style
// elements, doesn't stop it. Agents get no path: what a Boost changes is the user's.
class Boosts : public content::WebContentsObserver,
               public content::WebContentsUserData<Boosts>,
               public content::RenderWidgetHostObserver {
 public:
  ~Boosts() override;

  // The Boost key of |url|: its host without "www.", for http(s) pages, else empty.
  static std::string SiteOf(const GURL& url);
  // Gives |site| |css| in the profile, empty to take it away, and shows it on the pages of it that
  // are open.
  static void Set(bc_id profile, std::string_view site, std::string_view css);

  // Turns Zap on or off on the tab's page: while it's on, a left press on the page goes to Zap
  // instead of the page, and tab_boost_zapped says what was under it.
  void Zap(bool on);
  bool zapping() const { return zapping_; }
  // Ends Zap, and says so with tab_boost_zap_ended if |tell|: Arrange ends it, as both take the
  // page's presses.
  void EndZap(bool tell);

 private:
  friend class content::WebContentsUserData<Boosts>;

  Boosts(content::WebContents* contents, bc_id tab);

  // Shows the profile's CSS for its site in |frame|, or takes it away.
  void Apply(content::RenderFrameHost* frame);
  void Hook(content::RenderWidgetHost* widget);
  void Unhook();
  bool OnMouse(content::RenderWidgetHost* widget, const blink::WebMouseEvent& event);
  void Zapped(base::Value selector);

  // content::WebContentsObserver:
  void DidFinishNavigation(content::NavigationHandle* navigation) override;
  void RenderFrameCreated(content::RenderFrameHost* frame) override;
  void PrimaryPageChanged(content::Page& page) override;
  void WebContentsDestroyed() override;
  // content::RenderWidgetHostObserver:
  void RenderWidgetHostDestroyed(content::RenderWidgetHost* widget) override;

  const bc_id tab_;
  bool zapping_ = false;
  std::map<raw_ptr<content::RenderWidgetHost>, content::RenderWidgetHost::MouseEventCallback>
      hooked_;
  base::ScopedMultiSourceObservation<content::RenderWidgetHost, content::RenderWidgetHostObserver>
      widgets_{this};
  base::WeakPtrFactory<Boosts> weak_factory_{this};

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

}  // namespace browser_core

#endif  // BROWSER_CORE_BOOSTS_H_
