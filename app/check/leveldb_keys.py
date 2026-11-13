"""The keys a LevelDB store's files hold, one a line, as Chromium's stores keep them: its log
(*.log) and its tables (*.ldb). A key taken out stays in them until they compact, so a key here was
written, not that it's still there. IndexedDB's keys are UTF-16, so each key's NUL bytes are left
out, and they print as bytes.

The files can't be grepped for a key: a table compresses its blocks with Snappy, and a block's keys
share their prefixes, so once IndexedDB's log moves into a table, no key in it reads as text
(#2514). A block holds entries until it passes 4 KB, so the value that ends a large one, such as
uBlock Origin's selfie of tens of MB, follows every key in its first few KB, and only that much is
taken out of Snappy.
Usage: python3 -I check/leveldb_keys.py STORE..."""

import glob
import os
import struct
import sys


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


def block_keys(data, offset, size, keys):
    raw = data[offset : offset + size]
    snappy = data[offset + size] == 1
    want = 64 << 10
    while True:
        n, b = unsnappy(raw, want) if snappy else (size, raw)
        whole = len(b) >= n
        # Past the block's last entry are its restarts, one each 16 entries, and their count.
        end = n - 4 - 4 * struct.unpack_from("<I", b, n - 4)[0] if whole else n
        i, key, found = 0, b"", []
        while i < end:
            if not whole and n - i == 4 * (2 + (len(found) - 1) // 16):
                break
            if not whole and i + 15 > len(b):
                found = None
                break
            shared, i = varint(b, i)
            unshared, i = varint(b, i)
            length, i = varint(b, i)
            if i + unshared > len(b):
                found = None
                break
            key = key[:shared] + b[i : i + unshared]
            found.append(key[:-8])  # less its sequence number and kind
            i += unshared + length
        if found is not None:
            keys += found
            return
        want = 2 * len(b) + 64


def table_keys(path, keys):
    data = open(path, "rb").read()
    if len(data) < 48:
        return
    _, i = varint(data, len(data) - 48)
    _, i = varint(data, i)
    index_offset, i = varint(data, i)
    index_size, _ = varint(data, i)
    _, index = unsnappy(data[index_offset : index_offset + index_size], 1 << 62) \
        if data[index_offset + index_size] == 1 else (0, data[index_offset : index_offset + index_size])
    restarts = struct.unpack_from("<I", index, len(index) - 4)[0]
    end = len(index) - 4 - 4 * restarts
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
        block_keys(data, offset, size, keys)


def log_keys(path, keys):
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
            batch_keys(record, keys)


def batch_keys(batch, keys):
    count = struct.unpack_from("<I", batch, 8)[0]
    i = 12
    for _ in range(count):
        if i >= len(batch):
            return
        tag = batch[i]
        size, i = varint(batch, i + 1)
        keys.append(batch[i : i + size])
        i += size
        if tag == 1:
            size, i = varint(batch, i)
            i += size


keys = []
for store in sys.argv[1:]:
    for pattern, read in (("*.log", log_keys), ("*.ldb", table_keys)):
        for path in sorted(glob.glob(os.path.join(glob.escape(store), pattern))):
            # The store writes as this reads: a log's last record or a new table may be partial,
            # or a file gone, and its keys come on a later read.
            try:
                read(path, keys)
            except (IndexError, OSError, struct.error):
                pass
out = sys.stdout.buffer
for key in keys:
    out.write(key.replace(b"\0", b"").replace(b"\n", b" ") + b"\n")
