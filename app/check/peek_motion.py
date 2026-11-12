"""Reads the frames check/peek-motion.sh filmed of Peek opening and closing (#2519).

  peek_motion.py <frames-dir> <content-width> <page x y w h> <from x y>

<frames-dir> holds open-1/, close-1/, open-2/ and so on, each a motion's pictures of the window's
content, named for the seconds since 1970 they were asked for (tools/window-frames.sh). The page
rests at <page>, in points from the content's top left, growing from <from>. Prints a line a frame
and a verdict for each motion; exits 1 if a frame shows Peek's page outside the card, 2 if the
frames can't tell yet, as when none was taken mid-motion or the dim isn't drawn blended.

Each frame says where the motion is itself. The dim over the window's page darkens it linearly as
the preview opens and closes, so a point of the window's page outside the card, D, gives the
motion's progress, and with it the card's scale on the preview's curves (link_preview.rs). A point
inside the page's resting frame, P, near the corner furthest from where it grows, is outside the
card until the card's scale reaches it: until then P must show the window's own page, as it does
on the Mac, and never Peek's. A point inside the card at every scale, Q, must never show the
window's page as it is, undimmed, while the dim is up: the card has its plate there, or Peek's page.
"""
import os
import struct
import sys
import zlib

# link_preview.rs: the dim's colour and opacity, the scale it grows from, and the curves.
DIM, DIM_OPACITY, FROM_SCALE = 0x15, 0.4, 0.5
OPEN_CURVE, CLOSE_CURVE = (0.0, 0.0, 0.58, 1.0), (0.42, 0.0, 1.0, 1.0)
RIM = 1.0
# Inside the resting frame by this share of its size from the far corner, and the scale's margin
# either side of where the card reaches it, against a frame read between two of the app's.
IN, MARGIN = 0.05, 0.04


def read_png(path, last=None):
    """The PNG's width, height, bytes a pixel and rows, decoded up to the pixel `last` (x, y): what
    the reads need, as a whole frame takes seconds here."""
    data = open(path, "rb").read()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", path
    at, idat = 8, b""
    while at < len(data):
        length, kind = struct.unpack_from(">I4s", data, at)
        body = data[at + 8:at + 8 + length]
        if kind == b"IHDR":
            width, height, depth, color, _, _, interlace = struct.unpack(">IIBBBBB", body)
            assert depth == 8 and interlace == 0 and color in (2, 6), (path, depth, color)
        elif kind == b"IDAT":
            idat += body
        at += 12 + length
    size = 3 if color == 2 else 4
    raw, line = zlib.decompress(idat), width * size
    lines, needed = height, line
    if last:
        lines, needed = min(height, last[1] + 1), min(line, (last[0] + 1) * size)
    rows, previous = [], bytearray(line)
    for y in range(lines):
        kind, row = raw[y * (line + 1)], bytearray(raw[y * (line + 1) + 1:(y + 1) * (line + 1)])
        for i in range(needed if kind else 0):
            left = row[i - size] if i >= size else 0
            up = previous[i]
            corner = previous[i - size] if i >= size else 0
            if kind == 1:
                row[i] = (row[i] + left) & 255
            elif kind == 2:
                row[i] = (row[i] + up) & 255
            elif kind == 3:
                row[i] = (row[i] + (left + up) // 2) & 255
            elif kind == 4:
                p = left + up - corner
                pa, pb, pc = abs(p - left), abs(p - up), abs(p - corner)
                row[i] = (row[i] + (left if pa <= pb and pa <= pc else up if pb <= pc else corner)) & 255
        rows.append(bytes(row))
        previous = row
    return width, height, size, rows


def bezier(x1, y1, x2, y2, x):
    """The cubic Bézier easing at time `x`, as shell::cubic_bezier."""
    if x <= 0 or x >= 1:
        return min(max(x, 0.0), 1.0)
    low, high = 0.0, 1.0
    for _ in range(40):
        t = (low + high) / 2
        at = 3 * (1 - t) ** 2 * t * x1 + 3 * (1 - t) * t * t * x2 + t ** 3
        low, high = (t, high) if at < x else (low, t)
    t = (low + high) / 2
    return 3 * (1 - t) ** 2 * t * y1 + 3 * (1 - t) * t * t * y2 + t ** 3


def main():
    folder, content_width = sys.argv[1], float(sys.argv[2])
    px, py, pw, ph = (float(each) for each in sys.argv[3:7])
    fx, fy = float(sys.argv[7]), float(sys.argv[8])
    # P, near the resting frame's corner furthest from where it grows, and the scale at which the
    # card's page reaches it: the larger of the two axes'.
    right = fx < px + pw / 2
    down = fy < py + ph / 2
    p = (px + pw * (1 - IN) if right else px + pw * IN, py + ph * (1 - IN) if down else py + ph * IN)
    edge = (px + pw if right else px, py + ph if down else py)
    reach = max((p[0] - fx) / (edge[0] - fx), (p[1] - fy) / (edge[1] - fy))
    # D, the window's page under the dim, below the card, or above it if the card is low.
    d = (px + pw / 2, py + ph + RIM + 12)
    # Q, the resting frame's centre drawn in toward where it grows to its smallest scale.
    q = (fx + (px + pw / 2 - fx) * FROM_SCALE, fy + (py + ph / 2 - fy) * FROM_SCALE)
    print("P %.0f,%.0f (inside the resting frame; the card reaches it at scale %.2f), D %.0f,%.0f "
          "(the window's page, below the card), Q %.0f,%.0f (inside the card at every scale)"
          % (p + (reach,) + d + q))

    def hue(rgb):
        r, g, b = rgb
        if g > r + 30 and g > b + 30:
            return "page"
        if r > g + 30 and b > g + 30:
            return "peek"
        return "other"

    failed = unreadable = blended = False
    base = None
    caught_in = {"open": 0, "close": 0}
    filmed = sorted(
        (name for name in os.listdir(folder)
         if name.split("-")[0] in caught_in and name.split("-")[-1].isdigit()),
        key=lambda name: (int(name.split("-")[-1]), name != "open-" + name.split("-")[-1]))
    for filming in filmed:
        motion = filming.split("-")[0]
        names = sorted(os.listdir(os.path.join(folder, filming)))
        if not names:
            print("%s: no frames" % filming)
            continue
        rows = []
        for name in names:
            path = os.path.join(folder, filming, name)
            width = struct.unpack_from(">I", open(path, "rb").read(24), 16)[0]
            scale = width / content_width
            last = (int(max(p[0], d[0], q[0]) * scale), int(max(p[1], d[1], q[1]) * scale))
            width, height, size, picture = read_png(path, last)

            def at(point):
                x, y = int(point[0] * scale), int(point[1] * scale)
                return tuple(picture[y][x * size:x * size + 3])

            rows.append((name[:-4], at(p), at(d), at(q)))
        # The window's page as it is, at D in the first frame, before the click.
        if base is None:
            base = rows[0][2]
            print("the window's page at D before the click: %s" % (base,))
        # The dim's progress at D, from its green: 0 is the page as it is, 1 the dim at rest.
        g0 = base[1]
        dims = [max(0.0, min(1.0, (g0 - dd[1]) / ((g0 - DIM) * DIM_OPACITY))) for _, _, dd, _ in rows]
        if max(dims) < 0.8:
            print("%s: the dim never reached its rest at D (%.2f at most), so this filming can't say "
                  "where the motion is" % (filming, max(dims)))
            unreadable = True
            continue
        blended = True
        caught = wrong = 0
        for (name, pp, dd, qq), dim in zip(rows, dims):
            if motion == "open":
                scale = FROM_SCALE + (1 - FROM_SCALE) * bezier(*OPEN_CURVE, dim)
            else:
                scale = 1 + (FROM_SCALE - 1) * bezier(*CLOSE_CURVE, 1 - dim)
            # Outside the card: before the overlay's first frame, as it moves short of P, and once
            # the overlay has gone after the close.
            outside = dim <= 0.05 or scale < reach - MARGIN
            inside = 0.05 < dim and scale > reach + MARGIN
            mid = 0.05 < dim < 0.95
            verdict = ""
            bare = dim > 0.2 and all(abs(a - b) <= 6 for a, b in zip(qq, base))
            if outside and hue(pp) != "page":
                verdict = "WRONG: Peek's frame shows outside the card"
                wrong += 1
            elif bare:
                verdict = "WRONG: the window's page shows undimmed inside the card"
                wrong += 1
            elif outside and mid:
                verdict = "ok: the window's page outside the card"
                caught += 1
            elif inside:
                verdict = "card"
            print("%s %s dim %.2f scale %.2f P %s %s D %s Q %s %s" % (
                filming, name, dim, scale, hue(pp), pp, dd, qq, verdict))
        caught_in[motion] += caught
        if wrong:
            print("%s: FAIL: %d frames show Peek's resting frame outside the card or the window's page "
                  "undimmed inside it" % (filming, wrong))
            failed = True
        else:
            print("%s: %d frames mid-motion show the window's page at P, outside the card"
                  % (filming, caught))
    if not blended:
        print("the dim isn't drawn blended here: no filming saw it reach its rest at D")
    for motion, caught in caught_in.items():
        if not caught:
            print("%s: no frame caught the card moving short of P, so none says what's outside it"
                  % motion)
            unreadable = True
    sys.exit(1 if failed else 2 if unreadable else 0)


main()
