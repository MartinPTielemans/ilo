#include "browser_core/launch_space_rule.h"

namespace browser_core {

std::optional<size_t> SpaceShownAtLaunch(const std::vector<LaunchSpace>& spaces,
                                         std::optional<size_t> saved) {
  if (saved && *saved >= spaces.size()) {
    saved.reset();
  }
  if (saved && !spaces[*saved].agents) {
    return saved;
  }
  std::optional<size_t> users_first;
  for (size_t i = 0; i < spaces.size(); ++i) {
    if (spaces[i].agents) {
      continue;
    }
    if (saved && spaces[i].profile == spaces[*saved].profile) {
      return i;
    }
    if (!users_first) {
      users_first = i;
    }
  }
  if (users_first) {
    return users_first;
  }
  if (saved) {
    return saved;
  }
  return spaces.empty() ? std::nullopt : std::optional<size_t>(0);
}

}  // namespace browser_core
