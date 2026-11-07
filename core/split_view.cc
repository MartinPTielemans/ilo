#include "browser_core/split_view.h"

#include <algorithm>
#include <numeric>
#include <utility>

namespace browser_core {

SplitView* SplitHolding(std::vector<SplitView>& splits, bc_id tab) {
  auto it = std::ranges::find_if(
      splits, [tab](const SplitView& split) { return std::ranges::contains(split.tabs, tab); });
  return it == splits.end() ? nullptr : &*it;
}

const SplitView* SplitHolding(const std::vector<SplitView>& splits, bc_id tab) {
  auto it = std::ranges::find_if(
      splits, [tab](const SplitView& split) { return std::ranges::contains(split.tabs, tab); });
  return it == splits.end() ? nullptr : &*it;
}

std::optional<std::vector<double>> ScaledShares(std::vector<double> shares, size_t count) {
  if ((!shares.empty() && shares.size() != count) ||
      std::ranges::any_of(shares, [](double share) { return !(share > 0); })) {
    return std::nullopt;
  }
  const double sum = std::accumulate(shares.begin(), shares.end(), 0.0);
  for (double& share : shares) {
    share /= sum;
  }
  return shares;
}

namespace {

int64_t KeyOf(const std::map<bc_id, int64_t>& key, bc_id tab) {
  auto it = key.find(tab);
  return it == key.end() ? 0 : it->second;
}

}  // namespace

std::vector<std::vector<bc_id>> SplitsIn(const std::vector<bc_id>& list,
                                         const std::map<bc_id, int64_t>& key) {
  std::vector<int64_t> order;
  std::map<int64_t, std::vector<bc_id>> tabs;
  for (bc_id tab : list) {
    const int64_t of = KeyOf(key, tab);
    if (!of) {
      continue;
    }
    if (!tabs.contains(of)) {
      order.push_back(of);
    }
    tabs[of].push_back(tab);
  }
  std::vector<std::vector<bc_id>> splits;
  for (int64_t of : order) {
    if (tabs[of].size() > 1) {
      splits.push_back(std::move(tabs[of]));
    }
  }
  return splits;
}

std::vector<bc_id> Gathered(const std::vector<bc_id>& list, const std::map<bc_id, int64_t>& key) {
  std::map<int64_t, std::vector<bc_id>> tabs;
  for (bc_id tab : list) {
    if (const int64_t of = KeyOf(key, tab)) {
      tabs[of].push_back(tab);
    }
  }
  std::vector<bc_id> gathered;
  for (bc_id tab : list) {
    const int64_t of = KeyOf(key, tab);
    if (!of) {
      gathered.push_back(tab);
    } else if (tabs[of].front() == tab) {
      gathered.insert(gathered.end(), tabs[of].begin(), tabs[of].end());
    }
  }
  return gathered;
}

std::vector<FavoriteUnit> MergeUnits(const std::vector<std::vector<FavoriteUnit>>& lists,
                                     std::vector<bc_id>& losers) {
  std::vector<FavoriteUnit> merged;
  // Where each is, by its URLs and by how many like it come before it in its list.
  std::map<std::pair<std::vector<std::string>, int>, size_t> at;
  for (const std::vector<FavoriteUnit>& list : lists) {
    std::map<std::vector<std::string>, int> counts;
    for (const FavoriteUnit& unit : list) {
      const std::pair<std::vector<std::string>, int> key(unit.urls, counts[unit.urls]++);
      auto [it, fresh] = at.try_emplace(key, merged.size());
      if (fresh) {
        merged.push_back(unit);
      } else if (FavoriteUnit& kept = merged[it->second]; unit.rank > kept.rank) {
        losers.insert(losers.end(), kept.tabs.begin(), kept.tabs.end());
        kept = unit;
      } else {
        losers.insert(losers.end(), unit.tabs.begin(), unit.tabs.end());
      }
    }
  }
  return merged;
}

}  // namespace browser_core
