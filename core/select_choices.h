// A native select's choices as an agent's outline gives them (#2355), apart so
// browser_core_unittests tests the rule alone.

#ifndef BROWSER_CORE_SELECT_CHOICES_H_
#define BROWSER_CORE_SELECT_CHOICES_H_

#include <map>
#include <string>
#include <vector>

#include "base/values.h"

namespace browser_core {

// For each native select in |nodes|, a page's accessibility tree as DevTools'
// Accessibility.getFullAXTree gives it, by the select's node id: its options' names, in the
// page's order. They're the page's markup, as a custom list's options already show; which one is
// chosen is the user's, so it's never among them. Only a select's own popup's options count, never
// what's under another field, where what was typed or filled is.
std::map<std::string, std::vector<std::string>> SelectChoices(const base::ListValue& nodes);

}  // namespace browser_core

#endif  // BROWSER_CORE_SELECT_CHOICES_H_
