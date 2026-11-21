#include "browser_core/link_card.h"

#include <memory>
#include <optional>
#include <string_view>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "browser_core/host.h"
#include "browser_core/link_card_rule.h"
#include "browser_core/network_refuser.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "content/public/browser/browser_context.h"
#include "content/public/browser/storage_partition.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_observer.h"
#include "mojo/public/cpp/bindings/pending_remote.h"
#include "mojo/public/cpp/bindings/receiver.h"
#include "mojo/public/cpp/bindings/receiver_set.h"
#include "mojo/public/cpp/bindings/remote.h"
#include "net/base/isolation_info.h"
#include "net/base/load_flags.h"
#include "net/base/net_errors.h"
#include "net/base/network_anonymization_key.h"
#include "net/http/http_request_headers.h"
#include "net/http/http_response_headers.h"
#include "net/proxy_resolution/proxy_info.h"
#include "net/traffic_annotation/network_traffic_annotation.h"
#include "net/url_request/redirect_info.h"
#include "net/url_request/referrer_policy.h"
#include "services/network/public/cpp/constants.h"
#include "services/network/public/cpp/cors/cors_error_status.h"
#include "services/network/public/cpp/originating_process_id.h"
#include "services/network/public/cpp/resource_request.h"
#include "services/network/public/cpp/simple_url_loader.h"
#include "services/network/public/cpp/simple_url_loader_stream_consumer.h"
#include "services/network/public/cpp/url_loader_completion_status.h"
#include "services/network/public/mojom/network_context.mojom.h"
#include "services/network/public/mojom/proxy_lookup_client.mojom.h"
#include "services/network/public/mojom/url_loader_factory.mojom.h"
#include "services/network/public/mojom/url_loader_network_service_observer.mojom.h"
#include "services/network/public/mojom/url_response_head.mojom.h"
#include "third_party/blink/public/common/input/web_input_event.h"
#include "third_party/blink/public/common/input/web_keyboard_event.h"
#include "ui/events/keycodes/keyboard_codes.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

constexpr net::NetworkTrafficAnnotationTag kLinkCard =
    net::DefineNetworkTrafficAnnotation("browser_core_link_card", R"(
      semantics {
        sender: "Link cards"
        description:
          "The page behind a link the user pressed ⇧ on, or the picture that page names, asked "
          "without the user's cookies, client certificates or sign-ins, to show the page's title, "
          "description and picture in a card over the page."
        trigger:
          "The user presses ⇧ alone with the pointer on a link in a page, outside a field, and "
          "no key, mouse press or wheel follows within 150 ms."
        data:
          "The address asked, with the browser's user agent and the profile's languages. No "
          "cookies, referrer, client certificate or credentials."
        destination: WEBSITE
      }
      policy {
        cookies_allowed: NO
        setting: "Settings › Links › Link cards."
        policy_exception_justification:
          "Asked only on the user's ⇧ press, one page and its picture at a time."
      })");

// The link target's own delay, link-target's delay in app/design/tokens.toml: a ⇧ that's the
// start of a site's shortcut, such as Gmail's ⇧I, costs nothing.
constexpr base::TimeDelta kWait = base::Milliseconds(150);
// A page or its picture, every redirect included.
constexpr base::TimeDelta kTimeout = base::Seconds(5);

// Whether the card may ask from |tab|: one of the user's, never an agent's Space's.
content::WebContents* UsersPage(bc_id tab) {
  const Space* space = Space::Holding(tab);
  return space && space->agent() ? nullptr : Tab::FromId(tab);
}

class Card;

std::unique_ptr<Card>& Current() {
  static base::NoDestructor<std::unique_ptr<Card>> card;
  return *card;
}

uint64_t g_last_card = 0;
uint64_t g_cancelled_card = 0;

// What the card came to, told from a task of its own, once the card's own call has returned; and
// only while it's the last card and not cancelled, as the app may have moved on since.
void Tell(uint64_t card,
          int status,
          const std::string& url,
          const std::string& mime,
          const std::string& bytes) {
  if (card == g_last_card && card != g_cancelled_card && Host().link_card_fetched) {
    Host().link_card_fetched(Host().context, card, status, url.c_str(), mime.c_str(),
                             reinterpret_cast<const uint8_t*>(bytes.data()), bytes.size());
  }
}

// A card: its wait, then its page, then its picture if the app asks. The wait watches the tab's
// input and nothing else; the fetches follow redirects themselves, so each hop is checked as the
// first is, its proxy looked up again and its address refused if it's a private network's.
class Card : public content::WebContentsObserver,
             public network::SimpleURLLoaderStreamConsumer,
             public network::mojom::ProxyLookupClient {
 public:
  Card(uint64_t id, bc_id tab, content::WebContents* contents, GURL url)
      : content::WebContentsObserver(contents), id_(id), tab_(tab), url_(std::move(url)) {
    wait_.Start(FROM_HERE, kWait, base::BindOnce(&Card::Waited, base::Unretained(this)));
  }
  Card(const Card&) = delete;
  Card& operator=(const Card&) = delete;
  ~Card() override = default;

  uint64_t id() const { return id_; }

  void Picture(GURL url) {
    // Only once the page is told, and once.
    if (!told_page_ || asked_picture_) {
      return;
    }
    asked_picture_ = true;
    picture_ = true;
    hops_ = 0;
    // The page's, which a picture that fails before its answer starts mustn't be told with.
    type_.clear();
    mime_.clear();
    if (!LinkCardAsks(url)) {
      End(net::ERR_DISALLOWED_URL_SCHEME, url);
      return;
    }
    Limit();
    Ask(std::move(url));
  }

 private:
  // content::WebContentsObserver, only during the wait: anything the user does in the page ends
  // the card, a ⇧-click or ⇧-wheel too, but the ⇧ itself, which the page gets after the app asked
  // for the card, and its repeats.
  void DidGetUserInteraction(const blink::WebInputEvent& event) override {
    if (blink::WebInputEvent::IsKeyboardEventType(event.GetType()) &&
        Shift(static_cast<const blink::WebKeyboardEvent&>(event).windows_key_code)) {
      return;
    }
    End(net::ERR_ABORTED, GURL());
  }
  void PrimaryPageChanged(content::Page& page) override { End(net::ERR_ABORTED, GURL()); }
  void WebContentsDestroyed() override { End(net::ERR_ABORTED, GURL()); }

  static bool Shift(int key) {
    return key == ui::VKEY_SHIFT || key == ui::VKEY_LSHIFT || key == ui::VKEY_RSHIFT;
  }

  void Waited() {
    // The tab is let go here: the fetches need only its profile's network, which Ask finds again
    // for each hop.
    Observe(nullptr);
    Limit();
    Ask(url_);
  }

  // The page or its picture in kTimeout, proxy lookups and every redirect included.
  void Limit() {
    limit_.Start(FROM_HERE, kTimeout, base::BindOnce(&Card::TimedOut, base::Unretained(this)));
  }
  void TimedOut() { End(net::ERR_TIMED_OUT, url_); }

  network::mojom::NetworkContext* Network() {
    content::WebContents* contents = UsersPage(tab_);
    return contents
               ? contents->GetBrowserContext()->GetDefaultStoragePartition()->GetNetworkContext()
               : nullptr;
  }

  // One hop: the address is checked, then where the profile's proxy settings send it.
  void Ask(GURL url) {
    url_ = std::move(url);
    network::mojom::NetworkContext* network = Network();
    if (!network) {
      End(net::ERR_ABORTED, GURL());
      return;
    }
    lookup_.reset();
    network->LookUpProxyForURL(url_, net::NetworkAnonymizationKey::CreateTransient(),
                               lookup_.BindNewPipeAndPassRemote());
    lookup_.set_disconnect_handler(base::BindOnce(&Card::End, base::Unretained(this),
                                                  BC_LINK_CARD_PROXY, url_, std::string()));
  }

  // network::mojom::ProxyLookupClient.
  void OnProxyLookupComplete(int32_t net_error,
                             const std::optional<net::ProxyInfo>& proxy_info) override {
    lookup_.reset();
    if (!LinkCardDirect(net_error, proxy_info.has_value(), proxy_info && proxy_info->is_direct())) {
      End(BC_LINK_CARD_PROXY, url_);
      return;
    }
    Send();
  }

  void Send() {
    network::mojom::NetworkContext* network = Network();
    if (!network) {
      End(net::ERR_ABORTED, GURL());
      return;
    }
    if (!factory_.is_bound()) {
      auto params = network::mojom::URLLoaderFactoryParams::New();
      params->process_id = network::OriginatingProcessId::browser();
      params->is_trusted = true;
      params->is_orb_enabled = false;
      // Its own, never the profile's, which could pick a client certificate or answer a sign-in
      // for the user.
      params->url_loader_network_observer = refuser_.Bind();
      params->network_restrictions_id = network::GetNoOpNetworkRestrictionsId();
      network->CreateURLLoaderFactory(factory_.BindNewPipeAndPassReceiver(), std::move(params));
    }
    auto request = std::make_unique<network::ResourceRequest>();
    request->url = url_;
    request->method = net::HttpRequestHeaders::kGetMethod;
    request->credentials_mode = network::mojom::CredentialsMode::kOmit;
    request->referrer_policy = net::ReferrerPolicy::NO_REFERRER;
    request->do_not_prompt_for_login = true;
    // Nothing kept in the cache, and nothing shared with the user's browsing: its own sockets,
    // TLS sessions and cache partition, gone with it.
    request->load_flags = net::LOAD_DISABLE_CACHE;
    request->trusted_params = network::ResourceRequest::TrustedParams();
    request->trusted_params->isolation_info = net::IsolationInfo::CreateTransient(std::nullopt);
    request->headers.SetHeader(
        net::HttpRequestHeaders::kAccept,
        picture_ ? "image/webp,image/png,image/jpeg,image/gif" : "text/html,application/xhtml+xml");
    loader_ = network::SimpleURLLoader::Create(std::move(request), kLinkCard);
    // No cookie read or written, and no address on a private network, checked against the address
    // each connection actually reached.
    loader_->SetURLLoaderFactoryOptions(network::mojom::kURLLoadOptionBlockAllCookies |
                                        network::mojom::kURLLoadOptionBlockLocalRequest);
    loader_->SetAllowHttpErrorResults(true);
    loader_->SetOnRedirectCallback(base::BindRepeating(&Card::Redirected, weak_.GetWeakPtr()));
    loader_->SetOnResponseStartedCallback(base::BindOnce(&Card::Started, weak_.GetWeakPtr()));
    read_.clear();
    loader_->DownloadAsStream(factory_.get(), this);
  }

  void Redirected(const GURL& from,
                  const net::RedirectInfo& redirect,
                  const network::mojom::URLResponseHead& head,
                  std::vector<std::string>* removed_headers) {
    // Followed here, as a new request, so the next hop is checked as the first was.
    loader_.reset();
    if (!LinkCardFollows(redirect.new_url, ++hops_)) {
      End(BC_LINK_CARD_REDIRECT, redirect.new_url);
      return;
    }
    Ask(redirect.new_url);
  }

  void Started(const GURL& url, const network::mojom::URLResponseHead& head) {
    status_ = head.headers ? head.headers->response_code() : 0;
    type_ = head.mime_type;
    // With the charset the server named, which the app decodes the page with before its meta's.
    mime_ = head.charset.empty() ? type_ : type_ + ";charset=" + head.charset;
    const bool fine = status_ >= 200 && status_ < 300 &&
                      (picture_ ? LinkCardPicture(type_) : LinkCardPage(type_));
    if (!fine) {
      End(status_, url_);
      return;
    }
    if (picture_ && head.content_length > static_cast<int64_t>(kLinkCardPictureCap)) {
      End(BC_LINK_CARD_TOO_BIG, url_);
    }
  }

  // network::SimpleURLLoaderStreamConsumer.
  void OnDataReceived(std::string_view data, base::OnceClosure resume) override {
    read_.append(data);
    if (picture_) {
      if (read_.size() > kLinkCardPictureCap) {
        End(BC_LINK_CARD_TOO_BIG, url_);
        return;
      }
    } else {
      const size_t keep = LinkCardKeep(read_);
      if (keep < read_.size() || keep >= kLinkCardPageCap) {
        read_.resize(keep);
        End(status_, url_, std::move(read_));
        return;
      }
    }
    std::move(resume).Run();
  }
  void OnComplete(bool success) override {
    if (success) {
      End(status_, url_, std::move(read_));
      return;
    }
    const std::optional<network::URLLoaderCompletionStatus>& completion =
        loader_->CompletionStatus();
    if (completion && completion->cors_error_status) {
      switch (completion->cors_error_status->cors_error) {
        case network::mojom::CorsError::kInsecureLocalNetwork:
        case network::mojom::CorsError::kInvalidLocalNetworkAccess:
        case network::mojom::CorsError::kLocalNetworkAccessPermissionDenied:
          End(BC_LINK_CARD_PRIVATE, url_);
          return;
        default:
          break;
      }
    }
    End(loader_->NetError(), url_);
  }
  void OnRetry(base::OnceClosure start_retry) override {}

  // Tells the app what the page or the picture came to. A page told keeps the card, idle, for its
  // picture; anything else ends it.
  void End(int status, const GURL& url, std::string bytes = std::string()) {
    loader_.reset();
    lookup_.reset();
    wait_.Stop();
    limit_.Stop();
    Observe(nullptr);
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&Tell, id_, status, url.is_valid() ? url.spec() : std::string(),
                                  mime_, std::move(bytes)));
    if (!picture_ && status >= 200 && status < 300 && LinkCardPage(type_)) {
      told_page_ = true;
      return;
    }
    // Last: deletes this.
    Current().reset();
  }

  const uint64_t id_;
  const bc_id tab_;
  GURL url_;
  int hops_ = 0;
  bool picture_ = false;
  bool told_page_ = false;
  bool asked_picture_ = false;
  int status_ = 0;
  std::string type_;
  std::string mime_;
  std::string read_;
  base::OneShotTimer wait_;
  base::OneShotTimer limit_;
  NetworkRefuser refuser_;
  mojo::Remote<network::mojom::URLLoaderFactory> factory_;
  mojo::Receiver<network::mojom::ProxyLookupClient> lookup_{this};
  std::unique_ptr<network::SimpleURLLoader> loader_;
  base::WeakPtrFactory<Card> weak_{this};
};

}  // namespace

uint64_t StartLinkCard(bc_id tab) {
  // A new card cancels the one before, told of no more.
  Current().reset();
  content::WebContents* contents = UsersPage(tab);
  const GURL url = contents ? Tab::PointerLink(tab) : GURL();
  if (!LinkCardAsks(url)) {
    return 0;
  }
  Current() = std::make_unique<Card>(++g_last_card, tab, contents, url);
  return g_last_card;
}

void AskLinkCardPicture(uint64_t card, const std::string& url) {
  if (Current() && Current()->id() == card) {
    Current()->Picture(GURL(url));
  }
}

void CancelLinkCard(uint64_t card) {
  if (card == g_last_card) {
    g_cancelled_card = card;
  }
  if (Current() && Current()->id() == card) {
    Current().reset();
  }
}

}  // namespace browser_core
