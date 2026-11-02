#include "browser_core/session.h"

#include <algorithm>
#include <map>
#include <optional>
#include <set>
#include <string>
#include <utility>
#include <vector>

#include "base/base64.h"
#include "base/callback_list.h"
#include "base/files/file_path.h"
#include "base/files/file_util.h"
#include "base/files/important_file_writer.h"
#include "base/functional/bind.h"
#include "base/functional/callback.h"
#include "base/json/json_reader.h"
#include "base/json/json_writer.h"
#include "base/no_destructor.h"
#include "base/path_service.h"
#include "base/pickle.h"
#include "base/strings/utf_string_conversions.h"
#include "base/task/sequenced_task_runner.h"
#include "base/task/thread_pool.h"
#include "base/time/time.h"
#include "base/values.h"
#include "browser_core/ad_block.h"
#include "browser_core/archive.h"
#include "browser_core/auto_archive.h"
#include "browser_core/favorites.h"
#include "browser_core/profiles.h"
#include "browser_core/space.h"
#include "browser_core/sync_spaces.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "chrome/browser/lifetime/application_lifetime_desktop.h"
#include "chrome/browser/lifetime/browser_shutdown.h"
#include "chrome/browser/profiles/profile_manager.h"
#include "chrome/common/chrome_paths.h"
#include "components/sessions/content/content_serialized_navigation_builder.h"
#include "components/sessions/core/serialized_navigation_entry.h"
#include "components/sessions/core/session_constants.h"
#include "components/sync/protocol/unique_position.pb.h"
#include "content/public/browser/navigation_controller.h"
#include "content/public/browser/navigation_entry.h"
#include "content/public/browser/web_contents.h"
#include "ui/views/widget/widget.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

// A regular window, as the session file keeps it.
struct SavedWindow {
  // The Space it showed, among the session's.
  int space = 0;
  // Its frame out of full screen, empty if it was saved before frames were kept.
  gfx::Rect bounds;
  // Its docked sidebar's width, or 0 for none.
  double sidebar_width = 0;
  bool fullscreen = false;
};

struct Saved {
  // Left to right.
  std::vector<SavedTab> favorites;
  std::vector<SavedSpace> spaces;
  // Front to back, which is the order they were last active in.
  std::vector<SavedWindow> windows;
  std::vector<ArchivedTab> archive;
  Settings settings;
};

// The windows the last launch saved, for the app to open again, and the Spaces RestoreSession
// brought back, by their places in the session.
std::vector<SavedWindow>& LoadedWindows() {
  static base::NoDestructor<std::vector<SavedWindow>> windows;
  return *windows;
}
std::vector<bc_id>& RestoredSpaces() {
  static base::NoDestructor<std::vector<bc_id>> spaces;
  return *spaces;
}

// A navigation's page state is left out past this, as in Chrome's sessions.
constexpr int kMaxNavigationSize = 63 * 1024;

// Puts |node|'s id and place in |dict|, if it has them.
void WriteNode(const Space::Node& node, base::DictValue& dict) {
  if (!node.id.is_valid() || !node.position.IsValid()) {
    return;
  }
  std::string position;
  node.position.SerializeToString(&position);
  dict.Set("id", node.id.AsLowercaseString());
  dict.Set("position", base::Base64Encode(position));
}

// The node WriteNode put in |dict|, without what's missing or can't be read.
Space::Node ReadNode(const base::DictValue& dict) {
  Space::Node node;
  if (const std::string* id = dict.FindString("id")) {
    node.id = base::Uuid::ParseLowercase(*id);
  }
  std::string bytes;
  sync_pb::UniquePosition position;
  if (const std::string* base64 = dict.FindString("position");
      base64 && base::Base64Decode(*base64, &bytes) && position.ParseFromString(bytes)) {
    node.position = syncer::UniquePosition::FromProto(position);
  }
  return node;
}

// The favorite |id| as the session keeps it, else empty history if it has no page to keep.
SavedTab SaveFavorite(bc_id id) {
  const Tab* saving = Tab::Get(id);
  SavedTab tab = saving ? saving->Save() : SavedTab();
  if (tab.history.empty()) {
    return tab;
  }
  tab.pinned_url = Favorites::Url(id);
  tab.profile = Favorites::ProfileOf(id);
  tab.site = Favorites::SiteOf(id);
  return tab;
}

std::string ScopeName(int scope) {
  switch (scope) {
    case BC_FAVORITES_EVERY:
      return "every";
    case BC_FAVORITES_SPACE:
      return "space";
    default:
      return "profile";
  }
}

int ScopeFromName(const std::string& name) {
  return name == "every"   ? BC_FAVORITES_EVERY
         : name == "space" ? BC_FAVORITES_SPACE
                           : BC_FAVORITES_PROFILE;
}

}  // namespace

SavedTab SaveTab(content::WebContents& contents) {
  content::NavigationController& history = contents.GetController();
  SavedTab tab;
  const int current = history.GetCurrentEntryIndex();
  const int pending = history.GetPendingEntryIndex();
  const int end =
      std::min(current + sessions::gMaxPersistNavigationCount + 1, history.GetEntryCount());
  for (int i = std::max(current - sessions::gMaxPersistNavigationCount, 0); i < end; ++i) {
    content::NavigationEntry* entry =
        i == pending ? history.GetPendingEntry() : history.GetEntryAtIndex(i);
    // A tab starts with an entry for no page, until its first page commits.
    if (entry->IsInitialEntry()) {
      continue;
    }
    if (i == current) {
      tab.current = tab.history.size();
    }
    tab.history.push_back(sessions::ContentSerializedNavigationBuilder::FromNavigationEntry(
        tab.history.size(), entry));
  }
  // One whose first page hasn't committed keeps the page it's loading.
  if (tab.history.empty() && history.GetPendingEntry()) {
    tab.history.push_back(
        sessions::ContentSerializedNavigationBuilder::FromNavigationEntry(0, history.GetPendingEntry()));
  }
  // One showing a system page keeps its page as the last, current entry: shown after a relaunch,
  // it loads in Chromium, which hands it to the system's engine. WebKit's own history goes.
  const Tab* owner = Tab::FromWebContents(&contents);
  if (platform::SystemPage* page = owner ? owner->system_page() : nullptr) {
    if (!tab.history.empty()) {
      tab.history.resize(tab.current + 1);
    }
    const GURL url = page->url();
    if (url.is_valid() && (tab.history.empty() || tab.history.back().virtual_url() != url)) {
      sessions::SerializedNavigationEntry& entry = tab.history.emplace_back();
      entry.set_index(tab.history.size() - 1);
      entry.set_virtual_url(url);
      entry.set_original_request_url(url);
      entry.set_title(page->title());
      entry.set_transition_type(ui::PAGE_TRANSITION_LINK);
      entry.set_timestamp(base::Time::Now());
    }
    tab.current = tab.history.size() - 1;
  }
  // One opened behind and not yet shown keeps the link it waits to load, which loads once shown
  // after a relaunch, as the other tabs put back do.
  const Tab* waiting = Tab::FromWebContents(&contents);
  if (tab.history.empty() && waiting && waiting->waiting()) {
    const content::NavigationController::LoadURLParams& load = *waiting->waiting();
    sessions::SerializedNavigationEntry& entry = tab.history.emplace_back();
    entry.set_virtual_url(load.url);
    entry.set_original_request_url(load.url);
    entry.set_referrer_url(load.referrer.url);
    entry.set_referrer_policy(static_cast<int>(load.referrer.policy));
    entry.set_transition_type(load.transition_type);
    entry.set_timestamp(base::Time::Now());
  }
  return tab;
}

SavedSpace SaveSpace(const Space& space, bc_id shown, std::vector<bc_id>* today) {
  SavedSpace to;
  to.name = space.name();
  to.node = space.node();
  to.color = space.color();
  to.icon = space.icon();
  to.theme = space.theme();
  to.touched = space.touched();
  to.profile = space.profile();
  to.agent = space.agent();
  to.pinned_open = space.pinned_open();
  int index = 0;
  // Each folder saved, to where it is among them.
  std::map<bc_id, int> folders;
  const auto folder_of = [&](bc_id id) {
    auto it = folders.find(space.FolderOf(id));
    return it == folders.end() ? -1 : it->second;
  };
  const auto node_of = [&](bc_id id) {
    const Space::Node* node = space.NodeOf(id);
    return node ? *node : Space::Node();
  };
  for (auto [ids, tabs] : {std::pair(&space.pinned_rows(), &to.pinned),
                           std::pair(&space.today(), &to.today)}) {
    bc_id above = 0;
    for (bc_id id : *ids) {
      if (space.IsFolder(id)) {
        to.folders.push_back({.name = space.FolderName(id),
                              .icon = space.FolderIcon(id),
                              .open = space.IsOpen(id),
                              .folder = folder_of(id),
                              .at = to.pinned.size(),
                              .node = node_of(id)});
        folders[id] = static_cast<int>(to.folders.size()) - 1;
        continue;
      }
      const Tab* saving = Tab::Get(id);
      SavedTab tab = saving ? saving->Save() : SavedTab();
      if (tab.history.empty()) {
        continue;
      }
      if (id == shown) {
        to.shown = index;
      }
      tab.split = above && std::ranges::contains(space.Panes(id), above);
      tab.stacked = tab.split && space.Stacked(id);
      if (std::vector<double> shares = space.Shares(id); !shares.empty()) {
        std::vector<bc_id> panes = space.Panes(id);
        tab.share = shares[std::ranges::find(panes, id) - panes.begin()];
      }
      tab.expanded = space.Expanded(id) == id;
      above = id;
      tab.pinned_url = space.PinnedUrl(id);
      if (tabs == &to.pinned) {
        tab.folder = folder_of(id);
        tab.node = node_of(id);
      }
      if (tabs == &to.today) {
        tab.last_seen = Window::Showing(id) ? base::Time::Now() : saving->last_seen();
        if (today) {
          today->push_back(id);
        }
      }
      tabs->push_back(std::move(tab));
      ++index;
    }
  }
  // Under Each Space, the favorites that are its own.
  for (bc_id id : Favorites::OwnedBy(space.id())) {
    SavedTab tab = SaveFavorite(id);
    if (tab.history.empty()) {
      continue;
    }
    if (id == shown) {
      to.shown_own_favorite = to.favorites.size();
    }
    to.favorites.push_back(std::move(tab));
  }
  return to;
}

namespace {

Saved Collect() {
  Saved saved;
  // The regular windows, as nothing of a window with a Space of its own is saved.
  std::vector<Window*> windows = Window::All();
  std::erase_if(windows, [](const Window* each) { return each->own_space(); });
  saved.settings = UserSettings();
  // Each Space saved, by its place among them.
  std::map<bc_id, int> spaces;
  // Each favorite saved, by its place among them. Those of a Space are saved with it.
  std::map<bc_id, int> favorites;
  for (bc_id id : Favorites::All()) {
    SavedTab tab = Favorites::SpaceOf(id) ? SavedTab() : SaveFavorite(id);
    if (tab.history.empty()) {
      continue;
    }
    favorites[id] = saved.favorites.size();
    saved.favorites.push_back(std::move(tab));
  }
  for (Space* space : Space::All()) {
    spaces[space->id()] = saved.spaces.size();
    // The tab the window active last showed there, else the one another window showed.
    bc_id shown = 0;
    for (const Window* each : windows) {
      if ((shown = each->SelectedIn(space->id()))) {
        break;
      }
    }
    SavedSpace& to = saved.spaces.emplace_back(SaveSpace(*space, shown));
    if (auto favorite = favorites.find(shown); favorite != favorites.end()) {
      to.shown_favorite = favorite->second;
    }
  }
  for (const Window* window : windows) {
    auto space = spaces.find(window->space());
    views::Widget* widget = window->widget();
    saved.windows.push_back(
        {.space = space == spaces.end() ? 0 : space->second,
         .bounds = widget ? widget->GetRestoredBounds() : gfx::Rect(),
         .sidebar_width = window->sidebar_width(),
         // The user's full screen, not a page's: one saved as a video plays would come back
         // without it.
         .fullscreen = widget && widget->IsFullscreen() && !window->FullScreenForPage()});
  }
  saved.archive = Archived();
  return saved;
}

// The file holds JSON, with each navigation in Chrome's own pickle, as its sessions write it.
std::optional<std::string> ToJson(const Saved& saved) {
  // The folder |folder| of |folders| is, by its id, in |dict|, if it's in one.
  const auto write_folder = [](const std::vector<SavedFolder>& folders, int folder,
                               base::DictValue& dict) {
    if (folder >= 0 && folders[folder].node.id.is_valid()) {
      dict.Set("parent", folders[folder].node.id.AsLowercaseString());
    }
  };
  // |tabs|, pinned in |folders| if they're a Space's pinned tabs.
  const auto to_list = [&](const std::vector<SavedTab>& tabs,
                           const std::vector<SavedFolder>* folders = nullptr) {
    base::ListValue list;
    for (const SavedTab& tab : tabs) {
      base::ListValue history;
      for (const sessions::SerializedNavigationEntry& navigation : tab.history) {
        base::Pickle pickle;
        navigation.WriteToPickle(kMaxNavigationSize, &pickle);
        history.Append(base::Base64Encode(pickle.AsBytes()));
      }
      base::DictValue dict;
      dict.Set("history", std::move(history));
      dict.Set("current", tab.current);
      if (tab.pinned_url.is_valid()) {
        dict.Set("pinned_url", tab.pinned_url.spec());
      }
      if (!tab.last_seen.is_null()) {
        dict.Set("seen", tab.last_seen.InMillisecondsFSinceUnixEpoch());
      }
      if (!tab.name.empty()) {
        dict.Set("name", tab.name);
      }
      if (!tab.icon.empty()) {
        dict.Set("icon", tab.icon);
      }
      if (tab.split) {
        dict.Set("split", true);
      }
      if (tab.stacked) {
        dict.Set("stacked", true);
      }
      if (tab.share) {
        dict.Set("share", tab.share);
      }
      if (tab.expanded) {
        dict.Set("expanded", true);
      }
      if (folders) {
        write_folder(*folders, tab.folder, dict);
        WriteNode(tab.node, dict);
      }
      if (!tab.profile.empty()) {
        dict.Set("profile", tab.profile);
      }
      if (tab.site) {
        dict.Set("site", tab.site);
      }
      list.Append(std::move(dict));
    }
    return list;
  };
  base::ListValue spaces;
  for (const SavedSpace& space : saved.spaces) {
    // Each folder in the pinned list, above what's in it.
    base::ListValue pinned = to_list(space.pinned, &space.folders);
    for (auto folder = space.folders.rbegin(); folder != space.folders.rend(); ++folder) {
      base::DictValue row = base::DictValue().Set("folder", folder->name).Set("open", folder->open);
      if (!folder->icon.empty()) {
        row.Set("icon", folder->icon);
      }
      write_folder(space.folders, folder->folder, row);
      WriteNode(folder->node, row);
      pinned.Insert(pinned.begin() + folder->at, base::Value(std::move(row)));
    }
    base::DictValue dict = base::DictValue()
                               .Set("name", space.name)
                               .Set("pinned", std::move(pinned))
                               .Set("today", to_list(space.today))
                               .Set("shown", space.shown);
    WriteNode(space.node, dict);
    if (space.shown_favorite >= 0) {
      dict.Set("shown_favorite", space.shown_favorite);
    }
    if (!space.favorites.empty()) {
      dict.Set("favorites", to_list(space.favorites));
      if (space.shown_own_favorite >= 0) {
        dict.Set("shown_own_favorite", space.shown_own_favorite);
      }
    }
    if (space.color) {
      dict.Set("color", static_cast<int>(*space.color));
    }
    if (!space.icon.empty()) {
      dict.Set("icon", space.icon);
    }
    if (!space.theme.empty()) {
      dict.Set("theme", space.theme);
    }
    if (!space.pinned_open) {
      dict.Set("pinned_open", false);
    }
    if (!space.touched) {
      dict.Set("untouched", true);
    }
    if (!space.profile.empty()) {
      dict.Set("profile", space.profile);
    }
    if (space.agent) {
      dict.Set("agent",
               base::DictValue().Set("id", space.agent->id).Set("name", space.agent->name));
    }
    spaces.Append(std::move(dict));
  }
  base::DictValue root;
  root.Set("favorites", to_list(saved.favorites));
  root.Set("spaces", std::move(spaces));
  base::ListValue windows;
  for (const SavedWindow& window : saved.windows) {
    base::DictValue dict = base::DictValue().Set("space", window.space);
    if (!window.bounds.IsEmpty()) {
      dict.Set("bounds", base::ListValue()
                             .Append(window.bounds.x())
                             .Append(window.bounds.y())
                             .Append(window.bounds.width())
                             .Append(window.bounds.height()));
    }
    if (window.sidebar_width > 0) {
      dict.Set("sidebar_width", window.sidebar_width);
    }
    if (window.fullscreen) {
      dict.Set("fullscreen", true);
    }
    windows.Append(std::move(dict));
  }
  root.Set("windows", std::move(windows));
  base::ListValue archive;
  for (const ArchivedTab& tab : saved.archive) {
    archive.Append(base::DictValue()
                       .Set("url", tab.url.spec())
                       .Set("title", tab.title)
                       .Set("archived", tab.archived.InMillisecondsFSinceUnixEpochIgnoringNull()));
  }
  root.Set("archive", std::move(archive));
  root.Set("settings", base::DictValue()
                           .Set("archive_after", saved.settings.archive_after.InSecondsF())
                           .Set("haptics", saved.settings.haptics)
                           .Set("warn_before_quitting", saved.settings.warn_before_quitting)
                           .Set("archive_noted", saved.settings.archive_noted)
                           .Set("restore_windows", saved.settings.restore_windows)
                           .Set("warn_moving_profiles", saved.settings.warn_moving_profiles)
                           .Set("favorites_scope", ScopeName(saved.settings.favorites_scope))
                           .Set("unload_idle_tabs", saved.settings.unload_idle_tabs)
                           .Set("ad_blocking", saved.settings.ad_blocking)
                           .Set("send_crash_reports", saved.settings.send_crash_reports)
                           .Set("link_previews", saved.settings.link_previews)
                           .Set("auto_picture_in_picture",
                                saved.settings.auto_picture_in_picture)
                           .Set("option_reveals_sidebar", saved.settings.option_reveals_sidebar)
                           .Set("hover_kinds", saved.settings.hover_kinds)
                           .Set("crash_report_offered", saved.settings.crash_report_offered)
                           .Set("widevine", saved.settings.widevine));
  return base::WriteJson(root);
}

std::optional<SavedTab> ReadTab(const base::Value& value) {
  const base::DictValue* dict = value.GetIfDict();
  const base::ListValue* history = dict ? dict->FindList("history") : nullptr;
  if (!history) {
    return std::nullopt;
  }
  SavedTab tab;
  for (const base::Value& navigation : *history) {
    const std::string* base64 = navigation.GetIfString();
    std::optional<std::vector<uint8_t>> bytes =
        base64 ? base::Base64Decode(*base64) : std::nullopt;
    if (!bytes) {
      return std::nullopt;
    }
    base::Pickle pickle = base::Pickle::WithUnownedBuffer(*bytes);
    base::PickleIterator iterator(pickle);
    if (!tab.history.emplace_back().ReadFromPickle(&iterator)) {
      return std::nullopt;
    }
  }
  tab.current = dict->FindInt("current").value_or(0);
  if (tab.current < 0 || tab.current >= static_cast<int>(tab.history.size())) {
    return std::nullopt;
  }
  if (const std::string* url = dict->FindString("pinned_url")) {
    tab.pinned_url = GURL(*url);
  }
  if (std::optional<double> seen = dict->FindDouble("seen")) {
    tab.last_seen = base::Time::FromMillisecondsSinceUnixEpoch(*seen);
  }
  if (const std::string* name = dict->FindString("name")) {
    tab.name = *name;
  }
  if (const std::string* icon = dict->FindString("icon")) {
    tab.icon = *icon;
  }
  tab.split = dict->FindBool("split").value_or(false);
  tab.stacked = tab.split && dict->FindBool("stacked").value_or(false);
  tab.share = dict->FindDouble("share").value_or(0);
  tab.expanded = dict->FindBool("expanded").value_or(false);
  if (const std::string* profile = dict->FindString("profile")) {
    tab.profile = *profile;
  }
  tab.site = std::max(dict->FindInt("site").value_or(0), 0);
  return tab;
}

// A tab or Space that can't be read is left out, and the rest come back.
std::optional<Saved> Read(const base::FilePath& path) {
  std::string json;
  if (!base::ReadFileToString(path, &json)) {
    return std::nullopt;
  }
  std::optional<base::DictValue> root = base::JSONReader::ReadDict(json, base::JSON_PARSE_RFC);
  const base::ListValue* spaces = root ? root->FindList("spaces") : nullptr;
  if (!spaces) {
    return std::nullopt;
  }
  Saved saved;
  // Where each favorite saved went among those read, or -1.
  std::vector<int> favorites;
  if (const base::ListValue* list = root->FindList("favorites")) {
    for (const base::Value& value : *list) {
      std::optional<SavedTab> tab = ReadTab(value);
      favorites.push_back(tab ? static_cast<int>(saved.favorites.size()) : -1);
      if (tab) {
        saved.favorites.push_back(std::move(*tab));
      }
    }
  }
  for (const base::Value& value : *spaces) {
    const base::DictValue* space = value.GetIfDict();
    const std::string* name = space ? space->FindString("name") : nullptr;
    const base::ListValue* today = space ? space->FindList("today") : nullptr;
    if (!name || !today) {
      continue;
    }
    SavedSpace& to = saved.spaces.emplace_back();
    to.name = *name;
    to.node = ReadNode(*space);
    if (std::optional<int> color = space->FindInt("color")) {
      to.color = static_cast<uint32_t>(*color) & 0xffffff;
    }
    if (const std::string* icon = space->FindString("icon")) {
      to.icon = *icon;
    }
    if (const std::string* theme = space->FindString("theme")) {
      to.theme = *theme;
    }
    to.pinned_open = space->FindBool("pinned_open").value_or(true);
    to.touched = !space->FindBool("untouched").value_or(false);
    if (const std::string* profile = space->FindString("profile")) {
      to.profile = *profile;
    }
    // One whose agent can't be read is the user's, out of every agent's reach.
    const base::DictValue* agent = space->FindDict("agent");
    const std::string* agent_id = agent ? agent->FindString("id") : nullptr;
    const std::string* agent_name = agent ? agent->FindString("name") : nullptr;
    if (agent_id && !agent_id->empty() && agent_name) {
      to.agent = Space::Agent{.id = *agent_id, .name = *agent_name};
    }
    const int shown = space->FindInt("shown").value_or(-1);
    const int shown_favorite = space->FindInt("shown_favorite").value_or(-1);
    if (shown_favorite >= 0 && shown_favorite < static_cast<int>(favorites.size())) {
      to.shown_favorite = favorites[shown_favorite];
    }
    // Under Each Space, the favorites that are the Space's own.
    std::vector<int> own;
    if (const base::ListValue* list = space->FindList("favorites")) {
      for (const base::Value& row : *list) {
        std::optional<SavedTab> tab = ReadTab(row);
        own.push_back(tab ? static_cast<int>(to.favorites.size()) : -1);
        if (tab) {
          to.favorites.push_back(std::move(*tab));
        }
      }
    }
    const int shown_own = space->FindInt("shown_own_favorite").value_or(-1);
    if (shown_own >= 0 && shown_own < static_cast<int>(own.size())) {
      to.shown_own_favorite = own[shown_own];
    }
    // Saved before Spaces had pinned tabs, it has none.
    const base::ListValue no_pinned;
    const base::ListValue* pinned = space->FindList("pinned");
    int index = 0;
    int kept = 0;
    // Each folder read, by its id, to where it is among them.
    std::map<std::string, int> folder_ids;
    // The folders holding the row being read, outermost first, for a Space saved before folders
    // had ids, whose rows say only how many folders they're in.
    std::vector<int> holders;
    // The folder the pinned row |dict| is in, among those read, else -1.
    const auto folder_of = [&](const base::DictValue& dict) {
      if (const std::string* parent = dict.FindString("parent")) {
        auto it = folder_ids.find(*parent);
        return it == folder_ids.end() ? -1 : it->second;
      }
      const size_t depth = std::max(dict.FindInt("depth").value_or(0), 0);
      holders.resize(std::min(depth, holders.size()));
      return holders.empty() ? -1 : holders.back();
    };
    for (auto [list, tabs] : {std::pair(pinned ? pinned : &no_pinned, &to.pinned),
                              std::pair(today, &to.today)}) {
      for (const base::Value& row : *list) {
        const base::DictValue* dict = row.GetIfDict();
        const std::string* folder = dict ? dict->FindString("folder") : nullptr;
        if (folder && tabs == &to.pinned) {
          SavedFolder& read = to.folders.emplace_back(SavedFolder{
              .name = *folder,
              .icon = dict->FindString("icon") ? *dict->FindString("icon") : std::string(),
              .open = dict->FindBool("open").value_or(true),
              .folder = folder_of(*dict),
              .at = to.pinned.size(),
              .node = ReadNode(*dict),
          });
          const int at = static_cast<int>(to.folders.size()) - 1;
          holders.push_back(at);
          if (read.node.id.is_valid()) {
            folder_ids[read.node.id.AsLowercaseString()] = at;
          }
          continue;
        }
        std::optional<SavedTab> tab = ReadTab(row);
        if (index++ == shown && tab) {
          to.shown = kept;
        }
        if (tab && tabs == &to.pinned) {
          tab->folder = folder_of(*dict);
          tab->node = ReadNode(*dict);
        }
        if (tab) {
          tabs->push_back(std::move(*tab));
          ++kept;
        }
      }
    }
  }
  if (saved.spaces.empty()) {
    return std::nullopt;
  }
  const int last_space = static_cast<int>(saved.spaces.size()) - 1;
  if (const base::ListValue* windows = root->FindList("windows")) {
    for (const base::Value& value : *windows) {
      const base::DictValue* window = value.GetIfDict();
      if (!window) {
        continue;
      }
      SavedWindow& to = saved.windows.emplace_back();
      to.space = std::clamp(window->FindInt("space").value_or(0), 0, last_space);
      const base::ListValue* bounds = window->FindList("bounds");
      if (bounds && bounds->size() == 4 &&
          std::ranges::all_of(*bounds, [](const base::Value& each) { return each.is_int(); })) {
        to.bounds = gfx::Rect((*bounds)[0].GetInt(), (*bounds)[1].GetInt(), (*bounds)[2].GetInt(),
                              (*bounds)[3].GetInt());
      }
      to.sidebar_width = window->FindDouble("sidebar_width").value_or(0);
      to.fullscreen = window->FindBool("fullscreen").value_or(false);
    }
  } else {
    // Saved when the session kept one window, without its frame.
    saved.windows.push_back(
        {.space = std::clamp(root->FindInt("shown").value_or(0), 0, last_space),
         .sidebar_width = root->FindDouble("sidebar_width").value_or(0),
         .fullscreen = root->FindBool("fullscreen").value_or(false)});
  }
  if (const base::ListValue* archive = root->FindList("archive")) {
    for (const base::Value& value : *archive) {
      const base::DictValue* tab = value.GetIfDict();
      const std::string* url = tab ? tab->FindString("url") : nullptr;
      const std::string* title = tab ? tab->FindString("title") : nullptr;
      std::optional<double> archived = tab ? tab->FindDouble("archived") : std::nullopt;
      if (url && title && archived && GURL(*url).is_valid()) {
        saved.archive.push_back(
            {.url = GURL(*url),
             .title = *title,
             .archived = base::Time::FromMillisecondsSinceUnixEpoch(*archived)});
      }
    }
  }
  if (const base::DictValue* settings = root->FindDict("settings")) {
    const double archive_after = settings->FindDouble("archive_after").value_or(0);
    if (archive_after > 0) {
      saved.settings.archive_after = base::Seconds(archive_after);
    }
    saved.settings.haptics = settings->FindBool("haptics").value_or(true);
    saved.settings.warn_before_quitting =
        settings->FindBool("warn_before_quitting").value_or(true);
    saved.settings.archive_noted = settings->FindBool("archive_noted").value_or(false);
    saved.settings.warn_moving_profiles =
        settings->FindBool("warn_moving_profiles").value_or(true);
    saved.settings.restore_windows = settings->FindBool("restore_windows").value_or(true);
    if (const std::string* scope = settings->FindString("favorites_scope")) {
      saved.settings.favorites_scope = ScopeFromName(*scope);
    }
    saved.settings.unload_idle_tabs = settings->FindBool("unload_idle_tabs").value_or(true);
    saved.settings.ad_blocking = settings->FindBool("ad_blocking").value_or(true);
    saved.settings.send_crash_reports =
        settings->FindBool("send_crash_reports").value_or(false);
    saved.settings.link_previews = settings->FindBool("link_previews").value_or(true);
    saved.settings.auto_picture_in_picture =
        settings->FindBool("auto_picture_in_picture").value_or(true);
    saved.settings.option_reveals_sidebar =
        settings->FindBool("option_reveals_sidebar").value_or(true);
    saved.settings.hover_kinds = settings->FindInt("hover_kinds").value_or(15);
    saved.settings.crash_report_offered =
        settings->FindDouble("crash_report_offered").value_or(0);
    saved.settings.widevine = settings->FindBool("widevine").value_or(false);
  }
  return saved;
}

class Saver : public base::ImportantFileWriter::BackgroundDataSerializer {
 public:
  explicit Saver(const base::FilePath& path)
      // Blocks shutdown, so the Spaces saved as the browser quits reach the disk.
      : task_runner_(base::ThreadPool::CreateSequencedTaskRunner(
            {base::MayBlock(), base::TaskPriority::USER_VISIBLE,
             base::TaskShutdownBehavior::BLOCK_SHUTDOWN})),
        writer_(path, task_runner_),
        closing_all_(chrome::AddClosingAllBrowsersCallback(
            base::BindRepeating(&Saver::OnClosingAll, base::Unretained(this)))) {}

  void Load(base::OnceClosure loaded) {
    task_runner_->PostTaskAndReplyWithResult(
        FROM_HERE, base::BindOnce(&Read, writer_.path()),
        base::BindOnce(&Saver::OnRead, base::Unretained(this), std::move(loaded)));
  }

  void Changed() {
    if (!kept_) {
      writer_.ScheduleWriteWithBackgroundDataSerializer(this);
    }
  }

  void Keep() {
    if (!kept_) {
      writer_.ScheduleWriteWithBackgroundDataSerializer(this);
      writer_.DoScheduledWrite();
      kept_ = true;
    }
  }

  void Flush() {
    if (writer_.HasPendingWrite()) {
      writer_.DoScheduledWrite();
    }
  }

  std::optional<Saved> TakeLoaded() { return std::exchange(loaded_, std::nullopt); }
  const std::optional<Saved>& loaded() const { return loaded_; }

  // base::ImportantFileWriter::BackgroundDataSerializer:
  base::ImportantFileWriter::BackgroundDataProducerCallback
  GetSerializedDataProducerForBackgroundSequence() override {
    // What's in the Spaces is read here, and written out as JSON on the writer's sequence.
    return base::BindOnce(&ToJson, Collect());
  }

 private:
  // At logout, restart or SIGTERM, Chrome writes what it must and exits without waiting for the
  // writer's sequence, so the Spaces are written here, as it allows.
  void OnClosingAll(bool closing) {
    if (!closing || kept_ ||
        browser_shutdown::GetShutdownType() != browser_shutdown::ShutdownType::kEndSession) {
      return;
    }
    kept_ = true;
    if (std::optional<std::string> json = ToJson(Collect())) {
      base::ImportantFileWriter::WriteFileAtomically(writer_.path(), *json);
    }
  }

  void OnRead(base::OnceClosure loaded, std::optional<Saved> saved) {
    if (saved) {
      LoadArchive(std::exchange(saved->archive, {}));
      UserSettings() = saved->settings;
      LoadedWindows() = std::exchange(saved->windows, {});
      // Without the last session's windows, the launch opens one, where the front one was, and
      // windowed, as a new window would be.
      if (!UserSettings().restore_windows && !LoadedWindows().empty()) {
        LoadedWindows().erase(LoadedWindows().begin() + 1, LoadedWindows().end());
        LoadedWindows().front().fullscreen = false;
      }
    }
    // The profile the browser started in loaded before the settings were read.
    ApplyAdBlocking();
    ApplyAutoPictureInPicture(/*changed=*/false);
    ArchiveDefaultRead();
    // The profiles the Spaces and favorites are in load before they come back.
    std::set<std::string> profiles;
    if (saved) {
      for (const SavedTab& favorite : saved->favorites) {
        profiles.insert(favorite.profile);
      }
      for (const SavedSpace& space : saved->spaces) {
        profiles.insert(space.profile);
      }
      profiles.erase(std::string());
    }
    loaded_ = std::move(saved);
    LoadProfiles(profiles, std::move(loaded));
  }

  scoped_refptr<base::SequencedTaskRunner> task_runner_;
  base::ImportantFileWriter writer_;
  base::CallbackListSubscription closing_all_;
  std::optional<Saved> loaded_;
  bool kept_ = false;
};

// Made by LoadSession and never deleted, as Chrome's shutdown may still be writing its file.
Saver* g_saver = nullptr;


}  // namespace

void LoadSession(base::OnceClosure loaded) {
  base::FilePath dir;
  base::PathService::Get(chrome::DIR_USER_DATA, &dir);
  g_saver = new Saver(dir.Append(FILE_PATH_LITERAL("Spaces.json")));
  g_saver->Load(std::move(loaded));
}

void SessionChanged() {
  if (g_saver) {
    g_saver->Changed();
  }
  sync_spaces::Changed();
}

void KeepSession() {
  if (g_saver) {
    g_saver->Keep();
  }
}

void FlushSession() {
  if (g_saver) {
    g_saver->Flush();
  }
}

Settings& UserSettings() {
  static Settings settings;
  return settings;
}

double SavedSidebarWidth() {
  return LoadedWindows().empty() ? 0 : LoadedWindows().front().sidebar_width;
}

std::vector<gfx::Rect> SavedWindowFrames() {
  std::vector<gfx::Rect> frames;
  for (const SavedWindow& window : LoadedWindows()) {
    frames.push_back(window.bounds);
  }
  return frames;
}

bool RestoreSavedWindow(Window& window, size_t index) {
  if (index >= LoadedWindows().size()) {
    return false;
  }
  const SavedWindow& saved = LoadedWindows()[index];
  window.SetSidebarWidth(saved.sidebar_width);
  const size_t at = static_cast<size_t>(saved.space);
  const bc_id space = at < RestoredSpaces().size() ? RestoredSpaces()[at] : 0;
  if (!space) {
    return false;
  }
  // The tab the Space showed, which the Spaces were put back with in the first window.
  for (const Window* each : Window::All()) {
    if (bc_id tab = &window == each ? 0 : each->SelectedIn(space)) {
      window.SetSelectedIn(space, tab);
      break;
    }
  }
  // A new window's shows no page.
  window.ShowSpace(space, /*page=*/UserSettings().restore_windows);
  // The window is on screen already, so this is the transition the user would start. Only the
  // front window's: one behind would take the user to its own macOS Space as the browser launches,
  // so it comes back windowed, at the frame it had before full screen.
  if (index == 0 && saved.fullscreen && window.widget()) {
    window.widget()->SetFullscreen(true);
  }
  return true;
}

Space* RestoreSpace(Window& window,
                    Profile* profile,
                    const SavedSpace& saved_space,
                    bc_id id,
                    size_t at) {
  Space* space = Space::Create(saved_space.name, id, saved_space.agent, at);
  space->SetColor(saved_space.color);
  space->SetIcon(saved_space.icon);
  space->SetTheme(saved_space.theme);
  space->SetPinnedOpen(saved_space.pinned_open);
  space->SetTouched(saved_space.touched);
  space->SetProfile(DirOf(profile));
  // Else the Space comes back as a new window shows it: its pinned tabs, none shown, and no Today
  // tabs, which go in the archive.
  const bool whole = UserSettings().restore_windows;
  const auto restore = [&](const SavedTab& saved_tab, int index) -> bc_id {
    // Without its page until it's first shown, which saves launch making a page for each tab.
    const bc_id tab = Tab::Restore(profile, saved_tab);
    if (tab && whole && index == saved_space.shown) {
      window.SetSelectedIn(space->id(), tab);
    }
    return tab;
  };
  const int pinned = static_cast<int>(saved_space.pinned.size());
  std::vector<bc_id> pinned_ids(saved_space.pinned.size());
  std::vector<bc_id> today_ids(saved_space.today.size());
  // Each folder put back, by where it is among those saved, each in the one saved before it that
  // holds it.
  std::vector<bc_id> folder_ids;
  const auto in = [&](int folder) {
    return folder >= 0 && folder < static_cast<int>(folder_ids.size()) ? folder_ids[folder] : 0;
  };
  std::map<bc_id, Space::Node> nodes;
  const auto restore_folders = [&](size_t above) {
    while (folder_ids.size() < saved_space.folders.size() &&
           saved_space.folders[folder_ids.size()].at <= above) {
      const SavedFolder& saved_folder = saved_space.folders[folder_ids.size()];
      // At the end of the folder holding it, as the rows come back top to bottom.
      const bc_id folder = space->AddFolder(saved_folder.name, in(saved_folder.folder),
                                            space->pinned_rows().size());
      space->SetOpen(folder, saved_folder.open);
      space->SetFolderIcon(folder, saved_folder.icon);
      nodes[folder] = saved_folder.node;
      folder_ids.push_back(folder);
    }
  };
  for (int j = 0; j < pinned; ++j) {
    restore_folders(j);
    const SavedTab& saved_tab = saved_space.pinned[j];
    if (bc_id tab = pinned_ids[j] = restore(saved_tab, j)) {
      // Saved before pinned tabs kept their pinned URL, it's pinned where it is.
      nodes[tab] = saved_tab.node;
      space->AddPinned(tab, in(saved_tab.folder), space->pinned_rows().size(),
                       saved_tab.pinned_url.is_valid()
                           ? saved_tab.pinned_url
                           : saved_tab.history[saved_tab.current].virtual_url());
    }
  }
  restore_folders(saved_space.pinned.size());
  space->RestoreNodes(saved_space.node, nodes);
  // Under Each Space, its favorites, in its profile.
  for (size_t j = 0; j < saved_space.favorites.size(); ++j) {
    const SavedTab& saved_tab = saved_space.favorites[j];
    if (bc_id tab = Tab::Restore(profile, saved_tab)) {
      Favorites::Restore(tab,
                         saved_tab.pinned_url.is_valid()
                             ? saved_tab.pinned_url
                             : saved_tab.history[saved_tab.current].virtual_url(),
                         space->id(), saved_tab.site);
      if (whole && static_cast<int>(j) == saved_space.shown_own_favorite) {
        window.SetSelectedIn(space->id(), tab);
      }
    }
  }
  // Bottom up, as each goes at the top, of the Space or of the archive.
  for (size_t j = saved_space.today.size(); j-- > 0;) {
    if (!whole) {
      const SavedTab& saved_tab = saved_space.today[j];
      if (saved_tab.current >= 0 &&
          static_cast<size_t>(saved_tab.current) < saved_tab.history.size()) {
        const sessions::SerializedNavigationEntry& entry = saved_tab.history[saved_tab.current];
        Archive(/*tab=*/0, entry.virtual_url(), base::UTF16ToUTF8(entry.title()));
      }
      continue;
    }
    if (bc_id tab = today_ids[j] = restore(saved_space.today[j], pinned + static_cast<int>(j))) {
      space->Add(tab, /*opener=*/0, /*foreground=*/false);
    }
  }
  for (auto [tabs, ids] : {std::pair(&saved_space.pinned, &pinned_ids),
                           std::pair(&saved_space.today, &today_ids)}) {
    // The shares of the split being remade, and the tab that took its card, which it takes once
    // its last tab is in it.
    std::vector<double> shares;
    bc_id expanded = 0;
    for (size_t j = 0; j < ids->size(); ++j) {
      const SavedTab& tab = (*tabs)[j];
      if (!tab.split || j == 0) {
        shares.clear();
        expanded = 0;
      } else if ((*ids)[j] && (*ids)[j - 1] && space->Split((*ids)[j], (*ids)[j - 1], GURL()) &&
                 tab.stacked) {
        space->SetStacked((*ids)[j], true);
      }
      if ((*ids)[j]) {
        shares.push_back(tab.share);
        if (tab.expanded) {
          expanded = (*ids)[j];
        }
      }
      if (tab.split && (*ids)[j] && (j + 1 == ids->size() || !(*tabs)[j + 1].split)) {
        space->SetShares((*ids)[j], shares);
        if (expanded) {
          space->SetExpanded(expanded, true);
        }
      }
    }
  }
  return space;
}

bool RestoreSession(Window& window) {
  std::optional<Saved> saved = g_saver ? g_saver->TakeLoaded() : std::nullopt;
  if (!saved || !StartProfile() || !Space::All().empty()) {
    return false;
  }
  // First, as a Space may show one. Each in its profile, which loaded with the session, else in the
  // one the core started with.
  std::vector<bc_id> favorites;
  for (const SavedTab& saved_tab : saved->favorites) {
    const bc_id tab = favorites.emplace_back(
        Tab::Restore(ProfileFor(saved_tab.profile), saved_tab));
    if (tab) {
      Favorites::Restore(tab,
                         saved_tab.pinned_url.is_valid()
                             ? saved_tab.pinned_url
                             : saved_tab.history[saved_tab.current].virtual_url(),
                         /*space=*/0, saved_tab.site);
    }
  }
  RestoredSpaces().clear();
  for (const SavedSpace& saved_space : saved->spaces) {
    Space* space = RestoreSpace(window, ProfileFor(saved_space.profile), saved_space);
    RestoredSpaces().push_back(space->id());
    if (UserSettings().restore_windows && saved_space.shown_favorite >= 0 &&
        favorites[saved_space.shown_favorite]) {
      window.SetSelectedIn(space->id(), favorites[saved_space.shown_favorite]);
    }
  }
  // Under Every Space, a profile that came up short of a site gets its tab for it.
  Favorites::Complete();
  // A session saved with no regular window open shows the first Space.
  if (!RestoreSavedWindow(window, 0)) {
    window.ShowSpace(RestoredSpaces().front(), /*page=*/UserSettings().restore_windows);
  }
  return true;
}

}  // namespace browser_core
