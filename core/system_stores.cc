#include "browser_core/system_stores.h"

#include <optional>
#include <utility>

#include "base/files/file_enumerator.h"
#include "base/files/file_util.h"
#include "base/hash/sha1.h"
#include "base/json/json_reader.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/string_util.h"
#include "base/values.h"
#include "chrome/common/chrome_constants.h"
#include "components/prefs/pref_registry_simple.h"
#include "components/prefs/pref_service.h"

namespace browser_core {

void RegisterGivenStores(PrefRegistrySimple* registry) {
  registry->RegisterListPref(kGivenStoresPref);
}

std::vector<std::string> GivenStores(const PrefService& local_state) {
  std::vector<std::string> given;
  for (const base::Value& each : local_state.GetList(kGivenStoresPref)) {
    if (each.is_string()) {
      given.push_back(each.GetString());
    }
  }
  return given;
}

void SetGivenStores(PrefService& local_state, const std::vector<std::string>& ids) {
  base::ListValue list;
  for (const std::string& each : ids) {
    list.Append(each);
  }
  local_state.SetList(kGivenStoresPref, std::move(list));
}

std::string StoreNamedOnDisk(const base::FilePath& path) {
  std::string json;
  if (!base::ReadFileToString(path.Append(chrome::kPreferencesFilename), &json)) {
    return std::string();
  }
  std::optional<base::DictValue> prefs = base::JSONReader::ReadDict(json, base::JSON_PARSE_RFC);
  const std::string* id = prefs ? prefs->FindStringByDottedPath(kSystemStorePref) : nullptr;
  return id ? *id : std::string();
}

std::set<std::string> StoresNamedOnDisk(const base::FilePath& folder) {
  std::set<std::string> named;
  base::FileEnumerator profiles(folder, false, base::FileEnumerator::DIRECTORIES);
  for (base::FilePath each = profiles.Next(); !each.empty(); each = profiles.Next()) {
    if (std::string id = StoreNamedOnDisk(each); !id.empty()) {
      named.insert(std::move(id));
    }
  }
  return named;
}

std::vector<std::string> StoresUnnamed(const std::vector<std::string>& given,
                                       const std::set<std::string>& named) {
  std::vector<std::string> unnamed;
  for (const std::string& id : given) {
    if (!named.contains(id)) {
      unnamed.push_back(id);
    }
  }
  return unnamed;
}

std::string OldStoreIdFor(const base::FilePath& path) {
  // As StoreIdFor was before #2664, byte for byte, or the sweep finds none of its stores.
  std::string digest = base::SHA1HashString("ilo system store " + path.AsUTF8Unsafe());
  digest[6] = static_cast<char>((digest[6] & 0x0f) | 0x50);
  digest[8] = static_cast<char>((digest[8] & 0x3f) | 0x80);
  const std::string hex = base::ToLowerASCII(base::HexEncode(digest.substr(0, 16)));
  return hex.substr(0, 8) + "-" + hex.substr(8, 4) + "-" + hex.substr(12, 4) + "-" +
         hex.substr(16, 4) + "-" + hex.substr(20, 12);
}

std::vector<base::FilePath> OldProfilePaths(const base::FilePath& folder, int profiles_created) {
  // chrome::kInitialProfile, and ProfileManager::GenerateNextProfileDirectoryPath's names.
  std::vector<base::FilePath> paths = {folder.AppendASCII(chrome::kInitialProfile)};
  for (int n = 1; n < profiles_created; ++n) {
    paths.push_back(folder.AppendASCII(chrome::kMultiProfileDirPrefix + base::NumberToString(n)));
  }
  return paths;
}

std::vector<std::string> OldStoresUnnamed(const std::vector<std::string>& listed,
                                          const base::FilePath& folder,
                                          int profiles_created,
                                          const std::set<std::string>& named,
                                          const std::set<base::FilePath>& present) {
  const std::set<std::string> on_disk(listed.begin(), listed.end());
  std::vector<std::string> gone;
  for (const base::FilePath& path : OldProfilePaths(folder, profiles_created)) {
    std::string id = OldStoreIdFor(path);
    if (on_disk.contains(id) && !named.contains(id) && !present.contains(path)) {
      gone.push_back(std::move(id));
    }
  }
  return gone;
}

OldStores OldStoresFor(bool new_local_state, bool swept) {
  if (new_local_state) {
    return OldStores::kMark;
  }
  return swept ? OldStores::kNothing : OldStores::kSweep;
}

}  // namespace browser_core
