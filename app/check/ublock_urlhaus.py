"""Which copy of the urlhaus list uBlock Origin keeps in a profile: its `! Updated:` day and the host
it came from, such as `2026-10-14 malware-filter.pages.dev`, or `none` with no copy kept.

uBlock Origin takes each list update from one of the list's mirrors at random, and the urlhaus
list's mirrors have served copies a month apart and 2.5 against 12 MB, which put the launches on a
selfie made from them in two modes 200 ms or more apart (#2906). So perf's selfie rows name the copy.

The store is its extension storage, a LevelDB: its log (*.log) and tables (*.ldb), each record
numbered as written, the newest standing. A table compresses its blocks with Snappy, and only a
block's first 64 KB come out of it: a block ends with the entry that takes it past 4 KB, so a value
that large starts in them, and the list's day is in its first lines.
Usage: python3 -I check/ublock_urlhaus.py STORE"""

import glob
import os
import re
import struct
import sys

LIST = b"cache/urlhaus-1"
REGISTRY = b"assetCacheRegistry"
HEAD = 64 << 10


def varint(b, i):
    n = shift = 0
    while True:
        c = b[i]
        i += 1
        n |= (c & 0x7F) << shift
        shift += 7
        if c < 0x80:
            return n, i


def unsnappy(b, want):
    """The block's length, and its first `want` bytes or more."""
    n, i = varint(b, 0)
    out = bytearray()
    while i < len(b) and len(out) < want:
        tag = b[i]
        i += 1
        kind = tag & 3
        if kind == 0:
            size = tag >> 2
            if size >= 60:
                width = size - 59
                size = int.from_bytes(b[i : i + width], "little")
                i += width
            out += b[i : i + size + 1]
            i += size + 1
            continue
        if kind == 1:
            size = ((tag >> 2) & 7) + 4
            back = ((tag >> 5) << 8) | b[i]
            i += 1
        else:
            size = (tag >> 2) + 1
            width = 2 if kind == 2 else 4
            back = int.from_bytes(b[i : i + width], "little")
            i += width
        start = len(out) - back
        if back >= size:
            out += out[start : start + size]
        else:
            for k in range(size):
                out.append(out[start + k])
    return n, bytes(out)


def block_entries(data, offset, size, found):
    raw = data[offset : offset + size]
    n, b = unsnappy(raw, HEAD) if data[offset + size] == 1 else (size, raw)
    whole = len(b) >= n
    # Past the block's last entry are its restarts and their count.
    end = n - 4 - 4 * struct.unpack_from("<I", b, n - 4)[0] if whole else len(b)
    i, key = 0, b""
    while i < end:
        shared, i = varint(b, i)
        unshared, i = varint(b, i)
        length, i = varint(b, i)
        key = key[:shared] + b[i : i + unshared]
        i += unshared
        tag = struct.unpack("<Q", key[-8:])[0]
        found.append((tag >> 8, tag & 0xFF, key[:-8], b[i : i + min(length, HEAD)]))
        if i + length > len(b):
            return  # a value past what came out ends its block
        i += length


def table_entries(path, found):
    data = open(path, "rb").read()
    if len(data) < 48:
        return
    _, i = varint(data, len(data) - 48)
    _, i = varint(data, i)
    index_offset, i = varint(data, i)
    index_size, _ = varint(data, i)
    raw = data[index_offset : index_offset + index_size]
    _, index = unsnappy(raw, 1 << 62) if data[index_offset + index_size] == 1 else (0, raw)
    end = len(index) - 4 - 4 * struct.unpack_from("<I", index, len(index) - 4)[0]
    i, key = 0, b""
    while i < end:
        shared, i = varint(index, i)
        unshared, i = varint(index, i)
        length, i = varint(index, i)
        key = key[:shared] + index[i : i + unshared]
        i += unshared
        offset, j = varint(index, i)
        size, _ = varint(index, j)
        i += length
        block_entries(data, offset, size, found)


def log_entries(path, found):
    data = open(path, "rb").read()
    record, i = b"", 0
    while i + 7 <= len(data):
        # Records don't cross a 32 KB block, whose last few bytes are left as zeros.
        if 32768 - i % 32768 < 7:
            i += 32768 - i % 32768
            continue
        length, kind = struct.unpack_from("<HB", data, i + 4)
        piece = data[i + 7 : i + 7 + length]
        i += 7 + length
        if kind == 0:
            continue
        record = piece if kind in (1, 2) else record + piece
        if kind in (1, 4) and len(record) >= 12:
            seq, count = struct.unpack_from("<QI", record, 0)
            j = 12
            for k in range(count):
                tag = record[j]
                size, j = varint(record, j + 1)
                key = record[j : j + size]
                j += size
                value = b""
                if tag == 1:
                    size, j = varint(record, j)
                    value = record[j : j + min(size, HEAD)]
                    j += size
                found.append((seq + k, tag, key, value))


found = []
store = sys.argv[1]
for pattern, read in (("*.ldb", table_entries), ("*.log", log_entries)):
    for path in sorted(glob.glob(os.path.join(glob.escape(store), pattern))):
        read(path, found)
newest = {}
for seq, kind, key, value in sorted(found, key=lambda e: e[0]):
    if key in (LIST, REGISTRY):
        newest[key] = value if kind == 1 else None
day = re.search(rb"! Updated: (\d{4}-\d\d-\d\d)", newest.get(LIST) or b"")
host = re.search(rb"https://([A-Za-z0-9.-]+)/[A-Za-z0-9./_-]*urlhaus", newest.get(REGISTRY) or b"")
if not day:
    print("none")
else:
    print(day.group(1).decode() + (" " + host.group(1).decode() if host else ""))
