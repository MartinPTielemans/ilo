#ifndef BROWSER_CORE_AD_BLOCK_H_
#define BROWSER_CORE_AD_BLOCK_H_

#include <optional>
#include <string>
#include <vector>

#include "content/public/browser/content_browser_client.h"

class GURL;
class Profile;

namespace content {
class BrowserContext;
class NavigationThrottleRegistry;
class RenderFrameHost;
class WebContents;
}  // namespace content

namespace net {
class IsolationInfo;
}  // namespace net

namespace network {
class URLLoaderFactoryBuilder;
}  // namespace network

namespace url {
class Origin;
}  // namespace url

namespace browser_core {

// Ad blocking (docs/ux-spec.md › Ad blocking): the uBlock Origin build.sh puts in the framework's
// resources, loaded into every profile at each launch, so it updates with the browser and never
// from a store, and on in private windows. The core gives it its settings as policy, under the
// platform's, and turns it off everywhere or for a site as the user says. None of it is the
// agents': they get no call that changes it.

// Sets the hooks that load it into each profile and give it its settings, before Chrome starts.
void SetUpAdBlock();

// Whether |id| is it, which the user turns off rather than removes.
bool IsAdBlocker(const std::string& id);

// Holds a page that starts before it's ready in the page's profile, as at launch, until it is, so
// it blocks from the first page. The browser's own (native_ad_block.h) holds none, gives the page
// what it hides and runs from its start, and shows a tab's page the lists block a warning instead.
void AddAdBlockHold(content::NavigationThrottleRegistry& registry);

// Turns it on or off in every profile loaded, as UserSettings().ad_blocking says, once the
// settings are read and whenever the user changes them.
void ApplyAdBlocking();

// Puts the browser's own ad blocker (native_ad_block.h) between the requests of the factory
// |factory_builder| builds and the network, from ContentBrowserClient::WillCreateURLLoaderFactory,
// where it blocks: with --ad-block-engine=native and ad blocking on, for a web page on a site the
// user hasn't turned it off on. A page open as it's turned on blocks from its next load.
void ProxyForAdBlock(content::BrowserContext* context,
                     content::RenderFrameHost* frame,
                     content::ContentBrowserClient::URLLoaderFactoryType type,
                     const url::Origin& request_initiator,
                     const net::IsolationInfo& isolation_info,
                     network::URLLoaderFactoryBuilder& factory_builder);

// Whether the user turned it off on the site of |url| in |context|'s profile, where it neither
// blocks nor hides anything, nor warns of a page.
bool AdBlockingOffAt(content::BrowserContext* context, const GURL& url);

// Whether it blocks on the site of |contents|'s page, or nullopt where that's not the user's to
// change there: a page that isn't http(s), a private window, or with it off in the profile.
std::optional<bool> AdBlockingOnSite(content::WebContents* contents);

// Lets it block on the site of |contents|'s page or not, if AdBlockingOnSite has a say, and
// reloads the page, so what it held back loads or what loaded goes. The profile keeps it.
void SetAdBlockingOnSite(content::WebContents* contents, bool on);

// The sites |profile| has it turned off on, in the order they were turned off: each a registrable
// domain, which covers its subdomains, or a host alone, as an address or localhost.
std::vector<std::string> AdBlockingOffSites(Profile* profile);

// Lets it block on |site|, one of AdBlockingOffSites, in |profile| again. A page open there keeps
// what it loaded until it reloads.
void TurnOnAdBlockingOnSite(Profile* profile, const std::string& site);

// Settings › Ad blocking's model, which uBlock Origin applies today and a native engine could take
// over (#2069) without the header or Settings changing. The filter lists and the user's own
// filters are the browser's, kept with the Spaces (UserSettings), for every profile; each change
// applies at once, as a change of the policy the core gives uBlock Origin, which its background
// page hears (app/fetch-ublock.sh).

// One of its filter lists, as Settings shows it.
struct AdBlockList {
  std::string key;
  // default, ads, privacy, malware, multipurpose, annoyances or regions.
  std::string group;
  std::string title;
  bool on = false;
};

// Its filter lists, by group and title, each on as the user or an administrator's policy says, or
// else as it comes. Empty until they've been read, a moment after the first profile loads.
std::vector<AdBlockList> AdBlockLists();

// Whether an administrator's policy for uBlock Origin chose its lists, which then are as it says
// and SetAdBlockList doesn't change.
bool AdBlockListsLocked();

// Turns the list of |key|, one of AdBlockLists, on or off.
void SetAdBlockList(const std::string& key, bool on);

// The user's own filters, one a line: as they set them, else check.sh's, else none; or an
// administrator's, where AdBlockFiltersLocked.
std::string AdBlockFilters();

// Whether an administrator's policy for uBlock Origin set its filters, which SetAdBlockFilters
// then doesn't change.
bool AdBlockFiltersLocked();

// Makes |filters|, one a line, the user's own.
void SetAdBlockFilters(const std::string& filters);

// How many requests it blocked on |contents|'s page, as its action's badge counts them, such as
// "12" or ">1k", or "" while it blocked none.
std::string AdBlockedOn(content::WebContents* contents);

}  // namespace browser_core

#endif  // BROWSER_CORE_AD_BLOCK_H_
