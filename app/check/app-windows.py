"""The app's top-level windows on Windows, for lib.sh's quit and what it says of an app that didn't.

`close PID [front]` asks each browser window of process PID to close, as its close button does: the
top-level windows holding the UI's view (platform/windows/host.cc's BrowserUI), or with front the
one in front alone. `end-session PID` ends the session for the process as Windows does when the
user signs out: each of its top-level windows is asked WM_QUERYENDSESSION, and once all agree,
told WM_ENDSESSION, the hidden ones first, and it fails if one refuses (#1990). `list PID` prints each top-level window of the process, with its class, title,
state and whether it's a browser window, minimized (IsIconic) or the one in front
(GetForegroundWindow). `icon PID ICO` checks that each window of the process the taskbar and
Alt+Tab show has the .ico's image for its big and small icons, read as the shell reads them, and
prints them (#1350). `cover PID ilo|away|behind [FRONT]` opens a window of another app in front
of the process's browser window, over its lower half, with the foreground in ilo's window or in a
window away from it, or with behind between ilo's window with the foreground and its others, as a
window another app opens behind the one the user is in (#1689), says "cover <handle>, foreground
<ilo|away|other>" and keeps it until it's ended
(#1419), or with FRONT until that file exists, when it brings the browser window to the front, as
a click on its taskbar button does, and ends (#1365). `foreground PID [TITLE]` gives the
foreground to the process's browser window in front, or to its window titled TITLE (#2114).
`dialogs PID [cancel]` prints each dialog shown of the process and the processes it started, with
its title, and with cancel ends each as its Cancel button does (#2512).
Usage: app-windows.py close|end-session|list|icon PID [front|ICO], cover PID ilo|away|behind [FRONT], foreground PID [TITLE] or dialogs PID [cancel]"""

import ctypes
import ctypes.wintypes as w
import os
import sys
import time

# Git Bash's grep reads UTF-8 lines ending in \n alone.
sys.stdout.reconfigure(encoding="utf-8", newline="\n")
user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32
WM_CLOSE = 0x0010
WM_QUERYENDSESSION = 0x0011
WM_ENDSESSION = 0x0016
ENDSESSION_LOGOFF = 0x80000000
SMTO_ABORTIFHUNG = 0x0002
GW_OWNER = 4
WM_COMMAND = 0x0111
IDCANCEL = 2
EnumProc = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)


def text(call, hwnd):
    buffer = ctypes.create_unicode_buffer(256)
    call(hwnd, buffer, len(buffer))
    return buffer.value


def holds_ui(hwnd):
    return bool(user32.FindWindowExW(hwnd, None, "BrowserUI", None))


def windows(pid):
    found = []

    def each(hwnd, _):
        owner = w.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid:
            found.append(hwnd)
        return True

    # Front to back.
    user32.EnumWindows(EnumProc(each), 0)
    return found


class ICONINFO(ctypes.Structure):
    _fields_ = [("fIcon", w.BOOL), ("xHotspot", w.DWORD), ("yHotspot", w.DWORD),
                ("hbmMask", w.HBITMAP), ("hbmColor", w.HBITMAP)]


class BITMAP(ctypes.Structure):
    _fields_ = [("bmType", w.LONG), ("bmWidth", w.LONG), ("bmHeight", w.LONG),
                ("bmWidthBytes", w.LONG), ("bmPlanes", w.WORD), ("bmBitsPixel", w.WORD),
                ("bmBits", ctypes.c_void_p)]


def declare_icon_calls():
    """The handle-taking calls icon makes, typed so 64-bit handles pass whole."""
    user32.SendMessageTimeoutW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM, w.UINT, w.UINT,
                                           ctypes.POINTER(ctypes.c_size_t)]
    user32.GetClassLongPtrW.argtypes = [w.HWND, ctypes.c_int]
    user32.GetClassLongPtrW.restype = ctypes.c_size_t
    user32.GetIconInfo.argtypes = [w.HICON, ctypes.POINTER(ICONINFO)]
    user32.LoadImageW.argtypes = [w.HINSTANCE, w.LPCWSTR, w.UINT, ctypes.c_int, ctypes.c_int,
                                  w.UINT]
    user32.LoadImageW.restype = w.HANDLE
    user32.DestroyIcon.argtypes = [w.HICON]
    user32.GetDC.restype = w.HDC
    user32.ReleaseDC.argtypes = [w.HWND, w.HDC]
    gdi32.GetObjectW.argtypes = [w.HANDLE, ctypes.c_int, ctypes.c_void_p]
    gdi32.GetDIBits.argtypes = [w.HDC, w.HBITMAP, w.UINT, w.UINT, ctypes.c_void_p,
                                ctypes.c_void_p, w.UINT]
    gdi32.DeleteObject.argtypes = [w.HANDLE]


def window_icon(hwnd, big):
    """The icon the shell shows for |hwnd|: the one the window was given, or its class's."""
    result = ctypes.c_size_t()
    # WM_GETICON, ICON_BIG or ICON_SMALL, with SMTO_ABORTIFHUNG.
    user32.SendMessageTimeoutW(hwnd, 0x007F, 1 if big else 0, 0, 0x0002, 2000, ctypes.byref(result))
    # GCLP_HICON or GCLP_HICONSM.
    return result.value or user32.GetClassLongPtrW(hwnd, -14 if big else -34)


def pixels(icon):
    """|icon|'s size and its colour bitmap as 32-bit top-down BGRA."""
    info = ICONINFO()
    if not user32.GetIconInfo(icon, ctypes.byref(info)):
        return None
    try:
        bitmap = BITMAP()
        gdi32.GetObjectW(info.hbmColor or info.hbmMask, ctypes.sizeof(bitmap), ctypes.byref(bitmap))
        width = bitmap.bmWidth
        height = bitmap.bmHeight if info.hbmColor else bitmap.bmHeight // 2
        header = (ctypes.c_uint32 * 10)(40, width, -height & 0xFFFFFFFF, 1 | 32 << 16)
        data = ctypes.create_string_buffer(width * height * 4)
        dc = user32.GetDC(None)
        gdi32.GetDIBits(dc, info.hbmColor or info.hbmMask, 0, height, data, header, 0)
        user32.ReleaseDC(None, dc)
        return width, height, data.raw
    finally:
        gdi32.DeleteObject(info.hbmColor)
        gdi32.DeleteObject(info.hbmMask)


def check_icons(pid, ico):
    """Whether every window the taskbar and Alt+Tab show (visible, unowned, not a tool window) has
    |ico|'s image, at the size each icon has, for its big and small icons."""
    declare_icon_calls()
    shown = [hwnd for hwnd in windows(pid) if user32.IsWindowVisible(hwnd)
             and not user32.GetWindow(hwnd, GW_OWNER)
             and not user32.GetWindowLongW(hwnd, -20) & 0x80]  # GWL_EXSTYLE, WS_EX_TOOLWINDOW
    if not shown:
        print("no window of the process is shown")
        return False
    ok = True
    for hwnd in shown:
        for big in (True, False):
            icon = window_icon(hwnd, big)
            have = pixels(icon) if icon else None
            name = "%#x %s %s" % (hwnd, text(user32.GetClassNameW, hwnd), "big" if big else "small")
            if not have:
                print(name + ": no icon")
                ok = False
                continue
            width, height, data = have
            # LR_LOADFROMFILE, at the icon's size, so Windows picks and scales as it did for the
            # window's.
            loaded = user32.LoadImageW(None, ico, 1, width, height, 0x10)
            want = pixels(loaded) if loaded else None
            if loaded:
                user32.DestroyIcon(loaded)
            if not want:
                sys.exit("can't load " + ico)
            difference = sum(abs(a - b) for a, b in zip(data, want[2])) / len(data)
            same = want[:2] == (width, height) and difference <= 2
            print("%s: %dx%d, %s (mean difference %.1f)" % (
                name, width, height, "the .ico's" if same else "not the .ico's", difference))
            ok = ok and same
    return ok


class WNDCLASSW(ctypes.Structure):
    _fields_ = [("style", w.UINT), ("lpfnWndProc", ctypes.c_void_p), ("cbClsExtra", ctypes.c_int),
                ("cbWndExtra", ctypes.c_int), ("hInstance", w.HINSTANCE), ("hIcon", w.HICON),
                ("hCursor", w.HANDLE), ("hbrBackground", w.HBRUSH), ("lpszMenuName", w.LPCWSTR),
                ("lpszClassName", w.LPCWSTR)]


def end_session(pid):
    """Asks each top-level window of process PID whether the session may end, as Windows does at a
    sign-out, then tells each that it ends, as Windows does once every app agreed. A window that
    refuses, or doesn't answer within 5 s, fails it. The process may end while a window hears the
    end, and the windows left then answer nothing."""
    user32.SendMessageTimeoutW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM, w.UINT, w.UINT,
                                           ctypes.POINTER(ctypes.c_size_t)]
    # Windows promises no order, and its stacking put GPUI's hidden window first only while ilo was
    # behind another app's, which exited the process under Chromium (#2617): the hidden ones first.
    found = sorted(windows(pid), key=lambda hwnd: bool(user32.IsWindowVisible(hwnd)))
    for hwnd in found:
        answer = ctypes.c_size_t()
        sent = user32.SendMessageTimeoutW(hwnd, WM_QUERYENDSESSION, 0, ENDSESSION_LOGOFF,
                                          SMTO_ABORTIFHUNG, 5000, ctypes.byref(answer))
        if not sent or not answer.value:
            sys.exit("window %#x of %d refused the session end" % (hwnd or 0, pid))
    for hwnd in found:
        answer = ctypes.c_size_t()
        user32.SendMessageTimeoutW(hwnd, WM_ENDSESSION, 1, ENDSESSION_LOGOFF, SMTO_ABORTIFHUNG,
                                   30000, ctypes.byref(answer))


def bring_forward(pid, title):
    """Gives the foreground to process PID's browser window in front of its others, or with TITLE
    to its window of that title, such as Settings' "Sync", as a click on it would. The driver's
    window command can't: it activates a window within its own app alone, and the runs with several
    copies of the app need the one they drive in front (#2114). Windows lets a process take the
    foreground only while it shares the input of the thread that has it, so this one attaches to
    that thread for the moment. Fails if the window doesn't have the foreground 2 s later."""
    user32.GetForegroundWindow.restype = w.HWND
    kernel32 = ctypes.windll.kernel32
    found = [hwnd for hwnd in windows(pid) if user32.IsWindowVisible(hwnd) and
             (text(user32.GetWindowTextW, hwnd) == title if title else holds_ui(hwnd))]
    if not found:
        sys.exit("process %d has no %s on screen" % (pid, "window %r" % title if title else
                                                     "browser window"))
    target = found[0]
    if user32.GetForegroundWindow() != target:
        held = user32.GetWindowThreadProcessId(user32.GetForegroundWindow(), None)
        ours = kernel32.GetCurrentThreadId()
        attached = held and held != ours and user32.AttachThreadInput(ours, held, True)
        user32.BringWindowToTop(w.HWND(target))
        user32.SetForegroundWindow(w.HWND(target))
        if attached:
            user32.AttachThreadInput(ours, held, False)
    for _ in range(20):
        if user32.GetForegroundWindow() == target:
            print("foreground %#x" % target)
            return
        time.sleep(0.1)
    sys.exit("window %#x of %d didn't take the foreground" % (target, pid))


def cover(pid, foreground, front_file):
    """Another app's plain window over the lower half of the browser window of process PID, where
    the link target's corner is, with ilo's window under it, and the foreground in ilo's window
    (FOREGROUND "ilo"), as when a window opens over ilo without taking the keyboard, or in a small
    window away from ilo's ("away"), as the user's app on another part of the screen. Returns once
    the browser's process has ended, or once FRONT_FILE exists, having brought the browser window
    to the front, which Windows lets the app with the foreground do."""
    user32.SetProcessDPIAware()
    user32.GetForegroundWindow.restype = w.HWND
    shown = [hwnd for hwnd in windows(pid) if holds_ui(hwnd) and user32.IsWindowVisible(hwnd)]
    in_front = user32.GetForegroundWindow()
    # With behind, the backmost: the one with the foreground stays in front of the cover.
    browser = (shown[-1] if foreground == "behind" else shown[0]) if shown else None
    if not browser:
        sys.exit("process %d has no browser window on screen" % pid)
    frame = w.RECT()
    user32.GetWindowRect(w.HWND(browser), ctypes.byref(frame))
    WNDPROC = ctypes.WINFUNCTYPE(w.LPARAM, w.HWND, w.UINT, w.WPARAM, w.LPARAM)
    user32.DefWindowProcW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
    user32.DefWindowProcW.restype = w.LPARAM
    procedure = WNDPROC(lambda *message: user32.DefWindowProcW(*message))
    kernel32 = ctypes.windll.kernel32
    kernel32.GetModuleHandleW.restype = w.HMODULE
    gdi32.CreateSolidBrush.restype = w.HBRUSH
    instance = kernel32.GetModuleHandleW(None)
    window_class = WNDCLASSW(lpfnWndProc=ctypes.cast(procedure, ctypes.c_void_p),
                             hInstance=instance, hbrBackground=gdi32.CreateSolidBrush(0x806040),
                             lpszClassName="CheckCover")
    if not user32.RegisterClassW(ctypes.byref(window_class)):
        sys.exit("couldn't register the cover's class")
    user32.CreateWindowExW.argtypes = [w.DWORD, w.LPCWSTR, w.LPCWSTR, w.DWORD, ctypes.c_int,
                                       ctypes.c_int, ctypes.c_int, ctypes.c_int, w.HWND, w.HMENU,
                                       w.HINSTANCE, w.LPVOID]
    user32.CreateWindowExW.restype = w.HWND
    user32.SetWindowPos.argtypes = [w.HWND, w.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                                    ctypes.c_int, w.UINT]
    user32.GetForegroundWindow.restype = w.HWND
    SW_SHOWNOACTIVATE, SW_SHOW, SWP_NOSIZE, SWP_NOMOVE, SWP_NOACTIVATE = 4, 5, 0x1, 0x2, 0x10

    def show(title, x, y, width, height, front):
        hwnd = user32.CreateWindowExW(0, "CheckCover", title, 0x80000000 | 0x00800000, x, y,
                                      width, height, None, None, instance, None)
        if not hwnd:
            sys.exit("couldn't open the %s window" % title)
        user32.ShowWindow(w.HWND(hwnd), SW_SHOW if front else SW_SHOWNOACTIVATE)
        if front:
            user32.SetForegroundWindow(w.HWND(hwnd))
        return hwnd

    middle = (frame.top + frame.bottom) // 2
    cover = show("cover", frame.left, middle, frame.right - frame.left, frame.bottom - middle,
                 False)
    if foreground == "away":
        # Beside ilo's window where the screen has room, so it's over none of it.
        screen = (user32.GetSystemMetrics(0), user32.GetSystemMetrics(1))
        size = (160, 120)
        if frame.right + 20 + size[0] <= screen[0]:
            at = (frame.right + 20, frame.top)
        elif frame.left - 20 - size[0] >= 0:
            at = (frame.left - 20 - size[0], frame.top)
        elif frame.bottom + 20 + size[1] <= screen[1]:
            at = (frame.left, frame.bottom + 20)
        else:
            at = (screen[0] - size[0], 0)
        show("away", at[0], at[1], size[0], size[1], True)
    # ilo's window goes under the cover, rather than the cover over it, as Windows keeps a window
    # that doesn't have the foreground from going over the window that has it.
    if foreground == "behind":
        # Right under the window with the foreground, and the others under it, in their order.
        user32.SetWindowPos(cover, in_front, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE)
        for hwnd in reversed([hwnd for hwnd in shown if hwnd != in_front]):
            user32.SetWindowPos(hwnd, cover, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE)
    else:
        user32.SetWindowPos(browser, cover, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE)
    # Until it's ended, or the browser's process ends, so it's never over a later run's windows.
    SYNCHRONIZE, INFINITE, QS_ALLINPUT, PM_REMOVE = 0x100000, 0xFFFFFFFF, 0x4FF, 1
    kernel32.OpenProcess.argtypes = [w.DWORD, w.BOOL, w.DWORD]
    kernel32.OpenProcess.restype = w.HANDLE
    user32.MsgWaitForMultipleObjects.argtypes = [w.DWORD, ctypes.POINTER(w.HANDLE), w.BOOL,
                                                 w.DWORD, w.DWORD]
    user32.MsgWaitForMultipleObjects.restype = w.DWORD
    browser_process = w.HANDLE(kernel32.OpenProcess(SYNCHRONIZE, False, pid))
    if not browser_process:
        sys.exit("couldn't watch process %d" % pid)
    owner = w.DWORD()
    front = user32.GetForegroundWindow()
    user32.GetWindowThreadProcessId(front, ctypes.byref(owner))
    print("cover %#x, foreground %s" % (cover, "ilo" if owner.value == pid else
                                        "away" if text(user32.GetWindowTextW, front) == "away"
                                        else "other"), flush=True)
    message = w.MSG()
    WAIT_TIMEOUT = 0x102
    # Looking for FRONT_FILE every 100 ms, if there's one to look for.
    while user32.MsgWaitForMultipleObjects(1, ctypes.byref(browser_process), False,
                                           100 if front_file else INFINITE,
                                           QS_ALLINPUT) in (1, WAIT_TIMEOUT):
        if front_file and os.path.exists(front_file):
            user32.SetForegroundWindow(w.HWND(browser))
            return
        while user32.PeekMessageW(ctypes.byref(message), None, 0, 0, PM_REMOVE):
            user32.TranslateMessage(ctypes.byref(message))
            user32.DispatchMessageW(ctypes.byref(message))


class PROCESSENTRY32W(ctypes.Structure):
    _fields_ = [
        ("dwSize", w.DWORD),
        ("cntUsage", w.DWORD),
        ("th32ProcessID", w.DWORD),
        ("th32DefaultHeapID", ctypes.c_size_t),
        ("th32ModuleID", w.DWORD),
        ("cntThreads", w.DWORD),
        ("th32ParentProcessID", w.DWORD),
        ("pcPriClassBase", w.LONG),
        ("dwFlags", w.DWORD),
        ("szExeFile", w.WCHAR * 260),
    ]


def family(pid):
    """Process pid and the processes it started, and theirs."""
    kernel32 = ctypes.windll.kernel32
    kernel32.CreateToolhelp32Snapshot.restype = w.HANDLE
    each = [w.HANDLE, ctypes.POINTER(PROCESSENTRY32W)]
    kernel32.Process32FirstW.argtypes = kernel32.Process32NextW.argtypes = each
    kernel32.CloseHandle.argtypes = [w.HANDLE]
    snapshot = kernel32.CreateToolhelp32Snapshot(0x2, 0)  # TH32CS_SNAPPROCESS
    parents = {}
    entry = PROCESSENTRY32W()
    entry.dwSize = ctypes.sizeof(entry)
    more = kernel32.Process32FirstW(snapshot, ctypes.byref(entry))
    while more:
        parents[entry.th32ProcessID] = entry.th32ParentProcessID
        more = kernel32.Process32NextW(snapshot, ctypes.byref(entry))
    kernel32.CloseHandle(snapshot)
    found = {pid}
    grew = True
    while grew:
        grew = False
        for child, parent in parents.items():
            if parent in found and child not in found:
                found.add(child)
                grew = True
    return found


def dialogs(pid, cancel):
    """Prints the dialogs shown (class #32770) of process pid and the processes it started, as
    Chrome's file dialogs may be a utility process's, and with cancel ends each as its Cancel does."""
    processes = family(pid)
    found = []

    def each(hwnd, _):
        owner = w.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if (owner.value in processes and user32.IsWindowVisible(hwnd)
                and text(user32.GetClassNameW, hwnd) == "#32770"):
            found.append((hwnd, owner.value))
        return True

    user32.EnumWindows(EnumProc(each), 0)
    for hwnd, owner in found:
        print('%#x "%s" process %d' % (hwnd, text(user32.GetWindowTextW, hwnd), owner))
        if cancel:
            user32.PostMessageW(hwnd, WM_COMMAND, IDCANCEL, 0)


def main():
    command = sys.argv[1] if len(sys.argv) > 1 else ""
    front = command == "close" and sys.argv[3:] == ["front"]
    known = ("close", "end-session", "list", "icon", "cover", "foreground", "dialogs")
    wanted = ((4, 5) if command == "cover" else (3, 4) if command in ("foreground", "dialogs") else
              (4,) if command == "icon" or front else (3,))
    if len(sys.argv) not in wanted or command not in known:
        sys.exit(__doc__.splitlines()[-1])
    pid = int(sys.argv[2])
    if command == "foreground":
        bring_forward(pid, sys.argv[3] if len(sys.argv) == 4 else None)
        return
    if command == "dialogs":
        if sys.argv[3:] not in ([], ["cancel"]):
            sys.exit(__doc__.splitlines()[-1])
        dialogs(pid, sys.argv[3:] == ["cancel"])
        return
    if command == "icon":
        sys.exit(0 if check_icons(pid, sys.argv[3]) else 1)
    if command == "cover":
        if sys.argv[3] not in ("ilo", "away", "behind"):
            sys.exit(__doc__.splitlines()[-1])
        cover(pid, sys.argv[3], sys.argv[4] if len(sys.argv) == 5 else None)
        return
    if command == "end-session":
        end_session(pid)
        return
    user32.GetForegroundWindow.restype = w.HWND
    foreground = user32.GetForegroundWindow()
    for hwnd in windows(pid):
        browser = holds_ui(hwnd)
        if command == "close":
            if browser:
                user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)
                if front:
                    break
            continue
        states = [
            "browser" if browser else "",
            "visible" if user32.IsWindowVisible(hwnd) else "hidden",
            "disabled" if not user32.IsWindowEnabled(hwnd) else "",
            "owned" if user32.GetWindow(hwnd, GW_OWNER) else "",
            "minimized" if user32.IsIconic(hwnd) else "",
            "foreground" if hwnd == foreground else "",
        ]
        print(
            '%#x %s "%s" %s'
            % (
                hwnd or 0,
                text(user32.GetClassNameW, hwnd),
                text(user32.GetWindowTextW, hwnd),
                " ".join(state for state in states if state),
            )
        )


main()
