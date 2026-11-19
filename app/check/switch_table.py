"""perf's traced switches, a row each, from what the run kept and the trace's spans, for #2377.

ROWS has a line a switch, in order: its kind (loaded, check, or unloaded, to a tab without its
page), the drive step that made it or "-", the page, the count its "shown" stamp gave, the Unix ms
just before it and the Unix ms its page said it was shown. The page marks each stamp with
performance.mark, which the trace has with blink.user_timing, so each switch's span is the one that
began after the switch before was shown and before this one was. Without the marks, the spans are
taken in order. For ilo, LOG gives the core's "selected N in X ms" after each step and, on the Mac,
the let-go of the hold that step's "switch keeps the page it left" began: its ms, "limit" for one
let go at the limit, or "merged" for none before the next hold began, which took it over, as when
the next key comes before every page the hold waits on has drawn.

It prints a tab-separated row a switch: BROWSER, its kind, page, count, key ms, shown ms, the span's
tab_state and ms, the selected ms and the let-go ms, "-" for none. Each loaded switch without a span
is a line on standard error, and it exits 1 if there are any.

Usage: switch_table.py BROWSER TRACE ROWS [LOG]
"""

import json
import re
import sys

browser, trace_path, rows_path = sys.argv[1:4]
log_path = sys.argv[4] if len(sys.argv) > 4 else None
events = json.load(open(trace_path, encoding="utf-8"))
events = events["traceEvents"] if isinstance(events, dict) else events

stamp = re.compile(r"^(\S+) shown (\d+) \d+$")
marks = {}
begins = {}
spans = []
for event in events:
    name = event.get("name", "")
    if "blink.user_timing" in event.get("cat", "") and stamp.match(name):
        found = stamp.match(name)
        marks.setdefault((found.group(1), found.group(2)), event["ts"])
    elif name == "TabSwitching::Latency":
        if event.get("ph") == "b":
            measurement = event.get("args", {}).get("tab_switch_measurement", {})
            begins[event.get("id")] = (event["ts"], measurement.get("tab_state", "-"),
                                       measurement.get("result", ""))
        elif event.get("ph") == "e" and event.get("id") in begins:
            begin, state, result = begins.pop(event.get("id"))
            if result == "RESULT_INCOMPLETE":
                state += " incomplete"
            spans.append((begin, state, (event["ts"] - begin) / 1000))
spans.sort()

# A step's own line, not its "drive N done" after.
step_line = re.compile(r"^browser: drive \d+: ")
log = open(log_path, encoding="utf-8", errors="replace").read().splitlines() if log_path else []


def after(step, pattern):
    """The first line matching PATTERN from drive step STEP's line to the next step's."""
    if step == "-":
        return "-"
    inside = False
    for line in log:
        if step_line.match(line):
            if inside:
                break
            inside = line.startswith("browser: drive %s: " % step)
            continue
        found = inside and re.match(pattern, line)
        if found:
            return found.group(1)
    return "-"


def hold(step):
    """The let-go of the hold step STEP began, up to the next hold, on the Mac."""
    if step == "-":
        return "-"
    start = next((i for i, line in enumerate(log) if line.startswith("browser: drive %s: " % step)),
                 None)
    if start is None:
        return "-"
    held = None
    for i in range(start + 1, len(log)):
        if step_line.match(log[i]):
            break
        if log[i] == "browser: switch keeps the page it left":
            held = i
            break
    if held is None:
        return "-"
    for line in log[held + 1:]:
        if line == "browser: switch keeps the page it left":
            return "merged"
        found = re.match(r"^browser: switch let the page it left go after ([0-9.]+) ms$", line)
        if found:
            return found.group(1)
        if line == "browser: switch let the page it left go at the limit":
            return "limit"
    return "merged"


rows = [line.split() for line in open(rows_path, encoding="utf-8") if line.strip()]
missing = []
previous = None
taken = 0
for kind, step, page, count, key, shown in rows:
    span = None
    if marks:
        mark = marks.get((page, count))
        if mark is not None:
            span = next((s for s in spans if (previous is None or s[0] > previous) and s[0] <= mark),
                        None)
            previous = mark
    elif taken < len(spans):
        span, taken = spans[taken], taken + 1
    if span is None and kind == "loaded":
        missing.append("%s shown %s" % (page, count))
    state, ms = (span[1], "%.1f" % span[2]) if span else ("-", "-")
    selected = after(step, r"^browser: selected \d+ in ([0-9.]+) ms$")
    let_go = hold(step)
    print("\t".join([browser, kind, page, count, key, shown, state, ms, selected, let_go]))
for switch in missing:
    print("no TabSwitching::Latency span for the switch to %s" % switch, file=sys.stderr)
sys.exit(1 if missing else 0)
