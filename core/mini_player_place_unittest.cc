#include "browser_core/mini_player_place.h"

#include "testing/gtest/include/gtest/gtest.h"

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

}  // namespace
}  // namespace browser_core
