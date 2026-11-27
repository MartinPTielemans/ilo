#include "browser_core/closed_rule.h"

#include <algorithm>
#include <iterator>
#include <utility>

namespace browser_core::closed_rule {

std::vector<Closed> Merged(const std::vector<Closed>& tabs, const std::vector<Closed>& windows) {
  std::vector<Closed> merged;
  merged.reserve(tabs.size() + windows.size());
  std::ranges::merge(tabs, windows, std::back_inserter(merged),
                     [](const Closed& a, const Closed& b) { return a.at > b.at; });
  return merged;
}

std::optional<size_t> Next(const std::vector<Closed>& newest_first, const std::string& profile) {
  auto next = std::ranges::find_if(newest_first, [&profile](const Closed& closed) {
    return closed.by_user && closed.profile == profile;
  });
  if (next == newest_first.end()) {
    return std::nullopt;
  }
  return static_cast<size_t>(next - newest_first.begin());
}

void Save(const Closed& closed, base::DictValue& dict) {
  if (closed.by_user) {
    dict.Set("reopens", true);
  }
  if (!closed.profile.empty()) {
    dict.Set("profile", closed.profile);
  }
}

void Load(const base::DictValue& dict, Closed& closed) {
  closed.by_user = dict.FindBool("reopens").value_or(false);
  const std::string* profile = dict.FindString("profile");
  closed.profile = profile ? *profile : std::string();
}

std::vector<uint64_t> WithHistory(const std::vector<Closed>& newest_first) {
  std::vector<uint64_t> ids;
  for (const Closed& closed : newest_first) {
    if (ids.size() == kWithHistory) {
      break;
    }
    if (closed.by_user) {
      ids.push_back(closed.id);
    }
  }
  return ids;
}

base::DictValue SaveWindow(const ClosedWindow& window) {
  base::DictValue dict =
      base::DictValue()
          .Set("closed", window.closed.InMillisecondsFSinceUnixEpochIgnoringNull())
          .Set("space", window.space.AsLowercaseString());
  if (window.tab.is_valid()) {
    dict.Set("tab", window.tab.AsLowercaseString());
  }
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
  if (window.maximized) {
    dict.Set("maximized", true);
  }
  Save({.profile = window.profile, .by_user = true}, dict);
  return dict;
}

std::optional<ClosedWindow> LoadWindow(const base::DictValue& dict) {
  const std::string* space = dict.FindString("space");
  const std::optional<double> closed = dict.FindDouble("closed");
  if (!space || !closed) {
    return std::nullopt;
  }
  ClosedWindow window;
  window.closed = base::Time::FromMillisecondsSinceUnixEpoch(*closed);
  window.space = base::Uuid::ParseLowercase(*space);
  if (const std::string* tab = dict.FindString("tab")) {
    window.tab = base::Uuid::ParseLowercase(*tab);
  }
  const base::ListValue* bounds = dict.FindList("bounds");
  if (bounds && bounds->size() == 4 &&
      std::ranges::all_of(*bounds, [](const base::Value& each) { return each.is_int(); })) {
    window.bounds = gfx::Rect((*bounds)[0].GetInt(), (*bounds)[1].GetInt(),
                              (*bounds)[2].GetInt(), (*bounds)[3].GetInt());
  }
  window.sidebar_width = dict.FindDouble("sidebar_width").value_or(0);
  window.fullscreen = dict.FindBool("fullscreen").value_or(false);
  window.maximized = dict.FindBool("maximized").value_or(false);
  Closed who;
  Load(dict, who);
  window.profile = std::move(who.profile);
  return window;
}

}  // namespace browser_core::closed_rule
