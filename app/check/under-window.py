"""Another app's window under the browser window of process PID on Windows, for panel-shadows (#2988).

`open PID` opens a plain window of this process's, in one colour, reaching 160 pixels past the
browser window's frame on each side where the screen has room, under that window in Windows' order,
so what's past the frame's edge is this other app's. It says "under <handle>, frame L T R B, window
L T R B, client X Y": the frame in pixels on screen, then from the frame's top left the browser
window's own rectangle, with the clear border round the frame Windows resizes it by, and its
content's top left. Then "pressed X Y" for each press it hears, in pixels from the frame's
top left, and keeps it until it's ended or the browser's process ends. A press brings it in front,
as an app's does; once released, it goes back under the browser window, so the next step finds the
browser window over it, as the user's own windows would be.

`strip PID L T R B OUT [BEFORE]` reads the screen's pixels from L, T to R, B, in pixels from the
browser window's frame's top left, as the screen shows them, into OUT, and with BEFORE, says how
many changed from those read into it, failing if any did.
Usage: under-window.py open PID or strip PID L T R B OUT [BEFORE]"""

import ctypes
import ctypes.wintypes as w
import sys

# Git Bash's grep reads UTF-8 lines ending in \n alone.
sys.stdout.reconfigure(encoding="utf-8", newline="\n")
user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32
kernel32 = ctypes.windll.kernel32
dwmapi = ctypes.windll.dwmapi
DWMWA_EXTENDED_FRAME_BOUNDS = 9
# What the window shows, as GDI's COLORREF and as the screen's red, green and blue.
FILL = 0x806040
FILL_RGB = (0x40, 0x60, 0x80)
EnumProc = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
WNDPROC = ctypes.WINFUNCTYPE(w.LPARAM, w.HWND, w.UINT, w.WPARAM, w.LPARAM)


class WNDCLASSW(ctypes.Structure):
    _fields_ = [("style", w.UINT), ("lpfnWndProc", ctypes.c_void_p), ("cbClsExtra", ctypes.c_int),
                ("cbWndExtra", ctypes.c_int), ("hInstance", w.HINSTANCE), ("hIcon", w.HICON),
                ("hCursor", w.HANDLE), ("hbrBackground", w.HBRUSH), ("lpszMenuName", w.LPCWSTR),
                ("lpszClassName", w.LPCWSTR)]


class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [("biSize", w.DWORD), ("biWidth", w.LONG), ("biHeight", w.LONG),
                ("biPlanes", w.WORD), ("biBitCount", w.WORD), ("biCompression", w.DWORD),
                ("biSizeImage", w.DWORD), ("biXPelsPerMeter", w.LONG),
                ("biYPelsPerMeter", w.LONG), ("biClrUsed", w.DWORD), ("biClrImportant", w.DWORD)]


def aware():
    # In pixels, as the browser's frame and the check's captures are.
    try:
        user32.SetProcessDpiAwarenessContext(w.HANDLE(-4))
    except AttributeError:
        user32.SetProcessDPIAware()


def browser_window(pid):
    """The process's first browser window on screen: a top-level window holding the UI's view."""
    found = []

    def each(hwnd, _):
        owner = w.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if (owner.value == pid and user32.IsWindowVisible(hwnd) and
                user32.FindWindowExW(hwnd, None, "BrowserUI", None)):
            found.append(hwnd)
        return True

    user32.EnumWindows(EnumProc(each), 0)
    if not found:
        sys.exit("process %d has no browser window on screen" % pid)
    return found[0]


def frame_of(hwnd):
    frame = w.RECT()
    if dwmapi.DwmGetWindowAttribute(w.HWND(hwnd), DWMWA_EXTENDED_FRAME_BOUNDS,
                                    ctypes.byref(frame), ctypes.sizeof(frame)):
        user32.GetWindowRect(w.HWND(hwnd), ctypes.byref(frame))
    return frame


def open_under(pid):
    aware()
    browser = browser_window(pid)
    frame = frame_of(browser)
    left, top = user32.GetSystemMetrics(76), user32.GetSystemMetrics(77)
    right, bottom = left + user32.GetSystemMetrics(78), top + user32.GetSystemMetrics(79)
    reach = 160
    own = (max(left, frame.left - reach), max(top, frame.top - reach),
           min(right, frame.right + reach), min(bottom, frame.bottom + reach))
    user32.DefWindowProcW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
    user32.DefWindowProcW.restype = w.LPARAM
    user32.SetWindowPos.argtypes = [w.HWND, w.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                                    ctypes.c_int, w.UINT]
    SWP_NOSIZE, SWP_NOMOVE, SWP_NOACTIVATE = 0x1, 0x2, 0x10
    WM_LBUTTONDOWN, WM_LBUTTONUP = 0x0201, 0x0202

    def under_browser(hwnd):
        user32.SetWindowPos(hwnd, w.HWND(browser), 0, 0, 0, 0,
                            SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE)

    def procedure(hwnd, message, wparam, lparam):
        if message == WM_LBUTTONDOWN:
            x = ctypes.c_short(lparam & 0xFFFF).value
            y = ctypes.c_short((lparam >> 16) & 0xFFFF).value
            print("pressed %d %d" % (own[0] + x - frame.left, own[1] + y - frame.top), flush=True)
        elif message == WM_LBUTTONUP:
            under_browser(hwnd)
        return user32.DefWindowProcW(hwnd, message, wparam, lparam)

    callback = WNDPROC(procedure)
    kernel32.GetModuleHandleW.restype = w.HMODULE
    gdi32.CreateSolidBrush.restype = w.HBRUSH
    instance = kernel32.GetModuleHandleW(None)
    window_class = WNDCLASSW(lpfnWndProc=ctypes.cast(callback, ctypes.c_void_p),
                             hInstance=instance, hbrBackground=gdi32.CreateSolidBrush(FILL),
                             lpszClassName="CheckUnder")
    if not user32.RegisterClassW(ctypes.byref(window_class)):
        sys.exit("couldn't register the window's class")
    user32.CreateWindowExW.argtypes = [w.DWORD, w.LPCWSTR, w.LPCWSTR, w.DWORD, ctypes.c_int,
                                       ctypes.c_int, ctypes.c_int, ctypes.c_int, w.HWND, w.HMENU,
                                       w.HINSTANCE, w.LPVOID]
    user32.CreateWindowExW.restype = w.HWND
    # A pop-up, with no frame of its own, so all of it is the one colour.
    hwnd = user32.CreateWindowExW(0, "CheckUnder", "under", 0x80000000, own[0], own[1],
                                  own[2] - own[0], own[3] - own[1], None, None, instance, None)
    if not hwnd:
        sys.exit("couldn't open the window")
    user32.ShowWindow(hwnd, 4)  # SW_SHOWNOACTIVATE
    under_browser(hwnd)
    # The browser window's own rectangle, with the border Windows leaves clear round its frame for
    # resizing, which takes a press there, and its content's top left, from the frame's top left.
    rect = w.RECT()
    user32.GetWindowRect(w.HWND(browser), ctypes.byref(rect))
    client = w.POINT(0, 0)
    user32.ClientToScreen(w.HWND(browser), ctypes.byref(client))
    print("under %#x, frame %d %d %d %d, window %d %d %d %d, client %d %d" %
          (hwnd, frame.left, frame.top, frame.right, frame.bottom, rect.left - frame.left,
           rect.top - frame.top, rect.right - frame.left, rect.bottom - frame.top,
           client.x - frame.left, client.y - frame.top), flush=True)
    SYNCHRONIZE, INFINITE, QS_ALLINPUT, PM_REMOVE = 0x100000, 0xFFFFFFFF, 0x4FF, 1
    kernel32.OpenProcess.restype = w.HANDLE
    process = w.HANDLE(kernel32.OpenProcess(SYNCHRONIZE, False, pid))
    if not process:
        sys.exit("couldn't watch process %d" % pid)
    user32.MsgWaitForMultipleObjects.argtypes = [w.DWORD, ctypes.POINTER(w.HANDLE), w.BOOL,
                                                 w.DWORD, w.DWORD]
    message = w.MSG()
    while user32.MsgWaitForMultipleObjects(1, ctypes.byref(process), False, INFINITE,
                                           QS_ALLINPUT) == 1:
        while user32.PeekMessageW(ctypes.byref(message), None, 0, 0, PM_REMOVE):
            user32.TranslateMessage(ctypes.byref(message))
            user32.DispatchMessageW(ctypes.byref(message))


def read_screen(left, top, width, height):
    """The screen's pixels in that rectangle, as rows of blue, green, red and a spare byte."""
    user32.GetDC.restype = w.HDC
    gdi32.CreateCompatibleDC.restype = w.HDC
    gdi32.CreateCompatibleBitmap.restype = w.HBITMAP
    gdi32.SelectObject.restype = w.HGDIOBJ
    screen = user32.GetDC(None)
    memory = gdi32.CreateCompatibleDC(w.HDC(screen))
    bitmap = gdi32.CreateCompatibleBitmap(w.HDC(screen), width, height)
    gdi32.SelectObject(w.HDC(memory), w.HGDIOBJ(bitmap))
    SRCCOPY = 0x00CC0020
    if not gdi32.BitBlt(w.HDC(memory), 0, 0, width, height, w.HDC(screen), left, top, SRCCOPY):
        sys.exit("couldn't read the screen")
    # Top row first.
    header = BITMAPINFOHEADER(biSize=ctypes.sizeof(BITMAPINFOHEADER), biWidth=width,
                              biHeight=-height, biPlanes=1, biBitCount=32, biCompression=0)
    pixels = ctypes.create_string_buffer(width * height * 4)
    if gdi32.GetDIBits(w.HDC(memory), w.HBITMAP(bitmap), 0, height, pixels, ctypes.byref(header),
                       0) != height:
        sys.exit("couldn't read the screen's pixels")
    gdi32.DeleteObject(w.HGDIOBJ(bitmap))
    gdi32.DeleteDC(w.HDC(memory))
    user32.ReleaseDC(None, w.HDC(screen))
    return pixels.raw


def strip(pid, left, top, right, bottom, out, before):
    aware()
    frame = frame_of(browser_window(pid))
    width, height = right - left, bottom - top
    if width <= 0 or height <= 0:
        sys.exit("the strip %d %d %d %d is empty" % (left, top, right, bottom))
    pixels = read_screen(frame.left + left, frame.top + top, width, height)
    with open(out, "wb") as file:
        file.write(pixels)
    filled = sum(1 for at in range(0, len(pixels), 4)
                 if (pixels[at + 2], pixels[at + 1], pixels[at]) == FILL_RGB)
    if before is None:
        print("strip %d %d %d %d: %d pixels, %d the other app's colour" %
              (left, top, right, bottom, width * height, filled))
        return True
    with open(before, "rb") as file:
        was = file.read()
    if len(was) != len(pixels):
        sys.exit("the strip read before is %d bytes, not %d" % (len(was), len(pixels)))
    changed, most, most_at = 0, 0, 0
    for at in range(0, len(pixels), 4):
        apart = max(abs(pixels[at + c] - was[at + c]) for c in range(3))
        if apart > 8:
            changed += 1
        if apart > most:
            most, most_at = apart, at
    if not changed:
        print("strip %d %d %d %d: as before, all %d pixels, %d the other app's colour" %
              (left, top, right, bottom, width * height, filled))
        return True
    x, y = left + most_at // 4 % width, top + most_at // 4 // width
    print("strip %d %d %d %d: %d of %d pixels changed, the most at %d,%d, from %s to %s" %
          (left, top, right, bottom, changed, width * height, x, y,
           tuple(was[most_at + 2 - c] for c in range(3)),
           tuple(pixels[most_at + 2 - c] for c in range(3))))
    return False


def main():
    command = sys.argv[1] if len(sys.argv) > 1 else ""
    if command == "open" and len(sys.argv) == 3:
        open_under(int(sys.argv[2]))
    elif command == "strip" and len(sys.argv) in (8, 9):
        left, top, right, bottom = (int(value) for value in sys.argv[3:7])
        sys.exit(0 if strip(int(sys.argv[2]), left, top, right, bottom, sys.argv[7],
                            sys.argv[8] if len(sys.argv) == 9 else None) else 1)
    else:
        sys.exit(__doc__.splitlines()[-1])


main()
