#include "browser_core/favorites.h"

#include <algorithm>
#include <functional>
#include <iterator>
#include <limits>
#include <map>
#include <string>
#include <utility>

#include "base/functional/bind.h"
#include "base/location.h"
#include "base/no_destructor.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/host.h"
#include "browser_core/profiles.h"
#include "browser_core/session.h"
#include "browser_core/space.h"
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
  // Its tab is closing, so it's in no list.
  bool doomed = false;
};

struct State {
  // Every favorite's tab, each list in its order among them.
  std::vector<bc_id> tabs;
  std::map<bc_id, Fav> favs;
  int next_site = 1;
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

size_t IndexIn(const State& state, bc_id tab) {
  const std::vector<bc_id> list = ListOf(state, state.favs.at(tab));
  return std::ranges::find(list, tab) - list.begin();
}

// Puts |tab|, which has its entry in |state.favs| and isn't in |state.tabs|, at |index| of its list,
// else after the last of it.
void Insert(State& state, bc_id tab, size_t index) {
  const std::vector<bc_id> mine = ListOf(state, state.favs.at(tab), tab);
  auto at = index < mine.size() ? std::ranges::find(state.tabs, mine[index])
            : mine.empty()      ? state.tabs.end()
                                : std::ranges::find(state.tabs, mine.back()) + 1;
  state.tabs.insert(at, tab);
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

// The directory of |tab|'s profile, with its page or without, or empty.
std::string ProfileDirOf(bc_id tab) {
  const Tab* of = Tab::Get(tab);
  return of && of->profile() ? DirOf(of->profile()) : std::string();
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
// makes it a favorite there, of |space| under Each Space, at |index| of its list. Returns it, or 0
// if there's no window or the profile isn't loaded.
bc_id Make(State& state,
           const Template& made,
           const std::string& profile,
           bc_id space,
           int site,
           size_t index) {
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
  state.favs[tab] = {.url = made.url, .profile = profile, .space = space, .site = site};
  Insert(state, tab, index);
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
    size_t index = 0;
    for (int site : sites) {
      if (const bc_id have = InstanceOf(state, profile, site)) {
        index = IndexIn(state, have) + 1;
      } else if (const bc_id from = AnyInstanceOf(state, site)) {
        if (Make(state, TemplateOf(state, from), profile, 0, site, index)) {
          ++index;
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
        Make(state, TemplateOf(state, tab), profile, holder->id(), state.favs.at(tab).site, kEnd);
      }
    }
  }
}

// How much a duplicate is worth keeping: a page a window shows, then one that's loaded.
int Rank(bc_id tab) {
  return Window::Showing(tab) ? 2 : Tab::Loaded(tab) ? 1 : 0;
}

// Out of Each Space: each profile's Spaces' lists are merged into the profile's. The list of the
// Space the window shows comes first, then each other Space's extras in the Spaces' order, then
// those no Space has. A favorite at the same URL as an earlier one is its duplicate, and the extra
// tab closes.
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
    std::vector<bc_id> merged;
    // Where each favorite is, by URL and by how many of that URL come before it in its list.
    std::map<std::pair<std::string, int>, size_t> at;
    // Those no Space has come last, as owner 0.
    owners.push_back(0);
    for (bc_id owner : owners) {
      std::map<std::string, int> counts;
      for (bc_id tab : state.tabs) {
        const Fav& fav = state.favs.at(tab);
        if (fav.doomed || fav.profile != profile) {
          continue;
        }
        const bc_id of = std::ranges::contains(owners, fav.space) ? fav.space : 0;
        if (of != owner) {
          continue;
        }
        const std::string url = fav.url.spec();
        const std::pair<std::string, int> key(url, counts[url]++);
        auto [it, fresh] = at.try_emplace(key, merged.size());
        if (fresh) {
          merged.push_back(tab);
        } else if (const bc_id kept = merged[it->second]; Rank(tab) > Rank(kept)) {
          merged[it->second] = tab;
          losers.push_back(kept);
        } else {
          losers.push_back(tab);
        }
      }
    }
    rebuilt.insert(rebuilt.end(), merged.begin(), merged.end());
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
  for (bc_id tab : losers) {
    Retire(tab);
  }
}

// Into Every Space: favorites at the same URL in different profiles become one site, each profile's
// list takes the merged order, the shown profile's first and then each other's extras, and each
// profile that shows favorites gets tabs for the sites it lacks.
void LinkProfiles(State& state) {
  std::vector<std::string> profiles = ShowingProfiles();
  for (const std::string& profile : ProfilesWithFavorites(state)) {
    if (!std::ranges::contains(profiles, profile)) {
      profiles.push_back(profile);
    }
  }
  // By URL and by how many of that URL come before it in its list.
  std::map<std::pair<std::string, int>, int> sites;
  for (const std::string& profile : profiles) {
    std::map<std::string, int> counts;
    for (bc_id tab : state.tabs) {
      Fav& fav = state.favs.at(tab);
      if (fav.doomed || fav.profile != profile) {
        continue;
      }
      const std::string url = fav.url.spec();
      const std::pair<std::string, int> key(url, counts[url]++);
      auto [it, fresh] = sites.try_emplace(key, state.next_site);
      if (fresh) {
        ++state.next_site;
      }
      fav.site = it->second;
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
        Make(state, made, profile, 0, fav.site, index);
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
  std::vector<bc_id> moving = {tab};
  if (CurrentScope() == BC_FAVORITES_EVERY) {
    for (const auto& [other, fav] : state.favs) {
      if (other != tab && !fav.doomed && fav.site == it->second.site) {
        moving.push_back(other);
      }
    }
  }
  for (bc_id each : moving) {
    std::erase(state.tabs, each);
    Insert(state, each, index);
  }
  ReportChanged();
}

// static
void Favorites::Remove(bc_id tab) {
  State& state = Get();
  if (state.favs.erase(tab)) {
    std::erase(state.tabs, tab);
    ReportChanged();
  }
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
    if (Make(state, TemplateOf(state, tab), in->profile(), space, state.favs.at(tab).site, kEnd)) {
      left.push_back(tab);
    }
  }
  for (bc_id tab : left) {
    state.favs.at(tab).doomed = true;
  }
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
void Favorites::Complete() {
  if (CurrentScope() == BC_FAVORITES_EVERY) {
    FillIn(Get());
    ReportChanged();
  }
}

}  // namespace browser_core
