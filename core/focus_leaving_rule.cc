#include "browser_core/focus_leaving_rule.h"

namespace browser_core::focus_leaving_rule {

Kept Leave(bool agent_tab, bool shown, base::FunctionRef<bool()> ask_app) {
  if (agent_tab) {
    return Kept::kAgentsPage;
  }
  if (!shown) {
    return Kept::kUnshown;
  }
  return ask_app() ? Kept::kByApp : Kept::kByPage;
}

const char* Name(Kept kept) {
  switch (kept) {
    case Kept::kByApp:
      return "app";
    case Kept::kByPage:
      return "page";
    case Kept::kAgentsPage:
      return "agent's page";
    case Kept::kUnshown:
      return "unshown page";
  }
  return "page";
}

}  // namespace browser_core::focus_leaving_rule
