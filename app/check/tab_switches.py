"""Chrome's tab switch spans in the JSON trace TRACE, in ms a line, for tabs and perf's traced leg.

The spans are TabSwitching::Latency's, from the tab strip's change to the first frame presented,
begin and end events paired by id, in microseconds. A switch to a tab whose page hasn't loaded yet,
as when a new tab shows, times the page load; Chrome reports those apart from switches, and so
does this. Chrome times none between the tabs of a split view, as to the one Add Split View opens
beside the tab shown, whose page is on screen already, and the core has it do the same for ours. A
switch cut off before its frame showed, as by the next switch while ⌃ is held, never finished, so
Chrome's histograms leave it out; it's a line "incomplete".

Usage: tab_switches.py TRACE
"""

import json
import sys

events = json.load(open(sys.argv[1], encoding="utf-8"))
events = events["traceEvents"] if isinstance(events, dict) else events
begins = {}
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
            print("incomplete" if incomplete else (event["ts"] - begin) / 1000)
