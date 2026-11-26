#include "browser_core/page_edge_strips.h"

#include <algorithm>

namespace browser_core {

gfx::Rect PageWithoutEdgeStrips(const gfx::Rect& page, int width, int strip) {
  const int left = std::max(page.x(), strip);
  const int right = std::min(page.right(), width - strip);
  if (right <= left || page.height() <= 0) {
    return gfx::Rect();
  }
  return gfx::Rect(left, page.y(), right - left, page.height());
}

bool InPageEdgeStrip(double x, double width, double strip) {
  return x < strip || x >= width - strip;
}

}  // namespace browser_core
