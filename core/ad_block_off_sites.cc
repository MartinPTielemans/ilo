#include "browser_core/ad_block_off_sites.h"

#include "components/prefs/pref_registry_simple.h"
#include "components/prefs/pref_service.h"

namespace browser_core {

void RegisterAdBlockOffSitesPrefs(PrefRegistrySimple* registry) {
  registry->RegisterListPref(kAdBlockOffSitesPref);
  registry->RegisterBooleanPref(kAdBlockOffSitesCarriedPref, false);
}

void CarryOverAdBlockOffSites(PrefService* prefs, const base::ListValue* old_sites) {
  if (prefs->GetBoolean(kAdBlockOffSitesCarriedPref)) {
    return;
  }
  if (old_sites && !old_sites->empty() && prefs->GetList(kAdBlockOffSitesPref).empty()) {
    prefs->SetList(kAdBlockOffSitesPref, old_sites->Clone());
  }
  prefs->SetBoolean(kAdBlockOffSitesCarriedPref, true);
}

}  // namespace browser_core
