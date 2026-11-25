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

// The split view it left has the tabs it had: it goes back between the ones it was between.
TEST(HandedPlaceTest, BackBetweenTheTabsOfItsSplit) {
  EXPECT_EQ(InSplit({true, false}), 1u);
}

TEST(HandedPlaceTest, BackAtTheEndOfItsSplitThatItWasAt) {
  EXPECT_EQ(InSplit({false, false}), 0u);
  EXPECT_EQ(InSplit({true, true}), 2u);
}

// One of the tabs it was beside closed or left the split since.
TEST(HandedPlaceTest, BackBesideTheOneTabLeftOfItsSplit) {
  EXPECT_EQ(InSplit({true}), 1u);
  EXPECT_EQ(InSplit({false}), 0u);
}

// The user moved the split's tabs about since: after the last of them that was before it.
TEST(HandedPlaceTest, AfterTheLastTabThatWasBeforeIt) {
  EXPECT_EQ(InSplit({false, true}), 2u);
}

// A tab put in the split since was at neither side of it, and doesn't move it.
TEST(HandedPlaceTest, ATabAddedToItsSplitSinceDoesntMoveIt) {
  EXPECT_EQ(InSplit({true, false, false}), 1u);
}

TEST(HandedPlaceTest, APinnedTabGoesBackIntoTheFolderThatHeldIt) {
  EXPECT_EQ(InFolder({true}), 0u);
  EXPECT_EQ(InFolder({true, true}), 0u);
  // Whatever became of the folders around it.
  EXPECT_EQ(InFolder({true, false}), 0u);
}

TEST(HandedPlaceTest, OnceItsFolderIsGoneIntoTheOneThatHeldThat) {
  EXPECT_EQ(InFolder({false, true}), 1u);
  EXPECT_EQ(InFolder({false, false, true}), 2u);
}

TEST(HandedPlaceTest, OutsideEveryFolderOnceAllItWasInAreGone) {
  EXPECT_EQ(InFolder({false}), 1u);
  EXPECT_EQ(InFolder({false, false}), 2u);
}

TEST(HandedPlaceTest, OutsideEveryFolderIfItWasInNone) {
  EXPECT_EQ(InFolder({}), 0u);
}

TEST(HandedPlaceTest, AFavoriteGoesBackOntoItsTile) {
  EXPECT_TRUE(OnItsTile({.favorite = true, .same_profile = true}));
}

// Removed by the user, moved out of the favorites into a Space, or closed by a change of whose
// favorites they are: the tab is a Today tab then.
TEST(HandedPlaceTest, NotOntoATileThatIsAFavoriteNoLonger) {
  EXPECT_FALSE(OnItsTile({.favorite = false, .same_profile = true}));
}

TEST(HandedPlaceTest, NotOntoATileOfAnotherProfile) {
  EXPECT_FALSE(OnItsTile({.favorite = true, .same_profile = false}));
}

// The user's click on the tile, a key that shows it, a load or an agent's call: none gives it a
// page.
TEST(HandedPlaceTest, TheTabKeepingAHandedFavoritesTileGetsNoPage) {
  EXPECT_TRUE(KeepsATile(/*named=*/true, /*favorite=*/true));
}

// The favorite's own tab is back on the tile, or closed in the agent's Space: a tab left in the
// favorites then is a favorite as any, at the address it was a favorite at.
TEST(HandedPlaceTest, AFavoriteNoMarkNamesGetsItsPage) {
  EXPECT_FALSE(KeepsATile(/*named=*/false, /*favorite=*/true));
}

// Moved into a Space by the user while it kept the tile: a tab of that Space as any.
TEST(HandedPlaceTest, OutOfTheFavoritesItGetsItsPage) {
  EXPECT_FALSE(KeepsATile(/*named=*/true, /*favorite=*/false));
}

}  // namespace
}  // namespace browser_core::handed_place
