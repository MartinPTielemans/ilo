#include "browser_core/boost_script.h"

#include "base/containers/span.h"
#include "base/files/file_util.h"
#include "base/files/important_file_writer.h"
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

// Beside the profile's Boosts folder, not in it, so a copy of the folder never brings one.
constexpr base::FilePath::CharType kCopies[] = FILE_PATH_LITERAL("Boost Scripts Turned On");

std::string Hash(std::string_view script) {
  return base::ToLowerASCII(base::HexEncode(crypto::hash::Sha256(base::as_byte_span(script))));
}

// A Boost's site as Boosts::SiteOf makes it from a page's host, which can name a file: a site
// that's anything else has no record, so its script never runs.
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
  const std::optional<std::string> recorded = Recorded(prefs, site);
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

std::optional<std::string> Recorded(const PrefService& prefs, std::string_view site) {
  if (!IsSite(site)) {
    return std::nullopt;
  }
  const std::string* hash = prefs.GetDict(kRecords).FindString(site);
  return hash ? std::optional<std::string>(*hash) : std::nullopt;
}

base::FilePath CopyPath(const base::FilePath& profile_dir, std::string_view site) {
  if (!IsSite(site)) {
    return base::FilePath();
  }
  return profile_dir.Append(kCopies).AppendASCII(std::string(site) + ".js");
}

std::optional<std::string> LastTurnedOn(const base::FilePath& path, std::string_view recorded) {
  std::string text;
  if (path.empty() || !base::ReadFileToStringWithMaxSize(path, &text, kMaxBytes) ||
      Hash(text) != recorded) {
    return std::nullopt;
  }
  return text;
}

bool KeepCopy(const base::FilePath& path, std::optional<std::string_view> script) {
  if (path.empty()) {
    return false;
  }
  if (!script) {
    return base::DeleteFile(path);
  }
  return base::CreateDirectory(path.DirName()) &&
         base::ImportantFileWriter::WriteFileAtomically(path, *script);
}

}  // namespace browser_core::boost_script
