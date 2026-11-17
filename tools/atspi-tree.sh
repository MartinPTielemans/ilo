#!/bin/sh
# Prints the accessibility tree of one process's windows as Orca reads it over AT-SPI, in
# tools/ax-tree.sh's lines: its depth, role, "name" (or description), and its text, selected,
# expanded and active states and identifier where it has them. Chromium's tree of the page is
# another application on the bus and left out. GPUI's whole tree comes in one call to AccessKit's
# Cache, and then each node is asked only for its identifier, and a field for its text. GPUI
# publishes it only while the session bus's org.a11y.Status says it's on, so with a `text` it asks
# again for up to 10 s until a line has it. It reads the AT-SPI bus $AT_SPI_BUS_ADDRESS names, or
# else the one the session bus's org.a11y.Bus gives, and prints "accessibility: skipped" if none
# answers.
# Usage: tools/atspi-tree.sh <pid> [text]
set -e
[ $# -ge 1 ] && [ $# -le 2 ] || { echo "usage: $0 <pid> [text]" >&2; exit 2; }
exec python3 -I - "$@" <<'PYTHON'
import json
import os
import subprocess
import sys
import time

pid = int(sys.argv[1])
until = sys.argv[2] if len(sys.argv) > 2 else ""
ROOT = "/org/a11y/atspi/accessible/root"
ACCESSIBLE = "org.a11y.atspi.Accessible"
# AT-SPI's state numbers, a bit each in two words.
ACTIVE, CHECKED, COLLAPSED, EXPANDABLE, EXPANDED, SELECTED = 1, 4, 5, 9, 10, 23
# AT-SPI's role names by number, as at-spi2-core's atspi_role_get_name gives them: AccessKit has
# no GetRoleName.
ROLES = [
    "invalid", "accelerator label", "alert", "animation", "arrow", "calendar", "canvas",
    "check box", "check menu item", "color chooser", "column header", "combo box", "date editor",
    "desktop icon", "desktop frame", "dial", "dialog", "directory pane", "drawing area",
    "file chooser", "filler", "focus traversable", "font chooser", "frame", "glass pane",
    "html container", "icon", "image", "internal frame", "label", "layered pane", "list",
    "list item", "menu", "menu bar", "menu item", "option pane", "page tab", "page tab list",
    "panel", "password text", "popup menu", "progress bar", "button", "radio button",
    "radio menu item", "root pane", "row header", "scroll bar", "scroll pane", "separator",
    "slider", "spin button", "split pane", "status bar", "table", "table cell",
    "table column header", "table row header", "tearoff menu item", "terminal", "text",
    "toggle button", "tool bar", "tool tip", "tree", "tree table", "unknown", "viewport", "window",
    "extended", "header", "footer", "paragraph", "ruler", "application", "autocomplete", "editbar",
    "embedded", "entry", "chart", "caption", "document frame", "heading", "page", "section",
    "redundant object", "form", "link", "input method window", "table row", "tree item",
    "document spreadsheet", "document presentation", "document text", "document web",
    "document email", "comment", "list box", "grouping", "image map", "notification", "info bar",
    "level bar", "title bar", "block quote", "audio", "video", "definition", "article", "landmark",
    "log", "marquee", "math", "rating", "timer", "static", "math fraction", "math root",
    "subscript", "superscript", "description list", "description term", "description value",
    "footnote", "content deletion", "content insertion", "mark", "suggestion", "push button menu",
]


def busctl(address, *args):
    where = ["--address=" + address] if address else ["--user"]
    # After --, so that an argument such as -1 isn't read as an option.
    command = ["busctl", *where, "--json=short", "--", *args]
    run = subprocess.run(command, capture_output=True, text=True)
    if run.returncode:
        raise LookupError(run.stderr.strip())
    return json.loads(run.stdout)["data"]


def skipped(why):
    print("accessibility: skipped, " + why)
    sys.exit(0)


def applications():
    return busctl(bus, "call", "org.a11y.atspi.Registry", ROOT, ACCESSIBLE, "GetChildren")[0]


try:
    bus = os.environ.get("AT_SPI_BUS_ADDRESS") or busctl(
        None, "call", "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus", "GetAddress"
    )[0]
    applications()
except (LookupError, OSError) as error:
    skipped(f"no AT-SPI bus answers: {error}")


def call(name, path, interface, method, *args):
    return busctl(bus, "call", name, path, interface, method, *args)


def get(name, path, interface, prop):
    return busctl(bus, "get-property", name, path, interface, prop)


def gpui():
    """The bus name of the process's GPUI windows: AccessKit's, beside Chromium's own."""
    for name, _ in applications():
        try:
            owner = call(
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "GetConnectionUnixProcessID",
                "s",
                name,
            )[0]
            toolkit = get(name, ROOT, "org.a11y.atspi.Application", "ToolkitName")
            if owner == pid and toolkit == "AccessKit":
                return name
        except LookupError:
            continue
    return None


def line(name, item, depth):
    (_, path), _, _, _, _, interfaces, title, role, description, states = item
    words = [f'{depth} {ROLES[role] if role < len(ROLES) else role} "{title or description}"']
    shown = ""
    if "org.a11y.atspi.Text" in interfaces:
        shown = call(name, path, "org.a11y.atspi.Text", "GetText", "ii", "0", "-1")[0]
    elif "org.a11y.atspi.Value" in interfaces:
        shown = f'{get(name, path, "org.a11y.atspi.Value", "CurrentValue"):g}'
    if shown:
        words.append(f"value={shown}")
    on = lambda state: states[state // 32] >> state % 32 & 1
    if on(SELECTED):
        words.append("selected")
    if on(CHECKED):
        words.append("checked")
    if on(EXPANDED):
        words.append("expanded")
    elif on(EXPANDABLE) or on(COLLAPSED):
        words.append("collapsed")
    if on(ACTIVE):
        words.append("active")
    identifier = get(name, path, ACCESSIBLE, "AccessibleId")
    if identifier:
        words.append(f"id={identifier}")
    return " ".join(words)


def dump():
    """The tree's lines, and why there are none."""
    name = gpui()
    if name is None:
        return [], f"process {pid} has no AccessKit application on the bus"
    try:
        items = call(name, "/org/a11y/atspi/cache", "org.a11y.atspi.Cache", "GetItems")[0]
        children = {}
        for item in items:
            # By bus name and path: the application's parent, the desktop, has its path too.
            children.setdefault(tuple(item[2]), []).append(item)
        lines = []

        def walk(item, depth):
            lines.append(line(name, item, depth))
            if depth < 15:
                for child in sorted(children.get(tuple(item[0]), []), key=lambda child: child[3]):
                    walk(child, depth + 1)

        # The windows are the application's children.
        for window in sorted(children.get((name, ROOT), []), key=lambda child: child[3]):
            walk(window, 0)
        return lines, "no line has it"
    except LookupError as error:
        # Such as a node that went as the tree changed.
        return [], str(error)


def found(lines):
    return lines and (not until or any(until in each for each in lines))


lines, why = dump()
deadline = time.monotonic() + 10
while not found(lines) and time.monotonic() < deadline:
    time.sleep(0.2)
    lines, why = dump()
if not found(lines):
    print(f"atspi-tree: {why}", file=sys.stderr)
print("\n".join(lines))
PYTHON
