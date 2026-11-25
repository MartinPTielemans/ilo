#include "browser_core/agent_share.h"

#include "build/build_config.h"

#if BUILDFLAG(IS_MAC) || BUILDFLAG(IS_WIN)
#include <string>
#include <utility>
#include <vector>

#include "browser_core/agent_permissions.h"
#include "browser_core/agent_space_page.h"
#include "chrome/browser/webshare/share_service_impl.h"
#include "content/public/browser/document_service.h"
#include "content/public/browser/web_contents.h"
#include "mojo/public/cpp/bindings/callback_helpers.h"
#include "mojo/public/cpp/bindings/pending_receiver.h"
#include "mojo/public/cpp/bindings/remote.h"
#include "third_party/blink/public/mojom/webshare/webshare.mojom.h"
#include "url/gurl.h"
#endif

namespace browser_core {

#if BUILDFLAG(IS_MAC) || BUILDFLAG(IS_WIN)

namespace {

// The share service of a page in an agent's Space, which ends with the page's document or its end
// of the pipe.
class AgentShare : public content::DocumentService<blink::mojom::ShareService> {
 public:
  static void Bind(content::RenderFrameHost* frame,
                   mojo::PendingReceiver<blink::mojom::ShareService> receiver) {
    // A frame Chrome gives no share service at all gets its answer from Chrome too.
    if (!RuleTabOf(content::WebContents::FromRenderFrameHost(frame)).agents ||
        frame->IsNestedWithinFencedFrame()) {
      ShareServiceImpl::Create(frame, std::move(receiver));
      return;
    }
    new AgentShare(*frame, std::move(receiver));
  }

 private:
  AgentShare(content::RenderFrameHost& frame,
             mojo::PendingReceiver<blink::mojom::ShareService> receiver)
      : content::DocumentService<blink::mojom::ShareService>(frame, std::move(receiver)) {}

  // blink::mojom::ShareService:
  void Share(const std::string& title,
             const std::string& text,
             const GURL& url,
             std::vector<blink::mojom::SharedFilePtr> files,
             ShareCallback callback) override {
    if (NoShare(&render_frame_host())) {
      std::move(callback).Run(blink::mojom::ShareError::PERMISSION_DENIED);
      return;
    }
    if (!chromes_) {
      ShareServiceImpl::Create(&render_frame_host(), chromes_.BindNewPipeAndPassReceiver());
      // Chrome's ends itself after a call it won't take: the next share makes another.
      chromes_.reset_on_disconnect();
    }
    chromes_->Share(title, text, url, std::move(files),
                    mojo::WrapCallbackWithDefaultInvokeIfNotRun(
                        std::move(callback), blink::mojom::ShareError::INTERNAL_ERROR));
  }

  mojo::Remote<blink::mojom::ShareService> chromes_;
};

}  // namespace

void AddShareBinder(mojo::BinderMapWithContext<content::RenderFrameHost*>* map) {
  map->Add<blink::mojom::ShareService>(&AgentShare::Bind);
}

#else

void AddShareBinder(mojo::BinderMapWithContext<content::RenderFrameHost*>* map) {}

#endif

}  // namespace browser_core
