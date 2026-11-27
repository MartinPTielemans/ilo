#ifndef BROWSER_CORE_PAGE_LOG_H_
#define BROWSER_CORE_PAGE_LOG_H_

#include <stdint.h>

#include <map>
#include <optional>
#include <string>

#include "base/memory/scoped_refptr.h"
#include "base/values.h"
#include "browser_core/page_log_rule.h"
#include "browser_core/public/browser_core.h"
#include "content/public/browser/devtools_agent_host_client.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/web_contents_user_data.h"

namespace content {
class DevToolsAgentHost;
}

namespace browser_core {

// What the page in a tab an agent owns reported, for bc_agent_page_log (#4377). It's made only for
// a tab in an agent's Space, and goes, with what it kept, once the tab leaves it or closes, so no
// other tab is ever heard. Its failed requests come from Chromium's own record of each request.
// In Developer Mode, which only the user turns on for a site, it hears the console too, and holds
// a DevTools session of the browser's own, the Network domain alone, for the failed requests'
// headers and bodies: attached only then, and detached as soon as the page isn't in it.
class PageLog : public content::WebContentsObserver,
                public content::WebContentsUserData<PageLog>,
                public content::DevToolsAgentHostClient {
 public:
  PageLog(const PageLog&) = delete;
  PageLog& operator=(const PageLog&) = delete;
  ~PageLog() override;

  // A log for each tab in an agent's Space, and none for any other. After every Space change.
  static void SpacesChanged();
  // The user turned Developer Mode on or off for the page in |contents|.
  static void DeveloperModeChanged(content::WebContents* contents);
  // See bc_agent_page_log: the log of |tab| as JSON, or nullopt for a tab the agent with the id
  // |agent| doesn't own, one in a private window, or one whose page isn't http(s).
  static std::optional<std::string> For(const std::string& agent, bc_id tab, bool* developer_mode);

 private:
  friend WebContentsUserData;
  PageLog(content::WebContents* contents, std::string agent);

  // Whether the page in the tab is in Developer Mode now, its log following, and the session.
  bool DeveloperModeNow() const;
  void Follow(bool developer_mode);
  void Attach();
  void Detach();
  // Sends a command to the session, if attached, and returns its id, else 0.
  int Send(const char* method, base::DictValue params);

  // content::WebContentsObserver:
  void DidFinishNavigation(content::NavigationHandle* navigation) override;
  void ResourceLoadComplete(content::RenderFrameHost* frame,
                            const content::GlobalRequestID& request_id,
                            const GURL& original_url,
                            const blink::mojom::ResourceLoadInfo& info) override;
  void OnDidAddMessageToConsole(content::RenderFrameHost* frame,
                                blink::mojom::ConsoleMessageLevel level,
                                const std::u16string& message,
                                int32_t line,
                                const std::u16string& source,
                                const std::optional<std::u16string>& stack) override;

  // content::DevToolsAgentHostClient:
  void DispatchProtocolMessage(content::DevToolsAgentHost* agent_host,
                               base::span<const uint8_t> message) override;
  void AgentHostClosed(content::DevToolsAgentHost* agent_host) override;

  // The agent whose Space held the tab when it was made: another agent's is another log.
  const std::string agent_;
  // Whether the page is one a log is kept for: http(s), outside a private window.
  bool kept_ = false;
  page_log::Log log_;
  scoped_refptr<content::DevToolsAgentHost> agent_host_;
  int next_command_ = 1;
  // The requests heard starting, by DevTools' id, until their response or failure: their
  // headers, for a response that failed.
  std::map<std::string, page_log::Headers> starting_;
  // getResponseBody's commands sent, by id, to the request each asks for.
  std::map<int, std::string> bodies_;

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

}  // namespace browser_core

#endif  // BROWSER_CORE_PAGE_LOG_H_
