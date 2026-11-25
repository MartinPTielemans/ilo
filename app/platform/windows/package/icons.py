"""Draws the Windows icons in Assets/ from the icon's outlines (branding/outline, which app/icon.sh
writes from the knot's drawings): the knot in ink on its light plate, rounded as Windows 11's app
icons are. The mark has a drawing for each range of sizes, so each icon is drawn from its own and
none is another scaled down. Run it again after the icon changes; the PNGs and ilo.ico it writes
are committed, as ilo.icns is.

    python3 -I icons.py
"""

import re
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw

# branding/ilo.icon/icon.json's two colours, on the outlines' 1024 canvas.
PLATE = (243, 240, 232, 255)
INK = (22, 22, 24, 255)
SIZE = 1024
RADIUS = 0.2 * SIZE
OUTLINES = Path(__file__).resolve().parents[3] / "branding" / "outline"


def drawing(size: int) -> str:
    """The outline an icon `size` across is drawn from: the small ones' cord is heavier."""
    return "knot-16" if size < 24 else "knot-32" if size < 40 else "knot"


def icon(size: int) -> Image.Image:
    image = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    ImageDraw.Draw(image).rounded_rectangle((0, 0, SIZE - 1, SIZE - 1), RADIUS, fill=PLATE)
    # An outline is closed pieces of lines, filled even-odd: a piece inside another is a hole in
    # it. A polygon here can't have a hole, so each piece is filled alone and they are combined by
    # exclusive or. A curve in the path stops this at float().
    d = re.search(r' d="([^"]+)"', (OUTLINES / f"{drawing(size)}.svg").read_text()).group(1)
    knot = Image.new("1", (SIZE, SIZE))
    for piece in d.split("M")[1:]:
        points = [tuple(map(float, point.split())) for point in piece.rstrip("Z").split("L")]
        one = Image.new("1", (SIZE, SIZE))
        ImageDraw.Draw(one).polygon(points, fill=1)
        knot = ImageChops.logical_xor(knot, one)
    image.paste(INK, mask=knot)
    return image.resize((size, size), Image.LANCZOS)


def main() -> None:
    assets = Path(__file__).resolve().parent / "Assets"
    assets.mkdir(exist_ok=True)
    # The names AppxManifest.xml gives them, at the scale-100 sizes, which Windows scales from.
    for name, size in [("Square44x44Logo", 44), ("Square150x150Logo", 150), ("StoreLogo", 50)]:
        icon(size).save(assets / f"{name}.png", optimize=True)
    sizes = [16, 20, 24, 32, 40, 48, 64, 256]
    icon(256).save(
        assets / "ilo.ico",
        sizes=[(s, s) for s in sizes],
        append_images=[icon(s) for s in sizes[:-1]],
    )


main()
