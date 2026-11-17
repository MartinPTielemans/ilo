"""Whether the overlays ilo's process $2 shows are under another app's window $1 in the X window
manager's stacking (_NET_CLIENT_LIST_STACKING, bottom first), as the manager keeps an overlay that
stays while ilo isn't active (#2790). An overlay is a dialog-type window of ilo's, transient for
another of its windows. Where $1 overlaps none of ilo's other windows, the manager put it beside
them, as i3 tiles it beside ilo's window and floats the overlay over ilo's tile, so the overlays are
beside $1, not over it, whatever the stacking. Each one in the stacking must also be viewable with
a shape that isn't empty, as one the manager mapped with no place yet, its shape kept back, would be
under $1 and never seen.
With a point $3 $4 in the content of ilo's largest window, one of the overlays must cover it, as
the one the run means is there, where its UI asked, and not only another that stays (the foot bar).
Prints the order and exits 0 when each one shown is below $1; otherwise says why and exits 1.
Usage: python3 -I check/x11-stacking.py <window> <pid> [<x> <y>]"""
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


class XWindowAttributes(ctypes.Structure):
    _fields_ = [("x", ci), ("y", ci), ("width", ci), ("height", ci), ("border_width", ci),
                ("depth", ci), ("visual", vp), ("root", ul), ("c_class", ci), ("bit_gravity", ci),
                ("win_gravity", ci), ("backing_store", ci), ("backing_planes", ul),
                ("backing_pixel", ul), ("save_under", ci), ("colormap", ul),
                ("map_installed", ci), ("map_state", ci), ("all_event_masks", ctypes.c_long),
                ("your_event_mask", ctypes.c_long), ("do_not_propagate_mask", ctypes.c_long),
                ("override_redirect", ci), ("screen", vp)]


x11.XGetWindowAttributes.argtypes = [vp, ul, ctypes.POINTER(XWindowAttributes)]
x11.XTranslateCoordinates.argtypes = [vp, ul, ul, ci, ci, ctypes.POINTER(ci), ctypes.POINTER(ci),
                                      ctypes.POINTER(ul)]
xext = ctypes.CDLL("libXext.so.6")
# XRectangle: x, y as shorts, width, height as unsigned shorts.
xext.XShapeGetRectangles.restype = vp
xext.XShapeGetRectangles.argtypes = [vp, ul, ci, ctypes.POINTER(ci), ctypes.POINTER(ci)]
x11.XSetErrorHandler.argtypes = [vp]
# A window going while it's read raises BadWindow, which would end the process.
handler = ctypes.CFUNCTYPE(ci, vp, vp)(lambda d, e: 0)
x11.XSetErrorHandler(ctypes.cast(handler, vp))

display = x11.XOpenDisplay(None)
if not display:
    sys.exit("no X display")
root = x11.XDefaultRootWindow(display)


def seen(window):
    """Whether `window` is viewable with a bounding shape that isn't empty, and what it is."""
    attributes = XWindowAttributes()
    if not x11.XGetWindowAttributes(display, window, ctypes.byref(attributes)):
        return False, "gone"
    if attributes.map_state != 2:
        return False, ["unmapped", "unviewable"][attributes.map_state]
    count, ordering = ci(), ci()
    rectangles = xext.XShapeGetRectangles(display, window, 0, ctypes.byref(count),
                                          ctypes.byref(ordering))
    if rectangles:
        x11.XFree(rectangles)
    if count.value == 0:
        return False, "viewable with an empty shape"
    return True, f"viewable, {count.value} rectangles"


def box(window):
    """`window`'s place and size across the root, or None once it's gone."""
    attributes = XWindowAttributes()
    if not x11.XGetWindowAttributes(display, window, ctypes.byref(attributes)):
        return None
    x, y, child = ci(), ci(), ul()
    if not x11.XTranslateCoordinates(display, window, root, 0, 0, ctypes.byref(x), ctypes.byref(y),
                                     ctypes.byref(child)):
        return None
    return x.value, y.value, attributes.width, attributes.height


def overlap(a, b):
    return a and b and a[0] < b[0] + b[2] and b[0] < a[0] + a[2] and a[1] < b[1] + b[3] and \
        b[1] < a[1] + a[3]


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
for i in overlays:
    ok, what = seen(stacking[i])
    order[i]["seen"] = what
    if not ok:
        sys.exit(f"ilo's overlay {order[i]['window']} is in the manager's stacking but not seen: it's "
                 f"{what}, bottom first: {json.dumps(order)}")
if len(sys.argv) > 4:
    point = int(sys.argv[3]), int(sys.argv[4])
    windows = [box(window) for window, o in zip(stacking, order) if o["is"] == "ilo"]
    windows = [each for each in windows if each]
    if not windows:
        sys.exit(f"no window of ilo's is in the manager's stacking, bottom first: {said}")
    window = max(windows, key=lambda each: each[2] * each[3])
    at = window[0] + point[0], window[1] + point[1], 1, 1
    for i in overlays:
        order[i]["box"] = box(stacking[i])
    if not any(overlap(order[i]["box"], at) for i in overlays):
        sys.exit(f"no overlay of ilo's in the manager's stacking covers {point[0]},{point[1]} in "
                 f"ilo's window at {window}, {at[0]},{at[1]} on the screen: "
                 f"{json.dumps(order)}")
cover = box(other)
beside = cover is not None and not any(
    overlap(box(window), cover) for window, o in zip(stacking, order) if o["is"] == "ilo")
for i in overlays:
    if i > stacking.index(other) and beside:
        order[i]["beside"] = True
said = json.dumps(order)
above = [order[i]["window"] for i in overlays
         if i > stacking.index(other) and not order[i].get("beside")]
if above:
    sys.exit(f"ilo's overlay {', '.join(above)} is over the other app's window {hex(other)} in the "
             f"manager's stacking, bottom first: {said}")
print(said)
