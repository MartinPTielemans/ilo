#include "browser_core/page_log.h"

#include <set>
#include <string_view>
#include <utility>

#include "base/containers/span.h"
#include "base/json/json_reader.h"
#include "base/json/json_writer.h"
#include "base/no_destructor.h"
#include "base/strings/utf_string_conversions.h"
#include "base/values.h"
#include "browser_core/developer_mode.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "chrome/browser/profiles/profile.h"
#include "content/public/browser/devtools_agent_host.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/page.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "net/base/net_errors.h"
#include "net/http/http_request_headers.h"
#include "net/http/http_response_headers.h"
#include "services/network/public/mojom/fetch_api.mojom-shared.h"
#include "third_party/blink/public/mojom/loader/resource_load_info.mojom.h"

namespace browser_core {

void PageLogsSpacesChanged() {
  PageLog::SpacesChanged();
}

namespace {

// Every log there is, one a tab an agent owns: none while no agent has a tab.
std::set<content::WebContents*>& Logged() {
  static base::NoDestructor<std::set<content::WebContents*>> logged;
  return *logged;
}

const char* LevelName(blink::mojom::ConsoleMessageLevel level) {
  switch (level) {
    case blink::mojom::ConsoleMessageLevel::kVerbose:
      return "verbose";
    case blink::mojom::ConsoleMessageLevel::kInfo:
      return "info";
    case blink::mojom::ConsoleMessageLevel::kWarning:
      return "warning";
    case blink::mojom::ConsoleMessageLevel::kError:
      return "error";
  }
  return "info";
}

page_log::Headers HeadersOf(const base::DictValue* headers) {
  page_log::Headers kept;
  if (!headers) {
    return kept;
  }
  for (const auto [name, value] : *headers) {
    if (value.is_string() && page_log::Kept(name)) {
      kept[name] = value.GetString();
    }
  }
  return kept;
}

bool Primary(content::RenderFrameHost* frame) {
  return frame && frame->GetPage().IsPrimary();
}

}  // namespace

PageLog::PageLog(content::WebContents* contents, std::string agent)
    : content::WebContentsObserver(contents),
      content::WebContentsUserData<PageLog>(*contents),
      agent_(std::move(agent)) {
  Logged().insert(contents);
  // Made for a page already open, it keeps what comes from now on.
  const GURL url = contents->GetLastCommittedURL();
  Profile* profile = Profile::FromBrowserContext(contents->GetBrowserContext());
  kept_ = url.SchemeIsHTTPOrHTTPS() && !profile->IsOffTheRecord();
  log_.NewPage(false);
  Follow(DeveloperModeNow());
}

PageLog::~PageLog() {
  Detach();
  Logged().erase(&GetWebContents());
}

// static
void PageLog::SpacesChanged() {
  std::map<content::WebContents*, std::string> owned;
  for (const Space* space : Space::All()) {
    if (!space->agent()) {
      continue;
    }
    std::vector<bc_id> tabs = space->pinned();
    tabs.insert(tabs.end(), space->today().begin(), space->today().end());
    for (bc_id id : tabs) {
      if (Tab* tab = Tab::Get(id); tab && tab->web_contents()) {
        owned[tab->web_contents()] = space->agent()->id;
      }
    }
  }
  // Copied: removing a log changes the set.
  for (content::WebContents* contents : std::set<content::WebContents*>(Logged())) {
    PageLog* log = FromWebContents(contents);
    auto it = owned.find(contents);
    if (log && (it == owned.end() || it->second != log->agent_)) {
      contents->RemoveUserData(UserDataKey());
    }
  }
  for (const auto& [contents, agent] : owned) {
    if (!FromWebContents(contents)) {
      CreateForWebContents(contents, agent);
    }
  }
}

// static
void PageLog::DeveloperModeChanged(content::WebContents* contents) {
  if (PageLog* log = contents ? FromWebContents(contents) : nullptr) {
    log->Follow(log->DeveloperModeNow());
  }
}

// static
std::optional<std::string> PageLog::For(const std::string& agent,
                                        bc_id tab_id,
                                        bool* developer_mode) {
  Tab* tab = Tab::Get(tab_id);
  if (!tab || !tab->web_contents() || !Space::AgentMayAct(agent, tab_id)) {
    return std::nullopt;
  }
  PageLog* log = FromWebContents(tab->web_contents());
  if (!log || log->agent_ != agent || !log->kept_) {
    return std::nullopt;
  }
  if (developer_mode) {
    *developer_mode = log->log_.developer_mode();
  }
  return log->log_.Json();
}

bool PageLog::DeveloperModeNow() {
  Profile* profile = Profile::FromBrowserContext(GetWebContents().GetBrowserContext());
  return kept_ && developer_mode::On(profile, GetWebContents().GetLastCommittedURL());
}

void PageLog::Follow(bool developer_mode) {
  log_.SetDeveloperMode(developer_mode);
  if (log_.WantsSession()) {
    Attach();
  } else {
    Detach();
  }
}

void PageLog::Attach() {
  if (agent_host_) {
    return;
  }
  scoped_refptr<content::DevToolsAgentHost> host =
      content::DevToolsAgentHost::GetOrCreateFor(&GetWebContents());
  // The profile's policy may keep DevTools from the page: then the requests come without details.
  if (!host || !host->AttachClient(this)) {
    return;
  }
  agent_host_ = std::move(host);
  Send("Network.enable", base::DictValue());
}

void PageLog::Detach() {
  starting_.clear();
  bodies_.clear();
  if (scoped_refptr<content::DevToolsAgentHost> host = std::move(agent_host_)) {
    host->DetachClient(this);
  }
}

int PageLog::Send(const char* method, base::DictValue params) {
  if (!agent_host_) {
    return 0;
  }
  const int id = next_command_++;
  std::optional<std::string> json = base::WriteJson(
      base::DictValue().Set("id", id).Set("method", method).Set("params", std::move(params)));
  if (!json) {
    return 0;
  }
  agent_host_->DispatchProtocolMessage(this, base::as_byte_span(*json));
  return id;
}

void PageLog::DidFinishNavigation(content::NavigationHandle* navigation) {
  if (!navigation->IsInPrimaryMainFrame() || !navigation->HasCommitted() ||
      navigation->IsSameDocument()) {
    return;
  }
  Profile* profile = Profile::FromBrowserContext(GetWebContents().GetBrowserContext());
  kept_ = navigation->GetURL().SchemeIsHTTPOrHTTPS() && !profile->IsOffTheRecord();
  starting_.clear();
  bodies_.clear();
  log_.NewPage(false);
  Follow(DeveloperModeNow());
  if (!kept_) {
    return;
  }
  // The page's own document, which Chromium's record of requests leaves to its navigation.
  const net::HttpResponseHeaders* response = navigation->GetResponseHeaders();
  const int status = response ? response->response_code() : 0;
  const int net_error = navigation->GetNetErrorCode();
  if (!page_log::Failed(net_error, status)) {
    return;
  }
  log_.Add(page_log::Request{.method = navigation->IsPost() ? "POST" : "GET",
                             .url = navigation->GetURL(),
                             .status = status,
                             .net_error = net_error,
                             .error = net_error ? net::ErrorToString(net_error) : ""});
  if (!log_.developer_mode()) {
    return;
  }
  page_log::Detail detail{.url = navigation->GetURL(), .status = status};
  for (const auto& header : navigation->GetRequestHeaders().GetHeaderVector()) {
    detail.request_headers[header.key] = header.value;
  }
  if (response) {
    size_t at = 0;
    std::string name;
    std::string value;
    while (response->EnumerateHeaderLines(&at, &name, &value)) {
      std::string& kept = detail.response_headers[name];
      kept = kept.empty() ? value : kept + "\n" + value;
    }
  }
  // Its body is the page itself, which read_page reads.
  log_.Add("navigation", std::move(detail));
}

void PageLog::ResourceLoadComplete(content::RenderFrameHost* frame,
                                   const content::GlobalRequestID& request_id,
                                   const GURL& original_url,
                                   const blink::mojom::ResourceLoadInfo& info) {
  if (!kept_ || !Primary(frame) || !page_log::Failed(info.net_error, info.http_status_code)) {
    return;
  }
  // The page's own document is its navigation's (DidFinishNavigation).
  if (frame->IsInPrimaryMainFrame() &&
      info.request_destination == network::mojom::RequestDestination::kDocument) {
    return;
  }
  log_.Add(page_log::Request{.method = info.method,
                             .url = info.final_url,
                             .status = info.http_status_code,
                             .net_error = info.net_error,
                             .error = info.net_error ? net::ErrorToString(info.net_error) : ""});
}

void PageLog::OnDidAddMessageToConsole(content::RenderFrameHost* frame,
                                       blink::mojom::ConsoleMessageLevel level,
                                       const std::u16string& message,
                                       int32_t line,
                                       const std::u16string& source,
                                       const std::optional<std::u16string>& stack) {
  if (!kept_ || !log_.developer_mode() || !Primary(frame)) {
    return;
  }
  log_.Add(page_log::Message{.level = LevelName(level),
                             .text = base::UTF16ToUTF8(message),
                             .source = base::UTF16ToUTF8(source),
                             .line = line});
}

void PageLog::DispatchProtocolMessage(content::DevToolsAgentHost* agent_host,
                                      base::span<const uint8_t> message) {
  std::optional<base::DictValue> dict = base::JSONReader::ReadDict(
      std::string_view(reinterpret_cast<const char*>(message.data()), message.size()),
      base::JSON_PARSE_RFC);
  if (!dict || !log_.developer_mode()) {
    return;
  }
  if (std::optional<int> id = dict->FindInt("id")) {
    auto asked = bodies_.find(*id);
    if (asked == bodies_.end()) {
      return;
    }
    const std::string request = std::move(asked->second);
    bodies_.erase(asked);
    const base::DictValue* result = dict->FindDict("result");
    const std::string* body = result ? result->FindString("body") : nullptr;
    if (body) {
      log_.Body(request, *body, result->FindBool("base64Encoded").value_or(false));
    }
    return;
  }
  const std::string* method = dict->FindString("method");
  const base::DictValue* params = dict->FindDict("params");
  const std::string* request = params ? params->FindString("requestId") : nullptr;
  if (!method || !request) {
    return;
  }
  if (*method == "Network.requestWillBeSent") {
    // Bounded as the log is: a page that never answers can't grow it.
    if (starting_.size() < page_log::kMostRequests) {
      starting_[*request] = HeadersOf(params->FindDictByDottedPath("request.headers"));
    }
  } else if (*method == "Network.responseReceived") {
    auto started = starting_.find(*request);
    page_log::Headers sent =
        started == starting_.end() ? page_log::Headers() : std::move(started->second);
    if (started != starting_.end()) {
      starting_.erase(started);
    }
    const base::DictValue* response = params->FindDict("response");
    const int status = response ? response->FindInt("status").value_or(0) : 0;
    const std::string* url = response ? response->FindString("url") : nullptr;
    if (!url || !page_log::Failed(0, status)) {
      return;
    }
    log_.Add(*request,
             page_log::Detail{.url = GURL(*url),
                              .status = status,
                              .request_headers = std::move(sent),
                              .response_headers = HeadersOf(response->FindDict("headers"))});
  } else if (*method == "Network.loadingFinished") {
    // The body is asked for once it's all there, so it's kept before an agent asks for the log.
    if (log_.HasDetail(*request)) {
      if (const int id =
              Send("Network.getResponseBody", base::DictValue().Set("requestId", *request))) {
        bodies_[id] = *request;
      }
    }
  } else if (*method == "Network.loadingFailed") {
    starting_.erase(*request);
  }
}

void PageLog::AgentHostClosed(content::DevToolsAgentHost* agent_host) {
  agent_host_ = nullptr;
  starting_.clear();
  bodies_.clear();
}

WEB_CONTENTS_USER_DATA_KEY_IMPL(PageLog);

}  // namespace browser_core
