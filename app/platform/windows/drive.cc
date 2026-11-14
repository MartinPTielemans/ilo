// Test driver for check.sh on Windows, as platform/mac/drive.mm is on the Mac. With BROWSER_DRIVE=1
// the app reads one command per line on stdin, in drive.mm's language, and posts the matching
// window messages to its own windows only: Chromium's browser windows and their pages, and GPUI's
// UI and overlays in and over them. Each window's own procedure handles them as it would the
// device's, so they go through the same hit testing, focus changes and shortcuts, without
// touching the desktop's pointer or keyboard. Modifiers are held in the UI thread's own keyboard
// state, which is what Chromium and GPUI read, never the system's.
//
// With BROWSER_DRIVE_REAL=1 as well, on a desktop that is the check's own, the pointer and the keys
// are real input instead, sent with SendInput as a hand's are, each once one of the app's windows
// is in front (#2544). Windows then routes them itself, through the mouse capture, the foreground
// window and its hit test, which posted messages skip: a drag in the theme picker moved the window
// for a hand and not for the driver until a probe sent real input (#2516). It takes the desktop's
// pointer and keyboard, so the driver refuses it outside CI unless BROWSER_DRIVE_OWN_DESKTOP=1 says
// the desktop is the check's. What has no real input to send stays as it is: another app's drag
// (dragover, dragdrop, dragleave), the emoji panel's text (insert), the scripted input method (ime
// on), and two fingers on a touchpad (scroll, swipe), whose own input SendInput can't make. So does
// the app's own drag once it hands it to Windows (a tab or a download dragged out), as Windows'
// drag loop dispatches none of the driver's steps: the stand-in takes the moves and the release.
//
// The commands are drive.mm's, with what Windows has: click, doubleclick, rightclick, middleclick,
// clickkey, doubleclickkey, rightclickkey, drag, dragkey, press, dragto, release, rightpress,
// rightdragto, rightrelease, move, key, keydown, keyup, flags, type, insert, scroll, swipe, wheel,
// resize, resizekey, place, minimize, window, order, layout, state, title, pasteboard, front, menu,
// menubar, choose, dismiss, agentspace, agentopen, agentmayact, agentrelease, agentdisconnect,
// takeback and age, pip, as Linux's finds Chromium's Picture in Picture window but with hit
// tests in place of drags, and Windows' own maximize, which the Mac's has no command for,
// dpi <percent>, which tells the browser window it moved to a display of that scale as Windows does
// (WM_DPICHANGED), keeping its size in pixels, and placement, which logs the window's maximized
// state and the frame it un-maximizes to as Windows keeps them; overlays, which lists the windows
// on screen over the app's browser windows; lives, which lists every window the host floats over
// them and where each is in its life; stack, which lists every app's windows over the driven
// one, front first; and reshow, which shows those of the UI's that are hidden again, as Windows
// does. window <n> behind drives a window without bringing it forward, as drive.mm's does. The
// Mac's modifiers are pressed as the app reads them off the Mac (keys.rs): cmd with Ctrl, ctrl with
// Alt, but Ctrl for ⌃Tab, and alt with both; a key's winalt holds Windows' own Alt, as the app's
// own views bind ⌥ off the Mac, and flags' winalt holds it with its key down and up, for Alt
// pressed alone. A click's or a drag's are held as a Windows user holds them, cmd with
// Ctrl and alt with Alt. A trackpad's scrolls, and its swipes over a page, are wheel messages with
// the pixels in their deltas, going the way drive.mm's go, which Chromium scrolls by. Its swipes
// over GPUI's view are handed to GPUI as Direct Manipulation hands it a precision touchpad's, with
// their touch and lift. choose works on ilo's menu, ilo's dialog and the alerts host_ask shows, as
// Linux's driver does, and dismiss on ilo's menu. savepanel <path> picks <path> in the dialog
// host_save_png shows, as drive.mm's does in the Mac's save panel, logging "browser: save panel
// answered", or "browser: no save panel" if none shows. tabpicture writes the picture a tab's drag
// shows for part of the window to a file and logs its size. dragover, dragdrop and dragleave hand a
// drag from another app, a shell data object carrying what Explorer or a browser would, straight to
// the drop target Windows would pick: the UI's, or over a page Chromium's; one over an overlay,
// whose target is GPUI's, logs as unsupported. A drag the app starts, a file's or a tab's, the
// driver carries the same way, from dragto to release, as Windows' own waits for the real pointer
// (#1583). ime on types as drive.mm's scripted input method does, in the UI's fields alone, as
// Linux's driver does. ime ja makes Microsoft's Japanese IME, where it's installed, the UI thread's
// input method, on in hiragana, and sends the keys typed until ime off without a shortcut's
// modifiers through Windows' input queue, to the window in front, which must be ilo's, so the IME
// composes them, in a page too: "nihongo" composes にほんご, space converts it to 日本語. ime ko does
// the same with Microsoft's Korean IME, in Hangul: "gksrmf" composes 한글 (#2457).
// fullscreen puts the window full screen or takes it out through the core, as Linux's driver does.
// realkey <key> [win] [ctrl] [shift] [alt] presses a key with Windows' own modifiers, unlike key's,
// as a hand does, through SendInput: Windows itself handles it, as it does Win+D, which no posted
// message stands in for. It goes to the desktop's foreground window, whichever app's that is, so
// the driver refuses it unless the desktop is the check's own (OwnDesktop).
// The others, pageswipe, displays, hide, backdrop and openmenu, Windows has nothing the driver can
// stand in for yet: each logs "browser: drive: unsupported <command>" before drive.mm's "can't do
// that", so a run fails at once.
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

#include <dwmapi.h>
#include <shlobj.h>

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <deque>
#include <functional>
#include <map>
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
const char* host_floating_name(void* hwnd);
void host_floating_lives(void (*each)(void* context, const char* name, void* hwnd, int life),
                         void* context);
bool host_save_dialog_answer(const char* path);
// The app's (src/main.rs), on GPUI's window |hwnd|, for the scripted input method.
bool browser_drive_compose(void* hwnd, const char* text, size_t start, size_t end, bool selected);
bool browser_drive_commit(void* hwnd, const char* text);
bool browser_drive_marked(void* hwnd, size_t* start, size_t* length);
// The app's, on GPUI's window |hwnd|, for two fingers as a precision touchpad's.
bool browser_drive_fingers(void* hwnd, float x, float y, float dx, float dy, int phase);
void* host_drop_target(void* hwnd);
void host_drive_drags(HRESULT (*drag)(IDataObject*, DWORD, DWORD*, POINT*));
void host_drive_real_input(void);
bool host_drive_window_part(void* hwnd, double x, double y, double width, double height,
                            const char* path, long* pixels_wide, long* pixels_high);
void* host_drive_little_tray(void);
bool host_drive_little_tray_click(void);
bool host_drive_little_tray_place(RECT* icon, RECT* area);
}

namespace {

UINT const kCommand = WM_APP + 1;
UINT const kStep = WM_APP + 2;
UINT_PTR const kStepTimer = 1;
// The step timer's backstop, posted from a pool thread (StepLate).
UINT const kStepLate = WM_APP + 3;
// Marks the pointer leaving that the driver posts, which its hook lets through.
LPARAM const kDriverLeave = 0x44524956;

// The Mac's modifiers as Windows' keys hold them.
int const kCtrl = 1;
int const kShift = 2;
int const kAlt = 4;
// Windows' own Alt, held with its key's messages, as a keyboard sends Alt pressed alone (flags
// winalt). The Mac has no key the app reads as it: its ⌥ is Ctrl+Alt here.
int const kAltKey = 8;

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
// Real input (#2544): whether the pointer and the keys are sent with SendInput, as a hand's are.
bool g_real = false;
bool Extended(UINT vk);
// Marks the driver's real events in their extra information, with each one's number below it, in
// its low 32 bits, which are all a mouse event keeps of it.
ULONG_PTR const kRealTag = 0x49000000;
ULONG_PTR const kRealNumber = 0x00FFFFFF;
// The number of the last real event sent, and of the last the UI thread has taken off its queue.
ULONG_PTR g_real_sent = 0;
ULONG_PTR g_real_heard = 0;
// The last real key sent, which the keyboard hook matches, as a key's hook has no extra
// information.
UINT g_real_vk = 0;
bool g_real_up = false;
HHOOK g_real_mouse_hook = nullptr;
HHOOK g_real_key_hook = nullptr;
// The window the step's real input is for, which BringInFront brings in front: a hand's click on
// the app's taskbar button brings back the window it last used, such as Settings, not always the
// browser window.
HWND g_real_aim = nullptr;
// The last real move: where to, its number, and whether it has been sent again.
POINT g_real_move{};
ULONG_PTR g_real_move_number = 0;
bool g_real_move_again = false;
void RealMoveTo(POINT at, bool again = false);
// The real input a step sends after it brought the app in front, held for the steps that send it
// once the app has had a turn, while it's true.
std::vector<INPUT> g_real_held;
bool g_real_holding = false;
bool g_real_resending = false;

// Whether the desktop is the check's own, which real input may take: a GitHub runner's, or one
// BROWSER_DRIVE_OWN_DESKTOP=1 says is. Never a developer's, whose pointer and keys it would take.
bool EnvIs(const wchar_t* name, const wchar_t* value) {
  wchar_t got[16] = L"";
  return GetEnvironmentVariableW(name, got, 16) > 0 && wcscmp(got, value) == 0;
}
bool OwnDesktop() {
  return EnvIs(L"GITHUB_ACTIONS", L"true") || EnvIs(L"BROWSER_DRIVE_OWN_DESKTOP", L"1");
}
// The last press, a click's or a drag's: its window, place on screen, button and time.
struct Click {
  HWND hwnd = nullptr;
  POINT at{};
  UINT button = 0;
  ULONGLONG time = 0;
} g_last_click;
// The app's menus, for menu and menubar: chooses the item titled |title| in the browser window
// whose UI view is |ui_view|, or with |bar| logs the menu so titled.
bool (*g_menus)(void* ui_view, const char* title, bool bar) = nullptr;
// The app's dialog, for choose: answers with the button |label| names in the one that shows.
bool (*g_dialog)(const char* label) = nullptr;

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

// Where real input landed, as check.sh names it: one of the host's floating windows, such as the
// theme picker or the menu, else a browser window's page or UI, else the window's class.
std::string Landed(HWND hwnd) {
  const HWND root = GetAncestor(hwnd, GA_ROOT);
  if (const char* floating = host_floating_name(root)) {
    return floating;
  }
  if (BrowserIndex(root) >= 0) {
    // A page's input goes to its pane, or to the window Chromium keeps in it for the page's
    // accessibility, the one under the pointer.
    for (HWND at = hwnd; at && at != root; at = GetParent(at)) {
      if (IsChromium(at) || ClassOf(at) == L"Chrome_RenderWidgetHostHWND") {
        return "a browser window's page";
      }
    }
    return "a browser window's UI";
  }
  return Utf8(ClassOf(root));
}

// The driver's real events as the UI thread takes them off its queue, before a hook of the host's
// can take one and pass it no further, as the menu's do. Each press and key logs where it landed,
// for the list of what real input reached (#2544).
LRESULT CALLBACK HearRealPointer(int code, WPARAM wparam, LPARAM lparam) {
  const auto* hook = reinterpret_cast<MOUSEHOOKSTRUCT*>(lparam);
  if (code == HC_ACTION && (hook->dwExtraInfo & ~kRealNumber) == kRealTag) {
    g_real_heard = (std::max)(g_real_heard, hook->dwExtraInfo & kRealNumber);
    switch (wparam) {
      case WM_LBUTTONDOWN:
      case WM_RBUTTONDOWN:
      case WM_MBUTTONDOWN:
      case WM_LBUTTONDBLCLK:
      case WM_NCLBUTTONDOWN:
      case WM_NCRBUTTONDOWN:
        fprintf(stderr, "browser: drive: real press in %s\n", Landed(hook->hwnd).c_str());
        break;
      case WM_LBUTTONUP: {
        // With the keys the page reads the click's from, as the thread's input has them then: a
        // link clicked with ⌥ or ⌘ opens where they say only if they're still held (#2544).
        std::string held;
        for (const auto& [vk, name] : {std::pair{VK_CONTROL, "ctrl"}, std::pair{VK_SHIFT, "shift"},
                                       std::pair{VK_MENU, "alt"}}) {
          if (GetKeyState(vk) & 0x8000) {
            held += held.empty() ? name : std::string(" ") + name;
          }
        }
        fprintf(stderr, "browser: drive: real release in %s, holding %s\n",
                Landed(hook->hwnd).c_str(), held.empty() ? "nothing" : held.c_str());
        break;
      }
      case WM_MOUSEWHEEL:
      case WM_MOUSEHWHEEL: {
        // Once for each window a run of the wheel's events lands in.
        static HWND last = nullptr;
        if (std::exchange(last, hook->hwnd) != hook->hwnd) {
          fprintf(stderr, "browser: drive: real wheel in %s, %s\n", Landed(hook->hwnd).c_str(),
                  Utf8(ClassOf(hook->hwnd)).c_str());
        }
        break;
      }
    }
  }
  return CallNextHookEx(nullptr, code, wparam, lparam);
}

LRESULT CALLBACK HearRealKey(int code, WPARAM wparam, LPARAM lparam) {
  const bool up = (static_cast<DWORD>(lparam) & 0x80000000u) != 0;
  if (code == HC_ACTION && g_real_vk && wparam == g_real_vk && up == g_real_up) {
    g_real_heard = g_real_sent;
    if (!up && wparam != VK_CONTROL && wparam != VK_SHIFT && wparam != VK_MENU) {
      const HWND focus = GetFocus();
      fprintf(stderr, "browser: drive: real key %#x in %s\n", static_cast<unsigned>(wparam),
              Landed(focus ? focus : GetActiveWindow()).c_str());
    }
  }
  return CallNextHookEx(nullptr, code, wparam, lparam);
}

// Whether one of the app's windows is in front, where real input goes.
bool OursInFront() {
  DWORD process = 0;
  GetWindowThreadProcessId(GetForegroundWindow(), &process);
  return process == GetCurrentProcessId();
}

// Brings the driven window in front when another app is, as a run that launches the app again
// finds the runner's console there, which a process that isn't in front can't take the foreground
// from. Windows lets it once input has come since, so an Alt pressed and let go goes first, to
// the app in front, as automation tools do: Alt alone does nothing in a console. The window comes
// back as a hand's taskbar click or Alt+Tab brings it, with its last active window, such as the
// quit dialog it owns, rather than itself.
void BringInFront() {
  const HWND front = GetForegroundWindow();
  const HWND aim = g_real_aim && IsWindowVisible(g_real_aim) ? g_real_aim : Driven();
  const HWND root = GetAncestor(aim, GA_ROOT);
  if (!root) {
    return;
  }
  const HWND popup = GetLastActivePopup(root);
  INPUT alt[2]{};
  alt[0].type = alt[1].type = INPUT_KEYBOARD;
  alt[0].ki.wVk = alt[1].ki.wVk = VK_MENU;
  alt[1].ki.dwFlags = KEYEVENTF_KEYUP;
  SendInput(2, alt, sizeof alt[0]);
  SetForegroundWindow(popup && IsWindowVisible(popup) ? popup : root);
  fprintf(stderr, "browser: drive: brings the app in front of another app's %s: %s\n",
          front ? Utf8(ClassOf(front)).c_str() : "nothing", OursInFront() ? "in front" : "not");
}

// Sends |input| as the driver's real event, once one of the app's windows is in front: it would
// land in another app's otherwise, so it's withheld and logged, and the step it was for fails.
void SendReal(INPUT input) {
  // A move goes to whatever window is under the pointer, in front or not, as a hand's does, and a
  // key or a button let go is sent wherever the foreground is, so none is left held down.
  const bool move = input.type == INPUT_MOUSE &&
                    !(input.mi.dwFlags & ~(MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE |
                                           MOUSEEVENTF_VIRTUALDESK));
  const bool let_go =
      input.type == INPUT_KEYBOARD
          ? (input.ki.dwFlags & KEYEVENTF_KEYUP) != 0
          : (input.mi.dwFlags & (MOUSEEVENTF_LEFTUP | MOUSEEVENTF_RIGHTUP | MOUSEEVENTF_MIDDLEUP)) != 0;
  if (g_real_holding) {
    g_real_held.push_back(input);
    return;
  }
  if (!move && !let_go && !OursInFront()) {
    BringInFront();
    if (!OursInFront()) {
      const HWND front = GetForegroundWindow();
      fprintf(stderr, "browser: drive: real input withheld, another app's %s is in front\n",
              front ? Utf8(ClassOf(front)).c_str() : "nothing");
      return;
    }
    // The app takes its turn first, as it does before a hand's next key: a window coming in
    // front hands the keyboard on as it settles, such as to a dialog shown meanwhile, and a key
    // sent at once went to the page under the dialog instead (#2544). What the step sends after
    // this is held with it. Held once: sent again, it goes at once.
    if (!g_real_resending) {
      g_real_holding = true;
      g_real_held.push_back(input);
      g_steps.push_front({[] {
        g_real_holding = false;
        // Where the app's turn left the keyboard, for a check that finds the input went elsewhere.
        fprintf(stderr, "browser: drive: after the app's turn, the keyboard is in %s\n",
                Landed(GetForegroundWindow()).c_str());
        g_real_resending = true;
        for (const INPUT& held : std::exchange(g_real_held, {})) {
          SendReal(held);
        }
        g_real_resending = false;
      }});
      g_steps.push_front({[] {}});
      g_steps.push_front({[] {}});
      return;
    }
  }
  // Hooks set later are called first, as the menu's are once it shows: the driver's go in front
  // again, so they hear each event before one of the host's takes it.
  const DWORD thread = GetCurrentThreadId();
  if (g_real_mouse_hook) {
    UnhookWindowsHookEx(g_real_mouse_hook);
  }
  if (g_real_key_hook) {
    UnhookWindowsHookEx(g_real_key_hook);
  }
  g_real_mouse_hook = SetWindowsHookExW(WH_MOUSE, HearRealPointer, nullptr, thread);
  g_real_key_hook = SetWindowsHookExW(WH_KEYBOARD, HearRealKey, nullptr, thread);
  const ULONG_PTR number = ++g_real_sent & kRealNumber;
  if (input.type == INPUT_KEYBOARD) {
    input.ki.dwExtraInfo = kRealTag | number;
    g_real_vk = input.ki.wVk;
    g_real_up = (input.ki.dwFlags & KEYEVENTF_KEYUP) != 0;
  } else {
    input.mi.dwExtraInfo = kRealTag | number;
    g_real_vk = 0;
  }
  if (SendInput(1, &input, sizeof input) != 1) {
    fprintf(stderr, "browser: drive: SendInput failed, error %lu\n", GetLastError());
    g_real_heard = number;
  } else if ((move || let_go) && !OursInFront()) {
    // Another app's window hears it, as it would a hand's: there's nothing of the app's to wait on.
    g_real_heard = number;
  }
}

// Waits for the UI thread to take the last real event off its queue, up to a second. On a timer,
// as Windows hands a thread its input before its timers, where a posted step would come first.
void AwaitReal(ULONGLONG since) {
  if (g_real_heard >= g_real_sent) {
    return;
  }
  if (GetTickCount64() - since > 1000) {
    // The first move after the app is brought in front can go unheard, while Windows still has
    // another app's window there: it's sent again, a pixel off and back, as Windows sends nothing
    // for a move to where the pointer is, which a hand's would then make.
    if (g_real_move_number > g_real_heard && !g_real_move_again) {
      g_real_move_again = true;
      const POINT at = g_real_move;
      fprintf(stderr, "browser: drive: real move to %ld,%ld not heard, sent again\n", at.x, at.y);
      RealMoveTo({at.x + 1, at.y}, true);
      RealMoveTo(at, true);
      g_steps.push_front({[] { AwaitReal(GetTickCount64()); }, 1});
      return;
    }
    fprintf(stderr, "browser: drive: real input %llu not heard in a second\n",
            static_cast<unsigned long long>(g_real_sent));
    g_real_heard = g_real_sent;
    return;
  }
  g_steps.push_front({[since] { AwaitReal(since); }, 1});
}

// A real key's event, |vk| going down or, with |up|, up.
void RealKey(UINT vk, bool up) {
  INPUT input{};
  input.type = INPUT_KEYBOARD;
  input.ki.wVk = static_cast<WORD>(vk);
  input.ki.wScan = static_cast<WORD>(MapVirtualKeyW(vk, MAPVK_VK_TO_VSC));
  input.ki.dwFlags = (up ? KEYEVENTF_KEYUP : 0) | (Extended(vk) ? KEYEVENTF_EXTENDEDKEY : 0);
  SendReal(input);
}

// The real keyboard's modifiers, from what it holds to |to|, a key each that changes, and the Windows
// keys up. What it holds is the desktop's, not this driver's: a run that quit with ⌃ down left it
// down for the next, whose escape was then ⌃esc, which opens Start.
void RealModifiers(int to) {
  static const std::pair<int, UINT> keys[] = {{kCtrl, VK_CONTROL}, {kShift, VK_SHIFT},
                                              {kAlt, VK_MENU},     {0, VK_LWIN},
                                              {0, VK_RWIN}};
  // Those let go first, the last held first, then those held, in order, as a hand does: Alt
  // pressed while the ⌃ a ⌘ key kept was still down made a chord of it, not ⌥ tapped alone.
  for (auto key = std::rbegin(keys); key != std::rend(keys); ++key) {
    if (!(to & key->first) && (GetAsyncKeyState(key->second) & 0x8000)) {
      if (key->second == VK_LWIN || key->second == VK_RWIN) {
        fprintf(stderr, "browser: drive: lets go of the Windows key the desktop held\n");
      }
      RealKey(key->second, true);
    }
  }
  for (const auto& [modifier, vk] : keys) {
    if ((to & modifier) && !(GetAsyncKeyState(vk) & 0x8000)) {
      RealKey(vk, false);
    }
  }
}

// The real pointer moving to |at| on screen, unless it's there, as a hand's resting pointer sends
// nothing.
void RealPointerTo(POINT at) {
  POINT now{};
  if (GetCursorPos(&now) && now.x == at.x && now.y == at.y) {
    return;
  }
  RealMoveTo(at);
}

// The real pointer's move to |at|, or with |again| the same move sent again.
void RealMoveTo(POINT at, bool again) {
  INPUT input{};
  input.type = INPUT_MOUSE;
  // SendInput's absolute places run from 0 to 65535 across the whole desktop.
  const auto along = [](LONG at, int start, int size) {
    return static_cast<LONG>(std::lround((at - GetSystemMetrics(start)) * 65535.0 /
                                         (GetSystemMetrics(size) - 1)));
  };
  input.mi.dx = along(at.x, SM_XVIRTUALSCREEN, SM_CXVIRTUALSCREEN);
  input.mi.dy = along(at.y, SM_YVIRTUALSCREEN, SM_CYVIRTUALSCREEN);
  input.mi.dwFlags = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;
  SendReal(input);
  g_real_move_again = again;
  g_real_move = at;
  g_real_move_number = g_real_sent;
}

// A real button's or wheel's event where the pointer is: |flags| as SendInput takes them.
void RealPointer(DWORD flags, DWORD data = 0) {
  INPUT input{};
  input.type = INPUT_MOUSE;
  input.mi.dwFlags = flags;
  input.mi.mouseData = data;
  SendReal(input);
}

DWORD RealFlags(UINT button, bool up) {
  if (button == MK_RBUTTON) {
    return up ? MOUSEEVENTF_RIGHTUP : MOUSEEVENTF_RIGHTDOWN;
  }
  if (button == MK_MBUTTON) {
    return up ? MOUSEEVENTF_MIDDLEUP : MOUSEEVENTF_MIDDLEDOWN;
  }
  return up ? MOUSEEVENTF_LEFTUP : MOUSEEVENTF_LEFTDOWN;
}

// The modifiers held, flags' and |modifiers|, in the UI thread's keyboard state, and the buttons.
// With real input, the real keyboard's modifiers, and the buttons are the real pointer's own.
void HoldKeys(int modifiers, UINT buttons = 0) {
  if (g_real) {
    RealModifiers(modifiers | g_flags | (g_key_ctrl ? kCtrl : 0));
    return;
  }
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
  if (g_real) {
    return;
  }
  if (g_hovered && g_hovered != target.hwnd && IsWindow(g_hovered)) {
    PostMessageW(g_hovered, WM_MOUSELEAVE, 0, kDriverLeave);
  }
  g_hovered = target.hwnd;
}

// Windows sends all of the thread's mouse input to the window that has the mouse captured,
// wherever the pointer is, as GPUI's has from a press until it hears the release: the driver's
// goes there too, so a capture left behind shows in a check as it does to a hand (#2516).
Target Captured(const Target& target) {
  const HWND held = GetCapture();
  return held && held != target.hwnd && Ours(held) ? At(held, target.screen) : target;
}

// Posts the pointer moving to |to|, with |buttons| held.
void Move(const Target& to, int modifiers, UINT buttons) {
  g_real_aim = to.hwnd;
  HoldKeys(modifiers, buttons);
  if (g_real) {
    RealPointerTo(to.screen);
    return;
  }
  if (host_drive_pointer(WM_MOUSEMOVE, to.screen.x, to.screen.y)) {
    return;
  }
  const Target target = Captured(to);
  // Over the frame, as over the UI's maximize button (host.cc's FrameHit), Windows sends the
  // window a non-client move, with what's there and the point on the screen. Entered, so the leave
  // Windows sends it at once, for the desktop's pointer, is dropped.
  if (target.hwnd == to.hwnd && Ours(target.hwnd)) {
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
void Press(const Target& at, UINT button, int modifiers, bool second) {
  g_real_aim = at.hwnd;
  HoldKeys(modifiers, button);
  if (g_real) {
    // Windows activates the window, tells its parents and makes a second press a double click.
    RealPointerTo(at.screen);
    RealPointer(RealFlags(button, false));
    return;
  }
  const UINT down = Down(button);
  if (host_drive_pointer(down, at.screen.x, at.screen.y)) {
    return;
  }
  const Target target = Captured(at);
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

void Release(const Target& at, UINT button, int modifiers) {
  g_real_aim = at.hwnd;
  HoldKeys(modifiers);
  if (g_real) {
    RealPointerTo(at.screen);
    RealPointer(RealFlags(button, true));
    // Its keys stay held past the button, as a hand's do, until the next step's.
    return;
  }
  if (host_drive_pointer(Up(button), at.screen.x, at.screen.y)) {
    return;
  }
  const Target target = Captured(at);
  PostMessageW(target.hwnd, Up(button), MouseKeys(modifiers, 0), ClientParam(target));
}

// How long a press at |target| waits, so GPUI, which counts every press, a drag's too, doesn't
// take it with the last one for a double click, as the driver presses faster than a hand does.
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

// The steps of a press at |target| that starts a drag, waiting as a click does (platform/linux/
// drive.rs waits the same), so a drag from where the last one started isn't a double click.
void AddPress(const Target& target, UINT button, int modifiers) {
  if (const UINT wait = WaitBeforeClick(target, button)) {
    g_steps.push_back({[] {}, wait});
  }
  g_steps.push_back({[=] { Move(target, 0, 0); }});
  g_steps.push_back({[=] {
    Press(target, button, modifiers, false);
    g_last_click = {target.hwnd, target.screen, button, GetTickCount64()};
  }});
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
    modifiers |= word == "cmd"      ? kCtrl
                 : word == "shift"  ? kShift
                 : word == "alt"    ? kCtrl | kAlt
                 : word == "winalt" ? kAlt
                                    : 0;
  }
  if (ctrl) {
    modifiers |= key == "tab" && !cmd ? kCtrl : kAlt;
  }
  return modifiers;
}

// The Mac's modifiers named in |words| for a click or a drag, as the page and the UI read a click's
// here: cmd with Ctrl and alt with Alt, so ⌥-click is Alt+click and ⌥⌘-click Ctrl+Alt+click.
int PointerModifiers(std::istringstream& words) {
  int modifiers = 0;
  std::string word;
  while (words >> word) {
    modifiers |= word == "cmd" ? kCtrl : word == "shift" ? kShift : word == "alt" ? kAlt : 0;
  }
  return modifiers;
}

// The virtual key of drive.mm's |key|, and the modifiers this keyboard types it with, or 0.
UINT VirtualKey(const std::string& key, int* modifiers) {
  static const std::pair<const char*, UINT> named[] = {
      {"return", VK_RETURN},  {"escape", VK_ESCAPE}, {"tab", VK_TAB},      {"space", VK_SPACE},
      {"backspace", VK_BACK}, {"delete", VK_DELETE}, {"pageup", VK_PRIOR}, {"pagedown", VK_NEXT},
      {"up", VK_UP},          {"down", VK_DOWN},     {"left", VK_LEFT},    {"right", VK_RIGHT},
      {"home", VK_HOME},      {"f4", VK_F4},         {"f6", VK_F6},        {"f10", VK_F10},
      {"f12", VK_F12}};
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
    case VK_LWIN:
    case VK_RWIN:
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
  g_real_aim = KeyTarget();
  HoldKeys(modifiers);
  if (g_real) {
    RealKey(vk, !down);
    return;
  }
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
  const bool system = ((held & kAlt || vk == VK_MENU) && !(held & kCtrl)) || vk == VK_F10;
  const UINT message =
      down ? (system ? WM_SYSKEYDOWN : WM_KEYDOWN) : (system ? WM_SYSKEYUP : WM_KEYUP);
  PostMessageW(KeyTarget(), message, vk, lparam);
}

// Ctrl's and Shift's keys going down or up, from |from| held to |to|, as the keyboard sends them,
// then winalt's Alt. A key's Alt is otherwise left out: Alt let go alone is a tap of its own, which
// a key with it in between stops on a real keyboard, and a key's Alt is read from the state held,
// as Chromium and GPUI read every modifier.
void PostModifiers(int from, int to) {
  if (g_real) {
    RealModifiers(to);
    return;
  }
  static const std::pair<int, UINT> keys[] = {{kCtrl, VK_CONTROL}, {kShift, VK_SHIFT}};
  for (const auto& [modifier, vk] : keys) {
    if ((from & modifier) != (to & modifier)) {
      from = (to & modifier) ? from | modifier : from & ~modifier;
      PostKey(vk, from, (to & modifier) != 0);
    }
  }
  if ((from & kAltKey) != (to & kAltKey)) {
    const bool down = (to & kAltKey) != 0;
    PostKey(VK_MENU, down ? from | kAlt | kAltKey : from & ~(kAlt | kAltKey), down);
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
  // The ring the core draws inside a split view's focused page, over the pages, as the Mac's driver
  // lists its view: from the content's bottom-left corner, as the pages are.
  bc_rect ring{};
  if (bc_window_focus_ring(host_browser_window_id(g_driven), &ring)) {
    snprintf(part, sizeof part, ", BrowserCoreFocusRing %.0f,%.0f %.0fx%.0f", ring.x,
             height - (ring.y + ring.height), ring.width, ring.height);
    line += part;
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

// Sets |root|'s content to |width| by |height| points, where it is.
bool ResizeContent(HWND root, double width, double height) {
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

// Microsoft's Japanese IME, while ime ja is on. Its keys go through Windows' input queue, as a
// keyboard's do, so the IME composes them itself, in the UI's fields and in a page alike: posted
// ones would pass it by.
bool g_japanese = false;
// Microsoft's Korean IME, while ime ko is on, its keys sent as the Japanese IME's are.
bool g_korean = false;
// The key that switches a Korean IME between Hangul and the keyboard's letters.
constexpr UINT kImeHangul = VK_HANGUL;
// The keys that turn a Japanese IME on in hiragana (VK_DBE_HIRAGANA, which not every SDK's headers
// name) and on or off (VK_KANJI, Alt+` on a US keyboard).
constexpr UINT kImeHiragana = 0xF2;
constexpr UINT kImeOnOff = VK_KANJI;

// The input method for |language|, where it's one of the user's, which a runner gets from that
// language's Language.Basic capability and its language list (check-windows.yml).
HKL LanguageLayout(WORD language) {
  HKL layouts[32];
  const int count = GetKeyboardLayoutList(32, layouts);
  for (int i = 0; i < count; ++i) {
    if (LOWORD(reinterpret_cast<UINT_PTR>(layouts[i])) == language) {
      return layouts[i];
    }
  }
  return nullptr;
}

// Microsoft's Japanese IME.
HKL JapaneseLayout() {
  return LanguageLayout(0x0411);
}

// |vk| pressed and let go through the input queue, to the window in front.
void SendVirtualKey(UINT vk) {
  INPUT inputs[2]{};
  inputs[0].type = inputs[1].type = INPUT_KEYBOARD;
  inputs[0].ki.wVk = inputs[1].ki.wVk = static_cast<WORD>(vk);
  inputs[1].ki.dwFlags = KEYEVENTF_KEYUP;
  SendInput(2, inputs, sizeof inputs[0]);
}

// The key window's title, once it holds |text|, looking again every 100 ms up to |tries| times, as
// drive.mm's title waits: a window that opens from a command, such as DevTools undocked, takes
// the keyboard a moment after it (#1922). With no |text|, the title it has now.
void AwaitKeyTitle(const std::string& text, int tries) {
  wchar_t title[512] = L"";
  GetWindowTextW(GetActiveWindow(), title, 512);
  const std::string shown = Utf8(title);
  if (text.empty() || shown.find(text) != std::string::npos) {
    fprintf(stderr, "browser: key window titled %s\n", shown.c_str());
  } else if (tries == 0) {
    fprintf(stderr, "browser: no key window titled %s, the key window titled %s\n", text.c_str(),
            shown.c_str());
  } else {
    g_steps.push_front({[text, tries] { AwaitKeyTitle(text, tries - 1); }});
    g_steps.push_front({[] {}, 100});
  }
}

// Waits, up to |tries| times, for the keys SendInput queued to be handled: the driver's posted
// messages come off the queue before input does.
void WaitForKeys(int tries) {
  if (tries > 0 && HIWORD(GetQueueStatus(QS_KEY))) {
    g_steps.push_front({[tries] { WaitForKeys(tries - 1); }, 16});
  }
}

// Adds |vk| through the input queue, once the input it reaches is ilo's.
bool AddSentKey(UINT vk) {
  g_steps.push_back({[vk] {
    DWORD process = 0;
    GetWindowThreadProcessId(GetForegroundWindow(), &process);
    if (process != GetCurrentProcessId()) {
      fprintf(stderr, "browser: drive: the IME's key went to another app's window\n");
      return;
    }
    SendVirtualKey(vk);
  }, 30});
  g_steps.push_back({[] { WaitForKeys(100); }, 16});
  // What the IME does with the key, as a composition it cancels, can come a moment after.
  g_steps.push_back({[] {}, 150});
  return true;
}

// Adds the step that handles |key| as drive.mm's scripted input method does, with the calls a
// Japanese one makes in the UI's field with the keys: letters compose, space converts them all,
// return commits and escape cancels. Windows' own input methods can't be scripted, and the runners
// have no Japanese one, so it calls GPUI's input handler as Linux's driver does. Returns 1 for a key
// it took, 0 for one it leaves to the window, as one with a shortcut's modifiers, and -1 for one it
// would compose in a page, whose input method is Chromium's, which the driver can't type through:
// as plain keys it would pass for a composition.
int AddImeKey(const std::string& key, int modifiers) {
  const bool letter = key.size() == 1 && key[0] >= 'a' && key[0] <= 'z';
  // Microsoft's Japanese and Korean IMEs take the keys without a shortcut's modifiers, as a
  // keyboard's.
  if ((g_japanese || g_korean) && !((modifiers | g_flags) & (kCtrl | kAlt))) {
    int held = modifiers;
    const UINT vk = VirtualKey(key, &held);
    return vk && !held && AddSentKey(vk) ? 1 : 0;
  }
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

// Chromium's Picture in Picture window, if one shows: a video's, the Mini Player, or a page's, as a
// call floats in. Chrome keeps it over the others, the only one of the app's own top-level windows
// it does but a browser window kept on top; the host's floating windows are owned.
HWND PictureInPicture() {
  for (HWND top = GetTopWindow(nullptr); top; top = GetWindow(top, GW_HWNDNEXT)) {
    if (Ours(top) && IsWindowVisible(top) && !GetWindow(top, GW_OWNER) &&
        (GetWindowLongPtrW(top, GWL_EXSTYLE) & WS_EX_TOPMOST) && BrowserIndex(top) < 0 &&
        ClassOf(top).rfind(L"Chrome_WidgetWin", 0) == 0) {
      return top;
    }
  }
  return nullptr;
}

// What Windows' hit test makes of a point, as it decides a press there moves or sizes the window.
const char* HitName(LRESULT hit) {
  switch (hit) {
    case HTCAPTION: return "caption";
    case HTCLIENT: return "client";
    case HTLEFT: return "left";
    case HTRIGHT: return "right";
    case HTTOP: return "top";
    case HTBOTTOM: return "bottom";
    case HTTOPLEFT: return "topleft";
    case HTTOPRIGHT: return "topright";
    case HTBOTTOMLEFT: return "bottomleft";
    case HTBOTTOMRIGHT: return "bottomright";
    case HTCLOSE: return "close";
    case HTNOWHERE: return "nowhere";
    default: return "other";
  }
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
// one, a link as browsers do, with its text, or text; or a tab, as host_drag_tab carries one from
// another of the app's windows. Null for another kind.
IDataObject* DragData(const std::string& kind, const std::string& value) {
  if (kind != "file" && kind != "url" && kind != "text" && kind != "tab") {
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
  if (kind == "tab") {
    const std::wstring tab = std::to_wstring(GetCurrentProcessId()) + L" " + wide;
    put(RegisterClipboardFormatW(L"dev.ilo.browser.tab"), tab.c_str(),
        (tab.size() + 1) * sizeof(wchar_t));
    return data;
  }
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
  // A page's is Chromium's own, which RegisterDragDrop keeps in the property itself, the target
  // object as this process has it, so the driver hands it the drag as OLE would (#2114).
  if (view && !drop && IsChromium(view)) {
    drop = static_cast<IDropTarget*>(GetPropW(view, L"OleDropTargetInterface"));
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
  // Named as the Mac names the views: a page's, else the UI's.
  fprintf(stderr, "browser: drag over %s %s\n",
          !view              ? "nothing"
          : IsChromium(view) ? "WebContentsViewCocoa"
                             : "HostUIView",
          *taken ? "taken" : "refused");
  return true;
}

// The app's own drag, a file's or a tab's, which the host hands the driver in place of DoDragDrop
// (host_drive_drags): Windows' drag waits for the real pointer, which the driver keeps from the
// app, and stopped the app there (#1583). It's held from the move that starts it to the release,
// and goes to the targets under the driver's pointer as another app's drag does: the UI's take it
// or not, and over the page, whose target is Chromium's, nothing takes it.
struct AppDrag {
  bool held = true;
  bool taken = false;
  DWORD effect = DROPEFFECT_NONE;
  POINT at{};
};
AppDrag* g_app_drag = nullptr;

// Moves the app's drag to |x|, |y| of the driven window, if it holds one.
bool AppDragTo(double x, double y) {
  if (!g_app_drag || !g_app_drag->held) {
    return false;
  }
  std::istringstream none;
  if (!DragOver(x, y, none, &g_app_drag->taken)) {
    g_app_drag->taken = false;
    if (g_drag_target) {
      g_drag_target->DragLeave();
    }
    g_drag_view = nullptr;
    g_drag_target = nullptr;
  }
  return true;
}

// Lets the app's drag go at |x|, |y| of the driven window, if it holds one.
bool AppDragLetGo(double x, double y, int modifiers) {
  if (!g_app_drag || !g_app_drag->held) {
    return false;
  }
  AppDragTo(x, y);
  g_app_drag->at = OnScreen(Driven(), x, y);
  if (g_app_drag->taken) {
    DWORD effect = DROPEFFECT_COPY | DROPEFFECT_LINK | DROPEFFECT_MOVE;
    g_drag_target->Drop(g_drag, MK_LBUTTON, {g_app_drag->at.x, g_app_drag->at.y}, &effect);
    g_app_drag->effect = effect;
  } else if (g_drag_target) {
    g_drag_target->DragLeave();
  }
  fprintf(stderr, "browser: drive: the app's drag let go, effect %lu\n", g_app_drag->effect);
  EndDrag();
  HoldKeys(modifiers, 0);
  g_app_drag->held = false;
  // The real button that started it comes up where the drag went.
  if (g_real) {
    RealPointerTo(g_app_drag->at);
    RealPointer(MOUSEEVENTF_LEFTUP);
  }
  return true;
}

// The host's DoDragDrop in a driven run: a loop of its own until the release, as DoDragDrop's.
HRESULT DriveDrag(IDataObject* data, DWORD, DWORD* effect, POINT* at) {
  if (g_drag_target) {
    g_drag_target->DragLeave();
  }
  EndDrag();
  // DoDragDrop's loop takes the pointer for a window of its own, which ends the press that started
  // the drag. Held on, the window the tab left kept the pointer, and the driver sent it the click
  // meant for the window the tab opened in (#2607).
  ReleaseCapture();
  data->AddRef();
  g_drag = data;
  AppDrag drag;
  g_app_drag = &drag;
  fprintf(stderr, "browser: drive: holds the app's drag\n");
  MSG message;
  while (drag.held) {
    const BOOL got = GetMessageW(&message, nullptr, 0, 0);
    if (got <= 0) {
      // The app quits: its own loop hears it too.
      if (got == 0) {
        PostQuitMessage(static_cast<int>(message.wParam));
      }
      if (g_drag_target) {
        g_drag_target->DragLeave();
      }
      EndDrag();
      break;
    }
    TranslateMessage(&message);
    DispatchMessageW(&message);
  }
  g_app_drag = nullptr;
  *effect = drag.effect;
  *at = drag.at;
  return drag.held ? DRAGDROP_S_CANCEL : DRAGDROP_S_DROP;
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
      "insert",        "choose",         "dismiss",     "savepanel"};
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
  // tabpicture <x> <y> <width> <height> <path>: the picture host_drag_tab shows for that part of
  // the driven window, the row a drag holds, written to <path> as a BMP, logging its size: the
  // driver carries the app's drag without Windows' own, which shows it (#1583).
  if (action == "tabpicture") {
    double x, y, width, height;
    std::string path;
    if (!(words >> x >> y >> width >> height) || !std::getline(words >> std::ws, path)) {
      return false;
    }
    Target target;
    long wide = 0, high = 0;
    // From the UI's window, whose points start where the browser window's content does.
    if (!TargetAt(x + 1, y + 1, &target) ||
        !host_drive_window_part(target.hwnd, x, y, width, height, path.c_str(), &wide, &high)) {
      return false;
    }
    fprintf(stderr, "browser: tab picture %ldx%ld\n", wide, high);
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
    std::string behind;
    if (words >> behind && behind != "behind") {
      return false;
    }
    g_driven = number - 1;
    fprintf(stderr, "browser: drive window %zu%s\n", number, behind.empty() ? "" : " behind");
    // With behind, it stays where it is, for the pointer over a window behind another (#1432).
    if (!behind.empty()) {
      return true;
    }
    // In front of the app's other browser windows with the keyboard, as a click on it does, but
    // never over another app's, nor over one Stay On Top keeps on top, which a click can't put
    // under it and which SetWindowPos would take out of the topmost windows.
    HWND root = Driven();
    // The others are behind a window on top already, and put after it, Windows would make them
    // topmost too: still over a window opened later, while it had the keyboard (#2607).
    const bool on_top = GetWindowLongPtrW(root, GWL_EXSTYLE) & WS_EX_TOPMOST;
    for (size_t index = 0; index < 64 && !on_top; index++) {
      HWND other = Browser(index);
      const bool topmost = other && (GetWindowLongPtrW(other, GWL_EXSTYLE) & WS_EX_TOPMOST);
      if (other && other != root && !topmost) {
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
  if (action == "realkey") {
    if (!OwnDesktop()) {
      fprintf(stderr, "browser: drive: realkey refused, the desktop isn't the check's own\n");
      return false;
    }
    std::string key;
    words >> key;
    int typed = 0;
    const UINT vk = VirtualKey(key, &typed);
    if (!vk || typed) {
      return false;
    }
    std::vector<WORD> keys;
    std::string word;
    while (words >> word) {
      const WORD modifier = word == "win"     ? VK_LWIN
                            : word == "ctrl"  ? VK_CONTROL
                            : word == "shift" ? VK_SHIFT
                            : word == "alt"   ? VK_MENU
                                              : 0;
      if (!modifier) {
        return false;
      }
      keys.push_back(modifier);
    }
    keys.push_back(static_cast<WORD>(vk));
    // Each key down 15 ms after the last, held 80 ms, then up in turn, as a hand's.
    const auto send = [](WORD vk, bool up) {
      INPUT input{};
      input.type = INPUT_KEYBOARD;
      input.ki.wVk = vk;
      input.ki.dwFlags =
          (up ? KEYEVENTF_KEYUP : 0) | (vk == VK_LWIN || Extended(vk) ? KEYEVENTF_EXTENDEDKEY : 0);
      SendInput(1, &input, sizeof input);
    };
    for (size_t at = 0; at < keys.size(); ++at) {
      const WORD down = keys[at];
      g_steps.push_back({[send, down] { send(down, false); }, at + 1 < keys.size() ? 15u : 80u});
    }
    for (auto up = keys.rbegin(); up != keys.rend(); ++up) {
      const WORD each = *up;
      g_steps.push_back({[send, each] { send(each, true); }, 15});
    }
    // What Windows did with them, for a check that waits on it in vain, as Win+D's.
    g_steps.push_back({[] {
      const HWND front = GetForegroundWindow();
      const HWND driven = Driven();
      fprintf(stderr, "browser: drive: after realkey, %s in front, the driven window %s\n",
              front ? Utf8(ClassOf(front)).c_str() : "nothing",
              !driven                ? "gone"
              : IsIconic(driven)     ? "minimized"
              : IsWindowVisible(driven) ? "shown"
                                        : "hidden");
    }});
    return true;
  }
  // The windows the app has over its browser windows that are on screen, by their own visibility:
  // its overlays, its menu and Chromium's bubbles, each owned by its browser window.
  // pip logs Chromium's Picture in Picture window's frame, "browser: pip at <x> <y> <width>
  // <height>", or "browser: pip none", as Linux's driver does. pip hit <x> <y>, at points from its
  // top-left corner, logs "browser: pip hit <part>", what Windows' hit test says is there: caption
  // for a part a press moves the window by, bottomright for a corner it sizes it from, client for
  // the page. Windows moves and sizes it with the desktop's pointer, which the driver never moves.
  if (action == "pip") {
    HWND pip = PictureInPicture();
    if (!pip) {
      fprintf(stderr, "browser: pip none\n");
      return true;
    }
    RECT rect;
    GetWindowRect(pip, &rect);
    const double scale = Scale(pip);
    std::string step;
    words >> step;
    if (step.empty()) {
      fprintf(stderr, "browser: pip at %.0f %.0f %.0f %.0f\n", rect.left / scale, rect.top / scale,
              (rect.right - rect.left) / scale, (rect.bottom - rect.top) / scale);
      return true;
    }
    double x, y;
    if (step != "hit" || !(words >> x >> y)) {
      return false;
    }
    const LONG at_x = rect.left + std::lround(x * scale);
    const LONG at_y = rect.top + std::lround(y * scale);
    const LRESULT hit = SendMessageW(pip, WM_NCHITTEST, 0, MAKELPARAM(at_x, at_y));
    fprintf(stderr, "browser: pip hit %s\n", HitName(hit));
    return true;
  }
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
  // Each window the host floats over a browser window, made so far, and the menu, as the host's
  // overlay machine has it and as Windows does, for a check that puts each through what made a
  // hidden one go stale (#2563): "browser: lives command bar key on screen active focus,
  // tooltip hidden, menu shown on screen capture dpi 96 over 120". Its life is the host's (Life),
  // and the rest Windows': whether it's visible, the active window, the one with the keyboard's
  // focus or the pointer's capture on its thread, and its DPI where it isn't its window's.
  if (action == "lives") {
    std::string line;
    host_floating_lives(
        [](void* context, const char* name, void* hwnd_pointer, int life) {
          static const char* const kLives[] = {"hidden", "placed", "shown", "key", "hiding"};
          const HWND hwnd = static_cast<HWND>(hwnd_pointer);
          std::string& out = *static_cast<std::string*>(context);
          out += out.empty() ? "" : ", ";
          out += name ? name : "unnamed";
          out += " ";
          out += life >= 0 && life < static_cast<int>(std::size(kLives)) ? kLives[life] : "?";
          GUITHREADINFO thread{sizeof(GUITHREADINFO)};
          GetGUIThreadInfo(GetWindowThreadProcessId(hwnd, nullptr), &thread);
          out += IsWindowVisible(hwnd) ? " on screen" : "";
          out += thread.hwndActive == hwnd ? " active" : "";
          out += thread.hwndFocus == hwnd ? " focus" : "";
          out += thread.hwndCapture == hwnd ? " capture" : "";
          HWND owner = GetWindow(hwnd, GW_OWNER);
          const UINT dpi = GetDpiForWindow(hwnd), over = owner ? GetDpiForWindow(owner) : dpi;
          if (dpi != over) {
            out += " dpi " + std::to_string(dpi) + " over " + std::to_string(over);
          }
        },
        &line);
    fprintf(stderr, "browser: lives %s\n", line.empty() ? "none" : line.c_str());
    return true;
  }
  // The DPI Windows gives the driven browser window and each window the host floats over one,
  // shown or not, and the system's, from when the user signed in (#2541): "browser: dpis system 96,
  // window 120, command bar 120, link preview 96".
  if (action == "dpis") {
    std::string line = "system " + std::to_string(GetDpiForSystem()) + ", window " +
                       std::to_string(GetDpiForWindow(Driven()));
    for (HWND top = GetTopWindow(nullptr); top; top = GetWindow(top, GW_HWNDNEXT)) {
      if (const char* name = Ours(top) ? host_floating_name(top) : nullptr) {
        line += std::string(", ") + name + " " + std::to_string(GetDpiForWindow(top));
      }
    }
    fprintf(stderr, "browser: dpis %s\n", line.c_str());
    return true;
  }
  // Where Peek's page is, the page in front in the driven browser window, in pixels from its
  // content's top left, for the frame the preview's card has it in (#2541).
  if (action == "peek") {
    HWND root = Driven();
    for (HWND child = root ? GetWindow(root, GW_CHILD) : nullptr; child;
         child = GetWindow(child, GW_HWNDNEXT)) {
      if (IsWindowVisible(child) && IsChromium(child)) {
        RECT rect;
        GetWindowRect(child, &rect);
        MapWindowPoints(HWND_DESKTOP, root, reinterpret_cast<POINT*>(&rect), 2);
        fprintf(stderr, "browser: peek page %ld %ld %ld %ld\n", rect.left, rect.top,
                rect.right - rect.left, rect.bottom - rect.top);
        return true;
      }
    }
    fprintf(stderr, "browser: peek page none\n");
    return true;
  }
  // The windows on screen over the driven browser window, front first, down to it, by Windows' own
  // z-order (#1419): the host's floating windows by their names, another of the app's by what front
  // calls it, and another app's as "other" and its title. Cloaked ones aren't on screen.
  if (action == "stack") {
    HWND root = Driven();
    if (!root) {
      return false;
    }
    RECT over;
    GetWindowRect(root, &over);
    std::string stack;
    for (HWND top = GetTopWindow(nullptr); top; top = GetWindow(top, GW_HWNDNEXT)) {
      RECT rect, both;
      DWORD cloaked = 0;
      DwmGetWindowAttribute(top, DWMWA_CLOAKED, &cloaked, sizeof cloaked);
      if (top != root && (!IsWindowVisible(top) || cloaked || !GetWindowRect(top, &rect) ||
                          !IntersectRect(&both, &rect, &over))) {
        continue;
      }
      std::string name;
      if (top == root) {
        name = "driven";
      } else if (!Ours(top)) {
        // Without WM_GETTEXT, which would wait on the window's thread.
        wchar_t title[128] = L"";
        InternalGetWindowText(top, title, static_cast<int>(std::size(title)));
        name = "other " + Utf8(title);
      } else if (const char* floating = host_floating_name(top)) {
        name = floating;
      } else {
        name = Describe(top);
      }
      stack += (stack.empty() ? "" : ", ") + name;
      if (top == root) {
        break;
      }
    }
    fprintf(stderr, "browser: stack %s\n", stack.c_str());
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
  // Windows' system command, as the title bar's button and Win+↑ send it: it maximizes the window,
  // or restores it once maximized, through DefWindowProc, as the resize that crashed did (#1340).
  if (action == "maximize") {
    HWND root = Driven();
    g_steps.push_back({[root] {
      PostMessageW(root, WM_SYSCOMMAND, IsZoomed(root) ? SC_RESTORE : SC_MAXIMIZE, 0);
    }});
    return true;
  }
  // The driven window's state and the frame it un-maximizes to, as Windows keeps them, in
  // LogFrame's terms: "browser: placement <maximized|minimized|normal> <x> <y> <width> <height>".
  if (action == "placement") {
    HWND root = Driven();
    WINDOWPLACEMENT placement{sizeof placement};
    if (!root || !GetWindowPlacement(root, &placement)) {
      return false;
    }
    // Windows keeps the frame from the work area's corner, not the screen's.
    MONITORINFO monitor{sizeof monitor};
    GetMonitorInfoW(MonitorFromWindow(root, MONITOR_DEFAULTTONEAREST), &monitor);
    RECT normal = placement.rcNormalPosition;
    OffsetRect(&normal, monitor.rcWork.left - monitor.rcMonitor.left,
               monitor.rcWork.top - monitor.rcMonitor.top);
    const char* state = placement.showCmd == SW_SHOWMAXIMIZED   ? "maximized"
                        : placement.showCmd == SW_SHOWMINIMIZED ? "minimized"
                                                                : "normal";
    const double scale = Scale(root);
    fprintf(stderr, "browser: placement %s %.0f %.0f %.0f %.0f\n", state, normal.left / scale,
            normal.top / scale, (normal.right - normal.left) / scale,
            (normal.bottom - normal.top) / scale);
    return true;
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
    AddClick(target, button, in_key ? 0 : PointerModifiers(words),
             action.rfind("double", 0) == 0);
    return true;
  }
  if (action == "drag") {
    double x, y, x2, y2;
    Target from;
    if (!(words >> x >> y >> x2 >> y2) || !TargetAt(x, y, &from)) {
      return false;
    }
    const Target to = At(from.hwnd, OnScreen(Driven(), x2, y2));
    AddPress(from, MK_LBUTTON, 0);
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
    AddPress(from, MK_LBUTTON, 0);
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
    const int keys = PointerModifiers(words);
    if (step == "press") {
      Target target;
      if (!TargetAt(x, y, &target)) {
        return false;
      }
      g_pressed = target.hwnd;
      AddPress(target, button, keys);
      return true;
    }
    Target target;
    if (g_pressed && IsWindow(g_pressed)) {
      target = At(g_pressed, OnScreen(Driven(), x, y));
    } else if (!TargetAt(x, y, &target)) {
      return false;
    }
    // The app's own drag, if it has started one since, takes them in place of its window.
    if (step == "dragto") {
      g_steps.push_back({[=] {
        if (!AppDragTo(x, y)) {
          Move(target, keys, button);
        }
      }});
    } else {
      g_pressed = nullptr;
      g_steps.push_back({[=] {
        if (!AppDragLetGo(x, y, keys)) {
          Release(target, button, keys);
        }
      }});
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
  // scrollkey scrolls in the window with the keyboard, such as Settings', as clickkey clicks
  // there.
  if (action == "scroll" || action == "scrollkey" || action == "swipe" || action == "wheel") {
    double x, y, dx = 0, dy = 0;
    int count = 1, pause = 0;
    if (!(words >> x >> y)) {
      return false;
    }
    const bool in_key = action == "scrollkey";
    if ((action == "scroll" || in_key) && !(words >> dy >> count)) {
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
    if (count < 1 || !(in_key ? TargetInKey(x, y, &target) : TargetAt(x, y, &target))) {
      return false;
    }
    // A hand's touchpad scrolls the window under the pointer, which after a relaunch is the
    // runner's console in front of the app; the posted stand-ins bring the app in front first, as a
    // press or a real wheel's notch does (#2544).
    if (g_real && action != "wheel") {
      g_steps.push_back({[] {
        if (!OursInFront()) {
          BringInFront();
        }
      }});
    }
    // Two fingers over GPUI's view reach it as Direct Manipulation hands it a precision
    // touchpad's: a touch, its moves and its lift, which no wheel message carries (#1212). At
    // drive.mm's pace, the lift <pause> ms after the last move. Over a page they stay wheel
    // messages, which Chromium scrolls by.
    if (action == "swipe" && IsGpui(target.hwnd)) {
      const float at_x = static_cast<float>(target.client.x);
      const float at_y = static_cast<float>(target.client.y);
      const float across = static_cast<float>(dx);
      const float along = static_cast<float>(dy);
      g_steps.push_back({[=] { Move(target, 0, 0); }});
      for (int event = 0; event < count; event++) {
        g_steps.push_back(
            {[=] {
               HoldKeys(0);
               browser_drive_fingers(target.hwnd, at_x, at_y, across, along, event == 0 ? 0 : 1);
             },
             event + 1 < count ? 8u : static_cast<UINT>(pause > 0 ? pause : 0)});
      }
      g_steps.push_back({[=] { browser_drive_fingers(target.hwnd, at_x, at_y, 0, 0, 2); }});
      return true;
    }
    // A mouse's tick is a notch; a trackpad's points, content moving up for scrolling down. Across,
    // drive.mm's dx moves the content right, where Windows' positive delta scrolls right, so
    // content moves left: a positive dx goes back a Space, as on the Mac.
    const int across = action == "wheel" ? static_cast<int>(-dx * WHEEL_DELTA)
                                         : static_cast<int>(std::lround(-dx * 1.2));
    const int along = static_cast<int>(std::lround(-dy * 1.2));
    g_steps.push_back({[=] { Move(target, 0, 0); }});
    for (int event = 0; event < count; event++) {
      g_steps.push_back(
          {[=] {
             HoldKeys(0);
             // Two fingers on a touchpad reach a page through the touchpad's own input, which
             // SendInput can't make: scroll stays the posted stand-in it was, wheel's notches real.
             if (g_real && action == "wheel") {
               if (along) {
                 RealPointer(MOUSEEVENTF_WHEEL, static_cast<DWORD>(along));
               }
               if (across) {
                 RealPointer(MOUSEEVENTF_HWHEEL, static_cast<DWORD>(across));
               }
               return;
             }
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
    return words >> width >> height && ResizeContent(Driven(), width, height);
  }
  // The window with the keyboard, such as Settings, as resize does the driven one.
  if (action == "resizekey") {
    double width, height;
    return words >> width >> height && ResizeContent(GetActiveWindow(), width, height);
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
  // As Windows tells a window that moved to a display of another scale, |percent|, keeping its
  // size in pixels: Chromium and the core take the scale the message carries (#1823). With a name
  // after it, such as "link preview", the window the host floats by that name hears it instead, so
  // GPUI draws it at that scale where Windows has another, as one made hidden on another display
  // and moved to the browser window's does (#2541).
  if (action == "dpi") {
    int percent = 0;
    if (!(words >> percent) || percent < 100 || percent > 500) {
      return false;
    }
    std::string name;
    std::getline(words >> std::ws, name);
    g_steps.push_back({[percent, name] {
      HWND root = Driven();
      if (!name.empty()) {
        root = nullptr;
        for (HWND top = GetTopWindow(nullptr); top && !root; top = GetWindow(top, GW_HWNDNEXT)) {
          const char* floating = Ours(top) ? host_floating_name(top) : nullptr;
          root = floating && name == floating ? top : nullptr;
        }
        if (!root) {
          fprintf(stderr, "browser: dpi found no %s\n", name.c_str());
          return;
        }
      }
      RECT rect{};
      GetWindowRect(root, &rect);
      const WORD dpi = static_cast<WORD>(MulDiv(USER_DEFAULT_SCREEN_DPI, percent, 100));
      SendMessageW(root, WM_DPICHANGED, MAKEWPARAM(dpi, dpi), reinterpret_cast<LPARAM>(&rect));
      if (!name.empty()) {
        fprintf(stderr, "browser: dpi %d told %s\n", dpi, name.c_str());
      }
    }, 300});
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
  // The notification area's Little ilo icon, as the Mac's driver has the menu bar's button:
  // "browser: status item shown" or "none", or with click what Explorer sends for a click on it,
  // as the driver can't reach Explorer's area or its flyout. With place, where Explorer draws it:
  // "browser: status item at <left> <top> <right> <bottom> in area <left> <top> <right> <bottom>",
  // the notification area's rectangle, in the screen's pixels, or "placed nowhere".
  if (action == "statusitem") {
    if (rest == "click") {
      return host_drive_little_tray_click();
    }
    RECT icon{}, area{};
    if (rest == "place") {
      if (!host_drive_little_tray_place(&icon, &area)) {
        fprintf(stderr, "browser: status item placed nowhere\n");
      } else {
        fprintf(stderr, "browser: status item at %ld %ld %ld %ld in area %ld %ld %ld %ld\n",
                icon.left, icon.top, icon.right, icon.bottom, area.left, area.top, area.right,
                area.bottom);
      }
      return true;
    }
    fprintf(stderr, "browser: status item %s\n", host_drive_little_tray() ? "shown" : "none");
    return true;
  }
  if (action == "title") {
    AwaitKeyTitle(rest, 100);
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
    return host_menu_window_choose(rest.c_str()) || (g_dialog && g_dialog(rest.c_str())) ||
           host_choose(rest.c_str());
  }
  if (action == "dismiss") {
    return host_menu_window_dismiss();
  }
  if (action == "savepanel") {
    fprintf(stderr, host_save_dialog_answer(rest.c_str()) ? "browser: save panel answered\n"
                                                          : "browser: no save panel\n");
    return true;
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
  if (action == "agentopen") {
    std::string agent, url;
    if (!(words >> agent >> url)) {
      return false;
    }
    const bc_id tab = bc_agent_open_tab(agent.c_str(), url.c_str());
    if (!tab) {
      return false;
    }
    fprintf(stderr, "browser: agent %s opened %llu\n", agent.c_str(), (unsigned long long)tab);
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
  if (action == "battery") {
    int percent;
    std::string power;
    if (!(words >> percent >> power) || (power != "plugged" && power != "unplugged")) {
      return false;
    }
    bc_battery_for_testing(percent, power == "plugged");
    return true;
  }
  if (action == "pair") {
    bc_id tab;
    std::string kind;
    if (!(words >> tab >> kind)) {
      return false;
    }
    const int pair = kind == "pin" ? BC_PAIR_PIN : kind == "confirm" ? BC_PAIR_CONFIRM
                   : kind == "passkey" ? BC_PAIR_PASSKEY : -1;
    if (pair < 0) {
      return false;
    }
    bc_tab_pair_for_testing(tab, pair);
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
  // is Ctrl+Tab here, so it holds Ctrl, which the app watches for its release. winalt is Windows'
  // Alt, its key going down and up as a keyboard sends it, so it alone is a tap.
  if (action == "flags") {
    int modifiers = 0;
    std::string word;
    while (words >> word) {
      modifiers |= word == "cmd" || word == "ctrl" ? kCtrl
                   : word == "shift"               ? kShift
                   : word == "alt"                 ? kCtrl | kAlt
                   : word == "winalt"              ? kAlt | kAltKey
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
    if (state != "on" && state != "off" && state != "ja" && state != "ko") {
      return false;
    }
    if (state == "ko") {
      // The UI thread's input method is the Korean IME, switched to Hangul as the user would.
      HKL korean = LanguageLayout(0x0412);
      if (!korean) {
        fprintf(stderr, "browser: drive: no Korean IME\n");
        return false;
      }
      g_steps.push_back({[korean] { ActivateKeyboardLayout(korean, 0); }});
      g_korean = AddSentKey(kImeHangul);
      return true;
    }
    if (std::exchange(g_korean, false)) {
      AddSentKey(kImeHangul);
    }
    if (state == "ja") {
      // The UI thread's input method is the IME, on, in hiragana, as the user would pick it. Off,
      // it stays the thread's, closed, which types as the keyboard does.
      HKL japanese = JapaneseLayout();
      if (!japanese) {
        fprintf(stderr, "browser: drive: no Japanese IME\n");
        return false;
      }
      g_steps.push_back({[japanese] { ActivateKeyboardLayout(japanese, 0); }});
      g_japanese = AddSentKey(kImeHiragana);
      return true;
    }
    if (std::exchange(g_japanese, false)) {
      AddSentKey(kImeOnOff);
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

// A step's timer's backstop. Windows makes a WM_TIMER only once the thread has no message, input or
// paint waiting, and in window's run on real input (#2556's 43347302182) the step timer never came
// again after an escape closed a menu, while the UI thread went on with its other work. A step
// whose timer is 2 s past its time runs from here, and the driver says what the thread's queue
// held then. One at a time, and only while a step waits, so an idle app wakes for nothing.
HANDLE g_step_backstop = nullptr;
// The step timer set last, numbered, or 0 once it has come.
WPARAM g_step_timer = 0;

void CALLBACK StepLate(PVOID timer, BOOLEAN) {
  PostMessageW(g_driver, kStepLate, reinterpret_cast<WPARAM>(timer), 0);
}

void SetStepTimer(UINT wait) {
  static WPARAM timers = 0;
  SetTimer(g_driver, kStepTimer, wait, nullptr);
  if (g_step_backstop) {
    DeleteTimerQueueTimer(nullptr, g_step_backstop, nullptr);
    g_step_backstop = nullptr;
  }
  g_step_timer = ++timers;
  CreateTimerQueueTimer(&g_step_backstop, nullptr, StepLate, reinterpret_cast<PVOID>(g_step_timer),
                        wait + 2000, 0, WT_EXECUTEONLYONCE);
}

// What the UI thread's queue holds, for a step whose timer didn't come.
std::string Queued() {
  static constexpr std::pair<DWORD, const char*> kinds[] = {{QS_POSTMESSAGE, "posted"},
                                                             {QS_SENDMESSAGE, "sent"},
                                                             {QS_INPUT, "input"},
                                                             {QS_PAINT, "paint"},
                                                             {QS_TIMER, "timer"}};
  const DWORD queued = HIWORD(GetQueueStatus(QS_ALLINPUT));
  std::string said;
  for (const auto& [flag, name] : kinds) {
    if (queued & flag) {
      said += said.empty() ? name : std::string(", ") + name;
    }
  }
  return said.empty() ? "nothing" : said;
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
    // A click's real keys, held past its button as a hand's are, are let go as the command ends,
    // so none is still down for the next, such as Win+D's (realkey). flags' and ⌃Tab's stay.
    if (g_real) {
      g_steps.push_back({[] { RealModifiers(g_flags | (g_key_ctrl ? kCtrl : 0)); }});
    }
    // Done once the messages posted are handled, and what handling them posted. Twice, as GPUI
    // and Chromium post more as they handle some, such as the characters a key types.
    g_steps.push_back({[] {}});
    g_steps.push_back({[] {}});
    g_steps.push_back({[number] { ReportDone(number); }});
  }
  Step step = std::move(g_steps.front());
  g_steps.pop_front();
  const ULONG_PTR sent = g_real_sent;
  step.run();
  if (g_real_sent != sent) {
    g_steps.push_front({[since = GetTickCount64()] { AwaitReal(since); }, step.wait});
    step.wait = 1;
  }
  if (step.wait) {
    SetStepTimer(step.wait);
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
        g_step_timer = 0;
        Next();
      }
      return 0;
    case kStepLate:
      if (wparam && wparam == g_step_timer) {
        KillTimer(hwnd, kStepTimer);
        g_step_timer = 0;
        // check.sh shows this line when a step's wait ran out.
        fprintf(stderr, "browser: drive: the step's timer didn't come 2 s past its time, the queue "
                        "holding %s\n", Queued().c_str());
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
  if (code != HC_ACTION || wparam != PM_REMOVE || g_real) {
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

// Logs "browser: window zoomed <0|1>" as a browser window maximizes or comes back from it, as
// drive.mm logs AppKit's zoom and Linux's driver the window manager's, from the WM_SIZE Windows
// sends it once it has.
LRESULT CALLBACK WatchZoom(int code, WPARAM wparam, LPARAM lparam) {
  static std::map<HWND, bool> zoomed;
  const auto* sent = reinterpret_cast<CWPRETSTRUCT*>(lparam);
  if (code == HC_ACTION && sent->message == WM_SIZE &&
      (sent->wParam == SIZE_MAXIMIZED || sent->wParam == SIZE_RESTORED) &&
      GetAncestor(sent->hwnd, GA_ROOT) == sent->hwnd &&
      FindWindowExW(sent->hwnd, nullptr, L"BrowserUI", nullptr)) {
    const bool now = sent->wParam == SIZE_MAXIMIZED;
    const auto was = zoomed.find(sent->hwnd);
    if (was == zoomed.end() ? now : was->second != now) {
      fprintf(stderr, "browser: window zoomed %d\n", now ? 1 : 0);
    }
    zoomed[sent->hwnd] = now;
  }
  return CallNextHookEx(nullptr, code, wparam, lparam);
}

extern "C" {

void drive_watch_menus(bool (*menus)(void* ui_view, const char* title, bool bar),
                       bool (*dialog)(const char* label)) {
  g_menus = menus;
  g_dialog = dialog;
}

void drive_start(void) {
  WNDCLASSW window_class{};
  window_class.lpfnWndProc = DriverProc;
  window_class.hInstance = GetModuleHandleW(nullptr);
  window_class.lpszClassName = L"BrowserDriver";
  RegisterClassW(&window_class);
  g_driver = CreateWindowExW(0, window_class.lpszClassName, L"", 0, 0, 0, 0, 0, HWND_MESSAGE,
                             nullptr, window_class.hInstance, nullptr);
  if (EnvIs(L"BROWSER_DRIVE_REAL", L"1")) {
    g_real = OwnDesktop();
    if (g_real) {
      host_drive_real_input();
    }
    fprintf(stderr, g_real ? "browser: drive: real input\n"
                           : "browser: drive: real input refused, this desktop isn't the check's "
                             "own: BROWSER_DRIVE_OWN_DESKTOP=1 says one is\n");
  }
  SetWindowsHookExW(WH_GETMESSAGE, DropRealPointer, nullptr, GetCurrentThreadId());
  // With real input too: Windows' own drag loop takes only the pointer's input and dispatches
  // none of the driver's steps, so the next one never came (#1583, #2544). The stand-in takes the
  // driver's moves and release in place of the pointer's, from the move that starts the drag.
  host_drive_drags(DriveDrag);
  SetWindowsHookExW(WH_CBT, WatchKeyboard, nullptr, GetCurrentThreadId());
  SetWindowsHookExW(WH_CALLWNDPROCRET, WatchZoom, nullptr, GetCurrentThreadId());
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
