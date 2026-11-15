"""The Windows desktop's picture and colour, for capture's Portrait Mode on a solid colour (#2682).

`colour RRGGBB` takes the desktop's picture away and fills it with the colour, for this session
only, and prints what it was, which `back` takes to put it back. capture.sh does this only on a CI
runner, never on a desktop someone uses.
Usage: desktop-windows.py colour RRGGBB | back STATE"""

import ctypes
import ctypes.wintypes as w
import sys

user32 = ctypes.windll.user32
SPI_GETDESKWALLPAPER = 0x0073
SPI_SETDESKWALLPAPER = 0x0014
SPIF_SENDCHANGE = 0x02
COLOR_DESKTOP = 1


def set_desktop(picture, colour):
    # Neither change is written to the user's profile: SPIF_UPDATEINIFILE isn't passed, and
    # SetSysColors lasts for the session.
    index = (ctypes.c_int * 1)(COLOR_DESKTOP)
    value = (w.DWORD * 1)(colour)
    if not user32.SetSysColors(1, index, value):
        sys.exit("SetSysColors failed")
    if not user32.SystemParametersInfoW(SPI_SETDESKWALLPAPER, 0, picture, SPIF_SENDCHANGE):
        sys.exit("SPI_SETDESKWALLPAPER failed")


def main():
    if len(sys.argv) != 3 or sys.argv[1] not in ("colour", "back"):
        sys.exit(__doc__.splitlines()[-1])
    if sys.argv[1] == "colour":
        picture = ctypes.create_unicode_buffer(260)
        user32.SystemParametersInfoW(SPI_GETDESKWALLPAPER, len(picture), picture, 0)
        print(f"{user32.GetSysColor(COLOR_DESKTOP)}|{picture.value}")
        rgb = int(sys.argv[2], 16)
        # COLORREF is 0x00BBGGRR.
        set_desktop("", (rgb & 0xFF) << 16 | (rgb & 0xFF00) | rgb >> 16)
    else:
        colour, picture = sys.argv[2].split("|", 1)
        set_desktop(picture, int(colour))


main()
