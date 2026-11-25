// A tab of the user's that an agent has (bc_tab_give_to_agent), from the give until it leaves the
// agent's Space: what the core keeps of it, so that it's the user's still wherever the user's tabs
// are kept, and comes back where it left. Which tabs may be given is handed_give_rule's, where one
// comes back handed_place's, its way back into its Space handed_back's, and how far through its
// history one may be taken handed_history's.

#ifndef BROWSER_CORE_HANDED_H_
#define BROWSER_CORE_HANDED_H_

#include <cstddef>
#include <map>
#include <optional>
#include <vector>

#include "browser_core/handed_back.h"
#include "browser_core/handed_history.h"
#include "browser_core/public/browser_core.h"
#include "browser_core/space.h"

namespace browser_core::handed {

// What's kept of a handed tab. In memory only: the session keeps the tab itself, as the user's, or
// for a favorite its tile.
struct Mark {
  // The Space it left, or for a favorite, which is in none, the one the user gave it from.
  bc_id home = 0;
  // Its node there, with its place among that Space's Today tabs, or for a pinned tab among what
  // its folder held.
  Space::Node node;
  // The tabs of the split view it left, left to right with itself among them, or none: it goes
  // back into that split while one of them is still in its list there.
  std::vector<bc_id> split;
  // Its pin, for a tab that was pinned: an agent has it as a tab alone, and it comes back pinned
  // as it was.
  std::optional<handed_back::Pin> pin;
  // For a favorite, the tab that keeps its tile while it's away, else 0: one without a page, at
  // the URL it's a favorite at, which is the favorite to the favorites, the session and each
  // window, so the tile stays where it is, as it is. It gets no page while this mark names it
  // (KeepsATile). The favorite's own tab is a tab alone in the agent's Space, and takes the tile
  // back when it returns, on the page it's at, and the keeper closes. If the user removed the
  // favorite meanwhile, or moved it into a Space, it comes back a Today tab at the top of |home|.
  // After a quit the favorite is the keeper, at that URL.
  bc_id tile = 0;
  // The history it had then, which stays the user's.
  handed_history::Bound history;
};

// What's kept of |tab| as it leaves |from|, a Space of the user's, for an agent's: all but its
// history, which Given takes once it's the agent's.
Mark Leaving(const Space& from, bc_id tab);

// |tab| was given to an agent, and |left| is what Leaving made of it before it went.
void Given(bc_id tab, Mark left);

// Whether |tab| keeps the tile of a favorite an agent has (Mark::tile), so has no page and gets
// none: what Tab::Wake asks, the one place a tab without a page gets it, so whatever shows or loads
// one finds none to act on.
bool KeepsATile(bc_id tab);

// The mark on |tab|, or null if no agent has it from the user.
const Mark* Of(bc_id tab);

// Each handed tab's mark.
const std::map<bc_id, Mark>& All();

// |tab| left the agent's Space, or closed: it's handed no longer.
void Left(bc_id tab);

// bc_tab_return_from_agent.
bool Return(bc_id tab);

// The history |tab|'s page has now is the user's from here, but for the entry it's on: at the
// give, when the user hands the tab back after holding it, and when its page is made again, as an
// unloaded tab's is. Nothing for a tab that isn't handed or has no page.
void BoundHistory(bc_id tab);

// Whether |tab| may be taken to the entry of its history with the id |entry|
// (NavigationEntry::GetUniqueID), by a page's own traversal (|from_page|) or the browser's: any,
// unless it's handed. What Chromium asks before each traversal of a handed tab.
bool MayGoTo(bc_id tab, int entry, bool from_page);

// The same for the agent's own back and forward, which ask first, to say why not: never the
// user's step, whoever holds the tab.
bool AgentMayGoTo(bc_id tab, int entry);

// Whether |tab|'s own step to the entry |offset| from the one it's on, -1 for back and 1 for
// forward, is one it may take: what its back and forward are lit by, and its history's list made
// of. Any for a tab that isn't handed, and none for a handed one without its page, unless the user
// holds it: its history is bound again as its page is made.
bool MayStep(bc_id tab, int offset);

// Where a tab that kept |node| goes among |rows|, the Today tabs of |home|, the Space it left, or
// what one of its folders holds, each of them |home|'s own or another handed tab that left it.
size_t PlaceAmong(const Space& home, const std::vector<bc_id>& rows, const Space::Node& node);

// |home|'s pinned rows with the pinned tabs handed out of it among them, each where it would come
// back now.
handed_back::Pinned PinnedWith(const Space& home);

}  // namespace browser_core::handed

#endif  // BROWSER_CORE_HANDED_H_
