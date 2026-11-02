// The app's Windows glue: platform/host.h, as platform/mac/host.mm is on the Mac. The core puts the
// UI's holder window in the browser window as a child under the pages
// (core/platform/win/window_views.cc). What floats over a page, such as the command bar, is a GPUI
// window of its own that the browser window owns, which this places over it.

#include <windows.h>
#include <windowsx.h>

#include <commctrl.h>
#include <commdlg.h>
#include <dwmapi.h>
#include <shellapi.h>
#include <shlobj.h>
#include <shlwapi.h>
#include <shobjidl.h>
#include <wincodec.h>
#include <wrl/client.h>

#include <winrt/Windows.ApplicationModel.DataTransfer.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Storage.Streams.h>

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <ctime>
#include <functional>
#include <iterator>
#include <map>
#include <optional>
#include <string>
#include <thread>
#include <utility>
#include <vector>

#include "../host.h"

// UrlCreateFromPathW, for the files dragged onto the UI.
#pragma comment(lib, "shlwapi.lib")

// The app's (platform/windows/fade.rs): fades GPUI's window |hwnd| from |from|, or from where it
// shows now if that's below 0, to |to| over |seconds|.
extern "C" void browser_fade(void* hwnd, double from, double to, double seconds);
// The preview has no fade.rs, so there the linker takes this instead, as Linux's weak symbols do:
// the window shows unfaded, and the host still hides it once its fade would have ended.
extern "C" void browser_fade_none(void*, double, double, double) {}
#pragma comment(linker, "/alternatename:browser_fade=browser_fade_none")

namespace {

std::wstring Wide(const char* utf8) {
  if (!utf8) {
    return {};
  }
  const int length = MultiByteToWideChar(CP_UTF8, 0, utf8, -1, nullptr, 0);
  std::wstring wide(length > 0 ? length - 1 : 0, L'\0');
  if (length > 1) {
    MultiByteToWideChar(CP_UTF8, 0, utf8, -1, wide.data(), length);
  }
  return wide;
}

std::string Utf8(const std::wstring& wide) {
  const int length =
      WideCharToMultiByte(CP_UTF8, 0, wide.c_str(), -1, nullptr, 0, nullptr, nullptr);
  std::string utf8(length > 0 ? length - 1 : 0, '\0');
  if (length > 1) {
    WideCharToMultiByte(CP_UTF8, 0, wide.c_str(), -1, utf8.data(), length, nullptr, nullptr);
  }
  return utf8;
}

HWND Hwnd(void* view) {
  return static_cast<HWND>(view);
}

// The browser window a UI view is in, once the core has put it there.
HWND Root(void* ui_view) {
  return ui_view ? GetAncestor(Hwnd(ui_view), GA_ROOT) : nullptr;
}

double Scale(HWND hwnd) {
  const UINT dpi = hwnd ? GetDpiForWindow(hwnd) : GetDpiForSystem();
  return (dpi ? dpi : 96) / 96.0;
}

// The browser window's content, in points.
void ContentSize(HWND root, double* width, double* height) {
  RECT client{};
  GetClientRect(root, &client);
  const double scale = Scale(root);
  *width = client.right / scale;
  *height = client.bottom / scale;
}

// A window's size in points.
void WindowSize(HWND hwnd, double* width, double* height) {
  RECT rect{};
  GetWindowRect(hwnd, &rect);
  const double scale = Scale(hwnd);
  *width = (rect.right - rect.left) / scale;
  *height = (rect.bottom - rect.top) / scale;
}

// Runs |task| on the main thread after the event being handled, as AppKit's "after the event" does.
UINT const kRunTask = WM_APP + 1;

LRESULT CALLBACK TaskProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) {
  if (message == kRunTask) {
    auto* task = reinterpret_cast<std::function<void()>*>(lparam);
    (*task)();
    delete task;
    return 0;
  }
  return DefWindowProcW(hwnd, message, wparam, lparam);
}

void Later(std::function<void()> task) {
  static HWND tasks = [] {
    WNDCLASSW window_class{};
    window_class.lpfnWndProc = TaskProc;
    window_class.hInstance = GetModuleHandleW(nullptr);
    window_class.lpszClassName = L"BrowserHostTasks";
    RegisterClassW(&window_class);
    return CreateWindowExW(0, window_class.lpszClassName, L"", 0, 0, 0, 0, 0, HWND_MESSAGE,
                           nullptr, window_class.hInstance, nullptr);
  }();
  PostMessageW(tasks, kRunTask, 0, reinterpret_cast<LPARAM>(new std::function<void()>(task)));
}

// The tasks After runs, by their timers.
std::map<UINT_PTR, std::function<void()>>& Timed() {
  static std::map<UINT_PTR, std::function<void()>> all;
  return all;
}

void CALLBACK TimedOut(HWND, UINT, UINT_PTR id, DWORD) {
  KillTimer(nullptr, id);
  auto timed = Timed().extract(id);
  if (!timed.empty()) {
    timed.mapped()();
  }
}

// Runs |task| once |seconds| have gone by, as a fade ends.
void After(double seconds, std::function<void()> task) {
  const UINT ms = static_cast<UINT>(std::lround(seconds * 1000));
  const UINT_PTR id = SetTimer(nullptr, 0, ms, TimedOut);
  if (id) {
    Timed()[id] = std::move(task);
  }
}

// A GPUI window floating over a browser window: owned by it, so it stays in front of it and goes
// with it when it's minimized, and placed in points from its content's top-left.
struct Floating {
  HWND hwnd = nullptr;
  // Clicks go through it to the browser window under it.
  bool mouse_through = false;
  // Where it takes clicks, in pixels from its top-left, if not all of it: the hover card's plate,
  // whose margin, where its shadow draws, lets them through as on the Mac and Linux.
  std::optional<RECT> plate;
  // Called after the event, when the user leaves it for a window that isn't its own.
  void (*dismissed)(void) = nullptr;
  // Set while the host hides it, so that isn't the user leaving it.
  bool hiding = false;
  // Counts its fades, so one that ends after it showed or hid again leaves it be.
  int fades = 0;
  // Counts the overlays' shows and hides, so a show still to come after a hide leaves it hidden.
  int shows = 0;
};

bool HideOverlayOf(HWND hwnd);

std::map<HWND, Floating*>& FloatingByHwnd() {
  static std::map<HWND, Floating*> all;
  return all;
}

// ilo's own context menu (app/src/menu.rs): a GPUI window over a browser window, or over another
// of the app's windows for its menus. While it shows it sees every pointer event in the app first,
// through a thread mouse hook, so a press outside closes it before the click lands; it takes the
// keyboard, as the command bar does, so its keys reach it (menu_window.mm does the same on the
// Mac). The window lives in the one struct, made once. The Mac also closes it when its browser
// window moves or resizes; here acting on that window's frame is itself a press outside, which the
// pointer hook already closes it on, so there's no separate watch.
struct Menu {
  HWND hwnd = nullptr;
  // Every pointer event while it shows, as |kind| (0 moved, 1 pressed, 2 right-pressed, 3 released,
  // 4 scrolled) at |x|, |y| points from its top left; returns whether the menu takes it.
  bool (*pointer)(int, double, double) = nullptr;
  // The user left it, for another window or app, or its browser window changed under it.
  void (*dismissed)(void) = nullptr;
  // Chooses an item by title, for the driver.
  bool (*choose)(const char*) = nullptr;
  bool shown = false;
  // Where it sits on screen, device pixels, which the pointer is measured from.
  RECT frame{};
  // The browser window it's over, and the window it's owned by: that one, or its reveal's panel.
  HWND browser = nullptr;
  HWND parent = nullptr;
  // The window that had the keyboard, to give back unless the user went to another.
  HWND had_keyboard = nullptr;
  bool give_back = true;
  // Bumped by each show and hide, so a show posted before a hide doesn't put the menu back.
  unsigned showing = 0;
};
Menu g_menu;
// The thread hooks that watch the pointer and the keys while the menu shows.
HHOOK g_menu_mouse_hook = nullptr;
HHOOK g_menu_key_hook = nullptr;

LRESULT CALLBACK FloatingProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam,
                              UINT_PTR, DWORD_PTR data) {
  auto* floating = reinterpret_cast<Floating*>(data);
  switch (message) {
    case WM_NCCALCSIZE:
      // All of it is the UI's. GPUI takes only the caption away and leaves Windows' border on
      // the other sides, which put what it drew 8 pixels in and cut its right and bottom edges.
      if (wparam) {
        return 0;
      }
      break;
    case WM_WINDOWPOSCHANGING: {
      // Windows keeps a window with a caption, as GPUI's are, at least 39 pixels high, which the
      // find bar's 32 is under, and a smaller least size in WM_GETMINMAXINFO doesn't lower it. An
      // overlay is the size the UI gives it.
      auto* pos = reinterpret_cast<WINDOWPOS*>(lparam);
      const int width = pos->cx, height = pos->cy;
      const LRESULT result = DefSubclassProc(hwnd, message, wparam, lparam);
      if (!(pos->flags & SWP_NOSIZE)) {
        pos->cx = width;
        pos->cy = height;
      }
      return result;
    }
    case WM_NCHITTEST:
      // To the browser window under it, which is on this thread.
      if (floating->mouse_through) {
        return HTTRANSPARENT;
      }
      if (floating->plate) {
        POINT at{GET_X_LPARAM(lparam), GET_Y_LPARAM(lparam)};
        ScreenToClient(hwnd, &at);
        if (!PtInRect(&*floating->plate, at)) {
          return HTTRANSPARENT;
        }
      }
      break;
    case WM_MOUSEACTIVATE:
      if (floating->mouse_through) {
        return MA_NOACTIVATE;
      }
      break;
    case WM_ACTIVATE:
      if (LOWORD(wparam) == WA_INACTIVE && floating->dismissed && !floating->hiding &&
          IsWindowVisible(hwnd)) {
        // ilo's menu, opened over this overlay, isn't the user leaving it.
        if (g_menu.shown && reinterpret_cast<HWND>(lparam) == g_menu.hwnd) {
          break;
        }
        // After the press that left it, as Windows has taken it by now: an overlay goes then, as
        // the Mac's and Linux's hosts close one, and the UI hears so after the press (#872).
        void (*dismissed)(void) = floating->dismissed;
        Later([dismissed, hwnd] {
          HideOverlayOf(hwnd);
          dismissed();
        });
      }
      break;
  }
  return DefSubclassProc(hwnd, message, wparam, lparam);
}

// Windows 11 outlines a window it rounds, and the UI draws a floating panel's own rim, so with
// both the panel had two, and a window larger than its panel an outline around nothing.
void NoBorder(HWND hwnd) {
  COLORREF none = DWMWA_COLOR_NONE;
  DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, &none, sizeof none);
}

// GPUI makes its windows the taskbar's, and an overlay would put a button there each time it
// showed: one its browser window owns has none without it. Taken while hidden, as the taskbar
// reads it as a window shows.
void OffTaskbar(HWND hwnd) {
  SetWindowLongPtrW(hwnd, GWL_EXSTYLE, GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & ~WS_EX_APPWINDOW);
}

// An overlay draws its own shape and shadow, often in a margin it leaves clear round its plate or
// over the whole browser window: Windows' rounding, frame and backdrop would show what it leaves
// clear as a shaded box.
void OwnShadow(HWND hwnd) {
  DWM_WINDOW_CORNER_PREFERENCE corners = DWMWCP_DONOTROUND;
  DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corners, sizeof corners);
  DWMNCRENDERINGPOLICY policy = DWMNCRP_DISABLED;
  DwmSetWindowAttribute(hwnd, DWMWA_NCRENDERING_POLICY, &policy, sizeof policy);
  DWM_SYSTEMBACKDROP_TYPE backdrop = DWMSBT_NONE;
  DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &backdrop, sizeof backdrop);
}

void Adopt(Floating& floating, void* view, bool mouse_through, void (*dismissed)(void)) {
  floating.hwnd = Hwnd(view);
  floating.mouse_through = mouse_through;
  floating.dismissed = dismissed;
  ShowWindow(floating.hwnd, SW_HIDE);
  OffTaskbar(floating.hwnd);
  if (mouse_through) {
    SetWindowLongPtrW(floating.hwnd, GWL_EXSTYLE,
                      GetWindowLongPtrW(floating.hwnd, GWL_EXSTYLE) | WS_EX_NOACTIVATE);
  }
  SetWindowSubclass(floating.hwnd, FloatingProc, 1, reinterpret_cast<DWORD_PTR>(&floating));
  // GPUI made the window its size with a border round it, which FloatingProc gives the UI: the
  // window shrinks to that size, or the offer, which keeps the size it opened at, would be 16 by 8
  // points bigger than the UI's.
  RECT client{};
  GetClientRect(floating.hwnd, &client);
  SetWindowPos(floating.hwnd, nullptr, 0, 0, client.right, client.bottom,
               SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
  FloatingByHwnd()[floating.hwnd] = &floating;
  OwnShadow(floating.hwnd);
}

// An overlay whose blur and shadow the host gives, as the Mac's does, and that is a rounded
// rectangle as Windows' frame is (the command bar, the find bar and the offer): Windows 11 rounds
// it and draws its shadow, without its outline, which the UI's rim would double.
void SystemFrame(HWND hwnd) {
  DWM_WINDOW_CORNER_PREFERENCE corners = DWMWCP_ROUND;
  DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corners, sizeof corners);
  DWMNCRENDERINGPOLICY policy = DWMNCRP_USEWINDOWSTYLE;
  DwmSetWindowAttribute(hwnd, DWMWA_NCRENDERING_POLICY, &policy, sizeof policy);
  NoBorder(hwnd);
}

void WatchOwner(HWND root);
// Puts |hwnd| at |x|, |y|, |width| by |height| over the others and on screen. It's shown with
// ShowWindow rather than SWP_SHOWWINDOW, which doesn't send WM_SHOWWINDOW, the message GPUI learns
// its window is on screen from (window.is_visible()), where an overlay's motion starts.
// The UI sizes its overlays from inside its update, where GPUI can't take the new size: its
// renderer grows, but the UI goes on laying out at the old one, so what it adds, as the command
// bar's suggestions, is drawn cut off. The size again after the update reaches it.
void SizeAgainLater(HWND hwnd) {
  Later([hwnd] {
    RECT client{};
    if (IsWindow(hwnd) && GetClientRect(hwnd, &client)) {
      SendMessageW(hwnd, WM_SIZE, SIZE_RESTORED, MAKELPARAM(client.right, client.bottom));
    }
  });
}

void PlaceAndShow(HWND hwnd, int x, int y, int width, int height, bool activate) {
  SetWindowPos(hwnd, HWND_TOP, x, y, width, height, SWP_NOACTIVATE);
  ShowWindow(hwnd, activate ? SW_SHOW : SW_SHOWNA);
  SizeAgainLater(hwnd);
}

// Shows |floating| over |ui_view|'s browser window at |left|, |top| points in its content, |width|
// by |height| points, or its own size where one is negative, with the keyboard if |keyboard|.
void Show(Floating& floating, void* ui_view, double left, double top, double width, double height,
          bool keyboard) {
  HWND root = Root(ui_view);
  if (!floating.hwnd || !root) {
    return;
  }
  double own_width, own_height;
  WindowSize(floating.hwnd, &own_width, &own_height);
  width = width < 0 ? own_width : width;
  height = height < 0 ? own_height : height;
  const double scale = Scale(root);
  // Rounded, not truncated: at 125% a 50-point bar is 63 pixels, not 62 that cut its last row.
  POINT origin{std::lround(left * scale), std::lround(top * scale)};
  ClientToScreen(root, &origin);
  SetWindowLongPtrW(floating.hwnd, GWLP_HWNDPARENT, reinterpret_cast<LONG_PTR>(root));
  WatchOwner(root);
  PlaceAndShow(floating.hwnd, origin.x, origin.y, static_cast<int>(std::lround(width * scale)),
               static_cast<int>(std::lround(height * scale)), keyboard);
  if (keyboard) {
    SetForegroundWindow(floating.hwnd);
    SetFocus(floating.hwnd);
  }
}

// check.sh waits for |what|'s line once |floating| is on screen, as the Mac's and Linux's hosts
// log it as its fade in ends.
void LogShown(const Floating& floating, const char* what) {
  if (floating.hwnd && IsWindowVisible(floating.hwnd)) {
    fprintf(stderr, "browser: %s window shown\n", what);
  }
}

// The view in each browser window the UI gave the keyboard to while an overlay over it had it
// (host_focus), which has it once the overlay gives it back, as the Mac's first responder is the
// window's own while another window is key.
std::map<HWND, HWND>& FocusAfterOverlay() {
  static std::map<HWND, HWND> views;
  return views;
}

// Hides |floating|, giving its browser window the keyboard back if it had it.
void Hide(Floating& floating) {
  if (!floating.hwnd || !IsWindowVisible(floating.hwnd)) {
    return;
  }
  const bool had_keyboard = GetForegroundWindow() == floating.hwnd;
  HWND owner = GetWindow(floating.hwnd, GW_OWNER);
  HWND focus = nullptr;
  if (auto held = FocusAfterOverlay().find(owner); held != FocusAfterOverlay().end()) {
    focus = held->second;
    FocusAfterOverlay().erase(held);
  }
  floating.hiding = true;
  ShowWindow(floating.hwnd, SW_HIDE);
  floating.hiding = false;
  // Not to a browser window that's hiding to close.
  if (had_keyboard && owner && IsWindowVisible(owner)) {
    SetForegroundWindow(owner);
    if (focus && IsWindow(focus)) {
      SetFocus(focus);
    }
  }
}

// Shows |floating| over |ui_view|'s window at |left|, |top|, fading in over |fade| seconds: from
// where it is if it shows there already, as one fading out does. Logs |what| as the fade ends.
void FadeIn(Floating& floating, void* ui_view, double left, double top, double fade,
            const char* what) {
  const bool there = floating.hwnd && IsWindowVisible(floating.hwnd) &&
                     GetWindow(floating.hwnd, GW_OWNER) == Root(ui_view);
  browser_fade(floating.hwnd, there ? -1 : 0, 1, fade);
  Show(floating, ui_view, left, top, -1, -1, false);
  const int fades = ++floating.fades;
  if (fade <= 0) {
    LogShown(floating, what);
    return;
  }
  Floating* faded = &floating;
  After(fade, [faded, fades, what] {
    if (faded->fades == fades) {
      LogShown(*faded, what);
    }
  });
}

// Hides |floating|, fading out over |fade| seconds, then calls |hidden|; with none, at once, which
// also ends a fade out.
void FadeOut(Floating& floating, double fade, std::function<void()> hidden) {
  const int fades = ++floating.fades;
  if (fade <= 0 || !floating.hwnd || !IsWindowVisible(floating.hwnd)) {
    Hide(floating);
    hidden();
    return;
  }
  browser_fade(floating.hwnd, -1, 0, fade);
  Floating* faded = &floating;
  After(fade, [faded, fades, hidden] {
    if (faded->fades == fades) {
      Hide(*faded);
      hidden();
    }
  });
}

void Resize(Floating& floating, double height) {
  if (!floating.hwnd) {
    return;
  }
  RECT rect{};
  GetWindowRect(floating.hwnd, &rect);
  SetWindowPos(floating.hwnd, nullptr, 0, 0, rect.right - rect.left,
               static_cast<int>(std::lround(height * Scale(floating.hwnd))),
               SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
  SizeAgainLater(floating.hwnd);
}

// The UI views host_ui_view made, in order, for the watchers to find a browser window's.
std::vector<HWND>& UiViews() {
  static std::vector<HWND> views;
  return views;
}

// Every UI view host_ui_view made, closed ones too, for the driver's window numbers.
std::vector<HWND>& MadeUiViews() {
  static std::vector<HWND> views;
  return views;
}

void* UiViewOf(HWND root) {
  for (HWND view : UiViews()) {
    if (IsWindow(view) && GetAncestor(view, GA_ROOT) == root) {
      return view;
    }
  }
  return nullptr;
}

// The UI's, which hears the drags over its views (host_take_drops).
bool (*g_dragged)(void*, int, double, double, const host_dragged*, size_t) = nullptr;

// What |data| holds in |format| as text, or nothing.
std::wstring DraggedText(IDataObject* data, CLIPFORMAT format) {
  FORMATETC wanted{format, nullptr, DVASPECT_CONTENT, -1, TYMED_HGLOBAL};
  STGMEDIUM medium{};
  if (FAILED(data->GetData(&wanted, &medium))) {
    return {};
  }
  std::wstring text;
  if (const auto* chars = static_cast<const wchar_t*>(GlobalLock(medium.hGlobal))) {
    // Up to its end or its memory's, as a source may leave the null off.
    const size_t most = GlobalSize(medium.hGlobal) / sizeof(wchar_t);
    text.assign(chars, std::find(chars, chars + most, L'\0'));
    GlobalUnlock(medium.hGlobal);
  }
  ReleaseStgMedium(&medium);
  return text;
}

// The files |data| carries, as file URLs, as Explorer drags them.
std::vector<std::string> DraggedFiles(IDataObject* data) {
  FORMATETC wanted{CF_HDROP, nullptr, DVASPECT_CONTENT, -1, TYMED_HGLOBAL};
  STGMEDIUM medium{};
  std::vector<std::string> urls;
  if (FAILED(data->GetData(&wanted, &medium))) {
    return urls;
  }
  auto drop = static_cast<HDROP>(medium.hGlobal);
  const UINT count = DragQueryFileW(drop, 0xFFFFFFFF, nullptr, 0);
  for (UINT i = 0; i < count; i++) {
    std::wstring path(DragQueryFileW(drop, i, nullptr, 0), L'\0');
    DragQueryFileW(drop, i, path.data(), static_cast<UINT>(path.size() + 1));
    wchar_t url[4096];
    DWORD length = ARRAYSIZE(url);
    if (SUCCEEDED(UrlCreateFromPathW(path.c_str(), url, &length, 0))) {
      urls.push_back(Utf8(url));
    }
  }
  ReleaseStgMedium(&medium);
  return urls;
}

// The target the UI's holder registers for drags over it, from a page or another app, which hands
// them to the UI with what they carry, as the Mac's HostUIView does: files, else a link, else text.
// It has the shell draw the drag's image over the window, as Explorer's own windows do.
class DropTarget final : public IDropTarget {
 public:
  explicit DropTarget(HWND holder) : holder_(holder) {
    CoCreateInstance(CLSID_DragDropHelper, nullptr, CLSCTX_INPROC_SERVER,
                     IID_PPV_ARGS(&helper_));
  }

  HRESULT STDMETHODCALLTYPE QueryInterface(REFIID id, void** out) override {
    if (id != __uuidof(IUnknown) && id != __uuidof(IDropTarget)) {
      *out = nullptr;
      return E_NOINTERFACE;
    }
    *out = static_cast<IDropTarget*>(this);
    AddRef();
    return S_OK;
  }
  ULONG STDMETHODCALLTYPE AddRef() override { return ++references_; }
  ULONG STDMETHODCALLTYPE Release() override {
    const ULONG left = --references_;
    if (!left) {
      delete this;
    }
    return left;
  }

  HRESULT STDMETHODCALLTYPE DragEnter(IDataObject* data, DWORD, POINTL at, DWORD* effect) override {
    urls_ = DraggedFiles(data);
    files_ = !urls_.empty();
    if (urls_.empty()) {
      std::wstring url =
          DraggedText(data, static_cast<CLIPFORMAT>(RegisterClipboardFormatW(CFSTR_INETURLW)));
      // Text, which the UI opens only if it's a web link.
      if (url.empty()) {
        url = DraggedText(data, CF_UNICODETEXT);
        const size_t start = url.find_first_not_of(L" \t\r\n");
        url = start == std::wstring::npos
                  ? std::wstring()
                  : url.substr(start, url.find_last_not_of(L" \t\r\n") - start + 1);
      }
      if (!url.empty()) {
        urls_.push_back(Utf8(url));
      }
    }
    Tell(HOST_DRAG_OVER, at, effect);
    if (helper_) {
      POINT point{at.x, at.y};
      helper_->DragEnter(holder_, data, &point, *effect);
    }
    return S_OK;
  }

  HRESULT STDMETHODCALLTYPE DragOver(DWORD, POINTL at, DWORD* effect) override {
    Tell(HOST_DRAG_OVER, at, effect);
    if (helper_) {
      POINT point{at.x, at.y};
      helper_->DragOver(&point, *effect);
    }
    return S_OK;
  }

  HRESULT STDMETHODCALLTYPE DragLeave() override {
    if (helper_) {
      helper_->DragLeave();
    }
    Tell(HOST_DRAG_LEFT, at_, nullptr);
    urls_.clear();
    return S_OK;
  }

  HRESULT STDMETHODCALLTYPE Drop(IDataObject* data, DWORD, POINTL at, DWORD* effect) override {
    Tell(HOST_DRAG_DROP, at, effect);
    if (helper_) {
      POINT point{at.x, at.y};
      helper_->Drop(data, &point, *effect);
    }
    // After a drop too, as the Mac's draggingEnded, which the UI keeps.
    Tell(HOST_DRAG_LEFT, at, nullptr);
    urls_.clear();
    return S_OK;
  }

 private:
  ~DropTarget() {
    if (helper_) {
      helper_->Release();
    }
  }

  // Tells the UI, with where the pointer is in points from the holder's top-left corner, and
  // answers the drag's source with a copy if the UI takes it.
  void Tell(int phase, POINTL at, DWORD* effect) {
    at_ = at;
    POINT point{at.x, at.y};
    ScreenToClient(holder_, &point);
    const double scale = Scale(holder_);
    std::vector<host_dragged> items;
    for (const std::string& url : urls_) {
      items.push_back({url.c_str(), files_});
    }
    const bool taken = g_dragged && g_dragged(holder_, phase, point.x / scale, point.y / scale,
                                              items.data(), items.size());
    if (effect) {
      *effect = !taken                      ? DROPEFFECT_NONE
                : *effect & DROPEFFECT_COPY ? DROPEFFECT_COPY
                : *effect & DROPEFFECT_LINK ? DROPEFFECT_LINK
                                            : DROPEFFECT_NONE;
    }
  }

  HWND holder_;
  ULONG references_ = 1;
  IDropTargetHelper* helper_ = nullptr;
  std::vector<std::string> urls_;
  bool files_ = false;
  POINTL at_{};
};

// Each holder's target, for the driver's drags.
std::map<HWND, DropTarget*>& DropTargets() {
  static std::map<HWND, DropTarget*> targets;
  return targets;
}

// The frame of each browser window that has closed, from its UI's holder, until the app forgets it:
// the core closes the window before it tells the app, which then saves the last regular one's for
// the next launch (host_window_frame).
std::map<HWND, RECT>& ClosedFrames() {
  static std::map<HWND, RECT> frames;
  return frames;
}

// The holder keeps GPUI's window, its only child, filling it.
int FrameHit(HWND root, POINT at);  // Below, with the frame it answers for.

LRESULT CALLBACK HolderProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) {
  switch (message) {
    case WM_NCHITTEST:
      if (FrameHit(GetAncestor(hwnd, GA_ROOT), {GET_X_LPARAM(lparam), GET_Y_LPARAM(lparam)})) {
        return HTTRANSPARENT;
      }
      break;
    case WM_SIZE:
      if (HWND ui = GetWindow(hwnd, GW_CHILD)) {
        SetWindowPos(ui, nullptr, 0, 0, LOWORD(lparam), HIWORD(lparam),
                     SWP_NOZORDER | SWP_NOACTIVATE);
      }
      return 0;
    case WM_PARENTNOTIFY:
      // A click in the UI takes the keyboard from the page, as one in a top-level window would.
      if (LOWORD(wparam) == WM_LBUTTONDOWN || LOWORD(wparam) == WM_RBUTTONDOWN ||
          LOWORD(wparam) == WM_MBUTTONDOWN) {
        if (HWND ui = GetWindow(hwnd, GW_CHILD); ui && GetFocus() != ui) {
          SetFocus(ui);
        }
      }
      return 0;
    case WM_ERASEBKGND:
      return 1;
    case WM_DESTROY:
      // Its window is still there: Windows destroys a window's children before the window.
      if (HWND root = GetAncestor(hwnd, GA_ROOT);
          root && root != hwnd && !IsIconic(root) && !IsZoomed(root)) {
        GetWindowRect(root, &ClosedFrames()[hwnd]);
      }
      if (auto found = DropTargets().find(hwnd); found != DropTargets().end()) {
        RevokeDragDrop(hwnd);
        found->second->Release();
        DropTargets().erase(found);
      }
      return 0;
  }
  return DefWindowProcW(hwnd, message, wparam, lparam);
}

// Closes ilo's menu, or takes its keyboard back, as the front window changes (defined below).
void MenuForegroundChanged(HWND root, bool ours);
// Has the link preview leave as another of the app's windows comes to the front (defined below).
void LinkPreviewForegroundChanged(HWND front);

// The watchers, which Windows tells of each window that comes to the front.
void (*g_key_window_changed)(void*) = nullptr;
void (*g_resigning_active)(void) = nullptr;
void (*g_became_active)(void) = nullptr;
void (*g_reduce_motion_changed)(void) = nullptr;
bool g_active = true;

void CALLBACK ForegroundChanged(HWINEVENTHOOK, DWORD, HWND hwnd, LONG, LONG, DWORD, DWORD) {
  DWORD process = 0;
  GetWindowThreadProcessId(hwnd, &process);
  const bool ours = process == GetCurrentProcessId();
  // Once a browser window has heard of the change (AppActivating), these have already run.
  if (ours && !g_active && g_became_active) {
    g_became_active();
  } else if (!ours && g_active && g_resigning_active) {
    g_resigning_active();
  }
  MenuForegroundChanged(GetAncestor(hwnd, GA_ROOT), ours);
  if (ours) {
    LinkPreviewForegroundChanged(hwnd);
  }
  g_active = ours;
  if (ours && g_key_window_changed) {
    if (void* ui_view = UiViewOf(GetAncestor(hwnd, GA_ROOT))) {
      g_key_window_changed(ui_view);
    }
  }
}

// Whether |thread| is one of this process's, such as Chromium's file dialogs' own.
bool OurThread(DWORD thread) {
  HANDLE handle = OpenThread(THREAD_QUERY_LIMITED_INFORMATION, FALSE, thread);
  if (!handle) {
    return false;
  }
  const bool ours = GetProcessIdOfThread(handle) == GetCurrentProcessId();
  CloseHandle(handle);
  return ours;
}

// A browser window's WM_ACTIVATEAPP: Windows sends it as another app's window, on |thread|, is
// about to come to the front, or as one of ours is, before the foreground event, as AppKit's
// will-resign-active comes before the other app is in front.
void AppActivating(bool active, DWORD thread) {
  if (active && !g_active) {
    g_active = true;
    if (g_became_active) {
      g_became_active();
    }
  } else if (!active && g_active && !OurThread(thread)) {
    g_active = false;
    if (g_resigning_active) {
      g_resigning_active();
    }
  }
}

void WatchForeground() {
  static HWINEVENTHOOK hook = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND,
                                              nullptr, ForegroundChanged, 0, 0,
                                              WINEVENT_OUTOFCONTEXT);
  (void)hook;
}

Floating g_overlays[4];
// What check.sh calls each overlay, as the Mac's and Linux's hosts do.
const char* const kOverlayNames[std::size(g_overlays)] = {"command bar", "icon picker",
                                                         "theme picker", "folder preview"};
// Hides overlay |index| if it's shown, saying so, and whether it was.
bool HideOverlay(int index) {
  Floating& floating = g_overlays[index];
  ++floating.shows;
  if (!floating.hwnd || !IsWindowVisible(floating.hwnd)) {
    return false;
  }
  Hide(floating);
  // check.sh reads this line.
  fprintf(stderr, "browser: %s hidden\n", kOverlayNames[index]);
  return true;
}

// Hides the overlay whose window is |hwnd|, if it's one and shown, saying so.
bool HideOverlayOf(HWND hwnd) {
  for (int index = 0; index < int(std::size(g_overlays)); ++index) {
    if (g_overlays[index].hwnd == hwnd) {
      return HideOverlay(index);
    }
  }
  return false;
}
struct Corner {
  Floating floating;
  double right = 0;
  double top = 0;
};
std::map<void*, Corner> g_find_bars;
std::map<void*, Corner> g_offers;
std::map<void*, std::pair<double, double>> g_pane_corners;
Floating g_hud;
double g_hud_right = 0, g_hud_top = 0;
Floating g_drop_targets;
Floating g_dialog;
Floating g_site_controls;

// The link preview (platform/host.h), as Linux's (link_preview_window.rs): its window, the dim and
// the card, covers the browser window's content and never takes the keyboard. No window of the
// app's can hold a page here either, so the core shows the page in the browser window, over its
// pages (window_views.cc's ShowPageIn), and the overlay's region has a hole there, which the page
// shows and takes clicks through. Its keys are the page's, and the core hands the preview the ones
// it acts on.
//
// Another of the app's windows coming to the front, such as another browser window or Settings, is
// the user leaving the preview, as another window becoming key is on the Mac. Another app isn't.
//
// shortcut: the page doesn't scale with the card's motion, as the Mac's does, as on Linux.
struct LinkPreview {
  Floating floating;
  void (*dismissed)(void) = nullptr;
  // The browser window it covers, while it shows.
  HWND root = nullptr;
  // Where the page shows, device pixels from the content's top left, and its corners' diameter.
  struct Hole {
    RECT rect;
    int diameter;
    bool operator==(const Hole& other) const {
      return EqualRect(&rect, &other.rect) && diameter == other.diameter;
    }
  };
  std::optional<Hole> hole;
};
LinkPreview g_link_preview;
UINT_PTR const kLinkPreviewOwner = 2;

// The preview's frame, from its window's corner to its content's, and the browser window's content
// on screen, which its content covers.
struct LinkPreviewFrame {
  POINT inset;
  SIZE border;
  POINT origin;
  RECT client;
};

LinkPreviewFrame MeasureLinkPreview(HWND hwnd, HWND root) {
  LinkPreviewFrame frame{};
  GetClientRect(root, &frame.client);
  ClientToScreen(root, &frame.origin);
  // GPUI's floating window keeps a frame round its content, which the region leaves out.
  RECT window{};
  GetWindowRect(hwnd, &window);
  RECT content{};
  GetClientRect(hwnd, &content);
  POINT corner{0, 0};
  ClientToScreen(hwnd, &corner);
  frame.inset = {corner.x - window.left, corner.y - window.top};
  frame.border = {(window.right - window.left) - content.right,
                  (window.bottom - window.top) - content.bottom};
  return frame;
}

// Shows the preview only over the browser window's content, less the hole the page shows in.
void ShapeLinkPreview() {
  HWND hwnd = g_link_preview.floating.hwnd;
  HWND root = g_link_preview.root;
  if (!hwnd || !root) {
    return;
  }
  const LinkPreviewFrame frame = MeasureLinkPreview(hwnd, root);
  HRGN region = CreateRectRgn(0, 0, frame.client.right, frame.client.bottom);
  if (const auto& hole = g_link_preview.hole) {
    HRGN page = CreateRoundRectRgn(hole->rect.left, hole->rect.top, hole->rect.right + 1,
                                   hole->rect.bottom + 1, hole->diameter, hole->diameter);
    CombineRgn(region, region, page, RGN_DIFF);
    DeleteObject(page);
  }
  OffsetRgn(region, frame.inset.x, frame.inset.y);
  // The window owns the region from here.
  SetWindowRgn(hwnd, region, IsWindowVisible(hwnd));
}

// Puts the preview's content over all of its browser window's content, and shows it. Only from the
// host's own tasks (Later): Windows sends WM_SIZE as it resizes, and GPUI, inside its update while
// the app shows the preview or draws it, drops the size it can't take, so its view would keep its
// first size, the dim covering only that and missing a click on the rest.
void PlaceLinkPreview() {
  HWND hwnd = g_link_preview.floating.hwnd;
  HWND root = g_link_preview.root;
  if (!hwnd || !root) {
    return;
  }
  const LinkPreviewFrame frame = MeasureLinkPreview(hwnd, root);
  SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, reinterpret_cast<LONG_PTR>(root));
  // Shaped before it shows: the region is in the root's content, which doesn't need the new size.
  ShapeLinkPreview();
  PlaceAndShow(hwnd, frame.origin.x - frame.inset.x, frame.origin.y - frame.inset.y,
               frame.client.right + frame.border.cx, frame.client.bottom + frame.border.cy, false);
}

// The browser window under the preview moved or resized, which it follows, or is going.
LRESULT CALLBACK LinkPreviewOwnerProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam,
                                      UINT_PTR, DWORD_PTR) {
  if (message == WM_WINDOWPOSCHANGED && hwnd == g_link_preview.root) {
    Later([] { PlaceLinkPreview(); });
  } else if (message == WM_DESTROY && hwnd == g_link_preview.root) {
    host_hide_link_preview();
  }
  return DefSubclassProc(hwnd, message, wparam, lparam);
}

Floating g_capture;
Floating g_toast;
Floating g_tooltip;
double g_tooltip_margin = 0;
Floating g_card;
double g_card_margin = 0;
Floating g_link_target;
double g_link_margin = 0;
void (*g_link_pointer)(double, double) = nullptr;
void* g_link_shown_over = nullptr;
struct Reveal {
  Floating floating;
  void (*pointer_moved)(void*, double) = nullptr;
};
std::map<void*, Reveal> g_reveals;
Floating g_settings;
bool g_settings_placed = false;

// While the link target or a reveal shows, the pointer's moves over the app's windows.
HHOOK g_mouse_hook = nullptr;

// In a driven run only the driver's pointer reaches the app, as drive.cc keeps the desktop's out of
// its windows. The thread's mouse hooks hear the desktop's before that, and Windows moves it over a
// window as one shows, so a menu opening where it rests would hover its row (#1125).
bool DesktopPointerHeard() {
  static const bool driven = GetEnvironmentVariableW(L"BROWSER_DRIVE", nullptr, 0) > 0;
  return !driven;
}

// The pointer at |point| on screen, for the link target and the reveals.
void PointerMoved(POINT point) {
  if (g_link_pointer && g_link_shown_over && IsWindowVisible(g_link_target.hwnd)) {
    HWND root = Root(g_link_shown_over);
    POINT in = point;
    ScreenToClient(root, &in);
    const double scale = Scale(root);
    g_link_pointer(in.x / scale, in.y / scale);
  }
  for (auto& [ui_view, reveal] : g_reveals) {
    if (reveal.pointer_moved && IsWindowVisible(reveal.floating.hwnd)) {
      HWND root = Root(ui_view);
      POINT in = point;
      ScreenToClient(root, &in);
      reveal.pointer_moved(ui_view, in.x / Scale(root));
    }
  }
}

LRESULT CALLBACK MouseMoved(int code, WPARAM wparam, LPARAM lparam) {
  if (code == HC_ACTION && wparam == WM_MOUSEMOVE && DesktopPointerHeard()) {
    PointerMoved(reinterpret_cast<MOUSEHOOKSTRUCT*>(lparam)->pt);
  }
  return CallNextHookEx(nullptr, code, wparam, lparam);
}

void WatchPointer() {
  if (!g_mouse_hook) {
    g_mouse_hook = SetWindowsHookExW(WH_MOUSE, MouseMoved, nullptr, GetCurrentThreadId());
  }
}

// The pointer stops being watched once nothing needs it, so an idle browser isn't woken by it.
void MaybeStopWatchingPointer() {
  bool needed = g_link_target.hwnd && IsWindowVisible(g_link_target.hwnd);
  for (auto& [ui_view, reveal] : g_reveals) {
    needed = needed || IsWindowVisible(reveal.floating.hwnd);
  }
  if (!needed && g_mouse_hook) {
    UnhookWindowsHookEx(g_mouse_hook);
    g_mouse_hook = nullptr;
  }
}

void ShowCorner(Corner& corner, void* ui_view, bool keyboard) {
  HWND root = Root(ui_view);
  double content_width, content_height, width, height;
  ContentSize(root, &content_width, &content_height);
  WindowSize(corner.floating.hwnd, &width, &height);
  auto [extra_right, extra_top] = g_pane_corners[ui_view];
  Show(corner.floating, ui_view, content_width - corner.right - extra_right - width,
       corner.top + extra_top, -1, -1, keyboard);
}

// Control's release, for host_watch_control.
void (*g_control_released)(void) = nullptr;
HHOOK g_key_hook = nullptr;

// A key went down or up: Control may have been let go.
void KeyChanged() {
  if (g_control_released && (GetKeyState(VK_CONTROL) & 0x8000) == 0) {
    void (*released)(void) = g_control_released;
    g_control_released = nullptr;
    Later([released] { released(); });
    UnhookWindowsHookEx(g_key_hook);
    g_key_hook = nullptr;
  }
}

LRESULT CALLBACK KeyPressed(int code, WPARAM wparam, LPARAM lparam) {
  if (code == HC_ACTION) {
    KeyChanged();
  }
  return CallNextHookEx(nullptr, code, wparam, lparam);
}

// Keys typed right after a shortcut that opens the command bar or the find bar belong in its field
// (Ardia P7), but the bar takes the keyboard only once it has drawn, and a shortcut the page gets
// first, such as Ctrl+L, comes back only after the keys behind it reached the page. So from the
// shortcut until the bar has the keyboard, keys are held and then given to it in the order typed.
// For a shortcut the page gets first, the core says if the page kept it and no bar opens
// (host_release_held_keys). A hold lasts kHoldKeysMs at most, as the core's does. They're posted to
// our own window only; SendInput would reach whatever app is in front (platform/host.h's warning).
std::vector<MSG> g_held_keys;
bool g_holding_keys = false;
// The window the keys wait for, or null for whichever bar opens.
HWND g_keys_for = nullptr;
HHOOK g_held_hook = nullptr;
UINT_PTR g_hold_timer = 0;
constexpr UINT kHoldKeysMs = 1000;

// Gives the keys held to the window that now has the keyboard, unless they wait for another bar.
void ReleaseKeys(HWND from) {
  if ((from && g_keys_for && from != g_keys_for) || !g_holding_keys) {
    return;
  }
  g_holding_keys = false;
  if (g_hold_timer) {
    KillTimer(nullptr, g_hold_timer);
    g_hold_timer = 0;
  }
  // A bar, or with none the page, which kept the shortcut.
  const bool to_bar = g_keys_for || from;
  HWND to = g_keys_for ? g_keys_for : (from ? from : GetFocus());
  g_keys_for = nullptr;
  if (!g_held_keys.empty()) {
    // check.sh reads this line, which names the window that took them as the Mac's does.
    fprintf(stderr, "browser: held %zu key events for %s\n", g_held_keys.size(),
            to_bar ? "GPUIPanel" : "BrowserCoreNSWindow");
  }
  for (const MSG& held : g_held_keys) {
    PostMessageW(to, held.message, held.wParam, held.lParam);
  }
  g_held_keys.clear();
}

void CALLBACK HoldTimedOut(HWND, UINT, UINT_PTR id, DWORD) {
  if (id == g_hold_timer) {
    ReleaseKeys(nullptr);
  }
}

// Sets aside each keyboard message the thread's queue gives up while holding, swallowing it before
// TranslateMessage so no WM_CHAR is made for it; the repost is translated once it reaches the bar.
// The shortcut that opened the bar was pulled before the hold began, so it isn't held.
LRESULT CALLBACK HeldKeys(int code, WPARAM wparam, LPARAM lparam) {
  if (code == HC_ACTION && g_holding_keys && wparam == PM_REMOVE) {
    MSG* msg = reinterpret_cast<MSG*>(lparam);
    switch (msg->message) {
      case WM_KEYDOWN:
      case WM_SYSKEYDOWN:
      case WM_KEYUP:
      case WM_SYSKEYUP:
        g_held_keys.push_back(*msg);
        msg->message = WM_NULL;
        break;
    }
  }
  return CallNextHookEx(nullptr, code, wparam, lparam);
}

// Holds the keys typed from now until |window|, or if null whichever bar opens, has the keyboard.
void HoldKeys(HWND window) {
  if (!g_held_hook) {
    g_held_hook = SetWindowsHookExW(WH_GETMESSAGE, HeldKeys, nullptr, GetCurrentThreadId());
  }
  if (g_hold_timer) {
    KillTimer(nullptr, g_hold_timer);
  }
  g_holding_keys = true;
  g_keys_for = window;
  g_hold_timer = SetTimer(nullptr, 0, kHoldKeysMs, HoldTimedOut);
}

// A held Escape's repeats reach only the window its press went to, as on the Mac (host.mm's
// WatchHeldEscape): the command bar and the find bar close on Escape going down, which gives the
// browser window the keyboard, and the repeats would go on to the page there, taking it out of its
// full screen or closing its own dialog (#385). Once the key is let go, the next press goes
// wherever the keyboard is. Floating windows are owned, not children, so each is its own root.
HHOOK g_escape_hook = nullptr;
HWND g_escape_pressed_in = nullptr;

LRESULT CALLBACK HeldEscape(int code, WPARAM wparam, LPARAM lparam) {
  if (code == HC_ACTION && wparam == PM_REMOVE) {
    MSG* msg = reinterpret_cast<MSG*>(lparam);
    if (msg->wParam == VK_ESCAPE) {
      const HWND root = msg->hwnd ? GetAncestor(msg->hwnd, GA_ROOT) : nullptr;
      const bool repeat = msg->lParam & (1 << 30);
      switch (msg->message) {
        case WM_KEYUP:
        case WM_SYSKEYUP:
          g_escape_pressed_in = nullptr;
          break;
        case WM_KEYDOWN:
        case WM_SYSKEYDOWN:
          if (!repeat) {
            g_escape_pressed_in = root;
          } else if (g_escape_pressed_in && root != g_escape_pressed_in) {
            msg->message = WM_NULL;
          }
          break;
      }
    }
  }
  return CallNextHookEx(nullptr, code, wparam, lparam);
}

void WatchHeldEscape() {
  if (!g_escape_hook) {
    g_escape_hook = SetWindowsHookExW(WH_GETMESSAGE, HeldEscape, nullptr, GetCurrentThreadId());
  }
}

// The window ilo's menu's coordinates are from, and which it's owned by: a browser window's
// content, or its reveal's panel with |in_reveal|.
HWND MenuBase(void* ui_view, bool in_reveal) {
  if (in_reveal) {
    if (auto it = g_reveals.find(ui_view);
        it != g_reveals.end() && it->second.floating.hwnd &&
        IsWindowVisible(it->second.floating.hwnd)) {
      return it->second.floating.hwnd;
    }
  }
  return Root(ui_view);
}

// The user left the menu. Posted, as the UI may be handling what made them leave; the menu's own
// host_hide_menu_window follows once the UI closes it.
void PostMenuDismiss(bool give_back) {
  if (!g_menu.shown) {
    return;
  }
  g_menu.give_back = g_menu.give_back && give_back;
  if (void (*dismissed)(void) = g_menu.dismissed) {
    Later([dismissed] { dismissed(); });
  }
}

// Takes the app's windows off |root|, hidden: the menu, the overlays and the window's own panels.
// Windows destroys a window's owned windows with it, before it says it's going, and these are GPUI
// windows the app goes on using, shared between browser windows or closed by the app itself, so a
// quit with the command bar up crashed in one freed under it (#1058).
void Disown(HWND root) {
  // The menu may be over a reveal, which root owns.
  if (HWND owner = g_menu.hwnd ? GetWindow(g_menu.hwnd, GW_OWNER) : nullptr;
      owner && (owner == root || GetWindow(owner, GW_OWNER) == root)) {
    ShowWindow(g_menu.hwnd, SW_HIDE);
    SetWindowLongPtrW(g_menu.hwnd, GWLP_HWNDPARENT, 0);
    PostMenuDismiss(false);
  }
  for (auto& [hwnd, floating] : FloatingByHwnd()) {
    if (GetWindow(hwnd, GW_OWNER) == root) {
      Hide(*floating);
      SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, 0);
    }
  }
}

UINT_PTR const kOwner = 3;

// |root| moved or was resized: the find bar and the offer showing over it keep to its page's top
// right corner, as the Mac's child windows and Linux's panels do. Windows moves no owned window.
void PlaceCorners(HWND root) {
  for (auto* corners : {&g_find_bars, &g_offers}) {
    for (auto& [ui_view, corner] : *corners) {
      if (corner.floating.hwnd && IsWindowVisible(corner.floating.hwnd) && Root(ui_view) == root) {
        double content_width, content_height, width, height;
        ContentSize(root, &content_width, &content_height);
        WindowSize(corner.floating.hwnd, &width, &height);
        auto [extra_right, extra_top] = g_pane_corners[ui_view];
        const double scale = Scale(root);
        POINT origin{std::lround((content_width - corner.right - extra_right - width) * scale),
                     std::lround((corner.top + extra_top) * scale)};
        ClientToScreen(root, &origin);
        SetWindowPos(corner.floating.hwnd, nullptr, origin.x, origin.y, 0, 0,
                     SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
      }
    }
  }
}

// A browser window the app's windows are over. Windows hides one before it destroys it and the
// windows it owns, so they leave it as it hides.
LRESULT CALLBACK OwnerProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam, UINT_PTR,
                           DWORD_PTR) {
  const UINT flags =
      message == WM_WINDOWPOSCHANGED ? reinterpret_cast<const WINDOWPOS*>(lparam)->flags : 0;
  if (flags & SWP_HIDEWINDOW) {
    Disown(hwnd);
  } else if (message == WM_WINDOWPOSCHANGED && (~flags & (SWP_NOMOVE | SWP_NOSIZE))) {
    PlaceCorners(hwnd);
  } else if (message == WM_NCDESTROY) {
    RemoveWindowSubclass(hwnd, OwnerProc, kOwner);
  }
  return DefSubclassProc(hwnd, message, wparam, lparam);
}

void WatchOwner(HWND root) {
  SetWindowSubclass(root, OwnerProc, kOwner, 0);
}

// Every pointer event in the app, before the window under the pointer hears it: the menu takes
// what's on it, so a press outside closes it and goes no further (a right-press outside passes
// through, to open that window's own menu), as the Mac's local monitor does.
bool MenuTakesPointer(WPARAM message, POINT pt) {
  if (g_menu.shown && g_menu.pointer) {
    int kind = -1;
    switch (message) {
      case WM_MOUSEMOVE:
      case WM_NCMOUSEMOVE:
        kind = 0;
        break;
      case WM_LBUTTONDOWN:
      case WM_NCLBUTTONDOWN:
      case WM_MBUTTONDOWN:
      case WM_NCMBUTTONDOWN:
      case WM_XBUTTONDOWN:
      case WM_NCXBUTTONDOWN:
        kind = 1;
        break;
      case WM_RBUTTONDOWN:
      case WM_NCRBUTTONDOWN:
        kind = 2;
        break;
      case WM_LBUTTONUP:
      case WM_NCLBUTTONUP:
      case WM_RBUTTONUP:
      case WM_NCRBUTTONUP:
      case WM_MBUTTONUP:
      case WM_NCMBUTTONUP:
      case WM_XBUTTONUP:
      case WM_NCXBUTTONUP:
        kind = 3;
        break;
      case WM_MOUSEWHEEL:
      case WM_MOUSEHWHEEL:
        kind = 4;
        break;
    }
    if (kind >= 0) {
      const double scale = Scale(g_menu.hwnd);
      return g_menu.pointer(kind, (pt.x - g_menu.frame.left) / scale,
                            (pt.y - g_menu.frame.top) / scale);
    }
  }
  return false;
}

LRESULT CALLBACK MenuMouse(int code, WPARAM wparam, LPARAM lparam) {
  if (code == HC_ACTION && DesktopPointerHeard() &&
      MenuTakesPointer(wparam, reinterpret_cast<MOUSEHOOKSTRUCT*>(lparam)->pt)) {
    return 1;
  }
  return CallNextHookEx(nullptr, code, wparam, lparam);
}

// While the menu shows, a shortcut (a key with Ctrl, Alt or the Windows key) closes it and goes no
// further, as it was typed at the menu; its own keys, such as the arrows, Enter and Esc, reach its
// window, which has the keyboard.
bool MenuTakesKey(WPARAM wparam, LPARAM lparam) {
  const bool down = (HIWORD(lparam) & KF_UP) == 0;
  if (g_menu.shown && down) {
    const bool chord = (GetKeyState(VK_CONTROL) & 0x8000) || (GetKeyState(VK_MENU) & 0x8000) ||
                       (GetKeyState(VK_LWIN) & 0x8000) || (GetKeyState(VK_RWIN) & 0x8000);
    const bool modifier = wparam == VK_CONTROL || wparam == VK_MENU || wparam == VK_SHIFT ||
                          wparam == VK_LWIN || wparam == VK_RWIN;
    if (chord && !modifier) {
      PostMenuDismiss(true);
      return true;
    }
  }
  return false;
}

LRESULT CALLBACK MenuKeys(int code, WPARAM wparam, LPARAM lparam) {
  if (code == HC_ACTION && MenuTakesKey(wparam, lparam)) {
    return 1;
  }
  return CallNextHookEx(nullptr, code, wparam, lparam);
}

void InstallMenuHooks() {
  const DWORD thread = GetCurrentThreadId();
  if (!g_menu_mouse_hook) {
    g_menu_mouse_hook = SetWindowsHookExW(WH_MOUSE, MenuMouse, nullptr, thread);
  }
  if (!g_menu_key_hook) {
    g_menu_key_hook = SetWindowsHookExW(WH_KEYBOARD, MenuKeys, nullptr, thread);
  }
}

// Taken off once the menu hides, so an idle browser isn't woken by the pointer or keys.
void RemoveMenuHooks() {
  if (g_menu_mouse_hook) {
    UnhookWindowsHookEx(g_menu_mouse_hook);
    g_menu_mouse_hook = nullptr;
  }
  if (g_menu_key_hook) {
    UnhookWindowsHookEx(g_menu_key_hook);
    g_menu_key_hook = nullptr;
  }
}

void MenuForegroundChanged(HWND root, bool ours) {
  if (!g_menu.shown) {
    return;
  }
  if (!ours) {
    PostMenuDismiss(false);
  } else if (root == g_menu.hwnd) {
    // The menu itself.
  } else if (root == g_menu.browser || root == g_menu.parent) {
    // Chromium made its own window key, such as when a page commits; that isn't the user leaving,
    // so the menu takes the keyboard back.
    SetForegroundWindow(g_menu.hwnd);
  } else {
    PostMenuDismiss(false);
  }
}

// Each monitor's work area for host_screens, the primary first, as EnumDisplayMonitors finds them.
BOOL CALLBACK CollectMonitor(HMONITOR monitor, HDC, LPRECT, LPARAM data) {
  auto* works = reinterpret_cast<std::vector<RECT>*>(data);
  MONITORINFO info{sizeof info};
  if (GetMonitorInfoW(monitor, &info)) {
    if (info.dwFlags & MONITORINFOF_PRIMARY) {
      works->insert(works->begin(), info.rcWork);
    } else {
      works->push_back(info.rcWork);
    }
  }
  return TRUE;
}

using Microsoft::WRL::ComPtr;

// The drag source for host_drag_file: it keeps dragging while the left button is held, copies where
// it's let go, and cancels on Escape or the right button, as Explorer's own drag does.
class FileDropSource : public IDropSource {
 public:
  HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** out) override {
    if (iid == __uuidof(IUnknown) || iid == __uuidof(IDropSource)) {
      *out = static_cast<IDropSource*>(this);
      AddRef();
      return S_OK;
    }
    *out = nullptr;
    return E_NOINTERFACE;
  }
  ULONG STDMETHODCALLTYPE AddRef() override { return ++refs_; }
  ULONG STDMETHODCALLTYPE Release() override {
    const ULONG refs = --refs_;
    if (refs == 0) {
      delete this;
    }
    return refs;
  }
  HRESULT STDMETHODCALLTYPE QueryContinueDrag(BOOL escape, DWORD keys) override {
    if (escape || (keys & MK_RBUTTON)) {
      return DRAGDROP_S_CANCEL;
    }
    if (!(keys & MK_LBUTTON)) {
      return DRAGDROP_S_DROP;
    }
    return S_OK;
  }
  HRESULT STDMETHODCALLTYPE GiveFeedback(DWORD) override {
    return DRAGDROP_S_USEDEFAULTCURSORS;
  }

 private:
  ULONG refs_ = 1;
};

// The shell's own data object for the file at |path|, so a drag carries it as Explorer does, with
// its drag image; null if there's no such file.
ComPtr<IDataObject> FileDataObject(const std::wstring& path) {
  PIDLIST_ABSOLUTE pidl = nullptr;
  if (FAILED(SHParseDisplayName(path.c_str(), nullptr, &pidl, 0, nullptr)) || !pidl) {
    return nullptr;
  }
  ComPtr<IShellFolder> parent;
  PCUITEMID_CHILD child = nullptr;
  ComPtr<IDataObject> data;
  if (SUCCEEDED(SHBindToParent(pidl, IID_PPV_ARGS(&parent), &child))) {
    parent->GetUIObjectOf(nullptr, 1, &child, __uuidof(IDataObject), nullptr,
                          reinterpret_cast<void**>(data.GetAddressOf()));
  }
  CoTaskMemFree(pidl);
  return data;
}

// Encodes the shell's thumbnail or icon for |path| as a PNG, off the main thread. An image (or any
// file Windows makes a thumbnail for) gets its thumbnail scaled to fit 96 px, its aspect kept; the
// rest the file's 52 px icon. Fills |png|, |width|, |height| and |image|, returning whether it got
// one. The caller has CoInitialized this thread.
bool PreviewPng(const std::wstring& path, std::vector<uint8_t>* png, double* width, double* height,
                bool* image) {
  ComPtr<IShellItemImageFactory> factory;
  if (FAILED(SHCreateItemFromParsingName(path.c_str(), nullptr, IID_PPV_ARGS(&factory)))) {
    return false;
  }
  HBITMAP bitmap = nullptr;
  *image = SUCCEEDED(factory->GetImage({96, 96}, SIIGBF_THUMBNAILONLY, &bitmap)) && bitmap;
  if (!*image && (FAILED(factory->GetImage({52, 52}, SIIGBF_ICONONLY, &bitmap)) || !bitmap)) {
    return false;
  }
  BITMAP info{};
  GetObjectW(bitmap, sizeof info, &info);
  *width = info.bmWidth;
  *height = info.bmHeight;

  ComPtr<IWICImagingFactory> wic;
  ComPtr<IWICBitmap> source;
  ComPtr<IStream> stream;
  ComPtr<IWICBitmapEncoder> encoder;
  ComPtr<IWICBitmapFrameEncode> frame;
  ComPtr<IPropertyBag2> options;
  WICPixelFormatGUID format = GUID_WICPixelFormat32bppBGRA;
  STATSTG stat{};
  bool ok = false;
  HGLOBAL global = nullptr;
  if (SUCCEEDED(CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER,
                                 IID_PPV_ARGS(&wic))) &&
      SUCCEEDED(wic->CreateBitmapFromHBITMAP(bitmap, nullptr, WICBitmapUsePremultipliedAlpha,
                                             &source)) &&
      SUCCEEDED(CreateStreamOnHGlobal(nullptr, TRUE, &stream)) &&
      SUCCEEDED(wic->CreateEncoder(GUID_ContainerFormatPng, nullptr, &encoder)) &&
      SUCCEEDED(encoder->Initialize(stream.Get(), WICBitmapEncoderNoCache)) &&
      SUCCEEDED(encoder->CreateNewFrame(&frame, &options)) &&
      SUCCEEDED(frame->Initialize(options.Get())) &&
      SUCCEEDED(frame->SetSize(info.bmWidth, info.bmHeight)) &&
      SUCCEEDED(frame->SetPixelFormat(&format)) &&
      SUCCEEDED(frame->WriteSource(source.Get(), nullptr)) && SUCCEEDED(frame->Commit()) &&
      SUCCEEDED(encoder->Commit()) && SUCCEEDED(stream->Stat(&stat, STATFLAG_NONAME)) &&
      SUCCEEDED(GetHGlobalFromStream(stream.Get(), &global)) && global) {
    if (void* data = GlobalLock(global)) {
      const auto* bytes = static_cast<const uint8_t*>(data);
      png->assign(bytes, bytes + static_cast<size_t>(stat.cbSize.QuadPart));
      GlobalUnlock(global);
      ok = !png->empty();
    }
  }
  DeleteObject(bitmap);
  return ok;
}

// ilo's own green, for the taskbar dot without a Space colour, as the Dock icon's own green is.
constexpr uint32_t kIloGreen = 0x0B8045;

// A round dot in |rgb| (0xRRGGBB) as a 32 px icon with a soft edge and a clear surround, for the
// taskbar button's overlay. The caller destroys it.
HICON DotIcon(uint32_t rgb) {
  const int size = 32;
  BITMAPINFO info{};
  info.bmiHeader.biSize = sizeof info.bmiHeader;
  info.bmiHeader.biWidth = size;
  info.bmiHeader.biHeight = -size;  // Top-down, so the rows read as drawn.
  info.bmiHeader.biPlanes = 1;
  info.bmiHeader.biBitCount = 32;
  info.bmiHeader.biCompression = BI_RGB;
  uint8_t* bits = nullptr;
  HBITMAP color =
      CreateDIBSection(nullptr, &info, DIB_RGB_COLORS, reinterpret_cast<void**>(&bits), nullptr, 0);
  if (!color) {
    return nullptr;
  }
  const double red = (rgb >> 16) & 0xFF, green = (rgb >> 8) & 0xFF, blue = rgb & 0xFF;
  const double center = (size - 1) / 2.0;
  const double radius = size / 2.0 - 1;
  for (int y = 0; y < size; ++y) {
    for (int x = 0; x < size; ++x) {
      const double distance = std::hypot(x - center, y - center);
      const double a = distance <= radius - 0.5  ? 1.0
                       : distance >= radius + 0.5 ? 0.0
                                                  : radius + 0.5 - distance;
      uint8_t* pixel = bits + (y * size + x) * 4;  // BGRA, alpha premultiplied, as an icon wants.
      pixel[0] = static_cast<uint8_t>(blue * a);
      pixel[1] = static_cast<uint8_t>(green * a);
      pixel[2] = static_cast<uint8_t>(red * a);
      pixel[3] = static_cast<uint8_t>(a * 255 + 0.5);
    }
  }
  HBITMAP mask = CreateBitmap(size, size, 1, 1, nullptr);
  ICONINFO icon{};
  icon.fIcon = TRUE;
  icon.hbmColor = color;
  icon.hbmMask = mask;
  HICON result = CreateIconIndirect(&icon);
  DeleteObject(color);
  DeleteObject(mask);
  return result;
}

// The taskbar, made once. Null if it isn't there (an older Windows or a failure).
ITaskbarList3* Taskbar() {
  static ComPtr<ITaskbarList3> taskbar = [] {
    ComPtr<ITaskbarList3> list;
    if (SUCCEEDED(CoCreateInstance(__uuidof(TaskbarList), nullptr, CLSCTX_INPROC_SERVER,
                                   IID_PPV_ARGS(&list))) &&
        SUCCEEDED(list->HrInit())) {
      return list;
    }
    return ComPtr<ITaskbarList3>{};
  }();
  return taskbar.Get();
}

}  // namespace

namespace {

namespace wdt = winrt::Windows::ApplicationModel::DataTransfer;
namespace wss = winrt::Windows::Storage::Streams;
namespace wf = winrt::Windows::Foundation;

// Opens the system share flyout for |root|, letting |fill| put the shared content on the package
// the system asks for. ShowShareUIForWindow centres its own flyout, so unlike the Mac there's no
// point to hang it from. The UI thread is already an OLE apartment (GPUI's RegisterDragDrop), so
// the first share initializes C++/WinRT on it as single-threaded (RoInitialize accepts the running
// STA) and never uninitializes, keeping the count balanced for the process's life.
void ShowShareSheet(HWND root, std::function<void(const wdt::DataPackage&)> fill) {
  if (!root) {
    return;
  }
  try {
    static bool ready = false;
    if (!ready) {
      winrt::init_apartment(winrt::apartment_type::single_threaded);
      ready = true;
    }
    auto interop =
        winrt::get_activation_factory<wdt::DataTransferManager, IDataTransferManagerInterop>();
    wdt::DataTransferManager manager{nullptr};
    winrt::check_hresult(interop->GetForWindow(
        root, winrt::guid_of<wdt::DataTransferManager>(), winrt::put_abi(manager)));
    manager.DataRequested([fill](const wdt::DataTransferManager&,
                                 const wdt::DataRequestedEventArgs& args) {
      fill(args.Request().Data());
    });
    winrt::check_hresult(interop->ShowShareUIForWindow(root));
  } catch (const winrt::hresult_error& error) {
    fwprintf(stderr, L"browser: the share sheet failed: %ls\n", error.message().c_str());
  }
}

}  // namespace

namespace {

#ifndef PW_RENDERFULLCONTENT
#define PW_RENDERFULLCONTENT 0x00000002
#endif

// What the first window looks like until its UI has drawn, put on screen before the browser
// starts: the window's frame, the sidebar's picture and the sidebar's colour past it.
struct LaunchWindow {
  HWND hwnd = nullptr;
  // Never shown: owned by it, the launch window has no taskbar button of its own, which wouldn't
  // carry the app ID the core gives the browser window, whose button comes as it shows.
  HWND owner = nullptr;
  HBITMAP picture = nullptr;
  SIZE picture_size{};
  HBRUSH background = nullptr;
};
LaunchWindow g_launch;

// The picture of a |width| by |height| window's sidebar, named for the size, in host_window_frame's
// terms, as a window opens at another size than the one it quit at.
std::wstring LaunchPicturePath(const char* profile_dir, double width, double height) {
  wchar_t name[64];
  swprintf(name, 64, L"\\launch-picture-%.0fx%.0f.png", width, height);
  return Wide(profile_dir) + name;
}

// Whether apps are light in the user's Settings, which a window follows until its Space's look is
// known. Light when the setting isn't there, as Windows is.
bool SystemIsLight() {
  DWORD light = 1;
  DWORD size = sizeof light;
  RegGetValueW(HKEY_CURRENT_USER,
               L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
               L"AppsUseLightTheme", RRF_RT_REG_DWORD, nullptr, &light, &size);
  return light != 0;
}

// The picture at |path| as a top-down bitmap, if it's |height| pixels high: one taken on a screen
// of another scale is left out. Fills |edge| with the colour of its right edge halfway down, the
// sidebar's colour with nothing on it.
HBITMAP LoadLaunchPicture(const std::wstring& path, LONG height, SIZE* size, COLORREF* edge) {
  const HRESULT com = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
  HBITMAP bitmap = nullptr;
  {
    ComPtr<IWICImagingFactory> wic;
    ComPtr<IWICBitmapDecoder> decoder;
    ComPtr<IWICBitmapFrameDecode> frame;
    ComPtr<IWICFormatConverter> converter;
    UINT width = 0, high = 0;
    if (SUCCEEDED(CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER,
                                   IID_PPV_ARGS(&wic))) &&
        SUCCEEDED(wic->CreateDecoderFromFilename(path.c_str(), nullptr, GENERIC_READ,
                                                 WICDecodeMetadataCacheOnDemand, &decoder)) &&
        SUCCEEDED(decoder->GetFrame(0, &frame)) && SUCCEEDED(frame->GetSize(&width, &high)) &&
        width > 0 && static_cast<LONG>(high) == height &&
        SUCCEEDED(wic->CreateFormatConverter(&converter)) &&
        SUCCEEDED(converter->Initialize(frame.Get(), GUID_WICPixelFormat32bppBGR,
                                        WICBitmapDitherTypeNone, nullptr, 0,
                                        WICBitmapPaletteTypeCustom))) {
      BITMAPINFO info{};
      info.bmiHeader.biSize = sizeof info.bmiHeader;
      info.bmiHeader.biWidth = static_cast<LONG>(width);
      info.bmiHeader.biHeight = -static_cast<LONG>(high);
      info.bmiHeader.biPlanes = 1;
      info.bmiHeader.biBitCount = 32;
      info.bmiHeader.biCompression = BI_RGB;
      uint8_t* bits = nullptr;
      bitmap = CreateDIBSection(nullptr, &info, DIB_RGB_COLORS, reinterpret_cast<void**>(&bits),
                                nullptr, 0);
      if (bitmap && SUCCEEDED(converter->CopyPixels(nullptr, width * 4, width * 4 * high, bits))) {
        const uint8_t* pixel = bits + ((high / 2) * width + width - 1) * 4;
        *edge = RGB(pixel[2], pixel[1], pixel[0]);
        *size = {static_cast<LONG>(width), static_cast<LONG>(high)};
      } else if (bitmap) {
        DeleteObject(bitmap);
        bitmap = nullptr;
      }
    }
  }
  if (SUCCEEDED(com)) {
    CoUninitialize();
  }
  return bitmap;
}

LRESULT CALLBACK LaunchProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) {
  switch (message) {
    case WM_NCACTIVATE:
      // Its frame stands for the browser window's, which has the keyboard once it's under it.
      return DefWindowProcW(hwnd, message, TRUE, lparam);
    case WM_ERASEBKGND:
      return 1;
    case WM_PAINT: {
      PAINTSTRUCT paint;
      HDC dc = BeginPaint(hwnd, &paint);
      RECT client;
      GetClientRect(hwnd, &client);
      FillRect(dc, &client, g_launch.background);
      if (g_launch.picture) {
        HDC source = CreateCompatibleDC(dc);
        HGDIOBJ old = SelectObject(source, g_launch.picture);
        BitBlt(dc, 0, 0, g_launch.picture_size.cx, g_launch.picture_size.cy, source, 0, 0,
               SRCCOPY);
        SelectObject(source, old);
        DeleteDC(source);
      }
      EndPaint(hwnd, &paint);
      return 0;
    }
  }
  return DefWindowProcW(hwnd, message, wparam, lparam);
}

// The left |width| pixels of |window|'s content, top-down BGRX, filling |high|.
bool Sidebar(HWND window, LONG width, std::vector<uint8_t>* pixels, LONG* high) {
  RECT client{};
  GetClientRect(window, &client);
  width = width < client.right ? width : client.right;
  *high = client.bottom;
  if (width < 1 || *high < 1) {
    return false;
  }
  BITMAPINFO info{};
  info.bmiHeader.biSize = sizeof info.bmiHeader;
  info.bmiHeader.biWidth = client.right;
  info.bmiHeader.biHeight = -*high;
  info.bmiHeader.biPlanes = 1;
  info.bmiHeader.biBitCount = 32;
  info.bmiHeader.biCompression = BI_RGB;
  uint8_t* bits = nullptr;
  HDC screen = GetDC(nullptr);
  HDC dc = CreateCompatibleDC(screen);
  ReleaseDC(nullptr, screen);
  HBITMAP bitmap =
      CreateDIBSection(dc, &info, DIB_RGB_COLORS, reinterpret_cast<void**>(&bits), nullptr, 0);
  bool ok = false;
  if (bitmap) {
    HGDIOBJ old = SelectObject(dc, bitmap);
    // The full content, as GPUI draws through DirectComposition, which a plain copy of the
    // window's DC misses.
    ok = PrintWindow(window, dc, PW_CLIENTONLY | PW_RENDERFULLCONTENT);
    GdiFlush();
    if (ok) {
      pixels->resize(static_cast<size_t>(width) * *high * 4);
      for (LONG y = 0; y < *high; ++y) {
        memcpy(pixels->data() + static_cast<size_t>(y) * width * 4,
               bits + static_cast<size_t>(y) * client.right * 4, static_cast<size_t>(width) * 4);
      }
    }
    SelectObject(dc, old);
    DeleteObject(bitmap);
  }
  DeleteDC(dc);
  return ok;
}

// Writes |pixels|, |width| by |height| top-down BGRX, to |path| as a PNG. The caller has
// CoInitialized this thread.
void WritePng(const std::wstring& path, const std::vector<uint8_t>& pixels, UINT width,
              UINT height) {
  bool written = false;
  {
    ComPtr<IWICImagingFactory> wic;
    ComPtr<IWICBitmap> source;
    ComPtr<IWICStream> stream;
    ComPtr<IWICBitmapEncoder> encoder;
    ComPtr<IWICBitmapFrameEncode> frame;
    ComPtr<IPropertyBag2> options;
    WICPixelFormatGUID format = GUID_WICPixelFormat32bppBGR;
    written =
        SUCCEEDED(CoCreateInstance(CLSID_WICImagingFactory, nullptr, CLSCTX_INPROC_SERVER,
                                   IID_PPV_ARGS(&wic))) &&
        SUCCEEDED(wic->CreateBitmapFromMemory(width, height, GUID_WICPixelFormat32bppBGR,
                                              width * 4, static_cast<UINT>(pixels.size()),
                                              const_cast<BYTE*>(pixels.data()), &source)) &&
        SUCCEEDED(wic->CreateStream(&stream)) &&
        SUCCEEDED(stream->InitializeFromFilename(path.c_str(), GENERIC_WRITE)) &&
        SUCCEEDED(wic->CreateEncoder(GUID_ContainerFormatPng, nullptr, &encoder)) &&
        SUCCEEDED(encoder->Initialize(stream.Get(), WICBitmapEncoderNoCache)) &&
        SUCCEEDED(encoder->CreateNewFrame(&frame, &options)) &&
        SUCCEEDED(frame->Initialize(options.Get())) && SUCCEEDED(frame->SetSize(width, height)) &&
        SUCCEEDED(frame->SetPixelFormat(&format)) &&
        SUCCEEDED(frame->WriteSource(source.Get(), nullptr)) && SUCCEEDED(frame->Commit()) &&
        SUCCEEDED(encoder->Commit());
  }
  if (!written) {
    // Half a picture is worse than none: the next launch shows the colour alone. The stream that
    // held the file open is gone by now.
    DeleteFileW(path.c_str());
  }
}

}  // namespace

// For updates.cc, whose WinSparkle calls back on threads of its own.
void HostLater(void (*task)(void)) {
  Later(task);
}

namespace {

UINT_PTR const kFrame = 3;

// Whether the UI draws its buttons over the docked sidebar, by browser window
// (host_show_window_buttons).
std::map<HWND, bool>& WindowButtonsShown() {
  static std::map<HWND, bool> shown;
  return shown;
}

// How far down the top edge resizes, as thick as Windows' resize border on the other edges.
int TopEdge(HWND root) {
  const UINT dpi = GetDpiForWindow(root);
  return GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
}

// What the frame has at |at|, on the screen, over the UI: the top edge, which resizes, or the UI's
// maximize button, which Windows shows Snap layouts over. 0 where it's the UI's own.
int FrameHit(HWND root, POINT at) {
  // Full screen takes the caption style away, and the window has no frame then.
  if (!root || !(GetWindowLongPtrW(root, GWL_STYLE) & WS_CAPTION)) {
    return 0;
  }
  ScreenToClient(root, &at);
  RECT client{};
  GetClientRect(root, &client);
  const int edge = TopEdge(root);
  if (!IsZoomed(root) && at.y >= 0 && at.y < edge) {
    return at.x < 2 * edge                   ? HTTOPLEFT
           : at.x >= client.right - 2 * edge ? HTTOPRIGHT
                                             : HTTOP;
  }
  // Maximize, the second of shell.rs's WINDOW_BUTTONS in Windows' order (windows_buttons), 43
  // from the left and window-button-y (24) down, its plate windows-button-plate (22) across
  // (design/tokens.toml): kept in step with them.
  const double scale = Scale(root);
  auto shown = WindowButtonsShown().find(root);
  if (shown != WindowButtonsShown().end() && shown->second &&
      std::abs(at.x / scale - 43) <= 11 && std::abs(at.y / scale - 24) <= 11) {
    return HTMAXBUTTON;
  }
  return 0;
}

// GPUI's window in each browser window, which hears the pointer the frame answers for.
std::map<HWND, HWND>& FrameUi() {
  static std::map<HWND, HWND> ui;
  return ui;
}

// GPUI's window over the browser window steps aside where the frame answers.
LRESULT CALLBACK FrameChildProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam, UINT_PTR,
                                DWORD_PTR) {
  if (message == WM_NCHITTEST &&
      FrameHit(GetAncestor(hwnd, GA_ROOT), {GET_X_LPARAM(lparam), GET_Y_LPARAM(lparam)})) {
    return HTTRANSPARENT;
  }
  return DefSubclassProc(hwnd, message, wparam, lparam);
}

LRESULT CALLBACK FrameProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam, UINT_PTR,
                           DWORD_PTR) {
  switch (message) {
    case WM_NCCALCSIZE: {
      if (!wparam || !(GetWindowLongPtrW(hwnd, GWL_STYLE) & WS_CAPTION)) {
        break;
      }
      // Windows' sides and bottom, but no caption: the content starts at the window's top, or
      // where the screen does once maximized, which puts the frame past the screen's edges.
      auto* params = reinterpret_cast<NCCALCSIZE_PARAMS*>(lparam);
      const LONG top = params->rgrc[0].top;
      const LRESULT result = DefSubclassProc(hwnd, message, wparam, lparam);
      params->rgrc[0].top = top + (IsZoomed(hwnd) ? TopEdge(hwnd) : 0);
      return result;
    }
    case WM_NCHITTEST:
      if (const int hit = FrameHit(hwnd, {GET_X_LPARAM(lparam), GET_Y_LPARAM(lparam)})) {
        return hit;
      }
      break;
    // GPUI hears the pointer over the frame too, so the UI's maximize button shows its plate under
    // it, which the frame answers for, and the plate goes as the pointer leaves the window.
    case WM_NCMOUSEMOVE:
      if (auto ui = FrameUi().find(hwnd); ui != FrameUi().end()) {
        TRACKMOUSEEVENT leave{sizeof leave, TME_LEAVE | TME_NONCLIENT, hwnd, 0};
        TrackMouseEvent(&leave);
        SendMessageW(ui->second, WM_NCMOUSEMOVE, wparam, lparam);
      }
      break;
    case WM_NCMOUSELEAVE:
      if (auto ui = FrameUi().find(hwnd); ui != FrameUi().end()) {
        // Far off the window, where nothing of the UI's is under it.
        SendMessageW(ui->second, WM_NCMOUSEMOVE, HTNOWHERE, MAKELPARAM(-32000, -32000));
      }
      break;
    // Windows would draw its own maximize button at the press, so the press is the UI's to take:
    // the release maximizes or restores, as the UI's button does.
    case WM_NCLBUTTONDOWN:
    case WM_NCLBUTTONDBLCLK:
      if (wparam == HTMAXBUTTON) {
        return 0;
      }
      break;
    case WM_NCLBUTTONUP:
      if (wparam == HTMAXBUTTON) {
        PostMessageW(hwnd, WM_SYSCOMMAND, IsZoomed(hwnd) ? SC_RESTORE : SC_MAXIMIZE, 0);
        return 0;
      }
      break;
    case WM_ACTIVATEAPP:
      AppActivating(wparam, static_cast<DWORD>(lparam));
      break;
    case WM_NCDESTROY:
      WindowButtonsShown().erase(hwnd);
      FrameUi().erase(hwnd);
      break;
    // Windows tells every top-level window that "Show animations in Windows" changed.
    case WM_SETTINGCHANGE:
      if (wparam == SPI_SETCLIENTAREAANIMATION && g_reduce_motion_changed) {
        g_reduce_motion_changed();
      }
      break;
  }
  return DefSubclassProc(hwnd, message, wparam, lparam);
}

// Takes |ui_view|'s browser window's title bar away, once: the UI is in it by the time it shows.
void TakeTitleBar(void* ui_view) {
  HWND root = Root(ui_view);
  DWORD_PTR data = 0;
  if (!root || GetWindowSubclass(root, FrameProc, kFrame, &data)) {
    return;
  }
  SetWindowSubclass(root, FrameProc, kFrame, 0);
  if (HWND ui = GetWindow(Hwnd(ui_view), GW_CHILD)) {
    SetWindowSubclass(ui, FrameChildProc, kFrame, 0);
    FrameUi()[root] = ui;
  }
  // After the UI's update that showed the window: the content grows, and GPUI, still busy, would
  // miss the resize and keep its frame at the old height.
  Later([root] {
    if (IsWindow(root)) {
      SetWindowPos(root, nullptr, 0, 0, 0, 0,
                   SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
  });
}

// Windows' memory resource notifications, which it keeps signaled while the system is short of
// memory, and while it has plenty. The host waits for the low one on the thread pool, tells the
// UI, then waits for the high one before the low one again, so the UI hears once a spell, as the
// Mac's warning comes once.
struct MemoryWatch {
  HANDLE low = nullptr;
  HANDLE high = nullptr;
  HANDLE wait = nullptr;
  void (*pressed)(void) = nullptr;
};
MemoryWatch g_memory;

void WaitForMemory(bool low);

void CALLBACK MemoryChanged(PVOID low, BOOLEAN) {
  // On the pool's thread: the rest is the UI thread's.
  Later([low = low != nullptr] {
    if (low) {
      g_memory.pressed();
    }
    WaitForMemory(!low);
  });
}

void WaitForMemory(bool low) {
  if (g_memory.wait) {
    UnregisterWaitEx(g_memory.wait, nullptr);
    g_memory.wait = nullptr;
  }
  RegisterWaitForSingleObject(&g_memory.wait, low ? g_memory.low : g_memory.high, MemoryChanged,
                              low ? &g_memory : nullptr, INFINITE, WT_EXECUTEONLYONCE);
}

}  // namespace

extern "C" {

// Centered in the primary display's work area, in GPUI's logical pixels from its top-left.
void host_centered(double width, double height, double* x, double* y) {
  RECT work;
  SystemParametersInfoW(SPI_GETWORKAREA, 0, &work, 0);
  const double scale = GetDpiForSystem() / 96.0;
  *x = (work.left + work.right) / 2.0 / scale - width / 2;
  *y = (work.top + work.bottom) / 2.0 / scale - height / 2;
}

void host_show_launch_window(const char* profile_dir, double x, double y, double width,
                             double height) {
  static const wchar_t* launch_class = [] {
    WNDCLASSW window_class{};
    window_class.lpfnWndProc = LaunchProc;
    window_class.hInstance = GetModuleHandleW(nullptr);
    window_class.hCursor = LoadCursorW(nullptr, IDC_ARROW);
    // chrome.exe's own, IDR_MAINFRAME, which the browser window's frame shows too.
    window_class.hIcon = LoadIconW(GetModuleHandleW(nullptr), MAKEINTRESOURCEW(101));
    window_class.lpszClassName = L"BrowserLaunch";
    RegisterClassW(&window_class);
    return window_class.lpszClassName;
  }();
  const double scale = GetDpiForSystem() / 96.0;
  g_launch.owner = CreateWindowExW(0, launch_class, L"", WS_POPUP, 0, 0, 0, 0, nullptr, nullptr,
                                   GetModuleHandleW(nullptr), nullptr);
  // The browser window's frame, as Windows draws it. Clicks go through it, as they do through the
  // Mac's, and Chromium's occlusion tracker passes over a window they go through, so it can stay
  // over the browser window, whose pages still paint, until the UI has drawn under it.
  HWND window = CreateWindowExW(
      WS_EX_LAYERED | WS_EX_TRANSPARENT, launch_class, L"", WS_OVERLAPPEDWINDOW,
      static_cast<int>(std::lround(x * scale)), static_cast<int>(std::lround(y * scale)),
      static_cast<int>(std::lround(width * scale)), static_cast<int>(std::lround(height * scale)),
      g_launch.owner, nullptr, GetModuleHandleW(nullptr), nullptr);
  if (!window) {
    host_close_launch_window();
    return;
  }
  SetLayeredWindowAttributes(window, 0, 255, LWA_ALPHA);
  const bool light = SystemIsLight();
  const BOOL dark = !light;
  DwmSetWindowAttribute(window, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark, sizeof dark);
  RECT client{};
  GetClientRect(window, &client);
  // As the Mac's: Dia's light composite over grey 20, and the dark near its dark one.
  COLORREF color = light ? RGB(232, 232, 232) : RGB(38, 38, 38);
  g_launch.picture = LoadLaunchPicture(LaunchPicturePath(profile_dir, width, height), client.bottom,
                                       &g_launch.picture_size, &color);
  g_launch.background = CreateSolidBrush(color);
  g_launch.hwnd = window;
  if (g_launch.picture) {
    // check.sh reads this line.
    fprintf(stderr, "browser: launch picture shown\n");
  }
  ShowWindow(window, SW_SHOW);
  // On screen now, as Chromium's startup holds the main thread for a while before its loop.
  UpdateWindow(window);
}

void host_close_launch_window(void) {
  if (g_launch.hwnd) {
    DestroyWindow(g_launch.hwnd);
  }
  if (g_launch.owner) {
    DestroyWindow(g_launch.owner);
  }
  if (g_launch.picture) {
    DeleteObject(g_launch.picture);
  }
  if (g_launch.background) {
    DeleteObject(g_launch.background);
  }
  g_launch = {};
}

bool host_save_launch_picture(void* ui_view, const char* profile_dir, double width) {
  HWND root = Root(ui_view);
  double x = 0, y = 0, frame_width = 0, frame_height = 0;
  // In the frame's terms, which the next launch's window opens at, and named for its size.
  const bool framed = root && host_window_frame(ui_view, &x, &y, &frame_width, &frame_height);
  const std::wstring dir = Wide(profile_dir);
  WIN32_FIND_DATAW found;
  HANDLE find = FindFirstFileW((dir + L"\\launch-picture-*").c_str(), &found);
  if (find != INVALID_HANDLE_VALUE) {
    do {
      DeleteFileW((dir + L"\\" + found.cFileName).c_str());
    } while (FindNextFileW(find, &found));
    FindClose(find);
  }
  // Chromium's full screen takes the window's caption away.
  const bool full_screen = root && !(GetWindowLongPtrW(root, GWL_STYLE) & WS_CAPTION);
  if (!framed || !IsWindowVisible(root) || full_screen || width < 1) {
    return false;
  }
  // Only the sidebar: the page could be anything, and shouldn't be kept on disk.
  std::vector<uint8_t> pixels;
  LONG high = 0;
  const LONG wide = static_cast<LONG>(std::lround(width * Scale(root)));
  if (!Sidebar(root, wide, &pixels, &high)) {
    return false;
  }
  const UINT columns = static_cast<UINT>(pixels.size() / 4 / high);
  // Encoded off the main thread, which the user is about to use elsewhere or quit.
  std::thread([path = LaunchPicturePath(profile_dir, frame_width, frame_height),
               pixels = std::move(pixels), columns, high] {
    const HRESULT com = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    WritePng(path, pixels, columns, static_cast<UINT>(high));
    if (SUCCEEDED(com)) {
      CoUninitialize();
    }
  }).detach();
  return true;
}

// shortcut: links a launch carries come in on the command line, so there's nothing to wait for.
void host_watch_finished_launching(void (*finished)(void)) {
  finished();
}

// The work area of each monitor a window may open on, in host_centered's terms (logical pixels at
// the system scale, from the primary's top left), the primary first. Windows places the other
// monitors around it, so their coordinates may be negative.
size_t host_screens(double* frames, size_t count) {
  std::vector<RECT> works;
  EnumDisplayMonitors(nullptr, nullptr, CollectMonitor, reinterpret_cast<LPARAM>(&works));
  const double scale = GetDpiForSystem() / 96.0;
  for (size_t i = 0; i < works.size() && i < count; ++i) {
    frames[i * 4] = works[i].left / scale;
    frames[i * 4 + 1] = works[i].top / scale;
    frames[i * 4 + 2] = (works[i].right - works[i].left) / scale;
    frames[i * 4 + 3] = (works[i].bottom - works[i].top) / scale;
  }
  return works.size();
}

bool host_pointer(double* x, double* y) {
  POINT point{};
  if (!GetCursorPos(&point)) {
    return false;
  }
  const double scale = GetDpiForSystem() / 96.0;
  *x = point.x / scale;
  *y = point.y / scale;
  return true;
}

// The browser window's frame, in host_centered's terms, so it comes back where it was. Left be
// while it's maximized or minimized, as the Mac leaves a full-screen window's frame be: Windows
// brings those back to the size they had on its own.
bool host_window_frame(void* ui_view, double* x, double* y, double* width, double* height) {
  HWND root = Root(ui_view);
  RECT rect{};
  if (auto closed = ClosedFrames().find(Hwnd(ui_view)); !root && closed != ClosedFrames().end()) {
    rect = closed->second;
  } else if (!root || IsIconic(root) || IsZoomed(root)) {
    return false;
  } else {
    GetWindowRect(root, &rect);
  }
  const double scale = GetDpiForSystem() / 96.0;
  *x = rect.left / scale;
  *y = rect.top / scale;
  *width = (rect.right - rect.left) / scale;
  *height = (rect.bottom - rect.top) / scale;
  return true;
}

// Linux's alone: GPUI's loop runs inside Chromium's there. On Windows it runs on window messages.
void host_watch_fd(int, void (*)(void)) {}

void* host_ui_view(void) {
  static const wchar_t* holder_class = [] {
    WNDCLASSW window_class{};
    window_class.lpfnWndProc = HolderProc;
    window_class.hInstance = GetModuleHandleW(nullptr);
    window_class.hCursor = LoadCursorW(nullptr, IDC_ARROW);
    window_class.lpszClassName = L"BrowserUI";
    RegisterClassW(&window_class);
    // Drop targets need OLE on the thread, which Chromium has started already; this only counts.
    OleInitialize(nullptr);
    return window_class.lpszClassName;
  }();
  WatchForeground();
  WatchHeldEscape();
  // A window of its own until the core makes it a child of the browser window.
  HWND holder = CreateWindowExW(0, holder_class, L"", WS_POPUP | WS_CLIPCHILDREN, 0, 0, 0, 0,
                                nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
  UiViews().push_back(holder);
  MadeUiViews().push_back(holder);
  // GPUI's window in it registers no target of its own (patches/gpui-windows-embedded.patch), so
  // Windows hands the drags over it to the holder's.
  auto* target = new DropTarget(holder);
  if (SUCCEEDED(RegisterDragDrop(holder, target))) {
    DropTargets()[holder] = target;
  } else {
    target->Release();
  }
  return holder;
}

// The browser window has no title bar, as Arc's and Chrome's on Windows have none: the UI draws
// the window's buttons where the Mac's show (shell.rs's WINDOW_BUTTONS) and moves the window from
// its background (host_start_window_move), and Windows keeps the other edges, the shadow, Snap and
// Snap layouts, which it shows over the UI's maximize button as over its own.

void host_start_window_move(void* ui_view) {
  HWND root = Root(ui_view);
  HWND ui = GetWindow(Hwnd(ui_view), GW_CHILD);
  // After the move GPUI is handling: Windows' move loop runs until the button comes up.
  Later([root, ui] {
    if (!IsWindow(root)) {
      return;
    }
    ReleaseCapture();
    SendMessageW(root, WM_SYSCOMMAND, SC_MOVE | HTCAPTION, 0);
    // The loop took the release, which GPUI would otherwise still wait for.
    if (IsWindow(ui)) {
      POINT at{};
      GetCursorPos(&at);
      ScreenToClient(ui, &at);
      PostMessageW(ui, WM_LBUTTONUP, 0, MAKELPARAM(at.x, at.y));
    }
  });
}

bool host_window_maximized(void* ui_view) {
  HWND root = Root(ui_view);
  return root && IsZoomed(root);
}

void host_show_window(void* ui_view) {
  HWND root = Root(ui_view);
  TakeTitleBar(ui_view);
  ShowWindow(root, SW_SHOW);
  SetForegroundWindow(root);
  // The launch window stays over it until the UI has drawn (host_close_launch_window).
  if (g_launch.hwnd) {
    SetWindowPos(g_launch.hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
  }
}

// Shows the window right behind |above_ui_view|'s, neither key nor activating the app, as the
// windows a relaunch brings back after the first; in front when there's no window above it.
void host_show_window_behind(void* ui_view, void* above_ui_view) {
  HWND root = Root(ui_view);
  if (!root) {
    return;
  }
  TakeTitleBar(ui_view);
  HWND above = Root(above_ui_view);
  if (!above) {
    host_show_window(ui_view);
    return;
  }
  SetWindowPos(root, above, 0, 0, 0, 0,
               SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW);
}

void host_cascaded(void* ui_view, double* x, double* y, double* width, double* height) {
  HWND root = Root(ui_view);
  RECT rect{};
  GetWindowRect(root, &rect);
  const double scale = Scale(root);
  const double step = GetSystemMetrics(SM_CYCAPTION) / scale;
  *x = rect.left / scale + step;
  *y = rect.top / scale + step;
  *width = (rect.right - rect.left) / scale;
  *height = (rect.bottom - rect.top) / scale;
}

void host_set_window_title(void* ui_view, const char* title) {
  SetWindowTextW(Root(ui_view), Wide(title).c_str());
}

// The window has no material under the UI, whose tint is the window's look.
void host_set_window_light(void* ui_view, bool light) {}

// The UI takes the choice itself; the system's controls and the pages keep Windows'.
void host_set_app_appearance(int appearance) {}

void host_activate(void* ui_view) {
  HWND root = Root(ui_view);
  if (IsIconic(root)) {
    ShowWindow(root, SW_RESTORE);
  }
  SetForegroundWindow(root);
}

void host_watch_key_window(void (*changed)(void* ui_view)) {
  g_key_window_changed = changed;
  WatchForeground();
}

// The window is Chromium's, which goes full screen through the core (bc_window_toggle_full_screen),
// and the app hears of it as the window's bounds change, as on Linux.
void host_watch_full_screen(void (*)(void*, bool)) {}

static std::map<void*, uint64_t>& WindowIds() {
  static std::map<void*, uint64_t> ids;
  return ids;
}

void host_window_opened(void* ui_view, uint64_t window) {
  WindowIds()[ui_view] = window;
}

// Windows delivers a touchpad's swipes to the page as scrolls, which it takes, so there's nothing
// here for the host to watch.
void host_watch_swipes(bool (*)(void*, double, double, bool)) {}
void host_watch_option_taps(void (*)(void*)) {}

// A window's menu bar isn't replaced while it's open on Windows: it's modal and runs no tasks.
bool host_after_menus(void (*)(void)) { return false; }

void host_watch_resigning_active(void (*resigning)(void)) {
  g_resigning_active = resigning;
  WatchForeground();
}

void host_watch_active(void (*became_active)(void)) {
  g_became_active = became_active;
  WatchForeground();
}

// The ProgId the installer registers for ilo (ilo.iss's [Registry], AppxManifest's protocol), which
// the user's http/https choice names when ilo is the default. It must stay in step with those.
constexpr wchar_t kProgId[] = L"IloHTML";

// Whether the user's choice for |scheme| links is ilo's ProgId. Windows protects this key against
// anything but the user's own pick in Settings, so the browser only reads it (host_make_default_
// browser opens that page), never writes it.
bool ChoiceIsOurs(const wchar_t* scheme) {
  std::wstring path =
      L"Software\\Microsoft\\Windows\\Shell\\Associations\\UrlAssociations\\";
  path += scheme;
  path += L"\\UserChoice";
  HKEY key = nullptr;
  if (RegOpenKeyExW(HKEY_CURRENT_USER, path.c_str(), 0, KEY_QUERY_VALUE, &key) != ERROR_SUCCESS) {
    return false;
  }
  wchar_t progid[128] = {};
  // One short of the buffer, so the value stays terminated even if the stored one isn't.
  DWORD size = sizeof progid - sizeof(wchar_t);
  DWORD type = 0;
  const LONG read =
      RegQueryValueExW(key, L"ProgId", nullptr, &type, reinterpret_cast<BYTE*>(progid), &size);
  RegCloseKey(key);
  return read == ERROR_SUCCESS && type == REG_SZ && lstrcmpiW(progid, kProgId) == 0;
}

bool host_reduce_motion(void) {
  BOOL animations = TRUE;
  return SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &animations, 0) && !animations;
}

// FrameProc calls it as the browser windows hear of the change.
void host_watch_reduce_motion(void (*changed)(void)) {
  g_reduce_motion_changed = changed;
}

bool host_is_default_browser(void) {
  return ChoiceIsOurs(L"http") && ChoiceIsOurs(L"https");
}

void host_make_default_browser(void (*answered)(void)) {
  // Windows lets only the user choose, in Settings, on ilo's own page there: RegisteredApplications'
  // name in ilo.iss. An older Windows that doesn't know the parameter shows the whole list.
  ShellExecuteW(nullptr, L"open", L"ms-settings:defaultapps?registeredAppUser=ilo", nullptr,
                nullptr, SW_SHOWNORMAL);
  Later([answered] { answered(); });
}

double host_since_launch(void) {
  FILETIME created, exited, kernel, user, now;
  GetProcessTimes(GetCurrentProcess(), &created, &exited, &kernel, &user);
  GetSystemTimePreciseAsFileTime(&now);
  auto ticks = [](FILETIME time) {
    return (static_cast<ULONGLONG>(time.dwHighDateTime) << 32) | time.dwLowDateTime;
  };
  return (ticks(now) - ticks(created)) / 1e4;
}

void host_forget_window(void* ui_view) {
  Disown(Root(ui_view));
  FocusAfterOverlay().erase(Root(ui_view));
  g_find_bars.erase(ui_view);
  g_offers.erase(ui_view);
  g_reveals.erase(ui_view);
  g_pane_corners.erase(ui_view);
  std::erase(UiViews(), Hwnd(ui_view));
  ClosedFrames().erase(Hwnd(ui_view));
}

void host_add_overlay(int overlay, void* overlay_view, double, void (*dismissed)(void)) {
  Adopt(g_overlays[overlay], overlay_view, false, dismissed);
  // The command bar is a rounded rectangle, as Windows' frame is. The pickers and the folder
  // preview have a beak, whose shape a rectangle's corners and shadow would show round.
  if (overlay == 0) {
    SystemFrame(g_overlays[overlay].hwnd);
  }
}

// An overlay is a top-level window its browser window owns (host_add_overlay, through Adopt), so
// Windows 11 rounds its corners (DWMWCP_ROUND) and draws its drop shadow on its own. It gives one
// rounded-corner size and one shadow, not the Mac's free radius, per-corner shape or elevation
// levels, and the overlay doesn't grow or shrink as it shows or hides; these carry no Windows work.
void host_set_overlay_shape(int, const double*, size_t) {}
void host_set_overlay_radius(int, double) {}
void host_set_overlay_elevation(int, float, double, float, double, double) {}
void host_set_overlay_growth(int, double, double) {}
void host_set_overlay_shrink(int) {}

void host_show_overlay(int overlay,
                       void* ui_view,
                       bool centered,
                       double left,
                       double top,
                       double width) {
  for (int each = 0; each < int(std::size(g_overlays)); ++each) {
    Floating& other = g_overlays[each];
    if (each != overlay && HideOverlay(each)) {
      if (other.dismissed) {
        void (*dismissed)(void) = other.dismissed;
        Later([dismissed] { dismissed(); });
      }
    }
  }
  double content_width, content_height;
  ContentSize(Root(ui_view), &content_width, &content_height);
  if (centered) {
    left = (content_width - width) / 2;
    top = content_height / 2 - top;
  } else if (left + width > content_width - 8) {
    left = content_width - 8 - width;
  }
  Floating& floating = g_overlays[overlay];
  const int shows = ++floating.shows;
  // The window still holds what it drew when it last closed, such as the previous query, as the
  // Mac's does. It goes on screen once the UI has drawn it as it is now, which can't happen while
  // the UI is still handling the shortcut that opened it.
  Later([=, &floating] {
    if (shows != floating.shows || !floating.hwnd) {
      ReleaseKeys(floating.hwnd);
      return;
    }
    const bool was_shown = IsWindowVisible(floating.hwnd);
    if (!was_shown) {
      if (HWND root = Root(ui_view)) {
        // Placed and drawn before it's on screen, as the menu is: Windows sends a hidden window no
        // WM_PAINT, but GPUI's window draws on any it's sent.
        const double scale = Scale(root);
        POINT origin{std::lround(left * scale), std::lround(top * scale)};
        ClientToScreen(root, &origin);
        double own_width, own_height;
        WindowSize(floating.hwnd, &own_width, &own_height);
        SetWindowPos(floating.hwnd, nullptr, origin.x, origin.y,
                     static_cast<int>(std::lround(width * scale)),
                     static_cast<int>(std::lround(own_height * scale)),
                     SWP_NOACTIVATE | SWP_NOZORDER);
        SendMessageW(floating.hwnd, WM_PAINT, 0, 0);
      }
    }
    Show(floating, ui_view, left, top, width, -1, true);
    // The bar now has the keyboard, so the keys held since the shortcut go to it.
    ReleaseKeys(floating.hwnd);
    if (!IsWindowVisible(floating.hwnd)) {
      return;
    }
    double own_width, own_height;
    WindowSize(floating.hwnd, &own_width, &own_height);
    // check.sh reads these lines, in whole points, since at a fractional scale not every point is
    // a whole number of pixels.
    fprintf(stderr, "browser: %s at %g %g %g %g\n", kOverlayNames[overlay], left, top, width,
            std::round(own_height));
    if (!was_shown) {
      fprintf(stderr, "browser: %s shown\n", kOverlayNames[overlay]);
    }
  });
}

void host_set_overlay_height(int overlay, double height) {
  Floating& floating = g_overlays[overlay];
  if (!floating.hwnd) {
    return;
  }
  // In pixels, as Resize sets it.
  RECT rect{};
  GetWindowRect(floating.hwnd, &rect);
  if (rect.bottom - rect.top == static_cast<int>(std::lround(height * Scale(floating.hwnd)))) {
    return;
  }
  Resize(floating, height);
  // check.sh reads this line.
  fprintf(stderr, "browser: %s height %g\n", kOverlayNames[overlay], height);
}

// host_can_draw_emoji is the app's, from GPUI's text system (platform/linux/emoji.rs).

void host_hide_overlay(int overlay) {
  HideOverlay(overlay);
}

void host_dismiss_overlay(int overlay) {
  // Posted: the UI may be handling the event that closed the overlay, as Esc's action is, and
  // would borrow the app again.
  if (HideOverlay(overlay) && g_overlays[overlay].dismissed) {
    void (*dismissed)(void) = g_overlays[overlay].dismissed;
    Later([dismissed] { dismissed(); });
  }
}

void host_hold_keys(void) {
  HoldKeys(nullptr);
}

void host_release_held_keys(void) {
  // Unless a bar opened since, which holds them for itself.
  if (g_holding_keys && !g_keys_for) {
    ReleaseKeys(nullptr);
  }
}

void host_add_find_bar(void* ui_view, void* find_bar_view, double right, double top) {
  Corner& corner = g_find_bars[ui_view];
  Adopt(corner.floating, find_bar_view, false, nullptr);
  SystemFrame(corner.floating.hwnd);
  corner.right = right;
  corner.top = top;
}

void host_show_find_bar(void* ui_view, bool keyboard) {
  if (auto it = g_find_bars.find(ui_view); it != g_find_bars.end()) {
    HWND hwnd = it->second.floating.hwnd;
    const bool was_shown = hwnd && IsWindowVisible(hwnd);
    ShowCorner(it->second, ui_view, keyboard);
    if (keyboard) {
      // It has the keyboard, so the keys held since the shortcut go to it.
      ReleaseKeys(hwnd);
    }
    // check.sh reads this line, as the Mac's and Linux's hosts log it.
    if (!was_shown && hwnd && IsWindowVisible(hwnd)) {
      fprintf(stderr, "browser: find bar shown\n");
    }
  }
}

void host_hide_find_bar(void* ui_view) {
  if (auto it = g_find_bars.find(ui_view); it != g_find_bars.end()) {
    HWND hwnd = it->second.floating.hwnd;
    if (hwnd && IsWindowVisible(hwnd)) {
      Hide(it->second.floating);
      fprintf(stderr, "browser: find bar hidden\n");
    }
  }
}

void host_add_offer(void* ui_view, void* offer_view, double right, double top) {
  Corner& corner = g_offers[ui_view];
  Adopt(corner.floating, offer_view, false, nullptr);
  SystemFrame(corner.floating.hwnd);
  corner.right = right;
  corner.top = top;
}

void host_show_offer(void* ui_view) {
  if (auto it = g_offers.find(ui_view); it != g_offers.end()) {
    ShowCorner(it->second, ui_view, false);
    // check.sh reads this line, as the Mac's and Linux's hosts write it once the card is up.
    if (IsWindowVisible(it->second.floating.hwnd)) {
      fprintf(stderr, "browser: offer shown\n");
    }
  }
}

void host_hide_offer(void* ui_view) {
  if (auto it = g_offers.find(ui_view); it != g_offers.end()) {
    const bool shown = IsWindowVisible(it->second.floating.hwnd);
    Hide(it->second.floating);
    // check.sh reads this line, as the Mac's and Linux's hosts write it.
    if (shown) {
      fprintf(stderr, "browser: offer hidden\n");
    }
  }
}

void host_set_pane_corner(void* ui_view, double right, double top) {
  g_pane_corners[ui_view] = {right, top};
}

void host_add_hud(void* hud_view, double right, double top, double) {
  Adopt(g_hud, hud_view, true, nullptr);
  g_hud_right = right;
  g_hud_top = top;
}

void host_show_hud(void* ui_view) {
  double content_width, content_height, width, height;
  ContentSize(Root(ui_view), &content_width, &content_height);
  WindowSize(g_hud.hwnd, &width, &height);
  auto [extra_right, extra_top] = g_pane_corners[ui_view];
  Show(g_hud, ui_view, content_width - g_hud_right - extra_right - width, g_hud_top + extra_top, -1,
       -1, false);
}

void host_hide_hud(void) {
  Hide(g_hud);
}

void host_add_drop_targets(void* view) {
  Adopt(g_drop_targets, view, true, nullptr);
}

void host_show_drop_targets(void* ui_view) {
  double width, height;
  ContentSize(Root(ui_view), &width, &height);
  Show(g_drop_targets, ui_view, 0, 0, width, height, false);
}

void host_hide_drop_targets(void) {
  Hide(g_drop_targets);
}

void host_add_dialog(void* dialog_view) {
  Adopt(g_dialog, dialog_view, false, nullptr);
}

void host_show_dialog(void* ui_view, double x, double y, double width, double height) {
  // Without a rect, it covers all of the window.
  if (width <= 0) {
    x = y = 0;
    ContentSize(Root(ui_view), &width, &height);
  }
  Show(g_dialog, ui_view, x, y, width, height, true);
}

void host_hide_dialog(void) {
  Hide(g_dialog);
}

void host_add_site_controls(void* view, void (*dismissed)(void)) {
  // Its beak is in the window, whose rectangle Windows' corners and shadow would show round it.
  Adopt(g_site_controls, view, false, dismissed);
}

void host_set_site_controls_shape(const double*, size_t) {}

void host_show_site_controls(void* ui_view, double left, double top, double width, double height) {
  Show(g_site_controls, ui_view, left, top, width, height, true);
  // check.sh reads these lines, as the Mac's and Linux's hosts write them.
  fprintf(stderr, "browser: site controls at %g %g %g %g\n", left, top, width, height);
  fprintf(stderr, "browser: site controls shown\n");
}

void host_set_site_controls_height(double height) {
  if (!g_site_controls.hwnd || !IsWindowVisible(g_site_controls.hwnd)) {
    return;
  }
  RECT rect{};
  GetWindowRect(g_site_controls.hwnd, &rect);
  if (rect.bottom - rect.top ==
      static_cast<int>(std::lround(height * Scale(g_site_controls.hwnd)))) {
    return;
  }
  Resize(g_site_controls, height);
  fprintf(stderr, "browser: site controls height %g\n", height);
}

void host_site_controls_drew(double, double, double, double) {}

void host_release_site_controls(void) {
  if (!g_site_controls.hwnd || !IsWindowVisible(g_site_controls.hwnd) ||
      g_site_controls.mouse_through) {
    return;
  }
  g_site_controls.mouse_through = true;
  if (GetForegroundWindow() == g_site_controls.hwnd) {
    g_site_controls.hiding = true;
    SetForegroundWindow(GetWindow(g_site_controls.hwnd, GW_OWNER));
    g_site_controls.hiding = false;
  }
  fprintf(stderr, "browser: site controls released\n");
}

void host_hide_site_controls(void) {
  const bool shown = g_site_controls.hwnd && IsWindowVisible(g_site_controls.hwnd);
  Hide(g_site_controls);
  g_site_controls.mouse_through = false;
  if (shown) {
    // check.sh reads this line, as on the Mac and Linux, and escape's wait ended without it (#872).
    fprintf(stderr, "browser: site controls hidden\n");
  }
}

void* host_add_link_preview(void* view, void (*dismissed)(void)) {
  // It never has the keyboard, which stays with the page.
  Adopt(g_link_preview.floating, view, false, nullptr);
  g_link_preview.dismissed = dismissed;
  SetWindowLongPtrW(g_link_preview.floating.hwnd, GWL_EXSTYLE,
                    GetWindowLongPtrW(g_link_preview.floating.hwnd, GWL_EXSTYLE) |
                        WS_EX_NOACTIVATE);
  // The core shows the page in the browser window, under this one, and only needs to know there's
  // a view it goes with.
  return view;
}

void host_show_link_preview(void* ui_view) {
  HWND root = Root(ui_view);
  if (!g_link_preview.floating.hwnd || !root) {
    return;
  }
  if (g_link_preview.root && g_link_preview.root != root) {
    host_hide_link_preview();
  }
  if (g_link_preview.root != root) {
    SetWindowSubclass(root, LinkPreviewOwnerProc, kLinkPreviewOwner, 0);
  }
  g_link_preview.root = root;
  // All of it until its first frame says where the page is.
  g_link_preview.hole.reset();
  g_link_preview.floating.mouse_through = false;
  Later([root] {
    if (g_link_preview.root == root) {
      PlaceLinkPreview();
      // check.sh reads this line.
      fprintf(stderr, "browser: link preview shown\n");
    }
  });
}

void host_link_preview_drew(double left,
                            double top,
                            double width,
                            double height,
                            double radius,
                            double) {
  if (!g_link_preview.root) {
    return;
  }
  std::optional<LinkPreview::Hole> hole;
  if (width >= 1 && height >= 1) {
    const double scale = Scale(g_link_preview.root);
    radius = (std::min)({radius, width / 2, height / 2});
    hole = LinkPreview::Hole{
        RECT{static_cast<LONG>(std::lround(left * scale)),
             static_cast<LONG>(std::lround(top * scale)),
             static_cast<LONG>(std::lround((left + width) * scale)),
             static_cast<LONG>(std::lround((top + height) * scale))},
        static_cast<int>(std::lround(2 * radius * scale))};
  }
  if (!(hole == g_link_preview.hole)) {
    g_link_preview.hole = hole;
    ShapeLinkPreview();
  }
}

void host_release_link_preview(void) {
  if (!g_link_preview.root || g_link_preview.floating.mouse_through) {
    return;
  }
  // The app gives the window's page the keyboard back, through the core (LinkPreviewHost).
  g_link_preview.floating.mouse_through = true;
  // check.sh reads this line.
  fprintf(stderr, "browser: link preview released\n");
}

void host_hide_link_preview(void) {
  host_release_link_preview();
  if (!g_link_preview.root) {
    return;
  }
  RemoveWindowSubclass(g_link_preview.root, LinkPreviewOwnerProc, kLinkPreviewOwner);
  g_link_preview.root = nullptr;
  g_link_preview.hole.reset();
  Hide(g_link_preview.floating);
  g_link_preview.floating.mouse_through = false;
  // check.sh reads this line.
  fprintf(stderr, "browser: link preview hidden\n");
}

void host_add_capture(void* view, void (*dismissed)(void)) {
  Adopt(g_capture, view, false, dismissed);
}

void host_show_capture(void* ui_view, double left, double top, double width, double height) {
  Show(g_capture, ui_view, left, top, width, height, true);
  if (g_capture.hwnd && IsWindowVisible(g_capture.hwnd)) {
    // check.sh reads this line, as the Mac's and Linux's hosts log it.
    fprintf(stderr, "browser: capture shown at %g %g %g %g\n", left, top, width, height);
  }
}

void host_capture_camera(bool camera) {
  SetCursor(LoadCursorW(nullptr, camera ? IDC_CROSS : IDC_ARROW));
}

void host_hide_capture(void) {
  if (!g_capture.hwnd || !IsWindowVisible(g_capture.hwnd)) {
    return;
  }
  Hide(g_capture);
  // check.sh reads this line.
  fprintf(stderr, "browser: capture hidden\n");
}

void host_copy_png(const uint8_t* png, size_t size) {
  static const UINT png_format = RegisterClipboardFormatW(L"PNG");
  if (!OpenClipboard(nullptr)) {
    return;
  }
  EmptyClipboard();
  if (HGLOBAL memory = GlobalAlloc(GMEM_MOVEABLE, size)) {
    memcpy(GlobalLock(memory), png, size);
    GlobalUnlock(memory);
    if (SetClipboardData(png_format, memory)) {
      // check.sh reads this line, as Linux's host logs it.
      fprintf(stderr, "browser: clipboard png %zu bytes\n", size);
    } else {
      GlobalFree(memory);
    }
  }
  CloseClipboard();
}

void host_capture_name(char* name, size_t capacity) {
  SYSTEMTIME now;
  GetLocalTime(&now);
  if (capacity && snprintf(name, capacity, "Capture %04d-%02d-%02d at %02d.%02d.%02d.png",
                           now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute,
                           now.wSecond) < 0) {
    name[0] = 0;
  }
}

void host_save_png(void* ui_view, const uint8_t* png, size_t size) {
  wchar_t path[MAX_PATH];
  SYSTEMTIME now;
  GetLocalTime(&now);
  swprintf_s(path, L"Capture %04d-%02d-%02d at %02d.%02d.%02d.png", now.wYear, now.wMonth,
             now.wDay, now.wHour, now.wMinute, now.wSecond);
  OPENFILENAMEW save{};
  save.lStructSize = sizeof save;
  save.hwndOwner = Root(ui_view);
  save.lpstrFilter = L"PNG image\0*.png\0";
  save.lpstrFile = path;
  save.nMaxFile = MAX_PATH;
  save.lpstrDefExt = L"png";
  save.Flags = OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST;
  if (!GetSaveFileNameW(&save)) {
    return;
  }
  HANDLE file = CreateFileW(path, GENERIC_WRITE, 0, nullptr, CREATE_ALWAYS, 0, nullptr);
  if (file != INVALID_HANDLE_VALUE) {
    DWORD written;
    WriteFile(file, png, static_cast<DWORD>(size), &written, nullptr);
    CloseHandle(file);
  }
}

// Shares the screenshot through the system share flyout as an image. Windows has no Messages app,
// so |messages| and the on-screen rectangle don't apply.
void host_share_png(const uint8_t* png,
                    size_t size,
                    void* view,
                    double,
                    double,
                    double,
                    double,
                    bool) {
  HWND root = Root(view);
  if (!root || !png || size == 0) {
    return;
  }
  wss::InMemoryRandomAccessStream stream;
  wss::DataWriter writer(stream);
  writer.WriteBytes(winrt::array_view<const uint8_t>(png, png + size));
  try {
    // The store is a memory copy that finishes off the UI thread, so this waits only a moment.
    writer.StoreAsync().get();
    writer.DetachStream();
  } catch (const winrt::hresult_error& error) {
    fwprintf(stderr, L"browser: couldn't hold the shared image: %ls\n", error.message().c_str());
    return;
  }
  stream.Seek(0);
  auto reference = wss::RandomAccessStreamReference::CreateFromStream(stream);
  // Named for when it was taken, as the Mac names it.
  wchar_t name[64];
  SYSTEMTIME now;
  GetLocalTime(&now);
  wsprintfW(name, L"Screenshot %04u-%02u-%02u %02u.%02u.%02u.png", now.wYear, now.wMonth, now.wDay,
            now.wHour, now.wMinute, now.wSecond);
  winrt::hstring title{name};
  ShowShareSheet(root, [reference, title](const wdt::DataPackage& data) {
    data.Properties().Title(title);
    data.SetBitmap(reference);
  });
}

void host_add_toast(void* toast_view) {
  Adopt(g_toast, toast_view, true, nullptr);
}

void host_toast_takes_clicks(bool takes) {
  g_toast.mouse_through = !takes;
}

// shortcut: while it takes clicks, its whole window does, the margin round the plate included.
void host_set_toast_plate(double, double, double, double) {}

void host_show_toast(void* ui_view) {
  double content_width, content_height, width, height;
  ContentSize(Root(ui_view), &content_width, &content_height);
  WindowSize(g_toast.hwnd, &width, &height);
  // At the focused page's corner, which in a split view is its pane's.
  auto [extra_right, extra_top] = g_pane_corners[ui_view];
  Show(g_toast, ui_view, content_width - extra_right - width, extra_top, -1, -1, false);
}

void host_hide_toast(void) {
  Hide(g_toast);
}

void host_add_tooltip(void* tooltip_view, double margin, void (*)(void)) {
  Adopt(g_tooltip, tooltip_view, true, nullptr);
  g_tooltip_margin = margin;
}

void host_show_tooltip(void* ui_view, double left, double top, double fade) {
  FadeIn(g_tooltip, ui_view, left - g_tooltip_margin, top - g_tooltip_margin, fade, "tooltip");
}

void host_hide_tooltip(double fade) {
  FadeOut(g_tooltip, fade, [] {});
}

// shortcut: the card ends only as the pointer leaves it or the UI ends it, as a key does.
void host_add_hover_card(void* card_view, double margin, void (*)(void)) {
  Adopt(g_card, card_view, false, nullptr);
  // A click on its buttons leaves the keyboard where it was.
  SetWindowLongPtrW(g_card.hwnd, GWL_EXSTYLE,
                    GetWindowLongPtrW(g_card.hwnd, GWL_EXSTYLE) | WS_EX_NOACTIVATE);
  g_card_margin = margin;
}

void host_show_hover_card(void* ui_view, double left, double top, double width, double height) {
  if (HWND root = Root(ui_view)) {
    const double scale = Scale(root);
    g_card.plate = RECT{std::lround(g_card_margin * scale), std::lround(g_card_margin * scale),
                        std::lround((g_card_margin + width) * scale),
                        std::lround((g_card_margin + height) * scale)};
  }
  Show(g_card, ui_view, left - g_card_margin, top - g_card_margin, -1, -1, false);
  HWND root = Root(ui_view);
  RECT frame;
  if (root && g_card.hwnd && IsWindowVisible(g_card.hwnd) && GetWindowRect(g_card.hwnd, &frame)) {
    // Where the window really is, in the browser window's points, for check.sh to compare with
    // where the UI asked, as the Mac's and Linux's hosts log it.
    POINT at{frame.left, frame.top};
    ScreenToClient(root, &at);
    const double scale = Scale(root);
    fprintf(stderr, "browser: hover card window at %g %g\n", at.x / scale + g_card_margin,
            at.y / scale + g_card_margin);
  }
  LogShown(g_card, "hover card");
}

void host_hide_hover_card(void) {
  Hide(g_card);
}

void host_watch_memory_pressure(void (*pressed)(void)) {
  if (g_memory.pressed) {
    return;
  }
  g_memory.low = CreateMemoryResourceNotification(LowMemoryResourceNotification);
  g_memory.high = CreateMemoryResourceNotification(HighMemoryResourceNotification);
  if (!g_memory.low || !g_memory.high) {
    return;
  }
  g_memory.pressed = pressed;
  // Later's window is made here, on the UI thread, before the pool's thread posts to it.
  Later([] {});
  WaitForMemory(true);
}

void host_add_link_target(void* link_view, double margin, void (*pointer)(double x, double y)) {
  Adopt(g_link_target, link_view, true, nullptr);
  g_link_margin = margin;
  g_link_pointer = pointer;
}

void host_show_link_target(void* ui_view, double left, double top, double fade) {
  g_link_shown_over = ui_view;
  FadeIn(g_link_target, ui_view, left - g_link_margin, top - g_link_margin, fade, "link target");
  WatchPointer();
}

void host_hide_link_target(double fade) {
  FadeOut(g_link_target, fade, MaybeStopWatchingPointer);
}

void host_focus(void* view) {
  HWND hwnd = Hwnd(view);
  HWND root = GetAncestor(hwnd, GA_ROOT);
  // An overlay over the window, such as the command bar a Space shown under it follows, keeps the
  // keyboard: SetFocus would make the window the active one.
  HWND front = GetForegroundWindow();
  if (front != root && FloatingByHwnd().contains(front) && GetWindow(front, GW_OWNER) == root) {
    FocusAfterOverlay()[root] = hwnd;
    return;
  }
  FocusAfterOverlay().erase(root);
  if (front != root) {
    SetForegroundWindow(root);
  }
  SetFocus(hwnd);
}

void host_add_menu_window(void* view,
                          bool (*pointer)(int kind, double x, double y),
                          void (*dismissed)(void),
                          bool (*choose)(const char* title)) {
  g_menu.hwnd = Hwnd(view);
  g_menu.pointer = pointer;
  g_menu.dismissed = dismissed;
  g_menu.choose = choose;
  ShowWindow(g_menu.hwnd, SW_HIDE);
  OffTaskbar(g_menu.hwnd);
  // All of it is the UI's, as an overlay's is (FloatingProc).
  SetWindowSubclass(
      g_menu.hwnd,
      [](HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam, UINT_PTR, DWORD_PTR) -> LRESULT {
        return message == WM_NCCALCSIZE && wparam ? 0
                                                  : DefSubclassProc(hwnd, message, wparam, lparam);
      },
      1, 0);
  SetWindowPos(g_menu.hwnd, nullptr, 0, 0, 0, 0,
               SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
  // The UI draws the panel's shape and shadow, inside the window.
  OwnShadow(g_menu.hwnd);
  WatchForeground();
}

void host_show_menu_window(void* ui_view,
                           bool in_reveal,
                           double left,
                           double top,
                           double width,
                           double height) {
  HWND base = MenuBase(ui_view, in_reveal);
  HWND root = Root(ui_view);
  if (!g_menu.hwnd || !base || !root) {
    return;
  }
  if (!g_menu.shown) {
    HWND key = GetForegroundWindow();
    g_menu.had_keyboard = key == g_menu.hwnd ? nullptr : key;
  }
  g_menu.shown = true;
  g_menu.give_back = true;
  g_menu.parent = base;
  g_menu.browser = root;
  const double scale = Scale(base);
  POINT origin{static_cast<LONG>(left * scale), static_cast<LONG>(top * scale)};
  ClientToScreen(base, &origin);
  const RECT frame{origin.x, origin.y, origin.x + static_cast<LONG>(width * scale),
                   origin.y + static_cast<LONG>(height * scale)};
  g_menu.frame = frame;
  const unsigned showing = ++g_menu.showing;
  // It goes on screen after the event, so the press that opened it is over.
  Later([=] {
    if (showing != g_menu.showing) {
      return;
    }
    SetWindowLongPtrW(g_menu.hwnd, GWLP_HWNDPARENT, reinterpret_cast<LONG_PTR>(base));
    WatchOwner(root);
    const int wide = frame.right - frame.left, high = frame.bottom - frame.top;
    // Placed and drawn before it's on screen, as the Mac's host draws it, so its first frame is
    // this menu and not the last: Windows sends a hidden window no WM_PAINT, but GPUI's window
    // draws on any it's sent, shown or not.
    SetWindowPos(g_menu.hwnd, nullptr, frame.left, frame.top, wide, high,
                 SWP_NOACTIVATE | SWP_NOZORDER);
    SendMessageW(g_menu.hwnd, WM_PAINT, 0, 0);
    PlaceAndShow(g_menu.hwnd, frame.left, frame.top, wide, high, true);
    SetForegroundWindow(g_menu.hwnd);
    SetFocus(g_menu.hwnd);
    InstallMenuHooks();
    // check.sh reads this line.
    fprintf(stderr, "browser: menu window at %.0f,%.0f %.0fx%.0f\n", left, top, width, height);
  });
}

void host_hide_menu_window(void) {
  if (!g_menu.shown) {
    return;
  }
  g_menu.shown = false;
  ++g_menu.showing;
  RemoveMenuHooks();
  ShowWindow(g_menu.hwnd, SW_HIDE);
  HWND back = g_menu.had_keyboard && IsWindowVisible(g_menu.had_keyboard) ? g_menu.had_keyboard
                                                                         : g_menu.browser;
  g_menu.had_keyboard = nullptr;
  if (g_menu.give_back && g_active && back && IsWindowVisible(back)) {
    SetForegroundWindow(back);
  }
  fprintf(stderr, "browser: menu window hidden\n");
}

void host_menu_area(void* ui_view, bool in_reveal, double* area) {
  HWND base = MenuBase(ui_view, in_reveal);
  if (!base) {
    area[0] = area[1] = area[2] = area[3] = 0;
    return;
  }
  MONITORINFO info{sizeof info};
  GetMonitorInfoW(MonitorFromWindow(base, MONITOR_DEFAULTTONEAREST), &info);
  const double scale = Scale(base);
  POINT top_left{info.rcWork.left, info.rcWork.top};
  POINT bottom_right{info.rcWork.right, info.rcWork.bottom};
  ScreenToClient(base, &top_left);
  ScreenToClient(base, &bottom_right);
  area[0] = top_left.x / scale;
  area[1] = top_left.y / scale;
  area[2] = bottom_right.x / scale;
  area[3] = bottom_right.y / scale;
}

// For the driver: chooses |title|, or "Submenu › Item", in the menu that shows, or closes it. Each
// returns whether a menu was showing.
bool host_menu_window_choose(const char* title) {
  return g_menu.shown && g_menu.choose && g_menu.choose(title);
}

bool host_menu_window_dismiss(void) {
  if (!g_menu.shown || !g_menu.dismissed) {
    return false;
  }
  g_menu.dismissed();
  return true;
}

}  // extern "C"

namespace {

void LinkPreviewForegroundChanged(HWND front) {
  HWND root = g_link_preview.root;
  // The browser window's own, and those it owns, such as a page's dialog, aren't another window.
  if (!root || g_link_preview.floating.mouse_through || GetAncestor(front, GA_ROOTOWNER) == root) {
    return;
  }
  host_release_link_preview();
  if (void (*dismissed)(void) = g_link_preview.dismissed) {
    Later([dismissed] { dismissed(); });
  }
}

// The task dialog host_ask shows, for host_choose, its message, and its buttons' labels and ids.
HWND g_alert = nullptr;
std::string g_alert_message;
std::vector<std::pair<std::wstring, int>> g_alert_buttons;

HRESULT CALLBACK AlertEvent(HWND hwnd, UINT event, WPARAM, LPARAM, LONG_PTR texts) {
  if (event == TDN_CREATED) {
    g_alert = hwnd;
    // check.sh reads this line, as the Mac's host writes it for its alert, once host_choose can
    // answer it.
    fprintf(stderr, "browser: alert shown %s\n", g_alert_message.c_str());
    // Its texts and buttons, as the Mac's driver logs an alert's, for check.sh to wait for.
    if (GetEnvironmentVariableW(L"BROWSER_DRIVE", nullptr, 0) > 0) {
      fprintf(stderr, "browser: modal shown %s\n", reinterpret_cast<const char*>(texts));
    }
  } else if (event == TDN_DESTROYED) {
    g_alert = nullptr;
  }
  return S_OK;
}

}  // namespace

extern "C" {

int host_ask(const char* message, const char* detail, const char* const* buttons, size_t count,
             bool returns) {
  std::wstring title = Wide(message), text = Wide(detail);
  std::vector<std::wstring> labels;
  std::vector<TASKDIALOG_BUTTON> task_buttons;
  for (size_t i = 0; i < count; ++i) {
    labels.push_back(Wide(buttons[i]));
  }
  int cancel = -1;
  for (size_t i = 0; i < count; ++i) {
    // Escape and the close box answer as Cancel.
    if (labels[i] == L"Cancel") {
      cancel = static_cast<int>(i);
      task_buttons.push_back({IDCANCEL, labels[i].c_str()});
    } else {
      task_buttons.push_back({static_cast<int>(100 + i), labels[i].c_str()});
    }
  }
  TASKDIALOGCONFIG config{};
  config.cbSize = sizeof config;
  config.hwndParent = GetForegroundWindow();
  config.dwFlags = cancel >= 0 ? TDF_ALLOW_DIALOG_CANCELLATION : 0;
  config.pszMainInstruction = title.c_str();
  config.pszContent = text.c_str();
  config.cButtons = static_cast<UINT>(task_buttons.size());
  config.pButtons = task_buttons.data();
  // A task dialog always has a default, so return chooses Cancel where it can't choose the first.
  config.nDefaultButton = task_buttons.empty() ? 0 : task_buttons[0].nButtonID;
  if (!returns && cancel >= 0) {
    config.nDefaultButton = IDCANCEL;
  }
  g_alert_message = message ? message : "";
  std::string texts = g_alert_message + ", " + (detail ? detail : "");
  g_alert_buttons.clear();
  for (const TASKDIALOG_BUTTON& button : task_buttons) {
    texts += ", " + Utf8(button.pszButtonText);
    g_alert_buttons.emplace_back(button.pszButtonText, button.nButtonID);
  }
  config.pfCallback = AlertEvent;
  config.lpCallbackData = reinterpret_cast<LONG_PTR>(texts.c_str());
  // Common Controls 6, which has task dialogs, only where the process's manifest asks for it.
  using TaskDialogIndirectFn = HRESULT(WINAPI*)(const TASKDIALOGCONFIG*, int*, int*, BOOL*);
  auto task_dialog = reinterpret_cast<TaskDialogIndirectFn>(
      GetProcAddress(LoadLibraryW(L"comctl32.dll"), "TaskDialogIndirect"));
  int pressed = 0;
  if (task_dialog && SUCCEEDED(task_dialog(&config, &pressed, nullptr, nullptr))) {
    return pressed == IDCANCEL ? cancel : pressed - 100;
  }
  // The first button as OK, and Cancel if there is one.
  const int answer = MessageBoxW(config.hwndParent, (title + L"\n\n" + text).c_str(), L"",
                                 cancel >= 0 ? MB_OKCANCEL : MB_OK);
  return answer == IDCANCEL ? cancel : 0;
}

// The UI draws the buttons; the frame only needs to know where Snap layouts' button is.
void host_show_window_buttons(void* ui_view, bool shown) {
  if (HWND root = Root(ui_view)) {
    WindowButtonsShown()[root] = shown;
  }
}

void host_press_window_button(void* ui_view, int button) {
  HWND root = Root(ui_view);
  const WPARAM command = button == 0   ? SC_CLOSE
                         : button == 1 ? SC_MINIMIZE
                         : IsZoomed(root) ? SC_RESTORE
                                          : SC_MAXIMIZE;
  PostMessageW(root, WM_SYSCOMMAND, command, 0);
}

void host_add_reveal(void* ui_view,
                     void* reveal_view,
                     void (*pointer_moved)(void* ui_view, double x)) {
  Reveal& reveal = g_reveals[ui_view];
  Adopt(reveal.floating, reveal_view, false, nullptr);
  reveal.pointer_moved = pointer_moved;
}

void host_show_reveal(void* ui_view, double width) {
  auto it = g_reveals.find(ui_view);
  if (it == g_reveals.end()) {
    return;
  }
  double content_width, content_height;
  ContentSize(Root(ui_view), &content_width, &content_height);
  // The UI shows it again as its width changes; check.sh reads the line once a showing.
  const bool was_shown = it->second.floating.hwnd && IsWindowVisible(it->second.floating.hwnd);
  Show(it->second.floating, ui_view, 0, 0, width, content_height, false);
  if (!was_shown) {
    LogShown(it->second.floating, "reveal");
  }
  WatchPointer();
}

void host_hide_reveal(void* ui_view) {
  if (auto it = g_reveals.find(ui_view); it != g_reveals.end()) {
    Hide(it->second.floating);
  }
  MaybeStopWatchingPointer();
}

void host_add_settings(void* settings_view) {
  g_settings.hwnd = Hwnd(settings_view);
  ShowWindow(g_settings.hwnd, SW_HIDE);
}

void host_show_settings(void) {
  if (!g_settings_placed) {
    // Centred over the browser window in front, the first time.
    if (HWND front = GetForegroundWindow(); UiViewOf(front)) {
      RECT over{}, own{};
      GetWindowRect(front, &over);
      GetWindowRect(g_settings.hwnd, &own);
      const int width = own.right - own.left, height = own.bottom - own.top;
      SetWindowPos(g_settings.hwnd, nullptr, (over.left + over.right - width) / 2,
                   (over.top + over.bottom - height) / 2, 0, 0, SWP_NOSIZE | SWP_NOZORDER);
    }
    g_settings_placed = true;
  }
  ShowWindow(g_settings.hwnd, SW_SHOW);
  SetForegroundWindow(g_settings.hwnd);
  // check.sh waits for this line. The window's title is its tab's name, as on the Mac. Read without
  // WM_GETTEXT, which would reach GPUI's window procedure while the app is borrowed.
  wchar_t title[128] = L"";
  InternalGetWindowText(g_settings.hwnd, title, static_cast<int>(std::size(title)));
  fprintf(stderr, "browser: settings shown %s\n", Utf8(title).c_str());
}

void host_hide_settings(void) {
  ShowWindow(g_settings.hwnd, SW_HIDE);
}

bool host_close_other_window(void) {
  HWND window = GetForegroundWindow();
  if (!window) {
    return false;
  }
  DWORD process = 0;
  GetWindowThreadProcessId(window, &process);
  // Our own, but neither a browser window (Ctrl+W closes its tab) nor one of the UI's floating
  // windows: a Chrome window of its own, such as the Task Manager or undocked DevTools, which carry
  // a system close button. Closing it is that button's click.
  if (process != GetCurrentProcessId() || UiViewOf(window) || FloatingByHwnd().count(window) ||
      !(GetWindowLongPtrW(window, GWL_STYLE) & WS_SYSMENU)) {
    return false;
  }
  PostMessageW(window, WM_CLOSE, 0, 0);
  return true;
}

void host_detent(void) {}

// shortcut: Windows has no dot before a window, so nothing to restore at launch; host_dock_dot
// gives each window's taskbar button its overlay once the windows are up.
void host_dock_dot_launch(const char*) {}

// Gives each browser window's taskbar button an overlay dot in |color| (0xRRGGBB), or ilo's green
// without |colored|, the counterpart of the Mac's Dock dot. One colour for the app, as the Dock has
// one icon; the taskbar makes its own copy, so the icon is freed after.
void host_dock_dot(bool colored, uint32_t color) {
  ITaskbarList3* taskbar = Taskbar();
  if (!taskbar) {
    return;
  }
  HICON dot = DotIcon(colored ? color : kIloGreen);
  for (HWND view : UiViews()) {
    if (IsWindow(view)) {
      taskbar->SetOverlayIcon(GetAncestor(view, GA_ROOT), dot, L"Space");
    }
  }
  if (dot) {
    DestroyIcon(dot);
  }
}

void host_watch_control(void (*released)(void)) {
  if ((GetKeyState(VK_CONTROL) & 0x8000) == 0) {
    Later([released] { released(); });
    return;
  }
  g_control_released = released;
  if (!g_key_hook) {
    g_key_hook = SetWindowsHookExW(WH_KEYBOARD, KeyPressed, nullptr, GetCurrentThreadId());
  }
}

void host_take_drops(bool (*dragged)(void* ui_view, int phase, double x, double y,
                                     const host_dragged* items, size_t count)) {
  g_dragged = dragged;
}

// The driver's (drive.cc): the drop target of |hwnd|, if it's a UI view, for its drags.
void* host_drop_target(void* hwnd) {
  auto found = DropTargets().find(Hwnd(hwnd));
  return found == DropTargets().end() ? nullptr : static_cast<IDropTarget*>(found->second);
}

bool host_trash(const char* path) {
  // Double-null-terminated, as the shell asks.
  std::wstring from = Wide(path) + L'\0';
  SHFILEOPSTRUCTW operation{};
  operation.wFunc = FO_DELETE;
  operation.pFrom = from.c_str();
  operation.fFlags = FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT;
  return SHFileOperationW(&operation) == 0 ||
         GetFileAttributesW(Wide(path).c_str()) == INVALID_FILE_ATTRIBUTES;
}

bool host_drag_file(const char* path, void (*ended)(bool)) {
  std::wstring wide = Wide(path);
  if (GetFileAttributesW(wide.c_str()) == INVALID_FILE_ATTRIBUTES) {
    return false;
  }
  ComPtr<IDataObject> data = FileDataObject(wide);
  if (!data) {
    return false;
  }
  // From a task of its own, once the UI's event has returned, as the Mac's drag session starts:
  // DoDragDrop is modal until the file is let go, and inside the event it would hold the UI, and
  // the driver's step that moved the pointer, until then.
  Later([data, ended] {
    ComPtr<IDropSource> source;
    source.Attach(new FileDropSource());
    // On the UI thread GPUI has made an OLE apartment, as Explorer's drag is.
    DWORD effect = DROPEFFECT_NONE;
    const HRESULT dropped = DoDragDrop(data.Get(), source.Get(), DROPEFFECT_COPY, &effect);
    if (ended) {
      ended(dropped == DRAGDROP_S_DROP && effect != DROPEFFECT_NONE);
    }
  });
  return true;
}

void host_file_preview(const char* path,
                       uint64_t id,
                       void (*done)(uint64_t, const uint8_t*, size_t, double, double, bool)) {
  std::thread([wide = Wide(path), id, done] {
    const bool com = SUCCEEDED(CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED));
    std::vector<uint8_t> png;
    double width = 0, height = 0;
    bool image = false;
    const bool ok = PreviewPng(wide, &png, &width, &height, &image);
    if (com) {
      CoUninitialize();
    }
    // Back on the main thread, where the UI reads the preview; the PNG bytes are ours, not COM's.
    Later([done, id, png = std::move(png), width, height, image, ok] {
      if (ok) {
        done(id, png.data(), png.size(), width, height, image);
      } else {
        done(id, nullptr, 0, 0, 0, false);
      }
    });
  }).detach();
}

// shortcut: no video playback yet; Media's video tiles show their preview.
void* host_video_open(const char*, int) {
  return nullptr;
}

bool host_video_frame(void*, uint8_t*) {
  return false;
}

void host_video_close(void*) {}

void host_copy(const char* text) {
  std::wstring wide = Wide(text);
  if (!OpenClipboard(nullptr)) {
    return;
  }
  EmptyClipboard();
  const size_t bytes = (wide.size() + 1) * sizeof(wchar_t);
  if (HGLOBAL memory = GlobalAlloc(GMEM_MOVEABLE, bytes)) {
    memcpy(GlobalLock(memory), wide.c_str(), bytes);
    GlobalUnlock(memory);
    if (!SetClipboardData(CF_UNICODETEXT, memory)) {
      GlobalFree(memory);
    }
  }
  CloseClipboard();
}

namespace {

// The clipboard's sequence number once the secret was put on it, which anything copied since
// changes.
DWORD g_secret_sequence = 0;

void CALLBACK ClearSecret(HWND, UINT, UINT_PTR timer, DWORD) {
  KillTimer(nullptr, timer);
  if (GetClipboardSequenceNumber() == g_secret_sequence && OpenClipboard(nullptr)) {
    EmptyClipboard();
    CloseClipboard();
  }
}

// A clipboard format holding |value|, as the formats Windows' clipboard history reads hold.
void SetClipboardDword(const wchar_t* format, DWORD value) {
  if (HGLOBAL memory = GlobalAlloc(GMEM_MOVEABLE, sizeof value)) {
    memcpy(GlobalLock(memory), &value, sizeof value);
    GlobalUnlock(memory);
    if (!SetClipboardData(RegisterClipboardFormatW(format), memory)) {
      GlobalFree(memory);
    }
  }
}

}  // namespace

bool host_copy_secret(const char* text, uint32_t seconds) {
  std::wstring wide = Wide(text);
  if (!OpenClipboard(nullptr)) {
    return false;
  }
  EmptyClipboard();
  const size_t bytes = (wide.size() + 1) * sizeof(wchar_t);
  if (HGLOBAL memory = GlobalAlloc(GMEM_MOVEABLE, bytes)) {
    memcpy(GlobalLock(memory), wide.c_str(), bytes);
    GlobalUnlock(memory);
    if (!SetClipboardData(CF_UNICODETEXT, memory)) {
      GlobalFree(memory);
    }
  }
  // Kept out of clipboard history and the cloud clipboard, and from clipboard monitors that honour
  // it, as password managers mark what they copy.
  SetClipboardDword(L"ExcludeClipboardContentFromMonitorProcessing", 0);
  SetClipboardDword(L"CanIncludeInClipboardHistory", 0);
  SetClipboardDword(L"CanUploadToCloudClipboard", 0);
  CloseClipboard();
  g_secret_sequence = GetClipboardSequenceNumber();
  SetTimer(nullptr, 0, seconds * 1000, ClearSecret);
  return true;
}

void host_forget_secret(void) {
  if (g_secret_sequence && GetClipboardSequenceNumber() == g_secret_sequence &&
      OpenClipboard(nullptr)) {
    EmptyClipboard();
    CloseClipboard();
  }
  g_secret_sequence = 0;
}

bool host_key_repeated(void) {
  return false;
}

size_t host_paste(char* text, size_t capacity) {
  std::string held;
  if (OpenClipboard(nullptr)) {
    if (HANDLE memory = GetClipboardData(CF_UNICODETEXT)) {
      if (const wchar_t* wide = static_cast<const wchar_t*>(GlobalLock(memory))) {
        held = Utf8(wide);
        GlobalUnlock(memory);
      }
    }
    CloseClipboard();
  }
  if (capacity) {
    snprintf(text, capacity, "%s", held.c_str());
  }
  return held.size();
}

void host_open_full_disk_access(void) {}

// There's no Safari: Windows plays protected video with Chromium's own PlayReady (decision 1).
void host_open_in_safari(const char* url) {
  fprintf(stderr, "browser: no system engine for %s\n", url);
}

void host_share(void* ui_view,
                double,
                double,
                double,
                double,
                const char* url,
                const char* title) {
  HWND root = Root(ui_view);
  if (!root || !url) {
    return;
  }
  winrt::hstring link{Wide(url)};
  winrt::hstring name{title ? Wide(title) : std::wstring()};
  ShowShareSheet(root, [link, name](const wdt::DataPackage& data) {
    if (!name.empty()) {
      data.Properties().Title(name);
    }
    try {
      data.SetWebLink(wf::Uri(link));
    } catch (const winrt::hresult_error&) {
      // A page the engine opened but that isn't a web URL still shares as text below.
    }
    data.SetText(link);
  });
}

// Windows' share sheet (DataTransferManager) is a closed system flyout: it gives no way to list its
// targets or to invoke one directly, so the Mac's enumerate-then-share path has no counterpart. The
// tab menu's Share submenu stays empty on Windows; host_share and host_share_png open the sheet
// itself instead.
void host_share_services(const char*, void (*)(void*, const char*), void*) {}
void host_share_with(const char*, const char*, size_t) {}

void host_day_starts(double starts[4]) {
  const time_t now = time(nullptr);
  tm today{};
  localtime_s(&today, &now);
  today.tm_hour = today.tm_min = today.tm_sec = 0;
  today.tm_isdst = -1;
  tm yesterday = today;
  yesterday.tm_mday -= 1;
  // The user's first day of the week: 0 is Monday.
  wchar_t first[2] = L"0";
  GetLocaleInfoEx(LOCALE_NAME_USER_DEFAULT, LOCALE_IFIRSTDAYOFWEEK, first, 2);
  const int first_day = ((first[0] - L'0') + 1) % 7;  // As tm_wday counts, from Sunday.
  tm week = today;
  week.tm_mday -= (today.tm_wday - first_day + 7) % 7;
  tm last_week = week;
  last_week.tm_mday -= 7;
  starts[0] = mktime(&today) * 1000.0;
  starts[1] = mktime(&yesterday) * 1000.0;
  starts[2] = mktime(&week) * 1000.0;
  starts[3] = mktime(&last_week) * 1000.0;
}

bool host_choose(const char* title) {
  if (!g_alert) {
    return false;
  }
  for (const auto& [label, id] : g_alert_buttons) {
    if (label == Wide(title)) {
      PostMessageW(g_alert, TDM_CLICK_BUTTON, id, 0);
      return true;
    }
  }
  return false;
}

// The browser window, its root HWND.
void* host_browser_window(size_t index) {
  if (index >= MadeUiViews().size() || !IsWindow(MadeUiViews()[index])) {
    return nullptr;
  }
  HWND root = Root(MadeUiViews()[index]);
  return root != MadeUiViews()[index] && IsWindowVisible(root) ? root : nullptr;
}

// The core's id for that browser window, for the driver's commands that act on it through the core,
// or 0.
uint64_t host_browser_window_id(size_t index) {
  if (index >= MadeUiViews().size()) {
    return 0;
  }
  auto it = WindowIds().find(MadeUiViews()[index]);
  return it == WindowIds().end() ? 0 : it->second;
}

// For the driver (drive.cc), whose input is posted to the app's windows, which the thread hooks
// above never hear: it hands them each pointer event, at |x|, |y| on screen, and each key. Each
// returns whether ilo's menu took it, which then goes no further.
bool host_drive_pointer(unsigned message, long x, long y) {
  const POINT point{x, y};
  if (message == WM_MOUSEMOVE && g_mouse_hook) {
    PointerMoved(point);
  }
  return g_menu_mouse_hook && MenuTakesPointer(message, point);
}

bool host_drive_key(unsigned vk, long lparam) {
  if (g_key_hook) {
    KeyChanged();
  }
  return g_menu_key_hook && MenuTakesKey(vk, lparam);
}

}  // extern "C"
