#include "browser_core/tile_rule.h"

namespace browser_core {

TileRule::Shown TileRule::Show(uint64_t tab,
                               const void* view,
                               bool agent_tab,
                               bool window_shows,
                               bool platform_can) {
  if (!platform_can) {
    return Shown::kNoPlatform;
  }
  if (!view) {
    return Shown::kNoView;
  }
  if (!agent_tab) {
    return Shown::kUsersTab;
  }
  if (window_shows) {
    return Shown::kWindowShows;
  }
  if (cards_.contains(tab)) {
    return Shown::kCardShows;
  }
  auto [it, added] = views_.try_emplace(tab, view);
  if (added) {
    parked_.erase(tab);
    return Shown::kNew;
  }
  if (it->second == view) {
    return Shown::kFramed;
  }
  it->second = view;
  return Shown::kMoved;
}

const void* TileRule::Take(uint64_t tab) {
  auto it = views_.find(tab);
  if (it == views_.end()) {
    return nullptr;
  }
  const void* view = it->second;
  views_.erase(it);
  return view;
}

const void* TileRule::End(uint64_t tab) {
  const void* view = Take(tab);
  if (view) {
    parked_.insert(tab);
  }
  return view;
}

const void* TileRule::ShowInCard(uint64_t tab) {
  cards_.insert(tab);
  parked_.erase(tab);
  return Take(tab);
}

const void* TileRule::Left(uint64_t tab) {
  cards_.erase(tab);
  parked_.erase(tab);
  return Take(tab);
}

bool TileRule::Parked(uint64_t tab) const {
  return parked_.contains(tab);
}

bool TileRule::Holds(uint64_t tab) const {
  return views_.contains(tab);
}

// static
const char* TileRule::Name(Shown shown) {
  switch (shown) {
    case Shown::kNew:
      return "shown";
    case Shown::kFramed:
      return "framed";
    case Shown::kMoved:
      return "moved";
    case Shown::kUsersTab:
      return "refused, a user's tab";
    case Shown::kWindowShows:
      return "refused, a window shows it";
    case Shown::kCardShows:
      return "refused, a card shows it";
    case Shown::kNoPlatform:
      return "refused, no tiles here";
    case Shown::kNoView:
      return "refused, no view";
  }
  return "refused";
}

}  // namespace browser_core
