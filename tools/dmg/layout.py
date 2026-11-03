#!/usr/bin/env python3
# Writes the .DS_Store that lays out the DMG's Finder window: its size, no toolbar, sidebar, path
# bar or status bar, 128 pt icons where background.svg leaves room for them, and the background
# picture. Finder reads it when the user opens the DMG; nothing drives Finder to make it, so it
# runs on a headless CI runner. Python's standard library only, as macOS's own python3 has it.
# The picture is named by a classic Mac alias, which holds the volume's and the file's IDs and
# dates, so this runs on the mounted image, after .background/background.tiff is in place.
# The format is the one Finder writes, as Mark Mentovai's and Wim Lewis's notes and the ds_store
# and mac_alias Python packages (MIT, Alastair Houghton) describe it.
# Usage: tools/dmg/layout.py <where the volume is mounted> <its name> <the app, such as ilo.app>
import os
import plistlib
import struct
import sys

# In points from the window's top left. The icons' centres; keep background.svg in step.
WINDOW = (640, 400)
APP_AT = (176, 170)
APPLICATIONS_AT = (464, 170)
ICON_SIZE = 128
TEXT_SIZE = 13
BACKGROUND = ".background/background.tiff"

# Seconds from 1904, the classic Mac epoch, to 1970.
MAC_EPOCH = 2082844800


def alias(volume, name, path):
    """A version 2 alias to `path`, a file on the volume `name` mounted at `volume`."""
    full = os.path.join(volume, path)
    folder, filename = os.path.split(path)
    voldate = int(os.stat(volume).st_birthtime) + MAC_EPOCH
    crdate = int(os.stat(full).st_birthtime) + MAC_EPOCH
    # On HFS+ a file's inode number is its catalog node ID.
    folder_cnid = os.stat(os.path.join(volume, folder)).st_ino
    cnid = os.stat(full).st_ino

    def pascal(s, size):
        b = s.replace(":", "/").encode()
        return bytes([len(b)]) + b.ljust(size - 1, b"\0")

    def tag(t, data):
        return struct.pack(">hH", t, len(data)) + data + (b"\0" if len(data) & 1 else b"")

    def utf16(s):
        b = s.encode("utf-16-be")
        return struct.pack(">H", len(b) // 2) + b

    body = struct.pack(">h", 0)  # a file
    body += pascal(name, 28) + struct.pack(">I", voldate) + b"H+" + struct.pack(">h", 0)
    body += struct.pack(">I", folder_cnid) + pascal(filename, 64)
    body += struct.pack(">II", cnid, crdate) + b"\0" * 8  # creator and type
    body += struct.pack(">hhI", -1, -1, 0) + b"\0" * 12  # levels, volume flags, fs ID, reserved
    body += tag(0, folder.encode())
    body += struct.pack(">hHQhHQ", 16, 8, voldate << 16, 17, 8, crdate << 16)
    body += tag(1, struct.pack(">I", folder_cnid))
    body += tag(2, f"{name}:{folder}:\0{filename}".encode())
    body += tag(14, utf16(filename))
    body += tag(15, utf16(name))
    body += tag(18, f"/{path}".encode())
    body += tag(19, f"/Volumes/{name}".encode())
    body += struct.pack(">hH", -1, 0)
    return struct.pack(">4shh", b"\0" * 4, 8 + len(body), 2) + body


def record(filename, code, kind, value):
    out = struct.pack(">I", len(filename)) + filename.encode("utf-16-be") + code + kind
    if kind == b"long":
        return out + struct.pack(">I", value)
    if kind == b"type":
        return out + value
    if kind == b"blob":
        return out + struct.pack(">I", len(value)) + value
    raise ValueError(kind)


def iloc(point):
    return struct.pack(">IIII", point[0], point[1], 0xFFFFFFFF, 0xFFFF0000)


def store(records):
    """A .DS_Store holding `records`, sorted, in one leaf of the B-tree.

    The file is a buddy allocator over 2**31 bytes, its offsets counted from byte 4. The 32-byte
    header takes the first block, which leaves one free block of each width from 5 to 30 at the
    offset 2**width. The root block takes the one of width 11, the B-tree's header the one of
    width 5 and its only leaf the one of width 12; the free lists name the rest.
    """
    records = sorted(records, key=lambda r: (r[0].lower(), r[1]))
    leaf = struct.pack(">II", 0, len(records)) + b"".join(record(*r) for r in records)
    if len(leaf) > 4096:
        raise ValueError("the records don't fit in one leaf")
    dsdb = struct.pack(">IIIII", 2, 0, len(records), 1, 0x1000)
    used = {5: 32, 11: 2048, 12: 4096}
    blocks = [2048 | 11, 32 | 5, 4096 | 12]  # the root, the B-tree's header, its leaf
    root = struct.pack(">II", len(blocks), 0) + struct.pack(">256I", *blocks, *[0] * 253)
    root += struct.pack(">IB4sI", 1, 4, b"DSDB", 1)
    for width in range(32):
        free = [] if width < 5 or width > 30 or width in used else [1 << width]
        root += struct.pack(f">I{len(free)}I", len(free), *free)
    image = bytearray(4 + 4096 + 4096)
    header = struct.pack(">I4sIII", 1, b"Bud1", 2048, 2048, 2048)
    image[0 : len(header)] = header
    image[20:36] = bytes.fromhex("0000100c000000870000200b00000000")
    for offset, data in ((2048, root), (32, dsdb), (4096, leaf)):
        image[4 + offset : 4 + offset + len(data)] = data
    return bytes(image)


def main():
    volume, name, app = sys.argv[1:4]
    window = {
        # Where it opens on screen, and its size with the 28 pt title bar, which Finder counts.
        "WindowBounds": "{{200, 120}, {%d, %d}}" % (WINDOW[0], WINDOW[1] + 28),
        "ShowToolbar": False,
        "ShowSidebar": False,
        "ShowPathbar": False,
        "ShowStatusBar": False,
        "ShowTabView": False,
        "ContainerShowSidebar": False,
        "PreviewPaneVisibility": False,
        "SidebarWidth": 0,
    }
    icons = {
        "viewOptionsVersion": 1,
        "backgroundType": 2,
        "backgroundImageAlias": alias(volume, name, BACKGROUND),
        "backgroundColorRed": 1.0,
        "backgroundColorGreen": 1.0,
        "backgroundColorBlue": 1.0,
        "iconSize": float(ICON_SIZE),
        "textSize": float(TEXT_SIZE),
        "labelOnBottom": True,
        "showIconPreview": True,
        "showItemInfo": False,
        "arrangeBy": "none",
        "gridSpacing": 100.0,
        "gridOffsetX": 0.0,
        "gridOffsetY": 0.0,
        "scrollPositionX": 0.0,
        "scrollPositionY": 0.0,
    }
    binary = plistlib.FMT_BINARY
    records = [
        (".", b"bwsp", b"blob", plistlib.dumps(window, fmt=binary)),
        (".", b"icvp", b"blob", plistlib.dumps(icons, fmt=binary)),
        (".", b"icvl", b"type", b"icnv"),
        (".", b"vSrn", b"long", 1),
        (app, b"Iloc", b"blob", iloc(APP_AT)),
        ("Applications", b"Iloc", b"blob", iloc(APPLICATIONS_AT)),
    ]
    with open(os.path.join(volume, ".DS_Store"), "wb") as f:
        f.write(store(records))


if __name__ == "__main__":
    main()
