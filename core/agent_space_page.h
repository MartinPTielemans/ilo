#ifndef BROWSER_CORE_AGENT_SPACE_PAGE_H_
#define BROWSER_CORE_AGENT_SPACE_PAGE_H_

#include "browser_core/agent_space_rule.h"
#include "browser_core/public/browser_core.h"

namespace content {
class RenderFrameHost;
class WebContents;
}  // namespace content

namespace browser_core {

class Space;

// The agent's Space the page of |tab| is in, if it's in one: the one answer to whether a page is an
// agent's. A page a page opens is its opener's until it's listed: its opener's Space holds its tab
// from its making (Space::Expect), before the opener's script has the window, so nothing that
// becomes of the opener changes it. A link preview, which no Space holds, is its opener's too
// (Tab::preview_of_agents). Once a Space lists the tab, that Space answers.
const Space* AgentSpaceOf(bc_id tab);

// Whether the browser refuses |ask| from a page of |contents|, or from |frame|'s page: the rule's
// answer (agent_space_rule.h) for the tab it's in, a page inside another's being in its tab. A
// page with no tab is no agent's: one an agent's page makes has its tab from its making.
bool RefusesInAgentSpace(agent_space_rule::Ask ask, content::WebContents* contents);
bool RefusesInAgentSpace(agent_space_rule::Ask ask, content::RenderFrameHost* frame);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_SPACE_PAGE_H_
