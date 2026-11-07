#ifndef BROWSER_CORE_AD_BLOCK_H_
#define BROWSER_CORE_AD_BLOCK_H_

#include <optional>
#include <string>
#include <vector>

class Profile;

namespace content {
class NavigationThrottleRegistry;
class WebContents;
}  // namespace content

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
// it blocks from the first page.
void AddAdBlockHold(content::NavigationThrottleRegistry& registry);

// Turns it on or off in every profile loaded, as UserSettings().ad_blocking says, once the
// settings are read and whenever the user changes them.
void ApplyAdBlocking();

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

}  // namespace browser_core

#endif  // BROWSER_CORE_AD_BLOCK_H_
