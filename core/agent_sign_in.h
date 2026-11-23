#ifndef BROWSER_CORE_AGENT_SIGN_IN_H_
#define BROWSER_CORE_AGENT_SIGN_IN_H_

#include <cstddef>
#include <cstdint>
#include <string>

#include "browser_core/public/browser_core.h"

namespace browser_core {

// An agent's sign-in (bc_agent_sign_in, #4098): the core fills a login the user saved into the
// main frame's form as Chrome fills it for the user, and submits it once, holding the tab from
// every agent until it answers. The password goes from the profile's store to the page alone: it's
// never logged, never in a crash key or trace, and kept in memory only until the sign-in answers.
// The usernames go to the app alone (sign_in_choose, agent_signed_in), never to an agent.

// bc_agent_sign_in.
void StartAgentSignIn(const std::string& agent, bc_id tab, uint64_t request);

// Whether the core is signing |tab| in for an agent: no agent may act in it until that answers.
bool SigningIn(bc_id tab);

// bc_tab_sign_in_choose.
bool ChooseSignIn(bc_id tab, uint64_t request, size_t account);

// A sign-in under way in |tab| stops, as the user took the tab back or the agent's session with it
// ended: what it filled is emptied, and the agent hears |failure|.
void StopSignIn(bc_id tab, bc_agent_failure failure);

// The user hands |tab| back to agents (bc_tab_hand_back): before an agent's next action in its
// page, the main frame's password fields and the fields Chrome filled for the user are emptied.
void EmptyForAgent(bc_id tab);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_SIGN_IN_H_
