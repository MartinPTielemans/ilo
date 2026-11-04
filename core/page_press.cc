#include "browser_core/page_press.h"

#include <cstdint>
#include <utility>

#include "base/auto_reset.h"
#include "base/functional/bind.h"
#include "base/logging.h"
#include "browser_core/host.h"
#include "content/public/browser/render_frame_host.h"
#include "content/public/browser/render_widget_host_view.h"
#include "content/public/browser/web_contents.h"
#include "third_party/blink/public/common/input/web_input_event.h"

namespace browser_core {

namespace {

using Type = blink::WebInputEvent::Type;

uint32_t Modifiers(int held) {
  uint32_t modifiers = 0;
  for (auto [web, flag] : {std::pair{blink::WebInputEvent::kShiftKey, BC_MODIFIER_SHIFT},
                           std::pair{blink::WebInputEvent::kControlKey, BC_MODIFIER_CONTROL},
                           std::pair{blink::WebInputEvent::kAltKey, BC_MODIFIER_ALT},
                           std::pair{blink::WebInputEvent::kMetaKey, BC_MODIFIER_META}}) {
    if (held & web) {
      modifiers |= flag;
    }
  }
  return modifiers;
}

// |event|'s point in the page's view: an out-of-process frame's widget has it in its own.
gfx::PointF InPage(content::RenderWidgetHost* widget, const blink::WebMouseEvent& event) {
  content::RenderWidgetHostView* view = widget->GetView();
  return view ? view->TransformPointToRootCoordSpaceF(event.PositionInWidget())
              : event.PositionInWidget();
}

}  // namespace

PagePresses::PagePresses(content::WebContents* contents, bc_id tab)
    : content::WebContentsObserver(contents),
      content::WebContentsUserData<PagePresses>(*contents),
      tab_(tab) {
  // A page is often made with its first frames, before anything observes it.
  contents->ForEachRenderFrameHost(
      [this](content::RenderFrameHost* frame) { Hook(frame->GetRenderWidgetHost()); });
}

PagePresses::~PagePresses() {
  Unhook();
}

void PagePresses::Hook(content::RenderWidgetHost* widget) {
  // A frame in its parent's process shares its widget.
  if (!widget || widgets_.IsObservingSource(widget)) {
    return;
  }
  widgets_.AddObservation(widget);
  auto callback = base::BindRepeating(&PagePresses::OnMouse, base::Unretained(this),
                                      base::Unretained(widget));
  widget->AddMouseEventCallback(callback);
  hooked_.emplace(widget, std::move(callback));
}

void PagePresses::Unhook() {
  for (auto& [widget, callback] : hooked_) {
    widget->RemoveMouseEventCallback(callback);
  }
  hooked_.clear();
  widgets_.RemoveAllObservations();
  pressed_ = nullptr;
}

bool PagePresses::OnMouse(content::RenderWidgetHost* widget, const blink::WebMouseEvent& event) {
  if (replaying_) {
    return false;
  }
  const Type type = event.GetType();
  if (pressed_) {
    // From the event, not its modifiers: on the Mac those are the hardware's buttons, which an
    // event posted to the app, as the check's driver's are, never holds, while a drag's move says
    // its button.
    const bool held = event.button == blink::WebPointerProperties::Button::kLeft;
    if (type == Type::kMouseDown) {
      // The release went elsewhere, such as to a window that opened over the page.
      End(last_);
    } else if (widget != pressed_ || (type == Type::kMouseMove && !held)) {
      // Aura makes up moves with the buttons the pointer has, which may not be the press's.
      return false;
    } else if (type == Type::kMouseMove) {
      last_ = InPage(widget, event);
      if (Host().tab_press_moved) {
        Host().tab_press_moved(Host().context, tab_, last_.x(), last_.y());
      }
      return true;
    } else if (type == Type::kMouseUp) {
      if (End(InPage(widget, event))) {
        return true;
      }
      // Handed back: the page gets the press now, and this release after it, as a click.
      base::AutoReset<bool> replaying(&replaying_, true);
      widget->ForwardMouseEvent(press_);
      return false;
    } else {
      return false;
    }
  }
  if (type != Type::kMouseDown || event.button != blink::WebPointerProperties::Button::kLeft) {
    return false;
  }
  const uint32_t modifiers = Modifiers(event.GetModifiers());
  if (!modifiers || !Host().tab_press_before_page) {
    return false;
  }
  const gfx::PointF at = InPage(widget, event);
  const bool taken = Host().tab_press_before_page(Host().context, tab_, modifiers, at.x(), at.y());
  LOG(ERROR) << "browser: tab " << tab_ << " press with modifiers " << modifiers << " "
             << (taken ? "taken" : "left to the page");
  if (!taken) {
    return false;
  }
  pressed_ = widget;
  press_ = event;
  last_ = at;
  return true;
}

bool PagePresses::End(const gfx::PointF& at) {
  pressed_ = nullptr;
  const bool kept = Host().tab_press_released &&
                    Host().tab_press_released(Host().context, tab_, at.x(), at.y());
  LOG(ERROR) << "browser: tab " << tab_ << " press ended, " << (kept ? "kept" : "handed back");
  return kept;
}

void PagePresses::RenderFrameCreated(content::RenderFrameHost* frame) {
  Hook(frame->GetRenderWidgetHost());
}

void PagePresses::WebContentsDestroyed() {
  Unhook();
}

void PagePresses::RenderWidgetHostDestroyed(content::RenderWidgetHost* widget) {
  if (pressed_ == widget) {
    End(last_);
  }
  hooked_.erase(widget);
  widgets_.RemoveObservation(widget);
}

WEB_CONTENTS_USER_DATA_KEY_IMPL(PagePresses);

}  // namespace browser_core
