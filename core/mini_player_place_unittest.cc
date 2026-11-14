#include "browser_core/mini_player_place.h"

#include <cmath>

#include "testing/gtest/include/gtest/gtest.h"
#include "ui/gfx/geometry/point.h"

namespace browser_core {
namespace {

// A 1728 × 1117 screen under a 37 pt menu bar, as Arc's was measured on.
constexpr gfx::Rect kWorkArea(0, 37, 1728, 1080);

TEST(MiniPlayerPlaceTest, TheFirstShowsTenInFromTheTopRight) {
  EXPECT_EQ(FirstMiniPlayerPlace(gfx::Size(345, 194), std::nullopt, kWorkArea),
            gfx::Rect(1728 - 10 - 345, 37 + 10, 345, 194));
}

TEST(MiniPlayerPlaceTest, OneOfAnotherShapeKeepsTheCornerTheLastWasLeftAt) {
  // Left at the bottom left: a taller one grows up from its bottom edge.
  const gfx::Rect bottom_left(20, 37 + 1080 - 20 - 194, 345, 194);
  EXPECT_EQ(FirstMiniPlayerPlace(gfx::Size(300, 300), bottom_left, kWorkArea),
            gfx::Rect(20, 37 + 1080 - 20 - 300, 300, 300));
  // Left at the top right, as the first goes: a narrower one keeps its right edge.
  const gfx::Rect top_right(1728 - 10 - 345, 47, 345, 194);
  EXPECT_EQ(FirstMiniPlayerPlace(gfx::Size(258, 194), top_right, kWorkArea),
            gfx::Rect(1728 - 10 - 258, 47, 258, 194));
}

TEST(MiniPlayerPlaceTest, ItsKeptOnTheScreen) {
  // Left partly off the screen's right edge, as a drag can leave it.
  EXPECT_EQ(FirstMiniPlayerPlace(gfx::Size(345, 194), gfx::Rect(1600, 500, 345, 194), kWorkArea),
            gfx::Rect(1728 - 345, 500, 345, 194));
  // A screen too small for the inset.
  EXPECT_EQ(FirstMiniPlayerPlace(gfx::Size(345, 194), std::nullopt, gfx::Rect(0, 0, 350, 200)),
            gfx::Rect(0, 6, 345, 194));
}

// A Mini Player of the first's size, 10 in from the work area's left at x 10 and its right at
// 1373, its top at y 47 and its bottom at 913.
constexpr gfx::Size kSize(345, 194);

gfx::Rect At(int x, int y) {
  return gfx::Rect(gfx::Point(x, y), kSize);
}

TEST(MiniPlayerRestTest, AThrowLandsItInTheCornerWithinFortyFiveDegrees) {
  EXPECT_EQ(MiniPlayerRest(At(1373, 47), gfx::Vector2dF(-1500, 900), gfx::Vector2dF(), kWorkArea),
            At(10, 913));
  // Its last move alone throws it, from a release too slow to.
  EXPECT_EQ(MiniPlayerRest(At(700, 480), gfx::Vector2dF(), gfx::Vector2dF(8, 8), kWorkArea),
            At(1373, 913));
}

TEST(MiniPlayerRestTest, AThrowWithNoCornerThatNearGoesOnToTheSide) {
  // Left from near the left side: both left corners are more than 45° off.
  EXPECT_EQ(MiniPlayerRest(At(100, 480), gfx::Vector2dF(-2000, 0), gfx::Vector2dF(), kWorkArea),
            At(10, 480));
}

TEST(MiniPlayerRestTest, AReleaseNearASideOrTwoCatchesItThere) {
  EXPECT_EQ(MiniPlayerRest(At(40, 480), gfx::Vector2dF(100, 0), gfx::Vector2dF(), kWorkArea),
            At(10, 480));
  EXPECT_EQ(MiniPlayerRest(At(1350, 900), gfx::Vector2dF(), gfx::Vector2dF(), kWorkArea),
            At(1373, 913));
  // Past the side, as a drag can leave it partly off the screen.
  EXPECT_EQ(MiniPlayerRest(At(-50, 480), gfx::Vector2dF(), gfx::Vector2dF(), kWorkArea),
            At(10, 480));
}

TEST(MiniPlayerRestTest, ASlowReleaseAwayFromTheSidesStays) {
  EXPECT_EQ(MiniPlayerRest(At(700, 480), gfx::Vector2dF(300, 0), gfx::Vector2dF(5, 0), kWorkArea),
            std::nullopt);
}

TEST(MiniPlayerSpringTest, ItStartsAtTheReleasesSpeedAndSettles) {
  // The throw from the top right to the bottom left above, from where it's let go.
  const gfx::Vector2dF offset(1363, -866);
  const gfx::Vector2dF velocity(-1500, 900);
  EXPECT_EQ(MiniPlayerSpring(offset, velocity, 0), offset);
  const gfx::Vector2dF moved = MiniPlayerSpring(offset, velocity, 1e-4) - offset;
  EXPECT_NEAR(moved.x() / 1e-4, velocity.x(), 5);
  EXPECT_NEAR(moved.y() / 1e-4, velocity.y(), 5);
  const double settle = MiniPlayerSettleSeconds(offset, velocity);
  EXPECT_GT(settle, 0.9);
  EXPECT_LT(settle, 1.6);
  for (double seconds = settle; seconds < settle + 1; seconds += 0.01) {
    EXPECT_LE(MiniPlayerSpring(offset, velocity, seconds).Length(), 0.5 * std::sqrt(2.0))
        << seconds;
  }
  EXPECT_EQ(MiniPlayerSettleSeconds(gfx::Vector2dF(), gfx::Vector2dF()), 0);
}

}  // namespace
}  // namespace browser_core
