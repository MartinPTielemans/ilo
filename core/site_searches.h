#ifndef BROWSER_CORE_SITE_SEARCHES_H_
#define BROWSER_CORE_SITE_SEARCHES_H_

#include <string>

class PrefService;
class TemplateURL;
class TemplateURLService;

namespace user_prefs {
class PrefRegistrySyncable;
}

namespace browser_core {

// The profile pref SeedSiteSearches keeps.
void RegisterSiteSearchesPrefs(user_prefs::PrefRegistrySyncable* registry);

// Gives the profile of |prefs| Arc's 19 site searches (#1397) in |engines|, which have loaded, once,
// so one the user deletes stays deleted. One whose keyword the profile has already keeps its own
// page, and one Chrome made from the site's own search form, which it keeps inactive until the user
// turns it on, is turned on, as Arc's are; one the user turned off stays off.
void SeedSiteSearches(PrefService* prefs, TemplateURLService* engines);

// The site search whose keyword |text| starts, case aside, as Arc offers one by its host rather
// than by what the field completes: "twit" offers X's, twitter.com, while it completes twitch.tv,
// and "gm" none, as Gmail's is mail.google.com. The shortest keyword, as Chrome ranks them, and
// none for one letter, which would offer one for nearly anything typed. Null if none.
const TemplateURL* SiteStarted(TemplateURLService* engines, const std::u16string& text);

}  // namespace browser_core

#endif  // BROWSER_CORE_SITE_SEARCHES_H_
