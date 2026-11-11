// When a tab may arrange, and how giving one to an agent keeps an agent's Space free of it, apart
// so browser_core_unittests tests the rule alone. Together they're why an agent's call needs no
// Arrange rule of its own: no tab an agent may act on is arranging.

#ifndef BROWSER_CORE_ARRANGE_RULE_H_
#define BROWSER_CORE_ARRANGE_RULE_H_

#include <string_view>

#include "base/functional/function_ref.h"

namespace browser_core::arrange_rule {

// Whether a tab may start arranging. Arrange is the user's alone: only a web page, whose |site| is
// known, never in a private window or a tab an agent's Space holds.
bool MayStart(std::string_view site, bool off_the_record, bool in_agent_space);

// Gives a tab to an agent: |end_arrange| first, so it never reaches the agent's Space arranging,
// then |move|, whose answer it returns.
bool GiveToAgent(base::FunctionRef<void()> end_arrange, base::FunctionRef<bool()> move);

}  // namespace browser_core::arrange_rule

#endif  // BROWSER_CORE_ARRANGE_RULE_H_
