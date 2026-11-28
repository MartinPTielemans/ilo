#ifndef BROWSER_CORE_AGENT_TABS_CLOSE_H_
#define BROWSER_CORE_AGENT_TABS_CLOSE_H_

#include <string>

#include "browser_core/public/browser_core.h"

namespace browser_core {

// Agents › Close a thread's tabs (bc_agent_tabs_close_after): a ⌘T thread's own tabs close once it
// has been quiet that long, as agent_tabs_close_rule decides. A thread is an agent the app named
// with bc_agent_set_working; outside agents keep their tabs as before. Each thread has one
// wall-clock timer, armed only while it's idle with a tab of its own, so an idle browser wakes at
// most once a thread, and never polls.

// bc_agent_set_working.
void ThreadWorking(const std::string& agent, bool working);
// The core let |agent| act in one of its tabs, or opened one for it (AgentActed).
void ThreadActed(const std::string& agent);
// The setting changed: each thread's timer is armed again from its own last activity.
void ThreadTabsRearm();
// No window shows |tab| any more: it closes if it's a thread's own and the thread is due.
void ThreadTabHidden(bc_id tab);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_TABS_CLOSE_H_
