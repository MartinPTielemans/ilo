#include "browser_core/agent_sign_in_rule.h"

#include <set>

#include "base/strings/string_split.h"
#include "base/strings/string_util.h"
#include "base/strings/stringprintf.h"
#include "url/origin.h"
#include "url/url_constants.h"

namespace browser_core {

std::vector<size_t> UsableLogins(const std::vector<SavedLogin>& logins, const url::Origin& page) {
  std::vector<size_t> usable;
  if (page.opaque() || page.scheme() != url::kHttpsScheme) {
    return usable;
  }
  const std::string realm = page.Serialize() + "/";
  std::set<std::u16string> seen;
  // The account's store first, so a username saved in both is the account's.
  for (bool account : {true, false}) {
    for (size_t i = 0; i < logins.size(); ++i) {
      const SavedLogin& login = logins[i];
      if (login.account != account || !login.exact || login.blocked || login.federated ||
          !login.html || !login.has_password || login.realm != realm) {
        continue;
      }
      if (seen.insert(login.username).second) {
        usable.push_back(i);
      }
    }
  }
  return usable;
}

std::optional<size_t> UsernameOnlyField(const std::vector<FormField>& fields) {
  std::optional<size_t> found;
  for (size_t i = 0; i < fields.size(); ++i) {
    const FormField& field = fields[i];
    if (field.kind == FormField::Kind::kPassword) {
      return std::nullopt;
    }
    if (field.kind != FormField::Kind::kText && field.kind != FormField::Kind::kEmail) {
      continue;
    }
    if (!field.visible || field.readonly) {
      continue;
    }
    bool names = false;
    for (const std::string& token :
         base::SplitString(base::ToLowerASCII(field.autocomplete), base::kWhitespaceASCII,
                           base::TRIM_WHITESPACE, base::SPLIT_WANT_NONEMPTY)) {
      names = names || token == "username" || token == "email";
    }
    if (!names) {
      continue;
    }
    if (found) {
      return std::nullopt;
    }
    found = i;
  }
  return found;
}

namespace {

// |value| as a CSS string, quoted.
std::string CssString(std::string_view value) {
  std::string quoted = "\"";
  for (const char each : value) {
    if (each == '"' || each == '\\') {
      quoted += '\\';
      quoted += each;
    } else if (static_cast<unsigned char>(each) < 0x20 || each == 0x7f) {
      // A line break or other control character, which a CSS string can't hold as it is.
      quoted += base::StringPrintf("\\%x ", static_cast<unsigned char>(each));
    } else {
      quoted += each;
    }
  }
  return quoted + "\"";
}

}  // namespace

std::optional<std::string> FieldSelector(bool password, std::string_view id, std::string_view name) {
  std::string selector = password ? "input[type=password]" : "input";
  if (!id.empty()) {
    return selector + "[id=" + CssString(id) + "]";
  }
  if (!name.empty()) {
    return selector + "[name=" + CssString(name) + "]";
  }
  if (password) {
    return selector;
  }
  return std::nullopt;
}

bool ShowsPassword(std::u16string_view text, std::u16string_view password) {
  if (password.empty()) {
    return false;
  }
  if (password.size() >= 6) {
    return text.find(password) != std::u16string_view::npos;
  }
  return base::TrimWhitespace(text, base::TRIM_ALL) == password;
}

}  // namespace browser_core
