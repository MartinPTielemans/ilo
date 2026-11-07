#ifndef BROWSER_CORE_PAGE_PRESS_H_
#define BROWSER_CORE_PAGE_PRESS_H_

#include <map>

#include "base/memory/raw_ptr.h"
#include "base/scoped_multi_source_observation.h"
#include "browser_core/public/browser_core.h"
#include "content/public/browser/render_widget_host.h"
#include "content/public/browser/render_widget_host_observer.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/web_contents_user_data.h"
#include "third_party/blink/public/common/input/web_mouse_event.h"
#include "ui/gfx/geometry/point_f.h"

namespace browser_core {

// Asks the app about a left press with a modifier held on a page before the page gets it, as
// tab_key_before_page does a key, so a gesture such as ⇧⌘-drag's capture is the app's. A press
// the app takes sends it its moves and release instead of the page.
class PagePresses : public content::WebContentsObserver,
                    public content::WebContentsUserData<PagePresses>,
                    public content::RenderWidgetHostObserver {
 public:
  ~PagePresses() override;

 private:
  friend class content::WebContentsUserData<PagePresses>;

  PagePresses(content::WebContents* contents, bc_id tab);

  // Each frame's widget, the page's and an out-of-process frame's, as each gets its own input.
  void Hook(content::RenderWidgetHost* widget);
  void Unhook();
  bool OnMouse(content::RenderWidgetHost* widget, const blink::WebMouseEvent& event);
  // The press taken ends at |at|; returns whether the app kept it.
  bool End(const gfx::PointF& at);

  void RenderFrameCreated(content::RenderFrameHost* frame) override;
  void WebContentsDestroyed() override;
  void RenderWidgetHostDestroyed(content::RenderWidgetHost* widget) override;

  const bc_id tab_;
  std::map<raw_ptr<content::RenderWidgetHost>,
           content::RenderWidgetHost::MouseEventCallback>
      hooked_;
  base::ScopedMultiSourceObservation<content::RenderWidgetHost, content::RenderWidgetHostObserver>
      widgets_{this};
  // The widget of the press the app took, and the press, to give the page if it hands it back.
  raw_ptr<content::RenderWidgetHost> pressed_ = nullptr;
  blink::WebMouseEvent press_;
  // Where it was last, in the page's view.
  gfx::PointF last_;
  bool replaying_ = false;

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

}  // namespace browser_core

#endif  // BROWSER_CORE_PAGE_PRESS_H_
