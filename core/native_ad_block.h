#ifndef BROWSER_CORE_NATIVE_AD_BLOCK_H_
#define BROWSER_CORE_NATIVE_AD_BLOCK_H_

#include <string>
#include <vector>

namespace base {
class FilePath;
}  // namespace base

namespace content {
class NavigationThrottleRegistry;
}  // namespace content

namespace network {
class URLLoaderFactoryBuilder;
}  // namespace network

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

// Whether it blocks at all, as Settings' Ad blocking says. Off, the pages already open stop
// blocking at their next request, as uBlock Origin turned off does.
void SetNativeAdBlockOn(bool on);

// Puts it between the requests of the factory |factory_builder| builds and the network, from
// ContentBrowserClient::WillCreateURLLoaderFactory.
void ProxyForNativeAdBlock(network::URLLoaderFactoryBuilder& factory_builder);

// Gives the page |registry|'s navigation loads what the app's engine hides on it and runs in it
// from its start, sent to its frame before it commits (ad_block_page.mojom), from
// ContentBrowserClient::CreateThrottlesForNavigation where ad_block.cc says it blocks. A tab's page
// that the user's lists block, as uBlock Origin's strict blocking does, never starts its request:
// a warning shows in its place, whose Open anyway lets its host through for a while, as uBlock
// Origin's does, and which an agent's tab has no way past.
void AddNativeAdBlockPage(content::NavigationThrottleRegistry& registry);

}  // namespace browser_core

#endif  // BROWSER_CORE_NATIVE_AD_BLOCK_H_
