// Settings' values over Chromium's own preferences (third-party cookies, spell check, languages)
// and Agents › Close a thread's tabs: their defaults, how the session file keeps them and which
// follow the account, apart so browser_core_unittests tests them alone.

#ifndef BROWSER_CORE_SETTINGS_VALUES_H_
#define BROWSER_CORE_SETTINGS_VALUES_H_

#include <map>
#include <optional>
#include <string>
#include <string_view>

#include "base/values.h"
#include "browser_core/agent_tabs_close_rule.h"

namespace browser_core {

namespace sync_settings_pb {
class SettingEntity;
}

namespace settings_values {

struct Values {
  // Block All, else Allow, which is Chromium's own: blocked in private windows only.
  bool block_third_party_cookies = false;
  // Chromium's own default.
  bool spell_check = true;
  // The languages in order, as Languages gives them, or none until the user chooses, which
  // leaves each profile's as Chromium has it.
  std::optional<std::string> languages;
  // An agent_tabs_close_rule choice, in seconds.
  double agent_tabs_close_after = agent_tabs_close_rule::kDefault;
};

// |text|'s BCP 47 tags joined by commas in its order, without spaces or repeats (the first
// kept, whatever its case), or none for an empty list or one with a tag that isn't one.
std::optional<std::string> Languages(std::string_view text);

// Writes |values| into the session file's settings, and reads them back, each missing or
// unreadable one as its default.
void Write(const Values& values, base::DictValue& settings);
Values Read(const base::DictValue& settings);

// The entities of the values that follow the account, by name, into |all|: cookies, spell check
// and, once the user has chosen them, languages. Close a thread's tabs stays with the computer.
void ToEntities(const Values& values, std::map<std::string, sync_settings_pb::SettingEntity>& all);
// Takes |entity|'s value into |values| if it's one of theirs, with a value it takes. An entity
// without its field, as an older version writes, changes nothing.
void FromEntity(const sync_settings_pb::SettingEntity& entity, Values& values);

inline constexpr char kBlockThirdPartyCookies[] = "block_third_party_cookies";
inline constexpr char kSpellCheck[] = "spell_check";
inline constexpr char kLanguages[] = "languages";

}  // namespace settings_values
}  // namespace browser_core

#endif  // BROWSER_CORE_SETTINGS_VALUES_H_
