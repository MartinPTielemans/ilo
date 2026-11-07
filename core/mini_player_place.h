// Where a video's Mini Player first shows, apart so browser_core_unittests tests the rule alone.
#ifndef BROWSER_CORE_MINI_PLAYER_PLACE_H_
#define BROWSER_CORE_MINI_PLAYER_PLACE_H_

#include <optional>

#include "ui/gfx/geometry/rect.h"
#include "ui/gfx/geometry/size.h"

namespace browser_core {

// Arc's first Mini Player shows this far in from the top and right of its screen's work area (an
// outside reverse-engineering of Arc 1.168, #1407).
inline constexpr int kMiniPlayerInset = 10;

// Where a Mini Player of |size| first shows, on a screen whose work area is |work_area|: the first
// one 10 in from its top right, as Arc's, and after that where the last was left, |left|, at its
// corner nearest the screen's, so a video of another shape sits as the last did. Kept on the screen.
gfx::Rect FirstMiniPlayerPlace(const gfx::Size& size,
                               const std::optional<gfx::Rect>& left,
                               const gfx::Rect& work_area);

}  // namespace browser_core

#endif  // BROWSER_CORE_MINI_PLAYER_PLACE_H_
