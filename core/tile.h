// Tiles (#4115): an agent's page shown live in a view of the app's, for the user to watch it work,
// taking no input there (bc_tab_show_tile). Which tabs may be tiles, and where, is TileRule's.

#ifndef BROWSER_CORE_TILE_H_
#define BROWSER_CORE_TILE_H_

#include "browser_core/public/browser_core.h"
#include "ui/gfx/geometry/rect.h"

namespace browser_core::tiles {

// bc_tab_show_tile.
bool Show(bc_id tab, void* view, const gfx::Rect& frame, double corner_radius);

// Its page going: takes the page out of its tile or the park, and its window, and a card has it no
// more. Nothing if none had it.
void End(bc_id tab);

// A window showing |tab| takes its page, with its input, from its tile or the park, within the
// window, and a card has it no more. Nothing if none had it.
void TakenByWindow(bc_id tab);

// bc_tab_end_tile: takes the page out of its tile, parking it in the tile's window, so it stays
// visible (#3788). Nothing if it isn't a tile.
void EndTile(bc_id tab);

// bc_tab_show_in: a card has the page, with its input, from now on, until End. Takes it out of its
// tile first.
void ShowInCard(bc_id tab);

// A page from another site committed in |tab|, in a view of its own: if it's in a tile, live or
// parked, the view keeps the page's size.
void PageViewChanged(bc_id tab);

// Chrome is about to show a tab's dialog, as it's made or as its page becomes visible: each tile
// that holds its tab's dialog (TileRule::HoldsDialog) takes one its tab has now.
void DialogComing();

// Whether |tab|'s page is in a tile, live or parked, whose own asks to be activated are refused.
bool Holds(bc_id tab);

// Whether |tab|'s page stays crashed if it dies: in a window's pane or a tile, live or parked
// (TileRule::KeepsCrashed).
bool KeepsCrashed(bc_id tab);

// |tab|'s page died, or isn't dead any more: its tile leaves its place to the app's card under
// it, as a window's pane does, until then. Nothing if it's in no tile.
void Died(bc_id tab, bool died);

}  // namespace browser_core::tiles

#endif  // BROWSER_CORE_TILE_H_
