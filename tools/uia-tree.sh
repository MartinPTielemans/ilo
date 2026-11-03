#!/bin/sh
# Prints the accessibility tree of one Windows process's windows as Narrator reads it over UI
# Automation, in tools/ax-tree.sh's lines: its depth, control type/ARIA role, "name", and its value,
# selected, expanded and active states and automation id where it has them. Chromium's windows,
# the pages' trees among them, are left out. GPUI's AccessKit builds its tree when it's first asked
# for, so with a `text` it asks again for up to 10 s until a line has it. It needs Python's comtypes
# (check-windows.yml installs it) and prints "accessibility: skipped" without it.
# Usage: tools/uia-tree.sh <Windows process id> [text]
set -e
[ $# -ge 1 ] && [ $# -le 2 ] || { echo "usage: $0 <pid> [text]" >&2; exit 2; }
exec python3 -I - "$@" <<'PYTHON'
import ctypes
import sys
import time

# Git Bash's grep reads UTF-8 lines ending in \n alone.
sys.stdout.reconfigure(encoding="utf-8", newline="\n")
sys.stderr.reconfigure(encoding="utf-8", newline="\n")
pid = int(sys.argv[1])
until = sys.argv[2] if len(sys.argv) > 2 else ""

try:
    import comtypes
    import comtypes.client
except ImportError:
    print("accessibility: skipped, Python has no comtypes to read UI Automation with")
    sys.exit(0)
# The bindings comtypes makes from UI Automation's own type library.
uia = comtypes.client.GetModule("UIAutomationCore.dll")
automation = comtypes.client.CreateObject(uia.CUIAutomation, interface=uia.IUIAutomation)
walker = automation.RawViewWalker
TYPES = {
    getattr(uia, name): name[len("UIA_"):-len("ControlTypeId")]
    for name in dir(uia)
    if name.startswith("UIA_") and name.endswith("ControlTypeId")
}
COLLAPSED, EXPANDED, PARTIALLY_EXPANDED = 0, 1, 2


def get(element, name):
    return element.GetCurrentPropertyValue(getattr(uia, "UIA_%sPropertyId" % name))


def line(element, depth, front):
    kind = TYPES.get(get(element, "ControlType"), "?")
    words = ['%d %s/%s "%s"' % (depth, kind, get(element, "AriaRole") or "-", get(element, "Name"))]
    if get(element, "IsValuePatternAvailable") and get(element, "ValueValue"):
        words.append("value=" + get(element, "ValueValue"))
    if get(element, "IsSelectionItemPatternAvailable") and get(element, "SelectionItemIsSelected"):
        words.append("selected")
    if get(element, "IsExpandCollapsePatternAvailable"):
        state = get(element, "ExpandCollapseExpandCollapseState")
        if state in (EXPANDED, PARTIALLY_EXPANDED):
            words.append("expanded")
        elif state == COLLAPSED:
            words.append("collapsed")
    # As AT-SPI's active: the window that has the keyboard, which is in front.
    if depth == 0 and get(element, "NativeWindowHandle") == front:
        words.append("active")
    if get(element, "AutomationId"):
        words.append("id=" + get(element, "AutomationId"))
    return " ".join(words)


GWL_STYLE, WS_CHILD = -16, 0x40000000


def owned(element):
    """Whether it's a window of its own that UI Automation puts under its owner, as a menu's is."""
    hwnd = get(element, "NativeWindowHandle")
    return bool(hwnd) and not ctypes.windll.user32.GetWindowLongW(hwnd, GWL_STYLE) & WS_CHILD


def dump():
    """The tree's lines, and why there are none."""
    front = ctypes.windll.user32.GetForegroundWindow() or 0
    lines = []

    # A window's lines, after those of the windows it owns: as AT-SPI and AX have them, each window
    # is at depth 0, and one's owned windows are in front of it.
    def window(element):
        mine, theirs = [], []

        def walk(element, depth):
            mine.append(line(element, depth, front))
            # A page's tree is Chromium's, and long, as are its windows'.
            if depth >= 15 or get(element, "ControlType") == uia.UIA_DocumentControlTypeId:
                return
            child = walker.GetFirstChildElement(element)
            while child:
                if (get(child, "ClassName") or "").startswith("Chrome_"):
                    pass
                elif owned(child):
                    theirs.append(child)
                else:
                    walk(child, depth + 1)
                child = walker.GetNextSiblingElement(child)

        walk(element, 0)
        for each in theirs:
            window(each)
        lines.extend(mine)

    try:
        ours = automation.CreatePropertyCondition(uia.UIA_ProcessIdPropertyId, pid)
        # The desktop's children come front to back.
        windows = automation.GetRootElement().FindAll(uia.TreeScope_Children, ours)
        for each in range(windows.Length):
            window(windows.GetElement(each))
    except comtypes.COMError as error:
        # Such as an element that went as the tree changed.
        return [], str(error)
    if not lines:
        return [], "process %d has no windows" % pid
    return lines, "no line has it"


def found(lines):
    return lines and (not until or any(until in each for each in lines))


lines, why = dump()
deadline = time.monotonic() + 10
while not found(lines) and time.monotonic() < deadline:
    time.sleep(0.2)
    lines, why = dump()
if not found(lines):
    print("uia-tree: " + why, file=sys.stderr)
print("\n".join(lines))
PYTHON
