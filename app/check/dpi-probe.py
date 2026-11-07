# Probe for #1358, never for main: whether a hosted Windows runner's display can change its scale
# while windows are open, as moving a window to a display of another scale does, through the call
# Settings' Scale uses (DisplayConfigSetDeviceInfo's DPI scale). Prints each display's scale steps,
# steps the first up one, reads the monitor's DPI and a window's WM_DPICHANGED, and steps it back.
import ctypes
import ctypes.wintypes as w
import sys
import time

user32 = ctypes.WinDLL("user32", use_last_error=True)
shcore = ctypes.WinDLL("shcore")
user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))  # per monitor v2


class LUID(ctypes.Structure):
    _fields_ = [("LowPart", w.DWORD), ("HighPart", w.LONG)]


class Header(ctypes.Structure):
    _fields_ = [("type", ctypes.c_int32), ("size", ctypes.c_uint32), ("adapterId", LUID),
                ("id", ctypes.c_uint32)]


class GetScale(ctypes.Structure):
    _fields_ = [("header", Header), ("minimum", ctypes.c_int32), ("current", ctypes.c_int32),
                ("maximum", ctypes.c_int32)]


class SetScale(ctypes.Structure):
    _fields_ = [("header", Header), ("relative", ctypes.c_int32)]


class PathInfo(ctypes.Structure):
    _fields_ = [("source_adapter", LUID), ("source_id", ctypes.c_uint32), ("rest", ctypes.c_byte * 60)]


class ModeInfo(ctypes.Structure):
    _fields_ = [("bytes", ctypes.c_byte * 64)]


assert ctypes.sizeof(PathInfo) == 72 and ctypes.sizeof(ModeInfo) == 64

paths, modes = ctypes.c_uint32(), ctypes.c_uint32()
QDC_ONLY_ACTIVE_PATHS = 2
print("buffer sizes:", user32.GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, ctypes.byref(paths),
                                                          ctypes.byref(modes)), paths.value, modes.value)
path_array = (PathInfo * paths.value)()
mode_array = (ModeInfo * modes.value)()
print("query:", user32.QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, ctypes.byref(paths), path_array,
                                          ctypes.byref(modes), mode_array, None))


user32.MonitorFromPoint.argtypes = [w.POINT, w.DWORD]
user32.MonitorFromPoint.restype = ctypes.c_void_p


def monitor_dpi():
    monitor = user32.MonitorFromPoint(w.POINT(0, 0), 1)
    x, y = ctypes.c_uint(), ctypes.c_uint()
    shcore.GetDpiForMonitor(ctypes.c_void_p(monitor), 0, ctypes.byref(x), ctypes.byref(y))
    return x.value


def scale(path):
    get = GetScale()
    get.header = Header(-3, ctypes.sizeof(GetScale), path.source_adapter, path.source_id)
    result = user32.DisplayConfigGetDeviceInfo(ctypes.byref(get))
    return result, get.minimum, get.current, get.maximum


def set_scale(path, relative):
    put = SetScale()
    put.header = Header(-4, ctypes.sizeof(SetScale), path.source_adapter, path.source_id)
    put.relative = relative
    return user32.DisplayConfigSetDeviceInfo(ctypes.byref(put))


# A window of our own, to see whether Windows tells it the scale changed.
heard = []
WNDPROC = ctypes.WINFUNCTYPE(ctypes.c_ssize_t, w.HWND, w.UINT, w.WPARAM, w.LPARAM)
user32.DefWindowProcW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
user32.DefWindowProcW.restype = ctypes.c_ssize_t


def proc(hwnd, message, wparam, lparam):
    if message == 0x02E0:  # WM_DPICHANGED
        rect = ctypes.cast(lparam, ctypes.POINTER(w.RECT)).contents
        heard.append((wparam & 0xFFFF, rect.left, rect.top, rect.right, rect.bottom))
    return user32.DefWindowProcW(hwnd, message, wparam, lparam)


callback = WNDPROC(proc)


class WNDCLASS(ctypes.Structure):
    _fields_ = [("style", w.UINT), ("lpfnWndProc", WNDPROC), ("cbClsExtra", ctypes.c_int),
                ("cbWndExtra", ctypes.c_int), ("hInstance", w.HINSTANCE), ("hIcon", w.HICON),
                ("hCursor", w.HANDLE), ("hbrBackground", w.HBRUSH), ("lpszMenuName", w.LPCWSTR),
                ("lpszClassName", w.LPCWSTR)]


window_class = WNDCLASS(lpfnWndProc=callback, lpszClassName="DpiProbe")
user32.RegisterClassW(ctypes.byref(window_class))
user32.CreateWindowExW.restype = w.HWND
hwnd = user32.CreateWindowExW(0, "DpiProbe", "dpi probe", 0x10CF0000, 100, 100, 400, 300, None, None,
                              None, None)
user32.GetDpiForWindow.argtypes = [w.HWND]


def pump(seconds):
    end = time.time() + seconds
    message = w.MSG()
    while time.time() < end:
        while user32.PeekMessageW(ctypes.byref(message), None, 0, 0, 1):
            user32.TranslateMessage(ctypes.byref(message))
            user32.DispatchMessageW(ctypes.byref(message))
        time.sleep(0.02)


pump(0.5)
print("window", hwnd, "dpi", user32.GetDpiForWindow(hwnd), "monitor dpi", monitor_dpi())
for index in range(paths.value):
    path = path_array[index]
    print("path", index, "source", path.source_id, "scale (result, min, current, max):", scale(path))
if paths.value == 0:
    sys.exit("no display path")
path = path_array[0]
result, minimum, current, maximum = scale(path)
target = current + 1 if current < maximum else current - 1
print("set to", target, "->", set_scale(path, target))
pump(3)
print("after: window dpi", user32.GetDpiForWindow(hwnd), "monitor dpi", monitor_dpi(),
      "scale", scale(path), "WM_DPICHANGED heard", heard)
print("back to", current, "->", set_scale(path, current))
pump(3)
print("restored: window dpi", user32.GetDpiForWindow(hwnd), "monitor dpi", monitor_dpi(),
      "scale", scale(path), "WM_DPICHANGED heard", heard)
