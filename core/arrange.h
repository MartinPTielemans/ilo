#ifndef BROWSER_CORE_ARRANGE_H_
#define BROWSER_CORE_ARRANGE_H_

#include <cstdint>
#include <map>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "base/scoped_multi_source_observation.h"
#include "base/time/time.h"
#include "base/timer/timer.h"
#include "base/values.h"
#include "browser_core/public/browser_core.h"
#include "content/public/browser/render_widget_host.h"
#include "content/public/browser/render_widget_host_observer.h"
#include "content/public/browser/web_contents_observer.h"
#include "content/public/browser/web_contents_user_data.h"
#include "third_party/blink/public/common/input/web_mouse_event.h"

namespace browser_core {

// Arrange (#2347, docs/ux-spec.md › Boosts): the user carries the page's own elements where they
// want them, and a site's arrangement is placed as its pages load. The core's script does both, in
// Chrome's internal isolated world as Zap does, so the page's scripts see the elements move but
// never the code, its state or the marks' insides. An arrangement reaches the page only as data the
// core checked (arrangement.h), and only while it's the one the user made or turned on here.
//
// The page's later changes reach the core only when it asks, as main has no channel from the
// renderer: at load, once more as the hold for late elements ends, after the user's input and as a
// scroll starts and rests.
class Arrange : public content::WebContentsObserver,
                public content::WebContentsUserData<Arrange>,
                public content::RenderWidgetHostObserver,
                public content::RenderWidgetHost::InputEventObserver {
 public:
  // How the marks look, bc_arrange_look: the app has it from its tokens and the Space's accent,
  // with its words for the kinds the in-page label names, by ARIA role or tag.
  struct Look {
    uint32_t accent = 0;
    uint32_t label = 0;
    uint32_t ink = 0;
    double line_width = 0;
    double radius = 0;
    std::vector<std::pair<std::string, std::string>> kinds;
    std::string other;
    // How it moves, in seconds and per second, as bc_arrange_look has them; these are the defaults
    // a value outside its range takes.
    double rest = 0.3;
    double room_omega = 25;
    double land_response = 0.13;
    double land_damping = 0.9;
  };

  ~Arrange() override;

  // Registers the record's profile pref, before the first profile is made.
  static void RegisterProfilePrefs();
  // bc_profile_set_arrangement: |bytes| for |site| in |profile|, or none to take it away. Returns
  // its BC_ARRANGEMENT_ state, NEW while the profile isn't loaded, as no page of it is open.
  static int Set(bc_id profile, std::string_view site, std::optional<std::string_view> bytes);
  // bc_profile_turn_on_arrangement.
  static bool TurnOn(bc_id profile, std::string_view site, std::string_view bytes);

  // bc_tab_arrange: whether it's on after.
  bool Start(const Look& look);
  // Ends the mode, and says so with tab_arrange_ended if |tell|.
  void End(bool tell);
  bool on() const { return on_; }
  // bc_tab_arrange_do.
  void Do(int action);
  // Places the site's arrangement on the page, if it's on here, or takes it away.
  void Load();

 private:
  friend class content::WebContentsUserData<Arrange>;

  Arrange(content::WebContents* contents, bc_id tab);

  // Runs |op| with |arg| in the page's runtime, then |done| with its answer.
  void Run(std::string_view op,
           base::DictValue arg,
           base::OnceCallback<void(base::Value)> done = {});
  void RunScript(std::string_view op, const std::string& arg, bool whole,
                 base::OnceCallback<void(base::Value)> done);
  void HoldEnded();
  void Fits(base::Value answer);
  // What the runtime answered the user's input or a scroll: the state for the app, a change to
  // save, an element to hide.
  void Answered(base::Value answer);
  void Saved(const base::DictValue& arrangement);
  void Tell(const base::DictValue& state);

  void Hook(content::RenderWidgetHost* widget);
  void Unhook();
  bool OnMouse(content::RenderWidgetHost* widget, const blink::WebMouseEvent& event);
  // Sends the latest move once the one before it is answered, so moves never queue up.
  void Move();
  void Moved(base::Value answer);
  void Rest();
  void Settle();

  std::string Site() const;
  // The site's arrangement in the tab's profile, as it would be placed: empty if it isn't on.
  std::string Placed() const;
  // The page's scale: CSS pixels to the view's points.
  double Zoom() const;

  // content::WebContentsObserver:
  void DidFinishNavigation(content::NavigationHandle* navigation) override;
  void RenderFrameCreated(content::RenderFrameHost* frame) override;
  void PrimaryPageChanged(content::Page& page) override;
  void WebContentsDestroyed() override;
  // content::RenderWidgetHostObserver:
  void RenderWidgetHostDestroyed(content::RenderWidgetHost* widget) override;
  // content::RenderWidgetHost::InputEventObserver:
  void OnInputEvent(const content::RenderWidgetHost& widget,
                    const blink::WebInputEvent& event,
                    InputEventSource source) override;

  const bc_id tab_;
  bool on_ = false;
  // Whether the page's document has the runtime, so a call sends the whole script only to a new
  // one. A call that finds none sends it then.
  bool installed_ = false;
  // A left press the mode took, and the last move of it not yet sent.
  bool pressed_ = false;
  bool moving_ = false;
  bool scrolling_ = false;
  std::optional<base::DictValue> move_;
  // The last state told, so the app hears each change once.
  std::optional<base::DictValue> told_;
  // How long a carry holds still before the page shows the drop made: the look's |rest|.
  base::TimeDelta rest_after_ = base::Seconds(Look().rest);
  base::OneShotTimer rest_;
  base::OneShotTimer settle_;
  base::OneShotTimer hold_;
  std::map<raw_ptr<content::RenderWidgetHost>, content::RenderWidgetHost::MouseEventCallback>
      hooked_;
  base::ScopedMultiSourceObservation<content::RenderWidgetHost, content::RenderWidgetHostObserver>
      widgets_{this};
  base::ScopedMultiSourceObservation<content::RenderWidgetHost,
                                     content::RenderWidgetHost::InputEventObserver>
      inputs_{this};
  base::WeakPtrFactory<Arrange> weak_factory_{this};

  WEB_CONTENTS_USER_DATA_KEY_DECL();
};

}  // namespace browser_core

#endif  // BROWSER_CORE_ARRANGE_H_
