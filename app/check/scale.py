"""The scale of the display Windows shows the app on, changed while the app runs, as Settings' Scale
changes it: Windows tells each window on it its new DPI (WM_DPICHANGED), as when a window moves to
a display of another scale (#1358). The hosted runner has one display, so this is the one way to
give the app's windows a new DPI there.

`get` prints the first display's steps from the one Windows recommends: "current minimum maximum",
such as "0 0 1" for 100% of 100% to 125%. `set N` makes it step N and waits until a window of the
helper's own has heard its new DPI, then prints that DPI.
Usage: scale.py get|set N"""

import ctypes
import ctypes.wintypes as w
import sys
import time

user32 = ctypes.WinDLL("user32", use_last_error=True)
user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))  # per monitor v2


class LUID(ctypes.Structure):
    _fields_ = [("LowPart", w.DWORD), ("HighPart", w.LONG)]


class Header(ctypes.Structure):
    _fields_ = [("type", ctypes.c_int32), ("size", ctypes.c_uint32), ("adapterId", LUID),
                ("id", ctypes.c_uint32)]


# DISPLAYCONFIG_DEVICE_INFO_GET_DPI_SCALE and _SET_DPI_SCALE, which Settings uses and the SDK
# doesn't declare: steps relative to the display's recommended scale.
class GetScale(ctypes.Structure):
    _fields_ = [("header", Header), ("minimum", ctypes.c_int32), ("current", ctypes.c_int32),
                ("maximum", ctypes.c_int32)]


class SetScale(ctypes.Structure):
    _fields_ = [("header", Header), ("relative", ctypes.c_int32)]


class PathInfo(ctypes.Structure):
    _fields_ = [("source_adapter", LUID), ("source_id", ctypes.c_uint32), ("rest", ctypes.c_byte * 60)]


class ModeInfo(ctypes.Structure):
    _fields_ = [("bytes", ctypes.c_byte * 64)]


def first_display():
    paths, modes = ctypes.c_uint32(), ctypes.c_uint32()
    QDC_ONLY_ACTIVE_PATHS = 2
    if user32.GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, ctypes.byref(paths), ctypes.byref(modes)):
        sys.exit("scale: no display config")
    path_array = (PathInfo * paths.value)()
    mode_array = (ModeInfo * modes.value)()
    if user32.QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, ctypes.byref(paths), path_array,
                                 ctypes.byref(modes), mode_array, None) or not paths.value:
        sys.exit("scale: no display")
    return path_array[0]


def steps(path):
    get = GetScale()
    get.header = Header(-3, ctypes.sizeof(GetScale), path.source_adapter, path.source_id)
    if user32.DisplayConfigGetDeviceInfo(ctypes.byref(get)):
        sys.exit("scale: the display's scale can't be read")
    return get.current, get.minimum, get.maximum


WNDPROC = ctypes.WINFUNCTYPE(ctypes.c_ssize_t, w.HWND, w.UINT, w.WPARAM, w.LPARAM)
user32.DefWindowProcW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
user32.DefWindowProcW.restype = ctypes.c_ssize_t
user32.CreateWindowExW.restype = w.HWND
user32.GetDpiForWindow.argtypes = [w.HWND]


class WNDCLASS(ctypes.Structure):
    _fields_ = [("style", w.UINT), ("lpfnWndProc", WNDPROC), ("cbClsExtra", ctypes.c_int),
                ("cbWndExtra", ctypes.c_int), ("hInstance", w.HINSTANCE), ("hIcon", w.HICON),
                ("hCursor", w.HANDLE), ("hbrBackground", w.HBRUSH), ("lpszMenuName", w.LPCWSTR),
                ("lpszClassName", w.LPCWSTR)]


def set_step(path, relative):
    heard = []

    def proc(hwnd, message, wparam, lparam):
        if message == 0x02E0:  # WM_DPICHANGED
            heard.append(wparam & 0xFFFF)
        return user32.DefWindowProcW(hwnd, message, wparam, lparam)

    callback = WNDPROC(proc)
    user32.RegisterClassW(ctypes.byref(WNDCLASS(lpfnWndProc=callback, lpszClassName="ScaleHelper")))
    # A hidden window of its own on the display, which Windows tells as it tells the app's.
    hwnd = user32.CreateWindowExW(0x80, "ScaleHelper", "", 0x80000000, 0, 0, 10, 10, None, None, None,
                                  None)
    before = user32.GetDpiForWindow(hwnd)
    put = SetScale()
    put.header = Header(-4, ctypes.sizeof(SetScale), path.source_adapter, path.source_id)
    put.relative = relative
    if user32.DisplayConfigSetDeviceInfo(ctypes.byref(put)):
        sys.exit("scale: the display's scale can't be set")
    message = w.MSG()
    end = time.time() + 10
    while time.time() < end:
        while user32.PeekMessageW(ctypes.byref(message), None, 0, 0, 1):
            user32.DispatchMessageW(ctypes.byref(message))
        if user32.GetDpiForWindow(hwnd) != before or steps(path)[0] == relative and heard:
            break
        time.sleep(0.02)
    print(user32.GetDpiForWindow(hwnd))
    user32.DestroyWindow(hwnd)


def main():
    if sys.argv[1:2] == ["get"] and len(sys.argv) == 2:
        print(*steps(first_display()))
    elif sys.argv[1:2] == ["set"] and len(sys.argv) == 3:
        set_step(first_display(), int(sys.argv[2]))
    else:
        sys.exit(__doc__)


main()
