#ifndef BROWSER_CORE_NATIVE_AD_BLOCK_H_
#define BROWSER_CORE_NATIVE_AD_BLOCK_H_

#include <cstdint>
#include <string>
#include <vector>

#include "base/functional/callback_forward.h"

namespace base {
class FilePath;
}  // namespace base

namespace content {
class NavigationThrottleRegistry;
class RenderFrameHost;
class WebContents;
}  // namespace content

namespace network {
class URLLoaderFactoryBuilder;
}  // namespace network

class GURL;

namespace browser_core {

// The browser's own ad blocker (#2111), which blocks in place of uBlock Origin with
// --ad-block-engine=native until it passes the parity gate (app/check/adblock-parity.sh). The
// core finds the requests pages make and asks the app's engine, adblock-rust, about each through
// bc_host's ad_block_ calls. ad_block.cc decides where it blocks, with the user's settings and the
// sites they turned it off on, as it does for uBlock Origin.

// Whether it blocks in place of uBlock Origin.
bool NativeAdBlock();

// Gives the app's engine what to block with: |folder|, uBlock Origin's as the browser comes with
// it, for its stand-ins and scriptlets, the files of the |lists| turned on, and the user's
// |filters|, one a line. At launch, before the first page, and whenever they change.
void UseNativeAdBlock(const base::FilePath& folder,
                      const std::vector<base::FilePath>& lists,
                      const std::string& filters);

// The tab |popup|, which content made for |opener|'s page to show |url| in, |in_front| as a page's
// own tab or pop-up comes, from a page in an agent's Space if |agent|: as its tab strip gets it,
// it waits for the pop-up filters' answer about |url| if HoldsPopup says so (popup_hold.h, #2806).
void HoldPopup(content::WebContents* popup,
               content::WebContents* opener,
               const GURL& url,
               bool in_front,
               bool agent);

// Whether the tab |contents|'s report to the app, |report|, waits. A held tab's runs, posted, once
// the filters let its first address load or warn, or once no answer can come, and never if they
// close the tab: a pop-up they block is never listed or shown.
bool HoldPopupReport(content::WebContents* contents, base::OnceClosure report);

// Whether it blocks at all, as Settings' Ad blocking says. Off, the pages already open stop
// blocking at their next request, as uBlock Origin turned off does.
void SetNativeAdBlockOn(bool on);

// Puts it between the requests of the factory |factory_builder| builds and the network, from
// ContentBrowserClient::WillCreateURLLoaderFactory. The requests it blocks count for |frame|'s tab
// and |page|, the UKM source id of the page they're for, which is the next page's for the
// document a main frame's navigation commits.
void ProxyForNativeAdBlock(network::URLLoaderFactoryBuilder& factory_builder,
                           content::RenderFrameHost* frame,
                           int64_t page);

// Tells |blocked| how many requests it blocked or gave a stand-in on the page |contents| shows now,
// since that page's document committed: at once, then at most once a second while that changes,
// and never while it doesn't, until the next call or StopWatchingNativeAdBlocked. One page at a
// time: Site Controls'.
void WatchNativeAdBlocked(content::WebContents* contents,
                          base::RepeatingCallback<void(int)> blocked);
void StopWatchingNativeAdBlocked();

// Gives the page |registry|'s navigation loads what the app's engine hides on it and runs in it
// from its start, sent to its frame before it commits (ad_block_page.mojom), from
// ContentBrowserClient::CreateThrottlesForNavigation where ad_block.cc says it blocks. A tab's page
// that the user's lists block, as uBlock Origin's strict blocking does, never starts its request:
// a warning shows in its place, whose Open anyway lets its host through for a while, as uBlock
// Origin's does, and which an agent's tab has no way past.
void AddNativeAdBlockPage(content::NavigationThrottleRegistry& registry);

}  // namespace browser_core

#endif  // BROWSER_CORE_NATIVE_AD_BLOCK_H_
