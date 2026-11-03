#include "browser_core/mini_player.h"

#include <algorithm>
#include <cmath>
#include <optional>

#include "base/logging.h"
#include "browser_core/hooks.h"
#include "build/build_config.h"
#include "chrome/browser/picture_in_picture/picture_in_picture_window_manager.h"
#include "chrome/browser/ui/views/overlay/video_overlay_window_views.h"
#include "ui/display/display.h"
#include "ui/display/screen.h"
#include "ui/events/event.h"
#include "ui/events/event_constants.h"
#include "ui/gfx/geometry/point.h"
#include "ui/gfx/geometry/rect.h"
#include "ui/gfx/geometry/size.h"
#include "ui/gfx/geometry/vector2d.h"
#include "ui/gfx/geometry/vector2d_conversions.h"
#include "ui/gfx/geometry/vector2d_f.h"
#include "ui/views/view.h"
#include "ui/views/widget/widget.h"

#if BUILDFLAG(IS_LINUX)
#include "browser_core/platform/linux/wayland.h"
#endif

namespace browser_core {

namespace {

// The key that makes a scroll size the window rather than move it: ⌘, or Ctrl where there's none.
#if BUILDFLAG(IS_MAC)
constexpr int kSizing = ui::EF_COMMAND_DOWN;
#else
constexpr int kSizing = ui::EF_CONTROL_DOWN;
#endif

// How much a notch of the wheel, or a scroll as far, sizes it.
constexpr float kNotchScale = 1.1f;

// Where the left button went down in the window, off its controls, until it's dragged far enough to
// move the window, or let go.
std::optional<gfx::Point> g_pressed;

// Puts |window| at |bounds|, moved onto the display it's mostly on.
void Place(VideoOverlayWindowViews* window, gfx::Rect bounds) {
  bounds.AdjustToFit(display::Screen::Get()->GetDisplayMatching(bounds).work_area());
  window->SetBounds(bounds);
}

// Sizes |window| by |scale| about its middle, keeping its shape, within its least and greatest.
void Size(VideoOverlayWindowViews* window, float scale) {
  const gfx::Rect bounds = window->GetWindowBoundsInScreen();
  if (bounds.IsEmpty() || !(scale > 0)) {
    return;
  }
  const float shape = static_cast<float>(bounds.height()) / bounds.width();
  const gfx::Size least = window->GetMinimumSize();
  gfx::Size most = window->GetMaximumSize();
  most.SetToMax(least);
  float width = std::clamp(bounds.width() * scale, static_cast<float>(least.width()),
                           static_cast<float>(most.width()));
  width = std::clamp(width * shape, static_cast<float>(least.height()),
                     static_cast<float>(most.height())) /
          shape;
  const gfx::Size size(std::round(width), std::round(width * shape));
  gfx::Rect sized(size);
  sized.set_origin(bounds.CenterPoint() - gfx::Vector2d(size.width() / 2, size.height() / 2));
  Place(window, sized);
}

// A scroll of |offset| over |window|, with the keys in |flags|: it moves the window with the
// content a scroll would move, or with ⌘ grows it as the content would go down.
void Scrolled(VideoOverlayWindowViews* window, const gfx::Vector2dF& offset, int flags) {
  if (flags & kSizing) {
    Size(window, std::pow(kNotchScale, offset.y() / ui::MouseWheelEvent::kWheelDelta));
    return;
  }
  gfx::Rect bounds = window->GetWindowBoundsInScreen();
  bounds.Offset(gfx::ToRoundedVector2d(offset));
  Place(window, bounds);
}

bool OnEvent(VideoOverlayWindowViews* window, ui::Event* event) {
  switch (event->type()) {
    case ui::EventType::kMousewheel:
      Scrolled(window, gfx::Vector2dF(event->AsMouseWheelEvent()->offset()), event->flags());
      break;
    case ui::EventType::kScroll:
      Scrolled(window,
               gfx::Vector2dF(event->AsScrollEvent()->x_offset(),
                              event->AsScrollEvent()->y_offset()),
               event->flags());
      break;
    case ui::EventType::kGesturePinchUpdate:
      Size(window, event->AsGestureEvent()->details().scale());
      break;
    case ui::EventType::kMousePressed: {
      g_pressed.reset();
      const ui::MouseEvent* mouse = event->AsMouseEvent();
      if (!mouse->IsOnlyLeftMouseButton()) {
        return false;
      }
      if (window->ControlsHitTestContainsPoint(mouse->location())) {
        // check.sh reads these lines.
        LOG(ERROR) << "browser: mini player pressed on its controls";
        return false;
      }
      if (!(mouse->flags() & ui::EF_IS_DOUBLE_CLICK)) {
        g_pressed = mouse->location();
        LOG(ERROR) << "browser: mini player pressed at " << g_pressed->ToString();
        return false;
      }
      event->SetHandled();
      PictureInPictureWindowManager::GetInstance()->ExitPictureInPictureViaWindowUi(
          PictureInPictureWindowManager::UiBehavior::kCloseWindowAndFocusOpener);
      return true;
    }
    case ui::EventType::kMouseDragged: {
      const gfx::Point at = event->AsMouseEvent()->location();
      if (!g_pressed || !views::View::ExceededDragThreshold(at - *g_pressed)) {
        return false;
      }
      const gfx::Point press = *g_pressed;
      g_pressed.reset();
      event->SetHandled();
      // check.sh reads this line.
      LOG(ERROR) << "browser: mini player moving from " << press.ToString();
#if BUILDFLAG(IS_LINUX)
      // The window manager or compositor moves it, as it moves a window by its title bar: under
      // Wayland a window can't move itself, and under X11 Chromium's move loop left it in place,
      // with a real pointer too (#813).
      platform::StartMove(window, press);
#else
      // As Chrome's caption would, but the window sees the press first. It may be gone after.
      if (window->IsMoveLoopSupported()) {
        window->RunMoveLoop(press.OffsetFromOrigin(), views::Widget::MoveLoopSource::kMouse,
                            views::Widget::MoveLoopEscapeBehavior::kDontHide);
      }
#endif
      return true;
    }
    case ui::EventType::kMouseReleased:
      g_pressed.reset();
      return false;
    default:
      return false;
  }
  event->SetHandled();
  return true;
}

}  // namespace

void SetUpMiniPlayer() {
  g_browser_core_mini_player_event = &OnEvent;
}

}  // namespace browser_core
