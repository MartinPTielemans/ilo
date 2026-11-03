#include "browser_core/mini_player.h"

#include <algorithm>
#include <cmath>
#include <optional>

#include "base/logging.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/scoped_observation.h"
#include "browser_core/hooks.h"
#include "browser_core/mini_player_place.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "build/build_config.h"
#include "chrome/browser/picture_in_picture/picture_in_picture_window_manager.h"
#include "chrome/browser/ui/views/overlay/video_overlay_window_views.h"
#include "content/public/browser/web_contents.h"
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
#include "ui/views/widget/widget_observer.h"

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

// The Mini Player Chrome made last, as it makes one for each video floated.
base::WeakPtr<views::Widget> g_made;

// Tells the windows as the Mini Player takes the keyboard, which on Linux can come after the window
// that let it go asked whether it had (MiniPlayerHasKeyboard).
class KeyboardWatch : public views::WidgetObserver {
 public:
  void Watch(views::Widget* widget) {
    observation_.Reset();
    observation_.Observe(widget);
  }

  void OnWidgetActivationChanged(views::Widget* widget, bool active) override {
    if (active) {
      Window::MiniPlayerTookKeyboard();
    }
  }

  void OnWidgetDestroying(views::Widget* widget) override { observation_.Reset(); }

 private:
  base::ScopedObservation<views::Widget, views::WidgetObserver> observation_{this};
};

KeyboardWatch& Watching() {
  static base::NoDestructor<KeyboardWatch> watch;
  return *watch;
}

// Where the last Mini Player was left, for the next, while ilo runs.
std::optional<gfx::Rect> g_left;

// Where |window| goes, given Chrome's |bounds| for it at its present place.
gfx::Rect Placed(VideoOverlayWindowViews* window,
                 const gfx::Rect& bounds,
                 bool shown,
                 const gfx::Rect& work_area) {
  if (g_made.get() != window) {
    g_made = window->GetWeakPtr();
    Watching().Watch(window);
  }
  gfx::Rect placed = bounds;
  if (shown) {
    // Kept on its screen as a move keeps it, where Chrome would keep it a margin from its corner.
    placed.AdjustToFit(work_area);
    return placed;
  }
  const gfx::Rect on =
      g_left ? display::Screen::Get()->GetDisplayMatching(*g_left).work_area() : work_area;
  placed = FirstMiniPlayerPlace(bounds.size(), g_left, on);
  // check.sh reads this line.
  LOG(ERROR) << "browser: mini player placed at " << placed.x() << " " << placed.y() << " "
             << placed.width() << " " << placed.height() << " on " << on.x() << " " << on.y()
             << " " << on.width() << " " << on.height();
  return placed;
}

void Left(VideoOverlayWindowViews* window) {
  // One closed before it showed was never anywhere.
  if (!window->IsVisible()) {
    return;
  }
  g_left = window->GetWindowBoundsInScreen();
  // check.sh reads this line.
  LOG(ERROR) << "browser: mini player left at " << g_left->x() << " " << g_left->y() << " "
             << g_left->width() << " " << g_left->height();
}

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
  g_browser_core_mini_player_place = &Placed;
  g_browser_core_mini_player_left = &Left;
}

bool MiniPlayerHasKeyboard() {
  return g_made && g_made->IsVisible() && g_made->IsActive();
}

void CameBackTo(Window& window) {
  PictureInPictureWindowManager* manager = PictureInPictureWindowManager::GetInstance();
  content::WebContents* contents = manager->GetWebContents();
  if (!manager->IsInPictureInPicture() || !contents || manager->GetChildWebContents() ||
      Window::Showing(Tab::IdOf(contents)) != &window) {
    return;
  }
  // check.sh reads this line.
  LOG(ERROR) << "browser: mini player back in its tab";
  // Not while the window's activation is being told.
  PictureInPictureWindowManager::ExitPictureInPictureSoon();
}

}  // namespace browser_core
