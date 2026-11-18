#ifndef BROWSER_CORE_SEARCH_DEFAULT_H_
#define BROWSER_CORE_SEARCH_DEFAULT_H_

class PrefService;
class TemplateURL;
class TemplateURLService;
struct TemplateURLData;

namespace user_prefs {
class PrefRegistrySyncable;
}

namespace browser_core {

// The profile prefs ForgetPickedDefault and AddGoogleChoice keep.
void RegisterSearchDefaultPrefs(user_prefs::PrefRegistrySyncable* registry);

// DuckDuckGo is the default engine (engine/patches/ours/chromium-search-default-duckduckgo.patch).
// Before, the core picked it for a profile as though the user had, and a recommended
// DefaultSearchProvider policy never outranks the user. This forgets that pick, once a profile, so
// a pick the user makes after it stays theirs.
void ForgetPickedDefault(PrefService* prefs);

// ungoogled-chromium takes Google out of the engines a user picks from, and the owner wants it
// back as a choice only (decision 2): a plain search URL, with no suggest URL, tracking parameters
// or favicon, never the default and never preselected. Its pick is an engine like any other, which
// ForgetPickedDefault keeps.
TemplateURLData GoogleChoice();

// Whether |engine| is GoogleChoice's, which the default list doesn't show by itself, since only
// engines from Chromium's prepopulated data or a policy are in it.
bool IsGoogleChoice(const TemplateURL& engine);

// Adds GoogleChoice to |engines|, once a profile, so a user who removes it doesn't get it back. An
// engine the user made for google.com stays theirs; one Chrome made from the site's search form
// gives way to it.
void AddGoogleChoice(PrefService* prefs, TemplateURLService* engines);

}  // namespace browser_core

#endif  // BROWSER_CORE_SEARCH_DEFAULT_H_
