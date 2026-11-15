#ifndef BROWSER_CORE_AD_BLOCK_OFF_SITES_H_
#define BROWSER_CORE_AD_BLOCK_OFF_SITES_H_

#include "base/values.h"

class PrefRegistrySimple;
class PrefService;

namespace browser_core {

// The sites the user turned ad blocking off on (ad_block.h), as URL patterns in the order they
// did, in the profile's own prefs, so they outlast the uBlock Origin that kept them in its
// extension prefs. Apart so browser_core_unittests tests the carry-over alone.

// The profile's list, a list of URL pattern strings.
inline constexpr char kAdBlockOffSitesPref[] = "browser_core.ad_block.off_sites";

// Whether the profile took up uBlock Origin's list, once, so a list the user empties stays empty.
inline constexpr char kAdBlockOffSitesCarriedPref[] = "browser_core.ad_block.off_sites_carried";

void RegisterAdBlockOffSitesPrefs(PrefRegistrySimple* registry);

// The first time it's called for a profile's |prefs|, copies |old_sites|, uBlock Origin's list or
// null, into the profile's list, in its order, if that's empty. |old_sites| stays as it is for a
// release, as the way back to a build that reads it.
void CarryOverAdBlockOffSites(PrefService* prefs, const base::ListValue* old_sites);

}  // namespace browser_core

#endif  // BROWSER_CORE_AD_BLOCK_OFF_SITES_H_
