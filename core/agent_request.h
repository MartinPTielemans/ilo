#ifndef BROWSER_CORE_AGENT_REQUEST_H_
#define BROWSER_CORE_AGENT_REQUEST_H_

#include <cstdint>
#include <string>

#include "base/functional/callback_helpers.h"
#include "browser_core/public/browser_core.h"

namespace content {
class WebContents;
}  // namespace content

namespace browser_core {

// Sends an agent's HTTP request from the page in |contents|, as bc_agent_request says, then
// answers through agent_request_done with |request|. |lift| holds the page's vote until then.
void SendAgentRequest(content::WebContents& contents,
                      uint64_t request,
                      std::string method,
                      const std::string& url,
                      const std::string& headers,
                      std::string body,
                      base::ScopedClosureRunner lift);

// Answers |request| with |why| it wasn't sent, and |failure| for agent code to decide on, from a
// task of its own, as an answer never comes before its call returns.
void RefuseAgentRequest(uint64_t request,
                        std::string why,
                        bc_agent_failure failure = BC_AGENT_FAILURE_NONE);

}  // namespace browser_core

#endif  // BROWSER_CORE_AGENT_REQUEST_H_
