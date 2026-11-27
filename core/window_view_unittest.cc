#include "browser_core/window_view.h"

#include <vector>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

WindowView Read(const base::DictValue& dict,
                const std::vector<int>& favorites,
                const std::vector<std::vector<int>>& kept) {
  return ReadWindowView(dict, favorites, kept, static_cast<int>(kept.size()) - 1);
}

// A window's hidden sidebar, Stay On Top, its ⌃Tab order and each Space's scroll read back as they
// were written, with every tab read.
TEST(WindowViewTest, WhatIsWrittenReadsBack) {
  const WindowView view{.sidebar_hidden = true,
                        .on_top = true,
                        .recent = {{1, 2}, {-1, 0}, {0, 1}},
                        .scroll = {{0, 120.5}, {1, 48}}};
  base::DictValue dict;
  WriteWindowView(view, dict);
  EXPECT_EQ(Read(dict, {0}, {{0, 1}, {0, 1, 2}}), view);
}

// A window saved before views were kept, and one with nothing to keep, read as a new window: shown,
// with no order and every list at its top. Nothing is written for the second.
TEST(WindowViewTest, AWindowWithoutOneReadsAsNew) {
  EXPECT_EQ(Read(base::DictValue().Set("space", 0), {}, {{0}}), WindowView());
  base::DictValue dict;
  WriteWindowView(WindowView(), dict);
  EXPECT_TRUE(dict.empty());
}

// A tab that couldn't be read, its Space's tabs after it moved up, leaves the order, and the rest
// keep theirs by where they are now: a place names a tab as saved, not as read.
TEST(WindowViewTest, ATabNotReadLeavesTheOrder) {
  WindowView view{.recent = {{0, 2}, {0, 0}, {-1, 1}, {0, 1}, {3, 0}, {0, 9}, {-2, 0}}};
  base::DictValue dict;
  WriteWindowView(view, dict);
  // Space 0's second tab and the first favorite weren't read; there's no Space 3.
  const WindowView read = Read(dict, {-1, 0}, {{0, -1, 1}});
  EXPECT_EQ(read.recent, (std::vector<TabPlace>{{0, 1}, {0, 0}, {-1, 0}}));
}

// A scroll of a Space past those read, or at the top, isn't kept.
TEST(WindowViewTest, AScrollOfNoSpaceIsLeftOut) {
  base::DictValue dict;
  WriteWindowView({.scroll = {{0, 30}, {1, 0}, {2, 64}}}, dict);
  EXPECT_EQ(Read(dict, {}, {{}, {}}).scroll, (std::vector<std::pair<int, double>>{{0, 30}}));
}

}  // namespace
}  // namespace browser_core
