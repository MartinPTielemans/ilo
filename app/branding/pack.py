"""Puts render.sh's PNGs where the app, its packages and the site read them, each size its own
image: ilo.icns, Windows' ilo.ico and MSIX logos, the Flatpak's hicolor icons, and the favicons.

    python3 -I pack.py <render.sh's PNGs> <repo>
"""

import shutil
import struct
import sys
from pathlib import Path

from PIL import Image

pngs, root = Path(sys.argv[1]), Path(sys.argv[2])


def mac(size):
    return pngs / (f"mac-{size}-{size}.png" if size <= 64 else f"mac-{size}.png")


def win(size):
    return pngs / (f"win-{size}-{size}.png" if size <= 64 else f"win-{size}.png")


# The .icns by hand: Pillow's writer has no 16 px entry and no 1x 32 and 64. Each entry is a PNG,
# 1x then 2x, as iconutil writes them.
ICNS = [(b"icp4", 16), (b"icp5", 32), (b"icp6", 64), (b"ic07", 128), (b"ic08", 256),
        (b"ic09", 512), (b"ic10", 1024), (b"ic11", 32), (b"ic12", 64), (b"ic13", 256),
        (b"ic14", 512)]
body = b"".join(kind + struct.pack(">I", 8 + len(data)) + data
                for kind, data in ((kind, mac(size).read_bytes()) for kind, size in ICNS))
(root / "app/branding/ilo.icns").write_bytes(b"icns" + struct.pack(">I", 8 + len(body)) + body)

windows = root / "app/platform/windows/package/Assets"
sizes = [16, 20, 24, 32, 40, 48, 64, 256]
images = [Image.open(win(size)) for size in sizes]
images[-1].save(windows / "ilo.ico", sizes=[(s, s) for s in sizes], append_images=images[:-1])
# The names AppxManifest.xml gives them, at the scale-100 sizes.
for name, size in [("Square44x44Logo", 44), ("Square150x150Logo", 150), ("StoreLogo", 50)]:
    shutil.copyfile(win(size), windows / f"{name}.png")

flatpak = root / "app/platform/linux/flatpak"
(flatpak / "icons").mkdir(exist_ok=True)
for size in [16, 24, 32, 48, 64, 128, 256]:
    shutil.copyfile(win(size), flatpak / "icons" / f"{size}.png")
shutil.copyfile(root / "app/branding/icon/win.svg", flatpak / "app.ilo.browser.svg")

site = root / "site/public"
small = [Image.open(win(size)) for size in (16, 32)]
small[1].save(site / "favicon.ico", sizes=[(16, 16), (32, 32)], append_images=small[:1])
shutil.copyfile(root / "app/branding/icon/win-32.svg", site / "favicon.svg")
shutil.copyfile(pngs / "square-180.png", site / "apple-touch-icon.png")
