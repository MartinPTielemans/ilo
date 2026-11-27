#include "browser_core/agent_space_rule.h"

#include <algorithm>
#include <utility>

namespace browser_core::agent_space_rule {

bool Refuses(Ask ask, const Tab& tab) {
  if (!tab.agents) {
    return false;
  }
  switch (ask) {
    case Ask::kPaste:
    case Ask::kCopy:
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

int AgentSpaceIn(const std::vector<AgentSpace>& spaces,
                 std::string_view agent,
                 std::string_view profile) {
  int found = -1;
  for (size_t at = 0; at < spaces.size(); ++at) {
    if (agent.empty() || spaces[at].agent != agent) {
      continue;
    }
    if (profile.empty()) {
      found = static_cast<int>(at);
    } else if (spaces[at].profile == profile) {
      return static_cast<int>(at);
    }
  }
  return found;
}

bool OpensIn(std::string_view profile, std::string_view given) {
  return !profile.empty() && profile == given;
}

uint64_t Picked(const std::vector<PickCandidate>& candidates) {
  const PickCandidate* best = nullptr;
  for (const PickCandidate& each : candidates) {
    if (!each.profile || !each.loaded || each.private_session || each.last_visit <= 0) {
      continue;
    }
    if (!best || each.last_visit > best->last_visit ||
        (each.last_visit == best->last_visit && each.order < best->order)) {
      best = &each;
    }
  }
  return best ? best->profile : 0;
}

std::vector<std::string> PickHosts(std::string_view host, std::string_view site) {
  std::vector<std::string> hosts;
  for (std::string each : {std::string(host), std::string(site), "www." + std::string(site)}) {
    if (!each.empty() && each != "www." &&
        std::find(hosts.begin(), hosts.end(), each) == hosts.end()) {
      hosts.push_back(std::move(each));
    }
  }
  return hosts;
}

PickAnswer::PickAnswer(base::OnceCallback<void(int64_t)> answer) : answer_(std::move(answer)) {}

PickAnswer::~PickAnswer() {
  Answer(0);
}

void PickAnswer::Answer(int64_t last_visit) {
  if (answer_) {
    std::move(answer_).Run(last_visit);
  }
}

}  // namespace browser_core::agent_space_rule
