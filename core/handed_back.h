// The way back of a tab the user handed an agent, into the Space it left: what's kept of a pinned
// tab's pin meanwhile, and the putting back once the tab is one of that Space's Today tabs again.
// Only the Space is asked, apart from handed.cc so browser_core_unittests tests it with Space
// alone. Where it goes is handed_place's.

#ifndef BROWSER_CORE_HANDED_BACK_H_
#define BROWSER_CORE_HANDED_BACK_H_

#include <cstddef>
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

// Puts |tab|, a Today tab of |home| again as a tab moved to a Space is, back where it left with
// |node|: among the Today tabs, or with |pin| pinned at its address, in the folder that held it
// among what that holds now. Once that folder is gone it's in the one that held the folder, where
// the folder was, and outside every folder once all of them are. Then into the split view it
// left, which had |split|, itself among them, beside the first of those still in its list and
// between the ones it was between: it stays loose, where the split was, if none is left there or
// their split is full by now. Its node is |node| again and the rows around it keep theirs, so the
// account's other devices see no move.
void Put(Space& home,
         bc_id tab,
         const Space::Node& node,
         const std::vector<bc_id>& split,
         const std::optional<Pin>& pin);

}  // namespace browser_core::handed_back

#endif  // BROWSER_CORE_HANDED_BACK_H_
