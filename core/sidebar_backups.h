// Which sidebar backups Arc's schedule keeps (session.h, RestoreSidebar), apart so
// browser_core_unittests tests the rule alone.

#ifndef BROWSER_CORE_SIDEBAR_BACKUPS_H_
#define BROWSER_CORE_SIDEBAR_BACKUPS_H_

#include <vector>

#include "base/time/time.h"

namespace browser_core {

// Ours, as Arc's spacing isn't known: today's backups are an hour apart at least, so the ten kept
// span a day's use rather than its last few minutes, which a save every ten seconds would fill.
inline constexpr base::TimeDelta kBackupGap = base::Hours(1);

// Those of |times|, newest first, that Arc's schedule keeps at |now|, by the user's calendar: ten
// from today, then the newest of each day for the ten days before, of each week to a month back
// and of each month to a year back.
std::vector<base::Time> BackupsKept(const std::vector<base::Time>& times, base::Time now);

}  // namespace browser_core

#endif  // BROWSER_CORE_SIDEBAR_BACKUPS_H_
