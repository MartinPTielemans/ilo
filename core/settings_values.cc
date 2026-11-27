#include "browser_core/settings_values.h"

#include <set>
#include <utility>
#include <vector>

#include "base/strings/string_split.h"
#include "base/strings/string_util.h"
#include "browser_core/sync_settings.pb.h"

namespace browser_core::settings_values {
namespace {

constexpr char kAgentTabsCloseAfter[] = "agent_tabs_close_after";

// A BCP 47 tag as Chromium's accept-languages holds them: a language of two or three letters,
// then subtags of one to eight letters or digits.
bool IsTag(std::string_view tag) {
  const std::vector<std::string_view> parts =
      base::SplitStringPiece(tag, "-", base::KEEP_WHITESPACE, base::SPLIT_WANT_ALL);
  for (size_t i = 0; i < parts.size(); ++i) {
    const std::string_view part = parts[i];
    const size_t least = i == 0 ? 2 : 1;
    const size_t most = i == 0 ? 3 : 8;
    if (part.size() < least || part.size() > most) {
      return false;
    }
    for (char c : part) {
      if (i == 0 ? !base::IsAsciiAlpha(c) : !base::IsAsciiAlphaNumeric(c)) {
        return false;
      }
    }
  }
  return !parts.empty();
}

}  // namespace

std::optional<std::string> Languages(std::string_view text) {
  std::vector<std::string> tags;
  std::set<std::string> seen;
  for (std::string_view tag :
       base::SplitStringPiece(text, ",", base::TRIM_WHITESPACE, base::SPLIT_WANT_ALL)) {
    if (!IsTag(tag)) {
      return std::nullopt;
    }
    if (seen.insert(base::ToLowerASCII(tag)).second) {
      tags.emplace_back(tag);
    }
  }
  if (tags.empty()) {
    return std::nullopt;
  }
  return base::JoinString(tags, ",");
}

void Write(const Values& values, base::DictValue& settings) {
  settings.Set(kBlockThirdPartyCookies, values.block_third_party_cookies);
  settings.Set(kSpellCheck, values.spell_check);
  if (values.languages) {
    settings.Set(kLanguages, *values.languages);
  }
  settings.Set(kAgentTabsCloseAfter, values.agent_tabs_close_after);
}

Values Read(const base::DictValue& settings) {
  Values values;
  values.block_third_party_cookies =
      settings.FindBool(kBlockThirdPartyCookies).value_or(values.block_third_party_cookies);
  values.spell_check = settings.FindBool(kSpellCheck).value_or(values.spell_check);
  if (const std::string* languages = settings.FindString(kLanguages)) {
    values.languages = Languages(*languages);
  }
  if (std::optional<double> after = settings.FindDouble(kAgentTabsCloseAfter)) {
    values.agent_tabs_close_after = agent_tabs_close_rule::Choice(*after);
  }
  return values;
}

void ToEntities(const Values& values, std::map<std::string, sync_settings_pb::SettingEntity>& all) {
  const auto add = [&all](const char* name) -> sync_settings_pb::SettingEntity& {
    sync_settings_pb::SettingEntity& entity = all[name];
    entity.set_name(name);
    return entity;
  };
  add(kBlockThirdPartyCookies).set_on(values.block_third_party_cookies);
  add(kSpellCheck).set_on(values.spell_check);
  if (values.languages) {
    add(kLanguages).set_text(*values.languages);
  }
}

void FromEntity(const sync_settings_pb::SettingEntity& entity, Values& values) {
  if (entity.name() == kBlockThirdPartyCookies && entity.has_on()) {
    values.block_third_party_cookies = entity.on();
  }
  if (entity.name() == kSpellCheck && entity.has_on()) {
    values.spell_check = entity.on();
  }
  if (entity.name() == kLanguages && entity.has_text()) {
    if (std::optional<std::string> languages = Languages(entity.text())) {
      values.languages = std::move(languages);
    }
  }
}

}  // namespace browser_core::settings_values
