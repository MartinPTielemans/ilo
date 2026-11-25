// A tab of the user's that an agent has (bc_tab_give_to_agent), from the give until it leaves the
// agent's Space: what the core keeps of it, so that it's the user's still wherever the user's tabs
// are kept, and comes back where it left. Which tabs may be given is handed_give_rule's, and where
// one comes back handed_place's.

#ifndef BROWSER_CORE_HANDED_H_
#define BROWSER_CORE_HANDED_H_

#include <cstddef>
#include <map>
#include <vector>

#include "browser_core/public/browser_core.h"
#include "browser_core/space.h"

namespace browser_core::handed {

// What's kept of a handed tab. In memory only: the session keeps the tab itself, as the user's.
struct Mark {
  // The Space it left.
  bc_id home = 0;
  // Its node there, with its place among that Space's Today tabs.
  Space::Node node;
};

// |tab| was given to an agent, out of the Space |home|, where it had |node|.
void Given(bc_id tab, bc_id home, Space::Node node);

// The mark on |tab|, or null if no agent has it from the user.
const Mark* Of(bc_id tab);

// Each handed tab's mark.
const std::map<bc_id, Mark>& All();

// |tab| left the agent's Space, or closed: it's handed no longer.
void Left(bc_id tab);

// bc_tab_return_from_agent.
bool Return(bc_id tab);

// Where a tab that kept |node| goes among |rows|, the Today tabs of |home|, the Space it left,
// each of them |home|'s own or another handed tab that left it.
size_t PlaceAmong(const Space& home, const std::vector<bc_id>& rows, const Space::Node& node);

}  // namespace browser_core::handed

#endif  // BROWSER_CORE_HANDED_H_
