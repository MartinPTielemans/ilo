#ifndef BROWSER_CORE_AGENT_SESSION_H_
#define BROWSER_CORE_AGENT_SESSION_H_

#include <cstddef>
#include <string>
#include <string_view>

#include "browser_core/public/browser_core.h"

namespace browser_core {

// An agent's session with a tab, which the app shows as the agent at work there: from the first
// call bc_agent_may_act lets the agent make in the tab until its client disconnects, it releases
// the tab or the user takes the tab back. Each of those tells the app (agent_session_ended), as
// the tab closing or leaving the agent's Space doesn't, since the app sees those itself. No timer
// ends one: an idle browser schedules nothing.

// Why the core refuses every agent |action|, for the user and the agent to read, or an empty string
// for one an agent may do. Agents get only the actions a user does in a page, and none that reads
// what the user's sessions keep, changes ad blocking or a profile's settings, or opens DevTools
// (decision 8).
std::string RefusedAction(std::string_view action);

// The core let |agent| act in |tab|: opens its session there if none is, blurring the page so no
// password the user showed in it stays shown.
void AgentActed(const std::string& agent, bc_id tab);

// Ends |agent|'s session with |tab| for |why|, a BC_AGENT_SESSION_ value, and returns whether it
// had one.
bool EndAgentSession(const std::string& agent, bc_id tab, int why);

// Ends each of |agent|'s sessions for |why| and returns how many it had.
size_t EndAgentSessions(const std::string& agent, int why);

// Ends whichever agent's session holds |tab| for |why|, and returns whether one did.
bool EndTabSession(bc_id tab, int why);

// The user holds |tab| from agents, as at a handoff or once they take it over: no agent may act
// in it until they hand it back. For this run of the browser, as sessions are.
void Hold(bc_id tab);

// Hands |tab| back, blurring its page as a session's start does, and returns whether the user held
// it.
bool HandBack(bc_id tab);

bool IsHeld(bc_id tab);

// What an agent's call on a tab the user holds hears, whenever the core finds it held.
inline constexpr char kHeldRefusal[] =
    "The user has this tab until they hand it back. Ask for it with ask_for_tab.";

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_SESSION_H_
