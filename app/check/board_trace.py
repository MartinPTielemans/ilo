"""What board-perf reads in a JSON trace, from DevTools' Tracing (cdp.py's trace end).

  board_trace.py switches TRACE
      Chrome's tab switch spans, TabSwitching::Latency's from the tab strip's change to the first
      frame presented, in ms a line in the order the switches began, as tab_switches.py pairs them,
      so a run that alternates two kinds of switch reads them apart. A switch to a page not yet
      loaded is left out, and one cut off before its frame is "incomplete".
  board_trace.py ui TRACE
      The browser's UI thread (CrBrowserMain) in the toplevel category: its longest task in ms and
      where it was posted from, then the Board's own, those taking a WebUI page's message (each
      slice's chrome.send) or posted from browser_core/board, their longest, how many and their
      sum, "MS FROM BOARD_MS BOARD_FROM BOARD_COUNT BOARD_SUM". The longest of all can be another's
      in the same window, such as the ad blocker's extension writing its storage.
  board_trace.py since STAMP LOG PATTERN
      For each line of LOG holding PATTERN, the ms from STAMP, Unix ms, to the time in the line's
      Chromium prefix, "[pid:tid:MMDD/HHMMSS.ffffff:ERROR:...]", which is local time with no year.
"""

import json
import re
import sys
import time


def events_of(path):
    events = json.load(open(path, encoding="utf-8"))
    return events["traceEvents"] if isinstance(events, dict) else events


def switches(events):
    begins = {}
    spans = []
    for event in events:
        if event.get("name") != "TabSwitching::Latency":
            continue
        if event.get("ph") == "b":
            measurement = event.get("args", {}).get("tab_switch_measurement", {})
            begins[event.get("id")] = (event["ts"], "NOT_LOADED" in measurement.get("tab_state", ""),
                                       measurement.get("result") == "RESULT_INCOMPLETE")
        elif event.get("ph") == "e" and event.get("id") in begins:
            begin, not_loaded, incomplete = begins.pop(event.get("id"))
            if not not_loaded:
                spans.append((begin, "incomplete" if incomplete else "%.1f" % ((event["ts"] - begin) / 1000)))
    for _, span in sorted(spans):
        print(span)


def ui(events):
    # The browser's main thread is the only CrBrowserMain: a renderer's is CrRendererMain.
    main = {(event.get("pid"), event.get("tid")) for event in events
            if event.get("ph") == "M" and event.get("name") == "thread_name"
            and event.get("args", {}).get("name") == "CrBrowserMain"}
    if not main:
        sys.exit("the trace names no CrBrowserMain thread")
    on_main = [event for event in events
               if (event.get("pid"), event.get("tid")) in main and event.get("ph") == "X"
               and "dur" in event]
    tasks = [event for event in on_main if "toplevel" in event.get("cat", "").split(",")]
    if not tasks:
        sys.exit("the trace has no toplevel task on CrBrowserMain")
    webui = [event["ts"] for event in on_main if event.get("name") == "Receive mojo message"
             and event.get("args", {}).get("chrome_mojo_event_info", {}).get("mojo_interface_tag")
             == "content.mojom.WebUIHost"]

    def posted(event):
        args = event.get("args", {})
        return str(args.get("src_func") or args.get("src_file") or "-").replace(" ", "_")

    def boards(event):
        if "browser_core/board" in str(event.get("args", {}).get("src_file", "")):
            return True
        return any(event["ts"] <= at <= event["ts"] + event["dur"] for at in webui)

    longest = max(tasks, key=lambda event: event["dur"])
    own = [event for event in tasks if boards(event)]
    if not own:
        sys.exit("the trace has no task of the Board's on CrBrowserMain")
    board = max(own, key=lambda event: event["dur"])
    print("%.1f %s %.1f %s %d %.1f" % (longest["dur"] / 1000, posted(longest), board["dur"] / 1000,
                                       posted(board), len(own), sum(e["dur"] for e in own) / 1000))


def since(stamp, log, pattern):
    stamp = int(stamp)
    year = time.localtime(stamp / 1000).tm_year
    prefix = re.compile(r"^\[\d+:\d+:(\d\d)(\d\d)/(\d\d)(\d\d)(\d\d)\.(\d+):")
    for line in open(log, encoding="utf-8", errors="replace"):
        if pattern not in line:
            continue
        at = prefix.match(line)
        if not at:
            sys.exit("no Chromium prefix: " + line.strip())
        month, day, hour, minute, second, fraction = at.groups()
        local = time.mktime((year, int(month), int(day), int(hour), int(minute), int(second), 0, 0, -1))
        print("%.0f" % ((local + float("0." + fraction)) * 1000 - stamp))


if __name__ == "__main__":
    if len(sys.argv) == 5 and sys.argv[1] == "since":
        since(*sys.argv[2:])
    elif len(sys.argv) == 3 and sys.argv[1] in ("switches", "ui"):
        {"switches": switches, "ui": ui}[sys.argv[1]](events_of(sys.argv[2]))
    else:
        sys.exit(__doc__)
