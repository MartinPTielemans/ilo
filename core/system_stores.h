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

// Stores from before #2664, whose ids were worked out from their profiles' paths (#2701).

// The Local State pref that marks a data folder whose old stores were swept, or that never had
// any: one made by a version that gives random ids.
inline constexpr char kOldStoresSweptPref[] = "browser_core.system_engine.old_stores_swept";

// The id a version before #2664 gave the store of the profile at |path|: a version 5 UUID of the
// path, so the same path always gave the same id.
std::string OldStoreIdFor(const base::FilePath& path);

// The paths a profile in the data folder |folder| can have had: Default, and Profile 1 up to the
// last Chrome numbered, |profiles_created| being Local State's count, one past it.
std::vector<base::FilePath> OldProfilePaths(const base::FilePath& folder, int profiles_created);

// Of the stores |listed| on disk, those an old version gave a profile of |folder| that's gone:
// the id is one of OldProfilePaths', no profile |named| it, and its path has no profile folder,
// none of those |present|. Any other id stays, so a mistake can only leave a store behind.
std::vector<std::string> OldStoresUnnamed(const std::vector<std::string>& listed,
                                          const base::FilePath& folder,
                                          int profiles_created,
                                          const std::set<std::string>& named,
                                          const std::set<base::FilePath>& present);

// What a launch does about a data folder's old stores.
enum class OldStores {
  // The folder is new: it never had an old store, so it's marked without a sweep, and a folder
  // set aside and put back later keeps its stores.
  kMark,
  // Swept before, or new before.
  kNothing,
  kSweep,
};
OldStores OldStoresFor(bool new_local_state, bool swept);

}  // namespace browser_core

#endif  // BROWSER_CORE_SYSTEM_STORES_H_
