// Which of the Safari-engine stores a data folder's profiles were given still have a profile that
// names them (system_engine.h, SweepStores), apart so browser_core_unittests tests the rule alone.

#ifndef BROWSER_CORE_SYSTEM_STORES_H_
#define BROWSER_CORE_SYSTEM_STORES_H_

#include <set>
#include <string>
#include <vector>

#include "base/files/file_path.h"

class PrefRegistrySimple;
class PrefService;

namespace browser_core {

// The profile pref that names the profile's store.
inline constexpr char kSystemStorePref[] = "browser_core.system_engine.store";

// The local state pref listing the stores given to the data folder's profiles. In the folder, so
// it goes with it: a clean folder started at a moved-aside one's path lists none of the other's,
// and deletes none of them (#2664). A data folder removed whole leaves its stores, never deletes a
// live one.
inline constexpr char kGivenStoresPref[] = "browser_core.system_engine.given_stores";

void RegisterGivenStores(PrefRegistrySimple* registry);

// The ids listed in |local_state|, and lists |ids| there.
std::vector<std::string> GivenStores(const PrefService& local_state);
void SetGivenStores(PrefService& local_state, const std::vector<std::string>& ids);

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
