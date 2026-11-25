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
    case Ask::kPictureInPicture:
      return true;
  }
  return true;
}

bool MenuHasLinkPreviewRow(bool web_link, const Tab& tab) {
  return web_link && !Refuses(Ask::kLinkPreview, tab);
}

Listed ListedIn(const Opened& opened) {
  if (opened.made_for_a_space) {
    return opened.its_space_is_there ? Listed::kInTheSpaceItWasMadeFor : Listed::kClosed;
  }
  if (opened.opener_in_users_space) {
    return Listed::kInItsOpenersSpace;
  }
  if (opened.window_shows_users_space) {
    return Listed::kInTheSpaceShown;
  }
  return opened.profile_has_users_space ? Listed::kInTheUsersFirstSpace : Listed::kClosed;
}

bool TakesATabWithNowhereElse(const Candidate& space) {
  return !space.agents && !space.a_windows_own && space.of_the_tabs_profile;
}

}  // namespace browser_core::agent_space_rule
