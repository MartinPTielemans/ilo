"""Windows' clipboard, for sync-devices as xclip reads the X server's (#2114).

`text` prints the text it holds, if any. `formats` prints the name of each registered format it
holds, one a line, such as ExcludeClipboardContentFromMonitorProcessing, which the app adds to the
recovery words it copies (#641). `png FILE` writes the PNG it holds, as a capture copies one, to
FILE and prints its top left pixel as rrggbb, Portrait Mode's background there (#2682). Reading
leaves the clipboard's sequence number as it was, so the app still clears the words it copied when
their time is up.
Usage: clipboard-windows.py text|formats|png FILE"""

import ctypes
import ctypes.wintypes as w
import struct
import sys
import time
import zlib

# Git Bash's grep reads UTF-8 lines ending in \n alone.
sys.stdout.reconfigure(encoding="utf-8", newline="\n")
user32 = ctypes.windll.user32
kernel32 = ctypes.windll.kernel32
user32.GetClipboardData.restype = w.HANDLE
kernel32.GlobalLock.argtypes = [w.HGLOBAL]
kernel32.GlobalLock.restype = ctypes.c_void_p
kernel32.GlobalUnlock.argtypes = [w.HGLOBAL]
CF_UNICODETEXT = 13
kernel32.GlobalSize.argtypes = [w.HGLOBAL]
kernel32.GlobalSize.restype = ctypes.c_size_t


def top_left(png):
    """`png`, an 8-bit RGB or RGBA PNG, to its end, which the clipboard's memory may run past, and
    its first pixel as rrggbb. Every PNG filter leaves the first row's first pixel as it is, as it
    has nothing left of it or above it."""
    at, idat, colour = 8, b"", None
    while at < len(png):
        size, kind = struct.unpack(">I4s", png[at : at + 8])
        data = png[at + 8 : at + 8 + size]
        if kind == b"IHDR":
            colour = data[8], data[9]
        elif kind == b"IDAT":
            idat += data
        at += 12 + size
        if kind == b"IEND":
            break
    if colour not in ((8, 2), (8, 6)):
        sys.exit(f"the PNG is {colour}, not 8-bit RGB or RGBA")
    row = zlib.decompressobj().decompress(idat, 4)
    return png[:at], row[1:4].hex()


def main():
    if sys.argv[1:] not in (["text"], ["formats"]) and sys.argv[1:2] != ["png"] or (
        sys.argv[1:2] == ["png"] and len(sys.argv) != 3
    ):
        sys.exit(__doc__.splitlines()[-1])
    # Another process, the app, may hold it open for a moment.
    for _ in range(50):
        if user32.OpenClipboard(None):
            break
        time.sleep(0.1)
    else:
        sys.exit("the clipboard stayed open in another process")
    try:
        if sys.argv[1] == "png":
            memory = user32.GetClipboardData(user32.RegisterClipboardFormatW("PNG"))
            if not memory:
                sys.exit("the clipboard holds no PNG")
            png = ctypes.string_at(kernel32.GlobalLock(memory), kernel32.GlobalSize(memory))
            kernel32.GlobalUnlock(memory)
            png, corner = top_left(png)
            with open(sys.argv[2], "wb") as file:
                file.write(png)
            print(corner)
            return
        if sys.argv[1] == "text":
            memory = user32.GetClipboardData(CF_UNICODETEXT)
            if memory:
                print(ctypes.wstring_at(kernel32.GlobalLock(memory)))
                kernel32.GlobalUnlock(memory)
            return
        format = user32.EnumClipboardFormats(0)
        while format:
            name = ctypes.create_unicode_buffer(256)
            if user32.GetClipboardFormatNameW(format, name, len(name)):
                print(name.value)
            format = user32.EnumClipboardFormats(format)
    finally:
        user32.CloseClipboard()


main()
