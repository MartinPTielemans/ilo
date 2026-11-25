#include "browser_core/link_preview.h"

#include <memory>

#include "browser_core/agent_space_page.h"
#include "browser_core/host.h"
#include "browser_core/tab.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/navigation_throttle.h"
#include "content/public/browser/navigation_throttle_registry.h"
#include "content/public/browser/web_contents.h"
#include "net/base/registry_controlled_domains/registry_controlled_domain.h"
#include "ui/base/page_transition_types.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

class LinkPreviewThrottle : public content::NavigationThrottle {
 public:
  using content::NavigationThrottle::NavigationThrottle;

  // content::NavigationThrottle:
  ThrottleCheckResult WillStartRequest() override {
    content::WebContents* contents = navigation_handle()->GetWebContents();
    const GURL& from = contents->GetLastCommittedURL();
    const GURL& to = navigation_handle()->GetURL();
    // A site is a registrable domain, as Clear's is: a link within it stays in the tab, as a
    // pinned tab's own pages do. A tab with no page yet, as one a link opened, has no site.
    if (!Host().tab_link_to_site || !from.SchemeIsHTTPOrHTTPS() || !to.SchemeIsHTTPOrHTTPS() ||
        net::registry_controlled_domains::SameDomainOrHost(
            from, to, net::registry_controlled_domains::INCLUDE_PRIVATE_REGISTRIES)) {
      return PROCEED;
    }
    Tab* tab = Tab::FromWebContents(contents);
    // A page in an agent's Space the user doesn't hold opens none: its link goes on in its tab.
    if (!tab || RefusesInAgentSpace(agent_space_rule::Ask::kLinkPreview, contents)) {
      return PROCEED;
    }
    const gfx::PointF click = tab->last_click();
    // The app only answers here: it opens the preview after this call, outside the navigation.
    return Host().tab_link_to_site(Host().context, Tab::IdOf(contents), to.spec().c_str(),
                                   click.x(), click.y())
               ? CANCEL_AND_IGNORE
               : PROCEED;
  }

  const char* GetNameForLogging() override { return "browser_core::LinkPreviewThrottle"; }
};

}  // namespace

void AddLinkPreviewThrottle(content::NavigationThrottleRegistry& registry) {
  content::NavigationHandle& navigation = registry.GetNavigationHandle();
  // A link opened with a modifier, in a new tab or window, navigates a new page, which has no site
  // yet, and the throttle passes it.
  if (!navigation.IsInPrimaryMainFrame() || !navigation.IsRendererInitiated() ||
      !navigation.HasUserGesture() || navigation.IsPost() ||
      !ui::PageTransitionCoreTypeIs(navigation.GetPageTransition(), ui::PAGE_TRANSITION_LINK) ||
      !Tab::FromWebContents(navigation.GetWebContents())) {
    return;
  }
  registry.AddThrottle(std::make_unique<LinkPreviewThrottle>(registry));
}

}  // namespace browser_core
