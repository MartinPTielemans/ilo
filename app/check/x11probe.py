"""Probe (#2790): what the X server and the window manager say of each window, and the requests a
user's window manager gets, through Xlib by ctypes, as other-window.py does.
Usage: python3 -I check/x11probe.py <command> [args]
  snap                          the windows, one JSON line each, then the root's, bottom to top
  state <win> add|remove <A,B>  asks the manager for _NET_WM_STATE_<A> and _<B> on <win>
  iconify <win> | activate <win> | desktops <n> | todesktop <win> <n> | current <n>
  moveresize <win> <x> <y> <w> <h>   _NET_MOVERESIZE_WINDOW, -1 keeps one
  follow <browser> <overlay> <steps> <dx> <dy> <dw> <dh>  moves and sizes <browser> a step at a
                                time, timing each step's overlay from its browser window's
  watch <seconds>               the root's children's configures, maps and unmaps, timed
  firstframe <x> <y> <seconds>  the time the root's pixel at <x>,<y> first changes, and each map
"""
import ctypes
import json
import sys
import time

x = ctypes.CDLL("libX11.so.6")
ul, vp, ci = ctypes.c_ulong, ctypes.c_void_p, ctypes.c_int


class Attributes(ctypes.Structure):
    _fields_ = [("x", ci), ("y", ci), ("width", ci), ("height", ci), ("border_width", ci),
                ("depth", ci), ("visual", vp), ("root", ul), ("c_class", ci), ("bit_gravity", ci),
                ("win_gravity", ci), ("backing_store", ci), ("backing_planes", ul),
                ("backing_pixel", ul), ("save_under", ci), ("colormap", ul), ("map_installed", ci),
                ("map_state", ci), ("all_event_masks", ctypes.c_long),
                ("your_event_mask", ctypes.c_long), ("do_not_propagate_mask", ctypes.c_long),
                ("override_redirect", ci), ("screen", vp)]


class ClientMessage(ctypes.Structure):
    _fields_ = [("type", ci), ("serial", ul), ("send_event", ci), ("display", vp),
                ("window", ul), ("message_type", ul), ("format", ci), ("data", ctypes.c_long * 5)]


class Event(ctypes.Union):
    _fields_ = [("type", ci), ("client", ClientMessage), ("pad", ctypes.c_long * 24)]


x.XOpenDisplay.restype = vp
x.XOpenDisplay.argtypes = [ctypes.c_char_p]
x.XDefaultRootWindow.restype = ul
x.XDefaultRootWindow.argtypes = [vp]
x.XInternAtom.restype = ul
x.XInternAtom.argtypes = [vp, ctypes.c_char_p, ci]
x.XGetAtomName.restype = ctypes.c_char_p
x.XGetAtomName.argtypes = [vp, ul]
x.XQueryTree.argtypes = [vp, ul, ctypes.POINTER(ul), ctypes.POINTER(ul),
                         ctypes.POINTER(ctypes.POINTER(ul)), ctypes.POINTER(ctypes.c_uint)]
x.XGetWindowAttributes.argtypes = [vp, ul, ctypes.POINTER(Attributes)]
x.XTranslateCoordinates.argtypes = [vp, ul, ul, ci, ci, ctypes.POINTER(ci), ctypes.POINTER(ci),
                                    ctypes.POINTER(ul)]
x.XGetWindowProperty.argtypes = [vp, ul, ul, ctypes.c_long, ctypes.c_long, ci, ul,
                                 ctypes.POINTER(ul), ctypes.POINTER(ci), ctypes.POINTER(ul),
                                 ctypes.POINTER(ul), ctypes.POINTER(ctypes.c_void_p)]
x.XSendEvent.argtypes = [vp, ul, ci, ctypes.c_long, ctypes.POINTER(Event)]
x.XFlush.argtypes = [vp]
x.XSync.argtypes = [vp, ci]
x.XSelectInput.argtypes = [vp, ul, ctypes.c_long]
x.XPending.argtypes = [vp]
x.XNextEvent.argtypes = [vp, ctypes.POINTER(Event)]
x.XGetInputFocus.argtypes = [vp, ctypes.POINTER(ul), ctypes.POINTER(ci)]
x.XGetImage.restype = ctypes.POINTER(ctypes.c_char_p)
x.XGetImage.argtypes = [vp, ul, ci, ci, ctypes.c_uint, ctypes.c_uint, ul, ci]
x.XSetErrorHandler.argtypes = [vp]

# Windows going while they're read raise BadWindow, which would end the process.
handler = ctypes.CFUNCTYPE(ci, vp, vp)(lambda d, e: 0)
x.XSetErrorHandler(ctypes.cast(handler, vp))

d = x.XOpenDisplay(None)
if not d:
    sys.exit("no X display")
root = x.XDefaultRootWindow(d)


def atom(name):
    return x.XInternAtom(d, name.encode(), 0)


def name_of(a):
    n = x.XGetAtomName(d, a) if a else None
    return n.decode() if n else str(a)


def prop(w, name, fmt="atoms"):
    t, f, n, after, data = ul(), ci(), ul(), ul(), ctypes.c_void_p()
    if x.XGetWindowProperty(d, w, atom(name), 0, 4096, 0, 0, ctypes.byref(t), ctypes.byref(f),
                            ctypes.byref(n), ctypes.byref(after), ctypes.byref(data)) != 0:
        return None
    if not t.value or not data.value:
        return None
    if f.value == 8:
        return ctypes.string_at(data.value, n.value).decode(errors="replace")
    if f.value == 32:
        vals = list(ctypes.cast(data.value, ctypes.POINTER(ctypes.c_long))[:n.value])
        vals = [v & 0xFFFFFFFF for v in vals]
        return [name_of(v) for v in vals] if fmt == "atoms" else vals
    return None


def children(w):
    r, p, kids, n = ul(), ul(), ctypes.POINTER(ul)(), ctypes.c_uint()
    if not x.XQueryTree(d, w, ctypes.byref(r), ctypes.byref(p), ctypes.byref(kids), ctypes.byref(n)):
        return []
    return [kids[i] for i in range(n.value)]


def attrs(w):
    a = Attributes()
    return a if x.XGetWindowAttributes(d, w, ctypes.byref(a)) else None


def at_root(w):
    rx, ry, child = ci(), ci(), ul()
    x.XTranslateCoordinates(d, w, root, 0, 0, ctypes.byref(rx), ctypes.byref(ry), ctypes.byref(child))
    return rx.value, ry.value


def client_in(w, depth=0):
    if prop(w, "WM_STATE", "raw") is not None:
        return w
    if depth > 3:
        return None
    for c in children(w):
        found = client_in(c, depth + 1)
        if found:
            return found
    return None


def describe(w, frame, order):
    a = attrs(w)
    fa = attrs(frame)
    if not a or not fa:
        return None
    rx, ry = at_root(w)
    hints = prop(w, "WM_HINTS", "raw")
    return {
        "id": hex(w), "frame": hex(frame), "order": order,
        "override": bool(fa.override_redirect), "viewable": fa.map_state == 2 and a.map_state == 2,
        "rect": [rx, ry, a.width, a.height],
        "name": prop(w, "_NET_WM_NAME") or prop(w, "WM_NAME"),
        "pid": (prop(w, "_NET_WM_PID", "raw") or [None])[0],
        "transient_for": hex((prop(w, "WM_TRANSIENT_FOR", "raw") or [0])[0]),
        "type": prop(w, "_NET_WM_WINDOW_TYPE"), "state": prop(w, "_NET_WM_STATE"),
        "input": (bool(hints[1]) if hints and hints[0] & 1 else None),
        "protocols": prop(w, "WM_PROTOCOLS"),
        "desktop": (prop(w, "_NET_WM_DESKTOP", "raw") or [None])[0],
        "wm_state": (prop(w, "WM_STATE", "raw") or [None])[0],
    }


def snap():
    for order, frame in enumerate(children(root)):
        fa = attrs(frame)
        if not fa:
            continue
        if fa.override_redirect:
            if fa.map_state != 2:
                continue
            client = frame
        else:
            client = client_in(frame)
            if not client:
                continue
        info = describe(client, frame, order)
        if info:
            print(json.dumps(info))
    focus, revert = ul(), ci()
    x.XGetInputFocus(d, ctypes.byref(focus), ctypes.byref(revert))
    print(json.dumps({
        "root": True, "focus": hex(focus.value),
        "active": hex((prop(root, "_NET_ACTIVE_WINDOW", "raw") or [0])[0]),
        "clients": [hex(v) for v in prop(root, "_NET_CLIENT_LIST", "raw") or []],
        "stacking": [hex(v) for v in prop(root, "_NET_CLIENT_LIST_STACKING", "raw") or []],
        "desktops": (prop(root, "_NET_NUMBER_OF_DESKTOPS", "raw") or [None])[0],
        "current": (prop(root, "_NET_CURRENT_DESKTOP", "raw") or [None])[0],
    }))


def message(w, kind, *data):
    e = Event()
    e.client.type = 33  # ClientMessage
    e.client.send_event = 1
    e.client.window = w
    e.client.message_type = atom(kind)
    e.client.format = 32
    for i, v in enumerate(data):
        e.client.data[i] = v
    x.XSendEvent(d, root, 0, (1 << 19) | (1 << 20), ctypes.byref(e))
    x.XFlush(d)


def moveresize(w, mx, my, mw, mh):
    flags = 1 | (2 << 12)  # NorthWest gravity, a pager's request
    for bit, v in zip((8, 9, 10, 11), (mx, my, mw, mh)):
        if v >= 0:
            flags |= 1 << bit
    message(w, "_NET_MOVERESIZE_WINDOW", flags, max(mx, 0), max(my, 0), max(mw, 0), max(mh, 0))


def frame_of(w):
    each = w
    while True:
        r, p, kids, n = ul(), ul(), ctypes.POINTER(ul)(), ctypes.c_uint()
        x.XQueryTree(d, each, ctypes.byref(r), ctypes.byref(p), ctypes.byref(kids), ctypes.byref(n))
        if p.value in (root, 0):
            return each
        each = p.value


def events(until):
    """Each root child's configure, map and unmap until `until`, as (time, kind, window, x, y, w, h)."""
    e = Event()
    while time.monotonic() < until:
        if not x.XPending(d):
            time.sleep(0.001)
            continue
        x.XNextEvent(d, ctypes.byref(e))
        raw = (ctypes.c_long * 24).from_buffer_copy(bytes(e))
        ints = (ctypes.c_int * 48).from_buffer_copy(bytes(e))
        now = time.monotonic()
        if e.type == 22:  # ConfigureNotify: window at long 5, x y w h at int 12..15
            yield now, "configure", raw[5], ints[12], ints[13], ints[14], ints[15]
        elif e.type == 19:
            yield now, "map", raw[5], 0, 0, 0, 0
        elif e.type == 18:
            yield now, "unmap", raw[5], 0, 0, 0, 0


def follow(browser, overlay, steps, dx, dy, dw, dh):
    bframe, oframe = frame_of(browser), frame_of(overlay)
    x.XSelectInput(d, root, 1 << 19)  # SubstructureNotify
    x.XSync(d, 0)
    bx, by = at_root(browser)
    ox, oy = at_root(overlay)
    ba, oa = attrs(browser), attrs(overlay)
    start = [bx, by, ba.width, ba.height]
    offset = (ox - bx, oy - by)
    print(json.dumps({"start_browser": start, "start_overlay": [ox, oy, oa.width, oa.height]}))
    lags = []
    for step in range(1, steps + 1):
        want = [start[0] + dx * step, start[1] + dy * step, start[2] + dw * step, start[3] + dh * step]
        sent = time.monotonic()
        moveresize(browser, *want)
        moved = overlay_at = None
        for now, kind, w, ex, ey, ew, eh in events(sent + 1.0):
            if kind != "configure":
                continue
            if w == bframe and moved is None:
                moved = now
            if w == oframe and moved is not None:
                overlay_at = now
            if moved and overlay_at:
                # Settled once nothing more comes for 100 ms.
                for later in events(time.monotonic() + 0.1):
                    if later[2] == oframe:
                        overlay_at = later[0]
                break
        lag = None if moved is None or overlay_at is None else round((overlay_at - moved) * 1000, 1)
        lags.append(lag)
        print(json.dumps({"step": step, "browser_after_ms": None if moved is None else
                          round((moved - sent) * 1000, 1), "overlay_lag_ms": lag}))
    x.XSync(d, 0)
    bx, by = at_root(browser)
    ox, oy = at_root(overlay)
    ba, oa = attrs(browser), attrs(overlay)
    print(json.dumps({"end_browser": [bx, by, ba.width, ba.height],
                      "end_overlay": [ox, oy, oa.width, oa.height],
                      "offset_start": offset, "offset_end": (ox - bx, oy - by),
                      "lags_ms": lags}))


def watch(seconds):
    x.XSelectInput(d, root, 1 << 19)
    x.XSync(d, 0)
    print(json.dumps({"watching": time.time()}), flush=True)
    for now, kind, w, ex, ey, ew, eh in events(time.monotonic() + seconds):
        print(json.dumps({"t": round(time.time(), 4), "kind": kind, "window": hex(w),
                          "frame_rect": [ex, ey, ew, eh]}), flush=True)


def pixel(px, py):
    img = x.XGetImage(d, root, px, py, 1, 1, 0xFFFFFF, 2)  # ZPixmap
    if not img:
        return None
    data = ctypes.cast(img, ctypes.POINTER(ctypes.c_void_p))[2]  # width, height, xoffset, format ints
    return ctypes.string_at(data, 4) if data else None


def firstframe(px, py, seconds):
    x.XSelectInput(d, root, 1 << 19)
    x.XSync(d, 0)
    before = pixel(px, py)
    print(json.dumps({"ready": time.time()}), flush=True)
    until = time.monotonic() + seconds
    mapped = None
    while time.monotonic() < until:
        while x.XPending(d):
            e = Event()
            x.XNextEvent(d, ctypes.byref(e))
            if e.type == 19 and mapped is None:
                mapped = time.time()
        now = pixel(px, py)
        if now != before:
            print(json.dumps({"changed": time.time(), "mapped": mapped}), flush=True)
            return
        time.sleep(0.002)
    print(json.dumps({"changed": None, "mapped": mapped}), flush=True)


cmd, args = sys.argv[1], sys.argv[2:]
if cmd == "snap":
    snap()
elif cmd == "state":
    names = args[2].split(",")
    message(int(args[0], 0), "_NET_WM_STATE", {"remove": 0, "add": 1}[args[1]],
            atom("_NET_WM_STATE_" + names[0]), atom("_NET_WM_STATE_" + names[1]) if len(names) > 1 else 0, 2)
elif cmd == "iconify":
    message(int(args[0], 0), "WM_CHANGE_STATE", 3)
elif cmd == "activate":
    message(int(args[0], 0), "_NET_ACTIVE_WINDOW", 2, 0, 0)
elif cmd == "desktops":
    message(root, "_NET_NUMBER_OF_DESKTOPS", int(args[0]))
elif cmd == "todesktop":
    message(int(args[0], 0), "_NET_WM_DESKTOP", int(args[1]), 2)
elif cmd == "current":
    message(root, "_NET_CURRENT_DESKTOP", int(args[0]), 0)
elif cmd == "moveresize":
    moveresize(int(args[0], 0), *[int(a) for a in args[1:5]])
elif cmd == "follow":
    follow(int(args[0], 0), int(args[1], 0), *[int(a) for a in args[2:7]])
elif cmd == "watch":
    watch(float(args[0]))
elif cmd == "firstframe":
    firstframe(int(args[0]), int(args[1]), float(args[2]))
else:
    sys.exit(f"unknown command {cmd}")
