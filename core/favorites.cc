#include "browser_core/favorites.h"

#include <algorithm>
#include <functional>
#include <iterator>
#include <limits>
#include <map>
#include <string>
#include <tuple>
#include <utility>

#include "base/functional/bind.h"
#include "base/location.h"
#include "base/no_destructor.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/host.h"
#include "browser_core/profiles.h"
#include "browser_core/session.h"
#include "browser_core/space.h"
#include "browser_core/split_view.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "components/sessions/core/serialized_navigation_entry.h"
#include "content/public/browser/web_contents.h"

namespace browser_core {

namespace {

// Past the last of a list.
constexpr size_t kEnd = std::numeric_limits<size_t>::max();

// One favorite's tab.
struct Fav {
  GURL url;
  // Its profile's directory.
  std::string profile;
  // The Space it belongs to under Each Space, else 0.
  bc_id space = 0;
  // Names its site, which the tabs of one site in each profile share.
  int site = 0;
  // Names the split it's in, which its copies in the other lists share, else 0.
  int group = 0;
  // Its tab is closing, so it's in no list.
  bool doomed = false;
};

struct State {
  // Every favorite's tab, each list in its order among them.
  std::vector<bc_id> tabs;
  std::map<bc_id, Fav> favs;
  int next_site = 1;
  // The favorites that are splits, each in one list, its tabs together in it, in its order.
  std::vector<SplitView> splits;
  int next_group = 1;
};

State& Get() {
  static base::NoDestructor<State> state;
  return *state;
}

int CurrentScope() {
  return UserSettings().favorites_scope;
}

void SetScopeValue(int scope) {
  UserSettings().favorites_scope = scope;
}

void ReportChanged() {
  SessionChanged();
  // Posted, like a Space's changes, so the app hears of its own call once it has returned.
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE, base::BindOnce([] {
        if (Host().favorites_changed) {
          Host().favorites_changed(Host().context);
        }
      }));
}

// Whether |a| and |b| are in the same list: the same Space's under Each Space, else the same
// profile's.
bool SameList(const Fav& a, const Fav& b) {
  return CurrentScope() == BC_FAVORITES_SPACE ? a.space == b.space : a.profile == b.profile;
}

// The tabs of the list |like| is in, left to right, but |except|.
std::vector<bc_id> ListOf(const State& state, const Fav& like, bc_id except = 0) {
  std::vector<bc_id> list;
  for (bc_id tab : state.tabs) {
    const Fav& fav = state.favs.at(tab);
    if (tab != except && !fav.doomed && SameList(fav, like)) {
      list.push_back(tab);
    }
  }
  return list;
}

// The favorites of |list|, a list's tabs in its order: each split's tabs together, and each tab in
// none alone.
std::vector<std::vector<bc_id>> UnitsOf(const State& state, const std::vector<bc_id>& list) {
  std::vector<std::vector<bc_id>> units;
  std::map<const SplitView*, size_t> at;
  for (bc_id tab : list) {
    const SplitView* split = SplitHolding(state.splits, tab);
    if (!split) {
      units.push_back({tab});
      continue;
    }
    auto [it, fresh] = at.try_emplace(split, units.size());
    if (fresh) {
      units.emplace_back();
    }
    units[it->second].push_back(tab);
  }
  return units;
}

// Puts |tab|, which has its entry in |state.favs| and isn't in |state.tabs|, at |index| of its list,
// counting a split as one, else after the last of it.
void Insert(State& state, bc_id tab, size_t index) {
  const std::vector<bc_id> mine = ListOf(state, state.favs.at(tab), tab);
  const std::vector<std::vector<bc_id>> units = UnitsOf(state, mine);
  auto at = index < units.size() ? std::ranges::find(state.tabs, units[index].front())
            : mine.empty()        ? state.tabs.end()
                                  : std::ranges::find(state.tabs, mine.back()) + 1;
  state.tabs.insert(at, tab);
}

// Puts |tab|, as Insert takes it, right after |after|.
void InsertAfter(State& state, bc_id tab, bc_id after) {
  state.tabs.insert(std::ranges::find(state.tabs, after) + 1, tab);
}

// What names |fav|'s list: its Space under Each Space, else its profile.
std::string ListKey(const Fav& fav) {
  return CurrentScope() == BC_FAVORITES_SPACE ? "space " + base::NumberToString(fav.space)
                                              : "profile " + fav.profile;
}

// |tab|, and under Every Space its site's tabs in the other profiles, which are one favorite.
std::vector<bc_id> Copies(const State& state, bc_id tab) {
  std::vector<bc_id> copies = {tab};
  const Fav& of = state.favs.at(tab);
  if (CurrentScope() == BC_FAVORITES_EVERY) {
    for (const auto& [other, fav] : state.favs) {
      if (other != tab && !fav.doomed && fav.site == of.site) {
        copies.push_back(other);
      }
    }
  }
  return copies;
}

// The tab of |tabs| standing for |was|: |was| if it's one of them, else the one of its site, as a
// copy of it is, else 0.
bc_id CopyOf(const State& state, const std::vector<bc_id>& tabs, bc_id was) {
  if (!was || std::ranges::contains(tabs, was)) {
    return was;
  }
  auto fav = state.favs.find(was);
  if (fav == state.favs.end()) {
    return 0;
  }
  auto it = std::ranges::find_if(tabs, [&](bc_id tab) {
    auto each = state.favs.find(tab);
    return each != state.favs.end() && each->second.site == fav->second.site;
  });
  return it == tabs.end() ? 0 : *it;
}

// Makes the splits the favorites' groups say, after their lists were copied, merged or filled in.
// In each list a group's tabs are one split, together at the first's place. A split that was keeps
// its look, and a copy takes the look of the one it's a copy of, focused on the copy of the tab
// focused there. A tab whose group has no other tab in its list is in none.
void Regroup(State& state) {
  std::map<std::pair<std::string, int>, int64_t> keys;
  std::map<bc_id, int64_t> key;
  for (bc_id tab : state.tabs) {
    const Fav& fav = state.favs.at(tab);
    if (!fav.doomed && fav.group) {
      auto [it, fresh] =
          keys.try_emplace({ListKey(fav), fav.group}, static_cast<int64_t>(keys.size()) + 1);
      key[tab] = it->second;
    }
  }
  std::vector<SplitView> splits;
  for (std::vector<bc_id>& tabs : SplitsIn(state.tabs, key)) {
    // A split shows no more than a Space's does: the rest are favorites alone.
    for (size_t i = Space::kMaxPanes; i < tabs.size(); ++i) {
      key.erase(tabs[i]);
    }
    tabs.resize(std::min(tabs.size(), Space::kMaxPanes));
    const int group = state.favs.at(tabs.front()).group;
    const SplitView* same = nullptr;
    const SplitView* like = nullptr;
    for (const SplitView& old : state.splits) {
      if (old.tabs == tabs) {
        same = &old;
        break;
      }
      if (!like && std::ranges::any_of(old.tabs, [&](bc_id tab) {
            auto fav = state.favs.find(tab);
            return fav != state.favs.end() && fav->second.group == group;
          })) {
        like = &old;
      }
    }
    SplitView made{.tabs = tabs};
    if (const SplitView* from = same ? same : like) {
      made.stacked = from->stacked;
      made.name = from->name;
      made.icon = from->icon;
      if (from->shares.size() == tabs.size()) {
        made.shares = from->shares;
      }
      made.expanded = CopyOf(state, tabs, from->expanded);
      made.focused = CopyOf(state, tabs, from->focused);
    }
    splits.push_back(std::move(made));
  }
  for (auto& [tab, fav] : state.favs) {
    if (!SplitHolding(splits, tab)) {
      fav.group = 0;
      key.erase(tab);
    }
  }
  state.tabs = Gathered(state.tabs, key);
  state.splits = std::move(splits);
}

// The window active last without a Space of its own, which makes the tabs.
Window* AnyWindow() {
  for (Window* window : Window::All()) {
    if (!window->own_space()) {
      return window;
    }
  }
  return nullptr;
}

// The Space that window shows, or null.
const Space* ShownSpace() {
  Window* window = AnyWindow();
  return window ? Space::FromId(window->space()) : nullptr;
}

// The profiles of the user's Spaces, which show favorites, the one the window shows first and then
// in the order of the Spaces. An agent's Space shows the profile's favorites but has none made for
// it.
std::vector<std::string> ShowingProfiles() {
  std::vector<std::string> profiles;
  for (const Space* space : Space::All()) {
    if (!space->agent() && !std::ranges::contains(profiles, space->profile())) {
      profiles.push_back(space->profile());
    }
  }
  if (const Space* shown = ShownSpace(); shown && !shown->agent()) {
    std::ranges::stable_partition(profiles,
                                  [&](const std::string& each) { return each == shown->profile(); });
  }
  return profiles;
}

// The profiles that have a favorite, in the order of the first of them.
std::vector<std::string> ProfilesWithFavorites(const State& state) {
  std::vector<std::string> profiles;
  for (bc_id tab : state.tabs) {
    const Fav& fav = state.favs.at(tab);
    if (!fav.doomed && !std::ranges::contains(profiles, fav.profile)) {
      profiles.push_back(fav.profile);
    }
  }
  return profiles;
}

// The sites of |profiles|' lists as one: the first list's, then each later list's extras.
std::vector<int> MergedSites(const State& state, const std::vector<std::string>& profiles) {
  std::vector<int> sites;
  for (const std::string& profile : profiles) {
    for (bc_id tab : state.tabs) {
      const Fav& fav = state.favs.at(tab);
      if (!fav.doomed && fav.profile == profile && !std::ranges::contains(sites, fav.site)) {
        sites.push_back(fav.site);
      }
    }
  }
  return sites;
}

// |profile|'s tab for |site|, else 0.
bc_id InstanceOf(const State& state, const std::string& profile, int site) {
  for (bc_id tab : state.tabs) {
    const Fav& fav = state.favs.at(tab);
    if (!fav.doomed && fav.profile == profile && fav.site == site) {
      return tab;
    }
  }
  return 0;
}

// Any profile's tab for |site|, else 0.
bc_id AnyInstanceOf(const State& state, int site) {
  for (bc_id tab : state.tabs) {
    const Fav& fav = state.favs.at(tab);
    if (!fav.doomed && fav.site == site) {
      return tab;
    }
  }
  return 0;
}

// The directory of |tab|'s profile, with its page or without, or of the one it waits for, or empty.
std::string ProfileDirOf(bc_id tab) {
  const Tab* of = Tab::Get(tab);
  return of ? of->profile_dir() : std::string();
}

// What a new tab for a site is made from.
struct Template {
  GURL url;
  std::string title;
  std::string name;
  std::string icon;
};

Template TemplateOf(const State& state, bc_id tab) {
  Template made{.url = state.favs.at(tab).url};
  // With its page or without.
  if (const Tab* from = Tab::Get(tab)) {
    const SavedTab saved = from->Save();
    if (!saved.history.empty()) {
      made.title = base::UTF16ToUTF8(saved.history[saved.current].title());
    }
    made.name = from->name();
    made.icon = from->icon();
  }
  return made;
}

// Opens a tab from |made| in the profile in the directory |profile|, unloaded until it's shown, and
// makes it a favorite there, of |space| under Each Space and in the split |group| names, right
// after |after| if it isn't 0, else at |index| of its list. Returns it, or 0 if there's no window
// or the profile isn't loaded.
bc_id Make(State& state,
           const Template& made,
           const std::string& profile,
           bc_id space,
           int site,
           int group,
           size_t index,
           bc_id after = 0) {
  Window* window = AnyWindow();
  Profile* in = window ? ProfileFor(profile) : nullptr;
  if (!in || DirOf(in) != profile) {
    return 0;
  }
  SavedTab saved;
  sessions::SerializedNavigationEntry& page = saved.history.emplace_back();
  page.set_virtual_url(made.url);
  page.set_original_request_url(made.url);
  page.set_title(base::UTF8ToUTF16(made.title));
  saved.name = made.name;
  saved.icon = made.icon;
  const bc_id tab = Tab::Restore(in, saved);
  if (!tab) {
    return 0;
  }
  state.favs[tab] = {
      .url = made.url, .profile = profile, .space = space, .site = site, .group = group};
  if (after) {
    InsertAfter(state, tab, after);
  } else {
    Insert(state, tab, index);
  }
  return tab;
}

// Closes the tab of a favorite taken out of its list, which |Remove| forgets once it has.
void Retire(bc_id tab) {
  if (Tab::Get(tab)) {
    Window::CloseTab(tab);
  } else {
    Favorites::Remove(tab);
  }
}

// Under Every Space: gives each profile that shows favorites a tab, unloaded, for each site it
// lacks, in the order of the merged lists.
void FillIn(State& state) {
  const std::vector<std::string> profiles = ShowingProfiles();
  const std::vector<int> sites = MergedSites(state, profiles);
  for (const std::string& profile : profiles) {
    // Each after the site before it, the first at the front.
    bc_id after = 0;
    for (int site : sites) {
      if (const bc_id have = InstanceOf(state, profile, site)) {
        after = have;
      } else if (const bc_id from = AnyInstanceOf(state, site)) {
        if (const bc_id made = Make(state, TemplateOf(state, from), profile, 0, site,
                                    state.favs.at(from).group, 0, after)) {
          after = made;
        }
      }
    }
  }
}

// Under Each Space (already the scope): each profile's list is copied to each of its Spaces. A tab
// already made stays with one Space, the one the window shows if it's in the profile, so its page
// stays where it is, and the others get tabs of their own. A profile with no Space of the user's
// keeps its list, which the Spaces will show once it has one.
void CopyToSpaces(State& state) {
  const std::vector<Space*> spaces = Space::All();
  const Space* shown = ShownSpace();
  for (const std::string& profile : ProfilesWithFavorites(state)) {
    std::vector<const Space*> holders;
    for (const Space* space : spaces) {
      if (!space->agent() && space->profile() == profile) {
        holders.push_back(space);
      }
    }
    if (holders.empty()) {
      continue;
    }
    const Space* first =
        shown && !shown->agent() && shown->profile() == profile ? shown : holders.front();
    std::vector<bc_id> list;
    for (bc_id tab : state.tabs) {
      const Fav& fav = state.favs.at(tab);
      if (!fav.doomed && fav.profile == profile) {
        list.push_back(tab);
      }
    }
    for (bc_id tab : list) {
      state.favs.at(tab).space = first->id();
    }
    for (const Space* holder : holders) {
      if (holder == first) {
        continue;
      }
      for (bc_id tab : list) {
        const Fav& fav = state.favs.at(tab);
        Make(state, TemplateOf(state, tab), profile, holder->id(), fav.site, fav.group, kEnd);
      }
    }
  }
}

// How much a duplicate is worth keeping: a page a window shows, then one that's loaded.
int Rank(bc_id tab) {
  return Window::Showing(tab) ? 2 : Tab::Loaded(tab) ? 1 : 0;
}

// |list|'s favorites as the merges match them.
std::vector<FavoriteUnit> MatchedUnits(const State& state, const std::vector<bc_id>& list) {
  std::vector<FavoriteUnit> units;
  for (const std::vector<bc_id>& tabs : UnitsOf(state, list)) {
    FavoriteUnit& unit = units.emplace_back(FavoriteUnit{.tabs = tabs});
    for (bc_id tab : tabs) {
      unit.urls.push_back(state.favs.at(tab).url.spec());
      unit.rank = std::max(unit.rank, Rank(tab));
    }
  }
  return units;
}

// Out of Each Space: each profile's Spaces' lists are merged into the profile's. The list of the
// Space the window shows comes first, then each other Space's extras in the Spaces' order, then
// those no Space has. A favorite at the same URL as an earlier one is its duplicate, and a split
// at the same URLs in their order, and the extra tabs close.
void MergeSpaces(State& state) {
  const std::vector<Space*> spaces = Space::All();
  const Space* shown = ShownSpace();
  std::vector<bc_id> rebuilt;
  std::vector<bc_id> losers;
  for (const std::string& profile : ProfilesWithFavorites(state)) {
    std::vector<bc_id> owners;
    if (shown && shown->profile() == profile) {
      owners.push_back(shown->id());
    }
    for (const Space* space : spaces) {
      if (space->profile() == profile && !std::ranges::contains(owners, space->id())) {
        owners.push_back(space->id());
      }
    }
    // Those no Space has come last, as owner 0.
    owners.push_back(0);
    std::vector<std::vector<FavoriteUnit>> lists;
    for (bc_id owner : owners) {
      std::vector<bc_id> list;
      for (bc_id tab : state.tabs) {
        const Fav& fav = state.favs.at(tab);
        const bc_id of = std::ranges::contains(owners, fav.space) ? fav.space : 0;
        if (!fav.doomed && fav.profile == profile && of == owner) {
          list.push_back(tab);
        }
      }
      lists.push_back(MatchedUnits(state, list));
    }
    for (const FavoriteUnit& unit : MergeUnits(lists, losers)) {
      // Each split kept is a split of its own in the profile's list.
      const int group = unit.tabs.size() > 1 ? state.next_group++ : 0;
      for (bc_id tab : unit.tabs) {
        state.favs.at(tab).group = group;
      }
      rebuilt.insert(rebuilt.end(), unit.tabs.begin(), unit.tabs.end());
    }
  }
  for (bc_id tab : rebuilt) {
    state.favs.at(tab).space = 0;
  }
  for (bc_id tab : losers) {
    state.favs.at(tab).doomed = true;
  }
  // The tabs closing, and those of a profile nothing was merged for, keep their places at the end.
  for (bc_id tab : state.tabs) {
    if (!std::ranges::contains(rebuilt, tab)) {
      rebuilt.push_back(tab);
    }
  }
  state.tabs = std::move(rebuilt);
  SetScopeValue(BC_FAVORITES_PROFILE);
  Regroup(state);
  for (bc_id tab : losers) {
    Retire(tab);
  }
}

// Into Every Space: favorites at the same URL in different profiles become one site, and splits at
// the same URLs in their order one split, each pane a site. Each profile's list takes the merged
// order, the shown profile's first and then each other's extras, and each profile that shows
// favorites gets tabs for the sites it lacks.
void LinkProfiles(State& state) {
  std::vector<std::string> profiles = ShowingProfiles();
  for (const std::string& profile : ProfilesWithFavorites(state)) {
    if (!std::ranges::contains(profiles, profile)) {
      profiles.push_back(profile);
    }
  }
  // By the URLs of the favorite or split, how many like it come before it in its list, and the
  // tab's place in it.
  std::map<std::tuple<std::vector<std::string>, int, size_t>, int> sites;
  std::map<std::pair<std::vector<std::string>, int>, int> groups;
  for (const std::string& profile : profiles) {
    std::vector<bc_id> list;
    for (bc_id tab : state.tabs) {
      const Fav& fav = state.favs.at(tab);
      if (!fav.doomed && fav.profile == profile) {
        list.push_back(tab);
      }
    }
    std::map<std::vector<std::string>, int> counts;
    for (const FavoriteUnit& unit : MatchedUnits(state, list)) {
      const int count = counts[unit.urls]++;
      int group = 0;
      if (unit.tabs.size() > 1) {
        auto [it, fresh] = groups.try_emplace({unit.urls, count}, state.next_group);
        if (fresh) {
          ++state.next_group;
        }
        group = it->second;
      }
      for (size_t i = 0; i < unit.tabs.size(); ++i) {
        auto [it, fresh] = sites.try_emplace({unit.urls, count, i}, state.next_site);
        if (fresh) {
          ++state.next_site;
        }
        Fav& fav = state.favs.at(unit.tabs[i]);
        fav.site = it->second;
        fav.group = group;
      }
    }
  }
  SetScopeValue(BC_FAVORITES_EVERY);
  const std::vector<int> order = MergedSites(state, profiles);
  std::vector<bc_id> rebuilt;
  for (const std::string& profile : profiles) {
    std::vector<bc_id> list;
    for (bc_id tab : state.tabs) {
      const Fav& fav = state.favs.at(tab);
      if (!fav.doomed && fav.profile == profile) {
        list.push_back(tab);
      }
    }
    std::ranges::stable_sort(list, {}, [&](bc_id tab) {
      return std::ranges::find(order, state.favs.at(tab).site) - order.begin();
    });
    rebuilt.insert(rebuilt.end(), list.begin(), list.end());
  }
  for (bc_id tab : state.tabs) {
    if (!std::ranges::contains(rebuilt, tab)) {
      rebuilt.push_back(tab);
    }
  }
  state.tabs = std::move(rebuilt);
  FillIn(state);
}

}  // namespace

// static
int Favorites::scope() {
  return CurrentScope();
}

// static
void Favorites::SetScope(int scope) {
  if (scope < BC_FAVORITES_EVERY || scope > BC_FAVORITES_SPACE || scope == CurrentScope()) {
    return;
  }
  State& state = Get();
  // One step at a time: Every Space, each profile's Spaces, each Space.
  while (CurrentScope() != scope) {
    if (scope > CurrentScope()) {
      if (CurrentScope() == BC_FAVORITES_EVERY) {
        // Each profile has every site before the lists part.
        FillIn(state);
        SetScopeValue(BC_FAVORITES_PROFILE);
      } else {
        // Set first, as the copies are the lists of the Spaces.
        SetScopeValue(BC_FAVORITES_SPACE);
        CopyToSpaces(state);
      }
    } else if (CurrentScope() == BC_FAVORITES_SPACE) {
      MergeSpaces(state);
    } else {
      LinkProfiles(state);
    }
    Regroup(state);
  }
  ReportChanged();
}

// static
std::vector<bc_id> Favorites::All() {
  const State& state = Get();
  std::vector<bc_id> tabs;
  std::ranges::copy_if(state.tabs, std::back_inserter(tabs),
                       [&](bc_id tab) { return !state.favs.at(tab).doomed; });
  return tabs;
}

// static
std::vector<bc_id> Favorites::Of(const std::string& profile) {
  const State& state = Get();
  std::vector<bc_id> tabs;
  std::ranges::copy_if(state.tabs, std::back_inserter(tabs), [&](bc_id tab) {
    const Fav& fav = state.favs.at(tab);
    return !fav.doomed && fav.profile == profile;
  });
  return tabs;
}

// static
std::vector<bc_id> Favorites::In(const Space& space) {
  std::vector<bc_id> tabs;
  std::ranges::copy_if(Get().tabs, std::back_inserter(tabs),
                       [&](bc_id tab) { return ShownIn(tab, space); });
  return tabs;
}

// static
std::vector<bc_id> Favorites::OwnedBy(bc_id space) {
  const State& state = Get();
  std::vector<bc_id> tabs;
  std::ranges::copy_if(state.tabs, std::back_inserter(tabs), [&](bc_id tab) {
    const Fav& fav = state.favs.at(tab);
    return !fav.doomed && fav.space == space;
  });
  return tabs;
}

// static
bool Favorites::ShownIn(bc_id tab, const Space& space) {
  auto it = Get().favs.find(tab);
  // A window's own Space shows none, as its tabs are never saved.
  if (it == Get().favs.end() || it->second.doomed || space.own()) {
    return false;
  }
  // Under Each Space an agent's Space shows none, as no favorite belongs to it.
  return CurrentScope() == BC_FAVORITES_SPACE ? it->second.space == space.id()
                                              : it->second.profile == space.profile();
}

// static
std::string Favorites::ProfileOf(bc_id tab) {
  auto it = Get().favs.find(tab);
  return it == Get().favs.end() ? std::string() : it->second.profile;
}

// static
bc_id Favorites::SpaceOf(bc_id tab) {
  auto it = Get().favs.find(tab);
  return it == Get().favs.end() ? 0 : it->second.space;
}

// static
int Favorites::SiteOf(bc_id tab) {
  auto it = Get().favs.find(tab);
  return it == Get().favs.end() ? 0 : it->second.site;
}

// static
bool Favorites::Has(bc_id tab) {
  return Get().favs.contains(tab);
}

// static
GURL Favorites::Url(bc_id tab) {
  auto it = Get().favs.find(tab);
  return it == Get().favs.end() ? GURL() : it->second.url;
}

// static
void Favorites::Add(bc_id tab, GURL url, size_t index, bc_id space) {
  State& state = Get();
  if (state.favs.contains(tab)) {
    return;
  }
  const bool each_space = CurrentScope() == BC_FAVORITES_SPACE;
  state.favs[tab] = {.url = std::move(url),
                     .profile = ProfileDirOf(tab),
                     .space = each_space ? space : 0,
                     .site = state.next_site++};
  Insert(state, tab, index);
  if (CurrentScope() == BC_FAVORITES_EVERY) {
    const Template made = TemplateOf(state, tab);
    const Fav fav = state.favs.at(tab);
    for (const std::string& profile : ShowingProfiles()) {
      if (profile != fav.profile) {
        Make(state, made, profile, 0, fav.site, 0, index);
      }
    }
  }
  ReportChanged();
}

// static
void Favorites::Restore(bc_id tab, GURL url, bc_id space, int site) {
  State& state = Get();
  if (state.favs.contains(tab)) {
    return;
  }
  const int named = site > 0 ? site : state.next_site;
  state.next_site = std::max(state.next_site, named + 1);
  state.favs[tab] = {
      .url = std::move(url), .profile = ProfileDirOf(tab), .space = space, .site = named};
  Insert(state, tab, kEnd);
  ReportChanged();
}

// static
void Favorites::SetUrl(bc_id tab, GURL url) {
  State& state = Get();
  auto it = state.favs.find(tab);
  if (it == state.favs.end() || it->second.doomed || !url.is_valid()) {
    return;
  }
  // Under Every Space a site is one favorite in every profile, with the one URL, as Move moves it.
  const int site = it->second.site;
  bool changed = false;
  for (auto& [other, fav] : state.favs) {
    const bool same = other == tab || (CurrentScope() == BC_FAVORITES_EVERY && fav.site == site);
    if (same && !fav.doomed && fav.url != url) {
      fav.url = url;
      changed = true;
    }
  }
  if (changed) {
    ReportChanged();
  }
}

// static
void Favorites::Move(bc_id tab, size_t index) {
  State& state = Get();
  auto it = state.favs.find(tab);
  if (it == state.favs.end() || it->second.doomed) {
    return;
  }
  // Each with its split, in each profile under Every Space.
  for (bc_id each : Copies(state, tab)) {
    const SplitView* split = SplitHolding(state.splits, each);
    const std::vector<bc_id> unit = split ? split->tabs : std::vector<bc_id>{each};
    for (bc_id pane : unit) {
      std::erase(state.tabs, pane);
    }
    Insert(state, unit.front(), index);
    for (size_t i = 1; i < unit.size(); ++i) {
      InsertAfter(state, unit[i], unit[i - 1]);
    }
  }
  ReportChanged();
}

// static
void Favorites::Remove(bc_id tab) {
  State& state = Get();
  if (!state.favs.erase(tab)) {
    return;
  }
  std::erase(state.tabs, tab);
  // Its split goes on without it, and goes once one is left.
  if (SplitView* split = SplitHolding(state.splits, tab)) {
    std::erase(split->tabs, tab);
    split->shares.clear();
    if (split->expanded == tab) {
      split->expanded = 0;
    }
    if (split->focused == tab) {
      split->focused = 0;
    }
    if (split->tabs.size() < 2) {
      for (bc_id left : split->tabs) {
        state.favs.at(left).group = 0;
      }
      std::erase_if(state.splits, [&](const SplitView& each) { return &each == split; });
    }
  }
  ReportChanged();
}

// static
void Favorites::Unfavorite(bc_id tab) {
  State& state = Get();
  auto it = state.favs.find(tab);
  if (it == state.favs.end()) {
    return;
  }
  // The site goes from every profile, whose tabs for it close.
  std::vector<bc_id> others;
  if (CurrentScope() == BC_FAVORITES_EVERY) {
    for (auto& [other, fav] : state.favs) {
      if (other != tab && !fav.doomed && fav.site == it->second.site) {
        fav.doomed = true;
        others.push_back(other);
      }
    }
  }
  Remove(tab);
  for (bc_id other : others) {
    Retire(other);
  }
}

// static
void Favorites::SpaceMoved(bc_id space) {
  State& state = Get();
  const Space* in = Space::FromId(space);
  if (!in) {
    return;
  }
  if (CurrentScope() == BC_FAVORITES_EVERY) {
    Complete();
    return;
  }
  if (CurrentScope() != BC_FAVORITES_SPACE) {
    return;
  }
  // Its favorites open again in the new profile, after the old ones, which then close.
  std::vector<bc_id> left;
  for (bc_id tab : OwnedBy(space)) {
    if (state.favs.at(tab).profile == in->profile()) {
      continue;
    }
    const Fav& fav = state.favs.at(tab);
    if (Make(state, TemplateOf(state, tab), in->profile(), space, fav.site, fav.group, kEnd)) {
      left.push_back(tab);
    }
  }
  for (bc_id tab : left) {
    state.favs.at(tab).doomed = true;
  }
  Regroup(state);
  ReportChanged();
  for (bc_id tab : left) {
    Retire(tab);
  }
}

// static
void Favorites::SpaceDeleting(bc_id space) {
  State& state = Get();
  if (CurrentScope() != BC_FAVORITES_SPACE) {
    return;
  }
  const std::vector<bc_id> owned = OwnedBy(space);
  for (bc_id tab : owned) {
    state.favs.at(tab).doomed = true;
  }
  if (!owned.empty()) {
    ReportChanged();
  }
  for (bc_id tab : owned) {
    Retire(tab);
  }
}

// static
void Favorites::ProfileDeleting(const std::string& profile) {
  State& state = Get();
  const std::vector<bc_id> going = Of(profile);
  for (bc_id tab : going) {
    state.favs.at(tab).doomed = true;
  }
  if (!going.empty()) {
    ReportChanged();
  }
  for (bc_id tab : going) {
    Retire(tab);
  }
}

// static
void Favorites::Complete() {
  if (CurrentScope() == BC_FAVORITES_EVERY) {
    FillIn(Get());
    Regroup(Get());
    ReportChanged();
  }
}

// static
std::vector<bc_id> Favorites::Panes(bc_id tab) {
  const State& state = Get();
  if (!state.favs.contains(tab)) {
    return {};
  }
  const SplitView* split = SplitHolding(state.splits, tab);
  return split ? split->tabs : std::vector<bc_id>{tab};
}

// static
const SplitView* Favorites::SplitOf(bc_id tab) {
  return SplitHolding(Get().splits, tab);
}

// static
bc_id Favorites::Focused(bc_id tab) {
  const State& state = Get();
  if (!state.favs.contains(tab)) {
    return 0;
  }
  const SplitView* split = SplitHolding(state.splits, tab);
  if (!split) {
    return tab;
  }
  return std::ranges::contains(split->tabs, split->focused) ? split->focused : split->tabs.front();
}

// static
void Favorites::SetFocused(bc_id tab) {
  SplitView* split = SplitHolding(Get().splits, tab);
  if (split && split->focused != tab) {
    split->focused = tab;
    SessionChanged();
  }
}

// static
void Favorites::Separate(bc_id tab) {
  State& state = Get();
  if (!state.favs.contains(tab) || !SplitHolding(state.splits, tab)) {
    return;
  }
  for (bc_id each : Copies(state, tab)) {
    SplitView* split = SplitHolding(state.splits, each);
    if (!split) {
      continue;
    }
    std::erase(split->tabs, each);
    split->shares.clear();
    split->expanded = 0;
    if (split->focused == each) {
      split->focused = 0;
    }
    const bc_id last = split->tabs.back();
    state.favs.at(each).group = 0;
    if (split->tabs.size() < 2) {
      for (bc_id left : split->tabs) {
        state.favs.at(left).group = 0;
      }
      std::erase_if(state.splits, [&](const SplitView& view) { return &view == split; });
    }
    // Just after the tabs left in it.
    std::erase(state.tabs, each);
    InsertAfter(state, each, last);
  }
  ReportChanged();
}

// static
bool Favorites::MoveInSplit(bc_id tab, size_t index) {
  State& state = Get();
  const SplitView* in = SplitHolding(state.splits, tab);
  if (!state.favs.contains(tab) || !in || index >= in->tabs.size()) {
    return false;
  }
  for (bc_id each : Copies(state, tab)) {
    SplitView* split = SplitHolding(state.splits, each);
    if (!split || index >= split->tabs.size()) {
      continue;
    }
    // A split's tabs sit together, in its order.
    const auto first = std::ranges::find(state.tabs, split->tabs.front());
    std::erase(split->tabs, each);
    split->tabs.insert(split->tabs.begin() + index, each);
    std::ranges::copy(split->tabs, first);
  }
  ReportChanged();
  return true;
}

// static
bool Favorites::SetStacked(bc_id tab, bool stacked) {
  SplitView* split = SplitHolding(Get().splits, tab);
  if (!split) {
    return false;
  }
  split->stacked = stacked;
  ReportChanged();
  return true;
}

// static
bool Favorites::SetShares(bc_id tab, std::vector<double> shares) {
  SplitView* split = SplitHolding(Get().splits, tab);
  std::optional<std::vector<double>> scaled =
      split ? ScaledShares(std::move(shares), split->tabs.size()) : std::nullopt;
  if (!scaled) {
    return false;
  }
  split->shares = std::move(*scaled);
  split->expanded = 0;
  ReportChanged();
  return true;
}

// static
bool Favorites::SetExpanded(bc_id tab, bool expanded) {
  SplitView* split = SplitHolding(Get().splits, tab);
  if (!split) {
    return false;
  }
  split->shares.clear();
  split->expanded = expanded ? tab : 0;
  ReportChanged();
  return true;
}

// static
void Favorites::AddSplit(SplitView split,
                         const std::vector<GURL>& urls,
                         size_t index,
                         bc_id space) {
  State& state = Get();
  if (split.tabs.size() < 2 || split.tabs.size() > Space::kMaxPanes ||
      urls.size() != split.tabs.size() ||
      std::ranges::any_of(split.tabs, [&](bc_id tab) { return state.favs.contains(tab); })) {
    return;
  }
  const bool each_space = CurrentScope() == BC_FAVORITES_SPACE;
  const int group = state.next_group++;
  for (size_t i = 0; i < split.tabs.size(); ++i) {
    const bc_id tab = split.tabs[i];
    state.favs[tab] = {.url = urls[i],
                       .profile = ProfileDirOf(tab),
                       .space = each_space ? space : 0,
                       .site = state.next_site++,
                       .group = group};
    if (i == 0) {
      Insert(state, tab, index);
    } else {
      InsertAfter(state, tab, split.tabs[i - 1]);
    }
  }
  if (!std::ranges::contains(split.tabs, split.focused)) {
    split.focused = split.tabs.front();
  }
  if (!std::ranges::contains(split.tabs, split.expanded)) {
    split.expanded = 0;
  }
  if (split.shares.size() != split.tabs.size()) {
    split.shares.clear();
  }
  const std::vector<bc_id> tabs = split.tabs;
  state.splits.push_back(std::move(split));
  // Under Every Space every other profile that shows favorites gets the split in tabs of its own.
  if (CurrentScope() == BC_FAVORITES_EVERY) {
    const std::string profile = state.favs.at(tabs.front()).profile;
    for (const std::string& other : ShowingProfiles()) {
      if (other == profile) {
        continue;
      }
      bc_id after = 0;
      for (bc_id tab : tabs) {
        const Fav fav = state.favs.at(tab);
        if (const bc_id made = Make(state, TemplateOf(state, tab), other, 0, fav.site, group,
                                    index, after)) {
          after = made;
        }
      }
    }
    Regroup(state);
  }
  ReportChanged();
}

// static
void Favorites::RestoreSplit(SplitView split) {
  State& state = Get();
  if (split.tabs.size() < 2 || split.tabs.size() > Space::kMaxPanes) {
    return;
  }
  for (bc_id tab : split.tabs) {
    auto fav = state.favs.find(tab);
    if (fav == state.favs.end() || fav->second.doomed || SplitHolding(state.splits, tab) ||
        ListKey(fav->second) != ListKey(state.favs.at(split.tabs.front()))) {
      return;
    }
  }
  const int group = state.next_group++;
  std::map<bc_id, int64_t> key;
  for (bc_id tab : split.tabs) {
    state.favs.at(tab).group = group;
    key[tab] = 1;
  }
  // Together, in the split's order, at the place of its first.
  std::vector<bc_id> list = Gathered(state.tabs, key);
  const auto first =
      std::ranges::find_if(list, [&](bc_id tab) { return std::ranges::contains(split.tabs, tab); });
  std::ranges::copy(split.tabs, first);
  state.tabs = std::move(list);
  if (!std::ranges::contains(split.tabs, split.focused)) {
    split.focused = split.tabs.front();
  }
  if (!std::ranges::contains(split.tabs, split.expanded)) {
    split.expanded = 0;
  }
  if (!ScaledShares(split.shares, split.tabs.size())) {
    split.shares.clear();
  }
  state.splits.push_back(std::move(split));
  ReportChanged();
}

}  // namespace browser_core
