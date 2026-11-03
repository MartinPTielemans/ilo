#include "browser_core/platform/window_views.h"

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <map>
#include <memory>
#include <optional>
#include <utility>
#include <vector>

#include "base/functional/bind.h"
#include "base/functional/callback.h"
#include "base/functional/callback_helpers.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/scoped_multi_source_observation.h"
#include "base/scoped_observation.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/host.h"
#include "browser_core/platform/focus_ring_aura.h"
#include "browser_core/platform/linux/ui_text_input.h"
#include "browser_core/platform/linux/wayland.h"
#include "browser_core/window.h"
#include "chrome/browser/picture_in_picture/picture_in_picture_window_manager.h"
#include "chrome/browser/ui/views/native_widget_factory.h"
#include "content/public/browser/web_contents.h"
#include "third_party/skia/include/core/SkColor.h"
#include "ui/aura/client/focus_change_observer.h"
#include "ui/aura/client/focus_client.h"
#include "ui/aura/window.h"
#include "ui/aura/window_observer.h"
#include "ui/aura/window_tracker.h"
#include "ui/aura/window_tree_host_platform.h"
#include "ui/compositor/layer.h"
#include "ui/compositor/layer_solid_color.h"
#include "ui/events/event.h"
#include "ui/events/event_handler.h"
#include "ui/events/keycodes/dom/keycode_converter.h"
#include "ui/gfx/geometry/insets.h"
#include "ui/gfx/geometry/point.h"
#include "ui/gfx/geometry/rect.h"
#include "ui/gfx/geometry/rect_conversions.h"
#include "ui/gfx/geometry/rounded_corners_f.h"
#include "ui/gfx/geometry/size_conversions.h"
#include "ui/gfx/x/atom_cache.h"
#include "ui/gfx/x/connection.h"
#include "ui/gfx/x/event.h"
#include "ui/gfx/x/event_observer.h"
#include "ui/gfx/x/shape.h"
#include "ui/gfx/x/window_event_manager.h"
#include "ui/gfx/x/xproto.h"
#include "ui/base/hit_test.h"
#include "ui/platform_window/platform_window.h"
#include "ui/platform_window/wm/wm_move_resize_handler.h"
#include "ui/views/views_delegate.h"
#include "ui/views/widget/desktop_aura/desktop_native_widget_aura.h"
#include "ui/views/widget/widget.h"
#include "ui/views/widget/widget_delegate.h"
#include "ui/views/widget/widget_observer.h"
#include "ui/views/window/non_client_view.h"

// As in Chrome, each page shown goes in an aura window in the browser window, over its views, so
// the keys reach a page through the window's own focus, after the browser's shortcuts. Aura draws
// the pages into the window's own output, under the app's UI, which shows only where they aren't:
// - On Wayland the UI shares Chromium's connection and is a subsurface under the window's surface,
//   which is clear but on the pages, and takes clicks only there (wayland.cc).
// - On X11 the UI is a child X window, on a connection of its own, over the browser window's, with
//   holes in its shape where the pages show. Chromium's GPU process maps a child X window of its
//   own for the window's output, and the core sends it under the UI's.
// The keyboard is the window's, for its pages, but for the UI's from a click in the UI until a
// click in a page or the core focusing one. On X11 the X server sends the keys to the X window with
// the input focus, which GPUI takes on a click, and the core gives it back to the browser window as
// a page takes the keyboard. On Wayland every key reaches both, and the app gives the UI the
// keyboard (bc_window_ui_takes_keyboard) and asks whose it is.
// The app's overlays, such as the command bar, show over the pages: on Wayland as subsurfaces over
// the window's, on X11 as X windows of their own over it, which GPUI places.
// One that has the keyboard has it lent (bc_window_overlay_takes_keyboard): the page or UI that had
// it keeps its focus, and has it back as the overlay closes.

namespace browser_core::platform {

namespace {

x11::Window XWindowOf(views::Widget* widget) {
  return static_cast<x11::Window>(widget->GetNativeWindow()->GetHost()->GetAcceleratedWidget());
}

// Whether |window| is |ancestor| or inside it. A round trip to the X server a level.
bool IsWithin(x11::Window window, x11::Window ancestor) {
  x11::Connection* connection = x11::Connection::Get();
  while (window != x11::Window::None) {
    if (window == ancestor) {
      return true;
    }
    auto tree = connection->QueryTree({window}).Sync();
    if (!tree || tree->parent == tree->root) {
      return false;
    }
    window = tree->parent;
  }
  return false;
}

x11::Window InputFocus() {
  auto focus = x11::Connection::Get()->GetInputFocus().Sync();
  return focus ? focus->focus : x11::Window::None;
}

void SetInputFocus(x11::Window window) {
  x11::Connection::Get()
      ->SetInputFocus({x11::InputFocus::Parent, window, x11::Time::CurrentTime})
      .IgnoreError();
}

// |event| as the X server sends a key typed, but for the window it goes to.
std::optional<x11::KeyEvent> X11KeyOf(const ui::KeyEvent& event) {
  const int code = ui::KeycodeConverter::DomCodeToNativeKeycode(event.code());
  if (code == ui::KeycodeConverter::InvalidNativeKeycode()) {
    return std::nullopt;
  }
  constexpr auto kNone = x11::KeyButMask{};
  return x11::KeyEvent{
      .opcode = event.type() == ui::EventType::kKeyPressed ? x11::KeyEvent::Press
                                                           : x11::KeyEvent::Release,
      .detail = static_cast<x11::KeyCode>(code),
      .root = x11::Connection::Get()->default_root(),
      .state = (event.IsShiftDown() ? x11::KeyButMask::Shift : kNone) |
               (event.IsCapsLockOn() ? x11::KeyButMask::Lock : kNone) |
               (event.IsControlDown() ? x11::KeyButMask::Control : kNone) |
               (event.IsAltDown() ? x11::KeyButMask::Mod1 : kNone),
      .same_screen = true,
  };
}

ui::PlatformWindow* PlatformWindowOf(views::Widget* widget) {
  return static_cast<aura::WindowTreeHostPlatform*>(widget->GetNativeWindow()->GetHost())
      ->platform_window();
}

float ScaleOf(views::Widget* widget) {
  return widget->GetNativeWindow()->GetHost()->device_scale_factor();
}

gfx::Size ContentPixelsOf(views::Widget* widget) {
  return widget->GetNativeWindow()->GetHost()->GetBoundsInPixels().size();
}

// The window's content, in DIPs.
gfx::Size ContentSizeOf(views::Widget* widget) {
  return gfx::ScaleToFlooredSize(ContentPixelsOf(widget), 1 / ScaleOf(widget));
}

// An aura window in the browser window holding one pane's page and its docked DevTools, or the side
// panel, with their round corners.
struct Slot {
  std::unique_ptr<aura::Window> holder;
  raw_ptr<aura::Window> page = nullptr;
  raw_ptr<aura::Window> devtools = nullptr;
  // From each edge of the window's content to the slot's, kept as the window resizes.
  gfx::Insets insets;
  // With DevTools docked, where the page goes in the slot, from its top-left.
  gfx::Rect page_in_devtools;
  // Only the bottom corners are round, for the side panel under the app's header.
  bool bottom_corners_only = false;
  // Its corners' own, for a link preview's page, else the window's.
  std::optional<double> corner_radius;
};

// The pages and DevTools a window shows once it changes, which move between its slots unhidden.
using Staying = std::vector<aura::Window*>;

// |size| with round corners of |radius|, as the rows X's shape extension takes.
std::vector<gfx::Rect> RoundShape(const gfx::Size& size, double radius, bool bottom_only) {
  std::vector<gfx::Rect> rects;
  const int r = std::min<int>(std::lround(radius), std::min(size.width(), size.height()) / 2);
  auto inset_at = [r](int row) {
    const double dy = r - row - 0.5;
    return static_cast<int>(std::lround(r - std::sqrt(r * r - dy * dy)));
  };
  for (int row = 0; row < r; ++row) {
    const int inset = bottom_only ? 0 : inset_at(row);
    rects.emplace_back(inset, row, size.width() - 2 * inset, 1);
  }
  rects.emplace_back(0, r, size.width(), size.height() - 2 * r);
  for (int row = 0; row < r; ++row) {
    const int inset = inset_at(row);
    rects.emplace_back(inset, size.height() - 1 - row, size.width() - 2 * inset, 1);
  }
  return rects;
}

x11::Rectangle XRectangle(const gfx::Rect& rect) {
  return {static_cast<int16_t>(rect.x()), static_cast<int16_t>(rect.y()),
          static_cast<uint16_t>(rect.width()), static_cast<uint16_t>(rect.height())};
}

class WindowViews : public views::WidgetObserver,
                    public aura::WindowObserver,
                    public aura::client::FocusChangeObserver,
                    public ui::EventHandler,
                    public x11::EventObserver {
 public:
  using ui::EventHandler::OnEvent;

  // |ui| is the UI's X window, or none on Wayland.
  WindowViews(views::Widget* widget, x11::Window ui)
      : widget_(widget), ui_(ui), ui_text_input_(widget) {
    observation_.Observe(widget);
    content()->AddPreTargetHandler(this);
    if (auto* client = aura::client::GetFocusClient(content())) {
      focus_observation_.Observe(client);
    }
    if (ui_ != x11::Window::None) {
      x11::Connection* connection = x11::Connection::Get();
      children_events_ =
          connection->ScopedSelectEvent(window(), x11::EventMask::SubstructureNotify);
      x11_observation_.Observe(connection);
      // The app makes it unmapped, so no window manager has taken it as a window of its own.
      connection->ReparentWindow({ui_, window(), 0, 0});
      connection->MapWindow({ui_});
      SizeUI();
    }
  }

  aura::Window* content() const { return widget_->GetNativeView(); }
  x11::Window window() const { return XWindowOf(widget_); }
  bool ui_has_keyboard() const { return ui_has_keyboard_; }
  bool ui_holds_for_page() const { return ui_holds_for_page_; }
  bool overlay_has_keyboard() const { return overlay_has_keyboard_; }
  std::vector<Slot>& panes() { return panes_; }
  Slot& side_panel() { return side_panel_; }
  Slot& preview() { return preview_; }
  FocusRing& ring() { return ring_; }
  void set_corner_radius(double radius) { corner_radius_ = radius; }
  bool held() const { return held_; }
  void set_held(bool held) { held_ = held; }
  void set_preview_color(SkColor4f color) { preview_color_ = color; }
  UITextInput& ui_text_input() { return ui_text_input_; }

  // A slot's holder, made the first time it's needed, over the window's own views.
  aura::Window* HolderFor(Slot& slot) {
    if (!slot.holder) {
      slot.holder = std::make_unique<aura::Window>(nullptr);
      slot.holder->set_owned_by_parent(false);
      // A colour where its page hasn't drawn, as an aura page draws nothing before its first
      // frame, where the Mac's view shows its background.
      slot.holder->Init(ui::LAYER_SOLID_COLOR);
      slot.holder->layer()->AsSolidColor()->SetColor(SkColors::kTransparent);
      slot.holder->SetName("BrowserCorePane");
      slot.holder->layer()->SetMasksToBounds(true);
      slot.holder->layer()->SetIsFastRoundedCorner(true);
      content()->AddChild(slot.holder.get());
    }
    return slot.holder.get();
  }

  // Takes |view| out of |holder| if it's there, so closing a slot never deletes a page, and hides
  // it unless it's |staying| in the window. A page out of every window that's never hidden still
  // counts as visible, so its tab would go on drawing and running its timers and never unload.
  // It's hidden before it leaves: its visibility is its occlusion state, which Aura's tracker
  // stops updating once the view is out of the window, so one hidden after would stay visible.
  static void Detach(aura::Window* holder, aura::Window* view, const Staying& staying) {
    if (holder && view && view->parent() == holder) {
      if (!std::ranges::contains(staying, view)) {
        view->Hide();
      }
      holder->RemoveChild(view);
    }
  }

  // Holds |page|, which no window shows, hidden in the window under its slots until the runner
  // returned runs, so it has the window's compositor: a page out of every window has none, and a
  // copy of it waits for a frame until it's shown again (#745). Hidden, it costs no frames unless
  // it's captured.
  base::ScopedClosureRunner DrawUnseen(aura::Window* page) {
    page->Hide();
    content()->AddChild(page);
    content()->StackChildAtBottom(page);
    unseen_.Add(page);
    auto tracked = std::make_unique<aura::WindowTracker>();
    tracked->Add(page);
    return base::ScopedClosureRunner(base::BindOnce(
        [](base::WeakPtr<WindowViews> views, std::unique_ptr<aura::WindowTracker> tracked) {
          if (views && !tracked->windows().empty()) {
            views->Unsee(tracked->windows().front());
          }
        },
        weak_factory_.GetWeakPtr(), std::move(tracked)));
  }

  // Takes |page| out of the window, unless a slot has shown it since DrawUnseen held it.
  void Unsee(aura::Window* page) {
    if (!unseen_.Contains(page)) {
      return;
    }
    unseen_.Remove(page);
    if (page->parent() == content()) {
      content()->RemoveChild(page);
    }
  }

  // Holds |page| and |devtools| in |slot|. A slot forgets one destroyed before the window is, as
  // each tab's page is on quit, or emptying it would reach into the freed page.
  void Keep(Slot& slot, aura::Window* page, aura::Window* devtools) {
    for (aura::Window* view : {page, devtools}) {
      if (view && !kept_.IsObservingSource(view)) {
        kept_.AddObservation(view);
      }
    }
    slot.page = page;
    slot.devtools = devtools;
  }

  void Empty(Slot& slot, const Staying& staying = {}) {
    Detach(slot.holder.get(), slot.page, staying);
    Detach(slot.holder.get(), slot.devtools, staying);
    slot.page = nullptr;
    slot.devtools = nullptr;
    if (slot.holder) {
      slot.holder->Hide();
    }
  }

  // Shows |page|, a link preview's, at |frame| over the pages shown, in place of the preview it
  // shows. The app's overlay over the window has a hole where it shows, and its card round that.
  void ShowPreview(aura::Window* page, const gfx::Rect& frame, double corner_radius) {
    aura::Window* holder = HolderFor(preview_);
    if (preview_.page != page) {
      Empty(preview_);
      if (page->parent()) {
        page->parent()->RemoveChild(page);
      }
      holder->AddChild(page);
      Keep(preview_, page, nullptr);
    }
    page->Show();
    holder->layer()->AsSolidColor()->SetColor(preview_color_);
    preview_.corner_radius = corner_radius;
    const gfx::Size size = ContentSizeOf(widget_);
    preview_.insets = gfx::Insets::TLBR(frame.y(), frame.x(), size.height() - frame.bottom(),
                                        size.width() - frame.right());
    content()->StackChildAtTop(holder);
    holder->Show();
    LayOut(preview_, size);
    ShowThrough();
  }

  // Sizes each slot to its insets.
  void LayOut() {
    const gfx::Size size = ContentSizeOf(widget_);
    for (Slot& slot : panes_) {
      LayOut(slot, size);
    }
    LayOut(side_panel_, size);
    LayOut(preview_, size);
    ring_.Fit();
  }

  void LayOut(Slot& slot, const gfx::Size& window_size) {
    if (!slot.holder || !slot.page) {
      return;
    }
    gfx::Rect frame(window_size);
    frame.Inset(slot.insets);
    slot.holder->SetBounds(frame);
    const gfx::Rect bounds(frame.size());
    if (slot.devtools) {
      slot.devtools->SetBounds(bounds);
      gfx::Rect page = slot.page_in_devtools;
      page.Intersect(bounds);
      slot.page->SetBounds(page);
    } else {
      slot.page->SetBounds(bounds);
    }
    const float r = slot.corner_radius.value_or(corner_radius_);
    slot.holder->layer()->SetRoundedCornerRadius(
        slot.bottom_corners_only ? gfx::RoundedCornersF(0, 0, r, r) : gfx::RoundedCornersF(r));
  }

  // Lets the UI show, and take the clicks, but where the pages shown are and in the band at the
  // window's edges that resizes it, whose presses Chromium's hit test takes (wayland.cc).
  void ShowThrough() {
    const float scale = ScaleOf(widget_);
    const int edge = static_cast<int>(std::lround(ResizeEdgeOf(widget_) * scale));
    const gfx::Size pixels = ContentPixelsOf(widget_);
    std::vector<const Slot*> shown;
    auto add = [&](const Slot& slot) {
      if (slot.holder && slot.page && slot.holder->IsVisible()) {
        shown.push_back(&slot);
      }
    };
    for (const Slot& slot : panes_) {
      add(slot);
    }
    add(side_panel_);
    add(preview_);
    if (OnWayland()) {
      std::vector<gfx::Rect> region;
      for (const Slot* slot : shown) {
        region.push_back(gfx::ScaleToEnclosingRect(slot->holder->bounds(), scale));
      }
      if (edge) {
        const int w = pixels.width();
        const int h = pixels.height();
        region.push_back(gfx::Rect(0, 0, w, edge));
        region.push_back(gfx::Rect(0, h - edge, w, edge));
        region.push_back(gfx::Rect(0, edge, edge, h - 2 * edge));
        region.push_back(gfx::Rect(w - edge, edge, edge, h - 2 * edge));
      }
      // No rects is the whole surface; one empty rect is none of it.
      if (region.empty()) {
        region.emplace_back();
      }
      PlatformWindowOf(widget_)->SetInputRegion(region);
      return;
    }
    if (ui_ == x11::Window::None) {
      return;
    }
    // shortcut: a shape's corners are aliased, as Windows' regions are and Core Animation's on the
    // Mac aren't; the page's own round corners show through them.
    x11::Shape& shape = x11::Connection::Get()->shape();
    shape.Rectangles(x11::Shape::RectanglesRequest{
        .operation = x11::Shape::So::Set,
        .destination_kind = x11::Shape::Sk::Bounding,
        .ordering = x11::ClipOrdering::Unsorted,
        .destination_window = ui_,
        .rectangles = {XRectangle(gfx::Rect(pixels))},
    });
    // The band takes no input in the UI's window, so it goes to the browser window under it, still
    // showing the UI.
    gfx::Rect input(pixels);
    input.Inset(gfx::Insets(edge));
    shape.Rectangles(x11::Shape::RectanglesRequest{
        .operation = x11::Shape::So::Set,
        .destination_kind = x11::Shape::Sk::Input,
        .ordering = x11::ClipOrdering::Unsorted,
        .destination_window = ui_,
        .rectangles = {XRectangle(input)},
    });
    for (const Slot* slot : shown) {
      const gfx::Rect frame = gfx::ScaleToEnclosingRect(slot->holder->bounds(), scale);
      std::vector<x11::Rectangle> hole;
      for (gfx::Rect row :
           RoundShape(frame.size(), slot->corner_radius.value_or(corner_radius_) * scale,
                      slot->bottom_corners_only)) {
        row.Offset(frame.OffsetFromOrigin());
        hole.push_back(XRectangle(row));
      }
      shape.Rectangles(x11::Shape::RectanglesRequest{
          .operation = x11::Shape::So::Subtract,
          .destination_kind = x11::Shape::Sk::Bounding,
          .ordering = x11::ClipOrdering::Unsorted,
          .destination_window = ui_,
          .rectangles = std::move(hole),
      });
    }
  }

  // Lends the keyboard to an overlay of the app's, or takes it back, for the page or UI that had it.
  // On X11 the input focus says which that is once the overlay lets it go, as a click while it
  // showed may have moved it.
  void OverlayTakesKeyboard(bool takes) {
    overlay_has_keyboard_ = takes;
    early_keys_.clear();
    ui_text_input_.SetKeyboard(ui_has_keyboard_ || overlay_has_keyboard_);
    if (!takes && ui_ != x11::Window::None && !ui_has_keyboard_ && IsWithin(InputFocus(), ui_)) {
      UITakesKeyboard();
    }
  }

  // The UI lets the keyboard go to the window, which KeyboardForPage counts as the page's, and on
  // X11 the window has the input focus, even where a press in the UI that gives a page the
  // keyboard, as one on a tab's row does, had GPUI move it into the UI's window first.
  void PageTakesKeyboard() {
    if (ui_has_keyboard_) {
      ui_has_keyboard_ = false;
      ui_holds_for_page_ = false;
      ui_text_input_.SetKeyboard(overlay_has_keyboard_);
    }
    if (ui_ != x11::Window::None && IsWithin(InputFocus(), ui_)) {
      SetInputFocus(window());
    }
  }

  // The page that had the keyboard lets it go, and on X11 GPUI's window has the input focus, if
  // the window has it. Taken from another window, as by a window whose page moved out, that
  // window's handling of its focus event, which comes after it has the focus back, would take it
  // back too, and the two would go on without end (#531).
  enum class Asked { kNo, kToKeep, kToHold };
  void UITakesKeyboard(Asked asked = Asked::kNo) {
    // With no page shown, the keys only reach the window's shortcuts through the UI, which holds
    // them for the next page shown, as the Mac's window does. Taken again while it has them, as the
    // window manager focusing the window as it maps has it, it still holds them or keeps them.
    // Asked by the app, as for the New Space form's field, it keeps them, as the Mac's UI view
    // does once it's the first responder. Asked to hold them, as the app does as it shows a Space
    // with no page, the window's first too, it holds them for the next page shown, even with the
    // page before still in the window, unless it keeps them, as the form's Space shows (#895).
    if (asked == Asked::kToKeep) {
      ui_holds_for_page_ = false;
    } else if (asked == Asked::kToHold) {
      ui_holds_for_page_ = ui_holds_for_page_ || !ui_has_keyboard_;
    } else if (!ui_has_keyboard_) {
      ui_holds_for_page_ =
          std::ranges::none_of(panes_, [](const Slot& slot) { return slot.page; });
    }
    ui_has_keyboard_ = true;
    ui_lent_ = false;
    page_lent_.RemoveAll();
    if (auto* client = aura::client::GetFocusClient(content())) {
      client->ResetFocusWithinActiveWindow(content());
    }
    ui_text_input_.SetKeyboard(true);
    if (ui_ == x11::Window::None) {
      return;
    }
    const x11::Window focus = InputFocus();
    if (IsWithin(focus, ui_) || !IsWithin(focus, window())) {
      return;
    }
    if (x11::Window main = UIMain(); main != x11::Window::None) {
      SetInputFocus(main);
    }
  }

  // views::WidgetObserver:
  void OnWidgetBoundsChanged(views::Widget*, const gfx::Rect&) override {
    if (!held_) {
      LayOut();
    }
    if (!OnWayland()) {
      if (ui_ != x11::Window::None) {
        SizeUI();
        ShowThrough();
      }
      return;
    }
    // Chromium sets the input region back to the whole window as the bounds change, after telling
    // observers (DesktopWindowTreeHostLinux::UpdateFrameHints).
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&WindowViews::ShowThrough, weak_factory_.GetWeakPtr()));
    // No compositor sizes a subsurface, so the app sizes its UI to the window's content.
    if (!Host().window_resized) {
      return;
    }
    const gfx::Size size = ContentSizeOf(widget_);
    for (Window* window : Window::All()) {
      if (window->widget() == widget_) {
        Host().window_resized(Host().context, window->id(), size.width(), size.height());
      }
    }
  }

  // Maximized or full screen, the window has no band to resize by, and back from either has it
  // again, which the window manager may say after the bounds have changed.
  void OnWidgetShowStateChanged(views::Widget*) override { ShowThrough(); }

  void OnWidgetDestroying(views::Widget*) override {
    ui_text_input_.SetKeyboard(false);
    x11_observation_.Reset();
    children_events_ = x11::ScopedEventSelector();
    focus_observation_.Reset();
    content()->RemovePreTargetHandler(this);
    ring_.Hide();
    for (Slot& slot : panes_) {
      Empty(slot);
    }
    Empty(side_panel_);
    Empty(preview_);
    while (!unseen_.windows().empty()) {
      Unsee(unseen_.windows().front());
    }
    panes_.clear();
    side_panel_ = Slot();
    preview_ = Slot();
    kept_.RemoveAllObservations();
    observation_.Reset();
  }

  // aura::WindowObserver: a page or DevTools a slot holds is going.
  void OnWindowDestroying(aura::Window* window) override {
    kept_.RemoveObservation(window);
    auto forget = [window](Slot& slot) {
      if (slot.page == window) {
        slot.page = nullptr;
      }
      if (slot.devtools == window) {
        slot.devtools = nullptr;
      }
    };
    std::ranges::for_each(panes_, forget);
    forget(side_panel_);
    forget(preview_);
    // A link preview's page goes as its tab closes, and its hole with it.
    if (preview_.holder && !preview_.page && preview_.holder->IsVisible()) {
      preview_.holder->Hide();
      ShowThrough();
    }
  }

  // aura::client::FocusChangeObserver: the core focusing a page gives it the keyboard, as Chromium
  // focusing a window of its own in the browser window does, such as an extension's popup or a
  // tab's dialog. That gives it back as it goes to the UI or page that had it, as Chrome's views
  // restore the focus, which here would leave it with the window, where no page hears the keys.
  void OnWindowFocused(aura::Window* gained_focus, aura::Window* lost_focus) override {
    if (gained_focus && gained_focus != content()) {
      if (InSlot(gained_focus)) {
        ui_lent_ = false;
        page_lent_.RemoveAll();
      } else if (ui_has_keyboard_) {
        ui_lent_ = true;
      } else if (lost_focus && InSlot(lost_focus)) {
        page_lent_.RemoveAll();
        page_lent_.Add(lost_focus);
      }
      PageTakesKeyboard();
    } else if (ui_lent_) {
      UITakesKeyboard();
    } else if (!page_lent_.windows().empty()) {
      // Not while the focus client tells of the change.
      base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
          FROM_HERE,
          base::BindOnce(&WindowViews::PageTakesBackKeyboard, weak_factory_.GetWeakPtr()));
    }
  }

  // On X11 GPUI hears only the keys sent to its own windows, so ⌥ pressed and let go alone while
  // the page has the keyboard is told from here, as GPUI tells the UI's (host_watch_option_taps).
  // Under Wayland GPUI hears the page's keys too.
  void WatchOptionTap(const ui::KeyEvent& event) {
    const bool option = event.key_code() == ui::VKEY_MENU;
    const int others = ui::EF_CONTROL_DOWN | ui::EF_SHIFT_DOWN | ui::EF_COMMAND_DOWN;
    if (event.type() == ui::EventType::kKeyPressed) {
      option_alone_ = option && !(event.flags() & others);
      return;
    }
    if (!option || !std::exchange(option_alone_, false) || !Host().window_option_tapped) {
      return;
    }
    for (Window* window : Window::All()) {
      if (window->widget() == widget_) {
        Host().window_option_tapped(Host().context, window->id());
      }
    }
  }

  // ui::EventHandler, before the window's views and pages get the event:
  void OnScrollEvent(ui::ScrollEvent* event) override { option_alone_ = false; }

  void OnKeyEvent(ui::KeyEvent* event) override {
    if (!ui_has_keyboard_ && !overlay_has_keyboard_ && ui_ != x11::Window::None) {
      WatchOptionTap(*event);
    }
    // The UI's or an overlay's, which on Wayland reach the window too.
    if (ui_has_keyboard_ || overlay_has_keyboard_) {
      event->StopPropagation();
    }
    // On X11 the X server sends the UI's keys here while the input focus is on the window, not
    // GPUI's, unless the pointer is over the UI: wlroots 0.17's Xwayland puts it back up from
    // GPUI's window each time it's taken down (#628), as a new window it focuses keeps it. They go
    // on to GPUI's window, where the focus would have sent them.
    if (ui_has_keyboard_ && !overlay_has_keyboard_ && ui_ != x11::Window::None) {
      std::optional<x11::KeyEvent> key = X11KeyOf(*event);
      if (x11::Window main = UIMain(); key && main != x11::Window::None) {
        key->event = main;
        x11::Connection* connection = x11::Connection::Get();
        // With the focus, or GPUI, finding it on this window, hands the key back here as the
        // page's, and the two pass it on without end, as ⌘Q after full screen did (#919).
        SetInputFocus(main);
        connection->SendEvent(*key, main, x11::EventMask::NoEvent);
        connection->Flush();
      }
    }
    // On X11 only one typed before the overlay that took the keyboard had the input focus reaches
    // the window, as fast typing after ⌃F does once the keys held for the bar were given to it, as
    // GPUI moves the focus on its next turn. They go to the overlay once it has the focus.
    if (overlay_has_keyboard_ && ui_ != x11::Window::None) {
      if (std::optional<x11::KeyEvent> key = X11KeyOf(*event)) {
        early_keys_.push_back(*key);
        SendEarlyKeys();
      }
    }
  }

  void OnMouseEvent(ui::MouseEvent* event) override {
    // ⌥ held for a click or a scroll isn't a tap.
    if (event->type() == ui::EventType::kMousePressed ||
        event->type() == ui::EventType::kMousewheel) {
      option_alone_ = false;
    }
    if (event->type() != ui::EventType::kMousePressed) {
      return;
    }
    // On X11 a press outside an overlay with the keyboard takes the input focus from it, which
    // tells GPUI the user left it. Under Wayland GPUI sees the press itself.
    if (overlay_has_keyboard_ && ui_ != x11::Window::None) {
      SetInputFocus(window());
    }
    // A click in a page that's focused already focuses nothing.
    if (InSlot(static_cast<aura::Window*>(event->target()))) {
      PageTakesKeyboard();
    }
  }

  // The window manager's state for the window changed. A full screen it ended itself, by its own
  // key, Chromium's X11 window ignores, keeping its own (X11Window::OnXWindowStateChanged), so the
  // window leaves here, the one way every exit takes (Window::LeaveFullScreen).
  void WindowManagerStateChanged() {
    std::vector<x11::Atom> states;
    x11::Connection::Get()->GetArrayProperty(window(), x11::GetAtom("_NET_WM_STATE"), &states);
    const bool full_screen =
        std::ranges::contains(states, x11::GetAtom("_NET_WM_STATE_FULLSCREEN"));
    // Only once the window manager has had it full screen: until it takes Chromium's request, the
    // window's state lacks it.
    const bool ended = window_manager_full_screen_ && !full_screen;
    window_manager_full_screen_ = full_screen;
    if (!ended || !widget_->IsFullscreen()) {
      return;
    }
    for (Window* each : Window::All()) {
      if (each->widget() == widget_) {
        // Its resize runs the app, which may close the window, and this with it.
        each->LeaveFullScreen();
        return;
      }
    }
  }

  // x11::EventObserver:
  void OnEvent(const x11::Event& event) override {
    if (auto* property = event.As<x11::PropertyNotifyEvent>()) {
      if (property->window == window() && property->atom == x11::GetAtom("_NET_WM_STATE")) {
        WindowManagerStateChanged();
      }
      return;
    }
    if (auto* focus = event.As<x11::FocusEvent>()) {
      if (focus->opcode == x11::FocusEvent::Out) {
        SendEarlyKeys();
      }
      // An overlay with the keyboard lent has the input focus, which leaves the page's untouched.
      if (focus->event != window() || focus->mode == x11::NotifyMode::Grab ||
          focus->mode == x11::NotifyMode::Ungrab || overlay_has_keyboard_) {
        return;
      }
      const bool in = focus->opcode == x11::FocusEvent::In;
      if (focus->detail != x11::NotifyDetail::Inferior) {
        retook_ = false;
      }
      // GPUI taking the input focus, told after a page took it back from that press, is over.
      const bool ui_took = IsWithin(InputFocus(), ui_);
      switch (focus->detail) {
        // Out: GPUI took the input focus into the UI's window, as it does on a click. In: it came
        // back up to the window's own, as a window manager focusing the window may move it, which
        // while the keys are the UI's would leave them with neither (#593). That's taken back once
        // until the focus next leaves or enters the window: wlroots 0.17's Xwayland puts it back
        // up each time, and the two would go on without end (#628).
        case x11::NotifyDetail::Inferior:
          if (!in) {
            if (ui_took) {
              UITakesKeyboard();
            }
          } else if (ui_has_keyboard_ && !retook_) {
            retook_ = true;
            UITakesKeyboard();
          }
          break;
        // GPUI took the input focus into the UI's window from another's.
        case x11::NotifyDetail::Virtual:
        case x11::NotifyDetail::NonlinearVirtual:
          if (in && ui_took) {
            UITakesKeyboard();
          }
          break;
        // The window has it back from another's, and the UI had the keyboard.
        case x11::NotifyDetail::Ancestor:
        case x11::NotifyDetail::Nonlinear:
          if (in && ui_has_keyboard_) {
            UITakesKeyboard();
          }
          break;
        default:
          break;
      }
      return;
    }
    // A child X window the core didn't put in the browser window, mapped there, is Chromium's
    // output for the window, opaque and over the UI until it's sent under it. The window's own map
    // comes with the same |event|, and sending it under would put a new window under the others
    // once the window manager has raised it (#712).
    auto* map = event.As<x11::MapNotifyEvent>();
    if (map && map->event == window() && map->window != window() && map->window != ui_) {
      x11::Connection::Get()->ConfigureWindow(
          {.window = map->window, .stack_mode = x11::StackMode::Below});
    }
  }

  // Whether |window| is a page or DevTools a slot holds, or in one.
  bool InSlot(aura::Window* window) const {
    auto holds = [window](const Slot& slot) {
      return slot.holder && slot.holder->Contains(window);
    };
    return std::ranges::any_of(panes_, holds) || holds(side_panel_) || holds(preview_);
  }

  // Sends the keys typed before the overlay with the keyboard had the input focus to the window
  // that has it, unless that's still this one or the UI's. A bar is a child of this one.
  void SendEarlyKeys() {
    if (early_keys_.empty()) {
      return;
    }
    const x11::Window focus = InputFocus();
    if (focus == x11::Window::None || focus == window() || IsWithin(focus, ui_)) {
      return;
    }
    x11::Connection* connection = x11::Connection::Get();
    for (x11::KeyEvent& key : early_keys_) {
      key.event = focus;
      connection->SendEvent(key, focus, x11::EventMask::NoEvent);
    }
    connection->Flush();
    fprintf(stderr, "browser: sent %zu key events typed before the overlay had the input focus\n",
            early_keys_.size());
    early_keys_.clear();
  }

 private:
  // The page a window of Chromium's took the keyboard from has it back, if it's still shown and the
  // window still has it.
  void PageTakesBackKeyboard() {
    if (page_lent_.windows().empty()) {
      return;
    }
    aura::Window* page = page_lent_.Pop();
    aura::client::FocusClient* client = aura::client::GetFocusClient(content());
    aura::Window* focused = client ? client->GetFocusedWindow() : nullptr;
    if (client && !ui_has_keyboard_ && (!focused || focused == content()) && InSlot(page)) {
      client->FocusWindow(page);
    }
  }

  // Sizes the UI to the window's content.
  void SizeUI() {
    const gfx::Size pixels = ContentPixelsOf(widget_);
    x11::Connection* connection = x11::Connection::Get();
    connection->ConfigureWindow(
        {.window = ui_, .x = 0, .y = 0, .width = pixels.width(), .height = pixels.height()});
    // GPUI's window, which it opened inside |ui_| on its own connection, follows on its
    // ConfigureNotify.
    if (x11::Window main = UIMain(); main != x11::Window::None) {
      connection->ConfigureWindow(
          {.window = main, .x = 0, .y = 0, .width = pixels.width(), .height = pixels.height()});
    }
  }

  // GPUI's window for the UI, the one it opened in |ui_|.
  x11::Window UIMain() {
    if (ui_main_ == x11::Window::None) {
      auto tree = x11::Connection::Get()->QueryTree({ui_}).Sync();
      if (tree && !tree->children.empty()) {
        ui_main_ = tree->children.front();
      }
    }
    return ui_main_;
  }

  raw_ptr<views::Widget> widget_;
  x11::Window ui_;
  std::vector<Slot> panes_;
  Slot side_panel_;
  // A link preview's page, over the panes, and the colour where it hasn't drawn: the one under the
  // window's page, as the app's card is, else a page's white, as the hole would show the pages
  // under it.
  Slot preview_;
  SkColor4f preview_color_ = SkColors::kWhite;
  // After the slots, so it leaves its holder before that goes.
  FocusRing ring_;
  double corner_radius_ = 0;
  // The pages stay where they are as the window resizes, until ShowPages (HoldPagesForResize).
  bool held_ = false;
  // Whether the window manager's state for the window last said full screen.
  bool window_manager_full_screen_ = false;
  bool ui_has_keyboard_ = false;
  bool overlay_has_keyboard_ = false;
  // The UI has the keyboard while no page shows, for the next page shown (KeyboardForPage).
  bool ui_holds_for_page_ = false;
  // A window of Chromium's own has the keyboard the UI or a page had, and gives it back as it goes.
  bool ui_lent_ = false;
  aura::WindowTracker page_lent_;
  // The keys typed while the overlay with the keyboard didn't have the input focus yet.
  std::vector<x11::KeyEvent> early_keys_;
  // Whether the core took the input focus back down to the UI's window since the focus last left or
  // entered the window.
  bool retook_ = false;
  // ⌥ is down with no other key or click since it went down, while the page has the keyboard.
  bool option_alone_ = false;
  x11::Window ui_main_ = x11::Window::None;
  // Off Wayland, which the app never tells of a field, the UI's own input method composes for it.
  UITextInput ui_text_input_;
  // The pages DrawUnseen holds.
  aura::WindowTracker unseen_;
  base::ScopedObservation<views::Widget, views::WidgetObserver> observation_{this};
  base::ScopedMultiSourceObservation<aura::Window, aura::WindowObserver> kept_{this};
  base::ScopedObservation<aura::client::FocusClient, aura::client::FocusChangeObserver>
      focus_observation_{this};
  x11::ScopedEventSelector children_events_;
  base::ScopedObservation<x11::Connection, x11::EventObserver> x11_observation_{this};
  base::WeakPtrFactory<WindowViews> weak_factory_{this};
};

std::map<views::Widget*, std::unique_ptr<WindowViews>>& AllWindowViews() {
  static base::NoDestructor<std::map<views::Widget*, std::unique_ptr<WindowViews>>> all;
  return *all;
}

WindowViews* ViewsOf(views::Widget* widget) {
  auto it = AllWindowViews().find(widget);
  return it == AllWindowViews().end() ? nullptr : it->second.get();
}

gfx::Insets InsetsOf(const gfx::Rect& frame, const gfx::Size& window) {
  return gfx::Insets::TLBR(frame.y(), frame.x(), window.height() - frame.bottom(),
                           window.width() - frame.right());
}

// Puts |view| in |parent| if it isn't there, and shows it unless |hidden|.
void Hold(aura::Window* parent, aura::Window* view, bool hidden) {
  if (view->parent() != parent) {
    if (view->parent()) {
      view->parent()->RemoveChild(view);
    }
    parent->AddChild(view);
  }
  if (hidden) {
    view->Hide();
  } else {
    view->Show();
  }
}

// Empties |holder| of all but |keep| and |keep_too|.
void DetachOthers(aura::Window* holder,
                  aura::Window* keep,
                  aura::Window* keep_too,
                  const Staying& staying) {
  // A copy, as Detach changes the list; its raw_ptrs convert one at a time.
  const aura::Window::Windows& children = holder->children();
  for (aura::Window* child : std::vector<aura::Window*>(children.begin(), children.end())) {
    if (child != keep && child != keep_too) {
      WindowViews::Detach(holder, child, staying);
    }
  }
}

}  // namespace

views::NativeWidget* NewNativeWidget(views::Widget* widget, base::RepeatingClosure) {
  // Framed by none of the window manager's (wayland.cc's SetUpWindow): the app's header draws the
  // window's buttons and moves it. The window manager reports a state it has already left full
  // screen for, as a show state change, so the page leaves with it and nothing runs early.
  return new views::DesktopNativeWidgetAura(widget);
}

void AddWindowViews(views::Widget* widget, void* ui_view) {
  // On X11 the app's UI is its X window's id; on Wayland it's a subsurface the app puts in place.
  const auto ui = OnWayland() ? x11::Window::None
                              : static_cast<x11::Window>(reinterpret_cast<uintptr_t>(ui_view));
  AllWindowViews()[widget] = std::make_unique<WindowViews>(widget, ui);
  ViewsOf(widget)->ShowThrough();
}

void ShowPages(views::Widget* widget,
               base::span<const Pane> panes,
               double corner_radius,
               size_t focused,
               uint32_t ring_color,
               std::optional<SidePanelPage> side_panel) {
  WindowViews* views = ViewsOf(widget);
  if (!views) {
    return;
  }
  views->set_corner_radius(corner_radius);
  views->set_held(false);
  const bool hidden = !widget->IsVisible();
  const gfx::Size size = ContentSizeOf(widget);

  Staying staying;
  for (const Pane& pane : panes) {
    staying.push_back(pane.page);
    if (pane.devtools) {
      staying.push_back(pane.devtools->view);
    }
  }
  if (side_panel) {
    staying.push_back(side_panel->page);
  }

  // A link preview's page kept as a tab shows in a pane now.
  Slot& preview = views->preview();
  if (preview.page && std::ranges::contains(staying, preview.page.get())) {
    views->Empty(preview, staying);
  }

  std::vector<Slot>& slots = views->panes();
  // Pages that stay in the same pane keep their slot.
  for (size_t i = panes.size(); i < slots.size(); ++i) {
    views->Empty(slots[i], staying);
  }
  if (slots.size() < panes.size()) {
    slots.resize(panes.size());
  }
  for (size_t i = 0; i < panes.size(); ++i) {
    const Pane& pane = panes[i];
    Slot& slot = slots[i];
    aura::Window* holder = views->HolderFor(slot);
    aura::Window* devtools = pane.devtools ? pane.devtools->view : nullptr;
    DetachOthers(holder, pane.page, devtools, staying);
    if (devtools) {
      Hold(holder, devtools, hidden);
      slot.page_in_devtools = pane.devtools->page_frame;
    }
    Hold(holder, pane.page, hidden);
    if (devtools) {
      if (pane.devtools->page_under) {
        holder->StackChildBelow(pane.page, devtools);
      } else {
        holder->StackChildAbove(pane.page, devtools);
      }
    }
    views->Keep(slot, pane.page, devtools);
    SkColor4f color = SkColors::kTransparent;
    if (pane.color) {
      color = SkColor4f::FromColor(
          SkColorSetRGB(*pane.color >> 16, *pane.color >> 8 & 0xff, *pane.color & 0xff));
      if (i == 0) {
        views->set_preview_color(color);
      }
    }
    holder->layer()->AsSolidColor()->SetColor(color);
    slot.insets = InsetsOf(pane.frame, size);
    hidden ? holder->Hide() : holder->Show();
  }
  if (panes.size() > 1 && focused < panes.size()) {
    views->ring().Show(views->HolderFor(slots[focused]), ring_color, corner_radius);
  } else {
    views->ring().Hide();
  }

  Slot& panel = views->side_panel();
  if (side_panel) {
    aura::Window* holder = views->HolderFor(panel);
    DetachOthers(holder, side_panel->page, nullptr, staying);
    Hold(holder, side_panel->page, hidden);
    views->Keep(panel, side_panel->page, nullptr);
    panel.insets = InsetsOf(side_panel->frame, size);
    panel.bottom_corners_only = true;
    hidden ? holder->Hide() : holder->Show();
  } else {
    views->Empty(panel, staying);
  }
  views->LayOut();
  views->ShowThrough();
  // With no page, the keys are the UI's, as on the Mac, where its view is all that's left.
  if (panes.empty()) {
    views->UITakesKeyboard();
  }
}

void HoldPagesForResize(views::Widget* widget) {
  if (WindowViews* views = ViewsOf(widget)) {
    views->set_held(true);
  }
}

bool PagesHeldForResize(views::Widget* widget) {
  WindowViews* views = ViewsOf(widget);
  return views && views->held();
}

void HidePages(views::Widget* widget) {
  WindowViews* views = ViewsOf(widget);
  if (!views) {
    return;
  }
  for (Slot& slot : views->panes()) {
    if (slot.holder) {
      slot.holder->Hide();
    }
  }
  if (views->side_panel().holder) {
    views->side_panel().holder->Hide();
  }
  views->ShowThrough();
}

base::ScopedClosureRunner DrawUnseen(views::Widget* widget, gfx::NativeView page) {
  WindowViews* views = ViewsOf(widget);
  if (!views || !page || page->GetRootWindow()) {
    return base::ScopedClosureRunner();
  }
  return views->DrawUnseen(page);
}

// The app's view for a link preview is an overlay of its own over the window, which can't hold a
// page, so the page shows in the window over its pages, where the overlay leaves a hole, and the
// card the app draws round that.
void ShowPageIn(views::Widget* widget,
                gfx::NativeView page,
                void*,
                const gfx::Rect& frame,
                double corner_radius) {
  WindowViews* views = widget ? ViewsOf(widget) : nullptr;
  if (views && page) {
    views->ShowPreview(page, frame, corner_radius);
  }
}

std::optional<gfx::Rect> FocusRingFrame(views::Widget* widget) {
  WindowViews* views = ViewsOf(widget);
  if (!views) {
    return std::nullopt;
  }
  for (Slot& slot : views->panes()) {
    if (slot.holder && views->ring().In(slot.holder.get())) {
      return slot.holder->bounds();
    }
  }
  return std::nullopt;
}

bool PageDialogsInOwnWindow() {
  // Chrome's default, which hasn't been looked at here.
  return false;
}

bool KeyboardForPage(views::Widget* widget) {
  WindowViews* views = ViewsOf(widget);
  // The UI's, unless it only holds it while no page shows.
  if (views && views->ui_has_keyboard()) {
    return views->ui_holds_for_page();
  }
  aura::client::FocusClient* client = aura::client::GetFocusClient(widget->GetNativeView());
  aura::Window* focused = client ? client->GetFocusedWindow() : nullptr;
  // The window itself has it as it opens, or once the view that had it went.
  if (!views || !focused || focused == widget->GetNativeView()) {
    return true;
  }
  for (Slot& slot : views->panes()) {
    if (slot.holder && slot.holder->Contains(focused)) {
      return !slot.devtools || !slot.devtools->Contains(focused);
    }
  }
  // A link preview's, or else the side panel or a window of Chromium's own, such as an
  // extension's popup.
  return views->preview().holder && views->preview().holder->Contains(focused);
}

void ReleaseKeyboard(views::Widget* widget) {
  if (WindowViews* views = ViewsOf(widget)) {
    views->PageTakesKeyboard();
  }
}

void KeepKeyboard(views::Widget* widget, base::OnceClosure change) {
  WindowViews* views = ViewsOf(widget);
  const bool ui = views && views->ui_has_keyboard();
  aura::WindowTracker focused;
  if (auto* client = aura::client::GetFocusClient(widget->GetNativeView())) {
    if (aura::Window* window = client->GetFocusedWindow()) {
      focused.Add(window);
    }
  }
  std::move(change).Run();
  if (ui) {
    if (!views->ui_has_keyboard()) {
      views->UITakesKeyboard();
    }
    return;
  }
  if (focused.windows().empty()) {
    return;
  }
  aura::Window* window = focused.windows().front();
  auto* client = aura::client::GetFocusClient(window);
  if (client && client->GetFocusedWindow() != window &&
      widget->GetNativeView()->Contains(window)) {
    client->FocusWindow(window);
  }
}

void StartMove(views::Widget* widget, std::optional<gfx::Point> press) {
  ui::PlatformWindow* platform_window = PlatformWindowOf(widget);
  ui::WmMoveResizeHandler* handler = ui::GetWmMoveResizeHandler(*platform_window);
  if (!handler) {
    return;
  }
  // The window manager on X11 starts the move from where the pointer is on the screen, in pixels.
  // A compositor takes it from the press, which Chromium heard too (its serial).
  gfx::Point pointer;
  if (!OnWayland()) {
    if (press) {
      pointer = platform_window->GetBoundsInPixels().origin() +
                gfx::ScaleToRoundedPoint(*press, ScaleOf(widget)).OffsetFromOrigin();
    } else if (auto reply = x11::Connection::Get()->QueryPointer({XWindowOf(widget)}).Sync()) {
      pointer = gfx::Point(reply->root_x, reply->root_y);
    }
  }
  handler->DispatchHostWindowDragMovement(HTCAPTION, pointer);
}

void UITakesKeyboard(views::Widget* widget, bool until_page) {
  if (WindowViews* views = ViewsOf(widget)) {
    views->UITakesKeyboard(until_page ? WindowViews::Asked::kToHold : WindowViews::Asked::kToKeep);
  }
}

bool KeyboardIsUI(views::Widget* widget) {
  WindowViews* views = ViewsOf(widget);
  return views && views->ui_has_keyboard();
}

bool KeyboardInChild(views::Widget* widget) {
  WindowViews* views = ViewsOf(widget);
  if (!views || views->ui_has_keyboard()) {
    return false;
  }
  aura::client::FocusClient* client = aura::client::GetFocusClient(widget->GetNativeView());
  aura::Window* focused = client ? client->GetFocusedWindow() : nullptr;
  return focused && focused != widget->GetNativeView() && !views->InSlot(focused);
}

std::optional<gfx::Rect> KeyboardPageFrame(views::Widget* widget) {
  WindowViews* views = ViewsOf(widget);
  if (!views || views->ui_has_keyboard()) {
    return std::nullopt;
  }
  aura::client::FocusClient* client = aura::client::GetFocusClient(widget->GetNativeView());
  aura::Window* focused = client ? client->GetFocusedWindow() : nullptr;
  if (!focused) {
    return std::nullopt;
  }
  // A pane's page, not the DevTools docked in it.
  for (Slot& slot : views->panes()) {
    if (slot.holder && slot.holder->Contains(focused) &&
        !(slot.devtools && slot.devtools->Contains(focused))) {
      return slot.holder->bounds();
    }
  }
  return std::nullopt;
}

std::u16string ChildTitle(views::Widget* widget) {
  if (!KeyboardInChild(widget)) {
    return {};
  }
  // The focused window is the child widget's own or one in it, as a dialog's button.
  aura::client::FocusClient* client = aura::client::GetFocusClient(widget->GetNativeView());
  for (aura::Window* window = client->GetFocusedWindow();
       window && window != widget->GetNativeView(); window = window->parent()) {
    views::Widget* child = views::Widget::GetWidgetForNativeView(window);
    if (child && child != widget && child->widget_delegate()) {
      return child->widget_delegate()->GetWindowTitle();
    }
  }
  return {};
}

void OverlayTakesKeyboard(views::Widget* widget, bool takes) {
  if (WindowViews* views = ViewsOf(widget)) {
    views->OverlayTakesKeyboard(takes);
  }
}

void SetUITextInput(views::Widget* widget, std::optional<gfx::Rect> caret) {
  if (WindowViews* views = ViewsOf(widget)) {
    views->ui_text_input().SetCaret(caret);
  }
}

std::optional<gfx::Rect> DialogFrame(views::Widget* widget) {
  // A tab's dialog is a widget the window owns, an aura window in it, which the X server and the
  // compositor don't see.
  for (views::Widget* dialog : views::Widget::GetAllOwnedWidgets(widget->GetNativeView())) {
    if (dialog->IsVisible()) {
      // Inside its border and shadow, which its window's bounds take in: what Chromium puts at the
      // page's top (UpdateWidgetModalDialogPosition), as a Mac's sheet's frame is.
      gfx::Rect frame = dialog->GetWindowBoundsInScreen();
      frame.Offset(-widget->GetClientAreaBoundsInScreen().OffsetFromOrigin());
      if (dialog->non_client_view() && dialog->non_client_view()->frame_view()) {
        frame.Inset(dialog->non_client_view()->frame_view()->GetInsets());
      }
      return frame;
    }
  }
  return std::nullopt;
}

void StretchOnResize(content::RenderWidgetHostView*, bool) {
  // shortcut: no stretch on Linux; the page shows its last frame unscaled until it redraws.
}

void AnswerClipboardRequests() {
  if (OnWayland()) {
    return;
  }
  x11::Connection* connection = x11::Connection::Get();
  connection->ReadResponses();
  // The rest wait their turn in the queue: dispatched inside the UI's read, they could call the UI.
  // Taken out first, as answering one can read more into the queue.
  std::vector<x11::Event> requests;
  for (x11::Event& event : connection->events()) {
    if (event.As<x11::SelectionRequestEvent>()) {
      requests.push_back(std::move(event));
      event = x11::Event();
    }
  }
  for (const x11::Event& request : requests) {
    connection->DispatchEvent(request);
  }
  connection->Flush();
}

void FloatPictureInPictureBubbles() {
  if (OnWayland()) {
    return;
  }
  // Chrome keeps a bubble on screen, and grows the Picture in Picture window to hold a child one
  // (PipChildDialogObserverHelper): dragged partly off the screen, the window would jump back on
  // it, taller, and stay so. A window of its own, the bubble needs no room in it.
  views::ViewsDelegate::GetInstance()->set_native_widget_factory(base::BindRepeating(
      [](const views::Widget::InitParams& params,
         views::internal::NativeWidgetDelegate* delegate) -> views::NativeWidget* {
        if (params.type != views::Widget::InitParams::TYPE_BUBBLE || !params.parent) {
          return nullptr;
        }
        content::WebContents* floating =
            PictureInPictureWindowManager::GetInstance()->GetChildWebContents();
        if (!floating ||
            views::Widget::GetTopLevelWidgetForNativeView(params.parent) !=
                views::Widget::GetTopLevelWidgetForNativeView(floating->GetNativeView())) {
          return nullptr;
        }
        // It only reads where the widget goes.
        return ::CreateNativeWidget(NativeWidgetType::kDesktopNativeWidgetAura,
                                    const_cast<views::Widget::InitParams*>(&params), delegate);
      }));
}

}  // namespace browser_core::platform
