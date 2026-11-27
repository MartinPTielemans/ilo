#include "browser_core/window_view.h"

#include <optional>

namespace browser_core {

namespace {

// A pair as written, [where, at], or none.
std::optional<std::pair<int, double>> PairOf(const base::Value& each) {
  const base::ListValue* pair = each.GetIfList();
  if (!pair || pair->size() != 2 || !(*pair)[0].is_int() ||
      !((*pair)[1].is_int() || (*pair)[1].is_double())) {
    return std::nullopt;
  }
  return std::pair((*pair)[0].GetInt(), (*pair)[1].GetDouble());
}

}  // namespace

void WriteWindowView(const WindowView& view, base::DictValue& dict) {
  if (view.sidebar_hidden) {
    dict.Set("sidebar_hidden", true);
  }
  if (view.on_top) {
    dict.Set("on_top", true);
  }
  if (!view.recent.empty()) {
    base::ListValue recent;
    for (const auto& [space, index] : view.recent) {
      recent.Append(base::ListValue().Append(space).Append(index));
    }
    dict.Set("recent", std::move(recent));
  }
  if (!view.scroll.empty()) {
    base::ListValue scroll;
    for (const auto& [space, offset] : view.scroll) {
      scroll.Append(base::ListValue().Append(space).Append(offset));
    }
    dict.Set("scroll", std::move(scroll));
  }
}

WindowView ReadWindowView(const base::DictValue& dict,
                          const std::vector<int>& favorites,
                          const std::vector<std::vector<int>>& kept,
                          int last_space) {
  WindowView view;
  view.sidebar_hidden = dict.FindBool("sidebar_hidden").value_or(false);
  view.on_top = dict.FindBool("on_top").value_or(false);
  if (const base::ListValue* recent = dict.FindList("recent")) {
    for (const base::Value& each : *recent) {
      const std::optional<std::pair<int, double>> pair = PairOf(each);
      if (!pair) {
        continue;
      }
      const int space = pair->first;
      const int index = static_cast<int>(pair->second);
      const std::vector<int>* in = nullptr;
      if (space == -1) {
        in = &favorites;
      } else if (space >= 0 && static_cast<size_t>(space) < kept.size()) {
        in = &kept[space];
      }
      if (in && index >= 0 && static_cast<size_t>(index) < in->size() && (*in)[index] >= 0) {
        view.recent.push_back({space, (*in)[index]});
      }
    }
  }
  if (const base::ListValue* scroll = dict.FindList("scroll")) {
    for (const base::Value& each : *scroll) {
      const std::optional<std::pair<int, double>> pair = PairOf(each);
      if (pair && pair->first >= 0 && pair->first <= last_space && pair->second > 0) {
        view.scroll.push_back(*pair);
      }
    }
  }
  return view;
}

}  // namespace browser_core
