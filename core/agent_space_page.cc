#include "browser_core/agent_space_page.h"

#include "browser_core/agent_session.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"

namespace browser_core {

bool RefusesInAgentSpace(agent_space_rule::Ask ask, content::WebContents* contents) {
  const Tab* tab = contents ? Tab::FromWebContents(contents->GetOutermostWebContents()) : nullptr;
  if (!tab) {
    return false;
  }
  const Space* space = Space::Holding(tab->id());
  return agent_space_rule::Refuses(ask,
                                   {.agents = space && space->agent(), .held = IsHeld(tab->id())});
}

bool RefusesInAgentSpace(agent_space_rule::Ask ask, content::RenderFrameHost* frame) {
  return RefusesInAgentSpace(ask, content::WebContents::FromRenderFrameHost(frame));
}

}  // namespace browser_core
