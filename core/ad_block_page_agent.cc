#include "browser_core/ad_block_page_agent.h"

#include <cstdint>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "base/containers/span.h"
#include "base/functional/bind.h"
#include "base/memory/weak_ptr.h"
#include "base/time/time.h"
#include "base/values.h"
#include "browser_core/ad_block_page.mojom.h"
#include "content/public/renderer/render_frame.h"
#include "content/public/renderer/render_frame_observer.h"
#include "content/public/renderer/render_frame_observer_tracker.h"
#include "mojo/public/cpp/bindings/associated_receiver_set.h"
#include "mojo/public/cpp/bindings/pending_associated_receiver.h"
#include "mojo/public/cpp/bindings/pending_remote.h"
#include "mojo/public/cpp/bindings/remote.h"
#include "third_party/blink/public/common/associated_interfaces/associated_interface_registry.h"
#include "third_party/blink/public/mojom/script/script_evaluation_params.mojom-shared.h"
#include "third_party/blink/public/platform/web_string.h"
#include "third_party/blink/public/platform/web_url.h"
#include "third_party/blink/public/web/web_css_origin.h"
#include "third_party/blink/public/web/web_document.h"
#include "third_party/blink/public/web/web_local_frame.h"
#include "third_party/blink/public/web/web_script_execution_callback.h"
#include "third_party/blink/public/web/web_script_source.h"
#include "url/gurl.h"

namespace browser_core {

namespace {

// Gives the classes and ids the document has that it hasn't given yet, once there are some: at
// first those already there, then those of the elements the page adds or changes, as uBlock
// Origin's content script watches them. A promise, for the agent to await. In an isolated world of
// its own, so the page can neither see it nor change what it gives; it keeps what it has given
// there for as long as the document is shown.
constexpr char kWatch[] = R"((() => {
  let watch = globalThis.adBlockWatch;
  if (!watch) {
    watch = globalThis.adBlockWatch = {
      classes: new Set(), ids: new Set(), newClasses: [], newIds: [], wake: null,
    };
    const add = (element) => {
      const id = element.id;
      if (typeof id === 'string' && id !== '' && !watch.ids.has(id)) {
        watch.ids.add(id);
        watch.newIds.push(id);
      }
      for (const name of element.classList || []) {
        if (!watch.classes.has(name)) {
          watch.classes.add(name);
          watch.newClasses.push(name);
        }
      }
    };
    const scan = (node) => {
      if (node.nodeType !== Node.ELEMENT_NODE) {
        return;
      }
      add(node);
      for (const element of node.querySelectorAll('[id],[class]')) {
        add(element);
      }
    };
    new MutationObserver((records) => {
      for (const record of records) {
        if (record.type === 'attributes') {
          add(record.target);
        } else {
          record.addedNodes.forEach(scan);
        }
      }
      if (watch.wake && (watch.newClasses.length || watch.newIds.length)) {
        watch.wake();
      }
    }).observe(document, {
      childList: true, subtree: true, attributes: true, attributeFilter: ['class', 'id'],
    });
    if (document.documentElement) {
      scan(document.documentElement);
    }
  }
  return new Promise((resolve) => {
    watch.wake = () => {
      watch.wake = null;
      resolve({ classes: watch.newClasses, ids: watch.newIds });
      watch.newClasses = [];
      watch.newIds = [];
    };
    if (watch.newClasses.length || watch.newIds.length) {
      watch.wake();
    }
  });
})())";

// The strings of |list|, a list of them.
std::vector<std::string> Strings(const base::ListValue* list) {
  std::vector<std::string> strings;
  if (list) {
    for (const base::Value& value : *list) {
      if (const std::string* string = value.GetIfString()) {
        strings.push_back(*string);
      }
    }
  }
  return strings;
}

class AdBlockPageAgent : public content::RenderFrameObserver,
                         public content::RenderFrameObserverTracker<AdBlockPageAgent>,
                         public mojom::AdBlockPage {
 public:
  AdBlockPageAgent(content::RenderFrame* frame, int32_t world)
      : content::RenderFrameObserver(frame),
        content::RenderFrameObserverTracker<AdBlockPageAgent>(frame),
        world_(world) {
    frame->GetAssociatedInterfaceRegistry()->AddInterface<mojom::AdBlockPage>(
        base::BindRepeating(&AdBlockPageAgent::Bind, base::Unretained(this)));
  }
  AdBlockPageAgent(const AdBlockPageAgent&) = delete;
  AdBlockPageAgent& operator=(const AdBlockPageAgent&) = delete;
  ~AdBlockPageAgent() override = default;

  // mojom::AdBlockPage:
  void UseForNextDocument(const std::string& url,
                          const std::string& css,
                          const std::string& script,
                          const std::string& isolated,
                          mojo::PendingRemote<mojom::AdBlockHide> hide) override {
    next_ = Next{url, css, script, isolated, std::move(hide)};
  }

  // content::RenderFrameObserver:
  void DidCreateNewDocument() override {
    script_.clear();
    isolated_.clear();
    hide_.reset();
    // What the last document's watch gives after this isn't this one's.
    watching_.InvalidateWeakPtrs();
  }

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
    // Blink forbids script here, so they wait for RunScriptsAtDocumentStart, next.
    script_ = std::move(next.script);
    isolated_ = std::move(next.isolated);
    if (next.hide) {
      hide_.Bind(std::move(next.hide));
    }
  }

  void RunScriptsAtDocumentStart() {
    if (!script_.empty()) {
      std::string script = std::move(script_);
      script_.clear();
      // In the page's own world, as uBlock Origin's are, since they replace what the page sees.
      render_frame()->GetWebFrame()->ExecuteScript(
          blink::WebScriptSource(blink::WebString::FromUtf8(script)));
    }
    if (!isolated_.empty()) {
      std::string isolated = std::move(isolated_);
      isolated_.clear();
      // In the watch's world, as uBlock Origin's content script runs its procedural filters.
      render_frame()->GetWebFrame()->ExecuteScriptInIsolatedWorld(
          world_, blink::WebScriptSource(blink::WebString::FromUtf8(isolated)),
          blink::BackForwardCacheAware::kAllow);
    }
    if (hide_.is_bound()) {
      Watch();
    }
  }

  void OnDestruct() override { delete this; }

 private:
  struct Next {
    std::string url;
    std::string css;
    std::string script;
    std::string isolated;
    mojo::PendingRemote<mojom::AdBlockHide> hide;
  };

  void Bind(mojo::PendingAssociatedReceiver<mojom::AdBlockPage> receiver) {
    // The browser asks for the interface again for each document.
    receivers_.Add(this, std::move(receiver));
  }

  // Waits for the document's next classes and ids, without a timer: the watch's promise settles
  // only once the page adds some.
  void Watch() {
    const blink::WebScriptSource source(blink::WebString::FromUtf8(kWatch));
    render_frame()->GetWebFrame()->RequestExecuteScript(
        world_, base::span_from_ref(source), blink::mojom::UserActivationOption::kDoNotActivate,
        blink::mojom::EvaluationTiming::kAsynchronous,
        blink::mojom::LoadEventBlockingOption::kDoNotBlock,
        base::BindOnce(&AdBlockPageAgent::Watched, watching_.GetWeakPtr()),
        blink::BackForwardCacheAware::kAllow, blink::mojom::WantResultOption::kWantResult,
        blink::mojom::PromiseResultOption::kAwait, blink::WebString());
  }

  void Watched(std::optional<base::Value> names, base::TimeTicks) {
    // None once the document goes, or if the watch couldn't run.
    if (!names || !names->is_dict() || !hide_.is_bound()) {
      return;
    }
    hide_->Hide(Strings(names->GetDict().FindList("classes")),
                Strings(names->GetDict().FindList("ids")),
                base::BindOnce(&AdBlockPageAgent::Hidden, watching_.GetWeakPtr()));
    Watch();
  }

  void Hidden(const std::string& css) {
    if (!css.empty()) {
      render_frame()->GetWebFrame()->GetDocument().InsertStyleSheet(
          blink::WebString::FromUtf8(css), nullptr, blink::WebCssOrigin::kUser);
    }
  }

  // The isolated world the watch runs in, Chrome's for the ad blocker.
  const int32_t world_;
  std::optional<Next> next_;
  std::string script_;
  std::string isolated_;
  // The document's, where its page's generic rules apply.
  mojo::Remote<mojom::AdBlockHide> hide_;
  mojo::AssociatedReceiverSet<mojom::AdBlockPage> receivers_;
  // The document's watch and its answers, which go with it.
  base::WeakPtrFactory<AdBlockPageAgent> watching_{this};
};

}  // namespace

void CreateAdBlockPageAgent(content::RenderFrame* frame, int32_t world) {
  new AdBlockPageAgent(frame, world);
}

void RunAdBlockScriptsAtDocumentStart(content::RenderFrame* frame) {
  if (AdBlockPageAgent* agent = AdBlockPageAgent::Get(frame)) {
    agent->RunScriptsAtDocumentStart();
  }
}

}  // namespace browser_core
