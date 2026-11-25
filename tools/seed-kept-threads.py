#!/usr/bin/env python3
"""Seeds a throwaway profile folder with kept threads that ended Done, each with its card's line
and picture (#4090), to time a launch that reads them. Made-up titles, lines and pictures only.

The folder must not exist yet, so it can't be anyone's real profile. Start a dev build on it with
BROWSER_PROFILE_DIR=<folder>. A launch brings back the ten that ended last and asks the assistant
to remove the rest, so this writes no conversation: the assistant's removal of one it never had
finds nothing.

Usage: python3 -I tools/seed-kept-threads.py <new folder> [count, default 20]
"""

import json
import os
import secrets
import struct
import sys
import time
import uuid
import zlib


def png(width, height, seed):
    """A made-up picture: rows of one colour each, from `seed`."""
    rows = b""
    for y in range(height):
        colour = bytes([(seed * 40 + y) % 256, (seed * 90) % 256, (y * 3) % 256, 255])
        rows += b"\x00" + colour * width

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(rows))
        + chunk(b"IEND", b"")
    )


def private(path, data):
    with open(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "wb") as out:
        out.write(data)


def main():
    if len(sys.argv) not in (2, 3):
        sys.exit(__doc__)
    profile = sys.argv[1]
    count = int(sys.argv[2]) if len(sys.argv) == 3 else 20
    if os.path.lexists(profile):
        sys.exit(f"{profile} exists: give a folder that doesn't, so it's no one's profile")
    kept = os.path.join(profile, "Kept tasks")
    cards = os.path.join(profile, "Kept threads")
    for folder in (profile, kept, cards):
        os.mkdir(folder, 0o700)
    now = int(time.time())
    rows = []
    for at in range(count):
        card = secrets.token_hex(16)
        rows.append(
            {
                "id": str(uuid.uuid4()),
                "cli": "claude",
                "title": f"Made-up thread {at + 1}",
                "state": "done",
                "ended": now - 60 * at,
                "profile": "Default",
                "card": card,
            }
        )
        folder = os.path.join(cards, card)
        os.mkdir(folder, 0o700)
        line = f"Made-up finding {at + 1}: two seats at 9:05, car 4."
        private(os.path.join(folder, "line"), line.encode())
        private(os.path.join(folder, "card.png"), png(320, 200, at))
    private(os.path.join(kept, "index.json"), json.dumps(rows).encode())
    print(f"{count} kept threads in {profile}")


main()
