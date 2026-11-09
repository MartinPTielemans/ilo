#include "browser_core/agent_request.h"

#include <map>
#include <memory>
#include <optional>
#include <string_view>
#include <utility>

#include "base/functional/bind.h"
#include "base/no_destructor.h"
#include "base/strings/string_split.h"
#include "base/strings/string_util.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "browser_core/host.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/storage_partition.h"
#include "content/public/browser/web_contents.h"
#include "net/base/isolation_info.h"
#include "net/base/net_errors.h"
#include "net/cookies/site_for_cookies.h"
#include "net/http/http_util.h"
#include "net/traffic_annotation/network_traffic_annotation.h"
#include "net/url_request/referrer_policy.h"
#include "services/network/public/cpp/resource_request.h"
#include "services/network/public/cpp/shared_url_loader_factory.h"
#include "services/network/public/cpp/simple_url_loader.h"
#include "services/network/public/cpp/url_loader_completion_status.h"
#include "services/network/public/mojom/url_response_head.mojom.h"
#include "url/gurl.h"
#include "url/origin.h"
#include "url/url_constants.h"

namespace browser_core {

namespace {

constexpr net::NetworkTrafficAnnotationTag kAgentRequest =
    net::DefineNetworkTrafficAnnotation("browser_core_agent_request", R"(
      semantics {
        sender: "Agents"
        description:
          "An HTTP request an agent the user connected sends from a tab in its own Space, to that "
          "tab's page's own site, as the page's own script could."
        trigger:
          "An agent calls its request tool. A request that changes anything (any method but GET "
          "and HEAD) is sent only once the user allows it, each time."
        data: "What the agent asked to send: a method, a path on the page's site, headers, a body."
        destination: WEBSITE
      }
      policy {
        cookies_allowed: YES
        cookies_store: "user"
        setting: "Settings › Agents: turn agents off, or revoke the agent."
        policy_exception_justification: "Only for agents the user connected."
      })");

// The most of an answer's body the app is given.
constexpr size_t kMaxBody = 1 << 20;
constexpr base::TimeDelta kTimeout = base::Seconds(30);

// The requests sent, until each answers.
std::map<uint64_t, std::unique_ptr<network::SimpleURLLoader>>& Sent() {
  static base::NoDestructor<std::map<uint64_t, std::unique_ptr<network::SimpleURLLoader>>> sent;
  return *sent;
}

void Report(uint64_t request, const bc_agent_response& response) {
  if (Host().agent_request_done) {
    Host().agent_request_done(Host().context, request, &response);
  }
}

void Fail(uint64_t request, int status, const std::string& why) {
  const bc_agent_response response = {status, "", "", 0, false, why.c_str()};
  Report(request, response);
}

void Answered(uint64_t request, std::optional<std::string> body) {
  auto sent = Sent().find(request);
  if (sent == Sent().end()) {
    return;
  }
  const std::unique_ptr<network::SimpleURLLoader> loader = std::move(sent->second);
  Sent().erase(sent);
  const network::mojom::URLResponseHead* head = loader->ResponseInfo();
  const int status = head && head->headers ? head->headers->response_code() : 0;
  const int error = loader->NetError();
  const std::optional<network::URLLoaderCompletionStatus>& completion = loader->CompletionStatus();
  // A redirect to another site, which the request's same-origin mode fails.
  if (completion && completion->cors_error_status &&
      completion->cors_error_status->cors_error == network::mojom::CorsError::kDisallowedByMode) {
    Fail(request, status, "It redirects to another site, so it stopped there.");
    return;
  }
  // With partial results, a body past the most the app is given fails so, with what came of it.
  const bool cut = error == net::ERR_INSUFFICIENT_RESOURCES && body;
  std::string why;
  if (error != net::OK && !cut) {
    why = (status ? "The answer broke off: " : "No answer came: ") + net::ErrorToShortString(error);
  }
  const std::string type = head ? head->mime_type : "";
  const std::string text = body.value_or("");
  const bc_agent_response response = {
      status, type.c_str(), text.data(), text.size(), cut, why.empty() ? nullptr : why.c_str()};
  Report(request, response);
}

}  // namespace

void RefuseAgentRequest(uint64_t request, std::string why) {
  base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
      FROM_HERE,
      base::BindOnce([](uint64_t request, const std::string& why) { Fail(request, 0, why); },
                     request, std::move(why)));
}

void SendAgentRequest(content::WebContents& contents,
                      uint64_t request,
                      std::string method,
                      const std::string& url,
                      const std::string& headers,
                      std::string body) {
  content::RenderFrameHost* frame = contents.GetPrimaryMainFrame();
  const url::Origin origin = frame->GetLastCommittedOrigin();
  // A page from a site, never an error page, the browser's or a sandboxed one, whose origins are
  // opaque: the request is the page's own, with its cookies, so it needs the page's site.
  if (origin.opaque() ||
      (origin.scheme() != url::kHttpScheme && origin.scheme() != url::kHttpsScheme)) {
    RefuseAgentRequest(request,
                       "The tab has no site's page loaded. Look at it with snapshot first.");
    return;
  }
  method = base::ToUpperASCII(method);
  if (!net::HttpUtil::IsToken(method)) {
    RefuseAgentRequest(request,
                       "That method isn't one: GET, POST, PUT, PATCH, DELETE and the like.");
    return;
  }
  if (method == "CONNECT" || method == "TRACE" || method == "TRACK") {
    RefuseAgentRequest(request, "Agents can't send " + method + " requests, as pages can't.");
    return;
  }
  // The address the user was asked about, which the app resolved against the page it saw: a tab
  // that went to another site since sends nothing.
  const GURL target(url);
  if (!target.is_valid() || !origin.IsSameOriginWith(target)) {
    RefuseAgentRequest(request, "The tab's page is on " + origin.Serialize() +
                                    ", and agents send requests only to the tab's own site.");
    return;
  }
  auto sent = std::make_unique<network::ResourceRequest>();
  for (std::string_view line :
       base::SplitStringPiece(headers, "\n", base::TRIM_WHITESPACE, base::SPLIT_WANT_NONEMPTY)) {
    const size_t colon = line.find(':');
    const std::string_view name =
        colon == std::string_view::npos ? line : net::HttpUtil::TrimLWS(line.substr(0, colon));
    const std::string_view value = colon == std::string_view::npos
                                       ? std::string_view()
                                       : net::HttpUtil::TrimLWS(line.substr(colon + 1));
    if (!net::HttpUtil::IsValidHeaderName(name) || !net::HttpUtil::IsValidHeaderValue(value)) {
      RefuseAgentRequest(request, "The header \"" + std::string(name) + "\" isn't a valid one.");
      return;
    }
    // Those the browser sets, as for the page's own requests, and the sign-in a page's script
    // could set but an agent mustn't see or send.
    if (!net::HttpUtil::IsSafeHeader(name, value) ||
        base::EqualsCaseInsensitiveASCII(name, "authorization")) {
      RefuseAgentRequest(request, "Agents can't set the " + std::string(name) +
                                      " header: the browser sets it, as it does for the page's "
                                      "own requests.");
      return;
    }
    sent->headers.SetHeader(name, value);
  }
  if ((method == "GET" || method == "HEAD") && !body.empty()) {
    RefuseAgentRequest(request, "A " + method + " request has no body.");
    return;
  }
  sent->url = target;
  sent->method = method;
  // As the page's own fetch to its site: its cookies, its site as the first party, its address as
  // the referrer, and never across sites, even by a redirect.
  sent->credentials_mode = network::mojom::CredentialsMode::kInclude;
  sent->mode = network::mojom::RequestMode::kSameOrigin;
  sent->destination = network::mojom::RequestDestination::kEmpty;
  sent->request_initiator = origin;
  sent->site_for_cookies = net::SiteForCookies::FromOrigin(origin);
  GURL::Replacements without_ref;
  without_ref.ClearRef();
  sent->referrer = frame->GetLastCommittedURL().ReplaceComponents(without_ref);
  sent->referrer_policy = net::ReferrerPolicy::REDUCE_GRANULARITY_ON_TRANSITION_CROSS_ORIGIN;
  sent->trusted_params = network::ResourceRequest::TrustedParams();
  sent->trusted_params->isolation_info = net::IsolationInfo::Create(
      net::IsolationInfo::RequestType::kOther, origin, origin, sent->site_for_cookies);
  std::unique_ptr<network::SimpleURLLoader> loader =
      network::SimpleURLLoader::Create(std::move(sent), kAgentRequest);
  if (!body.empty()) {
    // The app gives the type in the headers, as the agent asked or for JSON.
    loader->AttachStringForUpload(std::move(body));
  }
  loader->SetAllowHttpErrorResults(true);
  loader->SetAllowPartialResults(true);
  loader->SetTimeoutDuration(kTimeout);
  network::SimpleURLLoader* sending = loader.get();
  Sent()[request] = std::move(loader);
  sending->DownloadToString(
      frame->GetStoragePartition()->GetURLLoaderFactoryForBrowserProcess().get(),
      base::BindOnce(&Answered, request), kMaxBody);
}

}  // namespace browser_core
