"""ilo's icon where an engine would carry Chromium's logo (#1350): written into the tree, and read
back from a build. On Windows, Explorer, Task Manager and shortcuts show an executable's first icon
group, and the browser windows take chrome.dll's IDR_MAINFRAME (#101). On Linux the windows' icon
and the logos Chromium shows are PNGs in its .pak files. tree.sh puts ilo.ico's images in place of
Chromium's logo files, and each engine's package.sh reads them back. Runs anywhere, on any engine.

    icons.py groups <exe|dll>               lists its icon groups, each with its images
    icons.py group <exe|dll> <group> <ico>  fails unless the group holds exactly the .ico's
                                            images. A group is a name, #<id>, or its index from 0,
                                            as Explorer counts them.
    icons.py png <ico> <size> <png>         writes the .ico's image at size, or its image at twice
                                            the size halved, as a PNG
    icons.py pak <pak> <ico> <size>...      fails unless the .pak holds png's image at each size,
                                            as many times as the size is listed
"""

import mmap
import os
import struct
import sys
import zlib

RT_ICON = 3
RT_GROUP_ICON = 14
PNG = b"\x89PNG\r\n\x1a\n"


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
    return " ".join(f"{w}:{'png' if data[:4] == PNG[:4] else 'bmp'}" for w, _, data in images)


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


def pixels(png):
    """A PNG's width, height and RGBA rows, or None for one that isn't 8-bit RGBA, as ilo's are."""
    if png[:8] != PNG or len(png) < 33:
        return None
    width, height, depth, kind, _, _, interlace = struct.unpack_from(">IIBBBBB", png, 16)
    if depth != 8 or kind != 6 or interlace:
        return None
    at, data = 8, []
    while at < len(png):
        length, name = struct.unpack_from(">I4s", png, at)
        if name == b"IDAT":
            data.append(png[at + 8:at + 8 + length])
        at += 12 + length
    raw = zlib.decompress(b"".join(data))
    stride = width * 4
    rows, above = [], bytearray(stride)
    for y in range(height):
        start = y * (stride + 1)
        kind, row = raw[start], bytearray(raw[start + 1:start + 1 + stride])
        for x in range(stride):
            left = row[x - 4] if x >= 4 else 0
            corner = above[x - 4] if x >= 4 else 0
            if kind == 1:
                row[x] = (row[x] + left) & 255
            elif kind == 2:
                row[x] = (row[x] + above[x]) & 255
            elif kind == 3:
                row[x] = (row[x] + (left + above[x]) // 2) & 255
            elif kind == 4:
                guess = left + above[x] - corner
                near = min((abs(guess - left), 0, left), (abs(guess - above[x]), 1, above[x]),
                           (abs(guess - corner), 2, corner))
                row[x] = (row[x] + near[2]) & 255
        rows.append(bytes(row))
        above = row
    return width, height, b"".join(rows)


def encoded(width, height, rgba):
    def chunk(name, body):
        crc = struct.pack(">I", zlib.crc32(name + body))
        return struct.pack(">I", len(body)) + name + body + crc

    stride = width * 4
    raw = b"".join(b"\0" + rgba[y * stride:(y + 1) * stride] for y in range(height))
    header = chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    return PNG + header + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def halved(width, height, rgba):
    """Each 2x2 block as one pixel, its colour weighted by alpha, so the transparent corners'
    colour doesn't darken the plate's edge."""
    stride, out = width * 4, bytearray()
    for y in range(0, height, 2):
        for x in range(0, stride, 8):
            top, bottom = y * stride + x, (y + 1) * stride + x
            block = [top, top + 4, bottom, bottom + 4]
            alpha = sum(rgba[at + 3] for at in block)
            for channel in range(3):
                weighted = sum(rgba[at + channel] * rgba[at + 3] for at in block)
                out.append((weighted + alpha // 2) // alpha if alpha else 0)
            out.append((alpha + 2) // 4)
    return width // 2, height // 2, bytes(out)


def ico_png(ico, size):
    """The .ico's image at size as a PNG: its own, or its image at twice the size halved."""
    images = {w: data for w, h, data in ico_images(ico) if w == h and data[:8] == PNG}
    if size in images:
        return images[size]
    if 2 * size in images and pixels(images[2 * size]):
        return encoded(*halved(*pixels(images[2 * size])))
    sys.exit(f"{ico} has no {size} or {2 * size} px PNG")


def pak_resources(pak):
    """A .pak's resources, as ui/base/resource/data_pack.cc reads version 4 and 5."""
    version = struct.unpack_from("<I", pak)[0]
    if version == 5:
        count, at = struct.unpack_from("<H", pak, 8)[0], 12
    elif version == 4:
        count, at = struct.unpack_from("<I", pak, 4)[0], 9
    else:
        sys.exit(f"a .pak of version {version}, not 4 or 5")
    table = [struct.unpack_from("<HI", pak, at + 6 * i)[1] for i in range(count + 1)]
    return [bytes(pak[start:end]) for start, end in zip(table, table[1:])]


def check_pak(path, ico, sizes):
    with open(path, "rb") as file:
        with mmap.mmap(file.fileno(), 0, access=mmap.ACCESS_READ) as pak:
            found = pak_resources(pak)
    ok = True
    for size in sorted(set(sizes)):
        want = pixels(ico_png(ico, size))
        # Compared as pixels: grit can add a chunk to a PNG it scales, and zlib's bytes can differ.
        have = sum(1 for png in found if png[:8] == PNG and len(png) >= 24
                   and struct.unpack_from(">II", png, 16) == (size, size) and pixels(png) == want)
        wanted = sizes.count(size)
        print(f"{path} has {os.path.basename(ico)}'s {size} px image {have} times, of {wanted}")
        ok = ok and have >= wanted
    return ok


def check_group(path, group, ico):
    with open(path, "rb") as file:
        with mmap.mmap(file.fileno(), 0, access=mmap.ACCESS_READ) as pe:
            table = resources(pe)
    # In the table's order, names and then IDs, which is how Explorer counts them.
    names = list(table.get(RT_GROUP_ICON, {}))
    if group is None:
        for index, name in enumerate(names):
            print(f"{index} {label(name)}: {describe(group_images(table, name))}")
        return True
    if group.isdigit():
        name = names[int(group)] if int(group) < len(names) else None
    else:
        name = int(group[1:]) if group.startswith("#") else group
    if name not in names:
        sys.exit(f"{path} has no icon group {group}")
    have, want = group_images(table, name), ico_images(ico)
    if sorted(have) != sorted(want):
        print(f"{path}'s {label(name)} isn't {os.path.basename(ico)}: it has {describe(have)}, "
              f"the .ico {describe(want)}")
        return False
    print(f"{path}'s {label(name)} is {os.path.basename(ico)}: {describe(have)}")
    return True


def main():
    command, given = (sys.argv[1], sys.argv[2:]) if len(sys.argv) > 1 else ("", [])
    if command == "groups" and len(given) == 1:
        ok = check_group(given[0], None, None)
    elif command == "group" and len(given) == 3:
        ok = check_group(*given)
    elif command == "png" and len(given) == 3 and given[1].isdigit():
        with open(given[2], "wb") as file:
            file.write(ico_png(given[0], int(given[1])))
        ok = True
    elif command == "pak" and len(given) > 2 and all(size.isdigit() for size in given[2:]):
        ok = check_pak(given[0], given[1], [int(size) for size in given[2:]])
    else:
        sys.exit(__doc__)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
