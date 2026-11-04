"""Another app's window on X11, for a run that needs ilo in the background: a plain window of its own
client, mapped and named "Other app", whose id it prints once the X server has it. The run makes it
active with xdotool, as a click on it would, and it goes once stdin closes.
Usage: python3 -I check/other-window.py < fifo"""
import ctypes
import sys

x11 = ctypes.CDLL("libX11.so.6")
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
x11.XDefaultRootWindow.restype = ctypes.c_ulong
x11.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
x11.XCreateSimpleWindow.restype = ctypes.c_ulong
x11.XCreateSimpleWindow.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_int,
                                    ctypes.c_uint, ctypes.c_uint, ctypes.c_uint, ctypes.c_ulong,
                                    ctypes.c_ulong]
x11.XStoreName.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_char_p]
x11.XMapRaised.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]

display = x11.XOpenDisplay(None)
if not display:
    sys.exit("no X display")
window = x11.XCreateSimpleWindow(display, x11.XDefaultRootWindow(display), 40, 40, 320, 200, 0, 0,
                                 0xFFFFFF)
x11.XStoreName(display, window, b"Other app")
x11.XMapRaised(display, window)
x11.XSync(display, 0)
print(window, flush=True)
sys.stdin.read()
