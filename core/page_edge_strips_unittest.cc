#include "browser_core/page_edge_strips.h"

#include "testing/gtest/include/gtest/gtest.h"
#include "ui/gfx/geometry/rect.h"

namespace browser_core {
namespace {

// A window 1200 across, in pixels at a scale of 2, its strips 6 wide.
constexpr int kWidth = 1200;
constexpr int kStrip = 6;

TEST(PageEdgeStripsTest, APageTheWholeWindowLeavesBothSides) {
  EXPECT_EQ(PageWithoutEdgeStrips(gfx::Rect(0, 0, kWidth, 800), kWidth, kStrip),
            gfx::Rect(kStrip, 0, kWidth - 2 * kStrip, 800));
}

TEST(PageEdgeStripsTest, APageShortOfTheSidesKeepsItAll) {
  // Beside the docked sidebar, 16 in from the window's top, right and bottom.
  const gfx::Rect docked(480, 16, kWidth - 480 - 16, 768);
  EXPECT_EQ(PageWithoutEdgeStrips(docked, kWidth, kStrip), docked);
}

TEST(PageEdgeStripsTest, PanesLeaveOnlyTheSideTheyReach) {
  // A split's two panes, the window's width between them, 20 apart.
  EXPECT_EQ(PageWithoutEdgeStrips(gfx::Rect(0, 0, 590, 800), kWidth, kStrip),
            gfx::Rect(kStrip, 0, 590 - kStrip, 800));
  EXPECT_EQ(PageWithoutEdgeStrips(gfx::Rect(610, 0, 590, 800), kWidth, kStrip),
            gfx::Rect(610, 0, 590 - kStrip, 800));
}

TEST(PageEdgeStripsTest, APageInsideAStripHasNothingLeft) {
  EXPECT_TRUE(PageWithoutEdgeStrips(gfx::Rect(0, 0, 4, 800), kWidth, kStrip).IsEmpty());
  EXPECT_TRUE(PageWithoutEdgeStrips(gfx::Rect(0, 0, kWidth, 0), kWidth, kStrip).IsEmpty());
}

TEST(PageEdgeStripsTest, NoStripLeavesThePage) {
  const gfx::Rect page(0, 0, kWidth, 800);
  EXPECT_EQ(PageWithoutEdgeStrips(page, kWidth, 0), page);
}

TEST(PageEdgeStripsTest, AStripRunsFromTheEdgeToItsWidth) {
  EXPECT_TRUE(InPageEdgeStrip(0, 800, 3));
  EXPECT_TRUE(InPageEdgeStrip(2.9, 800, 3));
  EXPECT_FALSE(InPageEdgeStrip(3, 800, 3));
  EXPECT_FALSE(InPageEdgeStrip(796.9, 800, 3));
  EXPECT_TRUE(InPageEdgeStrip(797, 800, 3));
  EXPECT_FALSE(InPageEdgeStrip(0, 800, 0));
}

}  // namespace
}  // namespace browser_core
