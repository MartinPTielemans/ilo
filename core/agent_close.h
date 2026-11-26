#ifndef BROWSER_CORE_AGENT_CLOSE_H_
#define BROWSER_CORE_AGENT_CLOSE_H_

#include <string>

#include "browser_core/public/browser_core.h"

namespace browser_core {

// Closes the agent's |tab|, which bc_agent_close_tab has checked is the agent's to close, and
// returns BC_AGENT_CLOSED or BC_AGENT_CLOSE_PENDING (#3256). A page with a beforeunload handler is
// asked as a user's close asks it, with WebContents::DispatchBeforeUnload's automatic cancel, so
// its "Leave site?" never shows: agent_close_answered says, once, what came of it. A second close
// while it's asked joins that answer. |discard| closes it at once without its handlers, one being
// asked too.
int AgentCloseTab(bc_id tab, bool discard);

// Archives |tab|, which bc_agent_archive_tab has checked the user handed |agent|, and returns a
// BC_AGENT_CLOSE value at once, never PENDING: its page is never asked (AgentArchive).
int AgentArchiveTab(const std::string& agent, bc_id tab, bool discard);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_CLOSE_H_
