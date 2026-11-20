#ifndef BROWSER_CORE_TAB_FOLLOW_RULE_H_
#define BROWSER_CORE_TAB_FOLLOW_RULE_H_

#include "ui/base/page_transition_types.h"
#include "url/gurl.h"

// What a tab loads when the user picks an address from what its site gave (bc_tab_follow), and an
// agent's load once its own rules allow it (bc_agent_navigate), apart so browser_core_unittests
// tests it alone (#3576).
namespace browser_core::tab_follow {

// A link, not typed: the address came from a site's data, so it doesn't count toward what the
// address bar suggests as typed, and the tab keeps where it was opened from.
inline constexpr ui::PageTransition kTransition = ui::PAGE_TRANSITION_LINK;

// Whether |url| may load this way: http(s) alone, never javascript:, file:, data: or the browser's
// own pages.
bool MayLoad(const GURL& url);

}  // namespace browser_core::tab_follow

#endif  // BROWSER_CORE_TAB_FOLLOW_RULE_H_
