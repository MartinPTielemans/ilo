"""The app's top-level windows on Windows, for lib.sh's quit and what it says of an app that didn't.

`close PID` asks each browser window of process PID to close, as its close button does: the
top-level windows holding the UI's view (platform/windows/host.cc's BrowserUI). `list PID` prints
each top-level window of the process, with its class, title, state and whether it's a browser
window. `icon PID ICO` checks that each window of the process the taskbar and Alt+Tab show has the
.ico's image for its big and small icons, read as the shell reads them, and prints them (#1350).
Usage: app-windows.py close|list|icon PID [ICO]"""

import ctypes
import ctypes.wintypes as w
import sys

# Git Bash's grep reads UTF-8 lines ending in \n alone.
sys.stdout.reconfigure(encoding="utf-8", newline="\n")
user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32
WM_CLOSE = 0x0010
GW_OWNER = 4
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


def main():
    command = sys.argv[1] if len(sys.argv) > 1 else ""
    if len(sys.argv) != (4 if command == "icon" else 3) or command not in ("close", "list", "icon"):
        sys.exit(__doc__.splitlines()[-1])
    pid = int(sys.argv[2])
    if command == "icon":
        sys.exit(0 if check_icons(pid, sys.argv[3]) else 1)
    for hwnd in windows(pid):
        browser = holds_ui(hwnd)
        if command == "close":
            if browser:
                user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)
            continue
        states = [
            "browser" if browser else "",
            "visible" if user32.IsWindowVisible(hwnd) else "hidden",
            "disabled" if not user32.IsWindowEnabled(hwnd) else "",
            "owned" if user32.GetWindow(hwnd, GW_OWNER) else "",
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
