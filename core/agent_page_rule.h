// Which pages an agent may act on, apart so browser_core_unittests tests the rule alone.

#ifndef BROWSER_CORE_AGENT_PAGE_RULE_H_
#define BROWSER_CORE_AGENT_PAGE_RULE_H_

class GURL;

namespace browser_core {

inline constexpr char kNotAgentsTab[] = "The tab isn't in the agent's own Space.";
inline constexpr char kNotWebPage[] =
    "The tab's page isn't an http or https page, the only pages agents may use.";

// Why an agent may not act on a tab whose page is at |page|, whose address field shows |visible|
// and which is pinned at |pinned|, or nullptr when it may. A Board is the user's alone, so it's
// answered as a tab of theirs, whichever of the three it's at. Any other page that isn't http(s)
// is refused as the address field shows it, but not the empty page a tab starts on, which the
// agent's page action waits past. A tab not yet loaded is held to its pinned page.
const char* AgentPageRefusal(const GURL& page, const GURL& visible, const GURL& pinned);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_PAGE_RULE_H_
