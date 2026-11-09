"""Spike (#359): whether the top-left WIDTH by HEIGHT points of two PNG captures differ, which is
where the first window's card is in a capture of every window. The standard library only, as the
check runners have no imaging module: 8-bit RGB or RGBA PNGs, not interlaced.

  card_changed.py A.png B.png WIDTH HEIGHT POINTS_WIDE
    POINTS_WIDE is the capture's width in points, from which its pixels a point follow.
    Prints "changed N" with the bytes that differ, or "same".
"""

import struct
import sys
import zlib


def rows(path):
    with open(path, "rb") as f:
        data = f.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        sys.exit(f"{path} isn't a PNG")
    at, idat, width = 8, b"", 0
    while at < len(data):
        length, kind = struct.unpack(">I4s", data[at:at + 8])
        body = data[at + 8:at + 8 + length]
        if kind == b"IHDR":
            width, height, depth, colour, _, _, interlace = struct.unpack(">IIBBBBB", body)
            if depth != 8 or colour not in (2, 6) or interlace:
                sys.exit(f"{path}: depth {depth}, colour {colour}, interlace {interlace}")
            channels = 4 if colour == 6 else 3
        elif kind == b"IDAT":
            idat += body
        at += 12 + length
    raw = zlib.decompress(idat)
    stride = width * channels
    out, previous, at = [], bytearray(stride), 0
    for _ in range(height):
        kind, line = raw[at], bytearray(raw[at + 1:at + 1 + stride])
        at += 1 + stride
        for i in range(stride):
            left = line[i - channels] if i >= channels else 0
            up = previous[i]
            corner = previous[i - channels] if i >= channels else 0
            if kind == 1:
                line[i] = (line[i] + left) & 255
            elif kind == 2:
                line[i] = (line[i] + up) & 255
            elif kind == 3:
                line[i] = (line[i] + (left + up) // 2) & 255
            elif kind == 4:
                p = left + up - corner
                pa, pb, pc = abs(p - left), abs(p - up), abs(p - corner)
                line[i] = (line[i] + (left if pa <= pb and pa <= pc else up if pb <= pc else corner)) & 255
        out.append(bytes(line))
        previous = line
    return width, channels, out


def main():
    a_path, b_path, points_w, points_h, wide = sys.argv[1:6]
    width, channels, a = rows(a_path)
    _, _, b = rows(b_path)
    scale = width / float(wide)
    w, h = int(int(points_w) * scale), int(int(points_h) * scale)
    differ = sum(
        1 for y in range(min(h, len(a), len(b)))
        for x, y2 in zip(a[y][:w * channels], b[y][:w * channels]) if x != y2)
    print(f"changed {differ}" if differ else "same")


main()
