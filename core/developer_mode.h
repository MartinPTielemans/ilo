#ifndef BROWSER_CORE_DEVELOPER_MODE_H_
#define BROWSER_CORE_DEVELOPER_MODE_H_

#include <string>
#include <vector>

#include "url/gurl.h"

class Profile;

namespace browser_core::developer_mode {

// Developer Mode is a developer's view of a site's tabs: the full URL, and a look of its own. It's
// on by itself for localhost, as Chrome counts it, and the user turns it on or off for any other
// site, or off for localhost, which the profile keeps. It's the user's view, so agents get no call
// for it.

// Registers what profiles keep for it. Before the first profile is made.
void RegisterProfilePrefs();

// Whether |profile|'s tabs at |url| are in it. A private window's follow its profile's.
bool On(Profile* profile, const GURL& url);

// Whether the user can turn it on or off for |url| in |profile|: an http(s) page, outside a private
// window, which keeps no sites.
bool Switchable(Profile* profile, const GURL& url);

// Turns it on or off for |url|'s site in |profile|, if Switchable, and reports every tab of the
// profile at the site.
void Set(Profile* profile, const GURL& url, bool on);

// The sites |profile| has it on for, as system_engine::SiteOf gives them: localhost and 127.0.0.1
// first while they're on by themselves, then those the user turned on, in alphabetical order
// (#4377).
std::vector<std::string> Sites(Profile* profile);
// Turns it off for |site|, one of Sites, as Set does.
void TurnOff(Profile* profile, const std::string& site);

}  // namespace browser_core::developer_mode

#endif  // BROWSER_CORE_DEVELOPER_MODE_H_
