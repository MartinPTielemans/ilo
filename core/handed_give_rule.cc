#include "browser_core/handed_give_rule.h"

namespace browser_core::handed_give_rule {

bool MayGive(const Tab& tab) {
  return tab.users && tab.to_agent && tab.same_profile && !tab.live_folder && !tab.favorite_split &&
         !tab.stands_in && tab.web_page && !tab.screen && !tab.permission_asked && !tab.dialog &&
         !tab.saved_login && !tab.device && !tab.files;
}

}  // namespace browser_core::handed_give_rule
