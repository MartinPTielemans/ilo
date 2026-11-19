// Who answers as Tab leaves a page past its last control, or ⇧Tab before its first, apart so
// browser_core_unittests tests the rule alone (#3220).

#ifndef BROWSER_CORE_FOCUS_LEAVING_RULE_H_
#define BROWSER_CORE_FOCUS_LEAVING_RULE_H_

#include "base/functional/function_ref.h"

namespace browser_core::focus_leaving_rule {

// What became of the keyboard.
enum class Kept {
  kByApp,       // The app took it, as onto the shown tab's row.
  kByPage,      // The user's page kept it: the app didn't take it.
  kAgentsPage,  // An agent's tab kept it, and the app wasn't asked.
  kUnshown,     // A tab no window shows kept it, and the app wasn't asked.
};

// Tab left a tab's page at one end. Only a tab of the user's that a window shows asks the app,
// through |ask_app|, which returns whether it took the keyboard. Chromium never moves it here:
// its own answer moves it to the window's toolbar, which ilo doesn't draw, or on the Mac to the
// window's next key view, which could be the user's from an agent's page. A page that keeps it
// goes on from its other end at the next Tab.
Kept Leave(bool agent_tab, bool shown, base::FunctionRef<bool()> ask_app);

// The word the core's log line uses for |kept|.
const char* Name(Kept kept);

// The user's Tab or ⇧Tab from the app's ring into the page a window shows, as
// bc_window_focus_page_end: |give| gives that page the keyboard at an end, moving its focused
// element. Not an agent's tab the user doesn't hold, whose page keeps its focus as the agent left
// it: a click there gives it the keyboard only as the same press takes the tab over from the agent
// (the app's page press), which a key from the ring doesn't. Returns whether it gave it.
bool Enter(bool agent_tab, base::FunctionRef<void()> give);

}  // namespace browser_core::focus_leaving_rule

#endif  // BROWSER_CORE_FOCUS_LEAVING_RULE_H_
