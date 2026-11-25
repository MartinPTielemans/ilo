// The way back of a tab the user handed an agent, into the Space it left: what's kept of a pinned
// tab's pin and of a Today tab's folder meanwhile, and the putting back once the tab is one of that
// Space's Today tabs again.
// Only the Space is asked, apart from handed.cc so browser_core_unittests tests it with Space
// alone. Where it goes is handed_place's.

#ifndef BROWSER_CORE_HANDED_BACK_H_
#define BROWSER_CORE_HANDED_BACK_H_

#include <cstddef>
#include <map>
#include <optional>
#include <vector>

#include "base/functional/function_ref.h"
#include "browser_core/public/browser_core.h"
#include "browser_core/space.h"
#include "components/sync/base/unique_position.h"
#include "url/gurl.h"

namespace browser_core::handed_back {

// What's kept of a pinned tab's pin for as long as an agent has the tab, which goes as a tab alone.
struct Pin {
  // A folder the tab was in, with the node it had.
  struct Folder {
    bc_id id = 0;
    Space::Node node;
  };
  // The address it's pinned at, which Back to Pinned URL goes to. The agent's pages change none
  // of it.
  GURL url;
  // The folders it was in, from the one that held it outward. None for one outside every folder.
  std::vector<Folder> folders;
};

// What's kept of |tab|'s pin in |space|, or none if it isn't pinned there.
std::optional<Pin> PinOf(const Space& space, bc_id tab);

// Where a row that kept |node| goes among |rows|, |home|'s Today tabs or what one of its folders
// holds, each at the place |place_of| gives it, or none for null (handed_place::Among).
size_t Among(const Space& home,
             const std::vector<bc_id>& rows,
             const Space::Node& node,
             base::FunctionRef<const syncer::UniquePosition*(bc_id)> place_of);

// A pinned tab handed out of a Space, with what's kept of it.
struct Held {
  bc_id tab = 0;
  Space::Node node;
  Pin pin;
};

// A Space's pinned rows with the pinned tabs handed out of it among them.
struct Pinned {
  // The pinned tabs and folders, top to bottom, each folder followed by what's in it.
  std::vector<bc_id> rows;
  // Each handed one, to the folder it's in there, or 0 for none.
  std::map<bc_id, bc_id> folders;
};

// |home|'s pinned rows with each of |held| where Put would pin it now, which is where the session
// saves it.
Pinned PinnedWith(const Space& home, const std::vector<Held>& held);

// The today's folder of |space| holding the Today tab |tab|, which is kept for as long as an agent
// has the tab, or 0 for a tab in none.
bc_id TodayFolderOf(const Space& space, bc_id tab);

// A Today tab handed out of a Space, with what's kept of it.
struct HeldToday {
  bc_id tab = 0;
  Space::Node node;
  // The today's folder it was in, or 0.
  bc_id folder = 0;
};

// A Space's Today tabs with the Today tabs handed out of it among them.
struct Today {
  std::vector<bc_id> rows;
  // Each handed one that is in a today's folder there, to that folder.
  std::map<bc_id, bc_id> folders;
};

// |home|'s Today tabs with each of |held| where Put would put it now, which is where the session
// saves it: a today's folder's tabs are one run with those handed out of it, so a launch brings
// the folder back whole.
Today TodayWith(const Space& home, const std::vector<HeldToday>& held);

// Where a tab handed out of a Space is among that Space's rows.
struct Place {
  // Its index among the rows, top to bottom, with every tab handed out of the Space among them.
  size_t index = 0;
  // The folder it's in there, a pinned one or a today's, or 0 for none.
  bc_id folder = 0;
};

// Where |tab| is among |rows|, which PinnedWith or TodayWith made with |folders|: where Put would
// put it now, so where the app draws its row meanwhile. None for a tab that isn't among them.
std::optional<Place> PlaceOf(const std::vector<bc_id>& rows,
                             const std::map<bc_id, bc_id>& folders,
                             bc_id tab);

// Puts |tab|, a Today tab of |home| again as a tab moved to a Space is, back where it left with
// |node|: among the Today tabs, or with |pin| pinned at its address, in the folder that held it
// among what that holds now. Once that folder is gone it's in the one that held the folder, where
// the folder was, and outside every folder once all of them are. A Today tab that was in
// |today_folder| is in it again, among its tabs, as its first or last tab too, while |home| has
// that folder, and loose at its place once it's gone: no folder is made for it. Then into the split
// view it left, which had |split|, itself among them, beside the first of those still in its list
// and between the ones it was between: it stays loose, where the split was, if none is left there
// or their split is full by now. Its node is |node| again and the rows around it, and the today's
// folders, keep theirs, so the account's other devices see no move.
void Put(Space& home,
         bc_id tab,
         const Space::Node& node,
         const std::vector<bc_id>& split,
         const std::optional<Pin>& pin,
         bc_id today_folder);

}  // namespace browser_core::handed_back

#endif  // BROWSER_CORE_HANDED_BACK_H_
