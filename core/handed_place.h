// Where a tab the user handed an agent goes in the Space it left: among its Today tabs, or for a
// pinned tab in which folder and among what that holds, and in the split view it left. Apart so
// browser_core_unittests tests the rule alone. The session's save and the tab's return both ask it,
// so the place is one fact.

#ifndef BROWSER_CORE_HANDED_PLACE_H_
#define BROWSER_CORE_HANDED_PLACE_H_

#include <cstddef>
#include <vector>

namespace browser_core::handed_place {

// A Today tab of the Space the tab left, as that Space is now, or for a pinned tab a pinned tab or
// folder that the folder it goes back into holds.
struct Row {
  // Its place is before the one the tab kept from when it left.
  bool before = false;
  // It shows in a split view with the tab above it, which nothing comes between.
  bool joined_above = false;
};

// Where the tab goes among |today|, top to bottom: under the tabs whose places are before its own,
// so where it left however the tabs around it came and went since, and below a split view that
// would put it inside. At the top if no place was kept for it, where a tab moved to a Space goes.
size_t Among(const std::vector<Row>& today);

// Where the tab goes back into the split view it left, among the tabs that split has now, left to
// right: after the last of them that was before it when it left, so between the ones it was
// between whichever of them are still there, and first if none was. |was_before| says that of each
// tab the split has now, one added since being none that was.
size_t InSplit(const std::vector<bool>& was_before);

// Which of the folders a pinned tab was in it goes back into. |still_there| says of each, from the
// one that held it outward, whether the Space still has it. The first that is, by its index: the
// folder that held it, or once that's gone the one that held that. |still_there.size()| for the
// pinned tabs outside every folder, where one goes that was in no folder or whose folders are all
// gone.
size_t InFolder(const std::vector<bool>& still_there);

}  // namespace browser_core::handed_place

#endif  // BROWSER_CORE_HANDED_PLACE_H_
