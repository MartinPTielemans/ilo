// The strips along a window's sides that a page reaching them leaves to the UI, apart so
// browser_core_unittests tests the rule alone.
#ifndef BROWSER_CORE_PAGE_EDGE_STRIPS_H_
#define BROWSER_CORE_PAGE_EDGE_STRIPS_H_

#include "ui/gfx/geometry/rect.h"

namespace browser_core {

// The strip's width in DIPs, the resize edge's (docs/ux-spec.md › Dragging the window): with the
// sidebar hidden the page reaches the window's sides, and the pointer there still brings the
// sidebar out (#3219) and, on Linux, resizes the window, which only the UI can hear over a page's
// view. A window in full screen has none, so a page there keeps its edges' presses.
inline constexpr int kPageEdgeStrip = 3;

// What of |page| takes the pointer in a window |width| across: all of it but the strips |strip|
// wide along the window's sides. Empty where nothing is left. A page short of the strips, as one
// beside the docked sidebar, is all of it.
gfx::Rect PageWithoutEdgeStrips(const gfx::Rect& page, int width, int strip);

// Whether |x|, across a window |width| wide, is in a strip |strip| wide along either side.
bool InPageEdgeStrip(double x, double width, double strip);

}  // namespace browser_core

#endif  // BROWSER_CORE_PAGE_EDGE_STRIPS_H_
