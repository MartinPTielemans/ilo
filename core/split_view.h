#ifndef BROWSER_CORE_SPLIT_VIEW_H_
#define BROWSER_CORE_SPLIT_VIEW_H_

#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <vector>

#include "browser_core/public/browser_core.h"

namespace browser_core {

// Tabs shown together, side by side or stacked top to bottom, as one row of a Space (space.h) or
// one favorite (favorites.h). Its tabs sit together in their list, in its order.
struct SplitView {
  // In their order, as they are in their list.
  std::vector<bc_id> tabs;
  bool stacked = false;
  // Each tab's share of its length, or none while they're even.
  std::vector<double> shares;
  // The tab that took the card, else 0, while the others are kept at 220 pt.
  bc_id expanded = 0;
  // The tab focused last, which showing the split shows focused, else 0 for its first. A
  // favorite's keeps it whichever Space it was focused in; a Space's window remembers the tab it
  // showed there instead.
  bc_id focused = 0;
};

// The split in |splits| holding |tab|, else null.
SplitView* SplitHolding(std::vector<SplitView>& splits, bc_id tab);
const SplitView* SplitHolding(const std::vector<SplitView>& splits, bc_id tab);

// |shares| for a split of |count| tabs, scaled to add up to 1, or none if there are none, as for
// even shares. Nothing if they aren't one above 0 for each tab.
std::optional<std::vector<double>> ScaledShares(std::vector<double> shares, size_t count);

// The favorites' splits follow their tabs as the favorites' scope copies and merges the lists
// (favorites.h). A tab's key names its split in its list, the same for each of the split's tabs
// there and 0 for a tab in none.

// The splits |list| holds by |key|: each key's tabs in |list|'s order, for each key with two tabs
// or more there, in the order of their first tabs.
std::vector<std::vector<bc_id>> SplitsIn(const std::vector<bc_id>& list,
                                         const std::map<bc_id, int64_t>& key);

// |list| with each split's tabs by |key| together, at the place of its first, in their order.
std::vector<bc_id> Gathered(const std::vector<bc_id>& list, const std::map<bc_id, int64_t>& key);

// A favorite, or a favorite split, as the scopes' merges match them: its tabs and their URLs in its
// order, and how much it's worth keeping, a page shown being worth more than one loaded.
struct FavoriteUnit {
  std::vector<bc_id> tabs;
  std::vector<std::string> urls;
  int rank = 0;
};

// Merges |lists|, each one list's favorites in its order, into one: the first list's, then each
// later list's extras. One is another's duplicate if they have the same URLs in the same order, and
// as many like it come before it in its list, so a split never merges with a lone favorite of one
// of its pages, nor with a split of the same pages in another order. Of duplicates the one ranked
// higher stays where the first was, the first on a tie. Adds the tabs of those that don't stay to
// |losers|.
std::vector<FavoriteUnit> MergeUnits(const std::vector<std::vector<FavoriteUnit>>& lists,
                                     std::vector<bc_id>& losers);

}  // namespace browser_core

#endif  // BROWSER_CORE_SPLIT_VIEW_H_
