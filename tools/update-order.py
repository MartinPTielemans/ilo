#!/usr/bin/env python3
# Whether an install of the newest release before this one is offered this commit's build as an
# update, by each updater's own rule (#4086): the version the user sees may start again (0.3.0 to
# 0.1.0), but what each updater compares has to keep climbing.
#   Mac: Sparkle compares CFBundleVersion with the feed's sparkle:version, the count of commits
#     (app/build.sh), with its own SUStandardVersionComparator, ported below.
#   Windows, the app's own updater: the compiled-in build with the feed's, as integers
#     (app/platform/windows/updates.cc).
#   Windows, MSIX and the installer's file version: the four-part version, part by part, which
#     app/platform/windows/package/version.sh makes; a release before it had the shown one's.
#   Flatpak: the portal offers the remote's commit when it isn't the installed one
#     (app/src/flatpak.rs), so the release has only to be a later commit on the same line.
#   winget compares the shown version with the installed one, so it's said, not required.
# Usage: update-order.py [<release tag>] in a whole clone: the release before is the newest tag
# v<x.y.z> or ilo-v<x.y.z> below HEAD but the one being made. Exits 1 if an updater would refuse.

# The comparator is adapted from Sparkle 2.10.1's Sparkle/SUStandardVersionComparator.m
# (https://github.com/sparkle-project/Sparkle, commit ab7d71291238df6e0492143b120c9b696a35f603),
# MIT License:
#
#   Copyright 2007 Andy Matuschak. All rights reserved.
#   Copyright (c) 2006-2013 Andy Matuschak.
#   Copyright (c) 2009-2013 Elgato Systems GmbH.
#   Copyright (c) 2011-2014 Kornel Lesiński.
#   Copyright (c) 2015-2017 Mayur Pawashe.
#   Copyright (c) 2014 C.W. Betts.
#   Copyright (c) 2014 Petroules Corporation.
#   Copyright (c) 2014 Big Nerd Ranch.
#
#   Permission is hereby granted, free of charge, to any person obtaining a copy of this software
#   and associated documentation files (the "Software"), to deal in the Software without
#   restriction, including without limitation the rights to use, copy, modify, merge, publish,
#   distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
#   Software is furnished to do so, subject to the following conditions:
#
#   The above copyright notice and this permission notice shall be included in all copies or
#   substantial portions of the Software.
#
#   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
#   BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
#   NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
#   DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
#   OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

import os
import re
import subprocess
import sys
import unicodedata

NUMBER, STRING, PERIOD, PUNCTUATION, WHITESPACE, DASH = range(6)


def kind(part):
    """A part's type, as Sparkle reads it: by the whole part for "." and "-", else its first
    character."""
    if part == ".":
        return PERIOD
    if part == "-":
        return DASH
    first = part[0]
    if unicodedata.category(first) == "Nd":
        return NUMBER
    if first.isspace():
        return WHITESPACE
    if unicodedata.category(first).startswith("P"):
        return PUNCTUATION
    return STRING


def separator(type_):
    return type_ in (PERIOD, PUNCTUATION, WHITESPACE)


def same_class(a, b):
    if separator(a):
        return separator(b)
    return a == b


def split(version):
    """Runs of one type, each separator alone, up to the first dash."""
    if not version:
        return []
    parts = []
    s = version[0]
    old = kind(s)
    for character in version[1:]:
        new = kind(character)
        if new == DASH:
            break
        if old != new or separator(old):
            parts.append(s)
            s = character
        else:
            s += character
        old = new
    parts.append(s)
    return parts


def leading(parts):
    count = 0
    for part in parts:
        if kind(part) not in (NUMBER, PERIOD):
            break
        count += 1
    return count


def balance(a, b):
    """1.1 against 1.0.0 becomes 1.1.0, so 1.0 and 1.0.0 are equal."""
    count_a, count_b = leading(a), leading(b)
    short, long_, count_short, count_long = (b, a, count_b, count_a)
    if count_b > count_a:
        short, long_, count_short, count_long = (a, b, count_a, count_b)
    for at in range(count_short, count_long):
        short.insert(at, "." if kind(long_[at]) == PERIOD else "0")


def compare(version_a, version_b):
    """Sparkle's order of two versions: -1 when b is newer, 1 when a is, 0 when they're equal."""
    a, b = split(version_a), split(version_b)
    balance(a, b)
    n = min(len(a), len(b))
    for part_a, part_b in zip(a, b):
        type_a, type_b = kind(part_a), kind(part_b)
        if same_class(type_a, type_b):
            if type_a == NUMBER:
                value_a, value_b = int(part_a), int(part_b)
                if value_a != value_b:
                    return 1 if value_a > value_b else -1
            elif type_a == STRING and part_a != part_b:
                return 1 if part_a > part_b else -1
        elif type_a != STRING and type_b == STRING:
            return 1
        elif type_a == STRING and type_b != STRING:
            return -1
        else:
            # A number against a period: the period is wrong.
            return 1 if type_a == NUMBER else -1
    if len(a) != len(b):
        longer, shorter_wins = (a, -1) if len(a) > len(b) else (b, 1)
        # A word left over is a prerelease, older; a number left over is newer.
        return shorter_wins if kind(longer[n]) == STRING else -shorter_wins
    return 0


TAG = re.compile(r"^(ilo-)?v(\d+\.\d+\.\d+)$")


def git(*args, root):
    return subprocess.run(
        ["git", "-C", root, *args], check=True, capture_output=True, text=True
    ).stdout.strip()


def shown(commit, root):
    """app/Cargo.toml's version at the commit, which the app shows."""
    text = git("show", f"{commit}:app/Cargo.toml", root=root)
    return re.search(r'^version = "(.*)"$', text, re.M).group(1)


def four_part(commit, root):
    """The four-part version the commit's Windows packages carry: version.sh's where the commit
    has it, else the shown version and a 0, as package.sh made it before."""
    try:
        script = git("show", f"{commit}:app/platform/windows/package/version.sh", root=root)
    except subprocess.CalledProcessError:
        return shown(commit, root) + ".0"
    return subprocess.run(
        ["bash", "-c", script, "version.sh", commit],
        cwd=root, check=True, capture_output=True, text=True,
    ).stdout.strip()


def release_before(root, making):
    """The newest release tag below HEAD but `making`, by its build."""
    found = []
    for tag in git("tag", "--merged", "HEAD", root=root).split():
        if TAG.match(tag) and tag != making:
            found.append((int(git("rev-list", "--count", tag, root=root)), tag))
    return max(found)[1] if found else None


def main(argv):
    root = git("rev-parse", "--show-toplevel", root=os.getcwd())
    if git("rev-parse", "--is-shallow-repository", root=root) != "false":
        print("update-order: a shallow clone's build numbers are wrong: git fetch --unshallow")
        return 1
    making = argv[1] if len(argv) > 1 else None
    before = release_before(root, making)
    if before is None:
        print("update-order: no release before this one, so nothing to update from.")
        return 0
    old = {
        "shown": shown(before, root),
        "build": git("rev-list", "--count", before, root=root),
        "four": four_part(before, root),
        "commit": git("rev-parse", f"{before}^{{commit}}", root=root),
    }
    new = {
        "shown": shown("HEAD", root),
        "build": git("rev-list", "--count", "HEAD", root=root),
        "four": four_part("HEAD", root),
        "commit": git("rev-parse", "HEAD", root=root),
    }
    numbers = lambda four: tuple(int(part) for part in four.split("."))
    rules = [
        ("Mac, Sparkle's comparator on the build", compare(new["build"], old["build"]) > 0,
         f'{old["build"]} → {new["build"]}'),
        ("Windows, the updater's build", int(new["build"]) > int(old["build"]),
         f'{old["build"]} → {new["build"]}'),
        ("Windows, MSIX and the installer's four-part version",
         numbers(new["four"]) > numbers(old["four"]), f'{old["four"]} → {new["four"]}'),
        ("Flatpak, a later commit on the release's line", new["commit"] != old["commit"],
         f'{old["commit"][:12]} → {new["commit"][:12]}'),
    ]
    print(f'update-order: {before} ({old["shown"]}) to this build ({new["shown"]})')
    failed = 0
    for name, offered, how in rules:
        print(f'  {"offered" if offered else "REFUSED"}: {name}, {how}')
        failed |= not offered
    winget = compare(new["shown"], old["shown"]) > 0
    print(
        f'  {"offered" if winget else "not offered"}: winget under the same id, by the shown '
        f'version, {old["shown"]} → {new["shown"]}; the app\'s own updater carries an install '
        "either way"
    )
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
