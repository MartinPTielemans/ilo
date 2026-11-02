#include "browser_core/space.h"

#include <algorithm>
#include <cstdint>
#include <iterator>
#include <map>
#include <memory>
#include <numeric>
#include <set>
#include <utility>

#include "base/functional/bind.h"
#include "base/location.h"
#include "base/memory/ptr_util.h"
#include "base/no_destructor.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/auto_archive.h"
#include "browser_core/host.h"
#include "browser_core/session.h"

namespace browser_core {

namespace {

// By id.
std::map<bc_id, std::unique_ptr<Space>>& Spaces() {
  static base::NoDestructor<std::map<bc_id, std::unique_ptr<Space>>> spaces;
  return *spaces;
}

// The listed Spaces' ids in the switcher's order, as a new one goes after the one shown (Arc).
std::vector<bc_id>& SpaceOrder() {
  static base::NoDestructor<std::vector<bc_id>> order;
  return *order;
}

// Gives each of |nodes|, siblings in their order, a place after the one before it: the most that
// already are in order keep theirs, and each other goes between its neighbours, so a move changes
// only the place of what moved. Each without an id gets one.
void Order(const std::vector<Space::Node*>& nodes) {
  constexpr size_t kNone = SIZE_MAX;
  // The longest run of places already in order, found as patience sorting does: |ends[k]| is the
  // node that ends lowest a run of k + 1, and |before| each node's in its run.
  std::vector<size_t> ends;
  std::vector<size_t> before(nodes.size(), kNone);
  for (size_t i = 0; i < nodes.size(); ++i) {
    const syncer::UniquePosition& position = nodes[i]->position;
    if (!position.IsValid()) {
      continue;
    }
    auto end = std::ranges::partition_point(
        ends, [&](size_t last) { return nodes[last]->position.LessThan(position); });
    if (end != ends.begin()) {
      before[i] = *(end - 1);
    }
    if (end == ends.end()) {
      ends.push_back(i);
    } else {
      *end = i;
    }
  }
  std::vector<bool> kept(nodes.size());
  for (size_t i = ends.empty() ? kNone : ends.back(); i != kNone; i = before[i]) {
    kept[i] = true;
  }
  for (size_t i = 0; i < nodes.size(); ++i) {
    Space::Node& node = *nodes[i];
    if (!node.id.is_valid()) {
      node.id = base::Uuid::GenerateRandomV4();
    }
    if (kept[i]) {
      continue;
    }
    size_t next = i + 1;
    while (next < nodes.size() && !kept[next]) {
      ++next;
    }
    const syncer::UniquePosition::Suffix suffix = syncer::UniquePosition::RandomSuffix();
    const syncer::UniquePosition* above = i ? &nodes[i - 1]->position : nullptr;
    const syncer::UniquePosition* below = next < nodes.size() ? &nodes[next]->position : nullptr;
    node.position = above && below ? syncer::UniquePosition::Between(*above, *below, suffix)
                    : above        ? syncer::UniquePosition::After(*above, suffix)
                    : below        ? syncer::UniquePosition::Before(*below, suffix)
                                   : syncer::UniquePosition::InitialPosition(suffix);
  }
}

// Posted, like a Space's changes.
void ReportSpacesChanged() {
  SessionChanged();
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce([] {
        if (Host().spaces_changed) {
          Host().spaces_changed(Host().context);
        }
      }));
}

}  // namespace

// static
Space* Space::Create(std::string name, bc_id id, std::optional<Agent> agent, size_t at) {
  if (!id) {
    id = NewId();
  }
  Space* space = new Space(id, std::move(name), std::move(agent));
  Spaces()[id] = base::WrapUnique(space);
  std::vector<bc_id>& order = SpaceOrder();
  order.insert(order.begin() + std::min(at, order.size()), id);
  PlaceSpaces();
  ReportSpacesChanged();
  return space;
}

// static
Space* Space::CreateOwn(std::string profile, Own own) {
  const bc_id id = NewId();
  Space* space = new Space(id, std::string(), std::nullopt);
  space->profile_ = std::move(profile);
  space->own_ = own;
  Spaces()[id] = base::WrapUnique(space);
  return space;
}

// static
void Space::Delete(bc_id id) {
  auto it = Spaces().find(id);
  if (it == Spaces().end()) {
    return;
  }
  // The Spaces listed don't change with a window's own.
  const bool listed = !it->second->own();
  Spaces().erase(it);
  if (listed) {
    std::erase(SpaceOrder(), id);
    ReportSpacesChanged();
  }
}

// static
Space* Space::FromId(bc_id id) {
  auto it = Spaces().find(id);
  return it == Spaces().end() ? nullptr : it->second.get();
}

// static
std::vector<Space*> Space::All() {
  std::vector<Space*> all;
  for (bc_id id : SpaceOrder()) {
    all.push_back(FromId(id));
  }
  return all;
}

// static
Space* Space::Holding(bc_id tab) {
  for (auto& [id, space] : Spaces()) {
    if (space->Has(tab)) {
      return space.get();
    }
  }
  return nullptr;
}

// static
Space* Space::HoldingFolder(bc_id folder) {
  for (auto& [id, space] : Spaces()) {
    if (space->IsFolder(folder)) {
      return space.get();
    }
  }
  return nullptr;
}

// static
bool Space::AgentMayAct(const std::string& agent, bc_id tab) {
  const Space* space = Holding(tab);
  return space && space->agent() && space->agent()->id == agent;
}

Space::Space(bc_id id, std::string name, std::optional<Agent> agent)
    : id_(id), agent_(std::move(agent)), name_(std::move(name)) {}

bool Space::Has(bc_id tab) const {
  return IsPinned(tab) || std::ranges::contains(today_, tab);
}

std::vector<bc_id> Space::pinned() const {
  std::vector<bc_id> tabs;
  std::ranges::copy_if(pinned_, std::back_inserter(tabs),
                       [this](bc_id id) { return !IsFolder(id); });
  return tabs;
}

bool Space::IsPinned(bc_id tab) const {
  return !IsFolder(tab) && std::ranges::contains(pinned_, tab);
}

const Space::Node* Space::NodeOf(bc_id id) const {
  auto it = nodes_.find(id);
  return it == nodes_.end() ? nullptr : &it->second;
}

void Space::RestoreNodes(const Node& space, const std::map<bc_id, Node>& rows) {
  if (space.id.is_valid()) {
    const bool moved = !node_.position.Equals(space.position);
    node_ = space;
    // The order follows the places sync gives, as the other devices have them, so a place that
    // came from one is kept here rather than made again and sent back.
    if (moved) {
      std::ranges::stable_sort(SpaceOrder(), [](bc_id a, bc_id b) {
        const syncer::UniquePosition &at = FromId(a)->node_.position,
                                     &bt = FromId(b)->node_.position;
        return at.IsValid() && (!bt.IsValid() || at.LessThan(bt));
      });
    }
    PlaceSpaces();
    if (moved) {
      ReportSpacesChanged();
    }
  }
  for (const auto& [id, node] : rows) {
    if (node.id.is_valid() && nodes_.contains(id)) {
      nodes_[id] = node;
    }
  }
  ReportChanged();
}

bc_id Space::FolderOf(bc_id id) const {
  auto it = folder_of_.find(id);
  return it == folder_of_.end() ? 0 : it->second;
}

std::string Space::FolderName(bc_id folder) const {
  auto it = folders_.find(folder);
  return it == folders_.end() ? std::string() : it->second.name;
}

std::string Space::FolderIcon(bc_id folder) const {
  auto it = folders_.find(folder);
  return it == folders_.end() ? std::string() : it->second.icon;
}

bool Space::IsOpen(bc_id folder) const {
  auto it = folders_.find(folder);
  return it != folders_.end() && it->second.open;
}

GURL Space::PinnedUrl(bc_id tab) const {
  auto it = pinned_urls_.find(tab);
  return it == pinned_urls_.end() ? GURL() : it->second;
}

std::vector<bc_id> Space::Panes(bc_id tab) const {
  if (auto split = SplitOf(tab); split != splits_.end()) {
    return split->tabs;
  }
  return Has(tab) ? std::vector<bc_id>{tab} : std::vector<bc_id>();
}

void Space::SetName(std::string name) {
  touched_ |= name != name_;
  name_ = std::move(name);
  ReportSpacesChanged();
}

void Space::SetColor(std::optional<uint32_t> color) {
  color_ = color;
  ReportSpacesChanged();
}

void Space::SetIcon(std::string icon) {
  touched_ |= icon != icon_;
  icon_ = std::move(icon);
  ReportSpacesChanged();
}

void Space::SetTheme(std::string theme) {
  theme_ = std::move(theme);
  ReportSpacesChanged();
}

void Space::SetProfile(std::string profile) {
  profile_ = std::move(profile);
  ReportSpacesChanged();
}

void Space::Add(bc_id tab, bc_id opener, bool foreground) {
  const bool pinned_opener = opener && IsPinned(opener);
  // A pinned opener sits above the Today tabs.
  auto at = pinned_opener ? today_.begin() : std::ranges::find(today_, opener);
  if (!opener || (!pinned_opener && at == today_.end())) {
    today_.insert(today_.begin(), tab);
    ReportChanged();
    return;
  }
  // One opener at a time leads back, as Chrome has it, or a sibling opened earlier in the day would
  // take the opener's place when this one closes.
  if (foreground) {
    ForgetOpeners();
  }
  openers_[tab] = opener;
  if (!pinned_opener) {
    ++at;
  }
  if (!foreground) {
    // Past the opener's family below it: the tabs it opened, and the tabs they opened.
    std::set<bc_id> family = {opener};
    for (; at != today_.end() && family.contains(OpenerOf(*at)); ++at) {
      family.insert(*at);
    }
  }
  today_.insert(today_.begin() + OutsideSplits(today_, at - today_.begin()), tab);
  ReportChanged();
}

void Space::AddAt(bc_id tab, size_t index) {
  today_.insert(today_.begin() + OutsideSplits(today_, index), tab);
  ReportChanged();
}

void Space::AddPinned(bc_id tab, bc_id folder, size_t index, GURL url) {
  if (!IsFolder(folder)) {
    folder = 0;
  }
  pinned_.insert(pinned_.begin() + PinnedAt(folder, index), tab);
  if (folder) {
    folder_of_[tab] = folder;
  }
  pinned_urls_[tab] = std::move(url);
  ReportChanged();
}

void Space::SetPinnedUrl(bc_id tab, GURL url) {
  if (!IsPinned(tab) || !url.is_valid() || pinned_urls_[tab] == url) {
    return;
  }
  pinned_urls_[tab] = std::move(url);
  ReportChanged();
}

void Space::Pin(bc_id tab, base::FunctionRef<GURL(bc_id)> url_of) {
  if (!std::ranges::contains(today_, tab) || own()) {
    return;
  }
  for (bc_id pane : Panes(tab)) {
    std::erase(today_, pane);
    pinned_.push_back(pane);
    pinned_urls_[pane] = url_of(pane);
  }
  ReportChanged();
}

void Space::Unpin(bc_id tab) {
  if (!IsPinned(tab)) {
    return;
  }
  const std::vector<bc_id> panes = Panes(tab);
  for (bc_id pane : panes) {
    std::erase(pinned_, pane);
    pinned_urls_.erase(pane);
    folder_of_.erase(pane);
  }
  // Where a tab the user opens goes.
  today_.insert(today_.begin(), panes.begin(), panes.end());
  ReportChanged();
}

void Space::Move(bc_id tab,
                 bool pinned,
                 bc_id folder,
                 size_t index,
                 base::FunctionRef<GURL(bc_id)> url_of) {
  const bool is_folder = IsFolder(tab);
  if ((!Has(tab) && !is_folder) || (is_folder && !pinned) || (folder && !IsFolder(folder)) ||
      (pinned && own()) || (is_folder && Within(folder, tab))) {
    return;
  }
  const bool was_pinned = IsPinned(tab) || is_folder;
  // A folder's rows, else the tab's split.
  std::vector<bc_id> rows;
  if (is_folder) {
    auto first = std::ranges::find(pinned_, tab);
    rows.assign(first, pinned_.begin() + EndOf(tab));
    pinned_.erase(first, pinned_.begin() + EndOf(tab));
  } else {
    rows = Panes(tab);
    for (bc_id pane : rows) {
      std::erase(was_pinned ? pinned_ : today_, pane);
    }
  }
  if (pinned) {
    pinned_.insert(pinned_.begin() + PinnedAt(folder, index), rows.begin(), rows.end());
  } else {
    today_.insert(today_.begin() + OutsideSplits(today_, index), rows.begin(), rows.end());
  }
  for (bc_id row : is_folder ? std::vector<bc_id>{tab} : rows) {
    if (pinned && folder) {
      folder_of_[row] = folder;
    } else {
      folder_of_.erase(row);
    }
  }
  for (bc_id pane : is_folder ? std::vector<bc_id>() : rows) {
    if (pinned && !was_pinned) {
      pinned_urls_[pane] = url_of(pane);
    } else if (!pinned) {
      pinned_urls_.erase(pane);
    }
    ForgetOpener(pane);
  }
  ReportChanged();
}

void Space::Remove(bc_id tab) {
  if (IsFolder(tab) || (std::erase(today_, tab) == 0 && std::erase(pinned_, tab) == 0)) {
    return;
  }
  pinned_urls_.erase(tab);
  folder_of_.erase(tab);
  ForgetOpener(tab);
  LeaveSplit(tab);
  ReportChanged();
}

bc_id Space::AddFolder(std::string name, bc_id folder, size_t index) {
  if (own()) {
    return 0;
  }
  if (!IsFolder(folder)) {
    folder = 0;
  }
  const bc_id id = NewId();
  pinned_.insert(pinned_.begin() + PinnedAt(folder, index), id);
  folders_[id] = {.name = std::move(name)};
  if (folder) {
    folder_of_[id] = folder;
  }
  ReportChanged();
  return id;
}

void Space::RenameFolder(bc_id folder, std::string name) {
  if (auto it = folders_.find(folder); it != folders_.end()) {
    it->second.name = std::move(name);
    ReportChanged();
  }
}

void Space::SetFolderIcon(bc_id folder, std::string icon) {
  if (auto it = folders_.find(folder); it != folders_.end() && it->second.icon != icon) {
    it->second.icon = std::move(icon);
    ReportChanged();
  }
}

void Space::SetOpen(bc_id folder, bool open) {
  if (auto it = folders_.find(folder); it != folders_.end() && it->second.open != open) {
    it->second.open = open;
    ReportChanged();
  }
}

void Space::SetPinnedOpen(bool open) {
  if (pinned_open_ != open) {
    pinned_open_ = open;
    ReportChanged();
  }
}

void Space::DeleteFolder(bc_id folder) {
  if (!IsFolder(folder)) {
    return;
  }
  const bc_id holder = FolderOf(folder);
  for (auto& [id, its_folder] : folder_of_) {
    if (its_folder == folder) {
      its_folder = holder;
    }
  }
  std::erase_if(folder_of_, [](const auto& entry) { return entry.second == 0; });
  folder_of_.erase(folder);
  folders_.erase(folder);
  std::erase(pinned_, folder);
  ReportChanged();
}

bool Space::Split(bc_id tab, bc_id with, GURL url, bool left) {
  if (tab == with || !Has(tab) || !Has(with)) {
    return false;
  }
  const std::vector<bc_id> panes = Panes(with);
  if (panes.size() >= kMaxPanes || std::ranges::contains(panes, tab)) {
    return false;
  }
  const bool was_pinned = IsPinned(tab);
  const GURL pinned_url = PinnedUrl(tab);
  LeaveSplit(tab);
  std::erase(was_pinned ? pinned_ : today_, tab);
  const bool pinned = IsPinned(with);
  std::vector<bc_id>& list = pinned ? pinned_ : today_;
  list.insert(left ? std::ranges::find(list, panes.front())
                   : std::ranges::find(list, panes.back()) + 1,
              tab);
  if (pinned) {
    pinned_urls_[tab] = was_pinned ? pinned_url : std::move(url);
  } else {
    pinned_urls_.erase(tab);
  }
  // In the folder its split is in.
  if (bc_id folder = FolderOf(with)) {
    folder_of_[tab] = folder;
  } else {
    folder_of_.erase(tab);
  }
  ForgetOpener(tab);
  if (auto split = SplitOf(with); split != splits_.end()) {
    split->tabs.insert(left ? split->tabs.begin() : split->tabs.end(), tab);
    split->shares.clear();
    split->expanded = 0;
  } else {
    splits_.push_back(
        {.tabs = left ? std::vector<bc_id>{tab, with} : std::vector<bc_id>{with, tab}});
  }
  ReportChanged();
  return true;
}

bool Space::MoveInSplit(bc_id tab, size_t index) {
  auto split = SplitOf(tab);
  if (split == splits_.end() || index >= split->tabs.size()) {
    return false;
  }
  // A split's tabs sit together in their list, in its order.
  std::vector<bc_id>& list = IsPinned(tab) ? pinned_ : today_;
  const auto first = std::ranges::find(list, split->tabs.front());
  std::erase(split->tabs, tab);
  split->tabs.insert(split->tabs.begin() + index, tab);
  std::ranges::copy(split->tabs, first);
  ReportChanged();
  return true;
}

bool Space::Stacked(bc_id tab) const {
  auto split = SplitOf(tab);
  return split != splits_.end() && split->stacked;
}

bool Space::SetStacked(bc_id tab, bool stacked) {
  auto split = SplitOf(tab);
  if (split == splits_.end()) {
    return false;
  }
  split->stacked = stacked;
  ReportChanged();
  return true;
}

std::vector<double> Space::Shares(bc_id tab) const {
  auto split = SplitOf(tab);
  return split != splits_.end() ? split->shares : std::vector<double>();
}

bool Space::SetShares(bc_id tab, std::vector<double> shares) {
  auto split = SplitOf(tab);
  if (split == splits_.end() || (!shares.empty() && shares.size() != split->tabs.size()) ||
      std::ranges::any_of(shares, [](double share) { return !(share > 0); })) {
    return false;
  }
  const double sum = std::accumulate(shares.begin(), shares.end(), 0.0);
  for (double& share : shares) {
    share /= sum;
  }
  split->shares = std::move(shares);
  split->expanded = 0;
  ReportChanged();
  return true;
}

bc_id Space::Expanded(bc_id tab) const {
  auto split = SplitOf(tab);
  return split != splits_.end() ? split->expanded : 0;
}

bool Space::SetExpanded(bc_id tab, bool expanded) {
  auto split = SplitOf(tab);
  if (split == splits_.end()) {
    return false;
  }
  split->shares.clear();
  split->expanded = expanded ? tab : 0;
  ReportChanged();
  return true;
}

void Space::Separate(bc_id tab) {
  auto split = SplitOf(tab);
  if (split == splits_.end()) {
    return;
  }
  std::erase(split->tabs, tab);
  split->shares.clear();
  split->expanded = 0;
  const bc_id last = split->tabs.back();
  if (split->tabs.size() < 2) {
    splits_.erase(split);
  }
  std::vector<bc_id>& list = IsPinned(tab) ? pinned_ : today_;
  std::erase(list, tab);
  list.insert(std::ranges::find(list, last) + 1, tab);
  ReportChanged();
}

bc_id Space::NextAfterLeaving(bc_id leaving, base::FunctionRef<bool(bc_id)> loaded) const {
  if (auto split = SplitOf(leaving); split != splits_.end()) {
    auto pane = std::ranges::find(split->tabs, leaving);
    auto right = std::find_if(pane + 1, split->tabs.end(), loaded);
    if (right != split->tabs.end()) {
      return *right;
    }
    auto left = std::find_if(std::make_reverse_iterator(pane), split->tabs.rend(), loaded);
    if (left != split->tabs.rend()) {
      return *left;
    }
  }
  const std::vector<bc_id> tabs = pinned();
  if (auto pinned = std::ranges::find(tabs, leaving); pinned != tabs.end()) {
    // As Dia: the top Today tab, else a pinned tab already loaded, so closing one loads no other.
    if (!today_.empty()) {
      return today_.front();
    }
    auto below = std::find_if(pinned + 1, tabs.end(), loaded);
    if (below != tabs.end()) {
      return *below;
    }
    auto above = std::find_if(std::make_reverse_iterator(pinned), tabs.rend(), loaded);
    return above != tabs.rend() ? *above : 0;
  }
  auto it = std::ranges::find(today_, leaving);
  // A favorite, which is in no Space.
  if (it == today_.end()) {
    return today_.empty() ? LastLoadedPinned(loaded) : today_.front();
  }
  const size_t index = it - today_.begin();
  if (bc_id child = NearestOpenedBy(leaving, index)) {
    return child;
  }
  if (bc_id opener = OpenerOf(leaving)) {
    if (bc_id sibling = NearestOpenedBy(opener, index)) {
      return sibling;
    }
    if (Has(opener)) {
      return opener;
    }
  }
  if (index + 1 < today_.size()) {
    return today_[index + 1];
  }
  if (index > 0) {
    return today_[index - 1];
  }
  return LastLoadedPinned(loaded);
}

bc_id Space::LastLoadedPinned(base::FunctionRef<bool(bc_id)> loaded) const {
  const std::vector<bc_id> tabs = pinned();
  auto pinned = std::find_if(tabs.rbegin(), tabs.rend(), loaded);
  return pinned != tabs.rend() ? *pinned : 0;
}

void Space::UserShowed(bc_id was, bc_id shown) {
  const bc_id was_opener = OpenerOf(was);
  const bc_id opener = OpenerOf(shown);
  if (opener != was_opener && opener != was && was_opener != shown) {
    ForgetOpeners();
  }
}

void Space::ForgetOpeners() {
  openers_.clear();
}

bc_id Space::OpenerOf(bc_id tab) const {
  auto it = openers_.find(tab);
  return it == openers_.end() ? 0 : it->second;
}

bc_id Space::NearestOpenedBy(bc_id opener, size_t index) const {
  for (size_t i = index + 1; i < today_.size(); ++i) {
    if (OpenerOf(today_[i]) == opener) {
      return today_[i];
    }
  }
  for (size_t i = index; i-- > 0;) {
    if (OpenerOf(today_[i]) == opener) {
      return today_[i];
    }
  }
  return 0;
}

std::vector<Space::SplitView>::iterator Space::SplitOf(bc_id tab) {
  return std::ranges::find_if(
      splits_, [tab](const SplitView& split) { return std::ranges::contains(split.tabs, tab); });
}

std::vector<Space::SplitView>::const_iterator Space::SplitOf(bc_id tab) const {
  return std::ranges::find_if(
      splits_, [tab](const SplitView& split) { return std::ranges::contains(split.tabs, tab); });
}

size_t Space::OutsideSplits(const std::vector<bc_id>& tabs, size_t index) const {
  index = std::min(index, tabs.size());
  while (index > 0 && index < tabs.size()) {
    auto split = SplitOf(tabs[index]);
    if (split == splits_.end() || !std::ranges::contains(split->tabs, tabs[index - 1])) {
      break;
    }
    ++index;
  }
  return index;
}

bool Space::Within(bc_id id, bc_id folder) const {
  for (; id; id = FolderOf(id)) {
    if (id == folder) {
      return true;
    }
  }
  return false;
}

size_t Space::EndOf(bc_id folder) const {
  if (!folder) {
    return pinned_.size();
  }
  size_t end = std::ranges::find(pinned_, folder) - pinned_.begin();
  if (end < pinned_.size()) {
    for (++end; end < pinned_.size() && Within(pinned_[end], folder); ++end) {
    }
  }
  return end;
}

size_t Space::PinnedAt(bc_id folder, size_t index) const {
  size_t at = folder ? std::ranges::find(pinned_, folder) - pinned_.begin() + 1 : 0;
  const size_t end = EndOf(folder);
  for (; at < end; ++at) {
    if (FolderOf(pinned_[at]) == folder && index-- == 0) {
      break;
    }
  }
  return OutsideSplits(pinned_, std::min(at, end));
}

void Space::LeaveSplit(bc_id tab) {
  auto split = SplitOf(tab);
  if (split == splits_.end()) {
    return;
  }
  std::erase(split->tabs, tab);
  split->shares.clear();
  split->expanded = 0;
  if (split->tabs.size() < 2) {
    splits_.erase(split);
  }
}

void Space::ForgetOpener(bc_id tab) {
  const bc_id opener = OpenerOf(tab);
  openers_.erase(tab);
  for (auto& [child, its_opener] : openers_) {
    if (its_opener == tab) {
      its_opener = opener;
    }
  }
  std::erase_if(openers_, [](const auto& entry) { return entry.second == 0; });
}

void Space::PlaceRows() {
  std::set<bc_id> rows(pinned_.begin(), pinned_.end());
  std::erase_if(nodes_, [&](const auto& node) { return !rows.contains(node.first); });
  // By the folder they're in, or 0.
  std::map<bc_id, std::vector<Node*>> siblings;
  for (bc_id row : pinned_) {
    siblings[FolderOf(row)].push_back(&nodes_[row]);
  }
  for (const auto& [folder, nodes] : siblings) {
    Order(nodes);
  }
}

// static
void Space::PlaceSpaces() {
  std::vector<Node*> nodes;
  for (Space* space : All()) {
    nodes.push_back(&space->node_);
  }
  Order(nodes);
}

void Space::ReportChanged() {
  // Every change to the Space comes here, so none leaves a row without its node or out of order.
  PlaceRows();
  SessionChanged();
  ScheduleAutoArchive();
  // Posted: the app often changed the Space itself, and hears of it once its call has returned.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce(
                     [](bc_id id) {
                       if (FromId(id) && Host().space_changed) {
                         Host().space_changed(Host().context, id);
                       }
                     },
                     id_));
}

}  // namespace browser_core
