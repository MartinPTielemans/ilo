#ifndef BROWSER_CORE_HISTORY_LIST_H_
#define BROWSER_CORE_HISTORY_LIST_H_

#include <stddef.h>
#include <stdint.h>

#include "browser_core/public/browser_core.h"

class Profile;

namespace browser_core {

// Lists up to |max| of |profile|'s visited pages before |before|, microseconds since 1970 or now
// if 0, newest first and each once a day, whose title or URL has |text|'s words, then tells the
// app's history_listed for |id| and |request| (bc_profile_list_history). False for a private
// profile, one without history or whose history hasn't loaded, or no |max|.
bool ListHistory(Profile* profile,
                 bc_id id,
                 uint64_t request,
                 int64_t before,
                 const char* text,
                 size_t max);

}  // namespace browser_core

#endif  // BROWSER_CORE_HISTORY_LIST_H_
