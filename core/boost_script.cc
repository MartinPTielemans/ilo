#include "browser_core/boost_script.h"

#include "base/containers/span.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/string_util.h"
#include "base/values.h"
#include "components/pref_registry/pref_registry_syncable.h"
#include "components/prefs/pref_service.h"
#include "components/prefs/scoped_user_pref_update.h"
#include "crypto/hash.h"

namespace browser_core::boost_script {

namespace {

// Each site's record: the SHA-256 of the script turned on, in lowercase hex.
constexpr char kRecords[] = "browser_core.boost_scripts";

std::string Hash(std::string_view script) {
  return base::ToLowerASCII(base::HexEncode(crypto::hash::Sha256(base::as_byte_span(script))));
}

// A Boost's site as Boosts::SiteOf makes it from a page's host, which names the app's files: a
// site that's anything else has no record, so its script never runs.
bool IsSite(std::string_view site) {
  if (site.empty() || site.size() > 253 || site.front() == '.' || site.back() == '.' ||
      site.find("..") != std::string_view::npos) {
    return false;
  }
  for (char c : site) {
    if (!base::IsAsciiLower(c) && !base::IsAsciiDigit(c) && c != '.' && c != '-') {
      return false;
    }
  }
  return true;
}

}  // namespace

void RegisterPrefs(user_prefs::PrefRegistrySyncable* registry) {
  // No sync flag: the record never leaves this computer.
  registry->RegisterDictionaryPref(kRecords);
}

State Check(const PrefService& prefs, std::string_view site, std::string_view script) {
  if (script.size() > kMaxBytes) {
    return State::kTooLarge;
  }
  const std::string* recorded = IsSite(site) ? prefs.GetDict(kRecords).FindString(site) : nullptr;
  if (!recorded) {
    return State::kNew;
  }
  return *recorded == Hash(script) ? State::kOn : State::kChanged;
}

bool TurnOn(PrefService& prefs, std::string_view site, std::string_view script) {
  if (script.size() > kMaxBytes || !IsSite(site)) {
    return false;
  }
  ScopedDictPrefUpdate(&prefs, kRecords)->Set(site, Hash(script));
  return true;
}

void Forget(PrefService& prefs, std::string_view site) {
  ScopedDictPrefUpdate(&prefs, kRecords)->Remove(site);
}

}  // namespace browser_core::boost_script
