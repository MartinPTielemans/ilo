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
      return true;
  }
  return true;
}

}  // namespace browser_core::agent_space_rule
