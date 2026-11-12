#ifndef BROWSER_CORE_CLEAR_SITE_H_
#define BROWSER_CORE_CLEAR_SITE_H_

#include "browser_core/platform/system_page.h"
#include "browser_core/public/browser_core.h"

namespace browser_core {

// Deletes |what| |tab|'s profile keeps for its page's site, then loads the page again. See
// bc_tab_clear_site and bc_tab_clear_site_data.
void ClearSiteAndReload(bc_id tab, platform::SiteData what);

}  // namespace browser_core

#endif  // BROWSER_CORE_CLEAR_SITE_H_
