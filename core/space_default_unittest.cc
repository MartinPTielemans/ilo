#include "browser_core/space_default.h"

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

TEST(SpaceDefaultTest, AnUntouchedSpaceGivesWay) {
  EXPECT_TRUE(GivesWay({}));
}

// Onboarding's "Pick a colour for your first Space" is setup: the Space still goes, with the tabs
// the user opened in it.
TEST(SpaceDefaultTest, AColourAloneGivesWay) {
  EXPECT_TRUE(GivesWay({.coloured = true}));
  EXPECT_TRUE(GivesWay({.coloured = true, .today_tabs = 3}));
}

TEST(SpaceDefaultTest, WhatTheUserMadeStays) {
  EXPECT_FALSE(GivesWay({.named = true}));
  EXPECT_FALSE(GivesWay({.coloured = true, .pinned_rows = 1}));
  EXPECT_FALSE(GivesWay({.today_tabs = 2, .favorites = 1}));
}

}  // namespace
}  // namespace browser_core
