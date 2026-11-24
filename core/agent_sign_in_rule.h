// Which saved logins and which fields an agent's sign-in uses (#4098), and whether a page shows
// the password it filled, apart so browser_core_unittests tests the rules alone.

#ifndef BROWSER_CORE_AGENT_SIGN_IN_RULE_H_
#define BROWSER_CORE_AGENT_SIGN_IN_RULE_H_

#include <cstddef>
#include <cstdint>
#include <map>
#include <optional>
#include <set>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "base/values.h"
#include "url/gurl.h"
#include "url/origin.h"

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

// Whether a form on a page of |page| whose action is |action| sends its fields within the page's
// site: none in the form's markup (|action_empty|, as autofill reads it), or an https one on the
// same site. An action that didn't parse is neither. A page can carry another site's form, as
// markup its users post, and the core never fills and submits a login into it.
bool PostsWithin(const GURL& action, bool action_empty, const url::Origin& page);

// Every control or element that can send a form somewhere its action doesn't say: a button's or an
// image input's formaction or formtarget, and a form's or the document's target.
inline constexpr char kSubmitters[] = "[formaction], [formtarget], form[target], base[target]";

// Whether an element kSubmitters found, by its attributes as name and value pairs, sends a form
// elsewhere: any formaction that isn't empty, or a formtarget or target naming another browsing
// context than the page's own. One in the main frame's document makes the whole sign-in there the
// user's step, stricter than its form alone, since a page can tie any button to any form.
bool SendsElsewhere(const std::vector<std::pair<std::string, std::string>>& attributes);

// Whether |text|, something an agent could read of a page, shows |password|. One of six characters
// or more shows in any text holding it. A shorter one, which ordinary words hold by chance, shows
// only as all of a text, spaces around it aside.
bool ShowsPassword(std::u16string_view text, std::u16string_view password);

// A frame's accessibility tree for the look after a fill, as Accessibility.getFullAXTree gives its
// nodes, without the agent's own typing: a field whose value is still exactly what the agent's last
// type left in it, by its DOM node's id in |typed|, loses its value and the nodes under it, where
// Chromium puts that text again. The agent knows that text, so it tells the look nothing; anything
// else in the field, even a letter the page changed, is looked at.
base::ListValue WithoutOwnTyping(base::ListValue nodes, const std::map<int, std::u16string>& typed);

// A tab's pages withheld from agents as they showed what a sign-in filled, for the tab's life: the
// page held then, the commit of any navigation under way as it was, and later any page at one of
// their addresses, which history, a reload or a redirect can bring back, a form sent with GET
// holding the password in its address. Addresses without their fragment, in memory only.
class WithheldHistory {
 public:
  WithheldHistory();
  WithheldHistory(const WithheldHistory&) = delete;
  WithheldHistory& operator=(const WithheldHistory&) = delete;
  ~WithheldHistory();

  // The page at |now| is withheld, and so is what each of |pending|, the main frame's navigations
  // under way, commits.
  void Withhold(const GURL& now, const std::set<int64_t>& pending);
  // A main frame's navigation ended without committing.
  void Ended(int64_t navigation);
  // A main frame's navigation committed |url|. Returns whether the page shown now is withheld: it
  // is one of the pending ones', or it's the withheld document still, or its address is one.
  bool Committed(int64_t navigation, const GURL& url, bool same_document);
  // Whether the tab showing |now| shows a withheld page, before Committed hears of its commit too:
  // other observers of the same commit may ask first.
  bool Shows(const GURL& now) const;

 private:
  std::set<GURL> addresses_;
  std::set<int64_t> pending_;
  bool shown_ = false;
};

// Each tab's WithheldHistory, kept by tab rather than by page as LookFirstTabs keeps its mark, so a
// tab that sleeps and wakes with a new page still withholds what it did, until it closes.
class WithheldTabs {
 public:
  WithheldTabs();
  WithheldTabs(const WithheldTabs&) = delete;
  WithheldTabs& operator=(const WithheldTabs&) = delete;
  ~WithheldTabs();

  // |tab|'s, made the first time something in it is withheld.
  WithheldHistory& Of(uint64_t tab);
  // |tab|'s, if anything in it was withheld.
  WithheldHistory* Find(uint64_t tab);
  // The tab closed.
  void Forget(uint64_t tab);

 private:
  std::map<uint64_t, WithheldHistory> tabs_;
};

// The tabs whose titles and addresses wait for their next look (AgentPage::LookFirst), kept by
// tab rather than by page, so a tab that sleeps and wakes with a new page still waits. A look
// clears a mark only if it started after it: one already out when the user took the tab was
// asked before what they did there.
class LookFirstTabs {
 public:
  LookFirstTabs();
  LookFirstTabs(const LookFirstTabs&) = delete;
  LookFirstTabs& operator=(const LookFirstTabs&) = delete;
  ~LookFirstTabs();

  void Mark(uint64_t tab);
  // A look starts, on any tab: its number, which its answer gives back.
  uint64_t Started();
  // Look |look| on |tab| answered.
  void Answered(uint64_t tab, uint64_t look);
  bool Marked(uint64_t tab) const;
  // The tab closed.
  void Forget(uint64_t tab);

 private:
  // Each marked tab, with the first look that clears it.
  std::map<uint64_t, uint64_t> marks_;
  uint64_t next_look_ = 1;
};

// The origins where an agent's sign-in submitted a login and didn't sign in, by agent, in memory
// until the user hands a tab at the origin back or the browser restarts: another sign-in there is
// the user's step, so an agent never tries a login again into a site's lock-out, and no agent can
// clear it by ending its sessions.
class SignInTries {
 public:
  SignInTries();
  SignInTries(const SignInTries&) = delete;
  SignInTries& operator=(const SignInTries&) = delete;
  ~SignInTries();

  void Failed(const std::string& agent, const url::Origin& origin);
  bool MayTry(const std::string& agent, const url::Origin& origin) const;
  // The user took a tab at |origin| back from an agent: every agent may try there again.
  void Forgive(const url::Origin& origin);

 private:
  std::map<std::string, std::set<url::Origin>> failed_;
};

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_SIGN_IN_RULE_H_
