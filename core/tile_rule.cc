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
  auto [it, added] = views_.try_emplace(tab, view);
  if (added) {
    return Shown::kNew;
  }
  if (it->second == view) {
    return Shown::kFramed;
  }
  it->second = view;
  return Shown::kMoved;
}

const void* TileRule::End(uint64_t tab) {
  auto it = views_.find(tab);
  if (it == views_.end()) {
    return nullptr;
  }
  const void* view = it->second;
  views_.erase(it);
  return view;
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
    case Shown::kNoPlatform:
      return "refused, no tiles here";
    case Shown::kNoView:
      return "refused, no view";
  }
  return "refused";
}

}  // namespace browser_core
