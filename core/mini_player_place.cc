#include "browser_core/mini_player_place.h"

#include "ui/gfx/geometry/point.h"

namespace browser_core {

gfx::Rect FirstMiniPlayerPlace(const gfx::Size& size,
                               const std::optional<gfx::Rect>& left,
                               const gfx::Rect& work_area) {
  gfx::Rect placed(size);
  if (!left) {
    placed.set_origin(gfx::Point(work_area.right() - kMiniPlayerInset - size.width(),
                                 work_area.y() + kMiniPlayerInset));
  } else {
    const gfx::Point middle = work_area.CenterPoint();
    placed.set_origin(gfx::Point(
        left->CenterPoint().x() > middle.x() ? left->right() - size.width() : left->x(),
        left->CenterPoint().y() > middle.y() ? left->bottom() - size.height() : left->y()));
  }
  placed.AdjustToFit(work_area);
  return placed;
}

}  // namespace browser_core
