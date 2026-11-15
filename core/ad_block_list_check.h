// When the browser's own ad blocker looks at its filter lists, and whether a list it downloaded
// takes the place of the copy it has (native_ad_block_lists.h), apart so browser_core_unittests
// tests the rules alone.

#ifndef BROWSER_CORE_AD_BLOCK_LIST_CHECK_H_
#define BROWSER_CORE_AD_BLOCK_LIST_CHECK_H_

#include <cstddef>
#include <optional>
#include <string_view>

#include "base/time/time.h"

namespace browser_core::ad_block_list_check {

// How long after a look at the lists a page starting looks again. A look only reads the copies'
// ages, and downloads those past their expiry, days long, so an hour is uBlock Origin's own
// autoUpdatePeriod.
inline constexpr base::TimeDelta kLookEvery = base::Hours(1);

// Whether a page starting at |now| looks at the lists, |last_look| being the last look: only once
// launch's has been, an hour or more ago. The lists have no timer of their own, so nothing wakes
// an idle browser for them.
bool PageLooks(base::TimeTicks last_look, base::TimeTicks now);

// How many rules |list| has: its lines neither blank nor a comment, as uBlock Origin reads them.
size_t Rules(std::string_view list);

// Whether |downloaded| takes the place of |last_good|, the copy the ad blocker reads now if there
// is one. It must read as a filter list, text with rules in it and not a server's page, and keep
// at least half the rules |last_good| has, so a broken or cut-short download never replaces a
// good copy. A list's day-to-day changes are a few rules in thousands.
bool Takes(std::string_view downloaded, std::optional<std::string_view> last_good);

}  // namespace browser_core::ad_block_list_check

#endif  // BROWSER_CORE_AD_BLOCK_LIST_CHECK_H_
