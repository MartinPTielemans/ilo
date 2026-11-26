#include "browser_core/page_edge_strips.h"

#include "testing/gtest/include/gtest/gtest.h"
#include "ui/gfx/geometry/rect.h"

namespace browser_core {
namespace {

// A window 1200 across, in pixels at a scale of 2, its strips 6 wide.
constexpr int kWidth = 1200;
constexpr int kStrip = 6;

TEST(PageEdgeStripsTest, APageTheWholeWindowLeavesBothSides) {
  EXPECT_EQ(PageWithoutEdgeStrips(gfx::Rect(0, 0, kWidth, 800), kWidth, kStrip, EdgeSides::kBoth),
            gfx::Rect(kStrip, 0, kWidth - 2 * kStrip, 800));
}

TEST(PageEdgeStripsTest, APageShortOfTheSidesKeepsItAll) {
  // Beside the docked sidebar, 16 in from the window's top, right and bottom.
  const gfx::Rect docked(480, 16, kWidth - 480 - 16, 768);
  EXPECT_EQ(PageWithoutEdgeStrips(docked, kWidth, kStrip, EdgeSides::kBoth), docked);
}

TEST(PageEdgeStripsTest, PanesLeaveOnlyTheSideTheyReach) {
  // A split's two panes, the window's width between them, 20 apart.
  EXPECT_EQ(PageWithoutEdgeStrips(gfx::Rect(0, 0, 590, 800), kWidth, kStrip, EdgeSides::kBoth),
            gfx::Rect(kStrip, 0, 590 - kStrip, 800));
  EXPECT_EQ(PageWithoutEdgeStrips(gfx::Rect(610, 0, 590, 800), kWidth, kStrip, EdgeSides::kBoth),
            gfx::Rect(610, 0, 590 - kStrip, 800));
}

TEST(PageEdgeStripsTest, APageInsideAStripHasNothingLeft) {
  EXPECT_TRUE(
      PageWithoutEdgeStrips(gfx::Rect(0, 0, 4, 800), kWidth, kStrip, EdgeSides::kBoth).IsEmpty());
  EXPECT_TRUE(PageWithoutEdgeStrips(gfx::Rect(0, 0, kWidth, 0), kWidth, kStrip, EdgeSides::kBoth)
                  .IsEmpty());
}

TEST(PageEdgeStripsTest, NoStripLeavesThePage) {
  const gfx::Rect page(0, 0, kWidth, 800);
  EXPECT_EQ(PageWithoutEdgeStrips(page, kWidth, 0, EdgeSides::kBoth), page);
}

TEST(PageEdgeStripsTest, AStripRunsFromTheEdgeToItsWidth) {
  EXPECT_TRUE(InPageEdgeStrip(0, 800, 3, EdgeSides::kBoth));
  EXPECT_TRUE(InPageEdgeStrip(2.9, 800, 3, EdgeSides::kBoth));
  EXPECT_FALSE(InPageEdgeStrip(3, 800, 3, EdgeSides::kBoth));
  EXPECT_FALSE(InPageEdgeStrip(796.9, 800, 3, EdgeSides::kBoth));
  EXPECT_TRUE(InPageEdgeStrip(797, 800, 3, EdgeSides::kBoth));
  EXPECT_FALSE(InPageEdgeStrip(0, 800, 0, EdgeSides::kBoth));
}

TEST(PageEdgeStripsTest, OneSideLeavesOnlyThatStrip) {
  const gfx::Rect page(0, 0, kWidth, 800);
  EXPECT_EQ(PageWithoutEdgeStrips(page, kWidth, kStrip, EdgeSides::kLeft),
            gfx::Rect(kStrip, 0, kWidth - kStrip, 800));
  EXPECT_EQ(PageWithoutEdgeStrips(page, kWidth, kStrip, EdgeSides::kRight),
            gfx::Rect(0, 0, kWidth - kStrip, 800));
  EXPECT_EQ(PageWithoutEdgeStrips(page, kWidth, kStrip, EdgeSides::kNone), page);
  EXPECT_TRUE(InPageEdgeStrip(1, 800, 3, EdgeSides::kLeft));
  EXPECT_FALSE(InPageEdgeStrip(799, 800, 3, EdgeSides::kLeft));
  EXPECT_TRUE(InPageEdgeStrip(799, 800, 3, EdgeSides::kRight));
  EXPECT_FALSE(InPageEdgeStrip(1, 800, 3, EdgeSides::kRight));
  EXPECT_FALSE(InPageEdgeStrip(1, 800, 3, EdgeSides::kNone));
  EXPECT_FALSE(InPageEdgeStrip(799, 800, 3, EdgeSides::kNone));
}

TEST(PageEdgeStripsTest, TheMacHasTheSidebarsSideAndLittleIloNone) {
  EXPECT_EQ(SidebarEdge(false, false), EdgeSides::kLeft);
  EXPECT_EQ(SidebarEdge(false, true), EdgeSides::kRight);
  EXPECT_EQ(SidebarEdge(true, false), EdgeSides::kNone);
  EXPECT_EQ(SidebarEdge(true, true), EdgeSides::kNone);
}

// The rule takes only the page, the window's width and the sides, so a window in full screen,
// whose width is the screen's, keeps its strips as any other does.
TEST(PageEdgeStripsTest, FullScreenKeepsTheStrips) {
  constexpr int kScreen = 3024;
  EXPECT_EQ(
      PageWithoutEdgeStrips(gfx::Rect(0, 0, kScreen, 1964), kScreen, kStrip, EdgeSides::kBoth),
      gfx::Rect(kStrip, 0, kScreen - 2 * kStrip, 1964));
  EXPECT_TRUE(InPageEdgeStrip(0, kScreen, kStrip, EdgeSides::kBoth));
  EXPECT_TRUE(InPageEdgeStrip(kScreen - 1, kScreen, kStrip, EdgeSides::kBoth));
}

}  // namespace
}  // namespace browser_core
