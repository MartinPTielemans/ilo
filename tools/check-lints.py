#!/usr/bin/env python3
"""Lints for check.sh's runs and the log lines they read (#2196), which check-scripts.yml runs:

- A "check.sh reads this line" marker sits over a log line that a run reads. Over a line nothing
  matches, it freezes dead wording and sends a reader looking for a step that isn't there.
- A bare `sleep N` outside a poll loop says on its line, or just above it, what window it waits
  out. One that stands in for a wait on the app passes on a fast machine and fails on a slow one.
- A `none_since` straight after a `send` of input follows an `expect` that the app handled it:
  without one it can pass because the app hasn't got to the input yet.
- A file in app/check/ that check.sh runs starts with its place among the runs, in the form
  check.sh reads (#2562). A file without one is a helper or a script of its own, which another
  file names as check/<file>; a run whose line is missing or misspelt would never run, unsaid.
- A key a run presses with a modifier, outside a Windows-only block, isn't one of the keybinds in
  app/check/openbox.xml. On X11 the driver presses keys through the X server as a hand does, so
  Openbox, under Xvfb, would take that key before the app (#2721). Only openbox.xml is read: a
  key that KWin or sway takes isn't caught here, and a step that loses one there is a finding.
- No pattern in a run has a "$" just before "\\|". BSD grep, the Mac's, reads that "$" as a
  dollar sign, not the line's end, so that half of the choice never matches there while GNU's
  matches it on Linux (#2676). A choice keeps its anchors outside it, as "^\\(A\\|B\\)$".
- No Swift, Objective-C or Metal in the product's code outside the Mac's platform layer, app/src,
  app/platform/ but mac/, and core/ but core/platform/mac/ (CONTRIBUTING.md#portability, #3392):
  no file of theirs, no Objective-C reached from Rust, and none of its syntax in a C++ file. The
  checks' own Mac helpers in app/check/ and CI's scripts aren't the product's.
- The app's log lines are written only when the checks run it, as many name what the user
  browses or types, and a desktop can keep an app's stderr, as GNOME's journal does (#3725):
  `log!` in app/src/lib.rs tests `checked()` before it formats, `log_always!` writes only the
  lines in ALWAYS, and no other write to stderr or stdout in the app's Rust formats a value. The
  MCP bridge's binary, which speaks to its client, the preview and the tests aren't the app's.
- The same holds for the hosts' C++ and Objective-C in app/platform/ (#3776): a print to stderr
  or stdout there that formats a string (%s, %@, %ls) sits under a test of Checked() or of the
  checks' variables, or each string it formats is a literal, or one in FIXED, which says why it
  names nothing of the user's. Numbers pass: a size, a count or an id says nothing of a page.
  The drivers, which run only under BROWSER_DRIVE, aren't the app's.

Run from anywhere in the checkout. Prints each finding and exits 1 if there are any.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SELF = "tools/check-lints.py"
READERS = ["app/check.sh", "app/check", "tools", ".github/workflows"]
RUNS = ["app/check.sh", "app/check/*.sh"]

MARKER = re.compile(r"(//|#).*\b(reads|waits for|can wait for) this line")
STRING = re.compile(r'"((?:[^"\\]|\\.)*)"', re.S)
# Rust's {...} and C's %s and the like.
PLACEHOLDER = re.compile(r"\{[^{}]*\}|%[-+ #0-9.*]*(?:ll|l|z|h|j)?[a-zA-Z@]")
# The driver's commands that ask it something rather than send the app input.
QUERIES = {"state", "layout", "screen", "order", "title", "pasteboard", "pip", "front", "stack",
           "menubar"}


def tracked(*paths):
    out = subprocess.run(["git", "ls-files", "-z", "--", *paths], cwd=ROOT, capture_output=True,
                         text=True, check=True)
    return [p for p in out.stdout.split("\0") if p]


def read(path):
    return (ROOT / path).read_text(errors="replace")


# --- Markers ---------------------------------------------------------------------------------


def literal(lines, at):
    """The log line's text under the marker at `at`: the first string in the next few lines of
    code, with the pieces a C++ << chain or a PRIu64 adds to it, each piece's value as {}."""
    code = []
    for line in lines[at + 1 : at + 16]:
        if line.lstrip().startswith(("//", "#")):
            continue
        code.append(line)
        if line.rstrip().endswith((";", "{", "}")) and "browser:" in "\n".join(code):
            break
    # A Rust string goes on past a \ at a line's end, without the next line's indent.
    code = re.sub(r"\\\n\s*", "", "\n".join(code))
    pieces = [p for p in STRING.finditer(code) if p.group(1).strip()]
    first = next((i for i, p in enumerate(pieces) if "browser:" in p.group(1)), 0)
    pieces = pieces[first:]
    if not pieces:
        return None
    text = pieces[0].group(1)
    for before, piece in zip(pieces, pieces[1:]):
        between = code[before.end() : piece.start()]
        if "<<" not in between and "PRI" not in between:
            break
        text += "{}" + piece.group(1)
    return text


def unescape(text):
    return text.replace('\\"', '"').replace("\\n", "").replace("{{", "{").replace("}}", "}")


def fixed(text):
    """The longest run of the line's words between its placeholders."""
    runs = [r.strip().removeprefix("browser:").strip() for r in PLACEHOLDER.split(unescape(text))]
    return max(runs, key=len)


def patterns(scripts):
    """The scripts' patterns for a browser: line, as Python regexes whose shell variables and
    wildcards match anything, each with whether it has any: a helper's
    `"^browser: command $2 via $3\\$"` reads a line for each call."""
    out = []
    for _, body in re.findall(r"""(['"])\^?(browser: (?:\\.|(?!\1).)*)\1""", scripts):
        body = re.sub(r"\$\([^()]*\)|\$\{[^}]*\}|\$[A-Za-z_0-9]+", "\0", body)
        body = body.replace("\\$", "$").replace(".*", "\0")
        # A pattern with no words of its own, as `"^browser: $1 "`, reads no line in particular.
        if not re.search(r"[A-Za-z]{2}", re.sub(r"\[[^]]*\]", "", body[len("browser:") :])):
            continue
        # grep's basic regex groups with \( \) and \|, its extended one with ( ) and |.
        swap = {"\\(": "(", "\\)": ")", "\\|": "|", "(": "\\(", ")": "\\)", "|": "\\|"}
        basic = re.sub(r"\\[()|]|[()|]", lambda m: swap[m.group()], body)
        for form in (basic, body):
            try:
                out.append((re.compile(form.replace("\0", "(.*)").replace('\\"', '"')), "\0" in body))
            except re.error:
                pass
    return out


def matches(pattern, logged, readers):
    """Whether `pattern` matches `logged`, with what its variables and wildcards take in the
    scripts too, as a helper's arguments are."""
    found = pattern.match(logged)
    return bool(found) and all(
        re.sub(r"\b1\b", "", g).strip(" :,.\"'") in readers for g in found.groups() if g
    )


def unread_markers():
    scripts = "\n".join(read(p) for p in tracked(*READERS) if p != SELF)
    readers, read_by = scripts.replace("\\", ""), patterns(scripts)
    marked = []
    for path in tracked("app/src", "app/platform", "core"):
        if not path.endswith((".rs", ".cc", ".mm", ".h", ".cpp", ".m")):
            continue
        lines = read(path).splitlines()
        marked += [(path, at + 1, literal(lines, at)) for at, l in enumerate(lines) if MARKER.search(l)]
    logged = {(p, n): PLACEHOLDER.sub("1", unescape(t)).rstrip() for p, n, t in marked if t}
    # A pattern with no variables that matches many marked lines, as `'^browser: command bar '`,
    # waits for the family, not for one line's words.
    read_by = [
        p for p, variable in read_by
        if variable or sum(bool(p.match(line)) for line in logged.values()) <= 3
    ]
    found = []
    for path, n, text in marked:
        if text is None:
            found.append((path, n, "a marker over no log line"))
            continue
        words = fixed(text)
        if len(words) < 4 or words in readers:
            continue
        if any(matches(p, logged[path, n], readers) for p in read_by):
            continue
        found.append((path, n, f"no run reads this line ({words!r}): take its marker away"))
    return found


# --- Sleeps and none_since -------------------------------------------------------------------


def run_lines(path):
    """The run's lines, each with its code: no comment, no quoted text, and empty inside a here
    document."""
    out, heredoc = [], None
    for line in read(path).split("\n"):
        if heredoc:
            out.append((line, ""))
            if line.strip() == heredoc:
                heredoc = None
            continue
        code = re.sub(r"'[^']*'|\"(?:[^\"\\]|\\.)*\"", "''", line)
        code = "" if code.lstrip().startswith("#") else re.split(r"\s#", code)[0]
        found = re.search(r"<<-?\s*'?(\w+)'?", code)
        if found:
            heredoc = found.group(1)
        out.append((line, code))
    return out


def bare_sleeps():
    found = []
    for path in tracked(*RUNS):
        lines, depth = run_lines(path), 0
        for n, (line, code) in enumerate(lines):
            for sleep in re.finditer(r"(^|[;&|(]|\bthen|\bdo|\belse)\s*sleep\s+[0-9$]", code):
                before = code[: sleep.start() + 1]
                looped = depth + len(re.findall(r"(?:^|[;\s])do(?=\s|$)", before)) - len(
                    re.findall(r"(?:^|[;\s])done\b", before))
                said = re.search(r"\s#", line) or (n and lines[n - 1][0].lstrip().startswith("#"))
                if looped <= 0 and not said:
                    found.append((path, n + 1, "a sleep with no word of what window it waits out: "
                                  "wait for the app's line, or say on its line or above it"))
            depth += len(re.findall(r"(?:^|[;\s])do(?=\s|$)", code))
            depth -= len(re.findall(r"(?:^|[;\s])done\b", code))
            depth = max(depth, 0)
    return found


def quiet_after_send():
    found = []
    for path in tracked(*RUNS):
        lines = run_lines(path)
        for n, (_, code) in enumerate(lines):
            if not re.search(r"(^|[;&|!(]|\bthen|\bif)\s*none_since\b", code):
                continue
            k = n - 1
            while k >= 0 and not lines[k][1].strip():
                k -= 1
            before = lines[k][1].split() if k >= 0 else []
            if before[:1] == ["send"] and before[1:2] and before[1] not in QUERIES:
                found.append((path, n + 1, "none_since straight after the input it's about: "
                              "expect a line that says the app handled it first"))
    return found


# --- Each run's place ------------------------------------------------------------------------

PLACE = re.compile(r"# check\.sh runs this at [0-9]+(, only when named)?\.")


def run_places():
    found = []
    others = {path: read(path) for path in tracked(*READERS)}
    # Only check/ itself: a file in a folder under it, such as check/import/, is one a run sources.
    for path in tracked(":(glob)app/check/*.sh"):
        lines = read(path).splitlines()
        said = [n for n, line in enumerate(lines) if line.startswith("# check.sh runs this")]
        if said:
            for n in said:
                if n != 0 or not PLACE.fullmatch(lines[n]):
                    found.append((path, n + 1, "a run's place is its first line, as \"# check.sh "
                                  "runs this at 120.\" or \"# check.sh runs this at 700, only when "
                                  "named.\", or check.sh never runs it"))
            continue
        name = "check/" + path.rsplit("/", 1)[1]
        if not any(name in text for other, text in others.items() if other != path):
            found.append((path, 1, "no first line \"# check.sh runs this at <n>.\", so check.sh "
                          "never runs it, and no other file names " + name + " to run it"))
    return found


# --- Keys Openbox takes ----------------------------------------------------------------------

OPENBOX = "app/check/openbox.xml"
# A drive command's key and the Mac's modifiers after it, quoted or not, as in show_space's
# "key 1 ctrl".
DRIVE_KEY = re.compile(r"\b(?:key|keydown|keyonreturn)\s+([^\s\"'$;]+)((?:\s+(?:cmd|ctrl|alt|shift|again))+)")
# The driver's key names whose X keysym is named otherwise.
KEYSYM = {"pageup": "prior", "pagedown": "next"}


def x11_chord(key, words):
    """The X11 chord, as Openbox names its keys, that the Linux driver presses for drive.mm's `key`
    with the Mac's modifiers `words`, as drive.rs's modifiers() maps them: ⌘ to Control, ⌃ and ⌥
    to Alt, and ⌃Tab to Control+Tab."""
    mods = set()
    for word in words:
        mods |= {"cmd": {"C"}, "ctrl": {"A"}, "alt": {"A"}, "shift": {"S"}}.get(word, set())
    if key == "tab" and "ctrl" in words and "cmd" not in words:
        mods = mods - {"A"} | {"C"} | ({"A"} if "alt" in words else set())
    return frozenset(mods), KEYSYM.get(key, key).lower()


def openbox_keys():
    """openbox.xml's keybinds, each as its modifiers and its key, by its name there."""
    binds = {}
    for name in re.findall(r'<keybind key="([^"]+)"', read(OPENBOX)):
        *mods, key = name.split("-")
        binds[(frozenset(mods), key.lower())] = name
    return binds


def windows_only(lines):
    """Whether each of a run's lines is inside an `if windows; then` block, to its else or fi."""
    inside, out = None, []
    for line, code in lines:
        indent = len(line) - len(line.lstrip())
        if inside is not None and indent == inside and re.match(r"\s*(else|elif|fi)\b", code):
            inside = None
        elif inside is None and re.fullmatch(r"\s*if windows; then\s*", code):
            out.append(True)
            inside = indent
            continue
        out.append(inside is not None)
    return out


def openbox_takes():
    found, binds = [], openbox_keys()
    for path in tracked(*RUNS):
        lines = run_lines(path)
        for n, ((line, code), windows) in enumerate(zip(lines, windows_only(lines))):
            if windows or not code.strip():
                continue
            for key in DRIVE_KEY.finditer(re.split(r"\s#", line)[0]):
                words = key.group(2).split()
                bind = binds.get(x11_chord(key.group(1), words))
                if bind:
                    found.append((path, n + 1, f"\"key {key.group(1)} {' '.join(words)}\" is X11's "
                                  f"{bind}, which {OPENBOX} binds, so under Xvfb Openbox takes it "
                                  "before the app: move the bind to Super there, as its others "
                                  "are. This reads openbox.xml alone; KWin's and sway's binds "
                                  "aren't checked"))
    return found


# --- Anchors in a choice ---------------------------------------------------------------------


def dollar_before_choice():
    found = []
    for path in tracked(*RUNS):
        for n, (line, code) in enumerate(run_lines(path)):
            # A here document's lines are another language's, and a comment's are prose.
            if code.strip() and re.search(r"\$\\\|", line):
                found.append((path, n + 1, "a \"$\" before \"\\|\": the Mac's grep, BSD's, reads it "
                              "as a dollar sign, so that half never matches there though GNU's "
                              "does on Linux. Keep the anchors outside the choice, as "
                              "\"^\\(A\\|B\\)$\""))
    return found


# --- The Mac's languages outside its platform layer -------------------------------------------

PRODUCT = ["app/src", "app/platform", "core"]
MAC_LAYER = ("app/platform/mac/", "core/platform/mac/")
MAC_FILE = re.compile(r"\.(swift|m|mm|metal)$")
# Objective-C reached from Rust, and Objective-C's own syntax in a C++ file or header.
OBJC_FROM_RUST = re.compile(r"msg_send!|\bobjc2?(?:_\w+)?::|\bcocoa::|\bclass!\(")
OBJC_IN_CPP = re.compile(r"^\s*(?:@interface|@implementation|@protocol|#import\s*<)")
MAC_CODE = {".rs": OBJC_FROM_RUST, ".cc": OBJC_IN_CPP, ".cpp": OBJC_IN_CPP, ".h": OBJC_IN_CPP}
MOVE_IT = ("CONTRIBUTING.md#portability keeps Swift, Objective-C and Metal in platform/mac/. "
           "Move this into "
           "app/platform/mac/ or core/platform/mac/, behind the host's interface that every OS "
           "implements")


def mac_outside_platform():
    found = []
    for path in tracked(*PRODUCT):
        if path.startswith(MAC_LAYER):
            continue
        if MAC_FILE.search(path):
            found.append((path, 1, f"a Swift, Objective-C or Metal file outside platform/mac/. "
                          f"{MOVE_IT}"))
            continue
        pattern = MAC_CODE.get(pathlib.PurePath(path).suffix)
        if not pattern:
            continue
        for n, line in enumerate(read(path).splitlines()):
            if not line.lstrip().startswith("//") and pattern.search(line):
                found.append((path, n + 1, f"Objective-C outside platform/mac/. {MOVE_IT}"))
    return found


# The lines `log_always!` writes in every launch, by their format strings, each holding nothing
# of what the user browses or types: BROWSER_LOG's areas, which the user turns on and whose lines
# name nothing browsed (app/src/logging.rs), its warning, and the time flatpak.yml reads.
ALWAYS = {
    "browser: [{}] {}",
    "browser: BROWSER_LOG names no area {}; the areas are {}",
    "browser: UI shown at {:.1} ms",
}
# Not the app's: the MCP bridge's binary and the `ilo` command's, whose output is their answer to
# the agent, the preview, whose tabs are made up, and test code.
NOT_THE_APP = ("app/src/bridge.rs", "app/src/agents/bridge.rs", "app/src/ilo_command.rs",
               "app/src/preview.rs", "app/src/shell/swipe/model.rs")
RAW_WRITE = re.compile(r"\b(e?println|e?print)!\(|\bio::std(err|out)\(\)")
MACRO = re.compile(r"macro_rules!\s+log\s*\{(.*?)\n\}", re.S)


def is_test(path):
    return bool(re.search(r"(^|/)tests?(\.rs|/)|_tests\.rs$", path))


def quiet_log_lines():
    found = []
    lib = "app/src/lib.rs"
    bodies = MACRO.findall(read(lib))
    if not bodies or any("$crate::checked()" not in body or re.search(r"format|write", body)
                         for body in bodies):
        found.append((lib, 1, "log! writes or formats without testing checked() first, so every "
                      "launch writes lines that can name what the user browses (#3725). Write "
                      "through log_always! inside `if $crate::checked()`."))
    for path in tracked("app/src", "app/platform"):
        if not path.endswith(".rs") or is_test(path) or path in NOT_THE_APP or path == lib:
            continue
        text = read(path)
        for m in re.finditer(r"\blog_always!\(\s*\"((?:[^\"\\]|\\.)*)\"", text):
            if m.group(1) not in ALWAYS:
                n = text[:m.start()].count("\n") + 1
                found.append((path, n, f"log_always! writes {m.group(1)!r} in every launch, which "
                              "isn't one of ALWAYS in tools/check-lints.py. Use log!, which only the "
                              "checks' runs write, or add it to ALWAYS with why it holds nothing "
                              "of the user's."))
        for m in RAW_WRITE.finditer(text):
            line = text[:m.start()].count("\n")
            call = text[m.start():text.find(";", m.start())]
            if "{" in "".join(STRING.findall(call)) or "io::std" in m.group(0):
                found.append((path, line + 1, "a write to stderr or stdout that formats a value, "
                              "in every launch. Use log!, which only the checks' runs write (#3725)."))
    return found


# --- The hosts' log lines --------------------------------------------------------------------

# The strings the hosts write in every launch, by file and the expression the print formats, each
# naming nothing the user browses or types. A bare name, which says less of where it comes from,
# passes only in its line, so the key holds its format, without its \n.
FIXED = {
    # An overlay's or floating window's name, a literal in the host's table: the command bar, Peek.
    ("app/platform/mac/host.mm", "overlay.name"),
    ("app/platform/mac/host.mm", "overlay->name"),
    ("app/platform/mac/host.mm", "each->name"),
    ("app/platform/windows/host.cc", "floating.name"),
    ("app/platform/windows/host.cc", "FloatingName(hwnd)"),
    ("app/platform/windows/host.cc", "kOverlayNames[index]"),
    ("app/platform/windows/host.cc", "kOverlayNames[overlay]"),
    ("app/platform/windows/host.cc",
     "browser: %s leaves the keyboard: its window is no longer in front", "name"),
    ("app/platform/windows/host.cc", "browser: %s takes the keyboard back from its window", "name"),
    # A class's name, AppKit's or the host's, saying which view has the keys.
    ("app/platform/mac/host.mm", "ClassName(responder)"),
    ("app/platform/mac/host.mm", "ClassName(window)"),
    ("app/platform/mac/host.mm", "ClassName(window.firstResponder)"),
    ("app/platform/mac/host.mm", "ClassName(key.firstResponder)"),
    ("app/platform/mac/host.mm", "NSStringFromClass(to.class).UTF8String"),
    # The names the code gave the thread and the queue that went through exit() (#937), and the quit
    # Apple Event's four-letter reason.
    ("app/platform/mac/host.mm", "browser: exit on thread %llx %s, queue %s", "name"),
    ("app/platform/mac/host.mm", "dispatch_queue_get_label(DISPATCH_CURRENT_QUEUE_LABEL)"),
    ("app/platform/mac/host.mm", "browser: quit Apple Event from %d %s, reason %s", "why"),
    # A literal from the function's callers in the file: how Little ilo's window opened, which
    # window showed, the jump list's step, why updates are off or an update wasn't kept.
    ("app/platform/mac/host.mm", "browser: little window %s", "how"),
    ("app/platform/windows/host.cc", "browser: %s window shown", "what"),
    ("app/platform/windows/host.cc", "browser: jump list refused at %s: 0x%08lx", "step"),
    ("app/platform/windows/updates.cc", "browser: no updates: %s", "off"),
    ("app/platform/windows/updates.cc", "browser: update %s: %s", "wrong"),
    # The Settings window's title, a section's name (Section::name in app/src/settings.rs).
    ("app/platform/mac/host.mm", "g_settings.title.UTF8String"),
    ("app/platform/windows/host.cc", "Utf8(title).c_str()"),
    # The Dock icon's colour as hex, or green or system; the badge's count.
    ("app/platform/mac/dock.mm", "name.UTF8String"),
    ("app/platform/mac/needs_you.mm", "NSApp.dockTile.badgeLabel.UTF8String"),
    ("app/platform/windows/host.cc", "std::to_string(count).c_str()"),
    # The system's words for an error, or Sparkle's, which a user's report of a failed update needs:
    # they may name a path in the app's own folders, never a page.
    ("app/platform/mac/needs_you.mm", "error.localizedDescription.UTF8String"),
    ("app/platform/windows/host.cc", "error.message().c_str()"),
    ("app/platform/mac/updates.mm", "error.domain"),
    ("app/platform/mac/updates.mm", "error.localizedDescription"),
    ('app/platform/mac/updates.mm', 'error.userInfo[@"SUNoUpdateFoundReason"]'),
    ('app/platform/mac/updates.mm', '[@" " stringByAppendingString:cause.description]'),
    ("app/platform/mac/updates.mm", "update %@ prewarmed in %.0f ms, %@", "outcome"),
    ("app/platform/mac/updates.mm", "update cycle ended: %@", "outcome"),
    # An update's version, the build's number and its feed or download's address: ilo's own service
    # or a fork's, never a page; the first line of its notes, from that feed; the app's own
    # signature.
    ("app/platform/mac/updates.mm", "update %@ prewarmed in %.0f ms, %@", "version"),
    ("app/platform/mac/updates.mm", "update %@ signature checked in %.0f ms", "version"),
    ("app/platform/mac/updates.mm", "item.versionString"),
    ("app/platform/mac/updates.mm", "latest.versionString"),
    ("app/platform/mac/updates.mm", "update.versionString"),
    ("app/platform/mac/updates.mm", "update feed %@", "feed"),
    ("app/platform/mac/updates.mm", "update %@ notes %@", "first"),
    ("app/platform/mac/updates.mm", "the cookie key regranted to %@: %d", "signature"),
    ("app/platform/mac/updates.mm",
     '[NSBundle.mainBundle objectForInfoDictionaryKey:@"CFBundleVersion"]'),
    ("app/platform/windows/updates.cc", "update.version.c_str()"),
    ("app/platform/windows/updates.cc", "browser: update: none newer than build %s", "APP_BUILD"),
    ("app/platform/windows/updates.cc", "browser: update feed %s", "url"),
    ("app/platform/windows/updates.cc", "url.c_str()"),
    # updates.mm's Log, whose values this rule reads at each call to it.
    ("app/platform/mac/updates.mm", "line.UTF8String"),
    # The file a driver's command names, as only drive.cc asks for a window's picture.
    ("app/platform/windows/host.cc", "browser: drive: can't write %s", "path"),
}
# The drivers, started only under BROWSER_DRIVE (app/src/main.rs).
DRIVERS = ("app/platform/mac/drive.mm", "app/platform/windows/drive.cc")
HOST_PRINT = re.compile(r"\b(?:fprintf\s*\(\s*std(?:err|out)\s*,|printf\s*\(|NSLog\s*\(|"
                        r"Log\s*\(\s*\[NSString\s+stringWithFormat:|std::c(?:err|out)\b)")
# A printf or NSString specifier: its width, its precision, which a * takes from the arguments,
# and its conversion, s, S or @ for a string.
SPECIFIER = re.compile(r"%(?:%|[-+ #0]*(\*|\d+)?(?:\.(\*|\d+))?(?:hh|h|ll|l|L|z|j|t|q)?"
                       r"([a-zA-Z@]))")
GATE = re.compile(r"(?<!!)(?<!! )\b(?:Checked\(\)|getenv\(\"BROWSER_(?:DRIVE|CHECK)\"\)|"
                  r"GetEnvironmentVariableW\(L\"BROWSER_(?:DRIVE|CHECK)\")")


def gated(condition):
    """Whether `condition` holds only when the checks run the app: it tests them, and nothing
    else can make it true."""
    condition = re.sub(r"^(else\s+)?if\s*", "", condition.strip())
    depth, either = 0, False
    # The if's own brackets, or a choice's around all of it, hold the whole condition.
    while condition.startswith("(") and split_top(condition, 1, len(condition), "") == [
            len(condition) - 1]:
        condition = condition[1:-1].strip()
    for i, c in enumerate(condition):
        depth += (c in "([") - (c in ")]")
        either |= depth == 0 and condition.startswith("||", i)
    return bool(GATE.search(condition)) and not either


def blank(text, strings=True):
    """`text` with its comments, and its strings' insides if `strings`, as spaces, so offsets
    and lines stay where they were."""
    out, i, n = [], 0, len(text)
    while i < n:
        if text.startswith("//", i) or text.startswith("/*", i):
            j = text.find("\n" if text[i + 1] == "/" else "*/", i)
            j = n if j < 0 else j + (0 if text[i + 1] == "/" else 2)
        elif text[i] in "\"'":
            j = i + 1
            while j < n and text[j] not in (text[i], "\n"):
                j += 2 if text[j] == "\\" else 1
            if not strings:
                out.append(text[i:j + 1])
                i = j + 1
                continue
            out.append(text[i] + " " * (j - i - 1) + text[i:j + 1][-1:])
            i = j + 1
            continue
        else:
            out.append(text[i])
            i += 1
            continue
        out.append(re.sub(r"[^\n]", " ", text[i:j]))
        i = j
    return "".join(out)


def split_top(code, start, end, at):
    """The offsets in code[start:end] of the characters in `at` outside brackets, but a `::`'s,
    and of the bracket that closes the one before `start`, if it's there, last."""
    depth, out = 0, []
    for i in range(start, end):
        c = code[i]
        if c in ")]}" and depth == 0:
            return out + [i]
        depth += (c in "([{") - (c in ")]}")
        if depth == 0 and c in at and "::" not in code[i - 1:i + 2]:
            out.append(i)
    return out


def unfixed(path, form, text, code, start, end):
    """The part of the expression text[start:end], written by the print whose format is `form`,
    that's neither a literal, a choice between literals, nor in FIXED, or None if there's none."""
    while start < end and text[start].isspace():
        start += 1
    while end > start and text[end - 1].isspace():
        end -= 1
    expr = " ".join(text[start:end].split())
    # A bare name says less of where it comes from, so FIXED lists it with its line's format.
    key = (path, form, expr) if re.fullmatch(r"[A-Za-z_]\w*", expr) else (path, expr)
    if re.fullmatch(r'@?"(?:[^"\\]|\\.)*"(\s*@?"(?:[^"\\]|\\.)*")*', expr) or key in FIXED:
        return None
    asks = split_top(code, start, end, "?")
    colons = split_top(code, asks[0] + 1, end, ":") if asks else []
    if not colons:
        return expr
    # `a ? b : c` writes b or c, and `a ?: c` a or c, but b only under the checks when a tests them.
    q, colon = asks[0], colons[0]
    first = (start, q) if colon == q + 1 else (q + 1, colon)
    checked_only = colon != q + 1 and gated(text[start:q])
    return ((None if checked_only else unfixed(path, form, text, code, *first)) or
            unfixed(path, form, text, code, colon + 1, end))


def quiet_host_lines():
    found = []
    for path in tracked("app/platform"):
        if not path.endswith((".cc", ".mm", ".h", ".cpp", ".m", ".c")) or path in DRIVERS:
            continue
        text = read(path)
        code, gates = blank(text), blank(text, strings=False)
        opened = []
        prints = {m.start(): m for m in HOST_PRINT.finditer(code)}
        for i, c in enumerate(code):
            if c in "{}":
                if c == "}":
                    opened = opened[:-1]
                    continue
                k = max(code.rfind(";", 0, i), code.rfind("{", 0, i), code.rfind("}", 0, i))
                opened.append(" ".join(gates[k + 1:i].split()))
                continue
            m = prints.get(i)
            if not m:
                continue
            k = max(code.rfind(";", 0, i), code.rfind("{", 0, i), code.rfind("}", 0, i))
            around = opened + [" ".join(gates[k + 1:i].split())]
            if any(re.match(r"(else\s+)?if\b", s) and gated(s) for s in around):
                continue
            if m.group(0).startswith("std::c"):
                found.append((path, text.count("\n", 0, i) + 1, "a stream to stderr or stdout "
                              "in every launch, which this rule can't read (#3776). Write with "
                              "fprintf, under `if (Checked())` if it names a value of the user's."))
                continue
            *commas, end = split_top(code, m.end(), len(code), ",")
            args = list(zip([m.end()] + [c + 1 for c in commas], commas + [end]))
            form = "".join(re.findall(r'"((?:[^"\\]|\\.)*)"', text[slice(*args[0])]))
            form = form.removesuffix("\\n")
            strings, at = [], 1
            for s in SPECIFIER.finditer(form):
                if s.group(0) == "%%":
                    continue
                at += (s.group(1) == "*") + (s.group(2) == "*")
                if s.group(3) in "s@S" and at < len(args):
                    strings.append(args[at])
                at += 1
            for value in strings:
                expr = unfixed(path, form, text, code, *value)
                if expr:
                    line = text.count("\n", 0, i) + 1
                    found.append((path, line, f"a print that writes {expr} in every launch, which "
                                  "can name what the user browses or types (#3776). Put it under "
                                  "`if (Checked())`, or add it to FIXED in tools/check-lints.py "
                                  f"with why it names nothing of the user's, a bare name with its "
                                  f"format {form!r}."))
    return found


def main():
    found = (unread_markers() + bare_sleeps() + quiet_after_send() + run_places() +
             openbox_takes() + dollar_before_choice() +
             mac_outside_platform() + quiet_log_lines() + quiet_host_lines())
    for path, line, message in found:
        print(f"::error file={path},line={line}::{message}")
    print(f"{len(found)} found")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
