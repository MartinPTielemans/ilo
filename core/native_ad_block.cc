#include "browser_core/native_ad_block.h"

#include <algorithm>
#include <atomic>
#include <cstdint>
#include <map>
#include <memory>
#include <optional>
#include <string>
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
#include "base/strings/strcat.h"
#include "base/task/sequenced_task_runner.h"
#include "base/task/thread_pool.h"
#include "base/values.h"
#include "browser_core/ad_block_page.mojom.h"
#include "browser_core/host.h"
#include "browser_core/public/browser_core.h"
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
#include "mojo/public/cpp/bindings/receiver_set.h"
#include "mojo/public/cpp/bindings/remote.h"
#include "mojo/public/cpp/system/data_pipe.h"
#include "net/base/data_url.h"
#include "net/base/net_errors.h"
#include "net/http/http_response_headers.h"
#include "services/network/public/cpp/resource_request.h"
#include "services/network/public/cpp/url_loader_completion_status.h"
#include "services/network/public/cpp/url_loader_factory_builder.h"
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

// What the app's engine does with |request|, and the address its answer writes into |answer|.
int Ask(const network::ResourceRequest& request, std::string* answer) {
  const bc_host& host = Host();
  // A page itself always loads: uBlock Origin's warning in place of a blocked one is yet to come.
  if (!g_on.load(std::memory_order_relaxed) || !host.ad_block_request ||
      !request.url.SchemeIsHTTPOrHTTPS() ||
      request.destination == network::mojom::RequestDestination::kDocument) {
    return BC_AD_BLOCK_ALLOW;
  }
  const std::string source =
      request.request_initiator ? request.request_initiator->GetURL().spec() : std::string();
  return host.ad_block_request(host.context, request.url.spec().c_str(), source.c_str(),
                               TypeOf(request), &Append, answer);
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

// Between a factory's requests and Chrome's factory, on Sequence(): each goes to the app's engine
// first. One allowed goes on as it came, with nothing of ours in its way once it's started.
class Proxy : public network::mojom::URLLoaderFactory {
 public:
  Proxy(const Proxy&) = delete;
  Proxy& operator=(const Proxy&) = delete;

  // Owns itself, until both ends have gone.
  static void Start(mojo::PendingReceiver<network::mojom::URLLoaderFactory> receiver,
                    mojo::PendingRemote<network::mojom::URLLoaderFactory> target) {
    new Proxy(std::move(receiver), std::move(target));
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
        Block(std::move(client));
        return;
      case BC_AD_BLOCK_REDIRECT:
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
        mojo::PendingRemote<network::mojom::URLLoaderFactory> target)
      : target_(std::move(target)) {
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
};

// What a page hides and runs from its start (ad_block_page.mojom).
struct Page {
  std::string css;
  std::string script;
  // Whether the page's generic rules apply, and those of them it doesn't hide, as a JSON array for
  // ad_block_hide.
  bool generic = false;
  std::string exceptions = "[]";
};

// The rules that hide |selectors|, a JSON array of them, appended to |css|.
void AppendRules(const base::ListValue& selectors, std::string& css) {
  for (const base::Value& selector : selectors) {
    // A rule each, so one the page's engine can't read drops alone, and none that could end its
    // rule or start a comment over the next.
    const std::string* text = selector.GetIfString();
    if (text && text->find_first_of("{}") == std::string::npos &&
        text->find("/*") == std::string::npos) {
      base::StrAppend(&css, {*text, "{display:none!important}\n"});
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
  if (const std::string* script = fields->FindString("script")) {
    page.script = *script;
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
    if (page.css.empty() && page.script.empty() && !page.generic) {
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
    agent->UseForNextDocument(navigation->GetURL().spec(), page.css, page.script,
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

// Asks the app's engine what the page a navigation loads hides and runs as its request starts, so
// the answer is in by its response, which waits for it only if it isn't.
class PageThrottle : public content::NavigationThrottle {
 public:
  using content::NavigationThrottle::NavigationThrottle;

  // content::NavigationThrottle:
  ThrottleCheckResult WillStartRequest() override {
    Ask();
    return PROCEED;
  }
  ThrottleCheckResult WillRedirectRequest() override {
    Ask();
    return PROCEED;
  }
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
  void Ask() {
    page_.reset();
    Sequence()->PostTaskAndReplyWithResult(
        FROM_HERE, base::BindOnce(&PageOf, navigation_handle()->GetURL()),
        base::BindOnce(&PageThrottle::Answer, weak_factory_.GetWeakPtr(), ++asked_));
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

void ProxyForNativeAdBlock(network::URLLoaderFactoryBuilder& factory_builder) {
  auto [receiver, target] = factory_builder.Append();
  Sequence()->PostTask(FROM_HERE,
                       base::BindOnce(&Proxy::Start, std::move(receiver), std::move(target)));
}

void AddNativeAdBlockPage(content::NavigationThrottleRegistry& registry) {
  registry.AddThrottle(std::make_unique<PageThrottle>(registry));
}

}  // namespace browser_core
