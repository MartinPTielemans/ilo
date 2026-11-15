// What the browser's own ad blocker carries over from uBlock Origin's storage (ad_block.cc), apart
// so browser_core_unittests tests the read alone.

#ifndef BROWSER_CORE_AD_BLOCK_CARRY_H_
#define BROWSER_CORE_AD_BLOCK_CARRY_H_

#include <optional>
#include <string>

#include "base/files/file_path.h"

namespace browser_core::ad_block_carry {

// My filters as uBlock Origin keeps them in its chrome.storage.local at |folder|, a profile's
// "Local Extension Settings/<its id>": "" where it keeps none, as with no folder, or nullopt where
// the storage can't be read. Its values are read, never written, repaired or deleted, so the way
// back finds them as they were. Blocks.
std::optional<std::string> OldFilters(const base::FilePath& folder);

}  // namespace browser_core::ad_block_carry

#endif  // BROWSER_CORE_AD_BLOCK_CARRY_H_
