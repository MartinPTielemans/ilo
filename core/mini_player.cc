#include "browser_core/mini_player.h"

#include <algorithm>
#include <cmath>
#include <optional>

#include "base/logging.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/scoped_observation.h"
#include "base/values.h"
#include "browser_core/hooks.h"
#include "browser_core/mini_player_place.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "build/build_config.h"
#include "chrome/browser/browser_process.h"
#include "chrome/browser/picture_in_picture/picture_in_picture_window_manager.h"
#include "chrome/browser/ui/views/overlay/video_overlay_window_views.h"
#include "components/prefs/pref_registry_simple.h"
#include "components/prefs/pref_service.h"
#include "content/public/browser/web_contents.h"
#include "third_party/skia/include/core/SkColor.h"
#include "ui/compositor/layer.h"
#include "ui/display/display.h"
#include "ui/display/screen.h"
#include "ui/events/event.h"
#include "ui/events/event_constants.h"
#include "ui/gfx/geometry/insets.h"
#include "ui/gfx/geometry/point.h"
#include "ui/gfx/geometry/rect.h"
#include "ui/gfx/geometry/rounded_corners_f.h"
#include "ui/gfx/geometry/size.h"
#include "ui/gfx/geometry/vector2d.h"
#include "ui/gfx/geometry/vector2d_conversions.h"
#include "ui/gfx/geometry/vector2d_f.h"
#include "ui/views/animation/ink_drop.h"
#include "ui/views/background.h"
#include "ui/views/border.h"
#include "ui/views/controls/highlight_path_generator.h"
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

int g_shows = 0;

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

// Where the last Mini Player was left, for the next, across launches as Arc's LastPiPPosition:
// app-wide, in local state, so every profile's and a private window's share it.
constexpr char kLeftPref[] = "browser_core.mini_player_left";

std::optional<gfx::Rect> LastLeft() {
  const base::DictValue& left = g_browser_process->local_state()->GetDict(kLeftPref);
  const std::optional<int> x = left.FindInt("x");
  const std::optional<int> y = left.FindInt("y");
  const std::optional<int> width = left.FindInt("width");
  const std::optional<int> height = left.FindInt("height");
  if (!x || !y || !width || !height || *width <= 0 || *height <= 0) {
    return std::nullopt;
  }
  return gfx::Rect(*x, *y, *width, *height);
}

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
  // Chrome hides a tab's Mini Player and shows the same window when the tab floats a video again,
  // so one showing again, hidden yet, goes by the rule as a new one does.
  if (shown && window->IsVisible()) {
    // Kept on its screen as a move keeps it, where Chrome would keep it a margin from its corner.
    placed.AdjustToFit(work_area);
    return placed;
  }
  ++g_shows;
  const std::optional<gfx::Rect> left = LastLeft();
  // A kept place's screen, or the nearest, if the screens changed since.
  const gfx::Rect on =
      left ? display::Screen::Get()->GetDisplayMatching(*left).work_area() : work_area;
  placed = FirstMiniPlayerPlace(bounds.size(), left, on);
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
  const gfx::Rect left = window->GetWindowBoundsInScreen();
  g_browser_process->local_state()->SetDict(kLeftPref, base::DictValue()
                                                           .Set("x", left.x())
                                                           .Set("y", left.y())
                                                           .Set("width", left.width())
                                                           .Set("height", left.height()));
  // check.sh reads this line.
  LOG(ERROR) << "browser: mini player left at " << left.x() << " " << left.y() << " "
             << left.width() << " " << left.height();
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

// Dia's Mini Player, as measured (docs/ux-parity/phase-4.md): 6 pt corners and a 1 pt rim of white
// at 0.17 at rest; under the pointer a scrim of black at 0.35, and its buttons 28 × 28 and 29 × 28,
// 12 pt in, on white at 0.09, 0.13 under the pointer.
constexpr float kCornerRadius = 6;
constexpr SkColor kRim = SkColorSetA(SK_ColorWHITE, 0x2B);
constexpr SkColor kScrim = SkColorSetA(SK_ColorBLACK, 0x59);
constexpr int kButtonInset = 12;
constexpr gfx::Size kCloseSize(28, 28);
constexpr gfx::Size kBackSize(29, 28);
constexpr SkColor kButton = SkColorSetA(SK_ColorWHITE, 0x17);
// The white ink drop's highlight over the button's 0.09, together about 0.13.
constexpr float kButtonHover = 0.045f;

// The rim's view, among the window's own, found again at each layout.
constexpr int kRimId = 0x1b0;

// Gives |button| Dia's background and hover, once.
void Restyle(views::View* button, const gfx::Size& size) {
  if (!button) {
    return;
  }
  button->SetSize(size);
  if (button->GetBackground()) {
    return;
  }
  button->SetBackground(views::CreateRoundedRectBackground(kButton, kCornerRadius));
  views::InstallRoundRectHighlightPathGenerator(button, gfx::Insets(), kCornerRadius);
  if (views::InkDrop::Get(button)) {
    views::InkDrop::Get(button)->SetHighlightOpacity(kButtonHover);
  }
}

// Once Chrome has laid out |window|'s controls, as it shows and each time it's sized: Dia's look.
// The close button goes at the top left and back to the tab at the top right, as a Mac window's.
void LaidOut(VideoOverlayWindowViews* window) {
  views::View* root = window->GetRootView();
  views::View* contents = window->GetContentsView();
  if (!root || !root->layer() || !contents) {
    return;
  }
#if !BUILDFLAG(IS_WIN)
  // The root view's layer clips the video and controls under it, as ChromeOS rounds its own. On
  // Windows Chrome's window isn't translucent (crbug.com/425711450), where the corners it clipped
  // would be black, so it keeps the system's.
  root->layer()->SetRoundedCornerRadius(gfx::RoundedCornersF(kCornerRadius));
  root->layer()->SetIsFastRoundedCorner(true);
#endif
  views::View* rim = contents->GetViewByID(kRimId);
  if (!rim) {
    // Over the video and the controls, on a layer of its own, taking no events, which go on to
    // the controls under it.
    rim = contents->AddChildView(std::make_unique<views::View>());
    rim->SetID(kRimId);
    rim->SetPaintToLayer();
    rim->layer()->SetFillsBoundsOpaquely(false);
    rim->SetCanProcessEventsWithinSubtree(false);
    rim->SetBorder(views::CreateRoundedRectBorder(1, kCornerRadius, kRim));
    // One flat scrim, rather than Chrome's and its gradients at the top and bottom.
    window->GetControlsScrimView()->SetBackground(views::CreateSolidBackground(kScrim));
    window->GetControlsTopScrimView()->SetVisible(false);
    window->GetControlsBottomScrimView()->SetVisible(false);
  }
  const gfx::Size size = window->GetBounds().size();
  rim->SetBoundsRect(gfx::Rect(size));
  views::View* close = window->GetCloseControlsView();
  views::View* back = window->GetBackToTabControlsView();
  Restyle(close, kCloseSize);
  Restyle(back, kBackSize);
  if (close) {
    close->SetPosition(gfx::Point(kButtonInset, kButtonInset));
  }
  if (back) {
    back->SetPosition(gfx::Point(size.width() - kButtonInset - kBackSize.width(), kButtonInset));
  }
  // check.sh reads this line.
  LOG(ERROR) << "browser: mini player look " << size.ToString() << " close "
             << (close ? close->bounds().ToString() : "none") << " back "
             << (back ? back->bounds().ToString() : "none");
}

}  // namespace

void SetUpMiniPlayer() {
  g_browser_core_mini_player_event = &OnEvent;
  g_browser_core_mini_player_place = &Placed;
  g_browser_core_mini_player_left = &Left;
  g_browser_core_mini_player_laid_out = &LaidOut;
}

void RegisterMiniPlayerPrefs(PrefRegistrySimple* registry) {
  registry->RegisterDictionaryPref(kLeftPref);
}

bool MiniPlayerHasKeyboard() {
  return g_made && g_made->IsVisible() && g_made->IsActive();
}

int MiniPlayerShows() {
  return g_shows;
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
