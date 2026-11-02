#include "browser_core/platform/window_views.h"

#include <windows.h>

#include <algorithm>
#include <map>
#include <memory>
#include <optional>
#include <set>
#include <string>
#include <utility>
#include <vector>

#include "base/auto_reset.h"
#include "base/functional/bind.h"
#include "base/functional/callback.h"
#include "base/logging.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/numerics/safe_conversions.h"
#include "base/scoped_multi_source_observation.h"
#include "base/scoped_observation.h"
#include "browser_core/platform/focus_ring_aura.h"
#include "chrome/installer/util/install_util.h"
#include "chrome/installer/util/shell_util.h"
#include "ui/aura/client/focus_change_observer.h"
#include "ui/aura/client/focus_client.h"
#include "ui/aura/client/window_types.h"
#include "ui/aura/env.h"
#include "ui/aura/env_observer.h"
#include "ui/aura/window.h"
#include "ui/aura/window_observer.h"
#include "ui/aura/window_tracker.h"
#include "ui/base/ime/input_method.h"
#include "ui/base/win/hwnd_subclass.h"
#include "ui/base/win/shell.h"
#include "ui/display/win/screen_win.h"
#include "ui/gfx/geometry/insets.h"
#include "ui/gfx/geometry/rect.h"
#include "ui/gfx/geometry/size_conversions.h"
#include "ui/views/focus/native_view_focus_manager.h"
#include "ui/views/widget/desktop_aura/desktop_native_widget_aura.h"
#include "ui/views/widget/native_widget_aura.h"
#include "ui/views/widget/widget.h"
#include "ui/views/widget/widget_observer.h"
#include "ui/views/win/hwnd_util.h"
#include "ui/views/window/non_client_view.h"

// Aura draws every view of a widget into its one HWND, so a page can't go over a native view in it
// the way an NSView goes over the app's on the Mac. Here the app's UI is a child HWND that fills the
// window, and each page shown sits in a child widget of its own, a child HWND above the UI's.

namespace browser_core::platform {

namespace {

// A native view in the logs below: its widget's name, and its own.
std::string Describe(aura::Window* window) {
  if (!window) {
    return "nothing";
  }
  views::Widget* widget = views::Widget::GetWidgetForNativeView(window);
  return (widget ? widget->GetName() : std::string("no widget")) + " / " + window->GetName();
}

// A window's class, in the logs below.
std::string ClassOf(HWND hwnd) {
  char name[64] = "";
  if (hwnd) {
    GetClassNameA(hwnd, name, sizeof(name));
  }
  return name;
}

// With --vmodule=window_views=1, what can close Chrome's dropdown under a page's field (#1037):
// Chrome hides it on any native focus that isn't its own, and as the widget it hangs from moves,
// which here is the page's pane's. It logs each popup's showing and hiding, and each native focus.
class PopupLog : public views::NativeViewFocusChangeListener,
                 public aura::EnvObserver,
                 public aura::WindowObserver {
 public:
  PopupLog() {
    views::NativeViewFocusManager::GetInstance()->AddFocusChangeListener(this);
    aura::Env::GetInstance()->AddObserver(this);
  }

  // views::NativeViewFocusChangeListener: what Chrome's dropdown hears.
  void OnNativeFocusChanged(gfx::NativeView focused_now) override {
    VLOG(1) << "native focus now " << Describe(focused_now);
  }

  // aura::EnvObserver:
  void OnWindowInitialized(aura::Window* window) override {
    if (window->GetType() == aura::client::WINDOW_TYPE_POPUP ||
        window->GetType() == aura::client::WINDOW_TYPE_MENU) {
      popups_.insert(window);
      window->AddObserver(this);
    }
  }

  // aura::WindowObserver: heard for a popup's own windows too, which aren't logged.
  void OnWindowVisibilityChanged(aura::Window* window, bool visible) override {
    if (popups_.contains(window)) {
      VLOG(1) << "popup " << Describe(window) << (visible ? " shown at " : " hidden at ")
              << window->GetBoundsInScreen().ToString();
    }
  }

  void OnWindowDestroying(aura::Window* window) override {
    if (popups_.erase(window)) {
      window->RemoveObserver(this);
    }
  }

 private:
  std::set<raw_ptr<aura::Window>> popups_;
};

// Shows |holder|, a slot's widget, without giving it the keyboard. Chromium focuses each window it
// shows that has no focus manager to take the first focus, as a child widget has none, even one
// shown inactive (HWNDMessageHandler::SetInitialFocus), and focusing it activates its window: that
// closed the command bar over it and brought a window restored behind to the front (#872).
void ShowInactive(views::Widget* holder) {
  HWND hwnd = views::HWNDForWidget(holder);
  const LONG_PTR style = GetWindowLongPtr(hwnd, GWL_EXSTYLE);
  SetWindowLongPtr(hwnd, GWL_EXSTYLE, style | WS_EX_NOACTIVATE);
  holder->ShowInactive();
  SetWindowLongPtr(hwnd, GWL_EXSTYLE, style);
}

// Gives a slot's HWND the keyboard while a page in it has aura's focus, and takes the page's focus
// as the HWND loses it. Windows sends keys and the input method's messages only to the focused
// HWND, and Chromium focuses a child widget's HWND, and tells its input method, only as a
// top-level window activates, which a child never does.
class PaneKeyboard : public aura::client::FocusChangeObserver,
                     public ui::HWNDMessageFilter,
                     public views::WidgetObserver {
 public:
  // |took_keyboard| runs each time the slot's HWND takes the keyboard.
  PaneKeyboard(views::Widget* widget, base::RepeatingClosure took_keyboard)
      : widget_(widget),
        hwnd_(views::HWNDForWidget(widget)),
        client_(aura::client::GetFocusClient(widget->GetNativeView())),
        took_keyboard_(std::move(took_keyboard)) {
    if (client_) {
      client_->AddObserver(this);
    }
    ui::HWNDSubclass::AddFilterToTarget(hwnd_, this);
    observation_.Observe(widget);
  }

  ~PaneKeyboard() override { Stop(); }

  // aura::client::FocusChangeObserver: heard before the page's view, so its input method finds the
  // HWND focused, which it needs to take the input method's text. Not while another of the app's
  // windows is in front, such as the command bar.
  void OnWindowFocused(aura::Window* gained_focus, aura::Window*) override {
    const bool take =
        gained_focus && GetFocus() != hwnd_ && GetActiveWindow() == GetAncestor(hwnd_, GA_ROOT);
    VLOG(1) << "pane " << hwnd_ << " focuses " << Describe(gained_focus)
            << (take ? ", taking the keyboard" : "");
    if (take) {
      base::AutoReset<bool> taking(&taking_, true);
      SetFocus(hwnd_);
    }
  }

  // ui::HWNDMessageFilter:
  bool FilterMessage(HWND, UINT message, WPARAM w_param, LPARAM, LRESULT*) override {
    ui::InputMethod* input_method = widget_->GetInputMethod();
    if (message == WM_SETFOCUS) {
      took_keyboard_.Run();
      if (input_method) {
        input_method->OnFocus();
      }
      // A page focused while another window was in front hears again, to take the input method.
      aura::Window* focused = client_ && !taking_ ? client_->GetFocusedWindow() : nullptr;
      VLOG(1) << "pane " << hwnd_ << " has the keyboard"
              << (focused ? ", focusing again " + Describe(focused) : std::string());
      if (focused) {
        aura::WindowTracker tracker;
        tracker.Add(focused);
        client_->FocusWindow(nullptr);
        if (tracker.Contains(focused)) {
          client_->FocusWindow(focused);
        }
      }
    } else if (message == WM_KILLFOCUS) {
      HWND to = reinterpret_cast<HWND>(w_param);
      if (to && IsChild(hwnd_, to)) {
        return false;
      }
      VLOG(1) << "pane " << hwnd_ << " loses the keyboard to " << to << " " << ClassOf(to)
              << ", blurring its page";
      if (input_method) {
        input_method->OnBlur();
      }
      // The page blurs with its HWND, as a page view does on the Mac as another takes the
      // keyboard, so focusing the page again reaches here and gives the HWND the keyboard back.
      if (client_) {
        client_->FocusWindow(nullptr);
      }
    }
    return false;
  }

  // views::WidgetObserver:
  void OnWidgetDestroying(views::Widget*) override { Stop(); }

  // What a dropdown hung from the pane hears, and closes for.
  void OnWidgetBoundsChanged(views::Widget*, const gfx::Rect& bounds) override {
    VLOG(1) << "pane " << hwnd_ << " bounds now " << bounds.ToString();
  }

 private:
  void Stop() {
    if (client_) {
      client_->RemoveObserver(this);
      client_ = nullptr;
    }
    ui::HWNDSubclass::RemoveFilterFromAllTargets(this);
    observation_.Reset();
  }

  raw_ptr<views::Widget> widget_;
  HWND hwnd_;
  raw_ptr<aura::client::FocusClient> client_;
  base::RepeatingClosure took_keyboard_;
  bool taking_ = false;
  base::ScopedObservation<views::Widget, views::WidgetObserver> observation_{this};
};

// A child widget, one HWND, holding one pane's page and its docked DevTools, or the side panel.
struct Slot {
  std::unique_ptr<views::Widget> widget;
  // After the widget, so it goes first.
  std::unique_ptr<PaneKeyboard> keyboard;
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

gfx::Insets InsetsOf(const gfx::Rect& frame, const gfx::Size& window) {
  return gfx::Insets::TLBR(frame.y(), frame.x(), window.height() - frame.bottom(),
                           window.width() - frame.right());
}

class WindowViews : public views::WidgetObserver,
                    public aura::WindowObserver,
                    public ui::HWNDMessageFilter {
 public:
  WindowViews(views::Widget* widget, HWND ui) : widget_(widget), ui_(ui) {
    observation_.Observe(widget);
    ui::HWNDSubclass::AddFilterToTarget(window(), this);
  }

  ~WindowViews() override { ui::HWNDSubclass::RemoveFilterFromAllTargets(this); }

  HWND window() const { return views::HWNDForWidget(widget_); }
  HWND ui() const { return ui_; }
  std::vector<Slot>& panes() { return panes_; }
  Slot& side_panel() { return side_panel_; }
  Slot& preview() { return preview_; }
  FocusRing& ring() { return ring_; }
  double corner_radius() const { return corner_radius_; }
  void set_corner_radius(double radius) { corner_radius_ = radius; }
  bool held() const { return held_; }
  void set_held(bool held) { held_ = held; }
  bool ui_holds_for_page() const { return ui_holds_for_page_; }
  void set_ui_holds_for_page(bool holds) { ui_holds_for_page_ = holds; }

  // A slot's widget, made the first time it's needed.
  views::Widget* WidgetFor(Slot& slot) {
    if (!slot.widget) {
      slot.widget = std::make_unique<views::Widget>();
      views::Widget::InitParams params(views::Widget::InitParams::CLIENT_OWNS_WIDGET,
                                       views::Widget::InitParams::TYPE_CONTROL);
      params.child = true;
      params.parent = widget_->GetNativeView();
      // Aura focuses nothing in a widget that can't activate, as a control's can't by default: a
      // click in the page, and the core's Focus of it, never gave it the keyboard (#872).
      params.activatable = views::Widget::InitParams::Activatable::kYes;
      params.native_widget = new views::DesktopNativeWidgetAura(slot.widget.get());
      params.name = "BrowserCorePane";
      slot.widget->Init(std::move(params));
      // Chrome finds the window a page is in by walking up its aura windows to a top-level widget,
      // and its dropdowns, autofill's among them, don't open without one (#1037). A pane is a
      // control in a host of its own, whose root no widget held, so the walk found nothing: the
      // root names the browser window's widget, as a page in Chrome's own window reaches it.
      views::NativeWidgetAura::RegisterNativeWidgetForWindow(
          widget_->native_widget_private(), slot.widget->GetNativeView()->GetRootWindow());
      // A page taking the keyboard ends the UI's hold on it, as on Linux.
      slot.keyboard = std::make_unique<PaneKeyboard>(
          slot.widget.get(), base::BindRepeating(&WindowViews::set_ui_holds_for_page,
                                                 base::Unretained(this), false));
    }
    return slot.widget.get();
  }

  // Takes |view| out of |holder| if it's there, so closing a slot never deletes a page, and hides
  // it unless it's |staying| in the window. A page out of every window that's never hidden still
  // counts as visible, so its tab would go on drawing and running its timers and never unload.
  // It's hidden before it leaves: its visibility is its occlusion state, which Aura's tracker
  // stops updating once the view is out of the slot's window, so one hidden after would stay
  // visible.
  static void Detach(aura::Window* holder, aura::Window* view, const Staying& staying) {
    if (holder && view && view->parent() == holder) {
      if (!std::ranges::contains(staying, view)) {
        view->Hide();
      }
      holder->RemoveChild(view);
    }
  }

  // Holds |page| and |devtools| in |slot|. A slot forgets one destroyed before ShowPages replaces
  // it, as a closed tab's page is when no other page shows after it, or the next resize would lay
  // out the freed page (#1340).
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
    aura::Window* holder = slot.widget ? slot.widget->GetNativeView() : nullptr;
    Detach(holder, slot.page, staying);
    Detach(holder, slot.devtools, staying);
    slot.page = nullptr;
    slot.devtools = nullptr;
    if (slot.widget) {
      slot.widget->Hide();
    }
  }

  // Holds |page|, which no window shows, hidden in the window under its slots until the runner
  // returned runs, so it has the window's compositor: a page out of every window has none, and a
  // copy of it waits for a frame until it's shown again (#1056), as on Linux. Hidden, it costs no
  // frames unless it's captured.
  base::ScopedClosureRunner DrawUnseen(aura::Window* page) {
    aura::Window* content = widget_->GetNativeView();
    page->Hide();
    content->AddChild(page);
    content->StackChildAtBottom(page);
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
    if (page->parent() == widget_->GetNativeView()) {
      widget_->GetNativeView()->RemoveChild(page);
    }
  }

  float Scale() const {
    return display::win::GetScreenWin()->GetScaleFactorForHWND(window());
  }

  // The window's content, in DIPs.
  gfx::Size ContentSize() const {
    RECT client;
    GetClientRect(window(), &client);
    return gfx::ScaleToFlooredSize(gfx::Size(client.right, client.bottom), 1 / Scale());
  }

  // Sizes the UI to the window's content.
  void SizeUI() {
    RECT client;
    GetClientRect(window(), &client);
    SetWindowPos(ui_, HWND_BOTTOM, 0, 0, client.right, client.bottom,
                 SWP_NOACTIVATE | SWP_NOOWNERZORDER);
  }

  // Sizes the UI to the window's content, and each slot to its insets, over the UI.
  void LayOut() {
    SizeUI();
    const float scale = Scale();
    const gfx::Size size = ContentSize();
    for (Slot& slot : panes_) {
      LayOut(slot, size, scale);
    }
    LayOut(side_panel_, size, scale);
    // Last, so it's over the panes.
    LayOut(preview_, size, scale);
    ring_.Fit();
  }

  void LayOut(Slot& slot, const gfx::Size& window_size, float scale) {
    if (!slot.widget || !slot.page) {
      return;
    }
    gfx::Rect frame(window_size);
    frame.Inset(slot.insets);
    const gfx::Rect pixels = gfx::ScaleToEnclosingRect(frame, scale);
    HWND hwnd = views::HWNDForWidget(slot.widget.get());
    SetWindowPos(hwnd, HWND_TOP, pixels.x(), pixels.y(), pixels.width(), pixels.height(),
                 SWP_NOACTIVATE | SWP_NOOWNERZORDER);
    const gfx::Rect bounds(frame.size());
    if (slot.devtools) {
      slot.devtools->SetBounds(bounds);
      gfx::Rect page = slot.page_in_devtools;
      page.Intersect(bounds);
      slot.page->SetBounds(page);
    } else {
      slot.page->SetBounds(bounds);
    }
    // shortcut: a window region's corners are aliased, unlike Core Animation's on the Mac; upgrade
    // to a DirectComposition clip if the edge shows.
    const int diameter =
        base::ClampRound(2 * slot.corner_radius.value_or(corner_radius_) * scale);
    HRGN region = CreateRoundRectRgn(0, 0, pixels.width() + 1, pixels.height() + 1, diameter,
                                     diameter);
    if (slot.bottom_corners_only) {
      HRGN top = CreateRectRgn(0, 0, pixels.width(), pixels.height() / 2);
      CombineRgn(region, region, top, RGN_OR);
      DeleteObject(top);
    }
    // The window owns the region from here.
    SetWindowRgn(hwnd, region, TRUE);
  }

  // Shows |page|, a link preview's, at |frame| over the pages shown, in place of the preview it
  // shows. The app's overlay over the window has a hole where it shows, and its card round that.
  void ShowPreview(aura::Window* page, const gfx::Rect& frame, double corner_radius) {
    views::Widget* holder = WidgetFor(preview_);
    aura::Window* parent = holder->GetNativeView();
    if (preview_.page != page) {
      Empty(preview_);
      if (page->parent()) {
        page->parent()->RemoveChild(page);
      }
      parent->AddChild(page);
      Keep(preview_, page, nullptr);
    }
    page->Show();
    preview_.corner_radius = corner_radius;
    preview_.insets = InsetsOf(frame, ContentSize());
    ShowInactive(holder);
    LayOut();
  }

  // aura::WindowObserver: a page or DevTools a slot holds is going.
  void OnWindowDestroying(aura::Window* window) override {
    kept_.RemoveObservation(window);
    const bool preview_went = preview_.page == window;
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
    // A link preview's page goes as its tab closes, and its slot with it.
    if (preview_went && preview_.widget) {
      preview_.widget->Hide();
    }
  }

  // views::WidgetObserver:
  void OnWidgetBoundsChanged(views::Widget*, const gfx::Rect&) override {
    if (held_) {
      SizeUI();
    } else {
      LayOut();
    }
  }

  void OnWidgetDestroying(views::Widget*) override {
    ring_.Hide();
    for (Slot& slot : panes_) {
      Empty(slot);
    }
    Empty(side_panel_);
    Empty(preview_);
    // A page held for its picture leaves too, or it would go with the window.
    while (!unseen_.windows().empty()) {
      Unsee(unseen_.windows().front());
    }
    panes_.clear();
    side_panel_ = Slot();
    preview_ = Slot();
    kept_.RemoveAllObservations();
    observation_.Reset();
    ui::HWNDSubclass::RemoveFilterFromAllTargets(this);
  }

  // ui::HWNDMessageFilter: Windows gives a window coming back to the front the keyboard itself, not
  // the child that had it, a page's or the UI's, so it goes back to that child, as AppKit gives it
  // back to the view that had it. Only from another window: the core gives the window the keyboard
  // to take it from both (ReleaseKeyboard).
  bool FilterMessage(HWND hwnd, UINT message, WPARAM w_param, LPARAM, LRESULT*) override {
    if (message == WM_ACTIVATE && LOWORD(w_param) == WA_INACTIVE) {
      HWND focus = GetFocus();
      had_keyboard_ = focus && IsChild(hwnd, focus) ? focus : nullptr;
    } else if (message == WM_SETFOCUS) {
      HWND from = reinterpret_cast<HWND>(w_param);
      HWND had = std::exchange(had_keyboard_, nullptr);
      if (had && from != hwnd && !(from && IsChild(hwnd, from)) && IsChild(hwnd, had) &&
          IsWindowVisible(had)) {
        SetFocus(had);
      }
    }
    return false;
  }

 private:
  raw_ptr<views::Widget> widget_;
  HWND ui_;
  std::vector<Slot> panes_;
  Slot side_panel_;
  // A link preview's page, over the panes.
  Slot preview_;
  // After the slots, so it leaves its holder before that goes.
  FocusRing ring_;
  double corner_radius_ = 0;
  // The pages stay where they are as the window resizes, until ShowPages (HoldPagesForResize).
  bool held_ = false;
  // The UI has the keyboard while no page shows, for the next page shown (KeyboardForPage).
  bool ui_holds_for_page_ = false;
  HWND had_keyboard_ = nullptr;
  // The pages DrawUnseen holds.
  aura::WindowTracker unseen_;
  // The pages and DevTools the slots have held, which Keep watches for their going.
  base::ScopedMultiSourceObservation<aura::Window, aura::WindowObserver> kept_{this};
  base::ScopedObservation<views::Widget, views::WidgetObserver> observation_{this};
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

}  // namespace

views::NativeWidget* NewNativeWidget(views::Widget* widget, base::RepeatingClosure) {
  // The app takes Windows' title bar away (host.cc's TakeTitleBar): its header draws the window's
  // buttons and moves it. Windows has no full screen of its own, so the window leaves it only when
  // Chrome or the app (bc_window_toggle_full_screen) asks, each through Window::LeaveFullScreen.
  return new views::DesktopNativeWidgetAura(widget);
}

void AddWindowViews(views::Widget* widget, void* ui_view) {
  HWND ui = static_cast<HWND>(ui_view);
  HWND window = views::HWNDForWidget(widget);
  // Chrome's toasts name the app by this ID, and the shortcut ilo.iss makes has it, so the windows
  // need it too to group with that shortcut pinned to the taskbar.
  ui::win::SetAppIdForWindow(ShellUtil::GetBrowserModelId(InstallUtil::IsPerUserInstall()), window);
  if (ui) {
    // The app's UI was a window of its own until now, made before the core's.
    SetWindowLongPtr(ui, GWL_STYLE,
                     (GetWindowLongPtr(ui, GWL_STYLE) & ~WS_POPUP) | WS_CHILD | WS_VISIBLE);
    SetParent(ui, window);
  }
  if (VLOG_IS_ON(1)) {
    static base::NoDestructor<PopupLog> popup_log;
  }
  auto views = std::make_unique<WindowViews>(widget, ui);
  WindowViews* raw = views.get();
  AllWindowViews()[widget] = std::move(views);
  if (ui) {
    raw->LayOut();
  }
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
  const gfx::Size size = views->ContentSize();

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
  // Pages that stay in the same pane keep their slot, and with it the keyboard.
  for (size_t i = panes.size(); i < slots.size(); ++i) {
    views->Empty(slots[i], staying);
  }
  if (slots.size() < panes.size()) {
    slots.resize(panes.size());
  }
  for (size_t i = 0; i < panes.size(); ++i) {
    const Pane& pane = panes[i];
    Slot& slot = slots[i];
    views::Widget* holder = views->WidgetFor(slot);
    aura::Window* parent = holder->GetNativeView();
    aura::Window* devtools = pane.devtools ? pane.devtools->view : nullptr;
    // A page or DevTools that left this pane leaves its slot.
    for (aura::Window* child : aura::Window::Windows(parent->children())) {
      if (child != pane.page && child != devtools) {
        WindowViews::Detach(parent, child, staying);
      }
    }
    if (devtools) {
      Hold(parent, devtools, hidden);
      slot.page_in_devtools = pane.devtools->page_frame;
    }
    const bool arrived = slot.page != pane.page;
    Hold(parent, pane.page, hidden);
    if (arrived && VLOG_IS_ON(1)) {
      // What Chrome's dropdown takes for the page's window, and the window it keeps within, once
      // the page is in its pane: before, it's in no window.
      views::Widget* top = views::Widget::GetTopLevelWidgetForNativeView(pane.page);
      views::Widget* root = top;
      while (root && root->parent()) {
        root = root->parent();
      }
      VLOG(1) << "pane " << i << " page " << Describe(pane.page) << ": top-level widget "
              << (top ? top->GetName() : std::string("none")) << ", in "
              << (root ? root->GetName() : std::string("none"));
    }
    if (devtools) {
      if (pane.devtools->page_under) {
        parent->StackChildBelow(pane.page, devtools);
      } else {
        parent->StackChildAbove(pane.page, devtools);
      }
    }
    views->Keep(slot, pane.page, devtools);
    slot.insets = InsetsOf(pane.frame, size);
    hidden ? holder->Hide() : ShowInactive(holder);
  }
  if (panes.size() > 1 && focused < panes.size()) {
    views->ring().Show(views->WidgetFor(slots[focused])->GetNativeView(), ring_color,
                       corner_radius);
  } else {
    views->ring().Hide();
  }

  Slot& panel = views->side_panel();
  if (side_panel) {
    views::Widget* holder = views->WidgetFor(panel);
    aura::Window* parent = holder->GetNativeView();
    for (aura::Window* child : aura::Window::Windows(parent->children())) {
      if (child != side_panel->page) {
        WindowViews::Detach(parent, child, staying);
      }
    }
    Hold(parent, side_panel->page, hidden);
    views->Keep(panel, side_panel->page, nullptr);
    panel.insets = InsetsOf(side_panel->frame, size);
    panel.bottom_corners_only = true;
    hidden ? holder->Hide() : ShowInactive(holder);
  } else {
    views->Empty(panel, staying);
  }
  views->LayOut();
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
    if (slot.widget) {
      slot.widget->Hide();
    }
  }
  if (views->side_panel().widget) {
    views->side_panel().widget->Hide();
  }
}

// The app's view for a link preview is an overlay of its own over the window, which can't hold a
// page, as on Linux, so the page shows in the window over its pages, where the overlay leaves a
// hole, and the card the app draws round that.
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
    if (slot.widget && views->ring().In(slot.widget->GetNativeView())) {
      gfx::Rect frame(views->ContentSize());
      frame.Inset(slot.insets);
      return frame;
    }
  }
  return std::nullopt;
}

views::Widget* PageDialog(views::Widget* widget) {
  // A tab's dialog is a window of its own that the browser window owns (PageDialogsInOwnWindow),
  // as the panes' widgets aren't, and the only one that's modal.
  for (views::Widget* dialog : views::Widget::GetAllOwnedWidgets(widget->GetNativeView())) {
    if (dialog->IsVisible() && dialog->IsModal()) {
      return dialog;
    }
  }
  return nullptr;
}

std::optional<gfx::Rect> DialogFrame(views::Widget* widget) {
  views::Widget* dialog = PageDialog(widget);
  if (!dialog) {
    return std::nullopt;
  }
  // Inside the border and shadow views draws in its window: what Chromium puts at the page's top
  // (UpdateWidgetModalDialogPosition), as a Mac's sheet's frame is.
  gfx::Rect frame = dialog->GetWindowBoundsInScreen();
  frame.Offset(-widget->GetClientAreaBoundsInScreen().OffsetFromOrigin());
  if (dialog->non_client_view() && dialog->non_client_view()->frame_view()) {
    frame.Inset(dialog->non_client_view()->frame_view()->GetInsets());
  }
  return frame;
}

bool PageDialogsInOwnWindow() {
  // Aura would draw it in the window's HWND, under the UI's and the pages' child HWNDs.
  return true;
}

bool KeyboardForPage(views::Widget* widget) {
  WindowViews* views = ViewsOf(widget);
  HWND focus = GetFocus();
  // The window itself has it as it opens, or once the view that had it went.
  if (!views || !focus || focus == views->window()) {
    return true;
  }
  // The UI's, unless it only holds it while no page shows.
  if (focus == views->ui() || IsChild(views->ui(), focus)) {
    return views->ui_holds_for_page();
  }
  for (Slot& slot : views->panes()) {
    if (!slot.widget || focus != views::HWNDForWidget(slot.widget.get())) {
      continue;
    }
    aura::client::FocusClient* client =
        aura::client::GetFocusClient(slot.widget->GetNativeView());
    aura::Window* focused = client ? client->GetFocusedWindow() : nullptr;
    return !slot.devtools || !focused || !slot.devtools->Contains(focused);
  }
  // A link preview's, or else the side panel or another window's.
  return views->preview().widget && focus == views::HWNDForWidget(views->preview().widget.get());
}

void ReleaseKeyboard(views::Widget* widget) {
  if (WindowViews* views = ViewsOf(widget)) {
    views->set_ui_holds_for_page(false);
    SetFocus(views->window());
  }
}

void UITakesKeyboard(views::Widget* widget, bool until_page) {
  WindowViews* views = ViewsOf(widget);
  if (!views) {
    return;
  }
  // Heard before the app gives the UI the keyboard (focus_ui), so the focus says whether the UI had
  // it. Asked to keep it, as for the New Space form's field, it keeps it. Asked to hold it, as the
  // app does as it shows a Space with no page, it holds it for the next page shown, unless it
  // keeps it already, as the form's Space shows (#895). The page that had it is hidden but keeps
  // Windows' focus, where no shortcut reaches the UI.
  HWND focus = GetFocus();
  const bool ui_had = focus && (focus == views->ui() || IsChild(views->ui(), focus));
  views->set_ui_holds_for_page(until_page && (views->ui_holds_for_page() || !ui_had));
}

void KeepKeyboard(views::Widget* widget, base::OnceClosure change) {
  WindowViews* views = ViewsOf(widget);
  HWND had = GetFocus();
  aura::WindowTracker focused;
  if (views) {
    for (Slot& slot : views->panes()) {
      if (slot.widget && had == views::HWNDForWidget(slot.widget.get())) {
        if (auto* client = aura::client::GetFocusClient(slot.widget->GetNativeView())) {
          if (aura::Window* window = client->GetFocusedWindow()) {
            focused.Add(window);
          }
        }
      }
    }
  }
  std::move(change).Run();
  if (!views || GetFocus() == had || !IsWindow(had) ||
      (had != views->window() && !IsChild(views->window(), had))) {
    return;
  }
  SetFocus(had);
  if (!focused.windows().empty()) {
    aura::Window* window = focused.windows().front();
    if (auto* client = aura::client::GetFocusClient(window)) {
      client->FocusWindow(window);
    }
  }
}

void StretchOnResize(content::RenderWidgetHostView*, bool) {
  // shortcut: no stretch on Windows; the page shows its last frame unscaled until it redraws.
}

base::ScopedClosureRunner DrawUnseen(views::Widget* widget, gfx::NativeView page) {
  WindowViews* views = ViewsOf(widget);
  if (!views || !page || page->GetRootWindow()) {
    return base::ScopedClosureRunner();
  }
  return views->DrawUnseen(page);
}

}  // namespace browser_core::platform
