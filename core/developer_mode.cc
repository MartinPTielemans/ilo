#include "browser_core/developer_mode.h"

#include <memory>
#include <optional>
#include <string>

#include "base/no_destructor.h"
#include "base/values.h"
#include "browser_core/page_log.h"
#include "browser_core/site_controls.h"
#include "browser_core/system_engine.h"
#include "browser_core/tab.h"
#include "chrome/browser/profiles/profile.h"
#include "chrome/browser/profiles/profile_keyed_service_factory.h"
#include "components/keyed_service/core/keyed_service.h"
#include "components/pref_registry/pref_registry_syncable.h"
#include "components/prefs/pref_service.h"
#include "components/prefs/scoped_user_pref_update.h"
#include "net/base/url_util.h"

namespace browser_core::developer_mode {

namespace {

// The sites the user turned it on or off for, each true or false, by system_engine::SiteOf. A site
// back at its default leaves, so localhost's entry is only ever false.
constexpr char kSites[] = "browser_core.developer_mode.sites";

// Builds no service: Chrome registers a profile's prefs through its factories.
class PrefsFactory : public ProfileKeyedServiceFactory {
 public:
  static PrefsFactory* GetInstance() {
    static base::NoDestructor<PrefsFactory> instance;
    return instance.get();
  }

  PrefsFactory() : ProfileKeyedServiceFactory("BrowserCoreDeveloperMode") {}

 private:
  std::unique_ptr<KeyedService> BuildServiceInstanceForBrowserContext(
      content::BrowserContext* context) const override {
    return nullptr;
  }

  void RegisterProfilePrefs(user_prefs::PrefRegistrySyncable* registry) override {
    registry->RegisterDictionaryPref(kSites);
  }
};

}  // namespace

void RegisterProfilePrefs() {
  PrefsFactory::GetInstance();
}

bool On(Profile* profile, const GURL& url) {
  if (!profile || !url.SchemeIsHTTPOrHTTPS()) {
    return false;
  }
  // A private window's profile reads its original's prefs through its own.
  const std::optional<bool> kept =
      profile->GetPrefs()->GetDict(kSites).FindBool(system_engine::SiteOf(url));
  return kept.value_or(net::IsLocalhost(url));
}

bool Switchable(Profile* profile, const GURL& url) {
  return profile && !profile->IsOffTheRecord() && url.SchemeIsHTTPOrHTTPS();
}

void Set(Profile* profile, const GURL& url, bool on) {
  if (!Switchable(profile, url) || On(profile, url) == on) {
    return;
  }
  const std::string site = system_engine::SiteOf(url);
  {
    ScopedDictPrefUpdate sites(profile->GetPrefs(), kSites);
    if (on == net::IsLocalhost(url)) {
      sites->Remove(site);
    } else {
      sites->Set(site, on);
    }
  }
  for (Tab* tab : Tab::All()) {
    Profile* its = tab->profile();
    if (its && its->GetOriginalProfile() == profile && tab->Url().SchemeIsHTTPOrHTTPS() &&
        system_engine::SiteOf(tab->Url()) == site) {
      tab->StateChanged();
      SiteControls::PageChanged(tab->id());
      PageLog::DeveloperModeChanged(tab->web_contents());
    }
  }
}

std::vector<std::string> Sites(Profile* profile) {
  std::vector<std::string> sites;
  if (!profile) {
    return sites;
  }
  const base::DictValue& kept = profile->GetPrefs()->GetDict(kSites);
  for (const char* local : {"localhost", "127.0.0.1"}) {
    if (kept.FindBool(local).value_or(true)) {
      sites.push_back(local);
    }
  }
  for (const auto [site, on] : kept) {
    if (on.GetIfBool().value_or(false)) {
      sites.push_back(site);
    }
  }
  return sites;
}

void TurnOff(Profile* profile, const std::string& site) {
  Set(profile, GURL("http://" + site + "/"), false);
}

}  // namespace browser_core::developer_mode
