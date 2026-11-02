// Test driver for check.sh on Windows, as platform/mac/drive.mm is on the Mac. With BROWSER_DRIVE=1
// the app reads one command per line on stdin, in drive.mm's language, and posts the matching
// window messages to its own windows only: Chromium's browser windows and their pages, and GPUI's
// UI and overlays in and over them. Each window's own procedure handles them as it would the
// device's, so they go through the same hit testing, focus changes and shortcuts, without
// touching the desktop's pointer or keyboard: nothing here calls SendInput or takes the
// foreground. Modifiers are held in the UI thread's own keyboard state, which is what Chromium and
// GPUI read, never the system's.
//
// The commands are drive.mm's, with what Windows has: click, doubleclick, rightclick, middleclick,
// clickkey, doubleclickkey, rightclickkey, drag, dragkey, press, dragto, release, rightpress,
// rightdragto, rightrelease, move, key, keydown, keyup, flags, type, insert, scroll, swipe, wheel,
// resize, place, minimize, window, order, layout, state, title, pasteboard, front, menu, menubar,
// choose, dismiss, agentspace, agentmayact, agentrelease, agentdisconnect, takeback and age, and
// Windows' own overlays, which lists the windows on screen over the app's browser windows, and
// reshow, which shows those of the UI's that are hidden again, as Windows does. The Mac's modifiers are
// pressed as the app reads them off the Mac (keys.rs): cmd with Ctrl, ctrl with Alt, but Ctrl for
// ⌃Tab, and alt with both. A trackpad's scrolls and swipes are wheel messages with the pixels in
// their deltas, as a precision touchpad sends them. choose and dismiss work on ilo's menu and the
// alerts host_ask shows. dragover, dragdrop and dragleave hand a drag from another app, a shell
// data object carrying what Explorer or a browser would, straight to the drop target Windows would
// pick, the UI's; one over a page or an overlay, whose targets are Chromium's and GPUI's, logs as
// unsupported. ime on types as drive.mm's scripted input method does, in the UI's fields alone, as
// Linux's driver does. fullscreen puts the window full screen or takes it out through the core, as
// Linux's driver does. The others, pageswipe, displays, hide, backdrop and openmenu, Windows has
// nothing the driver can stand in for yet: each logs "browser: drive: unsupported
// <command>" before drive.mm's "can't do that", so a run fails at once.
//
// It logs as drive.mm does: "browser: drive <n>: <command>" as command n arrives, and "browser:
// drive <n> done, key <window>, focus <view>, active <0|1>, marked <none|start,length>" once
// Chromium and GPUI have handled its messages, with what the UI's field with the keys has marked.
// The window and view are named as AppKit's classes are on the Mac, so the runs read one
// vocabulary: key is BrowserCoreNSWindow for a browser window, GPUIPanel for an overlay and
// GPUIWindow for another window of the UI's, such as Settings, and focus RenderWidgetHostViewCocoa
// for a page, AccessKitSubclassOfGPUIView for the UI and GPUIView for an overlay. Active is whether
// one of the app's windows is in front.

#include <windows.h>

#include <shlobj.h>

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <deque>
#include <functional>
#include <sstream>
#include <optional>
#include <string>
#include <thread>
#include <utility>
#include <vector>

#include "../host.h"
#include "browser_core.h"

// host.cc's, for the driver alone.
extern "C" {
bool host_menu_window_choose(const char* title);
bool host_menu_window_dismiss(void);
bool host_drive_pointer(unsigned message, long x, long y);
bool host_drive_key(unsigned vk, long lparam);
uint64_t host_browser_window_id(size_t index);
// The app's (src/main.rs), on GPUI's window |hwnd|, for the scripted input method.
bool browser_drive_compose(void* hwnd, const char* text, size_t start, size_t end, bool selected);
bool browser_drive_commit(void* hwnd, const char* text);
bool browser_drive_marked(void* hwnd, size_t* start, size_t* length);
void* host_drop_target(void* hwnd);
}

namespace {

UINT const kCommand = WM_APP + 1;
UINT const kStep = WM_APP + 2;
UINT_PTR const kStepTimer = 1;
// Marks the pointer leaving that the driver posts, which its hook lets through.
LPARAM const kDriverLeave = 0x44524956;

// The Mac's modifiers as Windows' keys hold them.
int const kCtrl = 1;
int const kShift = 2;
int const kAlt = 4;

// A step of a command: what it posts, then how long to wait before the next.
struct Step {
  std::function<void()> run;
  UINT wait = 0;
};

// The driver's message-only window, on the UI thread, which runs each command's steps.
HWND g_driver = nullptr;
// The commands come in, and each one's steps then run in turn, its messages handled before the
// next.
std::deque<std::pair<long, std::string>> g_commands;
std::deque<Step> g_steps;
bool g_running = false;
// The window command's, from 0.
size_t g_driven = 0;
// The modifiers flags holds from now on.
int g_flags = 0;
// Whether the last key's Ctrl is still down. It stays down until the next input, as the Mac's app
// takes ⌃ to be until the flags change, so a ⌃Tab walk goes on to the next ⌃Tab, as Linux's driver
// holds it (KEY_FLAGS).
bool g_key_ctrl = false;
// The window a press went to, which hears the drag until the release, as capture does.
HWND g_pressed = nullptr;
// The window the pointer is in, which hears it leave.
HWND g_hovered = nullptr;
// The moves the driver has posted and its windows haven't taken yet, which DropRealPointer lets
// through. Some are never taken, as by a window that closed, so only the latest are kept.
std::deque<std::pair<HWND, LPARAM>> g_posted_moves;
// The last click: its window, place on screen, button and time.
struct Click {
  HWND hwnd = nullptr;
  POINT at{};
  UINT button = 0;
  ULONGLONG time = 0;
} g_last_click;
// The app's menus, for menu and menubar: chooses the item titled |title| in the browser window
// whose UI view is |ui_view|, or with |bar| logs the menu so titled.
bool (*g_menus)(void* ui_view, const char* title, bool bar) = nullptr;

std::wstring ClassOf(HWND hwnd) {
  wchar_t name[64] = L"";
  GetClassNameW(hwnd, name, 64);
  return name;
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

std::wstring Wide(const std::string& utf8) {
  const int length = MultiByteToWideChar(CP_UTF8, 0, utf8.c_str(), -1, nullptr, 0);
  std::wstring wide(length > 0 ? length - 1 : 0, L'\0');
  if (length > 1) {
    MultiByteToWideChar(CP_UTF8, 0, utf8.c_str(), -1, wide.data(), length);
  }
  return wide;
}

// A window of Chromium's: a browser window, a page's pane in one, or a window of Chrome's own.
bool IsChromium(HWND hwnd) {
  return ClassOf(hwnd).rfind(L"Chrome_WidgetWin_", 0) == 0;
}

bool IsGpui(HWND hwnd) {
  return ClassOf(hwnd) == L"Zed::Window";
}

double Scale(HWND hwnd) {
  const UINT dpi = hwnd ? GetDpiForWindow(hwnd) : GetDpiForSystem();
  return (dpi ? dpi : 96) / 96.0;
}

HWND Browser(size_t index) {
  return static_cast<HWND>(host_browser_window(index));
}

// The browser window |hwnd| is, from 0, or -1.
long BrowserIndex(HWND hwnd) {
  for (size_t index = 0; index < 64; index++) {
    if (hwnd && Browser(index) == hwnd) {
      return static_cast<long>(index);
    }
  }
  return -1;
}

HWND Driven() {
  return Browser(g_driven);
}

bool Ours(HWND hwnd) {
  return hwnd && GetWindowThreadProcessId(hwnd, nullptr) == GetCurrentThreadId();
}

// Whether the pointer at |screen| would be in |hwnd|, as Windows hit tests: shown, enabled, not
// see-through, in its region if it has one, and not saying it's transparent there.
bool Hits(HWND hwnd, POINT screen) {
  if (!IsWindowVisible(hwnd) || !IsWindowEnabled(hwnd) || IsIconic(hwnd) ||
      (GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & WS_EX_TRANSPARENT)) {
    return false;
  }
  RECT rect;
  GetWindowRect(hwnd, &rect);
  if (!PtInRect(&rect, screen)) {
    return false;
  }
  HRGN region = CreateRectRgn(0, 0, 0, 0);
  const bool outside = GetWindowRgn(hwnd, region) != ERROR &&
                       !PtInRegion(region, screen.x - rect.left, screen.y - rect.top);
  DeleteObject(region);
  if (outside) {
    return false;
  }
  // Asked only of the UI thread's windows, as another thread's could be waiting on this one.
  return !Ours(hwnd) ||
         SendMessageW(hwnd, WM_NCHITTEST, 0, MAKELPARAM(screen.x, screen.y)) != HTTRANSPARENT;
}

// Whether |top| floats over |root|: owned by it, or by a window that is, as ilo's menu over a
// reveal is.
bool Over(HWND top, HWND root) {
  for (HWND owner = GetWindow(top, GW_OWNER); owner; owner = GetWindow(owner, GW_OWNER)) {
    if (owner == root) {
      return true;
    }
  }
  return false;
}

// The window in |hwnd| that the pointer at |screen| goes to: a page's pane, GPUI's view, or |hwnd|.
HWND Descend(HWND hwnd, POINT screen) {
  for (HWND child = GetWindow(hwnd, GW_CHILD); child; child = GetWindow(child, GW_HWNDNEXT)) {
    if (Hits(child, screen)) {
      // A page's pane takes its own pointer events, for the page in it.
      return IsChromium(child) ? child : Descend(child, screen);
    }
  }
  return hwnd;
}

// Where the pointer goes: a window and the point in its client area, in pixels.
struct Target {
  HWND hwnd = nullptr;
  POINT client{};
  POINT screen{};
};

Target At(HWND hwnd, POINT screen) {
  Target target{hwnd, screen, screen};
  ScreenToClient(hwnd, &target.client);
  return target;
}

// The point |x|, |y|, in points from |base|'s content's top-left corner, on screen.
POINT OnScreen(HWND base, double x, double y) {
  const double scale = Scale(base);
  POINT screen{static_cast<LONG>(std::lround(x * scale)),
               static_cast<LONG>(std::lround(y * scale))};
  ClientToScreen(base, &screen);
  return screen;
}

// The app's window on top at |x|, |y| of the driven browser window, such as an overlay over it.
bool TargetAt(double x, double y, Target* target) {
  HWND root = Driven();
  if (!root) {
    return false;
  }
  const POINT screen = OnScreen(root, x, y);
  for (HWND top = GetTopWindow(nullptr); top && top != root; top = GetWindow(top, GW_HWNDNEXT)) {
    if (Ours(top) && Over(top, root) && Hits(top, screen)) {
      *target = At(Descend(top, screen), screen);
      return true;
    }
  }
  *target = At(Descend(root, screen), screen);
  return true;
}

// The point in the window with the keyboard, as clickkey names it.
bool TargetInKey(double x, double y, Target* target) {
  HWND key = GetActiveWindow();
  if (!key) {
    return false;
  }
  const POINT screen = OnScreen(key, x, y);
  *target = At(Descend(key, screen), screen);
  return true;
}

// The modifiers held, flags' and |modifiers|, in the UI thread's keyboard state, and the buttons.
void HoldKeys(int modifiers, UINT buttons = 0) {
  BYTE state[256];
  GetKeyboardState(state);
  const int held = modifiers | g_flags | (g_key_ctrl ? kCtrl : 0);
  const auto set = [&state](int key, bool down) { state[key] = down ? 0x80 : 0; };
  set(VK_CONTROL, held & kCtrl);
  set(VK_LCONTROL, held & kCtrl);
  set(VK_SHIFT, held & kShift);
  set(VK_LSHIFT, held & kShift);
  set(VK_MENU, held & kAlt);
  set(VK_LMENU, held & kAlt);
  set(VK_LBUTTON, buttons & MK_LBUTTON);
  set(VK_RBUTTON, buttons & MK_RBUTTON);
  set(VK_MBUTTON, buttons & MK_MBUTTON);
  SetKeyboardState(state);
}

WPARAM MouseKeys(int modifiers, UINT buttons) {
  const int held = modifiers | g_flags;
  return buttons | ((held & kCtrl) ? MK_CONTROL : 0) | ((held & kShift) ? MK_SHIFT : 0);
}

LPARAM ClientParam(const Target& target) {
  return MAKELPARAM(target.client.x, target.client.y);
}

// The pointer moves into |target|'s window: the one it was in hears it leave, as Windows tells it.
void Enter(const Target& target) {
  if (g_hovered && g_hovered != target.hwnd && IsWindow(g_hovered)) {
    PostMessageW(g_hovered, WM_MOUSELEAVE, 0, kDriverLeave);
  }
  g_hovered = target.hwnd;
}

// Posts the pointer moving to |target|, with |buttons| held.
void Move(const Target& target, int modifiers, UINT buttons) {
  HoldKeys(modifiers, buttons);
  if (host_drive_pointer(WM_MOUSEMOVE, target.screen.x, target.screen.y)) {
    return;
  }
  // Over the frame, as over the UI's maximize button (host.cc's FrameHit), Windows sends the
  // window a non-client move, with what's there and the point on the screen. Entered, so the leave
  // Windows sends it at once, for the desktop's pointer, is dropped.
  if (Ours(target.hwnd)) {
    const LPARAM screen = MAKELPARAM(target.screen.x, target.screen.y);
    const LRESULT hit = SendMessageW(target.hwnd, WM_NCHITTEST, 0, screen);
    if (hit != HTCLIENT && hit != HTTRANSPARENT && hit != HTNOWHERE) {
      Enter(target);
      PostMessageW(target.hwnd, WM_NCMOUSEMOVE, hit, screen);
      return;
    }
  }
  Enter(target);
  if (g_posted_moves.size() == 64) {
    g_posted_moves.pop_front();
  }
  g_posted_moves.emplace_back(target.hwnd, ClientParam(target));
  PostMessageW(target.hwnd, WM_MOUSEMOVE, MouseKeys(modifiers, buttons), ClientParam(target));
}

UINT Down(UINT button) {
  return button == MK_RBUTTON   ? WM_RBUTTONDOWN
         : button == MK_MBUTTON ? WM_MBUTTONDOWN
                                : WM_LBUTTONDOWN;
}

UINT Up(UINT button) {
  return button == MK_RBUTTON ? WM_RBUTTONUP : button == MK_MBUTTON ? WM_MBUTTONUP : WM_LBUTTONUP;
}

// Posts |button| going down at |target|, as Windows sends a press: its window's root decides
// whether it's activated (only on this thread, never brought to the front) and the window's parents
// hear of it, which is how a click in the UI takes the keyboard from the page.
void Press(const Target& target, UINT button, int modifiers, bool second) {
  HoldKeys(modifiers, button);
  const UINT down = Down(button);
  if (host_drive_pointer(down, target.screen.x, target.screen.y)) {
    return;
  }
  Enter(target);
  HWND root = GetAncestor(target.hwnd, GA_ROOT);
  const LRESULT activate = SendMessageW(target.hwnd, WM_MOUSEACTIVATE,
                                        reinterpret_cast<WPARAM>(root), MAKELPARAM(HTCLIENT, down));
  const bool eaten = activate == MA_ACTIVATEANDEAT || activate == MA_NOACTIVATEANDEAT;
  // Windows makes the second press of a double click a double click for the windows that ask for
  // them. GPUI's don't, and count clicks themselves.
  const bool double_click =
      second && (GetClassLongPtrW(target.hwnd, GCL_STYLE) & CS_DBLCLKS) && button == MK_LBUTTON;
  // Queued before the activation, as Windows has taken a press off the queue by the time it
  // activates: what the windows post as they activate comes after the press, as an overlay's
  // dismissal does, which the press reads the bar still open before (#872).
  if (!eaten) {
    PostMessageW(target.hwnd, double_click ? WM_LBUTTONDBLCLK : down, MouseKeys(modifiers, button),
                 ClientParam(target));
  }
  if ((activate == MA_ACTIVATE || activate == MA_ACTIVATEANDEAT) && GetActiveWindow() != root) {
    SetActiveWindow(root);
  }
  if (eaten) {
    return;
  }
  for (HWND child = target.hwnd; GetWindowLongPtrW(child, GWL_STYLE) & WS_CHILD;) {
    if (GetWindowLongPtrW(child, GWL_EXSTYLE) & WS_EX_NOPARENTNOTIFY) {
      break;
    }
    HWND parent = GetParent(child);
    if (!parent) {
      break;
    }
    POINT in = target.screen;
    ScreenToClient(parent, &in);
    SendMessageW(parent, WM_PARENTNOTIFY, MAKEWPARAM(down, 0), MAKELPARAM(in.x, in.y));
    child = parent;
  }
}

void Release(const Target& target, UINT button, int modifiers) {
  HoldKeys(modifiers);
  if (host_drive_pointer(Up(button), target.screen.x, target.screen.y)) {
    return;
  }
  PostMessageW(target.hwnd, Up(button), MouseKeys(modifiers, 0), ClientParam(target));
}

// How long a click at |target| waits, so it isn't taken with the last one for a double click.
UINT WaitBeforeClick(const Target& target, UINT button) {
  const ULONGLONG since = GetTickCount64() - g_last_click.time;
  const bool close_by =
      std::abs(target.screen.x - g_last_click.at.x) <= GetSystemMetrics(SM_CXDOUBLECLK) &&
      std::abs(target.screen.y - g_last_click.at.y) <= GetSystemMetrics(SM_CYDOUBLECLK);
  if (g_last_click.hwnd != target.hwnd || g_last_click.button != button || !close_by ||
      since > GetDoubleClickTime()) {
    return 0;
  }
  return static_cast<UINT>(GetDoubleClickTime() - since + 50);
}

// The steps of a click at |target|, or with |twice| a double click.
void AddClick(const Target& target, UINT button, int modifiers, bool twice) {
  if (const UINT wait = WaitBeforeClick(target, button)) {
    g_steps.push_back({[] {}, wait});
  }
  g_steps.push_back({[=] { Move(target, modifiers, 0); }});
  for (int press = 0; press < (twice ? 2 : 1); press++) {
    g_steps.push_back({[=] { Press(target, button, modifiers, press == 1); }});
    g_steps.push_back({[=] {
      Release(target, button, modifiers);
      g_last_click = {target.hwnd, target.screen, button, GetTickCount64()};
    }});
  }
}

// The Mac's modifiers named in |words|, as the keys that press them off the Mac. ⌃Tab is Ctrl+Tab
// there, as Alt+Tab is the desktop's.
int Modifiers(std::istringstream& words, const std::string& key = "") {
  int modifiers = 0;
  bool cmd = false, ctrl = false;
  std::string word;
  while (words >> word) {
    cmd = cmd || word == "cmd";
    ctrl = ctrl || word == "ctrl";
    modifiers |= word == "cmd"     ? kCtrl
                 : word == "shift" ? kShift
                 : word == "alt"   ? kCtrl | kAlt
                                   : 0;
  }
  if (ctrl) {
    modifiers |= key == "tab" && !cmd ? kCtrl : kAlt;
  }
  return modifiers;
}

// The virtual key of drive.mm's |key|, and the modifiers this keyboard types it with, or 0.
UINT VirtualKey(const std::string& key, int* modifiers) {
  static const std::pair<const char*, UINT> named[] = {
      {"return", VK_RETURN},  {"escape", VK_ESCAPE}, {"tab", VK_TAB},      {"space", VK_SPACE},
      {"backspace", VK_BACK}, {"delete", VK_DELETE}, {"pageup", VK_PRIOR}, {"pagedown", VK_NEXT},
      {"up", VK_UP},          {"down", VK_DOWN},     {"left", VK_LEFT},    {"right", VK_RIGHT},
      {"f4", VK_F4},          {"f6", VK_F6},         {"f10", VK_F10},      {"f12", VK_F12}};
  for (const auto& [name, vk] : named) {
    if (key == name) {
      return vk;
    }
  }
  const std::wstring character = Wide(key);
  if (character.size() != 1) {
    return 0;
  }
  const SHORT scan = VkKeyScanW(character[0]);
  if (scan == -1) {
    return 0;
  }
  *modifiers |=
      ((scan & 0x100) ? kShift : 0) | ((scan & 0x200) ? kCtrl : 0) | ((scan & 0x400) ? kAlt : 0);
  return LOBYTE(scan);
}

bool Extended(UINT vk) {
  switch (vk) {
    case VK_PRIOR:
    case VK_NEXT:
    case VK_END:
    case VK_HOME:
    case VK_LEFT:
    case VK_UP:
    case VK_RIGHT:
    case VK_DOWN:
    case VK_INSERT:
    case VK_DELETE:
      return true;
  }
  return false;
}

// The window the keyboard's messages go to: the focus on the UI thread, else the window with it.
HWND KeyTarget() {
  if (HWND focus = GetFocus()) {
    return focus;
  }
  if (HWND active = GetActiveWindow()) {
    return active;
  }
  return Driven();
}

// Posts |vk| going down or up with |modifiers| held, as Windows posts a key: a system key with Alt
// and without Ctrl, or F10, which Chromium and GPUI each handle.
void PostKey(UINT vk, int modifiers, bool down, bool repeat = false) {
  HoldKeys(modifiers);
  BYTE state[256];
  GetKeyboardState(state);
  state[vk] = down ? 0x80 : 0;
  SetKeyboardState(state);
  const int held = modifiers | g_flags;
  LPARAM lparam = 1 | (MapVirtualKeyW(vk, MAPVK_VK_TO_VSC) << 16);
  lparam |= Extended(vk) ? 1 << 24 : 0;
  lparam |= (held & kAlt) ? 1 << 29 : 0;
  lparam |= (!down || repeat) ? 1 << 30 : 0;
  lparam |= !down ? 1u << 31 : 0;
  if (host_drive_key(vk, static_cast<long>(lparam))) {
    return;
  }
  const bool system = ((held & kAlt) && !(held & kCtrl)) || vk == VK_F10;
  const UINT message =
      down ? (system ? WM_SYSKEYDOWN : WM_KEYDOWN) : (system ? WM_SYSKEYUP : WM_KEYUP);
  PostMessageW(KeyTarget(), message, vk, lparam);
}

// Ctrl's and Shift's keys going down or up, from |from| held to |to|, as the keyboard sends them.
// Alt's are left out: Alt let go alone opens the window's menu, which a key with it in between
// stops on a real keyboard, and a key's Alt is read from the state held, as Chromium and GPUI read
// every modifier.
void PostModifiers(int from, int to) {
  static const std::pair<int, UINT> keys[] = {{kCtrl, VK_CONTROL}, {kShift, VK_SHIFT}};
  for (const auto& [modifier, vk] : keys) {
    if ((from & modifier) != (to & modifier)) {
      from = (to & modifier) ? from | modifier : from & ~modifier;
      PostKey(vk, from, (to & modifier) != 0);
    }
  }
}

// The steps of a key pressed and let go with |modifiers|, the modifiers' own keys around it. Each
// step's messages are handled with the keyboard state it leaves, so each change is a step.
void AddKey(UINT vk, int modifiers) {
  g_steps.push_back({[=] {
    const int held = g_flags | (std::exchange(g_key_ctrl, false) ? kCtrl : 0);
    PostModifiers(held, g_flags | modifiers);
    PostKey(vk, modifiers, true);
  }});
  g_steps.push_back({[=] { PostKey(vk, modifiers, false); }});
  g_steps.push_back({[=] {
    const int kept = modifiers & kCtrl & ~g_flags;
    PostModifiers(g_flags | modifiers, g_flags | kept);
    g_key_ctrl = kept != 0;
    HoldKeys(0);
  }});
}

// Lets go of the Ctrl the last key left down, as input that isn't a key comes.
void LetGoKeyCtrl() {
  if (std::exchange(g_key_ctrl, false)) {
    PostModifiers(g_flags | kCtrl, g_flags);
    HoldKeys(0);
  }
}

// The radius in points of |hwnd|'s round corners, as its window region cuts them, or 0: the
// diameter whose round rectangle, all four corners or the bottom two, is that region exactly, as
// the core makes it (core/platform/win/window_views.cc).
double CornerRadius(HWND hwnd, double scale) {
  HRGN region = CreateRectRgn(0, 0, 0, 0);
  RECT client;
  GetClientRect(hwnd, &client);
  double radius = 0;
  if (GetWindowRgn(hwnd, region) != ERROR) {
    for (int diameter = 2; diameter <= 128 && !radius; ++diameter) {
      HRGN round =
          CreateRoundRectRgn(0, 0, client.right + 1, client.bottom + 1, diameter, diameter);
      HRGN bottom = CreateRectRgn(0, 0, client.right, client.bottom / 2);
      CombineRgn(bottom, bottom, round, RGN_OR);
      if (EqualRgn(region, round) || EqualRgn(region, bottom)) {
        radius = diameter / (2 * scale);
      }
      DeleteObject(round);
      DeleteObject(bottom);
    }
  }
  DeleteObject(region);
  return radius;
}

void LogLayout(HWND root) {
  RECT client;
  GetClientRect(root, &client);
  const double scale = Scale(root);
  const double width = client.right / scale, height = client.bottom / scale;
  std::string line;
  char part[160];
  snprintf(part, sizeof part, "browser: layout content %.0fx%.0f", width, height);
  line += part;
  // The page with the keyboard, as drive.mm finds the WebContentsViewCocoa its first responder is
  // in: the pane whose window has the focus, or holds the one that has it.
  const HWND focus = GetFocus();
  std::string keyboard = "none";
  // Back to front, as AppKit lists a view's subviews: GPUI's view, then the pages over it. Frames
  // as the Mac's are, from the content's bottom-left corner.
  for (HWND child = GetWindow(GetWindow(root, GW_CHILD), GW_HWNDLAST); child;
       child = GetWindow(child, GW_HWNDPREV)) {
    if (!IsWindowVisible(child)) {
      continue;
    }
    const char* name = IsChromium(child)                ? "WebContentsViewCocoa"
                       : ClassOf(child) == L"BrowserUI" ? "GPUIView"
                                                        : nullptr;
    if (!name) {
      continue;
    }
    RECT rect;
    GetWindowRect(child, &rect);
    MapWindowPoints(HWND_DESKTOP, root, reinterpret_cast<POINT*>(&rect), 2);
    snprintf(part, sizeof part, "%.0f,%.0f %.0fx%.0f", rect.left / scale,
             (client.bottom - rect.bottom) / scale, (rect.right - rect.left) / scale,
             (rect.bottom - rect.top) / scale);
    line += std::string(", ") + name + " " + part;
    if (IsChromium(child) && focus && (focus == child || IsChild(child, focus))) {
      keyboard = part;
    }
    // As the Mac's driver gives a page view's rounded layer.
    if (const double radius = IsChromium(child) ? CornerRadius(child, scale) : 0; radius > 0) {
      snprintf(part, sizeof part, " radius %.0f", radius);
      line += part;
    }
  }
  // Windows' own title bar has the window's buttons.
  line += "\nbrowser: layout buttons hidden hidden hidden";
  // A tab's dialog, such as print preview, as the core gives it: inside the border and shadow
  // Chromium draws in its window, as the Mac's driver gives a sheet's frame.
  bc_rect dialog{};
  const bool has_dialog = bc_window_dialog(host_browser_window_id(g_driven), &dialog);
  if (has_dialog) {
    snprintf(part, sizeof part, "\nbrowser: layout child %.0f,%.0f %.0fx%.0f", dialog.x, dialog.y,
             dialog.width, dialog.height);
    line += part;
  }
  line += "\nbrowser: layout keyboard " + keyboard;
  // The overlays over it, such as the find bar: from the content's top-left corner, and their size.
  POINT origin{0, 0};
  ClientToScreen(root, &origin);
  for (HWND top = GetTopWindow(nullptr); top; top = GetWindow(top, GW_HWNDNEXT)) {
    if (!Ours(top) || GetWindow(top, GW_OWNER) != root || !IsWindowVisible(top)) {
      continue;
    }
    RECT rect;
    GetWindowRect(top, &rect);
    const double x = (rect.left - origin.x) / scale, y = (rect.top - origin.y) / scale;
    const double across = (rect.right - rect.left) / scale;
    const double down = (rect.bottom - rect.top) / scale;
    // The dialog's own window, listed above.
    if (has_dialog && IsChromium(top) && x <= dialog.x && y <= dialog.y &&
        x + across >= dialog.x + dialog.width && y + down >= dialog.y + dialog.height) {
      continue;
    }
    snprintf(part, sizeof part, "\nbrowser: layout child %.0f,%.0f %.0fx%.0f", x, y, across, down);
    line += part;
  }
  // Full screen, as Chrome's is on Windows: no title bar, over its whole display.
  MONITORINFO monitor{sizeof monitor};
  GetMonitorInfoW(MonitorFromWindow(root, MONITOR_DEFAULTTONEAREST), &monitor);
  RECT frame;
  GetWindowRect(root, &frame);
  const bool full =
      !(GetWindowLongPtrW(root, GWL_STYLE) & WS_CAPTION) && EqualRect(&frame, &monitor.rcMonitor);
  snprintf(part, sizeof part, "\nbrowser: layout full screen %d", full);
  line += part;
  fprintf(stderr, "%s\n", line.c_str());
}

// A browser window's frame in points from the main display's top-left, as place takes it: its
// window rectangle, invisible resize borders and all, which the app logs and saves as the Mac's
// frame (host_window_frame), not its content.
void LogFrame(const char* name, HWND root) {
  RECT frame;
  GetWindowRect(root, &frame);
  const double scale = Scale(root);
  fprintf(stderr, "browser: frame %s %.0f %.0f %.0f %.0f\n", name, frame.left / scale,
          frame.top / scale, (frame.right - frame.left) / scale,
          (frame.bottom - frame.top) / scale);
}

// Puts the driven window's frame, in LogFrame's terms, at |x|, |y|, |width| by |height| points.
bool PlaceFrame(double x, double y, double width, double height) {
  HWND root = Driven();
  if (!root) {
    return false;
  }
  const double scale = Scale(root);
  SetWindowPos(root, nullptr, static_cast<int>(x * scale), static_cast<int>(y * scale),
               static_cast<int>(width * scale), static_cast<int>(height * scale),
               SWP_NOZORDER | SWP_NOACTIVATE);
  return true;
}

// Sets the driven window's content to |width| by |height| points, where it is.
bool ResizeContent(double width, double height) {
  HWND root = Driven();
  if (!root) {
    return false;
  }
  RECT frame, client;
  GetWindowRect(root, &frame);
  GetClientRect(root, &client);
  const double scale = Scale(root);
  const int extra_width = (frame.right - frame.left) - client.right;
  const int extra_height = (frame.bottom - frame.top) - client.bottom;
  SetWindowPos(root, nullptr, 0, 0, static_cast<int>(width * scale) + extra_width,
               static_cast<int>(height * scale) + extra_height,
               SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE);
  return true;
}

// A window as front names it.
std::string Describe(HWND hwnd) {
  if (!hwnd) {
    return "none";
  }
  const long index = BrowserIndex(hwnd);
  if (index >= 0) {
    return index == static_cast<long>(g_driven) ? "driven" : "window " + std::to_string(index + 1);
  }
  RECT rect;
  GetWindowRect(hwnd, &rect);
  const double scale = Scale(hwnd);
  char text[160];
  snprintf(text, sizeof text, "%s %.0fx%.0f window", Utf8(ClassOf(hwnd)).c_str(),
           (rect.right - rect.left) / scale, (rect.bottom - rect.top) / scale);
  return text;
}

// The key window, as AppKit's class on the Mac.
std::string KeyName(HWND key) {
  if (!key) {
    return "none";
  }
  if (BrowserIndex(key) >= 0) {
    return "BrowserCoreNSWindow";
  }
  if (IsGpui(key)) {
    // An overlay is shown owned by its browser window; Settings is a window of its own, as on the
    // Mac, where it's an NSWindow and an overlay an NSPanel.
    return GetWindow(key, GW_OWNER) ? "GPUIPanel" : "GPUIWindow";
  }
  // Chrome's own windows, such as its dialogs, are views' on the Mac too.
  if (IsChromium(key)) {
    return "NativeWidgetMacNSWindow";
  }
  return Utf8(ClassOf(key));
}

// The view with the keyboard, as AppKit's class on the Mac. With none, the key window has it.
std::string FocusName(HWND focus, HWND key) {
  if (!focus) {
    return key ? KeyName(key) : "none";
  }
  if (IsGpui(focus)) {
    return GetAncestor(focus, GA_ROOT) == focus ? "GPUIView" : "AccessKitSubclassOfGPUIView";
  }
  if (ClassOf(focus) == L"BrowserUI") {
    return "AccessKitSubclassOfGPUIView";
  }
  if (BrowserIndex(focus) >= 0) {
    return "BrowserCoreNSWindow";
  }
  if ((IsChromium(focus) && GetParent(focus)) || ClassOf(focus) == L"Chrome_RenderWidgetHostHWND") {
    return "RenderWidgetHostViewCocoa";
  }
  if (IsChromium(focus)) {
    return "BridgedContentView";
  }
  return Utf8(ClassOf(focus));
}

// What the UI's field with the keys has marked, as "start,length" in UTF-16, or "none".
std::string Marked() {
  HWND focus = GetFocus();
  size_t start = 0, length = 0;
  if (!focus || !IsGpui(focus) || !browser_drive_marked(focus, &start, &length)) {
    return "none";
  }
  return std::to_string(start) + "," + std::to_string(length);
}

void ReportDone(long number) {
  HWND key = GetActiveWindow();
  DWORD process = 0;
  GetWindowThreadProcessId(GetForegroundWindow(), &process);
  fprintf(stderr, "browser: drive %ld done, key %s, focus %s, active %d, marked %s\n", number,
          KeyName(key).c_str(), FocusName(GetFocus(), key).c_str(),
          process == GetCurrentProcessId(), Marked().c_str());
}

// The scripted input method, while ime is on, and what it composes, if anything.
bool g_ime_on = false;
std::optional<std::string> g_composition;

// Adds the step that handles |key| as drive.mm's scripted input method does, with the calls a
// Japanese one makes in the UI's field with the keys: letters compose, space converts them all,
// return commits and escape cancels. Windows' own input methods can't be scripted, and the runners
// have no Japanese one, so it calls GPUI's input handler as Linux's driver does. Returns 1 for a key
// it took, 0 for one it leaves to the window, as one with a shortcut's modifiers, and -1 for one it
// would compose in a page, whose input method is Chromium's, which the driver can't type through:
// as plain keys it would pass for a composition.
int AddImeKey(const std::string& key, int modifiers) {
  const bool letter = key.size() == 1 && key[0] >= 'a' && key[0] <= 'z';
  if (!g_ime_on || ((modifiers | g_flags) & (kCtrl | kAlt)) || !(g_composition || letter)) {
    return 0;
  }
  HWND focus = GetFocus();
  if (!focus || !IsGpui(focus)) {
    return -1;
  }
  const std::string composed = g_composition.value_or("");
  if (key == "return") {
    g_composition.reset();
    g_steps.push_back({[=] { browser_drive_commit(focus, composed.c_str()); }});
  } else if (key == "escape") {
    g_composition.reset();
    g_steps.push_back({[=] { browser_drive_compose(focus, "", 0, 0, false); }});
  } else if (key == "space") {
    // 日本語, selected whole, as a real one selects its conversion.
    const std::string converted = "\xE6\x97\xA5\xE6\x9C\xAC\xE8\xAA\x9E";
    g_composition = converted;
    g_steps.push_back({[=] { browser_drive_compose(focus, converted.c_str(), 0, 3, true); }});
  } else if (letter) {
    const std::string text = composed + key;
    g_composition = text;
    g_steps.push_back(
        {[=] { browser_drive_compose(focus, text.c_str(), text.size(), text.size(), true); }});
  }
  return 1;
}

bool Unsupported(const std::string& action) {
  fprintf(stderr, "browser: drive: unsupported %s\n", action.c_str());
  return false;
}

// The drag dragover holds: what it carries, the window whose target has it and that target.
IDataObject* g_drag = nullptr;
HWND g_drag_view = nullptr;
IDropTarget* g_drag_target = nullptr;

// A drag's data as another app's would carry |kind| |value|: a file at the path as Explorer drags
// one, a link as browsers do, with its text, or text. Null for another kind.
IDataObject* DragData(const std::string& kind, const std::string& value) {
  if (kind != "file" && kind != "url" && kind != "text") {
    return nullptr;
  }
  IDataObject* data = nullptr;
  if (FAILED(SHCreateDataObject(nullptr, 0, nullptr, nullptr, IID_PPV_ARGS(&data)))) {
    return nullptr;
  }
  const auto put = [data](UINT format, const void* bytes, size_t size) {
    HGLOBAL memory = GlobalAlloc(GMEM_MOVEABLE, size);
    memcpy(GlobalLock(memory), bytes, size);
    GlobalUnlock(memory);
    FORMATETC wanted{static_cast<CLIPFORMAT>(format), nullptr, DVASPECT_CONTENT, -1, TYMED_HGLOBAL};
    STGMEDIUM medium{};
    medium.tymed = TYMED_HGLOBAL;
    medium.hGlobal = memory;
    if (FAILED(data->SetData(&wanted, &medium, TRUE))) {
      GlobalFree(memory);
    }
  };
  const std::wstring wide = Wide(value);
  const size_t size = (wide.size() + 1) * sizeof(wchar_t);
  if (kind == "file") {
    // DROPFILES, then the paths, each ending in a null, and one more null.
    std::vector<BYTE> files(sizeof(DROPFILES) + size + sizeof(wchar_t), 0);
    auto* header = reinterpret_cast<DROPFILES*>(files.data());
    header->pFiles = sizeof(DROPFILES);
    header->fWide = TRUE;
    memcpy(files.data() + sizeof(DROPFILES), wide.c_str(), size);
    put(CF_HDROP, files.data(), files.size());
  } else {
    if (kind == "url") {
      put(RegisterClipboardFormatW(CFSTR_INETURLW), wide.c_str(), size);
    }
    put(CF_UNICODETEXT, wide.c_str(), size);
  }
  return data;
}

// Ends the drag, letting go of what it carries.
void EndDrag() {
  if (g_drag) {
    g_drag->Release();
  }
  g_drag = nullptr;
  g_drag_view = nullptr;
  g_drag_target = nullptr;
}

// Moves the drag to |x|, |y| of the driven window, starting it with what |words| say it carries if
// there's none, and hands it to the target there as Windows would: the one registered on the
// nearest window up from the pointer's. Logs the window as the Mac names the UI's view, and
// whether its target takes the drag. False if it can't: no drag, or another's target there.
bool DragOver(double x, double y, std::istringstream& words, bool* taken) {
  if (!g_drag) {
    std::string kind, value;
    words >> kind;
    std::getline(words >> std::ws, value);
    g_drag = DragData(kind, value);
    if (!g_drag) {
      return false;
    }
  }
  Target target;
  if (!TargetAt(x, y, &target)) {
    return false;
  }
  HWND view = nullptr;
  IDropTarget* drop = nullptr;
  for (HWND hwnd = target.hwnd; hwnd && !view;
       hwnd = (GetWindowLongPtrW(hwnd, GWL_STYLE) & WS_CHILD) ? GetParent(hwnd) : nullptr) {
    drop = static_cast<IDropTarget*>(host_drop_target(hwnd));
    // OLE keeps a window's registered target in this property.
    if (drop || GetPropW(hwnd, L"OleDropTargetInterface")) {
      view = hwnd;
    }
  }
  if (view && !drop) {
    fprintf(stderr, "browser: drive: unsupported drag over %s\n", Utf8(ClassOf(view)).c_str());
    return false;
  }
  const POINTL at{target.screen.x, target.screen.y};
  DWORD effect = DROPEFFECT_COPY | DROPEFFECT_LINK | DROPEFFECT_MOVE;
  if (view != g_drag_view) {
    if (g_drag_target) {
      g_drag_target->DragLeave();
    }
    g_drag_view = view;
    g_drag_target = drop;
    if (drop) {
      drop->DragEnter(g_drag, MK_LBUTTON, at, &effect);
    }
  } else if (drop) {
    drop->DragOver(MK_LBUTTON, at, &effect);
  }
  *taken = drop && effect != DROPEFFECT_NONE;
  fprintf(stderr, "browser: drag over %s %s\n", view ? "HostUIView" : "nothing",
          *taken ? "taken" : "refused");
  return true;
}

// Adds the steps of |line|'s command. Returns false for one it can't do.
bool Run(const std::string& line) {
  std::istringstream words(line);
  std::string action;
  words >> action;
  // What follows the command's name, for those that take text.
  const std::string rest =
      line.size() > action.size() ? line.substr(action.size() + 1) : std::string();
  // The pointer's and text's input comes with no Ctrl the last key left down (LetGoKeyCtrl), so a
  // click after ⌘T is no Ctrl-click. A key takes it from there, and flags lets it go itself.
  static const char* const kInput[] = {
      "click",         "rightclick",     "middleclick", "doubleclick", "clickkey",
      "rightclickkey", "doubleclickkey", "drag",        "dragover",    "dragdrop",
      "dragleave",     "press",          "dragto",      "release",     "rightpress",
      "rightdragto",   "rightrelease",   "move",        "scroll",      "swipe",
      "wheel",         "keydown",        "keyup",       "type",        "ime",
      "insert",        "choose",         "dismiss"};
  for (const char* input : kInput) {
    if (action == input) {
      g_steps.push_back({[] { LetGoKeyCtrl(); }});
      break;
    }
  }
  if (action == "pageswipe" || action == "display" || action == "hide" || action == "backdrop" ||
      action == "openmenu") {
    return Unsupported(action);
  }
  // As the Mac's driver presses the green button: the window says it went once it has resized.
  if (action == "fullscreen") {
    const bc_id window = host_browser_window_id(g_driven);
    if (!window) {
      return false;
    }
    bc_window_toggle_full_screen(window);
    return true;
  }
  // At once, as the Mac's driver hands its drags to AppKit's views, since nothing is posted.
  if (action == "dragover" || action == "dragdrop") {
    double x, y;
    bool taken = false;
    if (!(words >> x >> y) || !DragOver(x, y, words, &taken)) {
      return false;
    }
    if (action == "dragdrop") {
      // The pointer let go where the target refused the drag cancels it, as OLE does.
      bool dropped = false;
      if (taken) {
        Target target;
        TargetAt(x, y, &target);
        DWORD effect = DROPEFFECT_COPY | DROPEFFECT_LINK | DROPEFFECT_MOVE;
        g_drag_target->Drop(g_drag, MK_LBUTTON, {target.screen.x, target.screen.y}, &effect);
        dropped = effect != DROPEFFECT_NONE;
      } else if (g_drag_target) {
        g_drag_target->DragLeave();
      }
      fprintf(stderr, "browser: drag %s\n", dropped ? "dropped" : "refused");
      EndDrag();
    }
    return true;
  }
  if (action == "dragleave") {
    if (g_drag_target) {
      g_drag_target->DragLeave();
    }
    EndDrag();
    return true;
  }
  if (action == "window") {
    size_t number;
    if (!(words >> number) || number == 0 || !Browser(number - 1)) {
      return false;
    }
    g_driven = number - 1;
    fprintf(stderr, "browser: drive window %zu\n", number);
    // In front of the app's other browser windows with the keyboard, as a click on it does, but
    // never over another app's.
    HWND root = Driven();
    for (size_t index = 0; index < 64; index++) {
      HWND other = Browser(index);
      if (other && other != root) {
        SetWindowPos(other, root, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
      }
    }
    SetActiveWindow(root);
    return true;
  }
  // Before the window's lookup, which finds no window out of sight.
  if (action == "minimize") {
    HWND root = static_cast<HWND>(host_browser_window(g_driven));
    if (!root) {
      // A minimized window isn't visible to host_browser_window; find it among the app's.
      for (HWND top = GetTopWindow(nullptr); top; top = GetWindow(top, GW_HWNDNEXT)) {
        if (Ours(top) && IsIconic(top) && FindWindowExW(top, nullptr, L"BrowserUI", nullptr)) {
          root = top;
          break;
        }
      }
    }
    if (!root) {
      return false;
    }
    ShowWindow(root, IsIconic(root) ? SW_SHOWNOACTIVATE : SW_SHOWMINNOACTIVE);
    return true;
  }
  // The windows the app has over its browser windows that are on screen, by their own visibility:
  // its overlays, its menu and Chromium's bubbles, each owned by its browser window.
  if (action == "overlays") {
    std::string shown;
    for (HWND top = GetTopWindow(nullptr); top; top = GetWindow(top, GW_HWNDNEXT)) {
      if (Ours(top) && GetWindow(top, GW_OWNER) && IsWindowVisible(top)) {
        RECT rect;
        GetWindowRect(top, &rect);
        const double scale = Scale(top);
        char text[160];
        snprintf(text, sizeof text, "%s%s %.0fx%.0f at %.0f,%.0f", shown.empty() ? "" : ", ",
                 Utf8(ClassOf(top)).c_str(), (rect.right - rect.left) / scale,
                 (rect.bottom - rect.top) / scale, rect.left / scale, rect.top / scale);
        shown += text;
      }
    }
    fprintf(stderr, "browser: overlays shown %s\n", shown.empty() ? "none" : shown.c_str());
    return true;
  }
  // Shows each of the UI's windows over a browser window that's hidden, as Windows shows those a
  // window owns as it comes back from minimized: none the UI has closed may come back (#1346).
  if (action == "reshow") {
    std::vector<HWND> hidden;
    for (HWND top = GetTopWindow(nullptr); top; top = GetWindow(top, GW_HWNDNEXT)) {
      if (Ours(top) && IsGpui(top) && GetWindow(top, GW_OWNER) && !IsWindowVisible(top)) {
        hidden.push_back(top);
      }
    }
    for (HWND hwnd : hidden) {
      ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    fprintf(stderr, "browser: reshown %zu\n", hidden.size());
    return true;
  }
  if (!Driven()) {
    return false;
  }
  if (action == "click" || action == "rightclick" || action == "middleclick" ||
      action == "doubleclick" || action == "clickkey" || action == "rightclickkey" ||
      action == "doubleclickkey") {
    double x, y;
    if (!(words >> x >> y)) {
      return false;
    }
    Target target;
    const bool in_key = action.size() > 3 && action.compare(action.size() - 3, 3, "key") == 0;
    if (!(in_key ? TargetInKey(x, y, &target) : TargetAt(x, y, &target))) {
      return false;
    }
    const UINT button = action.rfind("right", 0) == 0    ? MK_RBUTTON
                        : action.rfind("middle", 0) == 0 ? MK_MBUTTON
                                                         : MK_LBUTTON;
    AddClick(target, button, in_key ? 0 : Modifiers(words), action.rfind("double", 0) == 0);
    return true;
  }
  if (action == "drag") {
    double x, y, x2, y2;
    Target from;
    if (!(words >> x >> y >> x2 >> y2) || !TargetAt(x, y, &from)) {
      return false;
    }
    const Target to = At(from.hwnd, OnScreen(Driven(), x2, y2));
    g_steps.push_back({[=] { Move(from, 0, 0); }});
    g_steps.push_back({[=] { Press(from, MK_LBUTTON, 0, false); }});
    g_steps.push_back({[=] { Move(to, 0, MK_LBUTTON); }});
    g_steps.push_back({[=] { Release(to, MK_LBUTTON, 0); }});
    return true;
  }
  // A drag in the window with the keyboard, such as Settings', as clickkey clicks there: halfway
  // first, so the view sees the drag start before it ends.
  if (action == "dragkey") {
    double x, y, x2, y2;
    Target from;
    if (!(words >> x >> y >> x2 >> y2) || !TargetInKey(x, y, &from)) {
      return false;
    }
    HWND key = GetActiveWindow();
    const Target half = At(from.hwnd, OnScreen(key, (x + x2) / 2, (y + y2) / 2));
    const Target to = At(from.hwnd, OnScreen(key, x2, y2));
    g_steps.push_back({[=] { Move(from, 0, 0); }});
    g_steps.push_back({[=] { Press(from, MK_LBUTTON, 0, false); }});
    g_steps.push_back({[=] { Move(half, 0, MK_LBUTTON); }});
    g_steps.push_back({[=] { Move(to, 0, MK_LBUTTON); }});
    g_steps.push_back({[=] { Release(to, MK_LBUTTON, 0); }});
    return true;
  }
  if (action == "press" || action == "dragto" || action == "release" || action == "rightpress" ||
      action == "rightdragto" || action == "rightrelease") {
    double x, y;
    if (!(words >> x >> y)) {
      return false;
    }
    const bool right = action.rfind("right", 0) == 0;
    const std::string step = right ? action.substr(5) : action;
    const UINT button = right ? MK_RBUTTON : MK_LBUTTON;
    // The keys named after the point, such as ⌥ for a drag that drops a copy.
    const int keys = Modifiers(words);
    if (step == "press") {
      Target target;
      if (!TargetAt(x, y, &target)) {
        return false;
      }
      g_pressed = target.hwnd;
      g_steps.push_back({[=] { Move(target, 0, 0); }});
      g_steps.push_back({[=] { Press(target, button, keys, false); }});
      return true;
    }
    Target target;
    if (g_pressed && IsWindow(g_pressed)) {
      target = At(g_pressed, OnScreen(Driven(), x, y));
    } else if (!TargetAt(x, y, &target)) {
      return false;
    }
    if (step == "dragto") {
      g_steps.push_back({[=] { Move(target, keys, button); }});
    } else {
      g_pressed = nullptr;
      g_steps.push_back({[=] { Release(target, button, keys); }});
    }
    return true;
  }
  if (action == "move") {
    double x, y;
    Target target;
    if (!(words >> x >> y) || !TargetAt(x, y, &target)) {
      return false;
    }
    g_steps.push_back({[=] { Move(target, 0, 0); }});
    return true;
  }
  // A trackpad's scrolls, as a precision touchpad sends them: wheel messages of a pixel's worth
  // each, which Chrome scrolls 100 pixels by for every notch of 120, at the Mac driver's pace.
  if (action == "scroll" || action == "swipe" || action == "wheel") {
    double x, y, dx = 0, dy = 0;
    int count = 1, pause = 0;
    if (!(words >> x >> y)) {
      return false;
    }
    if (action == "scroll" && !(words >> dy >> count)) {
      return false;
    }
    if (action == "swipe" && !(words >> dx >> dy >> count)) {
      return false;
    }
    if (action == "swipe") {
      words >> pause;
    }
    if (action == "wheel" && !(words >> dx)) {
      return false;
    }
    Target target;
    if (count < 1 || !TargetAt(x, y, &target)) {
      return false;
    }
    // A mouse's tick is a notch; a trackpad's points, content moving up for scrolling down.
    const int across = action == "wheel" ? static_cast<int>(dx * WHEEL_DELTA)
                                         : static_cast<int>(std::lround(dx * 1.2));
    const int along = static_cast<int>(std::lround(-dy * 1.2));
    g_steps.push_back({[=] { Move(target, 0, 0); }});
    for (int event = 0; event < count; event++) {
      g_steps.push_back(
          {[=] {
             HoldKeys(0);
             const LPARAM at = MAKELPARAM(target.screen.x, target.screen.y);
             if (host_drive_pointer(WM_MOUSEWHEEL, target.screen.x, target.screen.y)) {
               return;
             }
             if (along) {
               PostMessageW(target.hwnd, WM_MOUSEWHEEL, MAKEWPARAM(MouseKeys(0, 0), along), at);
             }
             if (across) {
               PostMessageW(target.hwnd, WM_MOUSEHWHEEL, MAKEWPARAM(MouseKeys(0, 0), across), at);
             }
           },
           action == "wheel" ? 0u : 8u});
    }
    if (pause > 0) {
      g_steps.push_back({[] {}, static_cast<UINT>(pause)});
    }
    return true;
  }
  if (action == "resize") {
    double width, height;
    return words >> width >> height && ResizeContent(width, height);
  }
  if (action == "place") {
    double x, y, width, height;
    return words >> x >> y >> width >> height && PlaceFrame(x, y, width, height);
  }
  if (action == "order") {
    std::string order;
    for (HWND top = GetTopWindow(nullptr); top; top = GetWindow(top, GW_HWNDNEXT)) {
      const long index = IsWindowVisible(top) ? BrowserIndex(top) : -1;
      if (index < 0) {
        continue;
      }
      const std::string name = "window " + std::to_string(index + 1);
      order += " " + name;
      LogFrame(name.c_str(), top);
    }
    const long key = BrowserIndex(GetActiveWindow());
    fprintf(stderr, "browser: order%s, key %s\n", order.c_str(),
            key < 0 ? "none" : ("window " + std::to_string(key + 1)).c_str());
    return true;
  }
  if (action == "layout") {
    LogLayout(Driven());
    return true;
  }
  if (action == "state") {
    return true;
  }
  // The clipboard's text: the check machine's own, as drive.mm's pasteboard is the run's.
  if (action == "pasteboard") {
    std::wstring text;
    if (OpenClipboard(nullptr)) {
      if (HANDLE data = GetClipboardData(CF_UNICODETEXT)) {
        if (const auto* wide = static_cast<const wchar_t*>(GlobalLock(data))) {
          text = wide;
          GlobalUnlock(data);
        }
      }
      CloseClipboard();
    }
    fprintf(stderr, "browser: pasteboard text %s\n", Utf8(text).c_str());
    return true;
  }
  if (action == "title") {
    wchar_t title[512] = L"";
    GetWindowTextW(GetActiveWindow(), title, 512);
    fprintf(stderr, "browser: key window titled %s\n", Utf8(title).c_str());
    return true;
  }
  if (action == "front") {
    // As other apps list windows, such as WebRTC's window capture, which T3 Code's uses: an owned
    // window, as each overlay is, isn't one unless it has the taskbar's style.
    HWND first = nullptr;
    for (HWND top = GetTopWindow(nullptr); top && !first; top = GetWindow(top, GW_HWNDNEXT)) {
      const bool listed = !GetWindow(top, GW_OWNER) ||
                          (GetWindowLongPtrW(top, GWL_EXSTYLE) & WS_EX_APPWINDOW);
      first = Ours(top) && IsWindowVisible(top) && listed ? top : nullptr;
    }
    const std::string key = Describe(GetActiveWindow());
    fprintf(stderr, "browser: front %s, main %s, key %s, ax focused %s, ax main %s\n",
            Describe(first).c_str(), key.c_str(), key.c_str(), key.c_str(), key.c_str());
    return true;
  }
  if (action == "menu" || action == "menubar") {
    HWND ui_view = FindWindowExW(Driven(), nullptr, L"BrowserUI", nullptr);
    return g_menus && ui_view && g_menus(ui_view, rest.c_str(), action == "menubar");
  }
  if (action == "choose") {
    return host_menu_window_choose(rest.c_str()) || host_choose(rest.c_str());
  }
  if (action == "dismiss") {
    return host_menu_window_dismiss();
  }
  if (action == "agentspace") {
    std::string agent, name;
    if (!(words >> agent) || !std::getline(words >> std::ws, name) || name.empty()) {
      return false;
    }
    const bc_id space = bc_space_create_for_agent(name.c_str(), agent.c_str(), name.c_str());
    fprintf(stderr, "browser: made space %llu for agent %s\n", (unsigned long long)space,
            agent.c_str());
    return true;
  }
  if (action == "agentmayact") {
    std::string agent;
    bc_id tab;
    if (!(words >> agent >> tab)) {
      return false;
    }
    std::string what;
    if (!(words >> what)) {
      what = "navigate";
    }
    char reason[256];
    const bool may = bc_agent_may_act(agent.c_str(), tab, what.c_str(), reason, sizeof reason);
    fprintf(stderr, "browser: agent %s may act on %llu: %d\n", agent.c_str(),
            (unsigned long long)tab, may);
    if (!may) {
      fprintf(stderr, "browser: agent %s refused %llu: %s\n", agent.c_str(),
              (unsigned long long)tab, reason);
    }
    return true;
  }
  if (action == "agentrelease") {
    std::string agent;
    bc_id tab;
    if (!(words >> agent >> tab)) {
      return false;
    }
    const bool released = bc_agent_release_tab(agent.c_str(), tab);
    fprintf(stderr, "browser: agent %s released %llu: %d\n", agent.c_str(), (unsigned long long)tab,
            released);
    return true;
  }
  if (action == "agentdisconnect") {
    std::string agent;
    if (!(words >> agent)) {
      return false;
    }
    const size_t ended = bc_agent_disconnected(agent.c_str());
    fprintf(stderr, "browser: agent %s disconnected, %zu sessions ended\n", agent.c_str(), ended);
    return true;
  }
  if (action == "takeback") {
    bc_id tab;
    if (!(words >> tab)) {
      return false;
    }
    const bool took = bc_tab_take_back(tab);
    fprintf(stderr, "browser: took back %llu: %d\n", (unsigned long long)tab, took);
    return true;
  }
  if (action == "age") {
    double seconds;
    if (!(words >> seconds)) {
      return false;
    }
    bc_age_tabs_for_testing(seconds);
    return true;
  }
  if (action == "keydown" || action == "keyup") {
    std::string key, repeat;
    words >> key >> repeat;
    int modifiers = 0;
    const UINT vk = VirtualKey(key, &modifiers);
    if (!vk) {
      return false;
    }
    const bool down = action == "keydown";
    g_steps.push_back({[=] { PostKey(vk, modifiers, down, repeat == "repeat"); }});
    // The key stays as it is, without the modifiers it was typed with.
    g_steps.push_back({[] { HoldKeys(0); }});
    return true;
  }
  if (action == "key") {
    std::string key;
    words >> key;
    int modifiers = Modifiers(words, key);
    if (const int ime = AddImeKey(key, modifiers)) {
      return ime > 0;
    }
    const UINT vk = VirtualKey(key, &modifiers);
    if (!vk) {
      return false;
    }
    AddKey(vk, modifiers);
    return true;
  }
  // The modifiers held from now, as their keys going down or up. ⌃ is held only for ⌃Tab, which
  // is Ctrl+Tab here, so it holds Ctrl, which the app watches for its release.
  if (action == "flags") {
    int modifiers = 0;
    std::string word;
    while (words >> word) {
      modifiers |= word == "cmd" || word == "ctrl" ? kCtrl
                   : word == "shift"               ? kShift
                   : word == "alt"                 ? kCtrl | kAlt
                                                   : 0;
    }
    g_steps.push_back({[=] {
      const int from = g_flags | (std::exchange(g_key_ctrl, false) ? kCtrl : 0);
      g_flags = 0;
      PostModifiers(from, modifiers);
      g_flags = modifiers;
      HoldKeys(0);
    }});
    return true;
  }
  if (action == "type") {
    for (char character : rest) {
      int modifiers = 0;
      const std::string key(1, character);
      const int ime = AddImeKey(key, 0);
      if (ime < 0) {
        return false;
      }
      if (ime > 0) {
        continue;
      }
      const UINT vk = VirtualKey(key, &modifiers);
      if (!vk) {
        return false;
      }
      AddKey(vk, modifiers);
    }
    return true;
  }
  if (action == "ime") {
    std::string state;
    words >> state;
    if (state != "on" && state != "off") {
      return false;
    }
    // As the commands before it have run, which the keys after it are read with.
    g_ime_on = state == "on";
    g_composition.reset();
    g_steps.push_back({[] {}});
    return true;
  }
  // Commits the text to the focused view as typed characters, as the emoji panel does.
  if (action == "insert") {
    const std::wstring text = Wide(rest);
    g_steps.push_back({[=] {
      for (wchar_t unit : text) {
        PostMessageW(KeyTarget(), WM_CHAR, unit, 1);
      }
    }});
    return true;
  }
  return false;
}

// Runs the next step, or the next command once the last one's are done.
void Next() {
  if (g_steps.empty()) {
    g_running = false;
    if (g_commands.empty()) {
      return;
    }
    auto [number, line] = g_commands.front();
    g_commands.pop_front();
    g_running = true;
    fprintf(stderr, "browser: drive %ld: %s\n", number, line.c_str());
    if (!Run(line)) {
      g_steps.clear();
      fprintf(stderr, "browser: drive %ld: can't do that\n", number);
    }
    // Done once the messages posted are handled, and what handling them posted. Twice, as GPUI
    // and Chromium post more as they handle some, such as the characters a key types.
    g_steps.push_back({[] {}});
    g_steps.push_back({[] {}});
    g_steps.push_back({[number] { ReportDone(number); }});
  }
  Step step = std::move(g_steps.front());
  g_steps.pop_front();
  step.run();
  if (step.wait) {
    SetTimer(g_driver, kStepTimer, step.wait, nullptr);
  } else {
    PostMessageW(g_driver, kStep, 0, 0);
  }
}

LRESULT CALLBACK DriverProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) {
  switch (message) {
    case kCommand: {
      auto* line = reinterpret_cast<std::string*>(lparam);
      g_commands.emplace_back(static_cast<long>(wparam), *line);
      delete line;
      if (!g_running) {
        Next();
      }
      return 0;
    }
    case kStep:
      Next();
      return 0;
    case WM_TIMER:
      if (wparam == kStepTimer) {
        KillTimer(hwnd, kStepTimer);
        Next();
      }
      return 0;
  }
  return DefWindowProcW(hwnd, message, wparam, lparam);
}

// Windows tells a window the pointer left it once it asks to hear so and the desktop's pointer
// isn't in it, which it never is for the driver's pointer: that leave is dropped, and the driver
// posts its own as its pointer leaves. Windows also moves the desktop's pointer, where it rests,
// over the window under it as a window shows or hides, as the link target's plate does: the
// page under it heard the pointer leave the link the driver's pointer was on. Only the driver's
// own moves reach the app's windows.
LRESULT CALLBACK DropRealPointer(int code, WPARAM wparam, LPARAM lparam) {
  auto* message = reinterpret_cast<MSG*>(lparam);
  if (code != HC_ACTION || wparam != PM_REMOVE) {
    return CallNextHookEx(nullptr, code, wparam, lparam);
  }
  if ((message->message == WM_MOUSELEAVE || message->message == WM_NCMOUSELEAVE) &&
      message->lParam != kDriverLeave && message->hwnd == g_hovered) {
    message->message = WM_NULL;
  } else if (message->message == WM_MOUSEMOVE) {
    const auto posted =
        std::ranges::find(g_posted_moves, std::pair(message->hwnd, message->lParam));
    if (posted != g_posted_moves.end()) {
      g_posted_moves.erase(g_posted_moves.begin(), posted + 1);
    } else {
      fprintf(stderr, "browser: drive: dropped the desktop's pointer moving at %d %d\n",
              static_cast<short>(LOWORD(message->lParam)),
              static_cast<short>(HIWORD(message->lParam)));
      message->message = WM_NULL;
    }
  }
  return CallNextHookEx(nullptr, code, wparam, lparam);
}

}  // namespace

// Each window that comes to the front or takes the keyboard, logged from inside the call that
// moves it, so the log shows which of the app's steps moved it, as no "drive done" line can.
LRESULT CALLBACK WatchKeyboard(int code, WPARAM wparam, LPARAM lparam) {
  if (code == HCBT_ACTIVATE || code == HCBT_SETFOCUS) {
    HWND hwnd = reinterpret_cast<HWND>(wparam);
    const std::string name = code == HCBT_ACTIVATE
                                 ? KeyName(hwnd)
                                 : FocusName(hwnd, GetAncestor(hwnd, GA_ROOT));
    fprintf(stderr, "browser: drive: %s %s\n", code == HCBT_ACTIVATE ? "activates" : "focuses",
            name.c_str());
  }
  return CallNextHookEx(nullptr, code, wparam, lparam);
}

extern "C" {

void drive_watch_menus(bool (*menus)(void* ui_view, const char* title, bool bar)) {
  g_menus = menus;
}

void drive_start(void) {
  WNDCLASSW window_class{};
  window_class.lpfnWndProc = DriverProc;
  window_class.hInstance = GetModuleHandleW(nullptr);
  window_class.lpszClassName = L"BrowserDriver";
  RegisterClassW(&window_class);
  g_driver = CreateWindowExW(0, window_class.lpszClassName, L"", 0, 0, 0, 0, 0, HWND_MESSAGE,
                             nullptr, window_class.hInstance, nullptr);
  SetWindowsHookExW(WH_GETMESSAGE, DropRealPointer, nullptr, GetCurrentThreadId());
  SetWindowsHookExW(WH_CBT, WatchKeyboard, nullptr, GetCurrentThreadId());
  HWND driver = g_driver;
  std::thread([driver] {
    HANDLE input = GetStdHandle(STD_INPUT_HANDLE);
    std::string pending;
    char buffer[1024];
    DWORD read = 0;
    long number = 0;
    while (ReadFile(input, buffer, sizeof buffer, &read, nullptr) && read > 0) {
      pending.append(buffer, read);
      for (size_t end; (end = pending.find('\n')) != std::string::npos;) {
        std::string line = pending.substr(0, end);
        pending.erase(0, end + 1);
        line.erase(line.find_last_not_of("\r") + 1);
        PostMessageW(driver, kCommand, ++number, reinterpret_cast<LPARAM>(new std::string(line)));
      }
    }
  }).detach();
}

}  // extern "C"
