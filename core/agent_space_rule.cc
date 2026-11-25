#include "browser_core/agent_space_rule.h"

namespace browser_core::agent_space_rule {

bool Refuses(Ask ask, const Tab& tab) {
  if (!tab.agents) {
    return false;
  }
  switch (ask) {
    case Ask::kPaste:
      return !tab.held;
    case Ask::kFilePicker:
    case Ask::kDevice:
      return true;
  }
  return true;
}

bool InAgentSpace(Where where, bool preview_of_agents) {
  return where == Where::kAgentsSpace || (where == Where::kNoSpace && preview_of_agents);
}

}  // namespace browser_core::agent_space_rule
