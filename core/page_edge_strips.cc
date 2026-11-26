#include "browser_core/page_edge_strips.h"

#include <algorithm>

namespace browser_core {

namespace {

bool Left(EdgeSides sides) {
  return sides == EdgeSides::kLeft || sides == EdgeSides::kBoth;
}

bool Right(EdgeSides sides) {
  return sides == EdgeSides::kRight || sides == EdgeSides::kBoth;
}

}  // namespace

EdgeSides SidebarEdge(bool little, bool sidebar_right) {
  if (little) {
    return EdgeSides::kNone;
  }
  return sidebar_right ? EdgeSides::kRight : EdgeSides::kLeft;
}

gfx::Rect PageWithoutEdgeStrips(const gfx::Rect& page, int width, int strip, EdgeSides sides) {
  const int left = Left(sides) ? std::max(page.x(), strip) : page.x();
  const int right = Right(sides) ? std::min(page.right(), width - strip) : page.right();
  if (right <= left || page.height() <= 0) {
    return gfx::Rect();
  }
  return gfx::Rect(left, page.y(), right - left, page.height());
}

bool InPageEdgeStrip(double x, double width, double strip, EdgeSides sides) {
  return (Left(sides) && x < strip) || (Right(sides) && x >= width - strip);
}

}  // namespace browser_core
