#include "browser_core/system_stores.h"

#include <optional>
#include <utility>

#include "base/files/file_enumerator.h"
#include "base/files/file_util.h"
#include "base/json/json_reader.h"
#include "base/values.h"
#include "chrome/common/chrome_constants.h"

namespace browser_core {

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
