#include "browser_core/closed_rule.h"

#include <algorithm>
#include <iterator>

namespace browser_core::closed_rule {

std::vector<Closed> Merged(const std::vector<Closed>& tabs, const std::vector<Closed>& windows) {
  std::vector<Closed> merged;
  merged.reserve(tabs.size() + windows.size());
  std::ranges::merge(tabs, windows, std::back_inserter(merged),
                     [](const Closed& a, const Closed& b) { return a.at > b.at; });
  return merged;
}

std::optional<size_t> Next(const std::vector<Closed>& newest_first, const std::string& profile) {
  auto next = std::ranges::find_if(newest_first, [&profile](const Closed& closed) {
    return closed.by_user && closed.profile == profile;
  });
  if (next == newest_first.end()) {
    return std::nullopt;
  }
  return static_cast<size_t>(next - newest_first.begin());
}

void Save(const Closed& closed, base::DictValue& dict) {
  if (closed.by_user) {
    dict.Set("reopens", true);
  }
  if (!closed.profile.empty()) {
    dict.Set("profile", closed.profile);
  }
}

void Load(const base::DictValue& dict, Closed& closed) {
  closed.by_user = dict.FindBool("reopens").value_or(false);
  const std::string* profile = dict.FindString("profile");
  closed.profile = profile ? *profile : std::string();
}

}  // namespace browser_core::closed_rule
