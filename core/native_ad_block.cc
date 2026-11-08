#include "browser_core/native_ad_block.h"

#include <algorithm>
#include <atomic>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "base/byte_size.h"
#include "base/command_line.h"
#include "base/containers/span.h"
#include "base/files/file_path.h"
#include "base/functional/bind.h"
#include "base/location.h"
#include "base/memory/scoped_refptr.h"
#include "base/no_destructor.h"
#include "base/strings/strcat.h"
#include "base/task/sequenced_task_runner.h"
#include "base/task/thread_pool.h"
#include "browser_core/host.h"
#include "browser_core/public/browser_core.h"
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

}  // namespace browser_core
