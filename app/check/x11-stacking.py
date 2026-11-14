"""Whether the overlays ilo's process $2 shows are under another app's window $1 in the X window
manager's stacking (_NET_CLIENT_LIST_STACKING, bottom first), as the manager keeps an overlay that
stays while ilo isn't active (#2790). An overlay is a dialog-type window of ilo's, transient for
another of its windows. Prints the order and exits 0 when each one shown is below $1; otherwise
says why and exits 1.
Usage: python3 -I check/x11-stacking.py <window> <pid>"""
import ctypes
import json
import sys

x11 = ctypes.CDLL("libX11.so.6")
ul, vp, ci = ctypes.c_ulong, ctypes.c_void_p, ctypes.c_int
x11.XOpenDisplay.restype = vp
x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
x11.XDefaultRootWindow.restype = ul
x11.XDefaultRootWindow.argtypes = [vp]
x11.XInternAtom.restype = ul
x11.XInternAtom.argtypes = [vp, ctypes.c_char_p, ci]
x11.XGetWindowProperty.argtypes = [vp, ul, ul, ctypes.c_long, ctypes.c_long, ci, ul,
                                   ctypes.POINTER(ul), ctypes.POINTER(ci), ctypes.POINTER(ul),
                                   ctypes.POINTER(ul), ctypes.POINTER(vp)]
x11.XFree.argtypes = [vp]
x11.XSetErrorHandler.argtypes = [vp]
# A window going while it's read raises BadWindow, which would end the process.
handler = ctypes.CFUNCTYPE(ci, vp, vp)(lambda d, e: 0)
x11.XSetErrorHandler(ctypes.cast(handler, vp))

display = x11.XOpenDisplay(None)
if not display:
    sys.exit("no X display")
root = x11.XDefaultRootWindow(display)


def atom(name):
    return x11.XInternAtom(display, name.encode(), 0)


def longs(window, name):
    kind, fmt, n, rest, data = ul(), ci(), ul(), ul(), vp()
    if x11.XGetWindowProperty(display, window, atom(name), 0, 1024, 0, 0, ctypes.byref(kind),
                              ctypes.byref(fmt), ctypes.byref(n), ctypes.byref(rest),
                              ctypes.byref(data)) != 0 or not data.value:
        return []
    values = list((ctypes.c_ulong * n.value).from_address(data.value)) if fmt.value == 32 else []
    x11.XFree(data)
    return values


other, pid = int(sys.argv[1], 0), int(sys.argv[2])
dialog = atom("_NET_WM_WINDOW_TYPE_DIALOG")
stacking = longs(root, "_NET_CLIENT_LIST_STACKING")
order = []
for window in stacking:
    ilo = longs(window, "_NET_WM_PID")[:1] == [pid]
    overlay = ilo and dialog in longs(window, "_NET_WM_WINDOW_TYPE") and \
        bool(longs(window, "WM_TRANSIENT_FOR"))
    order.append({"window": hex(window),
                  "is": "other" if window == other else "overlay" if overlay else
                  "ilo" if ilo else "else"})
said = json.dumps(order)
if other not in stacking:
    sys.exit(f"the other app's window {hex(other)} isn't in the manager's stacking: {said}")
overlays = [i for i, o in enumerate(order) if o["is"] == "overlay"]
if not overlays:
    sys.exit("no overlay of ilo's is in the manager's stacking, as an override-redirect window "
             f"isn't, bottom first: {said}")
above = [order[i]["window"] for i in overlays if i > stacking.index(other)]
if above:
    sys.exit(f"ilo's overlay {', '.join(above)} is over the other app's window {hex(other)} in the "
             f"manager's stacking, bottom first: {said}")
print(said)
