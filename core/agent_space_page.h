#ifndef BROWSER_CORE_AGENT_SPACE_PAGE_H_
#define BROWSER_CORE_AGENT_SPACE_PAGE_H_

#include "browser_core/agent_space_rule.h"

namespace content {
class RenderFrameHost;
class WebContents;
}  // namespace content

namespace browser_core {

// Whether the browser refuses |ask| from a page of |contents|, or from |frame|'s page: the rule's
// answer (agent_space_rule.h) for the tab it's in, a page inside another's being in its tab. Never
// for a page in no tab.
bool RefusesInAgentSpace(agent_space_rule::Ask ask, content::WebContents* contents);
bool RefusesInAgentSpace(agent_space_rule::Ask ask, content::RenderFrameHost* frame);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_SPACE_PAGE_H_
