// Tiles (#4115): an agent's page shown live in a view of the app's, for the user to watch it work,
// taking no input there (bc_tab_show_tile). Which tabs may be tiles, and where, is TileRule's.

#ifndef BROWSER_CORE_TILE_H_
#define BROWSER_CORE_TILE_H_

#include "browser_core/public/browser_core.h"
#include "ui/gfx/geometry/rect.h"

namespace browser_core::tiles {

// bc_tab_show_tile.
bool Show(bc_id tab, void* view, const gfx::Rect& frame, double corner_radius);

// bc_tab_end_tile, a window or a card showing |tab|, and its page going: takes the page out of its
// tile. Nothing if it isn't one.
void End(bc_id tab);

// Whether |tab| is a tile, whose page's own asks to be activated are refused.
bool Holds(bc_id tab);

}  // namespace browser_core::tiles

#endif  // BROWSER_CORE_TILE_H_
