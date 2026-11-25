#include "browser_core/handed_back.h"

#include <algorithm>
#include <map>

#include "browser_core/handed_place.h"

namespace browser_core::handed_back {

namespace {

void RejoinSplit(Space& home, bc_id tab, const std::vector<bc_id>& split) {
  // A split's tabs are all pinned or all Today tabs.
  const bool pinned = home.IsPinned(tab);
  const auto self = std::ranges::find(split, tab);
  const auto with = std::ranges::find_if(split, [&](bc_id pane) {
    return pane != tab &&
           (pinned ? home.IsPinned(pane) : std::ranges::contains(home.today(), pane));
  });
  if (self == split.end() || with == split.end() || !home.Split(tab, *with, GURL())) {
    return;
  }
  std::vector<bool> was_before;
  for (bc_id pane : home.Panes(tab)) {
    if (pane != tab) {
      was_before.push_back(std::ranges::find(split, pane) < self);
    }
  }
  home.MoveInSplit(tab, handed_place::InSplit(was_before));
}

}  // namespace

std::optional<Pin> PinOf(const Space& space, bc_id tab) {
  if (!space.IsPinned(tab)) {
    return std::nullopt;
  }
  Pin pin{.url = space.PinnedUrl(tab)};
  for (bc_id folder = space.FolderOf(tab); folder; folder = space.FolderOf(folder)) {
    const Space::Node* node = space.NodeOf(folder);
    pin.folders.push_back({.id = folder, .node = node ? *node : Space::Node()});
  }
  return pin;
}

size_t Among(const Space& home,
             const std::vector<bc_id>& rows,
             const Space::Node& node,
             base::FunctionRef<const syncer::UniquePosition*(bc_id)> place_of) {
  std::vector<handed_place::Row> beside;
  beside.reserve(rows.size());
  for (size_t i = 0; i < rows.size(); ++i) {
    const syncer::UniquePosition* place = place_of(rows[i]);
    beside.push_back({
        .before =
            place && place->IsValid() && node.position.IsValid() && place->LessThan(node.position),
        .joined_above = i > 0 && std::ranges::contains(home.Panes(rows[i]), rows[i - 1]),
    });
  }
  return handed_place::Among(beside);
}

void Put(Space& home,
         bc_id tab,
         const Space::Node& node,
         const std::vector<bc_id>& split,
         const std::optional<Pin>& pin) {
  const auto place_of = [&](bc_id row) -> const syncer::UniquePosition* {
    const Space::Node* its = home.NodeOf(row);
    return its ? &its->position : nullptr;
  };
  // Each row's node as it is. The tab moves with the place it got at the top of Today, which the
  // Space may make room for by giving the row above it a new one, and none of them moved.
  std::map<bc_id, Space::Node> nodes;
  for (const std::vector<bc_id>* rows : {&home.pinned_rows(), &home.today()}) {
    for (bc_id row : *rows) {
      if (const Space::Node* its = home.NodeOf(row)) {
        nodes[row] = *its;
      }
    }
  }
  if (pin) {
    std::vector<bool> still_there;
    for (const Pin::Folder& folder : pin->folders) {
      still_there.push_back(home.IsFolder(folder.id));
    }
    const size_t in = handed_place::InFolder(still_there);
    const bc_id folder = in < pin->folders.size() ? pin->folders[in].id : 0;
    // By its own place among what its folder holds, else by the place of the folder it was in
    // that's gone, among what held that.
    const Space::Node& by = in == 0 ? node : pin->folders[in - 1].node;
    std::vector<bc_id> beside;
    for (bc_id row : home.pinned_rows()) {
      if (row != tab && home.FolderOf(row) == folder) {
        beside.push_back(row);
      }
    }
    home.Move(tab, /*pinned=*/true, folder, Among(home, beside, by, place_of),
              [&](bc_id) { return pin->url; });
  } else {
    std::vector<bc_id> others = home.today();
    std::erase(others, tab);
    home.Move(tab, /*pinned=*/false, 0, Among(home, others, node, place_of),
              [](bc_id) { return GURL(); });
  }
  RejoinSplit(home, tab, split);
  // Its own as it left with it, which fits where it's back unless the rows around it changed.
  if (node.id.is_valid()) {
    nodes[tab] = node;
  } else {
    nodes.erase(tab);
  }
  home.RestoreNodes(home.node(), nodes);
}

}  // namespace browser_core::handed_back
