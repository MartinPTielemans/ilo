// Where a video's Mini Player first shows, and where a drag of it comes to rest, apart so
// browser_core_unittests tests the rules alone.
#ifndef BROWSER_CORE_MINI_PLAYER_PLACE_H_
#define BROWSER_CORE_MINI_PLAYER_PLACE_H_

#include <optional>

#include "ui/gfx/geometry/rect.h"
#include "ui/gfx/geometry/size.h"
#include "ui/gfx/geometry/vector2d_f.h"

namespace browser_core {

// Arc's first Mini Player shows this far in from the top and right of its screen's work area (an
// outside reverse-engineering of Arc 1.168, #1407). A throw lands it as far in, as Dia's does.
inline constexpr int kMiniPlayerInset = 10;

// A release faster than this, in points a second, throws the Mini Player, as does one whose last
// move went further than kMiniPlayerThrowMove points (Dia 1.53.0 bin).
inline constexpr float kMiniPlayerThrowSpeed = 350;
inline constexpr float kMiniPlayerThrowMove = 6;

// A release this near its place along a side, kMiniPlayerInset in, catches it there (ours, #2099).
inline constexpr int kMiniPlayerCatch = 40;

// Where a Mini Player of |size| first shows, on a screen whose work area is |work_area|: the first
// one 10 in from its top right, as Arc's, and after that where the last was left, |left|, at its
// corner nearest the screen's, so a video of another shape sits as the last did. Kept on the screen.
gfx::Rect FirstMiniPlayerPlace(const gfx::Size& size,
                               const std::optional<gfx::Rect>& left,
                               const gfx::Rect& work_area);

// Where a Mini Player let go at |let_go|, moving at |velocity| points a second, its last move
// |last_move|, comes to rest on the screen whose work area is |work_area|, or none where it stays.
// A throw takes it to the corner within 45° of the throw, kMiniPlayerInset in, as Dia's does, or
// with none that near, along the throw to the side it meets. A release near a side puts it along
// that side, and near two in their corner, as far in.
std::optional<gfx::Rect> MiniPlayerRest(const gfx::Rect& let_go,
                                        const gfx::Vector2dF& velocity,
                                        const gfx::Vector2dF& last_move,
                                        const gfx::Rect& work_area);

// The spring a Mini Player settles on, Dia's response of 0.9 s at a damping ratio of 0.9 (Dia
// 1.53.0 bin): how far from its rest it is |seconds| after starting |offset| from it, moving at
// |velocity| points a second.
gfx::Vector2dF MiniPlayerSpring(const gfx::Vector2dF& offset,
                                const gfx::Vector2dF& velocity,
                                double seconds);

// How long that spring takes to come within half a point of its rest and stay there.
double MiniPlayerSettleSeconds(const gfx::Vector2dF& offset, const gfx::Vector2dF& velocity);

}  // namespace browser_core

#endif  // BROWSER_CORE_MINI_PLAYER_PLACE_H_
