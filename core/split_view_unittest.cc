#include "browser_core/split_view.h"

#include <map>
#include <string>
#include <vector>

#include "testing/gtest/include/gtest/gtest.h"

namespace browser_core {
namespace {

using Tabs = std::vector<bc_id>;

TEST(SplitViewTest, FindsTheSplitHoldingATab) {
  std::vector<SplitView> splits = {{.tabs = {1, 2}}, {.tabs = {3, 4, 5}}};
  ASSERT_NE(SplitHolding(splits, 4), nullptr);
  EXPECT_EQ(SplitHolding(splits, 4)->tabs, (Tabs{3, 4, 5}));
  EXPECT_EQ(SplitHolding(splits, 6), nullptr);
}

TEST(SplitViewTest, SharesAddUpToOneOrAreEven) {
  EXPECT_EQ(ScaledShares({1, 3}, 2), (std::vector<double>{0.25, 0.75}));
  // None are even shares.
  EXPECT_EQ(ScaledShares({}, 3), std::vector<double>());
  EXPECT_FALSE(ScaledShares({1, 1}, 3));
  EXPECT_FALSE(ScaledShares({1, 0}, 2));
}

// A favorites' list with two splits, keyed 7 and 9, one split's tabs apart.
TEST(SplitViewTest, ListsTheSplitsOfAListByKey) {
  const std::map<bc_id, int64_t> key = {{2, 7}, {3, 9}, {5, 7}, {6, 9}, {8, 4}};
  EXPECT_EQ(SplitsIn({1, 2, 3, 4, 5, 6, 8}, key), (std::vector<Tabs>{{2, 5}, {3, 6}}));
}

TEST(SplitViewTest, GathersASplitsTabsAtItsFirst) {
  const std::map<bc_id, int64_t> key = {{2, 7}, {5, 7}};
  EXPECT_EQ(Gathered({1, 2, 3, 4, 5, 6}, key), (Tabs{1, 2, 5, 3, 4, 6}));
  EXPECT_EQ(Gathered({1, 2, 3}, {}), (Tabs{1, 2, 3}));
}

// The favorites of two Spaces merged as Each Space ends: the first Space's, then the second's
// extras.
TEST(SplitViewTest, MergesSplitsOnlyWithTheSameSplit) {
  const std::vector<FavoriteUnit> first = {
      {.tabs = {1}, .urls = {"a"}},
      {.tabs = {2, 3}, .urls = {"b", "c"}},
  };
  const std::vector<FavoriteUnit> second = {
      // The same split: a duplicate.
      {.tabs = {11, 12}, .urls = {"b", "c"}},
      // A favorite alone at a split's page, and the split's pages in another order: extras.
      {.tabs = {13}, .urls = {"b"}},
      {.tabs = {14, 15}, .urls = {"c", "b"}},
      // A duplicate of the favorite alone.
      {.tabs = {16}, .urls = {"a"}},
  };
  std::vector<bc_id> losers;
  const std::vector<FavoriteUnit> merged = MergeUnits({first, second}, losers);
  std::vector<Tabs> tabs;
  for (const FavoriteUnit& unit : merged) {
    tabs.push_back(unit.tabs);
  }
  EXPECT_EQ(tabs, (std::vector<Tabs>{{1}, {2, 3}, {13}, {14, 15}}));
  EXPECT_EQ(losers, (Tabs{11, 12, 16}));
}

TEST(SplitViewTest, KeepsTheDuplicateRankedHigherWhereTheFirstWas) {
  std::vector<bc_id> losers;
  const std::vector<FavoriteUnit> merged =
      MergeUnits({{{.tabs = {1}, .urls = {"a"}}, {.tabs = {2, 3}, .urls = {"b", "c"}, .rank = 0}},
                  {{.tabs = {12, 13}, .urls = {"b", "c"}, .rank = 2}}},
                 losers);
  ASSERT_EQ(merged.size(), 2u);
  EXPECT_EQ(merged[1].tabs, (Tabs{12, 13}));
  EXPECT_EQ(losers, (Tabs{2, 3}));
}

// Two splits of the same pages in one list are two favorites, each with its duplicate in the next.
TEST(SplitViewTest, CountsLikeSplitsInTheirList) {
  std::vector<bc_id> losers;
  const std::vector<FavoriteUnit> merged =
      MergeUnits({{{.tabs = {1, 2}, .urls = {"a", "b"}}, {.tabs = {3, 4}, .urls = {"a", "b"}}},
                  {{.tabs = {11, 12}, .urls = {"a", "b"}}}},
                 losers);
  EXPECT_EQ(merged.size(), 2u);
  EXPECT_EQ(losers, (Tabs{11, 12}));
}

}  // namespace
}  // namespace browser_core
