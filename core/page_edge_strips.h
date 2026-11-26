// The strips along a window's sides that a page reaching them leaves to the UI, apart so
// browser_core_unittests tests the rule alone.
#ifndef BROWSER_CORE_PAGE_EDGE_STRIPS_H_
#define BROWSER_CORE_PAGE_EDGE_STRIPS_H_

#include "ui/gfx/geometry/rect.h"

namespace browser_core {

// The strip's width in DIPs, the resize edge's (docs/ux-spec.md › Dragging the window): with the
// sidebar hidden the page reaches the window's sides, and the pointer there still brings the
// sidebar out (#3219) and, on Linux, resizes the window, which only the UI can hear over a page's
// view. A window in full screen keeps them: the pointer at its edge brings the sidebar out there
// too, so the rule has no full-screen input.
inline constexpr int kPageEdgeStrip = 3;

// Which of a window's sides have a strip. The Mac's has only the sidebar's side, and none in a
// Little ilo window, as macOS resizes the window itself; Linux and Windows have both.
enum class EdgeSides { kNone, kLeft, kRight, kBoth };

// The Mac's sides: the sidebar's, or none for a Little ilo window.
EdgeSides SidebarEdge(bool little, bool sidebar_right);

// What of |page| takes the pointer in a window |width| across: all of it but the strips |strip|
// wide along the window's |sides|. Empty where nothing is left. A page short of the strips, as one
// beside the docked sidebar, is all of it.
gfx::Rect PageWithoutEdgeStrips(const gfx::Rect& page, int width, int strip, EdgeSides sides);

// Whether |x|, across a window |width| wide, is in a strip |strip| wide along one of |sides|.
bool InPageEdgeStrip(double x, double width, double strip, EdgeSides sides);

}  // namespace browser_core

#endif  // BROWSER_CORE_PAGE_EDGE_STRIPS_H_
