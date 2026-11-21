#include "browser_core/shortcut_key.h"

#include <algorithm>
#include <array>
#include <utility>

namespace browser_core {

namespace {

// The split keys, each with the web's code for its place on the US layout.
constexpr std::array<std::pair<char, const char*>, 3> kSplitKeys = {{
    {'[', "BracketLeft"},
    {']', "BracketRight"},
    {'-', "Minus"},
}};

bool IsSplitKey(const std::string& key) {
  return key.size() == 1 && std::ranges::any_of(kSplitKeys, [&](const auto& split) {
           return split.first == key[0];
         });
}

}  // namespace

bool SplitKeysHeld(const HeldKeys& held, KeysPlatform platform) {
  switch (platform) {
    case KeysPlatform::kMac:
      return held.control && held.shift && !held.meta;
    case KeysPlatform::kWindows:
      return held.shift && held.control != held.alt && !held.meta;
    case KeysPlatform::kLinux:
      return held.shift && (held.control || held.alt);
  }
  return false;
}

SplitLayout::SplitLayout(LayoutKeys layout_keys) : keys(std::move(layout_keys)) {
  // A layout that couldn't be read names no key.
  if (keys.empty()) {
    return;
  }
  auto base = [&](const char* code) {
    auto key = keys.find(code);
    return key == keys.end() ? std::string() : key->second;
  };
  for (const auto& [split, place] : kSplitKeys) {
    const std::string key(1, split);
    const bool own =
        std::ranges::any_of(keys, [&](const auto& each) { return each.second == key; });
    if (!own && !IsSplitKey(base(place))) {
      by_place.push_back(split);
    }
  }
}

const SplitLayout& SplitLayoutCache::For(const std::string& id,
                                         base::FunctionRef<LayoutKeys()> read) {
  if (id_ != id) {
    layout_ = SplitLayout(read());
    id_ = id;
  }
  return layout_;
}

std::string SplitKeyName(const std::string& code, const SplitLayout& layout) {
  auto own = layout.keys.find(code);
  if (own != layout.keys.end() && IsSplitKey(own->second)) {
    return own->second;
  }
  for (const auto& [split, place] : kSplitKeys) {
    if (code == place && layout.by_place.find(split) != std::string::npos) {
      return std::string(1, split);
    }
  }
  return std::string();
}

std::string ShortcutKeyName(const std::string& typed,
                            int windows_key_code,
                            const std::string& split_key,
                            const HeldKeys& held,
                            bool mac) {
  if (!split_key.empty()) {
    return split_key;
  }
  // Chrome's VKEYs for letters and digits are their ASCII capitals and digits.
  const bool letter = windows_key_code >= 'A' && windows_key_code <= 'Z';
  const bool digit = windows_key_code >= '0' && windows_key_code <= '9';
  const bool by_place =
      (held.alt && (letter || digit)) || (mac && held.control && held.shift && !held.meta && digit);
  return by_place ? std::string(1, static_cast<char>(windows_key_code)) : typed;
}

}  // namespace browser_core
