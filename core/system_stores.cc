#include "browser_core/system_stores.h"

#include <optional>
#include <utility>

#include "base/files/file_enumerator.h"
#include "base/files/file_util.h"
#include "base/json/json_reader.h"
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

}  // namespace browser_core
