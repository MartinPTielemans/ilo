#include "browser_core/tile_rule.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

using Shown = TileRule::Shown;

// Stand-ins for two of the app's views: the rule only compares them. One array, so the linker
// can't fold them into one address.
const int kViews[2] = {};
const int& kView = kViews[0];
const int& kOtherView = kViews[1];

TEST(TileRule, AnAgentsTabNoWindowShowsBecomesATile) {
  TileRule tiles;
  EXPECT_EQ(tiles.Show(1, &kView, /*agent_tab=*/true, /*window_shows=*/false,
                       /*platform_can=*/true),
            Shown::kNew);
  EXPECT_TRUE(tiles.Holds(1));
  EXPECT_FALSE(tiles.Holds(2));
}

TEST(TileRule, ASecondCallInTheSameViewOnlyFramesIt) {
  TileRule tiles;
  tiles.Show(1, &kView, true, false, true);
  EXPECT_EQ(tiles.Show(1, &kView, true, false, true), Shown::kFramed);
  EXPECT_EQ(tiles.End(1), &kView);
}

TEST(TileRule, ACallWithAnotherViewMovesItThere) {
  // One tile a tab: the old view gives it up.
  TileRule tiles;
  tiles.Show(1, &kView, true, false, true);
  EXPECT_EQ(tiles.Show(1, &kOtherView, true, false, true), Shown::kMoved);
  EXPECT_EQ(tiles.End(1), &kOtherView);
  EXPECT_FALSE(tiles.Holds(1));
}

TEST(TileRule, RefusalsLeaveTheTabAsItWas) {
  TileRule tiles;
  EXPECT_EQ(tiles.Show(1, &kView, /*agent_tab=*/false, false, true), Shown::kUsersTab);
  EXPECT_EQ(tiles.Show(2, &kView, true, /*window_shows=*/true, true), Shown::kWindowShows);
  EXPECT_EQ(tiles.Show(3, nullptr, true, false, true), Shown::kNoView);
  for (uint64_t tab : {1u, 2u, 3u}) {
    EXPECT_FALSE(tiles.Holds(tab));
  }
  // A refused call for a tile in its view keeps it there.
  tiles.Show(4, &kView, true, false, true);
  EXPECT_EQ(tiles.Show(4, &kOtherView, true, /*window_shows=*/true, true), Shown::kWindowShows);
  EXPECT_EQ(tiles.End(4), &kView);
}

TEST(TileRule, ACardShowingTheTabKeepsIt) {
  // bc_tab_show_in gives the page its input, as for a hand-off: a tile would take it from the user.
  TileRule tiles;
  EXPECT_EQ(tiles.ShowInCard(1), nullptr);
  EXPECT_EQ(tiles.Show(1, &kView, true, false, true), Shown::kCardShows);
  EXPECT_FALSE(tiles.Holds(1));
  // bc_tab_end_tile is the tile's way out, never the card's.
  EXPECT_EQ(tiles.End(1), nullptr);
  EXPECT_EQ(tiles.Show(1, &kView, true, false, true), Shown::kCardShows);
}

TEST(TileRule, ACardTakesATilesPage) {
  TileRule tiles;
  tiles.Show(1, &kView, true, false, true);
  EXPECT_EQ(tiles.ShowInCard(1), &kView);
  EXPECT_FALSE(tiles.Holds(1));
  EXPECT_EQ(tiles.Show(1, &kOtherView, true, false, true), Shown::kCardShows);
}

TEST(TileRule, AWindowOrTheTabClosingLetsTheCardGo) {
  TileRule tiles;
  tiles.ShowInCard(1);
  EXPECT_EQ(tiles.Left(1), nullptr);
  EXPECT_EQ(tiles.Show(1, &kView, true, false, true), Shown::kNew);
  EXPECT_EQ(tiles.Left(1), &kView);
  EXPECT_FALSE(tiles.Holds(1));
}

TEST(TileRule, APlatformWithOnePageAWindowShowsNoTile) {
  // Windows and Linux today: the app draws the tab's picture instead.
  TileRule tiles;
  EXPECT_EQ(tiles.Show(1, &kView, true, false, /*platform_can=*/false), Shown::kNoPlatform);
  EXPECT_FALSE(tiles.Holds(1));
}

TEST(TileRule, EndingIsTheWayBackAndHappensOnce) {
  // bc_tab_end_tile, a window showing the tab, the tab closing and its view going all end it; the
  // one that comes second finds nothing.
  TileRule tiles;
  tiles.Show(1, &kView, true, false, true);
  EXPECT_EQ(tiles.End(1), &kView);
  EXPECT_EQ(tiles.End(1), nullptr);
  EXPECT_FALSE(tiles.Holds(1));
}

TEST(TileRule, AClosedOrUnknownTabIsAsChromiumHasIt) {
  // The hooks default to Chromium's own answer for any tab the rule doesn't hold.
  TileRule tiles;
  EXPECT_FALSE(tiles.Holds(7));
  EXPECT_EQ(tiles.End(7), nullptr);
  tiles.Show(7, &kView, true, false, true);
  tiles.End(7);  // As the tab's closing does.
  EXPECT_FALSE(tiles.Holds(7));
}

TEST(TileRule, AnEndedTileIsParkedUntilATileHasItsPageAgain) {
  // An agent's page stays visible from its opening to its close, whatever the user looks at
  // (#3788).
  TileRule tiles;
  tiles.Show(1, &kView, true, false, true);
  EXPECT_FALSE(tiles.Parked(1));
  EXPECT_EQ(tiles.End(1), &kView);
  EXPECT_TRUE(tiles.Parked(1));
  EXPECT_FALSE(tiles.Holds(1));
  EXPECT_EQ(tiles.Show(1, &kOtherView, true, false, true), Shown::kNew);
  EXPECT_FALSE(tiles.Parked(1));
}

TEST(TileRule, ACardAWindowOrThePageGoingTakesAParkedPage) {
  TileRule tiles;
  tiles.Show(1, &kView, true, false, true);
  tiles.End(1);
  EXPECT_EQ(tiles.Show(1, &kView, true, /*window_shows=*/true, true), Shown::kWindowShows);
  EXPECT_TRUE(tiles.Parked(1));
  EXPECT_EQ(tiles.Left(1), nullptr);
  EXPECT_FALSE(tiles.Parked(1));
  tiles.Show(1, &kView, true, false, true);
  tiles.End(1);
  EXPECT_EQ(tiles.ShowInCard(1), nullptr);
  EXPECT_FALSE(tiles.Parked(1));
}

TEST(TileRule, OnlyATilesPageIsParked) {
  TileRule tiles;
  EXPECT_EQ(tiles.End(7), nullptr);
  EXPECT_FALSE(tiles.Parked(7));
  tiles.ShowInCard(8);
  EXPECT_EQ(tiles.End(8), nullptr);
  EXPECT_FALSE(tiles.Parked(8));
}

}  // namespace
}  // namespace browser_core
