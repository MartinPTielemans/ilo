#include "browser_core/agent_space_page.h"

#include "browser_core/agent_session.h"
#include "browser_core/space.h"
#include "browser_core/system_engine.h"
#include "browser_core/tab.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"

namespace browser_core {

const Space* AgentSpaceOf(bc_id tab) {
  using agent_space_rule::Where;
  const Space* space = Space::Holding(tab);
  const Tab* preview = !space && system_engine::Previewing(tab) ? Tab::Get(tab) : nullptr;
  const Space* opened_from = preview ? Space::FromId(preview->preview_of_agents()) : nullptr;
  const Where where = !space           ? Where::kNoSpace
                      : space->agent() ? Where::kAgentsSpace
                                       : Where::kUsersSpace;
  if (!agent_space_rule::InAgentSpace(where, opened_from && opened_from->agent())) {
    return nullptr;
  }
  return space ? space : opened_from;
}

bool RefusesInAgentSpace(agent_space_rule::Ask ask, content::WebContents* contents) {
  const Tab* tab = contents ? Tab::FromWebContents(contents->GetOutermostWebContents()) : nullptr;
  return tab && agent_space_rule::Refuses(
                    ask, {.agents = AgentSpaceOf(tab->id()) != nullptr, .held = IsHeld(tab->id())});
}

bool RefusesInAgentSpace(agent_space_rule::Ask ask, content::RenderFrameHost* frame) {
  return RefusesInAgentSpace(ask, content::WebContents::FromRenderFrameHost(frame));
}

}  // namespace browser_core
