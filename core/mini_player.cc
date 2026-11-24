#include "browser_core/mini_player.h"

#include <algorithm>
#include <cmath>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/functional/callback_helpers.h"
#include "base/location.h"
#include "base/logging.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/scoped_observation.h"
#include "base/strings/string_number_conversions.h"
#include "base/strings/stringprintf.h"
#include "base/task/single_thread_task_runner.h"
#include "base/time/time.h"
#include "base/values.h"
#include "browser_core/hooks.h"
#include "browser_core/mini_player_place.h"
#include "browser_core/platform/window_views.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "build/build_config.h"
#include "chrome/browser/browser_process.h"
#include "chrome/browser/picture_in_picture/picture_in_picture_window_manager.h"
#include "chrome/browser/ui/views/overlay/minimize_button.h"
#include "chrome/browser/ui/views/overlay/video_overlay_window_views.h"
#include "components/prefs/pref_registry_simple.h"
#include "components/prefs/pref_service.h"
#include "components/viz/common/frame_timing_details.h"
#include "content/public/browser/web_contents.h"
#include "third_party/skia/include/core/SkColor.h"
#include "ui/compositor/compositor.h"
#include "ui/compositor/compositor_animation_observer.h"
#include "ui/compositor/layer.h"
#include "ui/display/display.h"
#include "ui/display/screen.h"
#include "ui/events/event.h"
#include "ui/events/event_constants.h"
#include "ui/gfx/animation/animation.h"
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
#include "ui/views/controls/image_view.h"
#include "ui/views/controls/label.h"
#include "ui/views/view.h"
#include "ui/views/view_utils.h"
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

// Whether the pointer is in the Mini Player, as its window last heard, since it last showed.
bool g_pointer_in = false;

#if BUILDFLAG(IS_MAC)
// The window the Mini Player took the keyboard from, for the rest of the task it took it in: AppKit
// makes the Mini Player key as a press on it comes, in the same task as the press reaches it.
bc_id g_took_from = 0;
// The window a press that made the Mini Player key gives the keyboard back to as it ends.
bc_id g_give_back_to = 0;

// As the Mac's own floating video, a press on the Mini Player, a drag or a click on its buttons,
// leaves the keyboard where it was (#2896): the window that had it has it back as the press ends,
// while the app is still active, which the Mini Player's still having it says. One made key
// another way, by window cycling or VoiceOver, keeps it through a press, for its controls' keys.
void GiveKeyboardBack(views::Widget* window) {
  Window* to = Window::FromId(std::exchange(g_give_back_to, 0));
  if (!to || !to->widget() || !window->IsActive()) {
    return;
  }
  // check.sh reads this line.
  LOG(ERROR) << "browser: mini player pressed gave the keyboard back";
  to->widget()->Activate();
}
#endif

#if BUILDFLAG(IS_MAC)
// Brings |window| to the Space active as a task of its own, after the show that made it: in the
// show's task the move's MoveToActiveSpace would meet the show's own SetVisibleOnAllWorkspaces
// setting CanJoinAllSpaces, a pair AppKit throws on. A weak pointer, so a window that closed
// before the task moves nothing.
void WidgetMoveToActiveFullscreenSpace(base::WeakPtr<views::Widget> window) {
  if (window) {
    window->MoveToActiveFullscreenSpace();
  }
}
#endif

// Tells the windows as the Mini Player takes the keyboard, which on Linux can come after the window
// that let it go asked whether it had (MiniPlayerHasKeyboard). And logs it showing, and its first
// frame on screen, on the clock of the tab switch that floated it (LogHandover).
class MiniPlayerWatch : public views::WidgetObserver {
 public:
  void Watch(views::Widget* widget) {
    observation_.Reset();
    observation_.Observe(widget);
  }

  void OnWidgetVisibilityChanged(views::Widget* widget, bool visible) override {
    if (!visible) {
      return;
    }
#if BUILDFLAG(IS_MAC)
    // Over another app's window in full screen, as Arc's is. The Mac keeps a window to the Space
    // it was born on, and off one another app has made full screen, unless it may join one
    // (SetCanAppearInExistingFullscreenSpaces) and is on every Space at all: the PiP window's
    // InitParams set visible_on_all_workspaces, but only the Aura native widget reads it, so the
    // Mac's window gets the joining bits from its own show instead. Bits only let it stay; a
    // window born while another app holds the active Space needs a move to land there, so one
    // comes after the show's task. Not in it: the move holds MoveToActiveSpace until a later
    // task restores the bits, and the show's own SetVisibleOnAllWorkspaces inside the same task
    // would put CanJoinAllSpaces beside it, a pair AppKit refuses outright.
    widget->SetCanAppearInExistingFullscreenSpaces(true);
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&WidgetMoveToActiveFullscreenSpace,
                                  widget->GetWeakPtr()));
#endif
    // check.sh reads these lines.
    LogHandover("mini player shows", widget->GetWindowBoundsInScreen().size().ToString());
    if (ui::Compositor* compositor = widget->GetCompositor()) {
      compositor->RequestSuccessfulPresentationTimeForNextFrame(base::BindOnce(
          [](base::WeakPtr<views::Widget> widget, const viz::FrameTimingDetails& timing) {
            if (!widget) {
              return;
            }
            // Its size as its first frame is presented, not as it showed: Chrome can resize it
            // after the show, in the same task, as for its automatic Picture in Picture question
            // (#2822).
            std::string detail = widget->GetWindowBoundsInScreen().size().ToString();
            // The time is when this ran on the UI thread. Beside it, when the frame was
            // presented, so a UI thread late to run it shows apart from a late frame (#3181).
            if (const std::optional<base::TimeDelta> presented =
                    SinceHandoverClicked(timing.presentation_feedback.timestamp)) {
              detail += base::StringPrintf(", presented at %.1f ms", presented->InMillisecondsF());
            }
            LogHandover("mini player on screen", detail);
          },
          widget->GetWeakPtr()));
    }
  }

  void OnWidgetActivationChanged(views::Widget* widget, bool active) override {
    if (!active) {
      return;
    }
#if BUILDFLAG(IS_MAC)
    Window* from = Window::MiniPlayerTookKeyboard();
    g_took_from = from ? from->id() : 0;
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce([] { g_took_from = 0; }));
#else
    Window::MiniPlayerTookKeyboard();
#endif
  }

  void OnWidgetDestroying(views::Widget* widget) override { observation_.Reset(); }

 private:
  base::ScopedObservation<views::Widget, views::WidgetObserver> observation_{this};
};

MiniPlayerWatch& Watching() {
  static base::NoDestructor<MiniPlayerWatch> watch;
  return *watch;
}

// When the pointer event being handled happened, as the system says, or else now: a busy UI thread
// gets to a throw's last move and its release late, and times taken then read it as a rest (#3415).
[[maybe_unused]] base::TimeTicks PointerTime() {
  return platform::PointerEventTime().value_or(base::TimeTicks::Now());
}

// Where a Mini Player was each time its move loop moved it, for the speed it's let go at. Where the
// loop moves only its outline, and the window at the release, it's where the outline was (#3062).
class MoveWatch : public views::WidgetObserver {
 public:
  explicit MoveWatch(views::Widget* widget)
      : outline_(platform::WatchOutlineMoves(
            widget,
            base::BindRepeating(&MoveWatch::OutlineMoved, base::Unretained(this)))) {
    moves_.Window(widget->GetWindowBoundsInScreen().origin(), PointerTime());
    observation_.Observe(widget);
  }

  gfx::Vector2dF Velocity(base::TimeTicks released) const {
    return MiniPlayerReleaseVelocity(moves_.moves(), released);
  }
  gfx::Vector2dF LastMove(base::TimeTicks released) const {
    return MiniPlayerLastMove(moves_.moves(), released);
  }

  // views::WidgetObserver:
  void OnWidgetBoundsChanged(views::Widget* widget, const gfx::Rect& bounds) override {
    moves_.Window(bounds.origin(), PointerTime());
  }
  void OnWidgetDestroying(views::Widget* widget) override { observation_.Reset(); }

 private:
  void OutlineMoved(const gfx::Point& at) { moves_.Outline(at, PointerTime()); }

  MiniPlayerMoves moves_;
  base::ScopedObservation<views::Widget, views::WidgetObserver> observation_{this};
  base::ScopedClosureRunner outline_;
};

// Settles a Mini Player let go onto its rest, on the spring its release starts, moving it a frame
// at a time as the window's compositor draws, and no longer.
class Settle : public ui::CompositorAnimationObserver, public views::WidgetObserver {
 public:
  void Start(views::Widget* widget, const gfx::Rect& rest, const gfx::Vector2dF& velocity) {
    Stop();
    const gfx::Rect from = widget->GetWindowBoundsInScreen();
    ui::Compositor* compositor = widget->GetCompositor();
    // Under Reduce Motion it goes there at once, as Dia skips its throw.
    if (!compositor || gfx::Animation::PrefersReducedMotion()) {
      widget->SetBounds(rest);
      return;
    }
    widget_ = widget;
    compositor_ = compositor;
    rest_ = rest;
    offset_ = gfx::Vector2dF(from.x() - rest.x(), from.y() - rest.y());
    velocity_ = velocity;
    seconds_ = MiniPlayerSettleSeconds(offset_, velocity_);
    start_.reset();
    observation_.Observe(widget);
    compositor->AddAnimationObserver(this);
  }

  void Stop() {
    if (compositor_) {
      compositor_->RemoveAnimationObserver(this);
    }
    compositor_ = nullptr;
    widget_ = nullptr;
    observation_.Reset();
  }

  // Where |widget| is going, while it settles.
  std::optional<gfx::Rect> Rest(const views::Widget* widget) const {
    return widget_ && widget_ == widget ? std::optional<gfx::Rect>(rest_) : std::nullopt;
  }

  // ui::CompositorAnimationObserver:
  void OnAnimationStep(base::TimeTicks now) override {
    if (!start_) {
      start_ = now;
    }
    const double seconds = (now - *start_).InSecondsF();
    gfx::Rect at = rest_;
    views::Widget* widget = widget_;
    if (seconds >= seconds_) {
      Stop();
    } else {
      at.Offset(gfx::ToRoundedVector2d(MiniPlayerSpring(offset_, velocity_, seconds)));
    }
    widget->SetBounds(at);
  }
  void OnCompositingShuttingDown(ui::Compositor* compositor) override { Stop(); }

  // views::WidgetObserver:
  void OnWidgetDestroying(views::Widget* widget) override { Stop(); }

 private:
  raw_ptr<views::Widget> widget_ = nullptr;
  raw_ptr<ui::Compositor> compositor_ = nullptr;
  gfx::Rect rest_;
  gfx::Vector2dF offset_;
  gfx::Vector2dF velocity_;
  double seconds_ = 0;
  std::optional<base::TimeTicks> start_;
  base::ScopedObservation<views::Widget, views::WidgetObserver> observation_{this};
};

Settle& Settling() {
  static base::NoDestructor<Settle> settle;
  return *settle;
}

// |window| let go at |released| after its move loop, which |watch| watched: thrown or caught by a
// side, it settles at its rest, on the screen it's mostly on. The Mac's and Windows' alone: Linux's
// window manager or compositor moves it, and lets it go without the core hearing.
[[maybe_unused]] void LetGo(views::Widget* window, const MoveWatch& watch,
                            base::TimeTicks released) {
  const gfx::Rect bounds = window->GetWindowBoundsInScreen();
  const gfx::Vector2dF velocity = watch.Velocity(released);
  const std::optional<gfx::Rect> rest =
      MiniPlayerRest(bounds, velocity, watch.LastMove(released),
                     display::Screen::Get()->GetDisplayMatching(bounds).work_area());
  // check.sh reads this line.
  LOG(ERROR) << "browser: mini player let go at " << bounds.x() << " " << bounds.y()
             << " moving at " << std::lround(velocity.x()) << " " << std::lround(velocity.y())
             << (rest ? ", settling at " + base::NumberToString(rest->x()) + " " +
                            base::NumberToString(rest->y())
                      : std::string(", staying"));
  if (rest && *rest != bounds) {
    Settling().Start(window, *rest, velocity);
  }
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
  // A new one isn't the last one's settle.
  Settling().Stop();
  ++g_shows;
  g_pointer_in = false;
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
  // Where it was going, if it was settling, which stops with it.
  const gfx::Rect left = Settling().Rest(window).value_or(window->GetWindowBoundsInScreen());
  Settling().Stop();
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
  // The user's own moves and sizes take it from a settle.
  switch (event->type()) {
    case ui::EventType::kMousewheel:
    case ui::EventType::kScroll:
    case ui::EventType::kGesturePinchUpdate:
    case ui::EventType::kMousePressed:
      Settling().Stop();
      break;
    default:
      break;
  }
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
#if BUILDFLAG(IS_MAC)
      g_give_back_to = std::exchange(g_took_from, 0);
#endif
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
        const base::WeakPtr<views::Widget> moved = window->GetWeakPtr();
        MoveWatch watch(window);
        window->RunMoveLoop(press.OffsetFromOrigin(), views::Widget::MoveLoopSource::kMouse,
                            views::Widget::MoveLoopEscapeBehavior::kDontHide);
        // The loop returns as the button comes up, with that event the one being handled.
        if (moved) {
          LetGo(moved.get(), watch, PointerTime());
#if BUILDFLAG(IS_MAC)
          GiveKeyboardBack(moved.get());
#endif
        }
      }
#endif
      return true;
    }
    case ui::EventType::kMouseReleased:
      g_pressed.reset();
#if BUILDFLAG(IS_MAC)
      GiveKeyboardBack(window);
#endif
      return false;
    case ui::EventType::kMouseEntered:
    case ui::EventType::kMouseMoved:
      // check.sh reads this line, once each time the pointer comes in: Chrome has had the event and
      // shown the controls for it, so a pointer that never reaches the window has no line, and one
      // that reaches it and brings nothing says the controls are hidden (#3354). This is the
      // window's own handler, so its views' crossings never come here, and each OS's exit is the
      // window's: Windows drops a leave while the pointer is still over it, and Wayland's
      // subsurfaces take no input. A Mini Player shown under a still pointer has no line until the
      // OS sends it an enter or a move; the Mac's doesn't, as its view's tracking area doesn't
      // assume the pointer inside.
      if (!g_pointer_in) {
        g_pointer_in = true;
        LOG(ERROR) << "browser: mini player pointer in, controls "
                   << (window->AreControlsVisible() ? "shown" : "hidden");
      }
      return false;
    case ui::EventType::kMouseExited:
      g_pointer_in = false;
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

// Between the × and Chrome's favicon, and the origin's end and the button after it.
constexpr int kTitleGap = 8;
// Between Chrome's favicon and its origin, as Chrome has it.
constexpr int kFaviconGap = 4;

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

// The first child of |parent| that's a |T|, as Chrome's views have no getters for these.
template <typename T>
T* ChildOf(views::View* parent) {
  if (!parent) {
    return nullptr;
  }
  for (views::View* child : parent->children()) {
    if (T* found = views::AsViewClass<T>(child)) {
      return found;
    }
  }
  return nullptr;
}

// Chrome's origin, the label in its title by the favicon.
views::Label* Origin(VideoOverlayWindowViews* window) {
  return ChildOf<views::Label>(window->GetTitleView());
}

// Chrome's origin, its word on whose video this is, and its favicon, beside the × rather than under
// it, now that the × is at the top left: on the buttons' row, the origin ending short of the "–",
// or of back to the tab without one, and eliding its start as Chrome's does. The "–" goes on the
// row too. Returns the "–".
views::View* PlaceTitle(VideoOverlayWindowViews* window,
                        const gfx::Size& size,
                        views::View* close) {
  views::View* minimize = ChildOf<OverlayWindowMinimizeButton>(window->GetControlsContainerView());
  const int middle = kButtonInset + kCloseSize.height() / 2;
  int end = size.width() - kButtonInset - kBackSize.width();
  if (minimize) {
    minimize->SetY(middle - minimize->height() / 2);
    if (minimize->GetVisible()) {
      end = std::min(end, minimize->x());
    }
  }
  views::View* title = window->GetTitleView();
  views::ImageView* favicon = ChildOf<views::ImageView>(title);
  views::Label* origin = Origin(window);
  if (!close || !favicon || !origin) {
    return minimize;
  }
  favicon->SetPosition(
      gfx::Point(close->bounds().right() + kTitleGap, middle - favicon->height() / 2));
  const int left = favicon->bounds().right() + kFaviconGap;
  origin->SetBounds(left, middle - origin->height() / 2, std::max(0, end - kTitleGap - left),
                    origin->height());
  // The title clips what's in it, and Chrome sizes it to its own row.
  title->SetBounds(0, 0, size.width(), 2 * kButtonInset + kCloseSize.height());
  return minimize;
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
    // One flat scrim, rather than Chrome's and its gradients at the top and bottom. The gradients'
    // views lose their backgrounds rather than being hidden, since the controls' fade shows each
    // view it fades in (#2881).
    window->GetControlsScrimView()->SetBackground(views::CreateSolidBackground(kScrim));
    window->GetControlsTopScrimView()->SetBackground(nullptr);
    window->GetControlsBottomScrimView()->SetBackground(nullptr);
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
  views::View* minimize = PlaceTitle(window, size, close);
  // check.sh reads this line.
  views::View* origin = Origin(window);
  LOG(ERROR) << "browser: mini player look " << size.ToString() << " close "
             << (close ? close->bounds().ToString() : "none") << " back "
             << (back ? back->bounds().ToString() : "none") << " origin "
             << (origin ? origin->bounds().ToString() : "none") << " minimize "
             << (minimize && minimize->GetVisible() ? minimize->bounds().ToString() : "none");
}

// Where a page's own floating window, such as a call's, was placed and where it first moved, as a
// log for #3623: on X11 a call has now and then floated at the screen's top left rather than its
// bottom right. Chrome places the window on the display nearest its opener's window as the page
// asks (browser_navigator.cc), so the core reads that display then too, and the window a task
// later, once Chrome has made it.
class FloatingPageWatch : public PictureInPictureWindowManager::Observer,
                          public views::WidgetObserver {
 public:
  void OnEnterPictureInPicture() override {
    PictureInPictureWindowManager* manager = PictureInPictureWindowManager::GetInstance();
    content::WebContents* page = manager->GetChildWebContents();
    content::WebContents* opener = manager->GetWebContents();
    if (!page || !opener) {
      return;
    }
    Window* window = Window::Holding(Tab::IdOf(opener));
    views::Widget* widget = window ? window->widget() : nullptr;
    const display::Screen* screen = display::Screen::Get();
    std::string from;
    display::Display display;
    if (widget && widget->GetNativeWindow()) {
      from = "from a window at " + widget->GetWindowBoundsInScreen().ToString() +
             (widget->IsVisible() ? " shown" : " hidden");
      display = screen->GetDisplayNearestWindow(widget->GetNativeWindow());
    } else {
      from = "from no window";
      display = screen->GetDisplayForNewWindows();
    }
    from += display.is_valid() ? " on a display at " + display.bounds().ToString() +
                                     " with its work area at " + display.work_area().ToString()
                               : " with no display";
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&FloatingPageWatch::Made, base::Unretained(this),
                                  page->GetWeakPtr(), std::move(from)));
  }

  void OnWidgetBoundsChanged(views::Widget* widget, const gfx::Rect&) override {
    const gfx::Rect bounds = widget->GetWindowBoundsInScreen();
    if (bounds.origin() == placed_.origin()) {
      return;
    }
    // The first move alone: a window manager's place, or the user's first drag.
    observation_.Reset();
    LOG(ERROR) << "browser: floating page moved to " << bounds.ToString() << " on " << On(widget);
  }

  void OnWidgetDestroying(views::Widget* widget) override { observation_.Reset(); }

 private:
  void Made(base::WeakPtr<content::WebContents> page, const std::string& from) {
    views::Widget* widget =
        page ? views::Widget::GetTopLevelWidgetForNativeView(page->GetNativeView()) : nullptr;
    if (!widget) {
      LOG(ERROR) << "browser: floating page had no window a task after it asked, opened " << from;
      return;
    }
    placed_ = widget->GetWindowBoundsInScreen();
    observation_.Reset();
    observation_.Observe(widget);
    LOG(ERROR) << "browser: floating page placed at " << placed_.ToString() << " on " << On(widget)
               << (widget->IsVisible() ? ", shown" : ", not shown yet") << ", opened " << from;
  }

  // The display |widget| is on, as Chrome reads it for the window's later places.
  static std::string On(views::Widget* widget) {
    const display::Display display =
        display::Screen::Get()->GetDisplayNearestWindow(widget->GetNativeWindow());
    return display.is_valid() ? "a display at " + display.bounds().ToString() +
                                    " with its work area at " + display.work_area().ToString()
                              : "no display";
  }

  gfx::Rect placed_;
  base::ScopedObservation<views::Widget, views::WidgetObserver> observation_{this};
};

FloatingPageWatch& FloatingPage() {
  static base::NoDestructor<FloatingPageWatch> watch;
  return *watch;
}

}  // namespace

void SetUpMiniPlayer() {
  g_browser_core_mini_player_event = &OnEvent;
  g_browser_core_mini_player_place = &Placed;
  g_browser_core_mini_player_left = &Left;
  g_browser_core_mini_player_laid_out = &LaidOut;
}

void WatchFloatingPages() {
  PictureInPictureWindowManager::GetInstance()->AddObserver(&FloatingPage());
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
