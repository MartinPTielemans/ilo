#include "browser_core/native_ad_block.h"

#include <algorithm>
#include <atomic>
#include <cstdint>
#include <functional>
#include <iterator>
#include <map>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "base/byte_size.h"
#include "base/command_line.h"
#include "base/containers/span.h"
#include "base/files/file_path.h"
#include "base/functional/bind.h"
#include "base/json/json_reader.h"
#include "base/json/json_writer.h"
#include "base/location.h"
#include "base/memory/scoped_refptr.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/strings/escape.h"
#include "base/strings/strcat.h"
#include "base/strings/string_split.h"
#include "base/strings/string_util.h"
#include "base/supports_user_data.h"
#include "base/task/sequenced_task_runner.h"
#include "base/task/thread_pool.h"
#include "base/time/time.h"
#include "base/values.h"
#include "browser_core/ad_block.h"
#include "browser_core/ad_block_page.mojom.h"
#include "browser_core/agent_autofill.h"
#include "browser_core/host.h"
#include "browser_core/public/browser_core.h"
#include "browser_core/space.h"
#include "browser_core/tab.h"
#include "content/public/browser/browser_context.h"
#include "content/public/browser/browser_task_traits.h"
#include "content/public/browser/browser_thread.h"
#include "content/public/browser/global_routing_id.h"
#include "content/public/browser/navigation_controller.h"
#include "content/public/browser/navigation_handle.h"
#include "content/public/browser/navigation_throttle.h"
#include "content/public/browser/navigation_throttle_registry.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/web_contents.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/web_contents_user_data.h"
#include "mojo/public/cpp/bindings/associated_remote.h"
#include "mojo/public/cpp/bindings/pending_receiver.h"
#include "mojo/public/cpp/bindings/pending_remote.h"
#include "mojo/public/cpp/bindings/receiver.h"
#include "mojo/public/cpp/bindings/receiver_set.h"
#include "mojo/public/cpp/bindings/remote.h"
#include "mojo/public/cpp/system/data_pipe.h"
#include "net/base/data_url.h"
#include "net/base/net_errors.h"
#include "net/http/http_response_headers.h"
#include "net/url_request/redirect_info.h"
#include "services/metrics/public/cpp/ukm_source_id.h"
#include "services/network/public/cpp/resource_request.h"
#include "services/network/public/cpp/url_loader_completion_status.h"
#include "services/network/public/cpp/url_loader_factory_builder.h"
#include "services/network/public/mojom/early_hints.mojom.h"
#include "services/network/public/mojom/fetch_api.mojom.h"
#include "services/network/public/mojom/url_loader.mojom.h"
#include "services/network/public/mojom/url_loader_factory.mojom.h"
#include "services/network/public/mojom/url_response_head.mojom.h"
#include "third_party/blink/public/common/associated_interfaces/associated_interface_provider.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

// --ad-block-engine=native.
constexpr char kEngineSwitch[] = "ad-block-engine";
constexpr char kNative[] = "native";

// Settings' Ad blocking, read on the sequence as the main thread changes it.
std::atomic<bool> g_on{true};

// Where every request is matched: one sequence for the browser, never the main thread, as each
// takes about 10 µs. It may block, for the moment after launch while the app's engine loads.
scoped_refptr<base::SequencedTaskRunner> Sequence() {
  static base::NoDestructor<scoped_refptr<base::SequencedTaskRunner>> sequence(
      base::ThreadPool::CreateSequencedTaskRunner(
          {base::MayBlock(), base::TaskPriority::USER_BLOCKING,
           base::TaskShutdownBehavior::SKIP_ON_SHUTDOWN}));
  return *sequence;
}

// bc_writer, into a std::string.
void Append(void* out, const char* text, size_t size) {
  static_cast<std::string*>(out)->append(text, size);
}

// The request's type, as a filter's options name it.
const char* TypeOf(const network::ResourceRequest& request) {
  using network::mojom::RequestDestination;
  switch (request.destination) {
    case RequestDestination::kDocument:
      return "document";
    case RequestDestination::kFrame:
    case RequestDestination::kIframe:
    case RequestDestination::kFencedframe:
      return "subdocument";
    case RequestDestination::kScript:
    case RequestDestination::kWorker:
    case RequestDestination::kSharedWorker:
    case RequestDestination::kServiceWorker:
    case RequestDestination::kAudioWorklet:
    case RequestDestination::kPaintWorklet:
      return "script";
    case RequestDestination::kStyle:
      return "stylesheet";
    case RequestDestination::kImage:
      return "image";
    case RequestDestination::kFont:
      return "font";
    case RequestDestination::kAudio:
    case RequestDestination::kVideo:
    case RequestDestination::kTrack:
      return "media";
    case RequestDestination::kObject:
    case RequestDestination::kEmbed:
      return "object";
    // fetch(), XMLHttpRequest and navigator.sendBeacon, which keeps its request alive.
    case RequestDestination::kEmpty:
      return request.keepalive ? "ping" : "xmlhttprequest";
    default:
      return "other";
  }
}

// The page a request comes from, as the app's engine reads it.
std::string SourceOf(const network::ResourceRequest& request) {
  return request.request_initiator ? request.request_initiator->GetURL().spec() : std::string();
}

// What the app's engine does with the request for |url| of |type| from the page at |source|, and
// the address or header names its answer writes into |answer|.
int Ask(const GURL& url, const std::string& source, const char* type, std::string* answer) {
  const bc_host& host = Host();
  if (!g_on.load(std::memory_order_relaxed) || !host.ad_block_request ||
      !url.SchemeIsHTTPOrHTTPS()) {
    return BC_AD_BLOCK_ALLOW;
  }
  return host.ad_block_request(host.context, url.spec().c_str(), source.c_str(), type, &Append,
                               answer);
}

// What the app's engine does with |request|.
int Ask(const network::ResourceRequest& request, std::string* answer) {
  // A page itself was asked about before its request started, by its navigation (PageThrottle).
  if (request.destination == network::mojom::RequestDestination::kDocument) {
    return BC_AD_BLOCK_ALLOW;
  }
  return Ask(request.url, SourceOf(request), TypeOf(request), answer);
}

// How many pages of a tab it counts for at a time: the page it shows and those the back-forward
// cache keeps, six at most by default.
constexpr size_t kPagesCounted = 8;

// The longest Site Controls waits for a count that changed, so a page's burst of blocked requests
// wakes the main thread once a second, not at each.
constexpr base::TimeDelta kCountEvery = base::Seconds(1);

// The requests it blocked on a page, by its UKM source id. On Sequence().
struct Counted {
  int64_t page = ukm::kInvalidSourceId;
  int blocked = 0;
};

// What it blocked on each open tab's last few pages, the latest last, by CountedTab's id. On
// Sequence().
std::map<int, std::vector<Counted>>& Counts() {
  static base::NoDestructor<std::map<int, std::vector<Counted>>> counts;
  return *counts;
}

// The page Site Controls watches, if any. On Sequence().
struct Watched {
  int tab = 0;
  int64_t page = ukm::kInvalidSourceId;
  // Which of the main thread's watches it is, so a count sent for one before the next is dropped.
  int watch = 0;
  base::TimeTicks sent;
  bool waiting = false;
};
Watched& Watching() {
  static Watched watched;
  return watched;
}

// What the main thread hears the count with, and which watch that is. On the main thread.
base::RepeatingCallback<void(int)>& Watcher() {
  static base::NoDestructor<base::RepeatingCallback<void(int)>> watcher;
  return *watcher;
}
int g_watch = 0;

// On the main thread.
void Heard(int watch, int blocked) {
  if (watch == g_watch && Watcher()) {
    Watcher().Run(blocked);
  }
}

// Sends the watched page's count to the main thread. On Sequence().
void SendCount() {
  Watched& watched = Watching();
  if (!watched.tab) {
    return;
  }
  int blocked = 0;
  if (const auto tab = Counts().find(watched.tab); tab != Counts().end()) {
    if (const auto it = std::ranges::find(tab->second, watched.page, &Counted::page);
        it != tab->second.end()) {
      blocked = it->blocked;
    }
  }
  watched.sent = base::TimeTicks::Now();
  content::GetUIThreadTaskRunner({})->PostTask(FROM_HERE,
                                               base::BindOnce(&Heard, watched.watch, blocked));
}

// Watches |page| of |tab|, or nothing for 0. On Sequence().
void Watch(int tab, int64_t page, int watch) {
  Watched& watched = Watching();
  watched.tab = tab;
  watched.page = page;
  watched.watch = watch;
  SendCount();
}

// Counts a request blocked on |page| of |tab|, and sends the count if Site Controls watches it:
// now if it last did a while ago, else once kCountEvery has passed since. On Sequence().
void CountBlocked(int tab, int64_t page) {
  if (!tab || page == ukm::kInvalidSourceId) {
    return;
  }
  // A request that ends after its tab closed counts for none.
  const auto counts = Counts().find(tab);
  if (counts == Counts().end()) {
    return;
  }
  std::vector<Counted>& pages = counts->second;
  auto it = std::ranges::find(pages, page, &Counted::page);
  if (it == pages.end()) {
    if (pages.size() == kPagesCounted) {
      pages.erase(pages.begin());
    }
    pages.push_back(Counted{.page = page});
    it = std::prev(pages.end());
  } else {
    // A page shown again from the back-forward cache is the latest.
    it = std::rotate(it, it + 1, pages.end());
  }
  ++it->blocked;
  Watched& watched = Watching();
  if (watched.tab != tab || watched.page != page || watched.waiting) {
    return;
  }
  const base::TimeDelta wait = watched.sent + kCountEvery - base::TimeTicks::Now();
  if (!wait.is_positive()) {
    SendCount();
    return;
  }
  watched.waiting = true;
  Sequence()->PostDelayedTask(FROM_HERE, base::BindOnce([] {
                                Watching().waiting = false;
                                SendCount();
                              }),
                              wait);
}

// Gives a tab the id its counts are kept by, from before its first proxy starts, and lets go of
// them as it closes.
class CountedTab : public content::WebContentsUserData<CountedTab> {
 public:
  ~CountedTab() override {
    Sequence()->PostTask(FROM_HERE, base::BindOnce([](int id) { Counts().erase(id); }, id_));
  }

  int id() const { return id_; }

 private:
  friend WebContentsUserData;
  explicit CountedTab(content::WebContents* contents)
      : WebContentsUserData(*contents), id_(++next_id_) {
    Sequence()->PostTask(FROM_HERE, base::BindOnce([](int id) { Counts().try_emplace(id); }, id_));
  }

  static inline int next_id_ = 0;
  const int id_;

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

WEB_CONTENTS_USER_DATA_KEY_IMPL(CountedTab);

// The id |contents|' counts are kept by.
int CountedTabOf(content::WebContents* contents) {
  CountedTab::CreateForWebContents(contents);
  return CountedTab::FromWebContents(contents)->id();
}

// Fails a request as blocked.
void Block(mojo::PendingRemote<network::mojom::URLLoaderClient> client) {
  mojo::Remote<network::mojom::URLLoaderClient>(std::move(client))
      ->OnComplete(network::URLLoaderCompletionStatus(net::ERR_BLOCKED_BY_CLIENT));
}

// Answers a request with the stand-in |url|, a data: URL, or fails it as blocked without one. It
// comes from no server, so any page may read it.
void StandIn(const GURL& url, mojo::PendingRemote<network::mojom::URLLoaderClient> pending) {
  std::string mime_type;
  std::string charset;
  std::string data;
  mojo::ScopedDataPipeProducerHandle producer;
  mojo::ScopedDataPipeConsumerHandle consumer;
  if (!net::DataURL::Parse(url, &mime_type, &charset, &data) ||
      mojo::CreateDataPipe(static_cast<uint32_t>(std::max<size_t>(data.size(), 1)), producer,
                           consumer) != MOJO_RESULT_OK ||
      (!data.empty() && producer->WriteAllData(base::as_byte_span(data)) != MOJO_RESULT_OK)) {
    Block(std::move(pending));
    return;
  }
  producer.reset();
  auto head = network::mojom::URLResponseHead::New();
  head->mime_type = mime_type;
  head->charset = charset;
  head->content_length = static_cast<int64_t>(data.size());
  head->headers = net::HttpResponseHeaders::TryToCreate(base::StrCat(
      {"HTTP/1.1 200 OK\nContent-Type: ", mime_type, "\nAccess-Control-Allow-Origin: *\n\n"}));
  mojo::Remote<network::mojom::URLLoaderClient> client(std::move(pending));
  client->OnReceiveResponse(std::move(head), std::move(consumer), std::nullopt);
  network::URLLoaderCompletionStatus status(net::OK);
  status.decoded_body_length = base::ByteSize(data.size());
  client->OnComplete(status);
}

// Between the network and a request's client, on Sequence(), for a request the app's engine
// answered BC_AD_BLOCK_AT_RESPONSE: asks it about each response, a redirect's too, before any of
// it goes on, and fails the request as blocked where it says so. The page's loader talks to the
// network's as it would, so pausing, priorities and following a redirect are untouched.
class AtResponse : public network::mojom::URLLoaderClient {
 public:
  AtResponse(const AtResponse&) = delete;
  AtResponse& operator=(const AtResponse&) = delete;

  // The client for the network to answer, in place of |client|, for |request|, whose answer
  // |names| the headers to ask with, one a line. What it blocks counts for |page| of |tab|, as
  // the proxy's does. Owns itself, until either end has gone.
  static mojo::PendingRemote<network::mojom::URLLoaderClient> Watch(
      const network::ResourceRequest& request,
      const std::string& names,
      mojo::PendingRemote<network::mojom::URLLoaderClient> client,
      int tab,
      int64_t page) {
    mojo::PendingRemote<network::mojom::URLLoaderClient> watched;
    new AtResponse(request, names, watched.InitWithNewPipeAndPassReceiver(), std::move(client),
                   tab, page);
    return watched;
  }

  // network::mojom::URLLoaderClient:
  void OnReceiveEarlyHints(network::mojom::EarlyHintsPtr early_hints) override {
    client_->OnReceiveEarlyHints(std::move(early_hints));
  }
  void OnReceiveResponse(network::mojom::URLResponseHeadPtr head,
                         mojo::ScopedDataPipeConsumerHandle body,
                         std::optional<mojo_base::BigBuffer> cached_metadata) override {
    if (Blocks(*head)) {
      Fail();
      return;
    }
    client_->OnReceiveResponse(std::move(head), std::move(body), std::move(cached_metadata));
  }
  void OnReceiveRedirect(const net::RedirectInfo& redirect_info,
                         network::mojom::URLResponseHeadPtr head) override {
    if (Blocks(*head)) {
      Fail();
      return;
    }
    // uBlock Origin asks again about the address a redirect goes to.
    std::string answer;
    switch (Ask(redirect_info.new_url, source_, type_, &answer)) {
      case BC_AD_BLOCK_BLOCK:
      case BC_AD_BLOCK_REDIRECT:
        Fail();
        return;
      case BC_AD_BLOCK_AT_RESPONSE:
        names_ = Names(answer);
        break;
      default:
        asking_ = false;
        break;
    }
    url_ = redirect_info.new_url;
    method_ = redirect_info.new_method;
    client_->OnReceiveRedirect(redirect_info, std::move(head));
  }
  void OnUploadProgress(int64_t current_position,
                        int64_t total_size,
                        OnUploadProgressCallback callback) override {
    client_->OnUploadProgress(current_position, total_size, std::move(callback));
  }
  void OnTransferSizeUpdated(int32_t transfer_size_diff) override {
    client_->OnTransferSizeUpdated(transfer_size_diff);
  }
  void OnComplete(const network::URLLoaderCompletionStatus& status) override {
    client_->OnComplete(status);
    delete this;
  }

 private:
  AtResponse(const network::ResourceRequest& request,
             const std::string& names,
             mojo::PendingReceiver<network::mojom::URLLoaderClient> receiver,
             mojo::PendingRemote<network::mojom::URLLoaderClient> client,
             int tab,
             int64_t page)
      : tab_(tab),
        page_(page),
        source_(SourceOf(request)),
        type_(TypeOf(request)),
        url_(request.url),
        method_(request.method),
        names_(Names(names)),
        receiver_(this, std::move(receiver)),
        client_(std::move(client)) {
    receiver_.set_disconnect_handler(base::BindOnce(&AtResponse::Gone, base::Unretained(this)));
    client_.set_disconnect_handler(base::BindOnce(&AtResponse::Gone, base::Unretained(this)));
  }
  ~AtResponse() override = default;

  static std::vector<std::string> Names(const std::string& names) {
    return base::SplitString(names, "\n", base::TRIM_WHITESPACE, base::SPLIT_WANT_NONEMPTY);
  }

  // Whether the app's engine blocks the response |head| to the request for url_. It's handed the
  // first value of each header names_ has, as Chromium's webRequest lists them, comma-separated
  // values apart, and nothing else of the response. None of it is kept.
  bool Blocks(const network::mojom::URLResponseHead& head) const {
    const bc_host& host = Host();
    if (!asking_ || !g_on.load(std::memory_order_relaxed) || !host.ad_block_response) {
      return false;
    }
    std::string headers;
    if (head.headers) {
      for (const std::string& wanted : names_) {
        size_t iter = 0;
        std::string name;
        std::string value;
        while (head.headers->EnumerateHeaderLines(&iter, &name, &value)) {
          if (base::EqualsCaseInsensitiveASCII(name, wanted)) {
            base::StrAppend(&headers, {wanted, ": ", value, "\n"});
            break;
          }
        }
      }
    }
    return host.ad_block_response(host.context, url_.spec().c_str(), source_.c_str(), type_,
                                  method_.c_str(), headers.c_str()) == BC_AD_BLOCK_BLOCK;
  }

  // Fails the request as blocked before any of the response reaches the page, and drops the
  // network's end, which cancels its load.
  void Fail() {
    CountBlocked(tab_, page_);
    client_->OnComplete(network::URLLoaderCompletionStatus(net::ERR_BLOCKED_BY_CLIENT));
    delete this;
  }
  void Gone() { delete this; }

  const int tab_;
  const int64_t page_;
  const std::string source_;
  // TypeOf's, a string of its own.
  const char* const type_;
  GURL url_;
  std::string method_;
  std::vector<std::string> names_;
  // Until a redirect goes to an address the app's engine lets go as it is.
  bool asking_ = true;
  mojo::Receiver<network::mojom::URLLoaderClient> receiver_;
  mojo::Remote<network::mojom::URLLoaderClient> client_;
};

// Between a factory's requests and Chrome's factory, on Sequence(): each goes to the app's engine
// first. One allowed goes on as it came, with nothing of ours in its way once it's started, but
// one asked about again at its responses.
class Proxy : public network::mojom::URLLoaderFactory {
 public:
  Proxy(const Proxy&) = delete;
  Proxy& operator=(const Proxy&) = delete;

  // Owns itself, until both ends have gone. What it blocks counts for |page| of |tab|, if any.
  static void Start(mojo::PendingReceiver<network::mojom::URLLoaderFactory> receiver,
                    mojo::PendingRemote<network::mojom::URLLoaderFactory> target,
                    int tab,
                    int64_t page) {
    new Proxy(std::move(receiver), std::move(target), tab, page);
  }

  // network::mojom::URLLoaderFactory:
  void CreateLoaderAndStart(
      mojo::PendingReceiver<network::mojom::URLLoader> loader,
      int32_t request_id,
      uint32_t options,
      const network::ResourceRequest& request,
      mojo::PendingRemote<network::mojom::URLLoaderClient> client,
      const net::MutableNetworkTrafficAnnotationTag& traffic_annotation) override {
    std::string answer;
    switch (Ask(request, &answer)) {
      case BC_AD_BLOCK_BLOCK:
        CountBlocked(tab_, page_);
        Block(std::move(client));
        return;
      // A stand-in is blocked too, as uBlock Origin counts it.
      case BC_AD_BLOCK_REDIRECT:
        CountBlocked(tab_, page_);
        StandIn(GURL(answer), std::move(client));
        return;
      case BC_AD_BLOCK_REWRITE:
        if (const GURL to(answer); to.is_valid() && to.SchemeIsHTTPOrHTTPS()) {
          network::ResourceRequest rewritten(request);
          rewritten.url = to;
          target_->CreateLoaderAndStart(std::move(loader), request_id, options, rewritten,
                                        std::move(client), traffic_annotation);
          return;
        }
        break;
      case BC_AD_BLOCK_AT_RESPONSE:
        target_->CreateLoaderAndStart(std::move(loader), request_id, options, request,
                                      AtResponse::Watch(request, answer, std::move(client),
                                                        tab_, page_),
                                      traffic_annotation);
        return;
      default:
        break;
    }
    target_->CreateLoaderAndStart(std::move(loader), request_id, options, request,
                                  std::move(client), traffic_annotation);
  }
  void Clone(mojo::PendingReceiver<network::mojom::URLLoaderFactory> receiver) override {
    receivers_.Add(this, std::move(receiver));
  }

 private:
  Proxy(mojo::PendingReceiver<network::mojom::URLLoaderFactory> receiver,
        mojo::PendingRemote<network::mojom::URLLoaderFactory> target,
        int tab,
        int64_t page)
      : target_(std::move(target)), tab_(tab), page_(page) {
    receivers_.Add(this, std::move(receiver));
    receivers_.set_disconnect_handler(
        base::BindRepeating(&Proxy::OnProxyGone, base::Unretained(this)));
    target_.set_disconnect_handler(base::BindOnce(&Proxy::OnTargetGone, base::Unretained(this)));
  }
  ~Proxy() override = default;

  void OnProxyGone() {
    if (receivers_.empty()) {
      delete this;
    }
  }
  void OnTargetGone() { delete this; }

  mojo::ReceiverSet<network::mojom::URLLoaderFactory> receivers_;
  mojo::Remote<network::mojom::URLLoaderFactory> target_;
  const int tab_;
  const int64_t page_;
};

// What a page hides and runs from its start (ad_block_page.mojom).
struct Page {
  std::string css;
  std::string script;
  std::string isolated;
  // Whether the page's generic rules apply, and those of them it doesn't hide, as a JSON array for
  // ad_block_hide.
  bool generic = false;
  std::string exceptions = "[]";
};

// Whether |text| can be part of one rule: nothing that could end it or start a comment over the
// next.
bool InOneRule(const std::string* text) {
  return text && text->find_first_of("{}") == std::string::npos &&
         text->find("/*") == std::string::npos;
}

// The rules that hide |selectors|, a JSON array of them, appended to |css|.
void AppendRules(const base::ListValue& selectors, std::string& css) {
  for (const base::Value& selector : selectors) {
    // A rule each, so one the page's engine can't read drops alone.
    const std::string* text = selector.GetIfString();
    if (InOneRule(text)) {
      base::StrAppend(&css, {*text, "{display:none!important}\n"});
    }
  }
}

// The rules of |styles|, a JSON array of selectors each with its declarations, appended to |css|.
void AppendStyles(const base::ListValue& styles, std::string& css) {
  for (const base::Value& style : styles) {
    const base::ListValue* pair = style.GetIfList();
    if (!pair || pair->size() != 2) {
      continue;
    }
    const std::string* selector = (*pair)[0].GetIfString();
    const std::string* declarations = (*pair)[1].GetIfString();
    if (InOneRule(selector) && InOneRule(declarations)) {
      base::StrAppend(&css, {*selector, "{", *declarations, "}\n"});
    }
  }
}

// What the app's engine has the page at |url| hide and run, on Sequence().
Page PageOf(const GURL& url) {
  Page page;
  const bc_host& host = Host();
  if (!g_on.load(std::memory_order_relaxed) || !host.ad_block_page) {
    return page;
  }
  std::string answer;
  host.ad_block_page(host.context, url.spec().c_str(), &Append, &answer);
  std::optional<base::DictValue> fields = base::JSONReader::ReadDict(answer, base::JSON_PARSE_RFC);
  if (!fields) {
    return page;
  }
  if (const base::ListValue* hide = fields->FindList("hide")) {
    AppendRules(*hide, page.css);
  }
  if (const base::ListValue* styles = fields->FindList("styles")) {
    AppendStyles(*styles, page.css);
  }
  if (const std::string* script = fields->FindString("script")) {
    page.script = *script;
  }
  if (const std::string* isolated = fields->FindString("isolated")) {
    page.isolated = *isolated;
  }
  page.generic = fields->FindBool("generic").value_or(false) && host.ad_block_hide;
  if (const base::ListValue* exceptions = fields->FindList("exceptions")) {
    page.exceptions = base::WriteJson(*exceptions).value_or("[]");
  }
  return page;
}

// The rules that hide the elements with these |classes| and |ids| on a page whose generic rules
// don't hide its |exceptions|, on Sequence().
std::string HideOf(const std::vector<std::string>& classes,
                   const std::vector<std::string>& ids,
                   const std::string& exceptions) {
  std::string css;
  const bc_host& host = Host();
  if (!g_on.load(std::memory_order_relaxed) || !host.ad_block_hide) {
    return css;
  }
  const auto json = [](const std::vector<std::string>& names) {
    base::ListValue list;
    for (const std::string& name : names) {
      list.Append(name);
    }
    return base::WriteJson(list).value_or("[]");
  };
  std::string answer;
  host.ad_block_hide(host.context, json(classes).c_str(), json(ids).c_str(), exceptions.c_str(),
                     &Append, &answer);
  if (std::optional<base::Value> selectors = base::JSONReader::Read(answer, base::JSON_PARSE_RFC);
      selectors && selectors->is_list()) {
    AppendRules(selectors->GetList(), css);
  }
  return css;
}

// At most this many of a page's classes, and as many ids, in one ask, each at most this long:
// more than a page has, so a renderer can't have the engine match without end.
constexpr size_t kMostNames = 4096;
constexpr size_t kLongestName = 256;

// |names| as the renderer sent them, but those too long, and no more than kMostNames.
std::vector<std::string> Bounded(std::vector<std::string> names) {
  std::erase_if(names, [](const std::string& name) { return name.size() > kLongestName; });
  if (names.size() > kMostNames) {
    names.resize(kMostNames);
  }
  return names;
}

// Sends the frame each page of a tab commits in what it hides and runs, as it's ready to commit,
// so it's there before the page's first script, and answers its generic rules by class and id.
class PageSender : public content::WebContentsObserver,
                   public content::WebContentsUserData<PageSender>,
                   public mojom::AdBlockHide {
 public:
  PageSender(const PageSender&) = delete;
  PageSender& operator=(const PageSender&) = delete;
  ~PageSender() override = default;

  // What the page of the navigation |navigation| hides and runs.
  void Keep(int64_t navigation, Page page) { pages_[navigation] = std::move(page); }

  // content::WebContentsObserver:
  void ReadyToCommitNavigation(content::NavigationHandle* navigation) override {
    auto it = pages_.find(navigation->GetNavigationId());
    if (it == pages_.end()) {
      return;
    }
    Page page = std::move(it->second);
    pages_.erase(it);
    if (page.css.empty() && page.script.empty() && page.isolated.empty() && !page.generic) {
      return;
    }
    // The document's own, which goes when the agent drops it for the next document.
    mojo::PendingRemote<mojom::AdBlockHide> hide;
    if (page.generic) {
      hides_.Add(this, hide.InitWithNewPipeAndPassReceiver(), std::move(page.exceptions));
    }
    // On the frame's own channel, as the commit is, so it comes first.
    mojo::AssociatedRemote<mojom::AdBlockPage> agent;
    navigation->GetRenderFrameHost()->GetRemoteAssociatedInterfaces()->GetInterface(&agent);
    agent->UseForNextDocument(navigation->GetURL().spec(), page.css, page.script, page.isolated,
                              std::move(hide));
  }
  // One that doesn't commit, such as a download.
  void DidFinishNavigation(content::NavigationHandle* navigation) override {
    pages_.erase(navigation->GetNavigationId());
  }

 private:
  friend class content::WebContentsUserData<PageSender>;

  explicit PageSender(content::WebContents* contents)
      : content::WebContentsObserver(contents),
        content::WebContentsUserData<PageSender>(*contents) {}

  // mojom::AdBlockHide:
  void Hide(const std::vector<std::string>& classes,
            const std::vector<std::string>& ids,
            HideCallback answer) override {
    Sequence()->PostTaskAndReplyWithResult(
        FROM_HERE,
        base::BindOnce(&HideOf, Bounded(classes), Bounded(ids), hides_.current_context()),
        std::move(answer));
  }

  std::map<int64_t, Page> pages_;
  // Each document's, with its page's exceptions.
  mojo::ReceiverSet<mojom::AdBlockHide, std::string> hides_;

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

WEB_CONTENTS_USER_DATA_KEY_IMPL(PageSender);

// Whether the app's engine blocks the page at |url| itself, as uBlock Origin's strict blocking
// does: a filter for its documents, or one for its host alone. Asked as the page's own request,
// first-party, as uBlock Origin asks for a tab's. On Sequence().
bool BlocksPage(const GURL& url) {
  const bc_host& host = Host();
  if (!g_on.load(std::memory_order_relaxed) || !host.ad_block_request) {
    return false;
  }
  std::string answer;
  const int verdict = host.ad_block_request(host.context, url.spec().c_str(), url.spec().c_str(),
                                            "document", &Append, &answer);
  return verdict == BC_AD_BLOCK_BLOCK || verdict == BC_AD_BLOCK_REDIRECT;
}

// What a tab's page gets.
enum class Decision {
  kLoad,
  // The warning in its place: a list names the page.
  kWarn,
  // Its tab closed: a pop-up filter names the page another page opened, as uBlock Origin closes it.
  kClose,
};

// Of the page at |url|, in a tab |opener|'s page opened, if it did, and the warning if it |warns|.
// On Sequence().
Decision Decide(const GURL& url, const std::optional<GURL>& opener, bool warns) {
  const bc_host& host = Host();
  if (opener && g_on.load(std::memory_order_relaxed) && host.ad_block_request) {
    std::string answer;
    if (host.ad_block_request(host.context, url.spec().c_str(), opener->spec().c_str(), "popup",
                              &Append, &answer) == BC_AD_BLOCK_BLOCK) {
      return Decision::kClose;
    }
  }
  return warns && BlocksPage(url) ? Decision::kWarn : Decision::kLoad;
}

// How long Open anyway lets a host through from its last page, as uBlock Origin's
// strictBlockingBypassDuration.
constexpr base::TimeDelta kLetThrough = base::Seconds(120);

// The hosts the user let through a profile's warnings, and until when: each page there moves it
// on, as uBlock Origin's strictBlockBypasser. Read only as a page starts, so it sets no timer.
class LetThrough : public base::SupportsUserData::Data {
 public:
  static LetThrough& For(content::BrowserContext* context) {
    auto* through = static_cast<LetThrough*>(context->GetUserData(kKey));
    if (!through) {
      auto made = std::make_unique<LetThrough>();
      through = made.get();
      context->SetUserData(kKey, std::move(made));
    }
    return *through;
  }

  void Let(std::string_view host) {
    until_[std::string(host)] = base::TimeTicks::Now() + kLetThrough;
  }

  // Whether |host|, or a domain it's under, is let through.
  bool Lets(std::string_view host) {
    const base::TimeTicks now = base::TimeTicks::Now();
    std::erase_if(until_, [now](const auto& entry) { return entry.second <= now; });
    for (;;) {
      if (auto it = until_.find(host); it != until_.end()) {
        it->second = now + kLetThrough;
        return true;
      }
      const size_t dot = host.find('.');
      if (dot == std::string_view::npos) {
        return false;
      }
      host.remove_prefix(dot + 1);
    }
  }

 private:
  static constexpr char kKey[] = "browser_core::AdBlockLetThrough";
  std::map<std::string, base::TimeTicks, std::less<>> until_;
};

// The address a tab's warning was shown for last, which only that warning opens.
struct Warned : public content::WebContentsUserData<Warned> {
  explicit Warned(content::WebContents* contents)
      : content::WebContentsUserData<Warned>(*contents) {}
  GURL url;
  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

WEB_CONTENTS_USER_DATA_KEY_IMPL(Warned);

// The page a blocked page shows in its place, in the look of the core's error page (error_page.cc):
// its host, its address, the way back where there is one, and Open anyway, which lets its host
// through for kLetThrough, as uBlock Origin's warning does. Only the user |can_open| it: an agent's
// tab has no Open anyway, as an agent never gets past a page the user's lists block.
std::string WarningHtml(const GURL& url, bool can_open) {
  const std::string host = base::EscapeForHTML(url.host());
  const std::string address = base::EscapeForHTML(url.spec());
  return base::StrCat({R"html(<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Page blocked</title>
<style>
/* error_page.cc's: white words on the pane's colour, or in Light the palette's ink. */
:root { --ink: 255 255 255; }
@media (prefers-color-scheme: light) {
  :root { --ink: 22 20 15; }
}
html, body { height: 100%; margin: 0; background: transparent; }
body {
  display: flex; align-items: center; justify-content: center;
  font: 14px system-ui; color: rgb(var(--ink)); cursor: default; -webkit-user-select: none;
}
main {
  display: flex; flex-direction: column; align-items: center;
  max-width: 400px; padding: 0 24px; text-align: center;
}
h1 { margin: 0; font-size: 14px; font-weight: 500; }
p { margin: 4px 0 0; font-size: 13px; color: rgb(var(--ink) / 0.6); }
b { font-weight: 500; color: rgb(var(--ink)); }
#address {
  margin-top: 12px; max-width: 100%; overflow-wrap: anywhere; font-size: 11px;
  color: rgb(var(--ink) / 0.32); -webkit-user-select: text; cursor: text;
}
#actions { display: flex; gap: 8px; margin-top: 12px; }
button {
  height: 28px; padding: 0 12px; border: 0; border-radius: 6px;
  background: rgb(var(--ink) / 0.1); color: rgb(var(--ink)); font: 500 13px system-ui;
}
button:hover { background: rgb(var(--ink) / 0.12); }
[hidden] { display: none; }
</style>
</head>
<body>
<main>
<h1>Ad blocking stopped this page</h1>
<p><b>)html",
                       host, R"html(</b> is on a filter list in Settings › Ad blocking.</p>
<div id="address">)html",
                       address, R"html(</div>
<div id="actions">
<button id="back" hidden>Go back</button>
)html",
                       can_open ? base::StrCat({"<button id=\"open\" data-to=\"", address,
                                                "\">Open anyway</button>\n"})
                                : std::string(),
                       R"html(</div>
</main>
<script>
const back = document.getElementById('back');
if (history.length > 1) {
  back.hidden = false;
  back.onclick = () => history.back();
  back.focus();
}
// In this entry's place, so Go back from the page goes where the warning would have.
const open = document.getElementById('open');
if (open) {
  open.onclick = () => location.replace(open.dataset.to);
}
</script>
</body>
</html>
)html"});
}

// An agent's tab gets the warning with no Open anyway, and the activity log says why, as it does
// for a device the agent's page asks for: only the user gets past a page their lists block.
void LogAgentWarned(content::WebContents* contents) {
  const Tab* tab = Tab::FromWebContents(contents);
  const Space* space = tab && IsAgentTab(contents) ? Space::Holding(tab->id()) : nullptr;
  if (!space || !space->agent() || !Host().agent_activity) {
    return;
  }
  const bc_agent_activity entry = {
      .agent = space->agent()->id.c_str(),
      .tab = tab->id(),
      .action = "blocked_page",
      .refused = "A filter list in Settings › Ad blocking names this page, and only the user "
                 "can open it anyway, in a tab of their own.",
  };
  Host().agent_activity(Host().context, &entry);
}

// Asks the app's engine what the page a navigation loads hides and runs as its request starts, so
// the answer is in by its response, which waits for it only if it isn't. A tab's page also waits,
// at each address it goes to, to be asked whether the user's lists block it, before its request
// goes out, and shows WarningHtml in its place if they do: uBlock Origin's strict blocking, which
// keeps a badware site from opening at all (#744). It's asked at each address, as uBlock Origin
// sees each request, so a site the user turned it off on never warns, nor hides or runs anything.
class PageThrottle : public content::NavigationThrottle {
 public:
  explicit PageThrottle(content::NavigationThrottleRegistry& registry)
      : content::NavigationThrottle(registry),
        page_of_tab_(!registry.GetNavigationHandle().GetParentFrameOrOuterDocument()) {}

  // content::NavigationThrottle:
  ThrottleCheckResult WillStartRequest() override { return Ask(/*started=*/true); }
  ThrottleCheckResult WillRedirectRequest() override { return Ask(/*started=*/false); }
  ThrottleCheckResult WillProcessResponse() override {
    if (!page_) {
      waiting_ = true;
      return DEFER;
    }
    Keep();
    return PROCEED;
  }
  const char* GetNameForLogging() override { return "browser_core::AdBlockPage"; }

 private:
  // For the navigation's address now, in place of what was asked for one it redirected from.
  ThrottleCheckResult Ask(bool started) {
    page_.reset();
    const int asked = ++asked_;
    content::NavigationHandle* navigation = navigation_handle();
    const GURL& url = navigation->GetURL();
    bool checking = false;
    if (page_of_tab_) {
      if (AdBlockingOffAt(navigation->GetWebContents()->GetBrowserContext(), url)) {
        page_ = Page();
        return PROCEED;
      }
      const std::optional<GURL> opener = Opener();
      const bool warns = !LetThroughNow(url, started);
      checking = opener || warns;
      if (checking) {
        // First on the sequence, so it doesn't wait behind what the page hides.
        Sequence()->PostTaskAndReplyWithResult(
            FROM_HERE, base::BindOnce(&Decide, url, opener, warns),
            base::BindOnce(&PageThrottle::Checked, weak_factory_.GetWeakPtr(), asked));
      }
    }
    Sequence()->PostTaskAndReplyWithResult(
        FROM_HERE, base::BindOnce(&PageOf, url),
        base::BindOnce(&PageThrottle::Answer, weak_factory_.GetWeakPtr(), asked));
    return checking ? DEFER : PROCEED;
  }

  // The address of the page that opened this tab, while the tab has shown nothing else, as uBlock
  // Origin asks its pop-up filters about a tab a page opens, with or without window.opener, and
  // about each address it's sent on to. None from a site the user turned ad blocking off on.
  std::optional<GURL> Opener() {
    content::WebContents* contents = navigation_handle()->GetWebContents();
    if (!contents->GetController().IsInitialNavigation() ||
        !contents->HasLiveOriginalOpenerChain()) {
      return std::nullopt;
    }
    const GURL opener = contents->GetFirstWebContentsInLiveOriginalOpenerChain()
                            ->GetPrimaryMainFrame()
                            ->GetLastCommittedURL();
    if (!opener.SchemeIsHTTPOrHTTPS() ||
        AdBlockingOffAt(contents->GetBrowserContext(), opener)) {
      return std::nullopt;
    }
    return opener;
  }

  // Whether the page at |url| loads without asking: the user let its host through a warning in
  // this profile a moment ago, or does now with Open anyway, at the navigation's |started|. Never
  // in an agent's tab, which neither opens a warning nor uses the user's.
  bool LetThroughNow(const GURL& url, bool started) {
    content::NavigationHandle* navigation = navigation_handle();
    content::WebContents* contents = navigation->GetWebContents();
    if (IsAgentTab(contents)) {
      return false;
    }
    LetThrough& through = LetThrough::For(contents->GetBrowserContext());
    if (started && OpensWarning(url)) {
      through.Let(url.host());
    }
    return through.Lets(url.host());
  }

  // Whether this navigation is the user's click on Open anyway: the tab's warning for |url| shown
  // now, and the navigation its own, with a gesture. A page that opened the tab can't send it
  // on to |url| for the user.
  bool OpensWarning(const GURL& url) {
    content::NavigationHandle* navigation = navigation_handle();
    content::WebContents* contents = navigation->GetWebContents();
    const Warned* warned = Warned::FromWebContents(contents);
    content::RenderFrameHost* page = contents->GetPrimaryMainFrame();
    if (!warned || warned->url != url || !navigation->IsInPrimaryMainFrame() ||
        !navigation->IsRendererInitiated() || !navigation->HasUserGesture() ||
        !navigation->GetInitiatorFrameToken() || !page->IsErrorDocument() ||
        page->GetLastCommittedURL() != url) {
      return false;
    }
    return content::RenderFrameHost::FromFrameToken(content::GlobalRenderFrameHostToken(
               navigation->GetInitiatorProcessId(), *navigation->GetInitiatorFrameToken())) ==
           page;
  }

  void Checked(int asked, Decision decision) {
    if (asked != asked_) {
      return;
    }
    if (decision == Decision::kLoad) {
      Resume();
      return;
    }
    content::WebContents* contents = navigation_handle()->GetWebContents();
    if (decision == Decision::kClose) {
      // After the navigation goes, which closing the tab now would take with it mid-call.
      base::SequencedTaskRunner::GetCurrentDefault()->PostTask(
          FROM_HERE, base::BindOnce(
                         [](base::WeakPtr<content::WebContents> tab) {
                           if (tab) {
                             tab->Close();
                           }
                         },
                         contents->GetWeakPtr()));
      CancelDeferredNavigation(CANCEL_AND_IGNORE);
      return;
    }
    const GURL url = navigation_handle()->GetURL();
    if (navigation_handle()->IsInPrimaryMainFrame()) {
      Warned::CreateForWebContents(contents);
      Warned::FromWebContents(contents)->url = url;
      LogAgentWarned(contents);
    }
    // Last: the navigation, and this with it, can go here.
    CancelDeferredNavigation(
        {CANCEL, net::ERR_BLOCKED_BY_CLIENT, WarningHtml(url, !IsAgentTab(contents))});
  }
  void Answer(int asked, Page page) {
    if (asked != asked_) {
      return;
    }
    page_ = std::move(page);
    if (std::exchange(waiting_, false)) {
      Keep();
      Resume();
    }
  }
  void Keep() {
    content::WebContents* contents = navigation_handle()->GetWebContents();
    PageSender::CreateForWebContents(contents);
    PageSender::FromWebContents(contents)->Keep(navigation_handle()->GetNavigationId(),
                                                std::move(*page_));
  }

  // Whether the navigation is a tab's page, not a frame's, which the frame's page decides for.
  const bool page_of_tab_;
  std::optional<Page> page_;
  int asked_ = 0;
  bool waiting_ = false;
  base::WeakPtrFactory<PageThrottle> weak_factory_{this};
};

}  // namespace

bool NativeAdBlock() {
  static const bool native = base::CommandLine::ForCurrentProcess()->GetSwitchValueASCII(
                                 kEngineSwitch) == kNative;
  return native;
}

void UseNativeAdBlock(const base::FilePath& folder,
                      const std::vector<base::FilePath>& lists,
                      const std::string& filters) {
  const bc_host& host = Host();
  if (!host.ad_block_use) {
    return;
  }
  std::string files;
  for (const base::FilePath& list : lists) {
    base::StrAppend(&files, {list.AsUTF8Unsafe(), "\n"});
  }
  host.ad_block_use(host.context, folder.AsUTF8Unsafe().c_str(), files.c_str(), filters.c_str());
}

void SetNativeAdBlockOn(bool on) {
  g_on.store(on, std::memory_order_relaxed);
}

void ProxyForNativeAdBlock(network::URLLoaderFactoryBuilder& factory_builder,
                           content::RenderFrameHost* frame,
                           int64_t page) {
  // A worker's requests are no tab's, as uBlock Origin counts them.
  content::WebContents* contents =
      frame ? content::WebContents::FromRenderFrameHost(frame) : nullptr;
  const int tab = contents ? CountedTabOf(contents) : 0;
  auto [receiver, target] = factory_builder.Append();
  Sequence()->PostTask(
      FROM_HERE, base::BindOnce(&Proxy::Start, std::move(receiver), std::move(target), tab, page));
}

void WatchNativeAdBlocked(content::WebContents* contents,
                          base::RepeatingCallback<void(int)> blocked) {
  Watcher() = std::move(blocked);
  Sequence()->PostTask(
      FROM_HERE, base::BindOnce(&Watch, CountedTabOf(contents),
                                contents->GetPrimaryMainFrame()->GetPageUkmSourceId(), ++g_watch));
}

void StopWatchingNativeAdBlocked() {
  Watcher().Reset();
  Sequence()->PostTask(FROM_HERE, base::BindOnce(&Watch, 0, ukm::kInvalidSourceId, ++g_watch));
}

void AddNativeAdBlockPage(content::NavigationThrottleRegistry& registry) {
  registry.AddThrottle(std::make_unique<PageThrottle>(registry));
}

}  // namespace browser_core
