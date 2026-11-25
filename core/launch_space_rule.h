// Which of the Spaces put back at launch a window shows, apart so browser_core_unittests tests the
// rule alone (#4230).

#ifndef BROWSER_CORE_LAUNCH_SPACE_RULE_H_
#define BROWSER_CORE_LAUNCH_SPACE_RULE_H_

#include <cstddef>
#include <optional>
#include <string>
#include <vector>

namespace browser_core {

// A Space put back at launch, as the rule sees it.
struct LaunchSpace {
  // An outside agent's or a task's, whose work ended with the last run.
  bool agents = false;
  std::string profile;
};

// Which of |spaces| a window shows at launch, given the one it showed when it was saved, if any.
// An agent's Space is never shown, as a page shown would load again: the user's Space of its
// profile is, else the user's first. Only with no Space of the user's is an agent's the answer,
// and then the window shows it without a page. None with no Spaces.
std::optional<size_t> SpaceShownAtLaunch(const std::vector<LaunchSpace>& spaces,
                                         std::optional<size_t> saved);

}  // namespace browser_core

#endif  // BROWSER_CORE_LAUNCH_SPACE_RULE_H_
