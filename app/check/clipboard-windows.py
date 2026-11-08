"""Windows' clipboard, for sync-devices as xclip reads the X server's (#2114).

`text` prints the text it holds, if any. `formats` prints the name of each registered format it
holds, one a line, such as ExcludeClipboardContentFromMonitorProcessing, which the app adds to the
recovery words it copies (#641). Reading leaves the clipboard's sequence number as it was, so the
app still clears the words it copied when their time is up.
Usage: clipboard-windows.py text|formats"""

import ctypes
import ctypes.wintypes as w
import sys
import time

# Git Bash's grep reads UTF-8 lines ending in \n alone.
sys.stdout.reconfigure(encoding="utf-8", newline="\n")
user32 = ctypes.windll.user32
kernel32 = ctypes.windll.kernel32
user32.GetClipboardData.restype = w.HANDLE
kernel32.GlobalLock.argtypes = [w.HGLOBAL]
kernel32.GlobalLock.restype = ctypes.c_void_p
kernel32.GlobalUnlock.argtypes = [w.HGLOBAL]
CF_UNICODETEXT = 13


def main():
    if sys.argv[1:] not in (["text"], ["formats"]):
        sys.exit(__doc__.splitlines()[-1])
    # Another process, the app, may hold it open for a moment.
    for _ in range(50):
        if user32.OpenClipboard(None):
            break
        time.sleep(0.1)
    else:
        sys.exit("the clipboard stayed open in another process")
    try:
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
