// Which of the Safari-engine stores a data folder's profiles were given still have a profile that
// names them (system_engine.h, SweepStores), apart so browser_core_unittests tests the rule alone.

#ifndef BROWSER_CORE_SYSTEM_STORES_H_
#define BROWSER_CORE_SYSTEM_STORES_H_

#include <set>
#include <string>
#include <vector>

#include "base/files/file_path.h"

namespace browser_core {

// The profile pref that names the profile's store.
inline constexpr char kSystemStorePref[] = "browser_core.system_engine.store";

// The id of the store the profile at |path| names in its Preferences on disk, for a profile that
// isn't loaded. Empty for none.
std::string StoreNamedOnDisk(const base::FilePath& path);

// The ids of the stores the profiles on disk in the data folder |folder| name.
std::set<std::string> StoresNamedOnDisk(const base::FilePath& folder);

// Those of the stores |given| to a data folder's profiles that no profile names: their folders
// went, by hand or with the whole data folder, and took their stores' names with them.
std::vector<std::string> StoresUnnamed(const std::vector<std::string>& given,
                                       const std::set<std::string>& named);

}  // namespace browser_core

#endif  // BROWSER_CORE_SYSTEM_STORES_H_
