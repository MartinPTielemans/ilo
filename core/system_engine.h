#ifndef BROWSER_CORE_SYSTEM_ENGINE_H_
#define BROWSER_CORE_SYSTEM_ENGINE_H_

#include <string>

#include "base/files/file_path.h"
#include "base/functional/callback_forward.h"
#include "browser_core/platform/system_page.h"
#include "browser_core/public/browser_core.h"
#include "url/gurl.h"

class Profile;

namespace content {
class NavigationThrottleRegistry;
}

namespace browser_core::system_engine {

// The system's engine, WebKit on the Mac, shows a tab's page in place of Chromium's for a site
// whose protected video only it plays (decision 1). A short built-in list of streaming services
// opens there from the first visit. For any other site the user switches a tab to it once, from
// the protected-media note, and the profile keeps the site: its pages open in the system's engine
// from then on. Site Controls turns either off, per profile. A tab goes back to Chromium at another
// site. Agents never act in it, as the browser can't keep their rules in an engine whose pages it
// doesn't control, and an agent's Space never switches.

// Registers what profiles keep for it. Before the first profile is made.
void RegisterProfilePrefs();

// Whether the platform has one.
bool Available();

// Whether |tab| may show its page in the system's engine: one of the user's tabs, not an agent's.
bool Allowed(bc_id tab);

// The site of |url| as the profile keeps it: its registrable domain, else its host.
std::string SiteOf(const GURL& url);

// Whether |profile| opens |url|'s site in the system's engine: a listed service the user didn't
// turn off, or a site the user kept. Remember turns it on, taking back a listed service's off
// rather than keeping it, and Forget turns it off, which a listed service keeps as an override. A
// private window's profile has what its profile does, and what it changes goes with its session.
bool Remembered(Profile* profile, const GURL& url);
void Remember(Profile* profile, const GURL& url);
void Forget(Profile* profile, const GURL& url);

// Whether the user closed the protected-media note for |url|'s site in |profile|, or turned the
// site off in Site Controls, which then never shows it there again: Site Controls offer the
// system's engine for the site instead. Kept as the sites above are, a private window's for its
// session.
bool NoteClosed(Profile* profile, const GURL& url);
void CloseNote(Profile* profile, const GURL& url);

// Where |profile|'s system pages keep their cookies: a store of its own, apart from Chromium's and
// Safari's, or for a private window's profile one that keeps nothing. A profile's store has a
// random id the profile keeps from the first time it's asked for, just before the store is made.
platform::SystemStore StoreOf(Profile* profile);
// Whether |profile| has a store, from this run or an earlier one, so it has anything to clear.
// Asks without loading the system's engine, which only a store made needs.
bool StoreUsed(Profile* profile);

// The user's session with a site goes with the tab from one engine to the other, inside the
// profile: the cookies of the site and of its subdomains, and its origins' localStorage. Chromium's
// come into the system's store as a tab enters it, and the system's back to Chromium's as the site
// is turned off. A private window's go only between its own two stores, which keep nothing.

// Copies |profile|'s cookies of |url|'s site from Chromium's store into its system store, then runs
// |done| with the site's localStorage in Chromium, for a page's first load there to take.
void CopyToSystemEngine(Profile* profile,
                        const GURL& url,
                        base::OnceCallback<void(platform::StorageByOrigin)> done);
// Copies |profile|'s cookies of |url|'s site from its system store, and the localStorage of the
// site's pages its tabs show there, into Chromium's store, then has those tabs show their pages in
// Chromium. For a site the profile no longer keeps.
void ReturnToChromium(Profile* profile, const GURL& url);

// Deletes the store of the profile at |path|, if it made one, as the profile goes, then runs
// |then|. The profile's directory must still be there: an unloaded profile's preferences say.
void DeleteStore(const base::FilePath& path, base::OnceClosure then);
// After startup, deletes the stores given to profiles of the data folder that no profile names
// any more, as when a profile's folder was removed by hand. The system's engine is only loaded if
// there's one to delete.
void SweepStores();

// For ContentBrowserClient::CreateThrottlesForNavigation: a navigation of a tab's page in
// Chromium to a site its profile keeps for the system's engine loads there instead, and a tab
// showing a system page loads nothing in Chromium.
void CreateThrottlesForNavigation(content::NavigationThrottleRegistry& registry);

}  // namespace browser_core::system_engine

#endif  // BROWSER_CORE_SYSTEM_ENGINE_H_
