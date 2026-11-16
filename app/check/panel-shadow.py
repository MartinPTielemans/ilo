"""Reads a panel's shadow in a capture of the browser window, against one taken without the panel,
for check/panel-shadows.sh (#2988). The panel draws its shadow in a margin round it in its own
window, and the window's rectangle and the panel's come from the host's line "browser: <name> panel
X Y W H in its window X Y W H", in pixels from the capture's top left.

`shown BEFORE AFTER PANEL WINDOW DARKEN` passes when the margin never shows: every pixel on the
window's outer edge that is the page's colour in BEFORE, where nothing else of the app is, is still
that colour in AFTER, so no rectangle of the window's own is drawn behind the shadow. And the page
just under the panel's bottom edge is DARKEN levels darker at least: the shadow is there.
`gone BEFORE AFTER PANEL WINDOW` passes when the page under where the panel and its shadow were, the
panel's width from its top to the window's bottom, is as it was in BEFORE: no shadow is left.
PANEL and WINDOW are "X Y W H". Prints what it read, and exits 1 when it doesn't pass.
Usage: panel-shadow.py shown BEFORE.png AFTER.png "X Y W H" "X Y W H" DARKEN
       panel-shadow.py gone BEFORE.png AFTER.png "X Y W H" "X Y W H\""""

import struct
import sys
import zlib

# Two captures can differ by a level where Windows dithers or GPUI's blur rounds, never by more.
SAME = 2


def read_png(path, limit):
    """The PNG at path as (width, height, channels, rows), its first `limit` rows at most, each a
    bytes of RGB or RGBA, 8 bits a channel. Pure Python, as the runner's has no imaging library, so
    it decodes no row past those the check reads."""
    data = open(path, "rb").read()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", f"{path} isn't a PNG"
    at, idat = 8, b""
    while at < len(data):
        length, kind = struct.unpack_from(">I4s", data, at)
        body = data[at + 8:at + 8 + length]
        if kind == b"IHDR":
            width, height, depth, colour, _, _, interlace = struct.unpack(">IIBBBBB", body)
            assert depth == 8 and colour in (2, 6) and not interlace, \
                f"{path} is a PNG of depth {depth}, colour type {colour}, interlace {interlace}"
        elif kind == b"IDAT":
            idat += body
        at += 12 + length
    channels = 4 if colour == 6 else 3
    stride = width * channels
    raw = zlib.decompress(idat)
    rows, above = [], bytearray(stride)
    for y in range(min(height, limit)):
        start = y * (stride + 1)
        kind, line = raw[start], bytearray(raw[start + 1:start + 1 + stride])
        for i in range(stride):
            a = line[i - channels] if i >= channels else 0
            b = above[i]
            c = above[i - channels] if i >= channels else 0
            if kind == 1:
                line[i] = (line[i] + a) & 255
            elif kind == 2:
                line[i] = (line[i] + b) & 255
            elif kind == 3:
                line[i] = (line[i] + (a + b) // 2) & 255
            elif kind == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(bytes(line))
        above = line
    return width, len(rows), channels, rows


def at(image, x, y):
    _, _, channels, rows = image
    return tuple(rows[y][x * channels:x * channels + 3])


mode, before_path, after_path = sys.argv[1:4]
px, py, pw, ph = map(int, sys.argv[4].split())
wx, wy, ww, wh = map(int, sys.argv[5].split())
before, after = read_png(before_path, wy + wh), read_png(after_path, wy + wh)
width, height = before[0], before[1]
if (after[0], after[1]) != (width, height):
    print(f"the captures differ in size: {width}x{height} before, {after[0]}x{after[1]} after")
    sys.exit(1)
# The page's colour: the most common in BEFORE under the panel, which only the page drew there.
counts = {}
for y in range(max(py, 0), min(py + ph, height), 4):
    for x in range(max(px, 0), min(px + pw, width), 4):
        counts[at(before, x, y)] = counts.get(at(before, x, y), 0) + 1
if not counts:
    print(f"the panel at {px},{py} {pw}x{ph} is outside the {width}x{height} capture")
    sys.exit(1)
page = max(counts, key=counts.get)
# The page itself, in BEFORE: from the panel's middle out, as far as the page's colour goes. Only
# there is a pixel of the page's colour the page's, as the sidebar's selected row, which can be as
# white, changes as the panel opens: ⌘T selects a new tab's row. Its corners are rounded, so a few
# pixels in from its edges.
middle_x, middle_y = px + pw // 2, py + ph // 2
def reach(step_x, step_y):
    x, y = middle_x, middle_y
    while (0 <= x + step_x < width and 0 <= y + step_y < height
           and at(before, x + step_x, y + step_y) == page):
        x, y = x + step_x, y + step_y
    return x, y
CORNER = 8
page_left, page_right = reach(-1, 0)[0] + CORNER, reach(1, 0)[0] - CORNER
page_top, page_bottom = reach(0, -1)[1] + CORNER, reach(0, 1)[1] - CORNER
inside = lambda x, y: page_left <= x <= page_right and page_top <= y <= page_bottom
differ = lambda x, y: max(abs(a - b) for a, b in zip(at(before, x, y), at(after, x, y)))

if mode == "shown":
    darken_wanted = int(sys.argv[6])
    edge = [(x, y) for x in range(wx, wx + ww) for y in (wy, wy + wh - 1)]
    edge += [(x, y) for y in range(wy + 1, wy + wh - 1) for x in (wx, wx + ww - 1)]
    edge = [(x, y) for x, y in edge if inside(x, y) and at(before, x, y) == page]
    worst = max(edge, key=lambda p: differ(*p), default=None)
    # Two to eight pixels under the panel's bottom edge, across its middle third: under the outline,
    # where the shadow, which falls, is darkest.
    band = [(x, y) for y in range(py + ph + 2, py + ph + 9)
            for x in range(px + pw // 3, px + 2 * pw // 3)]
    band = [(x, y) for x, y in band if inside(x, y) and at(before, x, y) == page]
    luma = lambda c: 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
    darken = (sum(luma(at(before, x, y)) - luma(at(after, x, y)) for x, y in band) / len(band)
              if band else 0)
    said = (f"page {page} at {page_left},{page_top} to {page_right},{page_bottom}; "
            f"window's edge: {len(edge)} pixels over the page, "
            + (f"most changed {differ(*worst)} at {worst[0]},{worst[1]}, {at(before, *worst)} to "
               f"{at(after, *worst)}" if worst else "none over the page")
            + f"; under the panel: {len(band)} pixels darker by {darken:.1f}")
    print(said)
    ok = edge and differ(*worst) <= SAME and band and darken >= darken_wanted
    sys.exit(0 if ok else 1)

if mode == "gone":
    area = [(x, y) for y in range(py, wy + wh) for x in range(px, px + pw)]
    area = [(x, y) for x, y in area if inside(x, y) and at(before, x, y) == page]
    changed = [(x, y) for x, y in area if differ(x, y) > SAME]
    print(f"page {page} at {page_left},{page_top} to {page_right},{page_bottom}; under where the panel and its shadow were: {len(area)} pixels, "
          f"{len(changed)} changed" + (f", the first at {changed[0][0]},{changed[0][1]}, "
                                         f"{at(before, *changed[0])} to {at(after, *changed[0])}"
                                         if changed else ""))
    sys.exit(0 if area and not changed else 1)

sys.exit(f"unknown mode {mode}")
