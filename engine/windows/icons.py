"""Reads the icon groups a Windows executable or library carries and checks one against an .ico.
Explorer, Task Manager and shortcuts show an executable's first group, and Chromium's windows take
chrome.dll's IDR_MAINFRAME (#101), so the engine's must hold ilo's icon, not Chromium's (#1350).
A group is a name, #<id>, or its index as Explorer counts them, from 0. Runs anywhere, on any
Windows engine: windows/cross/package.sh checks each one it packs.

    icons.py <file>                  lists the groups, each with its images
    icons.py <file> <group> <ico>    fails unless the group holds exactly the .ico's images
"""

import mmap
import struct
import sys

RT_ICON = 3
RT_GROUP_ICON = 14


def resources(pe):
    """{type: {name: data}}, a name being a string or an int ID, each of a name's first language."""
    if pe[:2] != b"MZ":
        sys.exit("not a Windows executable or library")
    header = struct.unpack_from("<I", pe, 0x3C)[0]
    sections_count, optional_size = struct.unpack_from("<H12xH", pe, header + 6)
    optional = header + 24
    # The resource table is the third data directory, after PE32's 96 or PE32+'s 112 bytes.
    directories = optional + (112 if struct.unpack_from("<H", pe, optional)[0] == 0x20B else 96)
    root_rva = struct.unpack_from("<I", pe, directories + 16)[0]
    sections = []
    for i in range(sections_count):
        at = optional + optional_size + 40 * i + 8
        size, rva, raw_size, raw = struct.unpack_from("<4I", pe, at)
        sections.append((rva, max(size, raw_size), raw))

    def offset(rva):
        for start, size, raw in sections:
            if start <= rva < start + size:
                return raw + rva - start
        sys.exit(f"no section holds RVA {rva:#x}")

    root = offset(root_rva)

    def entries(at):
        named, ids = struct.unpack_from("<HH", pe, at + 12)
        for i in range(named + ids):
            name, target = struct.unpack_from("<II", pe, at + 16 + 8 * i)
            if name & 0x80000000:
                length = struct.unpack_from("<H", pe, root + (name & 0x7FFFFFFF))[0]
                start = root + (name & 0x7FFFFFFF) + 2
                name = bytes(pe[start:start + 2 * length]).decode("utf-16-le")
            yield name, target

    table = {}
    for kind, kinds in entries(root):
        if kind not in (RT_ICON, RT_GROUP_ICON) or not kinds & 0x80000000:
            continue
        table[kind] = {}
        for name, languages in entries(root + (kinds & 0x7FFFFFFF)):
            _, leaf = next(entries(root + (languages & 0x7FFFFFFF)))
            rva, size = struct.unpack_from("<II", pe, root + leaf)
            table[kind][name] = bytes(pe[offset(rva):offset(rva) + size])
    return table


def label(name):
    return name if isinstance(name, str) else f"#{name}"


def describe(images):
    return " ".join(f"{w}:{'png' if data[:4] == b'\x89PNG' else 'bmp'}" for w, _, data in images)


def group_images(table, name):
    """The group's images as (width, height, data), as an .ico's are."""
    group = table[RT_GROUP_ICON][name]
    count = struct.unpack_from("<H", group, 4)[0]
    images = []
    for i in range(count):
        w, h, _, _, _, _, _, icon = struct.unpack_from("<BBBBHHIH", group, 6 + 14 * i)
        images.append((w or 256, h or 256, table[RT_ICON][icon]))
    return images


def ico_images(path):
    data = open(path, "rb").read()
    count = struct.unpack_from("<H", data, 4)[0]
    images = []
    for i in range(count):
        w, h, _, _, _, _, size, at = struct.unpack_from("<BBBBHHII", data, 6 + 16 * i)
        images.append((w or 256, h or 256, data[at:at + size]))
    return images


def main():
    if len(sys.argv) not in (2, 4):
        sys.exit(__doc__)
    with open(sys.argv[1], "rb") as file:
        with mmap.mmap(file.fileno(), 0, access=mmap.ACCESS_READ) as pe:
            table = resources(pe)
    # In the table's order, names and then IDs, which is how Explorer counts them.
    names = list(table.get(RT_GROUP_ICON, {}))
    if len(sys.argv) == 2:
        for index, name in enumerate(names):
            print(f"{index} {label(name)}: {describe(group_images(table, name))}")
        return
    group, ico = sys.argv[2], sys.argv[3]
    if group.isdigit():
        name = names[int(group)] if int(group) < len(names) else None
    else:
        name = int(group[1:]) if group.startswith("#") else group
    if name not in names:
        sys.exit(f"{sys.argv[1]} has no icon group {group}")
    have, want = group_images(table, name), ico_images(ico)
    if sorted(have) != sorted(want):
        sys.exit(f"{sys.argv[1]}'s {label(name)} isn't {ico}: it has {describe(have)}, "
                 f"the .ico {describe(want)}")
    print(f"{sys.argv[1]}'s {label(name)} is {ico}: {describe(have)}")


main()
