"""The mcp run's agent of ten cases (check/mcp.sh): the check's MCP client, on its own connection,
which mcp.sh hands it over pipes on stdin and stdout, so the cases are steps of the client's one
card. Its last message is a ping with the id "cases-done", whose answer tells mcp.sh's relay that
the answers after it are mcp.sh's again. It does
what an agent does to move ten cases of a web toolkit to Closed, each the same form on a
page of its own: it opens the ten pages in one turn, then takes each step on all ten in one turn,
one call a tab, as the server's instructions tell an agent to, and the server answers a turn's
calls at once (#1476). The pages are tabs in the agent's Space, which the user isn't looking at.

  python3 -I check/cases.py URL < ANSWERS > REQUESTS

URL is the case page's address, to which each tab adds ?n=1 to ?n=10. The connection is past its
initialize already. A case's status is a scripted
dropdown, as such toolkits draw them: a button whose list opens on the page's next frame and closes
once focus leaves it. Save saves on a timer, as a request would. A tab that draws no frames, or
loses focus as the next tab is acted on, fails its step, and a timer a hidden page throttles too
far fails its wait. It prints a line for each turn on standard error, as standard output is the connection's, and
last:
  cases: 10 cases in TURNS turns of CALLS calls, SECONDS s
It fails with why, there too, and exit status 1.
"""

import atexit
import json
import os
import re
import sys
import time

CASES = 10


def fail(why):
    sys.stderr.write(f"cases: {why}\n")
    sys.exit(1)


url = sys.argv[1]
pending = b""
next_id = 10000


def send(message):
    os.write(sys.stdout.fileno(), (json.dumps(message) + "\n").encode())


# On every way out, failing ones too, so the relay never waits on an answer that isn't coming.
atexit.register(send, {"jsonrpc": "2.0", "id": "cases-done", "method": "ping"})


def read_answer():
    global pending
    while b"\n" not in pending:
        chunk = os.read(sys.stdin.fileno(), 65536)
        if not chunk:
            fail("the connection closed")
        pending += chunk
    line, pending = pending.split(b"\n", 1)
    return json.loads(line)


def turn(calls, errors=False):
    """Sends a turn's calls at once and gives their answers' texts in the calls' order, failing
    on an error unless |errors|, when an error's text is its answer."""
    global next_id
    ids = []
    for tool, arguments in calls:
        next_id += 1
        ids.append(next_id)
        send({
            "jsonrpc": "2.0", "id": next_id, "method": "tools/call",
            "params": {"name": tool, "arguments": arguments},
        })
    answers = {}
    while len(answers) < len(ids):
        answer = read_answer()
        answers[answer.get("id")] = answer
    texts = []
    for (tool, arguments), id in zip(calls, ids):
        result = answers[id]["result"]
        text = result["content"][0].get("text", "")
        if result["isError"] and not errors:
            fail(f"{tool} {json.dumps(arguments)} answered: {text}")
        texts.append(text)
    return texts


def ref(outline, role, name):
    found = re.search(rf'- {role} "{re.escape(name)}" \[(e\d+)\]', outline)
    return found and found.group(1)


started = time.monotonic()
turns = calls = 0


def timed(name, step, errors=False):
    global turns, calls
    began = time.monotonic()
    texts = turn(step, errors)
    turns += 1
    calls += len(step)
    print(f"cases: turn {turns} {name}, {len(step)} calls, {time.monotonic() - began:.1f} s",
          file=sys.stderr, flush=True)
    return texts


opened = timed("open", [("open_tab", {"url": f"{url}?n={n}"}) for n in range(1, CASES + 1)])
tabs = []
for text in opened:
    tab = re.match(r"Opened tab (\d+)\.", text)
    if not tab:
        fail(f"open_tab answered: {text[:200]}")
    tabs.append(int(tab.group(1)))
# The page says Ready once its script has run and it has painted, as its outline then has the
# dropdown.
outlines = timed("wait for the form", [
    ("run_steps", {"tab": tab, "steps": [{"action": "wait", "text": "Ready"},
                                         {"action": "snapshot"}]})
    for tab in tabs
])
picks = [ref(outline, "button", "Open") for outline in outlines]
if not all(picks):
    fail(f"a case had no status button: {outlines[picks.index(None)][:400]}")
outlines = timed("open the status", [
    ("click", {"tab": tab, "ref": pick}) for tab, pick in zip(tabs, picks)
])
# The list opens on the next frame, which the click's own outline may come before. On Linux a
# snapshot straight after has come before it too, as a hidden page there may draw a frame only
# every second or so. An agent that doesn't see it yet waits for it with run_steps' wait.
options = [ref(outline, "option", "Closed") for outline in outlines]
if not all(options):
    outlines = timed("wait for the list", [
        ("run_steps", {"tab": tab, "steps": [{"action": "wait", "text": "Closed"},
                                             {"action": "snapshot"}]})
        for tab, option in zip(tabs, options) if not option
    ], errors=True)
    looked = iter(outlines)
    options = [option or ref(next(looked), "option", "Closed") for option in options]
if not all(options):
    fail("a case's status list didn't open in its tab: it drew no frame, or closed as it lost focus")
outlines = timed("choose Closed", [
    ("click", {"tab": tab, "ref": option}) for tab, option in zip(tabs, options)
])
saves = [ref(outline, "button", "Save") for outline in outlines]
if not all(saves):
    fail(f"a case had no Save button: {outlines[saves.index(None)][:400]}")
timed("save", [("click", {"tab": tab, "ref": save}) for tab, save in zip(tabs, saves)])
timed("wait for Saved", [
    ("run_steps", {"tab": tab, "steps": [{"action": "wait", "text": "Saved Closed"}]})
    for tab in tabs
])
took = time.monotonic() - started
turn([("close_tab", {"tab": tab}) for tab in tabs])
print(f"cases: {CASES} cases in {turns} turns of {calls} calls, {took:.1f} s", file=sys.stderr,
      flush=True)
