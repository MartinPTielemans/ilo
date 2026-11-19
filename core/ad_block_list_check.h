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

// When |list| says it was last updated, in its header, the comments before its first rule: its
// `! Updated:`, `! Last modified:` or, in a hosts file, `# last updated:`, as an ISO 8601 time in
// UTC or an HTTP date. Null if it says none that reads as a time.
std::optional<base::Time> Updated(std::string_view list);

// What an update makes of a list it downloaded from one of the list's addresses.
enum class Download {
  // It takes the place of the copy held, and the update goes on to the next list.
  kTaken,
  // It's broken or stale, a fault worth a line in the log, and the next address gets its turn.
  kRefused,
  // It says it was updated when the copy held says it was: nothing new. The next address gets its
  // turn, as the first may be a mirror that stopped there, and if none has a newer copy the one
  // held stays. Not a fault: most days a list due for an update hasn't changed since the last.
  kNothingNew,
};

// What |downloaded| is to |last_good|, the copy the ad blocker reads now if there is one.
// - Refused unless it reads as a filter list, text with rules in it and not a server's page, and
//   keeps at least half the rules |last_good| has, so a broken or cut-short download never
//   replaces a good copy. A list's day-to-day changes are a few rules in thousands.
// - Then refused if it says it was updated before |last_good| says it was, and nothing new if it
//   says the same: a mirror that stopped updating serves a copy older than or the same as the one
//   held, whichever of the list's addresses it is, and the next address gets its turn (#3505).
// - A copy that says no date, or held with none, is judged by its rules alone, as is a first
//   download, with no copy held to be older than.
Download Judge(std::string_view downloaded, std::optional<std::string_view> last_good);

}  // namespace browser_core::ad_block_list_check

#endif  // BROWSER_CORE_AD_BLOCK_LIST_CHECK_H_
