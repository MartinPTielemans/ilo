"""Never to land (#4078): compares render.sh's PNGs with the owner's reference PNGs, rendered by
headless Chromium. For each size: the largest difference in any channel (colour premultiplied by
alpha, and alpha), over all pixels, over edge pixels (the reference's 3x3 neighbourhood isn't one
colour) and over the rest, and how many pixels differ by more than 4. Sizes of 64 and under also
get a side-by-side at 8x, nearest neighbour: reference, render, and the difference times 8.

    python3 -I diff.py <render.sh's PNGs> <reference dir> <out dir>
"""

import sys
from pathlib import Path

from PIL import Image

pngs, ref, out = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
out.mkdir(parents=True, exist_ok=True)
PAIRS = [("mac-16-16", "R3-mac-16"), ("mac-32-32", "R3-mac-32"), ("mac-64-64", "R3-mac-64"),
         ("mac-128", "R3-mac-128"), ("mac-1024", "R3-mac-1024"), ("win-16-16", "R3-win-16"),
         ("win-48-48", "R3-win-48"), ("win-256", "R3-win-256")]


def premul(p):
    r, g, b, a = p
    return (r * a // 255, g * a // 255, b * a // 255, a)


print("| size | max | max on edges | max inside | pixels > 4 |")
print("|---|---|---|---|---|")
for mine, theirs in PAIRS:
    a = Image.open(ref / f"{theirs}.png").convert("RGBA")
    b = Image.open(pngs / f"{mine}.png").convert("RGBA")
    if a.size != b.size:
        print(f"| {theirs} | size {b.size} against {a.size} | | | |")
        continue
    w, h = a.size
    pa, pb = a.load(), b.load()
    worst = edge = inside = over = 0
    diff = Image.new("RGBA", a.size, (0, 0, 0, 255))
    pd = diff.load()
    for y in range(h):
        for x in range(w):
            ca, cb = premul(pa[x, y]), premul(pb[x, y])
            d = max(abs(i - j) for i, j in zip(ca, cb))
            hood = {pa[i, j] for i in range(max(0, x - 1), min(w, x + 2))
                    for j in range(max(0, y - 1), min(h, y + 2))}
            if len(hood) > 1:
                edge = max(edge, d)
            else:
                inside = max(inside, d)
            worst = max(worst, d)
            over += d > 4
            v = min(255, d * 8)
            pd[x, y] = (v, v, v, 255)
    print(f"| {theirs[3:]} | {worst} | {edge} | {inside} | {over} of {w * h} |")
    if w <= 64:
        k = 8
        sheet = Image.new("RGBA", (3 * w * k + 2 * 8, h * k), (128, 128, 128, 255))
        for i, im in enumerate((a, b, diff)):
            sheet.paste(im.resize((w * k, h * k), Image.NEAREST), (i * (w * k + 8), 0))
        sheet.save(out / f"side-{theirs[3:]}.png")
