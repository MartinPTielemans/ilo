#include "browser_core/mini_player_place.h"

#include <algorithm>
#include <cmath>
#include <limits>
#include <numbers>

#include "ui/gfx/geometry/point.h"
#include "ui/gfx/geometry/point_f.h"
#include "ui/gfx/geometry/vector2d.h"

namespace browser_core {

namespace {

// The spring's natural frequency, from its response, and its damping ratio.
constexpr double kOmega = 2 * std::numbers::pi / 0.9;
constexpr double kDamping = 0.9;

// The least and most a Mini Player's origin goes along one of the work area's axes, from |start|
// and |length| long, for a window |size| long: kMiniPlayerInset in from each end, or where the
// work area is too short for that, its middle.
struct Span {
  int least;
  int most;
};

Span SpanOf(int start, int length, int size) {
  const int least = start + kMiniPlayerInset;
  const int most = start + length - kMiniPlayerInset - size;
  if (most >= least) {
    return {least, most};
  }
  const int middle = start + (length - size) / 2;
  return {middle, middle};
}

// Where |at| is caught along |span|: at its nearer end, if it's within kMiniPlayerCatch of it or
// past it.
std::optional<int> Caught(int at, const Span& span) {
  const int from_least = at - span.least;
  const int from_most = span.most - at;
  if (from_least <= from_most) {
    return from_least <= kMiniPlayerCatch ? std::optional<int>(span.least) : std::nullopt;
  }
  return from_most <= kMiniPlayerCatch ? std::optional<int>(span.most) : std::nullopt;
}

// The amplitude the spring along one axis decays from, from |offset| and |velocity|.
double Amplitude(double offset, double velocity) {
  const double damped = kOmega * std::sqrt(1 - kDamping * kDamping);
  return std::hypot(offset, (velocity + kDamping * kOmega * offset) / damped);
}

double SpringAlong(double offset, double velocity, double seconds) {
  const double damped = kOmega * std::sqrt(1 - kDamping * kDamping);
  return std::exp(-kDamping * kOmega * seconds) *
         (offset * std::cos(damped * seconds) +
          (velocity + kDamping * kOmega * offset) / damped * std::sin(damped * seconds));
}

}  // namespace

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

std::optional<gfx::Rect> MiniPlayerRest(const gfx::Rect& let_go,
                                        const gfx::Vector2dF& velocity,
                                        const gfx::Vector2dF& last_move,
                                        const gfx::Rect& work_area) {
  const Span across = SpanOf(work_area.x(), work_area.width(), let_go.width());
  const Span down = SpanOf(work_area.y(), work_area.height(), let_go.height());
  gfx::Rect rest = let_go;
  const bool fast = velocity.Length() > kMiniPlayerThrowSpeed;
  if (!fast && last_move.Length() <= kMiniPlayerThrowMove) {
    const std::optional<int> x = Caught(let_go.x(), across);
    const std::optional<int> y = Caught(let_go.y(), down);
    if (!x && !y) {
      return std::nullopt;
    }
    rest.set_origin(gfx::Point(x.value_or(let_go.x()), y.value_or(let_go.y())));
    rest.AdjustToFit(work_area);
    return rest;
  }
  // A last move long enough to throw it, of a release too slow to, says which way better.
  const gfx::Vector2dF way = fast ? velocity : last_move;
  const gfx::PointF from(std::clamp(let_go.x(), across.least, across.most),
                         std::clamp(let_go.y(), down.least, down.most));
  double nearest = -1;
  for (const int x : {across.least, across.most}) {
    for (const int y : {down.least, down.most}) {
      const gfx::Vector2dF to = gfx::PointF(x, y) - from;
      // Not the corner it's thrown from.
      if (to.IsZero()) {
        continue;
      }
      const double cosine = gfx::DotProduct(way, to) / (way.Length() * to.Length());
      if (cosine > nearest) {
        nearest = cosine;
        rest.set_origin(gfx::Point(x, y));
      }
    }
  }
  if (nearest >= std::cos(std::numbers::pi / 4) - 1e-6) {
    return rest;
  }
  // No corner within 45°: on along the throw until it meets a side.
  double reach = std::numeric_limits<double>::infinity();
  if (way.x() != 0) {
    reach = std::min<double>(reach, ((way.x() > 0 ? across.most : across.least) - from.x()) / way.x());
  }
  if (way.y() != 0) {
    reach = std::min<double>(reach, ((way.y() > 0 ? down.most : down.least) - from.y()) / way.y());
  }
  rest.set_origin(gfx::Point(
      std::clamp(static_cast<int>(std::lround(from.x() + way.x() * reach)), across.least,
                 across.most),
      std::clamp(static_cast<int>(std::lround(from.y() + way.y() * reach)), down.least,
                 down.most)));
  return rest;
}

gfx::Vector2dF MiniPlayerSpring(const gfx::Vector2dF& offset,
                                const gfx::Vector2dF& velocity,
                                double seconds) {
  return gfx::Vector2dF(SpringAlong(offset.x(), velocity.x(), seconds),
                        SpringAlong(offset.y(), velocity.y(), seconds));
}

double MiniPlayerSettleSeconds(const gfx::Vector2dF& offset, const gfx::Vector2dF& velocity) {
  // Each axis is within its amplitude's envelope, which decays at the damping's rate.
  const double amplitude =
      std::max(Amplitude(offset.x(), velocity.x()), Amplitude(offset.y(), velocity.y()));
  return amplitude <= 0.5 ? 0 : std::log(amplitude / 0.5) / (kDamping * kOmega);
}

}  // namespace browser_core
