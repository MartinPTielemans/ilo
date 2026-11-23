// Which agent's tabs are tiles, and in which of the app's views, apart so browser_core_unittests
// tests the rule alone (#4115).

#ifndef BROWSER_CORE_TILE_RULE_H_
#define BROWSER_CORE_TILE_RULE_H_

#include <cstdint>

#include "base/containers/flat_map.h"

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
    kNoPlatform,   // Refused: this platform holds one page a window, so the app draws a picture.
    kNoView,       // Refused: no view to show it in.
  };

  // bc_tab_show_tile of |tab| in |view|. Every refusal leaves what the tab was.
  Shown Show(uint64_t tab, const void* view, bool agent_tab, bool window_shows, bool platform_can);

  // bc_tab_end_tile, a window showing the tab, the tab closing, or its view going. Returns the
  // view it was in, or null if it wasn't a tile, as after any of those.
  const void* End(uint64_t tab);

  // Whether |tab| is a tile, which a page's focus(), an activation and a commit's focus are
  // refused for. A tab it never held, or holds no more, as a closed one, is as Chromium has it.
  bool Holds(uint64_t tab) const;

  // The word the core's log line uses for |shown|.
  static const char* Name(Shown shown);

 private:
  base::flat_map<uint64_t, const void*> views_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_TILE_RULE_H_
