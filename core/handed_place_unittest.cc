#include "browser_core/handed_place.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core::handed_place {
namespace {

constexpr Row kBefore = {.before = true};
constexpr Row kAfter = {};

TEST(HandedPlaceTest, BackBetweenTheTabsItWasBetween) {
  EXPECT_EQ(Among({kBefore, kBefore, kAfter}), 2u);
}

TEST(HandedPlaceTest, BackAtTheTopOrTheBottomIfThatsWhereItWas) {
  EXPECT_EQ(Among({kAfter, kAfter}), 0u);
  EXPECT_EQ(Among({kBefore, kBefore}), 2u);
}

// The tabs that were around it closed, and the Space is empty.
TEST(HandedPlaceTest, AloneInASpaceWhoseOtherTabsClosed) {
  EXPECT_EQ(Among({}), 0u);
}

// Tabs opened since go at the top, before every place there was: it's still under the ones it was
// under, and over the ones it was over.
TEST(HandedPlaceTest, TabsOpenedSinceDontMoveIt) {
  EXPECT_EQ(Among({kBefore, kBefore, kBefore, kAfter}), 3u);
}

// The tabs it was between were put in a split view since.
TEST(HandedPlaceTest, NeverInsideASplitView) {
  const Row joined = {.joined_above = true};
  EXPECT_EQ(Among({kBefore, joined, kAfter}), 2u);
  const Row third = {.joined_above = true};
  EXPECT_EQ(Among({kBefore, joined, third}), 3u);
}

// A split view under its place is no reason to move.
TEST(HandedPlaceTest, OverASplitViewItStaysOverIt) {
  const Row joined = {.joined_above = true};
  EXPECT_EQ(Among({kBefore, kAfter, joined}), 1u);
}

// No place was kept for it, as for a tab whose Space keeps none.
TEST(HandedPlaceTest, AtTheTopWithNoPlaceKept) {
  EXPECT_EQ(Among({kAfter, kAfter, kAfter}), 0u);
}

}  // namespace
}  // namespace browser_core::handed_place
