// When auto-archive takes a Today tab no window shows (auto_archive.h), apart so
// browser_core_unittests tests the rule alone.

#ifndef BROWSER_CORE_ARCHIVE_RULE_H_
#define BROWSER_CORE_ARCHIVE_RULE_H_

#include <optional>

#include "base/time/time.h"

namespace browser_core::archive_rule {

// A profile's archive pref meaning Never (bc_profile_set_archive_after's BC_ARCHIVE_NEVER).
inline constexpr double kNever = -1;

// How long a profile's Today tab waits unseen, from its pref |seconds|: its own time, |fallback|
// while it has none, or none at all for Never.
std::optional<base::TimeDelta> FromPref(double seconds, base::TimeDelta fallback);

// When a Today tab last seen at |last_seen| goes in the archive, if it ever does: never with no
// time, so Never takes no tab however long it waits. Clear and ⌘W archive by other ways.
std::optional<base::Time> DueAt(base::Time last_seen, std::optional<base::TimeDelta> after);

}  // namespace browser_core::archive_rule

#endif  // BROWSER_CORE_ARCHIVE_RULE_H_
