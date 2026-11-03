"""The appcast's entries for every OS, around generate_appcast, which knows only the Mac's.

Sparkle takes an enclosure marked "macos" or not marked at all, and ilo's Windows updater
(app/platform/windows/updates.cc) only one marked "windows…", so every enclosure is marked.
tools/appcast.sh runs it:

  appcast.py aside APPCAST ASIDE
      Moves the Windows entries to ASIDE, so generate_appcast sees only the Mac's, and prints how
      many Mac entries there are.
  appcast.py signed APPCAST BUILD
      Fails unless the Mac's entry for BUILD carries its EdDSA signature.
  appcast.py finish APPCAST ASIDE [--windows URL LENGTH SIGNATURE VERSION BUILD NOTES]
      Marks the Mac enclosures and puts the Windows entries back after them, with a new one for
      the installer at URL, which replaces one with the same build number. NOTES, the release's
      changelog entry, may be empty, as a nightly's is.
  appcast.py nightly APPCAST BUILD KEEP
      Names BUILD's entries by their version and build, as nightlies share a version, and keeps
      each OS's newest KEEP builds.

For the updates run (app/check/updates.sh), which never reads the owner's key or feed:

  appcast.py key KEYFILE
      Writes a new EdDSA key to KEYFILE, as Sparkle's tools read one with --ed-key-file, and prints
      its public half, for the app's SUPublicEDKey. It never touches the Keychain.
  appcast.py local APPCAST VERSION BUILD NOTES [URL LENGTH SIGNATURE]
      Writes an appcast of one update, as generate_appcast writes one with its Markdown notes
      embedded: the archive at URL, or with no archive, an informational update, which Sparkle
      shows and never downloads.
"""

import base64
import email.utils
import hashlib
import os
import sys
import xml.etree.ElementTree as ET

SPARKLE = "http://www.andymatuschak.org/xml-namespaces/sparkle"
ET.register_namespace("sparkle", SPARKLE)
ET.register_namespace("dc", "http://purl.org/dc/elements/1.1/")


def s(name):
    return f"{{{SPARKLE}}}{name}"


def os_of(item):
    enclosure = item.find("enclosure")
    return "" if enclosure is None else enclosure.get(s("os"), "")


def build_of(item):
    version = item.findtext(s("version"))
    enclosure = item.find("enclosure")
    if version is None and enclosure is not None:
        version = enclosure.get(s("version"))
    return version


def write(tree, path):
    ET.indent(tree, space="    ")
    tree.write(path, encoding="utf-8", xml_declaration=True)


def aside(appcast, aside_path):
    tree = ET.parse(appcast)
    channel = tree.getroot().find("channel")
    windows = [item for item in channel.findall("item") if os_of(item).startswith("windows")]
    for item in windows:
        channel.remove(item)
    rss = ET.Element("rss", {"version": "2.0"})
    ET.SubElement(rss, "channel").extend(windows)
    write(ET.ElementTree(rss), aside_path)
    write(tree, appcast)
    print(len(channel.findall("item")))


def signed(appcast, build):
    for item in ET.parse(appcast).getroot().find("channel").findall("item"):
        enclosure = item.find("enclosure")
        if build_of(item) == build and enclosure is not None and enclosure.get(s("edSignature")):
            return
    sys.exit(f"{appcast} has no signed update for build {build}")


def finish(appcast, aside_path, windows):
    tree = ET.parse(appcast)
    channel = tree.getroot().find("channel")
    for item in channel.findall("item"):
        enclosure = item.find("enclosure")
        if enclosure is not None and not enclosure.get(s("os")):
            enclosure.set(s("os"), "macos")
    kept = ET.parse(aside_path).getroot().find("channel").findall("item")
    if windows:
        url, length, signature, version, build, notes = windows
        kept = [item for item in kept if build_of(item) != build]
        item = ET.Element("item")
        ET.SubElement(item, "title").text = version
        ET.SubElement(item, "pubDate").text = email.utils.formatdate(usegmt=True)
        ET.SubElement(item, s("version")).text = build
        ET.SubElement(item, s("shortVersionString")).text = version
        # The release's changelog entry, for WinSparkle, which a copy from before ilo's own updater
        # still runs.
        if notes:
            ET.SubElement(item, s("releaseNotesLink")).text = notes
        ET.SubElement(item, "enclosure", {
            "url": url,
            s("os"): "windows-x64",
            "length": length,
            "type": "application/octet-stream",
            s("edSignature"): signature,
        })
        kept.insert(0, item)
    channel.extend(kept)
    write(tree, appcast)


def nightly(appcast, build, keep):
    tree = ET.parse(appcast)
    channel = tree.getroot().find("channel")
    items = channel.findall("item")
    for item in items:
        short = item.find(s("shortVersionString"))
        if build_of(item) != build or short is None:
            continue
        short.text = f"{short.text} ({build})"
        if item.find("title") is not None:
            item.find("title").text = short.text
    newest = {}
    for item in sorted(items, key=lambda item: int(build_of(item)), reverse=True):
        newest[os_of(item)] = newest.get(os_of(item), 0) + 1
        if newest[os_of(item)] > int(keep):
            channel.remove(item)
    write(tree, appcast)


# Ed25519 (RFC 8032) on the standard library: the public key of a 32-byte seed, which is what
# Sparkle's newer keys keep.
P = 2**255 - 19
D = -121665 * pow(121666, P - 2, P) % P


def point_add(a, b):
    x1, y1, z1, t1 = a
    x2, y2, z2, t2 = b
    e1 = (y1 - x1) * (y2 - x2) % P
    e2 = (y1 + x1) * (y2 + x2) % P
    e3 = 2 * t1 * t2 * D % P
    e4 = 2 * z1 * z2 % P
    e, f, g, h = e2 - e1, e4 - e3, e4 + e3, e2 + e1
    return (e * f % P, g * h % P, f * g % P, e * h % P)


def base_point():
    y = 4 * pow(5, P - 2, P) % P
    x2 = (y * y - 1) * pow(D * y * y + 1, P - 2, P) % P
    x = pow(x2, (P + 3) // 8, P)
    if (x * x - x2) % P:
        x = x * pow(2, (P - 1) // 4, P) % P
    x = P - x if x & 1 else x
    return (x, y, 1, x * y % P)


def public_key(seed):
    digest = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(digest[:32], "little") & ((1 << 254) - 8) | (1 << 254)
    point, total = base_point(), (0, 1, 1, 0)
    while scalar:
        if scalar & 1:
            total = point_add(total, point)
        point = point_add(point, point)
        scalar >>= 1
    x, y, z, _ = total
    inverse = pow(z, P - 2, P)
    x, y = x * inverse % P, y * inverse % P
    return (y | (x & 1) << 255).to_bytes(32, "little")


def key(path):
    seed = os.urandom(32)
    with open(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as f:
        f.write(base64.b64encode(seed).decode() + "\n")
    print(base64.b64encode(public_key(seed)).decode())


def local(appcast, version, build, notes, archive):
    rss = ET.Element("rss", {"version": "2.0"})
    channel = ET.SubElement(rss, "channel")
    ET.SubElement(channel, "title").text = version
    item = ET.SubElement(channel, "item")
    ET.SubElement(item, "title").text = version
    ET.SubElement(item, "pubDate").text = email.utils.formatdate(usegmt=True)
    ET.SubElement(item, s("version")).text = build
    ET.SubElement(item, s("shortVersionString")).text = version
    with open(notes) as f:
        ET.SubElement(item, "description", {s("format"): "markdown"}).text = f.read()
    if archive:
        url, length, signature = archive
        ET.SubElement(item, "enclosure", {
            "url": url,
            "length": length,
            "type": "application/octet-stream",
            s("edSignature"): signature,
        })
    else:
        ET.SubElement(item, s("informationalUpdate"))
        ET.SubElement(item, "link").text = "http://127.0.0.1/"
    write(ET.ElementTree(rss), appcast)


if __name__ == "__main__":
    command, args = sys.argv[1], sys.argv[2:]
    if command == "aside" and len(args) == 2:
        aside(*args)
    elif command == "signed" and len(args) == 2:
        signed(*args)
    elif command == "finish" and len(args) == 2:
        finish(*args, None)
    elif command == "finish" and len(args) == 9 and args[2] == "--windows":
        finish(args[0], args[1], args[3:])
    elif command == "nightly" and len(args) == 3:
        nightly(*args)
    elif command == "key" and len(args) == 1:
        key(*args)
    elif command == "local" and len(args) in (4, 7):
        local(*args[:4], args[4:])
    else:
        sys.exit(__doc__)
