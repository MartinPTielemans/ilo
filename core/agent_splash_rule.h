// Whether a page's trees show only a splash, apart so browser_core_unittests tests the rule alone
// (#3658).

#ifndef BROWSER_CORE_AGENT_SPLASH_RULE_H_
#define BROWSER_CORE_AGENT_SPLASH_RULE_H_

#include <vector>

#include "base/values.h"

namespace browser_core {

// Whether |trees|, a page's accessibility trees as DevTools gives them, show only a splash: a node
// says the page is loading, being marked busy or a progress bar with no value, and nothing in any
// tree takes focus but a document. Its content is then still to come, as an app's in a frame is
// after the page's own load.
bool IsSplash(const std::vector<base::ListValue>& trees);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_SPLASH_RULE_H_
