#include "browser_core/arrangement.h"

#include <cmath>
#include <initializer_list>
#include <utility>

#include "base/containers/span.h"
#include "base/json/json_reader.h"
#include "base/json/json_writer.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/string_util.h"
#include "components/pref_registry/pref_registry_syncable.h"
#include "components/prefs/pref_service.h"
#include "components/prefs/scoped_user_pref_update.h"
#include "crypto/hash.h"

namespace browser_core::arrangement {

namespace {

// Each site's record: the SHA-256 of the arrangement turned on, in lowercase hex.
constexpr char kRecords[] = "browser_core.arrangements";

// The file, its list of changes, a change, a neighbour, its target and its fingerprint: deeper is
// never the schema. JSONReader's depth counts the top level once more, so six containers take 7.
constexpr size_t kMaxDepth = 7;
// Further than any page is long or wide, in CSS pixels.
constexpr double kMaxPlace = 1e6;

std::string Hash(std::string_view bytes) {
  return base::ToLowerASCII(base::HexEncode(crypto::hash::Sha256(base::as_byte_span(bytes))));
}

// A Boost's site as Boosts::SiteOf makes it from a page's host, which names the app's files.
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

// Whether |dict| has no key but |keys|: a field the schema doesn't name refuses the file, so
// nothing unchecked rides along to the page.
bool OnlyKeys(const base::DictValue& dict, std::initializer_list<std::string_view> keys) {
  for (const auto [key, value] : dict) {
    bool known = false;
    for (std::string_view each : keys) {
      known = known || key == each;
    }
    if (!known) {
      return false;
    }
  }
  return true;
}

// Copies the string |key| of |from| to |to|. False if it's missing and |required|, or isn't a
// string within the limit.
bool CopyString(const base::DictValue& from,
                std::string_view key,
                base::DictValue& to,
                bool required) {
  const base::Value* value = from.Find(key);
  if (!value) {
    return !required;
  }
  if (!value->is_string() || value->GetString().size() > kMaxString) {
    return false;
  }
  to.Set(key, value->GetString());
  return true;
}

std::optional<double> Number(const base::DictValue& from, std::string_view key) {
  const base::Value* value = from.Find(key);
  if (!value || !(value->is_int() || value->is_double())) {
    return std::nullopt;
  }
  const double number = value->GetDouble();
  return std::isfinite(number) ? std::optional<double>(number) : std::nullopt;
}

std::optional<base::DictValue> Fingerprint(const base::Value* value) {
  if (!value || !value->is_dict()) {
    return std::nullopt;
  }
  const base::DictValue& from = value->GetDict();
  if (!OnlyKeys(from, {"tag", "id", "role", "label", "name", "href", "src", "landmark", "text",
                       "nth"})) {
    return std::nullopt;
  }
  // A tag name, as getElementsByTagName takes it.
  const std::string* tag = from.FindString("tag");
  if (!tag || tag->empty() || tag->size() > 64) {
    return std::nullopt;
  }
  for (char c : *tag) {
    if (!base::IsAsciiLower(c) && !base::IsAsciiDigit(c) && c != '-') {
      return std::nullopt;
    }
  }
  base::DictValue fp;
  fp.Set("tag", *tag);
  for (std::string_view key : {"id", "role", "label", "name", "href", "src", "landmark", "text"}) {
    if (!CopyString(from, key, fp, /*required=*/false)) {
      return std::nullopt;
    }
  }
  if (from.Find("nth")) {
    const std::optional<int> nth = from.FindInt("nth");
    if (!nth || *nth < 0 || *nth >= 10000) {
      return std::nullopt;
    }
    fp.Set("nth", *nth);
  }
  return fp;
}

// "target": a path, a fingerprint or both, each of which must find exactly one element.
std::optional<base::DictValue> Target(const base::Value* value) {
  if (!value || !value->is_dict()) {
    return std::nullopt;
  }
  const base::DictValue& from = value->GetDict();
  if (!OnlyKeys(from, {"path", "fp"}) || (!from.Find("path") && !from.Find("fp"))) {
    return std::nullopt;
  }
  base::DictValue target;
  if (!CopyString(from, "path", target, /*required=*/false) ||
      (target.FindString("path") && target.FindString("path")->empty())) {
    return std::nullopt;
  }
  if (from.Find("fp")) {
    std::optional<base::DictValue> fp = Fingerprint(from.Find("fp"));
    if (!fp) {
      return std::nullopt;
    }
    target.Set("fp", std::move(*fp));
  }
  return target;
}

// An element's "name", "role" and "target" from |from| into |to|.
bool Element(const base::DictValue& from, base::DictValue& to) {
  if (!CopyString(from, "name", to, /*required=*/true) ||
      !CopyString(from, "role", to, /*required=*/true)) {
    return false;
  }
  std::optional<base::DictValue> target = Target(from.Find("target"));
  if (!target) {
    return false;
  }
  to.Set("target", std::move(*target));
  return true;
}

// A settle's or a trade's other element.
std::optional<base::DictValue> Neighbour(const base::Value* value) {
  if (!value || !value->is_dict() || !OnlyKeys(value->GetDict(), {"name", "role", "target"})) {
    return std::nullopt;
  }
  base::DictValue neighbour;
  if (!Element(value->GetDict(), neighbour)) {
    return std::nullopt;
  }
  return neighbour;
}

std::optional<base::DictValue> Change(const base::Value& value) {
  if (!value.is_dict()) {
    return std::nullopt;
  }
  const base::DictValue& from = value.GetDict();
  const std::string* kind = from.FindString("kind");
  if (!kind) {
    return std::nullopt;
  }
  base::DictValue change;
  change.Set("kind", *kind);
  if (*kind == "settle") {
    // Placed against one neighbour, before or after it, never both.
    const bool before = from.Find("before");
    if (before == static_cast<bool>(from.Find("after")) ||
        !OnlyKeys(from, {"kind", "name", "role", "target", before ? "before" : "after"})) {
      return std::nullopt;
    }
    std::optional<base::DictValue> neighbour = Neighbour(from.Find(before ? "before" : "after"));
    if (!neighbour) {
      return std::nullopt;
    }
    change.Set(before ? "before" : "after", std::move(*neighbour));
  } else if (*kind == "trade") {
    std::optional<base::DictValue> other = Neighbour(from.Find("with"));
    if (!OnlyKeys(from, {"kind", "name", "role", "target", "with"}) || !other) {
      return std::nullopt;
    }
    change.Set("with", std::move(*other));
  } else if (*kind == "float") {
    if (!OnlyKeys(from, {"kind", "name", "role", "target", "at", "pin", "width"})) {
      return std::nullopt;
    }
    const base::DictValue* at = from.FindDict("at");
    if (!at || !OnlyKeys(*at, {"x", "y"})) {
      return std::nullopt;
    }
    const std::optional<double> x = Number(*at, "x");
    const std::optional<double> y = Number(*at, "y");
    if (!x || !y || std::abs(*x) > kMaxPlace || std::abs(*y) > kMaxPlace) {
      return std::nullopt;
    }
    change.Set("at", base::DictValue().Set("x", *x).Set("y", *y));
    const std::string* pin = from.FindString("pin");
    if (!pin || (*pin != "page" && *pin != "screen")) {
      return std::nullopt;
    }
    change.Set("pin", *pin);
    if (from.Find("width")) {
      const std::optional<double> width = Number(from, "width");
      if (!width || *width < 1 || *width > kMaxPlace) {
        return std::nullopt;
      }
      change.Set("width", *width);
    }
  } else {
    return std::nullopt;
  }
  if (!Element(from, change)) {
    return std::nullopt;
  }
  return change;
}

}  // namespace

std::optional<base::DictValue> Parse(std::string_view bytes) {
  if (bytes.size() > kMaxBytes) {
    return std::nullopt;
  }
  std::optional<base::DictValue> file =
      base::JSONReader::ReadDict(bytes, base::JSON_PARSE_RFC, kMaxDepth);
  if (!file || !OnlyKeys(*file, {"version", "changes"}) || file->FindInt("version") != 1) {
    return std::nullopt;
  }
  const base::ListValue* changes = file->FindList("changes");
  if (!changes || changes->size() > kMaxChanges) {
    return std::nullopt;
  }
  base::ListValue checked;
  for (const base::Value& each : *changes) {
    std::optional<base::DictValue> change = Change(each);
    if (!change) {
      return std::nullopt;
    }
    checked.Append(std::move(*change));
  }
  return base::DictValue().Set("version", 1).Set("changes", std::move(checked));
}

std::string Write(const base::DictValue& arrangement) {
  return base::WriteJson(arrangement).value_or(std::string());
}

void RegisterPrefs(user_prefs::PrefRegistrySyncable* registry) {
  // No sync flag: the record never leaves this computer.
  registry->RegisterDictionaryPref(kRecords);
}

State Check(const PrefService& prefs, std::string_view site, std::string_view bytes) {
  if (!Parse(bytes)) {
    return State::kRefused;
  }
  const std::string* recorded = IsSite(site) ? prefs.GetDict(kRecords).FindString(site) : nullptr;
  if (!recorded) {
    return State::kNew;
  }
  return *recorded == Hash(bytes) ? State::kOn : State::kChanged;
}

bool TurnOn(PrefService& prefs, std::string_view site, std::string_view bytes) {
  if (!IsSite(site) || !Parse(bytes)) {
    return false;
  }
  ScopedDictPrefUpdate(&prefs, kRecords)->Set(site, Hash(bytes));
  return true;
}

void Forget(PrefService& prefs, std::string_view site) {
  ScopedDictPrefUpdate(&prefs, kRecords)->Remove(site);
}

double Within(double value, double low, double high, double fallback) {
  return value >= low && value <= high ? value : fallback;
}

}  // namespace browser_core::arrangement
