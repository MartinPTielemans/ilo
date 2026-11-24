// Which agent's tabs are tiles, and in which of the app's views, apart so browser_core_unittests
// tests the rule alone (#4115).

#ifndef BROWSER_CORE_TILE_RULE_H_
#define BROWSER_CORE_TILE_RULE_H_

#include <cstdint>

#include "base/containers/flat_map.h"
#include "base/containers/flat_set.h"

namespace browser_core {

class TileRule {
 public:
  // What bc_tab_show_tile does.
  enum class Shown {
    kNew,          // The page goes into the view.
    kFramed,       // Already there: only its frame changes.
    kMoved,        // In another view: it leaves that one for this one.
    kUsersTab,     // Refused: tiles are agents' tabs only.
    kWindowShows,  // Refused: a window shows the tab, which keeps it.
    kCardShows,    // Refused: a card shows the tab, with its input, which keeps it.
    kNoPlatform,   // Refused: this platform holds one page a window, so the app draws a picture.
    kNoView,       // Refused: no view to show it in.
  };

  // bc_tab_show_tile of |tab| in |view|. Every refusal leaves what the tab was.
  Shown Show(uint64_t tab, const void* view, bool agent_tab, bool window_shows, bool platform_can);

  // bc_tab_end_tile. Returns the view it was in, or null if it wasn't a tile, as after its tab or
  // view went. A card keeps the tab. The page is parked: out of sight but still in that view's
  // window, so it stays visible to itself, until a tile, a card or a window has it, or it goes
  // (#3788).
  const void* End(uint64_t tab);

  // bc_tab_show_in: a card has |tab|'s page from now on, until Left. Ends its tile, returning the
  // view it was in, or null.
  const void* ShowInCard(uint64_t tab);

  // A window showing |tab|, or its page going: neither a tile nor a card has it now. Returns the
  // view of the tile it was, or null.
  const void* Left(uint64_t tab);

  // Whether End parked |tab|'s page and nothing has had it since.
  bool Parked(uint64_t tab) const;

  // Whether |tab| is a tile, which a page's focus(), an activation and a commit's focus are
  // refused for. A tab it never held, or holds no more, as a closed one, is as Chromium has it.
  bool Holds(uint64_t tab) const;

  // The word the core's log line uses for |shown|.
  static const char* Name(Shown shown);

 private:
  // Takes |tab| out of its tile, returning the view it was in, or null.
  const void* Take(uint64_t tab);

  base::flat_map<uint64_t, const void*> views_;
  base::flat_set<uint64_t> cards_;
  base::flat_set<uint64_t> parked_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_TILE_RULE_H_
