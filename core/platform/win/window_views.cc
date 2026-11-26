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
#include "base/functional/callback_helpers.h"
#include "base/location.h"
#include "base/logging.h"
#include "base/memory/raw_ptr.h"
#include "base/memory/weak_ptr.h"
#include "base/no_destructor.h"
#include "base/numerics/safe_conversions.h"
#include "base/scoped_multi_source_observation.h"
#include "base/scoped_observation.h"
#include "base/task/single_thread_task_runner.h"
#include "browser_core/dialog_keys.h"
#include "browser_core/host.h"
#include "browser_core/page_edge_strips.h"
#include "browser_core/platform/focus_ring_aura.h"
#include "browser_core/session.h"
#include "browser_core/tab.h"
#include "browser_core/window.h"
#include "chrome/browser/lifetime/application_lifetime_desktop.h"
#include "chrome/installer/util/install_util.h"
#include "chrome/installer/util/shell_util.h"
#include "content/public/browser/web_contents.h"
#include "ui/aura/client/focus_change_observer.h"
#include "ui/aura/client/focus_client.h"
#include "ui/aura/client/window_types.h"
#include "ui/aura/env.h"
#include "ui/aura/env_observer.h"
#include "ui/aura/window.h"
#include "ui/aura/window_observer.h"
#include "ui/aura/window_tracker.h"
#include "ui/aura/window_tree_host.h"
#include "ui/base/ime/input_method.h"
#include "ui/base/win/hwnd_subclass.h"
#include "ui/base/win/shell.h"
#include "ui/display/win/screen_win.h"
#include "ui/gfx/geometry/insets.h"
#include "ui/gfx/geometry/point_f.h"
#include "ui/gfx/geometry/rect.h"
#include "ui/gfx/geometry/rect_conversions.h"
#include "ui/gfx/geometry/rect_f.h"
#include "ui/gfx/geometry/size_conversions.h"
#include "ui/gfx/geometry/transform.h"
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
  // A tab switch shows the pane it's already in: showing it again sends its window the style and
  // show messages for nothing, about 0.2 ms of the switch (#2377).
  if (holder->IsVisible()) {
    return;
  }
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
    // A click on a page whose tab shows a dialog focuses the pane here, as the page, ignoring input
    // under the dialog, won't take it, and every key the pane then has is lost: the dialog takes
    // the keyboard back, as on the Mac (#2625). Only if the pane still has the keyboard on the next
    // turn: activating the window focuses its page first, and F6 then gives the sidebar the
    // keyboard, which the dialog took back from it (#3443).
    if (gained_focus) {
      base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
          FROM_HERE,
          base::BindOnce(&PaneKeyboard::PageFocusedWithKeyboard, weak_factory_.GetWeakPtr()));
    }
    const bool take =
        gained_focus && GetFocus() != hwnd_ && GetActiveWindow() == GetAncestor(hwnd_, GA_ROOT);
    VLOG(1) << "pane " << hwnd_ << " focuses " << Describe(gained_focus)
            << (take ? ", taking the keyboard" : "");
    if (!take) {
      return;
    }
    {
      base::AutoReset<bool> taking(&taking_, true);
      SetFocus(hwnd_);
    }
    // Windows gives a minimized window's keyboard to the window itself, not a child, so a page
    // focused while its window is minimized and still the active one has aura's focus without the
    // keyboard. Focused again, aura changes nothing, so once the window was back neither a click in
    // the page nor the core gave its HWND the keyboard, and the keys went to the window, where no
    // shortcut is heard (#1464). The page lets go of aura's focus too, as it does losing the
    // keyboard, so the next focus takes it. Posted, as aura ignores a focus change made while it
    // tells of one.
    if (!HasKeyboard()) {
      VLOG(1) << "pane " << hwnd_ << " couldn't take the keyboard, which " << GetFocus() << " "
              << ClassOf(GetFocus()) << " has";
      base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
          FROM_HERE,
          base::BindOnce(&PaneKeyboard::LetGoWithoutKeyboard, weak_factory_.GetWeakPtr()));
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

  bool HasKeyboard() const {
    HWND focus = GetFocus();
    return focus && (focus == hwnd_ || IsChild(hwnd_, focus));
  }

  void PageFocusedWithKeyboard() {
    if (client_ && HasKeyboard()) {
      dialog_keys::PageViewFocused(client_->GetFocusedWindow());
    }
  }

  void LetGoWithoutKeyboard() {
    if (client_ && !HasKeyboard()) {
      VLOG(1) << "pane " << hwnd_ << " blurs its page, without the keyboard";
      client_->FocusWindow(nullptr);
    }
  }

  raw_ptr<views::Widget> widget_;
  HWND hwnd_;
  raw_ptr<aura::client::FocusClient> client_;
  base::RepeatingClosure took_keyboard_;
  bool taking_ = false;
  base::ScopedObservation<views::Widget, views::WidgetObserver> observation_{this};
  base::WeakPtrFactory<PaneKeyboard> weak_factory_{this};
};

// Alt pressed and let go alone tells the app (window_option_tapped), which shows the hidden sidebar
// as ⌥ does on the Mac: ilo has no menu bar on Windows, and its menus are the sidebar's, where
// Chrome's lone Alt goes to its menu button (#1847). As in Chrome, another key, a click or a scroll
// while it's down, as with Alt+Tab, Alt+F4 or Alt+D, or Ctrl or Shift held, as AltGr holds Ctrl,
// makes it no tap. The keys go to the UI's child HWND or a page's, so the window hears none: a hook
// on the UI thread's queue sees each as it's taken out, and runs only for a message.
namespace alt_taps {

// The browser window Alt went down in alone, until something else does.
HWND g_pressed_in = nullptr;
// The window whose Alt was let go alone in the message being handled, whose SC_KEYMENU, sent while
// that release is handled, goes no further (Tapped).
HWND g_tapped_in = nullptr;

bool Held(int key) {
  return (GetKeyState(key) & 0x8000) != 0;
}

// The pointer's presses and scrolls, in a window's client area or its frame.
bool PointerPressed(UINT message) {
  switch (message) {
    case WM_LBUTTONDOWN:
    case WM_LBUTTONDBLCLK:
    case WM_RBUTTONDOWN:
    case WM_RBUTTONDBLCLK:
    case WM_MBUTTONDOWN:
    case WM_MBUTTONDBLCLK:
    case WM_XBUTTONDOWN:
    case WM_XBUTTONDBLCLK:
    case WM_NCLBUTTONDOWN:
    case WM_NCRBUTTONDOWN:
    case WM_NCMBUTTONDOWN:
    case WM_NCXBUTTONDOWN:
    case WM_MOUSEWHEEL:
    case WM_MOUSEHWHEEL:
    case WM_POINTERDOWN:
      return true;
  }
  return false;
}

// Tells the app once the release has been handled, as Linux's core tells it from the event.
void Tap(HWND root) {
  if (!Host().window_option_tapped) {
    return;
  }
  for (Window* window : Window::All()) {
    if (window->widget() && views::HWNDForWidget(window->widget()) == root) {
      if (UserSettings().option_reveals_sidebar) {
        g_tapped_in = root;
      }
      base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
          FROM_HERE,
          base::BindOnce([](bc_id id) { Host().window_option_tapped(Host().context, id); },
                         window->id()));
    }
  }
}

void Take(const MSG& msg) {
  g_tapped_in = nullptr;
  const HWND root = msg.hwnd ? GetAncestor(msg.hwnd, GA_ROOT) : nullptr;
  switch (msg.message) {
    case WM_KEYDOWN:
    case WM_SYSKEYDOWN:
      // Its repeats while held change nothing.
      if (msg.wParam != VK_MENU) {
        g_pressed_in = nullptr;
      } else if (!(msg.lParam & (1 << 30))) {
        const bool chord = Held(VK_CONTROL) || Held(VK_SHIFT) || Held(VK_LWIN) || Held(VK_RWIN);
        g_pressed_in = chord ? nullptr : root;
      }
      break;
    case WM_KEYUP:
    case WM_SYSKEYUP:
      if (msg.wParam == VK_MENU && root && std::exchange(g_pressed_in, nullptr) == root) {
        Tap(root);
      }
      break;
    default:
      if (PointerPressed(msg.message)) {
        g_pressed_in = nullptr;
      }
  }
}

LRESULT CALLBACK Heard(int code, WPARAM removed, LPARAM msg) {
  if (code == HC_ACTION && removed == PM_REMOVE) {
    Take(*reinterpret_cast<const MSG*>(msg));
  }
  return CallNextHookEx(nullptr, code, removed, msg);
}

// Once, on the UI thread, for every browser window.
void Watch() {
  static HHOOK hook = SetWindowsHookExW(WH_GETMESSAGE, Heard, nullptr, GetCurrentThreadId());
  (void)hook;
}

// |root| went to the back: Alt let go after another window took the keyboard, as Alt+Tab's is
// when the user comes back, isn't a tap.
void Deactivated(HWND root) {
  if (g_pressed_in == root) {
    g_pressed_in = nullptr;
  }
}

// Whether |root|'s SC_KEYMENU is the tap's, which the app took, so Windows doesn't also give the
// keyboard to the window's menu, nor Chromium hand it on as an accelerator.
bool Tapped(HWND root) {
  return root && root == g_tapped_in;
}

}  // namespace alt_taps

// Which window a press activating a browser window went to. Windows tells each of the pressed
// window's parents with WM_PARENTNOTIFY first, then sends it WM_MOUSEACTIVATE, and a thread's
// WH_CALLWNDPROC hook sees both whichever window answers them: the window under a page's content
// is content's own, which answers WM_MOUSEACTIVATE without passing it on to the pane
// (LegacyRenderWidgetHostHWND::OnMouseActivate), and the UI's holder focuses the UI on hearing of
// its press (host.cc), which activates the window before WM_MOUSEACTIVATE comes. The window's own
// WM_PARENTNOTIFY, which names no child, is the last. The hook is set only while a browser window
// is in the back, as only a press bringing one forward is asked about.
namespace presses {

HHOOK g_hook = nullptr;
int g_waiting = 0;
HWND g_pressed = nullptr;

LRESULT CALLBACK Sent(int code, WPARAM own, LPARAM sent) {
  const auto* message = reinterpret_cast<const CWPSTRUCT*>(sent);
  if (code != HC_ACTION) {
    return CallNextHookEx(nullptr, code, own, sent);
  }
  const UINT told = LOWORD(message->wParam);
  if (message->message == WM_MOUSEACTIVATE ||
      (message->message == WM_PARENTNOTIFY &&
       (told == WM_LBUTTONDOWN || told == WM_RBUTTONDOWN || told == WM_MBUTTONDOWN ||
        told == WM_XBUTTONDOWN) &&
       GetAncestor(message->hwnd, GA_ROOT) != message->hwnd)) {
    g_pressed = message->hwnd;
  }
  return CallNextHookEx(nullptr, code, own, sent);
}

// |root|, a browser window, went to the back. A press in it the hook heard while another window
// was in the back is forgotten: it came while |root| was in front, so it brought nothing forward,
// and |root| coming back by the keyboard or the taskbar would read it as its own.
void Wait(HWND root) {
  if (g_pressed && GetAncestor(g_pressed, GA_ROOT) == root) {
    g_pressed = nullptr;
  }
  if (g_waiting++ == 0) {
    g_pressed = nullptr;
    g_hook = SetWindowsHookExW(WH_CALLWNDPROC, Sent, nullptr, GetCurrentThreadId());
  }
}

// A browser window that went to the back came forward, or went.
void StopWaiting() {
  if (--g_waiting == 0) {
    UnhookWindowsHookEx(std::exchange(g_hook, nullptr));
    g_pressed = nullptr;
  }
}

// The window in |root| the press activating it went to, once.
HWND Pressed(HWND root) {
  return g_pressed && GetAncestor(g_pressed, GA_ROOT) == root ? std::exchange(g_pressed, nullptr)
                                                              : nullptr;
}

}  // namespace presses

// What runs once the window in front changes, while an overlay that has a window's keyboard is
// (WhenOverlayLetsGo), and the hook that hears it, set only then.
std::vector<base::OnceClosure>& OverlayLetGo() {
  static base::NoDestructor<std::vector<base::OnceClosure>> let_go;
  return *let_go;
}
HWINEVENTHOOK g_front_changes = nullptr;

void CALLBACK FrontChanged(HWINEVENTHOOK, DWORD, HWND, LONG, LONG, DWORD, DWORD) {
  UnhookWinEvent(std::exchange(g_front_changes, nullptr));
  for (base::OnceClosure& each : std::exchange(OverlayLetGo(), {})) {
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(FROM_HERE, std::move(each));
  }
}

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

// How a link preview's page is drawn, for the motion of the app's card round it (DrawPageIn).
struct Drawn {
  float scale = 1;
  gfx::PointF about;
  float opacity = 1;
};

// The pages and DevTools a window shows once it changes, which move between its slots unhidden.
using Staying = std::vector<aura::Window*>;

gfx::Insets InsetsOf(const gfx::Rect& frame, const gfx::Size& window) {
  return gfx::Insets::TLBR(frame.y(), frame.x(), window.height() - frame.bottom(),
                           window.width() - frame.right());
}

// Hands the pointer in the strips along a browser window's sides (page_edge_strips.h), and in the
// top edge that resizes it (host.cc's FrameHit), from a page reaching them to the UI's holder under
// the pane, out of full screen: over a page, the UI would hear neither the hidden sidebar's edge
// nor the top edge. Answers for the pane's window and the windows content makes in it on this
// thread, as content's own answers its hit test itself (LegacyRenderWidgetHostHWND::OnNCHitTest),
// and Windows passes HTTRANSPARENT on to the window under it on the same thread.
class EdgeStrips : public ui::HWNDMessageFilter {
 public:
  EdgeStrips() = default;
  EdgeStrips(const EdgeStrips&) = delete;
  EdgeStrips& operator=(const EdgeStrips&) = delete;
  ~EdgeStrips() override { ui::HWNDSubclass::RemoveFilterFromAllTargets(this); }

  // Answers for |pane| and the windows in it on this thread, from now. A window's filters are a
  // set, so a window already answered for stays as it is.
  void Watch(HWND pane) {
    Add(pane);
    EnumChildWindows(pane, &EdgeStrips::Each, reinterpret_cast<LPARAM>(this));
  }

  // ui::HWNDMessageFilter:
  bool FilterMessage(HWND hwnd, UINT message, WPARAM, LPARAM l_param, LRESULT* result) override {
    if (message != WM_NCHITTEST) {
      return false;
    }
    HWND root = GetAncestor(hwnd, GA_ROOT);
    // Full screen takes the caption style away, as FrameHit reads it.
    if (!root || !(GetWindowLongPtrW(root, GWL_STYLE) & WS_CAPTION)) {
      return false;
    }
    POINT at{static_cast<short>(LOWORD(l_param)), static_cast<short>(HIWORD(l_param))};
    ScreenToClient(root, &at);
    RECT client{};
    GetClientRect(root, &client);
    const UINT dpi = GetDpiForWindow(root);
    const double strip = kPageEdgeStrip * dpi / static_cast<double>(USER_DEFAULT_SCREEN_DPI);
    // host.cc's TopEdge, kept in step with it.
    const int top =
        GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
    if (InPageEdgeStrip(at.x, client.right, strip) || (!IsZoomed(root) && at.y >= 0 && at.y < top)) {
      *result = HTTRANSPARENT;
      return true;
    }
    return false;
  }

 private:
  static BOOL CALLBACK Each(HWND child, LPARAM strips) {
    reinterpret_cast<EdgeStrips*>(strips)->Add(child);
    return TRUE;
  }

  void Add(HWND window) {
    if (GetWindowThreadProcessId(window, nullptr) == GetCurrentThreadId()) {
      ui::HWNDSubclass::AddFilterToTarget(window, this);
    }
  }
};

class WindowViews : public views::WidgetObserver,
                    public aura::WindowObserver,
                    public ui::HWNDMessageFilter {
 public:
  WindowViews(views::Widget* widget, HWND ui) : widget_(widget), ui_(ui) {
    observation_.Observe(widget);
    ui::HWNDSubclass::AddFilterToTarget(window(), this);
    alt_taps::Watch();
  }

  ~WindowViews() override {
    ui::HWNDSubclass::RemoveFilterFromAllTargets(this);
    if (std::exchange(in_back_, false)) {
      presses::StopWaiting();
    }
  }

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
      slot.keyboard = std::make_unique<PaneKeyboard>(
          slot.widget.get(),
          base::BindRepeating(&WindowViews::PaneTookKeyboard, base::Unretained(this),
                              slot.widget.get()));
    }
    return slot.widget.get();
  }

  // The page in |holder| took the keyboard, which ends the UI's hold on it, as on Linux. If its
  // tab has a dialog up, the dialog takes it back (dialog_keys.cc), as the page drops every key
  // then, ilo's shortcuts with them. The core hears it here: a page whose tab has a dialog ignores
  // input, and Aura's page view then says nothing of taking the focus
  // (RenderWidgetHostViewAura::OnWindowFocused), where the Mac's does. Chrome's page never takes
  // it, as aura keeps a click from a page under its tab's dialog (WindowModalityController), but a
  // pane is a host of its own, whose window the dialog doesn't belong to (#2855).
  void PaneTookKeyboard(views::Widget* holder) {
    set_ui_holds_for_page(false);
    // On the next turn, if the pane still has the keyboard, as PaneKeyboard's focus (#3443).
    base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
        FROM_HERE, base::BindOnce(&WindowViews::PaneKeptKeyboard, weak_factory_.GetWeakPtr(),
                                  holder->GetWeakPtr()));
  }

  void PaneKeptKeyboard(base::WeakPtr<views::Widget> holder) {
    const HWND pane = holder ? views::HWNDForWidget(holder.get()) : nullptr;
    const HWND focus = GetFocus();
    if (!pane || !focus || (focus != pane && !IsChild(pane, focus))) {
      return;
    }
    for (Window* window : Window::All()) {
      if (window->widget() != widget_) {
        continue;
      }
      content::WebContents* shown = Tab::FromId(window->selected());
      if (shown && holder->GetNativeView()->Contains(shown->GetNativeView())) {
        dialog_keys::PageFocused(shown);
      }
    }
  }

  // The HWND of the pane showing the window's selected page, or null while that page has no view
  // in a shown pane, as while it loads.
  HWND ShownPane() {
    for (Window* window : Window::All()) {
      if (window->widget() != widget_) {
        continue;
      }
      content::WebContents* shown = Tab::FromId(window->selected());
      if (!shown) {
        return nullptr;
      }
      for (Slot& slot : panes_) {
        if (slot.widget && slot.widget->GetNativeView()->Contains(shown->GetNativeView())) {
          HWND pane = views::HWNDForWidget(slot.widget.get());
          return IsWindowVisible(pane) ? pane : nullptr;
        }
      }
    }
    return nullptr;
  }

  // The window's child |hwnd| is or is in, the UI's or a pane's, or null if it's none of them.
  HWND ChildHolding(HWND hwnd) const {
    if (!hwnd || !IsChild(window(), hwnd)) {
      return nullptr;
    }
    while (GetParent(hwnd) != window()) {
      hwnd = GetParent(hwnd);
    }
    return hwnd;
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

  // The app's scale, from its points to the window's pixels: the DPI GPUI draws at, the last
  // WM_DPICHANGED's, or the window's before one, not its display's, which lagged as the window
  // crossed to a display of another scale (#1823). Windows' Text size isn't in it, as it isn't in
  // GPUI's (#3621).
  float UIScale() const {
    const UINT dpi = dpi_ ? dpi_ : GetDpiForWindow(window());
    return dpi ? dpi / static_cast<float>(USER_DEFAULT_SCREEN_DPI) : 1;
  }

  // A slot's scale, from the window's pixels to its pages' DIPs: its host's, which has Windows'
  // Text size in it, so a page keeps the user's text size as Chrome's do (#3621).
  static float PageScale(const Slot& slot) {
    return slot.widget->GetNativeView()->GetHost()->device_scale_factor();
  }

  // The window's content, in the app's points.
  gfx::Size ContentSize() const {
    RECT client;
    GetClientRect(window(), &client);
    return gfx::ScaleToFlooredSize(gfx::Size(client.right, client.bottom), 1 / UIScale());
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
    const float scale = UIScale();
    const gfx::Size size = ContentSize();
    for (Slot& slot : panes_) {
      LayOut(slot, size, scale);
    }
    LayOut(side_panel_, size, scale);
    // Last, so it's over the panes.
    LayOut(preview_, size, scale);
    ring_.Fit();
  }

  // |window_size| and the slot's insets are in the app's points, which |scale| makes pixels.
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
    // Here, as the page content made its window for is in the slot by now.
    edge_strips_.Watch(hwnd);
    // The pages fill the slot in its own DIPs, which Windows' Text size makes fewer than the app's
    // points: in those the page ran past the window by the text scale (#3621).
    const float to_page = scale / PageScale(slot);
    const gfx::Rect bounds =
        gfx::ScaleToEnclosingRectIgnoringError(gfx::Rect(pixels.size()), 1 / PageScale(slot));
    if (slot.devtools) {
      slot.devtools->SetBounds(bounds);
      gfx::Rect page = gfx::ScaleToEnclosingRectIgnoringError(slot.page_in_devtools, to_page);
      page.Intersect(bounds);
      slot.page->SetBounds(page);
    } else {
      slot.page->SetBounds(bounds);
    }
    double radius = slot.corner_radius.value_or(corner_radius_);
    gfx::Rect shown(pixels.size());
    if (&slot == &preview_) {
      shown = DrawnIn(frame, scale, to_page, shown);
      radius *= preview_drawn_.scale;
    }
    // shortcut: a window region's corners are aliased, unlike Core Animation's on the Mac; upgrade
    // to a DirectComposition clip if the edge shows.
    const int diameter = base::ClampRound(2 * radius * scale);
    HRGN region = shown.IsEmpty() ? CreateRectRgn(0, 0, 0, 0)
                                  : CreateRoundRectRgn(shown.x(), shown.y(), shown.right() + 1,
                                                       shown.bottom() + 1, diameter, diameter);
    if (slot.bottom_corners_only) {
      HRGN top = CreateRectRgn(0, 0, pixels.width(), pixels.height() / 2);
      CombineRgn(region, region, top, RGN_OR);
      DeleteObject(top);
    }
    // Only a region that changed: each tab switch lays the pane out again at the same size, and
    // setting an equal region still makes Windows send the pane its frame's messages and redraw it,
    // about 0.4 ms of each switch before its frame (#2377).
    HRGN current = CreateRectRgn(0, 0, 0, 0);
    const bool same = GetWindowRgn(hwnd, current) != ERROR && EqualRgn(current, region);
    DeleteObject(current);
    if (same) {
      DeleteObject(region);
    } else {
      // The window owns the region from here.
      SetWindowRgn(hwnd, region, TRUE);
    }
  }

  // Shows |page|, a link preview's, at |frame| over the pages shown, in place of the preview it
  // shows. The app's overlay over the window has a hole where it shows, and its card round that.
  void ShowPreview(aura::Window* page, const gfx::Rect& frame, double corner_radius) {
    views::Widget* holder = WidgetFor(preview_);
    aura::Window* parent = holder->GetNativeView();
    if (preview_.page != page) {
      Empty(preview_);
      preview_drawn_ = Drawn();
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

  // Draws the link preview's page at |scale| of its frame about |about|, in the window's content,
  // and at |opacity|, so it's never laid out again for it: its child window stays at its frame and
  // shows only where the page is drawn, and the page blends with the window's background there,
  // not with the page under it.
  void DrawPreview(aura::Window* page, float scale, const gfx::PointF& about, float opacity) {
    if (preview_.page != page) {
      return;
    }
    preview_drawn_ = {scale, about, opacity};
    LayOut(preview_, ContentSize(), UIScale());
  }

  // As the preview's slot is laid out at |frame|, |pixels| in its child window: its content's
  // transform and opacity, and where in the window it shows, in pixels. |frame| is in the app's
  // points, which |scale| makes pixels and |to_page| the content's DIPs.
  gfx::Rect DrawnIn(const gfx::Rect& frame, float scale, float to_page, const gfx::Rect& pixels) {
    const Drawn& drawn = preview_drawn_;
    aura::Window* content = preview_.widget->GetNativeView();
    const float in_x = drawn.about.x() - frame.x();
    const float in_y = drawn.about.y() - frame.y();
    gfx::Transform transform;
    if (drawn.scale != 1) {
      transform.Translate(in_x * to_page, in_y * to_page);
      transform.Scale(drawn.scale, drawn.scale);
      transform.Translate(-in_x * to_page, -in_y * to_page);
    }
    if (content->transform() != transform) {
      content->SetTransform(transform);
    }
    content->layer()->SetOpacity(drawn.opacity);
    if (drawn.opacity <= 0) {
      return gfx::Rect();
    }
    if (drawn.scale == 1) {
      return pixels;
    }
    const gfx::RectF shown(in_x * (1 - drawn.scale), in_y * (1 - drawn.scale),
                           frame.width() * drawn.scale, frame.height() * drawn.scale);
    return gfx::ToEnclosingRect(gfx::ScaleRect(shown, scale));
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
  // to take it from both (ReleaseKeyboard). And Alt let go alone goes no further once it's a tap.
  bool FilterMessage(HWND hwnd,
                     UINT message,
                     WPARAM w_param,
                     LPARAM l_param,
                     LRESULT* result) override {
    // Signing out or restarting, Windows ends the process once the windows have heard it. Chrome's
    // own windows end the session then, as SIGTERM does elsewhere, writing what must be kept, the
    // Spaces among it; these aren't Chrome's, so they do it here (#1990). It ends the process,
    // unless the session is ending already.
    if (message == WM_ENDSESSION && w_param) {
      chrome::SessionEnding();
      return false;
    }
    if (message == WM_SYSCOMMAND && (w_param & 0xFFF0) == SC_KEYMENU && l_param == 0 &&
        alt_taps::Tapped(hwnd)) {
      *result = 0;
      return true;
    }
    if (message == WM_ACTIVATE && LOWORD(w_param) == WA_INACTIVE) {
      alt_taps::Deactivated(hwnd);
      HWND focus = GetFocus();
      had_keyboard_ = focus && IsChild(hwnd, focus) ? focus : nullptr;
      pressed_ = nullptr;
      if (!std::exchange(in_back_, true)) {
        presses::Wait(hwnd);
      }
    } else if (message == WM_ACTIVATE) {
      // By a press, on the UI or a pane, or by the keyboard or the taskbar. Not always
      // WA_CLICKACTIVE: a press on the UI activates the window from inside its holder's SetFocus
      // (WM_PARENTNOTIFY), before Windows sends WM_MOUSEACTIVATE (#2910).
      pressed_ = ChildHolding(presses::Pressed(hwnd));
      if (std::exchange(in_back_, false)) {
        presses::StopWaiting();
      }
    } else if (message == WM_SETFOCUS) {
      HWND from = reinterpret_cast<HWND>(w_param);
      HWND had = std::exchange(had_keyboard_, nullptr);
      HWND pressed = std::exchange(pressed_, nullptr);
      // Not for a press elsewhere, on the UI or another pane, which takes the keyboard itself: a
      // tab's dialog in the pane took it from the pane then, from the UI the user pressed (#2910).
      // Coming back by the keyboard or the taskbar, or by a press on that pane, the pane takes it
      // back, and a dialog there takes it from the pane, wherever the pointer is.
      if (had && from != hwnd && !(from && IsChild(hwnd, from)) && IsChild(hwnd, had) &&
          IsWindowVisible(had) && !(pressed && pressed != ChildHolding(had))) {
        SetFocus(had);
      }
    } else if (message == WM_SIZE) {
      // Windows activates a minimized window before it restores it, as the second Win+D does, and
      // gives the keyboard to nothing then or as it restores it, so no WM_SETFOCUS comes and keys
      // reached no view (#2635). The window takes it back as it's restored.
      if (w_param != SIZE_MINIMIZED && !GetFocus() && GetActiveWindow() == hwnd) {
        HWND had = std::exchange(had_keyboard_, nullptr);
        SetFocus(had && IsChild(hwnd, had) && IsWindowVisible(had) ? had : hwnd);
      }
      SizeChanged(w_param);
    } else if (message == WM_DPICHANGED) {
      dpi_ = LOWORD(w_param);
      // The pages go again at the new scale once Chromium has taken it and the window's new rect,
      // as the window's size in DIPs may stay the same, which lays nothing out (#1823).
      base::SingleThreadTaskRunner::GetCurrentDefault()->PostTask(
          FROM_HERE, base::BindOnce(&WindowViews::ScaleChanged, weak_factory_.GetWeakPtr()));
    }
    return false;
  }

  void ScaleChanged() {
    if (held_) {
      SizeUI();
    } else {
      LayOut();
    }
  }

 private:
  // Minimizing hides only the window's own aura window, and no page is in it, since each pane is a
  // window of its own. So the pages went on counting as visible and drawing (#1582). The window's
  // observers now hear it go out of sight and come back, as Chromium tells them on the Mac and
  // Linux, and the core hides its pages and shows them again (Window::OnWidgetVisibilityChanged).
  void SizeChanged(WPARAM kind) {
    if (kind != SIZE_MINIMIZED && kind != SIZE_RESTORED && kind != SIZE_MAXIMIZED) {
      return;
    }
    const bool minimized = kind == SIZE_MINIMIZED;
    if (minimized != minimized_) {
      minimized_ = minimized;
      widget_->OnNativeWidgetVisibilityChanged(!minimized);
    }
  }

  raw_ptr<views::Widget> widget_;
  HWND ui_;
  std::vector<Slot> panes_;
  Slot side_panel_;
  // A link preview's page, over the panes.
  Slot preview_;
  // How its page is drawn for the card's motion (DrawPreview).
  Drawn preview_drawn_;
  EdgeStrips edge_strips_;
  // After the slots, so it leaves its holder before that goes.
  FocusRing ring_;
  double corner_radius_ = 0;
  // The pages stay where they are as the window resizes, until ShowPages (HoldPagesForResize).
  bool held_ = false;
  // The UI has the keyboard while no page shows, for the next page shown (KeyboardForPage).
  bool ui_holds_for_page_ = false;
  HWND had_keyboard_ = nullptr;
  // The UI or the pane a press activating the window was on, from WM_ACTIVATE until the window
  // takes the keyboard; null when the keyboard or the taskbar activated it.
  HWND pressed_ = nullptr;
  // Between going to the back and coming forward, while presses::Wait() counts it.
  bool in_back_ = false;
  // The DPI the last WM_DPICHANGED carried, 0 before one comes (UIScale).
  UINT dpi_ = 0;
  bool minimized_ = false;
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
  // A minimized window still counts as visible to its widget on Windows, so a page laid out while
  // it's minimized stays hidden too.
  const bool hidden = !widget->IsVisible() || IsIconic(views->window());
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
    // The ring draws in the pane's DIPs, the radius is in the app's points (LayOut).
    Slot& ringed = slots[focused];
    aura::Window* holder = views->WidgetFor(ringed)->GetNativeView();
    views->ring().Show(holder, ring_color,
                       corner_radius * views->UIScale() / WindowViews::PageScale(ringed));
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

// shortcut: each pane is an HWND of its own, which would have to shrink with its content scaled
// in it; until then the pages keep their size under a tab dragged over them.
void ScalePages(views::Widget*, double) {}

gfx::PointF LaidOutPagePoint(gfx::NativeView page, const gfx::PointF& point) {
  // In the page's DIPs, which Windows' Text size makes fewer than the app's points (#3621).
  aura::WindowTreeHost* host = page ? page->GetHost() : nullptr;
  const HWND root = host ? GetAncestor(host->GetAcceleratedWidget(), GA_ROOT) : nullptr;
  for (const auto& each : AllWindowViews()) {
    if (root && each.second->window() == root) {
      return gfx::ScalePoint(point, host->device_scale_factor() / each.second->UIScale());
    }
  }
  return point;
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
// The app shows the page through its hole the frame after placing it, as no run showed it late
// here (#2640), so nothing waits for it to be on screen.
void ShowPageIn(views::Widget* widget,
                gfx::NativeView page,
                void*,
                const gfx::Rect& frame,
                double corner_radius,
                base::OnceClosure) {
  WindowViews* views = widget ? ViewsOf(widget) : nullptr;
  if (views && page) {
    views->ShowPreview(page, frame, corner_radius);
  }
}

void DrawPageIn(views::Widget* widget,
                gfx::NativeView page,
                float scale,
                const gfx::PointF& about,
                float opacity) {
  WindowViews* views = widget ? ViewsOf(widget) : nullptr;
  if (views && page) {
    views->DrawPreview(page, scale, about, opacity);
  }
}

std::optional<std::pair<gfx::Rect, gfx::Rect>> DevToolsFrames(views::Widget* widget) {
  // Aura windows in the pane's one HWND, which Windows doesn't see: DevTools fill the pane, and the
  // page is at its place in them (LayOut), each from the pane's top-left.
  WindowViews* views = ViewsOf(widget);
  if (!views) {
    return std::nullopt;
  }
  for (Slot& slot : views->panes()) {
    if (slot.widget && slot.page && slot.devtools && slot.devtools->IsVisible()) {
      gfx::Rect frame(views->ContentSize());
      frame.Inset(slot.insets);
      const gfx::Vector2d at = frame.OffsetFromOrigin();
      // In the app's points, from the pane's DIPs (LayOut).
      const float to_app = WindowViews::PageScale(slot) / views->UIScale();
      return std::pair(gfx::ScaleToRoundedRect(slot.devtools->bounds(), to_app) + at,
                       gfx::ScaleToRoundedRect(slot.page->bounds(), to_app) + at);
    }
  }
  return std::nullopt;
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
  // In the app's points, from the window's DIPs, which Windows' Text size makes fewer (#3621).
  WindowViews* views = ViewsOf(widget);
  const float to_app =
      views ? widget->GetNativeView()->GetHost()->device_scale_factor() / views->UIScale() : 1;
  return gfx::ScaleToRoundedRect(frame, to_app);
}

bool FocusSettledOnActivation() {
  // Windows activates a window inside SetForegroundWindow, which the core can call part way
  // through a change, and it hasn't been measured there: as before, from a posted task.
  return false;
}

namespace {

// Answers |widget|'s hit test with HTTRANSPARENT, so the pointer goes to the window of the same
// thread under it, the browser window, as views' accept_events doesn't for a popup. Goes with the
// widget.
class PassThrough : public ui::HWNDMessageFilter, public views::WidgetObserver {
 public:
  explicit PassThrough(views::Widget* widget) {
    ui::HWNDSubclass::AddFilterToTarget(views::HWNDForWidget(widget), this);
    observation_.Observe(widget);
  }

  // ui::HWNDMessageFilter:
  bool FilterMessage(HWND, UINT message, WPARAM, LPARAM, LRESULT* result) override {
    if (message != WM_NCHITTEST) {
      return false;
    }
    *result = HTTRANSPARENT;
    return true;
  }

  // views::WidgetObserver:
  void OnWidgetDestroying(views::Widget*) override {
    ui::HWNDSubclass::RemoveFilterFromAllTargets(this);
    observation_.Reset();
    delete this;
  }

 private:
  base::ScopedObservation<views::Widget, views::WidgetObserver> observation_{this};
};

// Hears the origins a move loop proposes for a window it moves as an outline, which WM_MOVING gives
// it whether or not it shows the window's contents.
class OutlineMoves : public ui::HWNDMessageFilter {
 public:
  OutlineMoves(HWND window, base::RepeatingCallback<void(const gfx::Point&)> moved)
      : moved_(std::move(moved)) {
    ui::HWNDSubclass::AddFilterToTarget(window, this);
  }
  ~OutlineMoves() override { ui::HWNDSubclass::RemoveFilterFromAllTargets(this); }

  // ui::HWNDMessageFilter:
  bool FilterMessage(HWND window, UINT message, WPARAM, LPARAM l_param, LRESULT*) override {
    if (message == WM_MOVING) {
      // The window's rectangle in pixels, converted as views converts the window's bounds
      // (DesktopWindowTreeHostWin::GetWindowBoundsInScreen) once it's there: by the display
      // nearest it, where the window isn't yet.
      proposed_ = gfx::Rect(*reinterpret_cast<const RECT*>(l_param));
      moved_.Run(display::win::GetScreenWin()->ScreenToDIPRect(nullptr, *proposed_).origin());
    } else if (message == WM_EXITSIZEMOVE && proposed_) {
      // Where the window landed beside the outline's last place, in pixels and in the DIPs each
      // gives the move watch, which counts a move only where they differ. check.sh reads this line.
      RECT landed;
      ::GetWindowRect(window, &landed);
      display::win::ScreenWin* screen = display::win::GetScreenWin();
      LOG(ERROR) << "browser: outline move ended at " << proposed_->origin().ToString() << " ("
                 << screen->ScreenToDIPRect(nullptr, *proposed_).origin().ToString()
                 << " DIPs), the window at " << gfx::Rect(landed).origin().ToString() << " ("
                 << screen->ScreenToDIPRect(window, gfx::Rect(landed)).origin().ToString()
                 << " DIPs)";
    }
    return false;
  }

 private:
  base::RepeatingCallback<void(const gfx::Point&)> moved_;
  // The outline's last place, in pixels.
  std::optional<gfx::Rect> proposed_;
};

}  // namespace

base::ScopedClosureRunner WatchOutlineMoves(
    views::Widget* widget,
    base::RepeatingCallback<void(const gfx::Point&)> moved) {
  BOOL contents = TRUE;
  if (!SystemParametersInfo(SPI_GETDRAGFULLWINDOWS, 0, &contents, 0) || contents) {
    return base::ScopedClosureRunner();
  }
  return base::ScopedClosureRunner(base::DoNothingWithBoundArgs(
      std::make_unique<OutlineMoves>(views::HWNDForWidget(widget), std::move(moved))));
}

void PassPointerThrough(views::Widget* widget) {
  new PassThrough(widget);
}

std::optional<base::TimeTicks> PointerEventTime() {
  // A message's time is the tick count's, 10 to 16 ms a step, as coarse as a throw's moves.
  return std::nullopt;
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
    // Straight to the shown page's pane, so Windows tells of no focus on the window before the
    // page's own: NVDA takes 40–55 ms over the window's, and takes the page's two that come in
    // that time as one, the control's, which its browse mode then ignores (#3396). To the window
    // itself, which KeyboardForPage counts as the page's, while the page has no view shown yet or
    // the window isn't the active one, or is minimized, where Windows keeps it on the window. And
    // while the pane has it already, as its DevTools may: the pane gives it up and the page takes
    // it back, as before.
    const HWND window = views->window();
    const HWND pane =
        GetActiveWindow() == window && !IsIconic(window) ? views->ShownPane() : nullptr;
    const HWND focus = GetFocus();
    const bool pane_has = focus && (focus == pane || IsChild(pane, focus));
    SetFocus(pane && !pane_has ? pane : window);
  }
}

bool EndKeyboardLoans(views::Widget*) {
  return false;
}

bool OverlayHasKeyboard(views::Widget* widget) {
  if (!widget) {
    return false;
  }
  const HWND window = views::HWNDForWidget(widget);
  for (HWND owner = GetWindow(GetForegroundWindow(), GW_OWNER); owner;
       owner = GetWindow(owner, GW_OWNER)) {
    if (owner == window) {
      return true;
    }
  }
  return false;
}

void WhenOverlayLetsGo(views::Widget* widget, base::OnceClosure let_go) {
  OverlayLetGo().push_back(std::move(let_go));
  // Out of context, so it's heard on this thread, from its message loop.
  if (!g_front_changes) {
    g_front_changes = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, nullptr,
                                      &FrontChanged, 0, 0, WINEVENT_OUTOFCONTEXT);
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

bool CanTile() {
  // One page a window here: the app draws an agent's tab's picture instead.
  return false;
}

void ShowTile(content::WebContents*, void*, const gfx::Rect&, double) {}

void EndTile(content::WebContents*, bool) {}

void ParkTile(content::WebContents*) {}

void HideTile(content::WebContents*, bool) {}

void TilePageChanged(content::WebContents*) {}

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
