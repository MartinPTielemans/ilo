// What a held tab's page has that's the user's to do, from what the browser already read of it:
// the fields its forms have and its accessibility tree's names. Apart from content so
// browser_core_unittests tests the rule alone (#4361).

#ifndef BROWSER_CORE_HELD_PAGE_RULE_H_
#define BROWSER_CORE_HELD_PAGE_RULE_H_

#include <string_view>
#include <vector>

namespace ui {
struct AXTreeUpdate;
}  // namespace ui

namespace browser_core::held_page_rule {

// What the page has, as bc_held_page says it.
struct Facts {
  bool password = false;
  bool one_time_code = false;
  bool captcha = false;

  friend bool operator==(const Facts&, const Facts&) = default;
};

// A field of a form the page has, as the password manager or autofill parsed it.
struct Field {
  // Its control is <input type="password">.
  bool password = false;
  // The user can reach it: a hidden one, such as a sign-in's first step keeps for the password
  // manager, is no step of theirs.
  bool focusable = false;
  // Autofill parsed its autocomplete as "one-time-code".
  bool one_time_code = false;
};

// What |fields| say of the page: a password field the user can reach, or a one-time-code field.
Facts FactsOf(const std::vector<Field>& fields);

// Whether an element's |name| holds one of |words|, a list of lowercase words each ending in a
// newline, as BC_CAPTCHA_WORDS is, in any case.
bool HoldsWord(std::string_view name, std::string_view words);

// Whether an element in |update| that isn't ignored or hidden has a name of its own, not its
// text, holding one of |words|: a frame's title, a document's or a label's. Text a page shows,
// such as "This site is protected by reCAPTCHA" under a form, is none.
bool CaptchaIn(const ui::AXTreeUpdate& update, std::string_view words);

// |known| with what |more| turns true, and whether it turned any: a fact the page had stays, as a
// later form or frame adds to what its load had.
bool Add(Facts& known, const Facts& more);

}  // namespace browser_core::held_page_rule

#endif  // BROWSER_CORE_HELD_PAGE_RULE_H_
