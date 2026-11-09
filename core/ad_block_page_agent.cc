#include "browser_core/ad_block_page_agent.h"

#include <optional>
#include <string>
#include <utility>

#include "base/functional/bind.h"
#include "browser_core/ad_block_page.mojom.h"
#include "content/public/renderer/render_frame.h"
#include "content/public/renderer/render_frame_observer.h"
#include "content/public/renderer/render_frame_observer_tracker.h"
#include "mojo/public/cpp/bindings/associated_receiver_set.h"
#include "mojo/public/cpp/bindings/pending_associated_receiver.h"
#include "third_party/blink/public/common/associated_interfaces/associated_interface_registry.h"
#include "third_party/blink/public/platform/web_string.h"
#include "third_party/blink/public/platform/web_url.h"
#include "third_party/blink/public/web/web_css_origin.h"
#include "third_party/blink/public/web/web_document.h"
#include "third_party/blink/public/web/web_local_frame.h"
#include "third_party/blink/public/web/web_script_source.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

class AdBlockPageAgent : public content::RenderFrameObserver,
                         public content::RenderFrameObserverTracker<AdBlockPageAgent>,
                         public mojom::AdBlockPage {
 public:
  explicit AdBlockPageAgent(content::RenderFrame* frame)
      : content::RenderFrameObserver(frame),
        content::RenderFrameObserverTracker<AdBlockPageAgent>(frame) {
    frame->GetAssociatedInterfaceRegistry()->AddInterface<mojom::AdBlockPage>(
        base::BindRepeating(&AdBlockPageAgent::Bind, base::Unretained(this)));
  }
  AdBlockPageAgent(const AdBlockPageAgent&) = delete;
  AdBlockPageAgent& operator=(const AdBlockPageAgent&) = delete;
  ~AdBlockPageAgent() override = default;

  // mojom::AdBlockPage:
  void UseForNextDocument(const std::string& url,
                          const std::string& css,
                          const std::string& script) override {
    next_ = Next{url, css, script};
  }

  // content::RenderFrameObserver:
  void DidCreateNewDocument() override { script_.clear(); }

  // The document's element is there for the style sheet, and none of the page's scripts has run.
  void DidCreateDocumentElement() override {
    if (!next_) {
      return;
    }
    Next next = std::move(*next_);
    next_.reset();
    blink::WebLocalFrame* frame = render_frame()->GetWebFrame();
    blink::WebDocument document = frame->GetDocument();
    // What the browser sent for another document, one that never committed, isn't this one's.
    if (GURL(document.Url()).spec() != next.url) {
      return;
    }
    if (!next.css.empty()) {
      // As the user's, as uBlock Origin's are, so a page's own !important can't show what it
      // hides.
      document.InsertStyleSheet(blink::WebString::FromUtf8(next.css), nullptr,
                                blink::WebCssOrigin::kUser);
    }
    // Blink forbids script here, so it waits for RunScriptsAtDocumentStart, next.
    script_ = std::move(next.script);
  }

  void RunScriptsAtDocumentStart() {
    if (script_.empty()) {
      return;
    }
    std::string script = std::move(script_);
    script_.clear();
    // In the page's own world, as uBlock Origin's are, since they replace what the page sees.
    render_frame()->GetWebFrame()->ExecuteScript(
        blink::WebScriptSource(blink::WebString::FromUtf8(script)));
  }

  void OnDestruct() override { delete this; }

 private:
  struct Next {
    std::string url;
    std::string css;
    std::string script;
  };

  void Bind(mojo::PendingAssociatedReceiver<mojom::AdBlockPage> receiver) {
    // The browser asks for the interface again for each document.
    receivers_.Add(this, std::move(receiver));
  }

  std::optional<Next> next_;
  std::string script_;
  mojo::AssociatedReceiverSet<mojom::AdBlockPage> receivers_;
};

}  // namespace

void CreateAdBlockPageAgent(content::RenderFrame* frame) {
  new AdBlockPageAgent(frame);
}

void RunAdBlockScriptsAtDocumentStart(content::RenderFrame* frame) {
  if (AdBlockPageAgent* agent = AdBlockPageAgent::Get(frame)) {
    agent->RunScriptsAtDocumentStart();
  }
}

}  // namespace browser_core
