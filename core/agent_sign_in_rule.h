// Which saved logins and which fields an agent's sign-in uses (#4098), and whether a page shows
// the password it filled, apart so browser_core_unittests tests the rules alone.

#ifndef BROWSER_CORE_AGENT_SIGN_IN_RULE_H_
#define BROWSER_CORE_AGENT_SIGN_IN_RULE_H_

#include <cstddef>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace url {
class Origin;
}

namespace browser_core {

// What the rule needs of a saved login. It holds no password: the caller keeps those, by index.
struct SavedLogin {
  // The store matched it to the page exactly, not by the public suffix, an affiliation or a group.
  bool exact = false;
  // The user said never to save for the site.
  bool blocked = false;
  // A login through another site, which holds no password here.
  bool federated = false;
  // An HTML form's, not HTTP auth's.
  bool html = false;
  bool has_password = false;
  // In the account's store, not the profile's.
  bool account = false;
  std::string realm;
  std::u16string username;
};

// The indices of the logins an agent's sign-in may fill on a page of |page|, account store first:
// an https page's own exact HTML logins with a password, never blocked or federated, whose realm is
// the page's origin, so another subdomain's, an affiliated app's and a public-suffix match's never
// come. Two with the same username are one, the account's.
std::vector<size_t> UsableLogins(const std::vector<SavedLogin>& logins, const url::Origin& page);

// What the rule needs of a field in a form.
struct FormField {
  enum class Kind { kText, kEmail, kPassword, kOther };
  Kind kind = Kind::kOther;
  std::string autocomplete;
  bool visible = false;
  bool readonly = false;
};

// A form asking for the username alone, the first page of a username-first sign-in: the index of its
// one visible, editable text or email field whose autocomplete names a username or an email, in a
// form with no password field at all, hidden ones included. Anything else is no such form.
std::optional<size_t> UsernameOnlyField(const std::vector<FormField>& fields);

// The CSS selector that finds the field the core fills last, in the main frame's document alone,
// outside any shadow root, from its type and its id or name: `input[type=password]` for the
// password, with `[id="…"]`, or `[name="…"]` without an id. None for a username field with neither,
// which no selector can tell apart.
std::optional<std::string> FieldSelector(bool password, std::string_view id, std::string_view name);

// Whether |text|, something an agent could read of a page, shows |password|. One of six characters
// or more shows in any text holding it. A shorter one, which ordinary words hold by chance, shows
// only as all of a text, spaces around it aside.
bool ShowsPassword(std::u16string_view text, std::u16string_view password);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_SIGN_IN_RULE_H_
