#ifndef BROWSER_CORE_POINT_H_
#define BROWSER_CORE_POINT_H_

#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "base/containers/flat_map.h"
#include "base/containers/span.h"
#include "base/functional/callback.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/scoped_refptr.h"
#include "base/memory/weak_ptr.h"
#include "base/scoped_multi_source_observation.h"
#include "base/types/expected.h"
#include "base/values.h"
#include "browser_core/public/browser_core.h"
#include "content/public/browser/devtools_agent_host_client.h"
#include "content/public/browser/render_widget_host.h"
#include "content/public/browser/render_widget_host_observer.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/web_contents_user_data.h"
#include "third_party/blink/public/common/input/web_mouse_event.h"
#include "ui/gfx/geometry/point_f.h"
#include "ui/gfx/geometry/rect_f.h"

class SkBitmap;

namespace content {
class DevToolsAgentHost;
}  // namespace content

namespace browser_core {

// A tab's Point (bc_tab_point): the user points at elements on the page for the agent that asked.
// Everything it reads, it reads through a DevTools client of the tab's own, as Capture does
// (capture.h), and the accessibility tree's snapshot, as bc_tab_read_text does: no script runs in
// the page and nothing is added to it, so the page can't tell the user is pointing. The marks are
// the app's, drawn over the page.
class PagePoint : public content::WebContentsObserver,
                  public content::WebContentsUserData<PagePoint>,
                  public content::DevToolsAgentHostClient,
                  public content::RenderWidgetHostObserver {
 public:
  PagePoint(const PagePoint&) = delete;
  PagePoint& operator=(const PagePoint&) = delete;
  ~PagePoint() override;

  // See bc_tab_may_point.
  static int May(content::WebContents& contents, std::string_view agent);

  // See bc_tab_point.
  int Start(std::string agent);
  // Ends it, telling the app with |tell| (tab_point_ended).
  void End(bool tell);
  // See bc_tab_point_do.
  void Do(int action);

  bool on() const { return on_; }
  // The agent that asked, while it's on.
  const std::string& agent() const { return agent_; }

 private:
  friend class content::WebContentsUserData<PagePoint>;

  using Reply = base::OnceCallback<void(const base::DictValue* result)>;

  // An element as the hover shows it: DevTools' backend node id, and its kind and name.
  struct Shown {
    int node = 0;
    std::string kind;
    std::string name;
    gfx::RectF box;
  };

  // An element the pointer is in, and its accessible name, from the hit one outward.
  struct Link {
    int node = 0;
    std::string name;
  };

  // What's gathered of the element picked, as it fills in, in the turn it was picked in.
  struct Picked;

  PagePoint(content::WebContents* contents, bc_id tab);

  // Each frame's widget, the page's and an out-of-process frame's, as each gets its own input.
  void Hook(content::RenderWidgetHost* widget);
  void Unhook();
  bool OnMouse(content::RenderWidgetHost* widget, const blink::WebMouseEvent& event);

  // The hover: one question in the page at a time, a newer point replacing any waiting, so a moving
  // pointer asks only for where it is now (Capture's ElementAt). Where the page has scrolled, the
  // node there, the elements it's in, then the shown one's name and box.
  void Ask(gfx::PointF point);
  void HitTest(gfx::PointF point, const base::DictValue* metrics);
  void Ancestors(const base::DictValue* node);
  void Chain(const base::DictValue* result);
  // Shows the element at |at_| in the chain, the hit one at 0 and those around it after.
  void Show();
  void Described(Link link, const base::DictValue* result);
  void Boxed(Link link, std::string kind, const base::DictValue* result);
  void Hover(std::optional<Shown> shown);
  void Asked();

  // The pick: the shown element's tag, place in the page and styles, its text, and its picture,
  // each step adding to |picked| until it goes to the app.
  void Pick();
  void Document(Picked picked, const base::DictValue* result);
  void Styled(Picked picked, const base::DictValue* result);
  // Whether an agent may read the frame DevTools names |frame|, and each it's in (IsWebFrame).
  bool Readable(const std::string& frame);
  void Texted(Picked picked, std::string text, bool withheld);
  void Drawn(Picked picked, base::expected<SkBitmap, std::string> picture);
  void Encoded(Picked picked, std::optional<std::vector<uint8_t>> png);
  void Report(const Picked& picked);

  // Sends a DevTools command, attaching first if need be. |reply| gets its result, or null for an
  // error or a page that went.
  void Send(const char* method, base::DictValue params, Reply reply);
  void Detach();

  double Zoom();

  // content::WebContentsObserver:
  void RenderFrameCreated(content::RenderFrameHost* frame) override;
  void PrimaryPageChanged(content::Page& page) override;
  void WebContentsDestroyed() override;

  // content::RenderWidgetHostObserver:
  void RenderWidgetHostDestroyed(content::RenderWidgetHost* widget) override;

  // content::DevToolsAgentHostClient:
  void DispatchProtocolMessage(content::DevToolsAgentHost* agent_host,
                               base::span<const uint8_t> message) override;
  void AgentHostClosed(content::DevToolsAgentHost* agent_host) override;

  const bc_id tab_;
  bool on_ = false;
  std::string agent_;

  std::map<raw_ptr<content::RenderWidgetHost>, content::RenderWidgetHost::MouseEventCallback>
      hooked_;
  base::ScopedMultiSourceObservation<content::RenderWidgetHost, content::RenderWidgetHostObserver>
      widgets_{this};
  // A left press taken, whose release is taken too.
  bool pressed_ = false;

  scoped_refptr<content::DevToolsAgentHost> agent_host_;
  int next_command_ = 1;
  base::flat_map<int, Reply> replies_;
  // Accessibility is on in the client, which the elements' ancestors are read from.
  bool accessible_ = false;
  // Each question and pick belongs to one turn on: one asked in a turn since ended is dropped.
  int turn_ = 0;

  bool asking_ = false;
  std::optional<gfx::PointF> waiting_;
  // The elements under the pointer, the hit one first, then each it's in, and which ↑ and ↓ have
  // shown.
  std::vector<Link> chain_;
  // The frame the hit element is in, as DevTools names it, and so are those it's in.
  std::string frame_;
  size_t at_ = 0;
  std::optional<Shown> shown_;
  // A pick is gathered one at a time; a click while one is still gathered is dropped.
  bool picking_ = false;

  base::WeakPtrFactory<PagePoint> weak_factory_{this};

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

}  // namespace browser_core

#endif  // BROWSER_CORE_POINT_H_
