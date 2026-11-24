#include "browser_core/agent_sign_in_rule.h"

#include <iterator>
#include <set>

#include "base/strings/string_split.h"
#include "base/strings/string_util.h"
#include "base/strings/stringprintf.h"
#include "net/base/registry_controlled_domains/registry_controlled_domain.h"
#include "url/gurl.h"
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

bool PostsWithin(const GURL& action, const url::Origin& page) {
  if (action.is_empty()) {
    return true;
  }
  if (!action.is_valid() || !action.SchemeIs(url::kHttpsScheme)) {
    return false;
  }
  return net::registry_controlled_domains::SameDomainOrHost(
      url::Origin::Create(action), page,
      net::registry_controlled_domains::INCLUDE_PRIVATE_REGISTRIES);
}

bool SendsElsewhere(const std::vector<std::pair<std::string, std::string>>& attributes) {
  for (const auto& [name, value] : attributes) {
    const std::string_view trimmed = base::TrimWhitespaceASCII(value, base::TRIM_ALL);
    if (base::EqualsCaseInsensitiveASCII(name, "formaction") && !trimmed.empty()) {
      return true;
    }
    if ((base::EqualsCaseInsensitiveASCII(name, "formtarget") ||
         base::EqualsCaseInsensitiveASCII(name, "target")) &&
        !trimmed.empty() && !base::EqualsCaseInsensitiveASCII(trimmed, "_self")) {
      return true;
    }
  }
  return false;
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

WithheldHistory::WithheldHistory() = default;
WithheldHistory::~WithheldHistory() = default;

void WithheldHistory::Withhold(const GURL& now, const std::set<int64_t>& pending) {
  addresses_.insert(now.GetWithoutRef());
  pending_.insert(pending.begin(), pending.end());
  shown_ = true;
}

void WithheldHistory::Ended(int64_t navigation) {
  pending_.erase(navigation);
}

bool WithheldHistory::Committed(int64_t navigation, const GURL& url, bool same_document) {
  const GURL address = url.GetWithoutRef();
  if (pending_.erase(navigation) || (same_document && shown_)) {
    addresses_.insert(address);
    shown_ = true;
  } else {
    shown_ = addresses_.contains(address);
  }
  return shown_;
}

bool WithheldHistory::Shows(const GURL& now) const {
  return shown_ || addresses_.contains(now.GetWithoutRef());
}

SignInTries::SignInTries() = default;
SignInTries::~SignInTries() = default;

void SignInTries::Failed(const std::string& agent, const url::Origin& origin) {
  failed_[agent].insert(origin);
}

bool SignInTries::MayTry(const std::string& agent, const url::Origin& origin) const {
  const auto it = failed_.find(agent);
  return it == failed_.end() || !it->second.contains(origin);
}

void SignInTries::Forgive(const url::Origin& origin) {
  for (auto it = failed_.begin(); it != failed_.end();) {
    it->second.erase(origin);
    it = it->second.empty() ? failed_.erase(it) : std::next(it);
  }
}

}  // namespace browser_core
