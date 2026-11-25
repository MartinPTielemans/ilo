#include "browser_core/agent_space_rule.h"

namespace browser_core::agent_space_rule {

bool Refuses(Ask ask, const Tab& tab) {
  if (!tab.agents) {
    return false;
  }
  switch (ask) {
    case Ask::kPaste:
    case Ask::kLinkPreview:
      return !tab.held;
    case Ask::kFilePicker:
    case Ask::kDevice:
      return true;
  }
  return true;
}

bool MenuHasLinkPreviewRow(bool web_link, const Tab& tab) {
  return web_link && !Refuses(Ask::kLinkPreview, tab);
}

}  // namespace browser_core::agent_space_rule
