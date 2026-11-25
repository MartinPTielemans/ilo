#include "browser_core/handed_give_rule.h"

namespace browser_core::handed_give_rule {

bool MayGive(const Tab& tab) {
  return tab.users && tab.to_agent && tab.same_profile && !tab.pinned && !tab.split &&
         tab.web_page && !tab.camera && !tab.microphone && !tab.screen && !tab.permission_asked &&
         !tab.dialog && !tab.saved_login && !tab.device_or_location && !tab.files;
}

}  // namespace browser_core::handed_give_rule
