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
    for path in tracked("app/check/*.sh"):
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


def main():
    found = unread_markers() + bare_sleeps() + quiet_after_send() + run_places()
    for path, line, message in found:
        print(f"::error file={path},line={line}::{message}")
    print(f"{len(found)} found")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
