// bc_profile_read (#3820): a saved read on a profile's own session, with each profile's reads
// ending with it. saved_read.h does the reading.

#ifndef BROWSER_CORE_PROFILE_READS_H_
#define BROWSER_CORE_PROFILE_READS_H_

#include <cstddef>
#include <cstdint>

#include "browser_core/public/browser_core.h"

class GURL;

namespace browser_core {

// Reads |url| on |profile|'s session, loading the profile first if it isn't, then
// profile_read_done with |read|, from a task of its own. A read under way when its profile goes
// ends with an answer that says so; once shutdown has started, nothing more is told.
void ReadOnProfile(bc_id profile, uint64_t read, const GURL& url, size_t max_bytes,
                   int64_t timeout_ms);

}  // namespace browser_core

#endif  // BROWSER_CORE_PROFILE_READS_H_
