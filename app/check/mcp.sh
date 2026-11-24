# check.sh runs this at 380.
# An MCP client reaches the browser through the bridge beside it once Settings › Agents is on and
# it's connected, as the saved settings say here. The socket is the user's alone, and the bridge
# gets nowhere without the agent's token. Its first call makes its Space, which no window lists, and
# its first call that acts on a tab starts its card, one for each connection. Unnamed, the card
# is titled by the web site it first acted on; title names it, its line leading with the client's
# name, and a second name within a minute is refused. It may list and read
# only the tabs there: its list leaves out the user's tab, a read of the user's tab is refused, with
# an activity-log entry, and a read of the check's agent.html, opened in its Space, gives the page's text. A
# link to an app (ms-word:) opens nothing and asks nothing, the tab staying on its page, and the
# click's answer names the scheme. Ten
# cases, each the same form in a tab of its own the user isn't looking at, move to Closed a step a
# turn, a turn's ten calls answered at once, which logs the turns and how long they took. A page
# that hasn't come once an action has waited for it is still loading, as the answer says. An app
# in another site's frame, whose content comes after its load, is answered with its content or
# as busy, never as its splash alone, and navigate waits for a text it names past the page's
# settling. A tab
# its page opens, by a link or window.open, goes in its Space, behind, and the click's answer names
# it. A page with fields gives its prose and none of their values in its text. Its outline gives
# the page's own values and the agent's own typing, and of the rest only that they're filled: the
# user's typing, a script's fill as autofill's is, a card's and a password's. The agent can't type
# into the password field. It types into another, whose line then holds what it typed (#2355).
# A frame nested two sites deep, one the page adds after its load and one it fills by posting a
# form into it are in the page's outline and text too, an empty helper frame is in neither, and a
# click two frames deep lands. A skip link in a frame, off the page until it's focused, is focused
# and then clicked, as Tab would bring it, and a click on one off the page that can't take focus
# is refused rather than answered as done.
# It puts tabs of its own in a split view, adds one, lists it and separates one, each in the
# activity log, and is refused a tab of the user's.
# Every answer is 12,000 characters at most: an article's, a 120-row table's, a JSON response's.
# A page's frames, of its own site and of another, are in its outline, where the agent clicks and
# types, each answered with what changed in the outline alone, and not into a frame's password
# field. Its text leaves out a frame an extension puts in the page, which isn't the page's. Nor is
# the rest of what an extension puts there (#2355): its button is in neither the outline nor the
# text, a click or the wheel where its frame covers the page, or a frame of the page, is refused,
# and so are keys while its frame has the keyboard, each with an activity-log entry, and its
# listeners don't hear the agent's keys or typing. Neither the agent's click nor its ArrowDown and
# Enter in a login field opens or accepts Chrome's suggestion of a saved login. Its keys, typed or
# pressed, go to the page alone, and Escape the page doesn't keep runs no Stop of ilo's. Its Tab
# past its page's last control stays in its page, whose next Tab starts it over, and never reaches
# the user's ring. Where it
# acts in the page watched, its pointer and a
# ring with the element's ref show it. It opens a page, clicks its link, goes back, where the tab
# takes the page's title again, scrolls, takes a screenshot and closes a tab, pins a tab of its own, unpins it and removes it. A close asks a page with a beforeunload
# handler first, with no dialog: one whose handler doesn't ask closes at the first call, as does
# one that asks no one clicked in, and one the agent clicked or typed in stays open until the agent
# discards it. While a page's own alert is up in its tab, a call there answers at once and close_tab
# closes the tab. A site's sign-in that the browser asks for hands its tab to the user, as hand_off
# does, and shows over its page once they watch it; the call that met it waits until Hand Back, and
# the password they type never reaches the agent, the log or the profile. Its download at Work on pages, where each
# task starts, answers with the file, which the answer names. Its upload hands the field's step to
# the user by the field's name, in a subdomain's frame, an object's or another site's, ilo opening
# no dialog: Hand Back on ilo's line gives the tab back and Not Now leaves it the user's. Before any
# of that, upload is refused for a folder's field, a button, a sandboxed frame's field
# and a field in a document whose root element frames none, and a field hidden behind its label is
# outlined as the label's text with no ref (#3656). It sends requests to its tab's site, carrying a session it
# never sees, and a change isn't part of Work on pages: ilo refuses it and it's never sent, a POST
# whose override says GET too. A change without the agent's sentence of what it does is refused
# for that first (#3548). Settings › Agents opens each request to the agent's sentence, where it
# went and what it sent, or that the refused one wasn't, and names each by the method sent. Paying,
# an order, a message as the user and a bank's site are stopped at Work on pages too, nothing
# asked (#4086): each is ilo's line in the conversation with Give Full access, which the keys
# press, and at Full access each goes through, with no answer kept for Settings › Agents to list.
# A site the user adds to Settings › Agents' Off limits is refused whatever the task's access, an
# open of it and each call on a
# tab that came there by a link, which gives nothing of the page meanwhile (#3724). A login the
# user saved fills in neither as the agent's tab loads it nor from a dropdown, and fills once the
# user takes the tab over, without a reload. Closing the client's end ends its session with the tab.
# Reconnected, its next call is a card of its own, whose Stop answers the call under way and closes
# the client's connection. Turning Settings › Agents off takes the socket away and drops a client
# connected. Relaunched, a call with a tab's id from before is refused as one from before ilo
# restarted, its done, sent while its handoff waits on the user, ends its task once that's answered,
# and its next task ends 3 s after it closed its last page (#3652). Its tools offer it nothing of
# Boosts, which are the user's alone.
# It scrolls in a panel and in another site's frame by their refs, until each answers that
# nothing scrolled. A table's rows and a menu's items have refs it clicks by, and an answer says what
# changed in an element, as a button now enabled.
# Its picture of a page paints over each field whose contents its outline withholds: one filled by
# the page's script, as the user's or a password manager's fill is, one it typed into that changed
# since, a password field made a text field and a card's field, in another site's frame and half
# out of view too, while its own unchanged typing and the page's own values show. An extension's
# own frames, over the page and in its flow, are painted over whole, as its outline leaves them out.
# Through all it does before the user first looks at its card, the user's Space, tab and focused
# field stay as the user left them, and the user's next keys go into that field (decision 8).

log="$out/mcp.log"
step=0
rm -f "$saved" "$launch_frame"
# The throwaway login the run saves, on localhost, goes once it ends, however it ends, and one an
# earlier run left goes before it starts: the import run counts the logins Settings lists. The
# app has stopped by then, as Chrome keeps the file open while it runs.
forget_login() {
  python3 -I - "$profile_dir/Default/Login Data" << 'PY'
import os, sqlite3, sys
if os.path.exists(sys.argv[1]):
    db = sqlite3.connect(sys.argv[1])
    db.execute("delete from logins where username_value = 'check@example.test'"
               " and signon_realm like 'http://localhost:%'")
    db.commit()
PY
}
forget_login
trap 'cleanup; forget_login' EXIT
agents_file="$(dirname "$saved")/Agents"
token=$(python3 -I -c 'import secrets; print(secrets.token_hex(32))')
printf '{"on":true,"agents":[{"client":"mcp-client","token":"%s"}]}' "$token" > "$agents_file"
chmod 600 "$agents_file"
# An extension of the check's own, whose content script puts a frame of its page in the frames
# page, as a password manager puts its own in pages. The frame tells the page it's there. In the
# page with fields it puts two frames showing red text, one over the page and one in its flow.
extension="$PWD/$out/frame-extension"
mkdir -p "$extension"
cat > "$extension/manifest.json" << 'EOF'
{
  "manifest_version": 3,
  "name": "Frame check",
  "version": "1",
  "content_scripts": [
    {"matches": ["http://localhost/frames.html*"], "js": ["content.js"]},
    {"matches": ["http://localhost/withheld.html*"], "js": ["withheld.js"]},
    {"matches": ["http://localhost/covered.html*"], "js": ["covered.js"]},
    {"matches": ["http://127.0.0.1/covered-inner.html*"], "js": ["covered-inner.js"], "all_frames": true}
  ],
  "web_accessible_resources": [
    {"resources": ["frame.html", "shown.html"], "matches": ["http://localhost/*", "http://127.0.0.1/*"]}
  ]
}
EOF
printf '%s' 'const frame = document.createElement("iframe");
frame.src = chrome.runtime.getURL("frame.html");
document.body.append(frame);' > "$extension/content.js"
printf '%s' '<p>Extension text</p><script src=frame.js></script>' > "$extension/frame.html"
printf '%s' 'parent.postMessage("extension frame", "*");' > "$extension/frame.js"
# On the covered page it adds a button of its own, a frame of its page over the page's button, as a
# password manager's menu is, and key and input listeners of its own that mark the page when they
# hear one. In the other site's frame there, it covers the frame's page with a frame of its own.
printf '%s' 'const button = document.createElement("button");
button.textContent = "Extension button";
document.body.append(button);
const frame = document.createElement("iframe");
frame.src = chrome.runtime.getURL("frame.html");
frame.style = "position: absolute; left: 0; top: 0; width: 300px; height: 120px; border: 0";
document.body.append(frame);
for (const type of ["keydown", "keypress", "beforeinput", "input"]) {
  document.addEventListener(type, () => document.documentElement.dataset.heard = "1", true);
}' > "$extension/covered.js"
printf '%s' 'const frame = document.createElement("iframe");
frame.src = chrome.runtime.getURL("frame.html");
frame.style = "position: fixed; left: 0; top: 0; width: 100%; height: 100%; border: 0";
document.body.append(frame);' > "$extension/covered-inner.js"
printf '%s' 'for (const style of ["position: fixed; left: 40vw; top: 76vh", "display: block; margin-left: 2vw"]) {
  const frame = document.createElement("iframe");
  frame.src = chrome.runtime.getURL("shown.html");
  frame.style = style + "; width: 25vw; height: 15vh; border: 0";
  document.body.append(frame);
}
const changed = document.querySelector("[aria-label=\"Extension changed\"]");
addEventListener("message", event => {
  if (event.data != "extension, change") return;
  changed.value += "█";
  postMessage("extension changed", "*");
});
for (const label of ["Copied", "Copied note"]) {
  const field = document.querySelector("[aria-label=\"" + label + "\"]");
  field.value = "██████";
  field.dispatchEvent(new Event("input", { bubbles: true }));
}' > "$extension/withheld.js"
printf '%s' '<!doctype html><meta charset=utf-8><style>
body { margin: 0; background: #fff; color: #f00; font: 60vh/1 monospace;
  -webkit-text-stroke: 0.1em #f00; overflow: hidden }
</style><p style="margin: 20vh 5vw">██████</p><script src=shown.js></script>' > "$extension/shown.html"
printf '%s' 'requestAnimationFrame(() => requestAnimationFrame(() =>
  parent.postMessage("extension painted", "*")));' > "$extension/shown.js"
# On Linux the app's notifications go to the desktop's server: check/notify-server.py, on a session
# bus of the run's own, takes them and clicks them, as a desktop's does. Without dbus-daemon and
# python3-gi, the run says it skips them.
notes=
notify_buses=
if [ "$(uname)" = Linux ] && command -v dbus-daemon > /dev/null &&
  python3 -I -c 'from gi.repository import Gio' 2> /dev/null; then
  started=$(dbus-daemon --session --fork --print-address=1 --print-pid=1 2> /dev/null | tr '\n' ' ')
  notify_bus=$(echo "$started" | cut -d' ' -f1)
  notify_buses=$(echo "$started" | cut -d' ' -f2)
  [ -n "$notify_bus" ] || fail "dbus-daemon didn't start the run's session bus"
  notes="$out/notify-server.log"
  rm -f "$out/notify-in"
  mkfifo "$out/notify-in"
  DBUS_SESSION_BUS_ADDRESS=$notify_bus python3 -I check/notify-server.py < "$out/notify-in" > "$notes" 2>&1 &
  notify_buses="$notify_buses $!"
  exec 6> "$out/notify-in"
  for _ in $(seq 50); do
    grep -q '^ready$' "$notes" && break
    sleep 0.1
  done
  grep -q '^ready$' "$notes" || fail "the notification server didn't take its name: $(cat "$notes")"
  export DBUS_SESSION_BUS_ADDRESS=$notify_bus
  trap 'cleanup; forget_login; kill $notify_buses 2> /dev/null' EXIT
fi
# With motion, as the run clicks Stop while a step's shimmer draws, which a system that reduces
# motion has none of, as GitHub's hosted Windows runner does (main.rs, set_reduce_motion).
# The user's own page: a field over all of it, which says when it takes the keyboard, loses it or is
# typed in, and names itself once it has drawn. A URL drops its line breaks, so a line that goes on
# from an attribute starts with a space.
yours='data:text/html,<title>-</title><textarea id=f style="position:fixed;inset:0;width:100vw;height:100vh"
 onfocus="document.title=`yours focused`" onblur="document.title=`yours left`"
 oninput="document.title=`yours typed `+f.value"></textarea>
<script>requestAnimationFrame(()=>requestAnimationFrame(()=>document.title="yours"))</script>'
# A host of the check's server with no https, where an agent's tab comes to Chrome's warning before
# http. The profile remembers a host that fell back to http, so each run names a new one, as
# history.sh's $plain.
serve
plain="mcp-plain-$(date +%s).example.com"
# The client's calls pause for the run's own work between them, as engine_has's git does for over a
# minute on Windows, so its task's quiet end after its last page waits an hour, past the run, with
# the spec's ask and handoff waits; the waits relaunch below sees the quiet end (#3652).
BROWSER_DRIVE=1 BROWSER_MOTION=full BROWSER_URL="$yours" \
  "$browser" --load-extension="$(app_path "$extension")" --agent-waits=300,900,3600 \
  --host-resolver-rules="MAP $plain 127.0.0.1:$port" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
user_tab=$(today | cut -d' ' -f1)
await "^browser: title $user_tab yours\$" || fail "the user's page didn't draw"
# The user clicks into their field before the agent starts; where Browser isn't the active app, the
# click reaches no field, and the check of their focus below says it skips.
send click $page_point
yours_step=
if [ "$active" = 1 ]; then
  expect "^browser: title $user_tab yours focused\$" "a click on the user's page didn't focus its field"
  yours_step=$step
  yours_key=$key
  yours_focus=$focus
  yours_space=$(space)
fi
# A socket only the user can open, or on Windows a named pipe only they can.
case "$(uname -s)" in
  MINGW* | MSYS*) on='^browser: agents server on \\\\\.\\pipe\\.*-agents-[0-9a-f]*$' ;;
  *) on='^browser: agents server on /.*/Agents\.sock$' ;;
esac
await "$on" || fail "the saved switch didn't start the server"
socket=$(grep '^browser: agents server on' "$log" | tail -1 | sed 's/^browser: agents server on //')
# Beside the executable, which on Windows is chrome.exe in the app's folder, run by a function.
if windows; then bridge="$app/mcp-bridge.exe"; else bridge="$(dirname "$browser")/mcp-bridge"; fi
await_log() { await "$1" || fail "$2"; }
# Waits for more lines matching $1 than the $2 there were.
await_more() {
  for _ in $(seq 300); do
    [ "$(grep -c "$1" "$log")" -gt "$2" ] && return 0
    kill -0 "$pid" 2>/dev/null || fail "the app exited"
    sleep 0.1
  done
  return 1
}
# Fails by name when what's on screen stopped following the log (#3507): the capture $2, taken
# once the app's line said it drew $4 at "x y width height" ($3, in the window's points), shows
# there what the capture $1, taken before it was asked for, did. In #3270's second sighting GPUI's
# output stopped reaching the screen while its log ran on a hundred steps, and the run went red only
# at the next Settings show, far from where it froze. Sets follows to the part of that place that
# changed, in percent.
follows_log() {
  send layout
  width=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
    sed -n 's/^browser: layout content \([0-9]*\)x.*/\1/p')
  [ -n "$width" ] || fail "the driver's layout named no content size: $(grep '^browser: layout' "$log" | tail -1)"
  for capture in "$1" "$2"; do
    if windows; then
      to_bmp "$capture" "$capture.bmp" || fail "PowerShell couldn't read $capture"
    elif [ "$(uname)" = Linux ]; then
      magick "$capture" -type TrueColor BMP3:"$capture.bmp" || fail "ImageMagick couldn't read $capture"
    else
      sips -s format bmp "$capture" --out "$capture.bmp" > /dev/null || fail "sips couldn't read $capture"
    fi
  done
  # Of every other pixel 8 points inside the place, the share that differs by more than 32 in a
  # channel. The pictures are in pixels and the place in points, so the window's width scales it.
  follows=$(python3 -I - "$1.bmp" "$2.bmp" "$width" $3 << 'PY'
import struct, sys
def read(path):
    data = open(path, "rb").read()
    offset, = struct.unpack_from("<I", data, 10)
    width, height, _, bits = struct.unpack_from("<iiHH", data, 18)
    return data, offset, width, height, bits // 8, (bits * width + 31) // 32 * 4
before, after = read(sys.argv[1]), read(sys.argv[2])
if before[2:4] != after[2:4]:
    print(100)
    sys.exit()
_, _, width, height, size, stride = after
scale = width / float(sys.argv[3])
x, y, w, h = (float(n) for n in sys.argv[4:8])
left, top = int((x + 8) * scale), int((y + 8) * scale)
right, bottom = min(width, int((x + w - 8) * scale)), min(abs(height), int((y + h - 8) * scale))
changed = total = 0
for row in range(top, bottom, 2):
    line = row if height < 0 else abs(height) - 1 - row
    for column in range(left, right, 2):
        at = line * stride + column * size
        a = before[0][before[1] + at:][:3]
        b = after[0][after[1] + at:][:3]
        total += 1
        changed += max(abs(a[i] - b[i]) for i in range(3)) > 32
print(changed * 100 // max(total, 1))
PY
  ) || fail "the captures $1 and $2 couldn't be compared"
  rm -f "$1.bmp" "$2.bmp"
  [ "$follows" -ge 5 ] ||
    fail "what's on screen stopped following the log: the app drew $4 at $3, but the capture after it shows that place as it was before it was asked for, $follows% changed ($1, $2), so the window's frames stopped reaching the screen (#3270). The log's last lines: $(grep '^browser: ' "$log" | tail -3 | tr '\n' ';')"
}

case "$socket" in
  '\\.\pipe\'*) ;;
  *)
    [ -S "$socket" ] || fail "there's no socket at $socket"
    # GNU stat on Linux, BSD stat on the Mac.
    mode=$(stat -c %a "$socket" 2>/dev/null || stat -f %Lp "$socket")
    [ "$mode" = 600 ] || fail "the socket is $mode, not the user's alone"
    ;;
esac
[ -x "$bridge" ] || fail "the app has no bridge beside its executable"
if BROWSER_AGENT_TOKEN=0 "$bridge" "$socket" < /dev/null > /dev/null 2> "$out/mcp-refused"; then
  fail "the bridge got in without the agent's token"
fi
grep -q 'Settings › Agents' "$out/mcp-refused" || fail "a refused bridge said: $(cat "$out/mcp-refused")"
# A copy from the Store names its bridge to clients by the alias its MSIX declares, which Windows
# keeps in the user's WindowsApps folder across updates, as the package's own folder is named for
# its version (#1442). check-windows.yml installs an MSIX of this build and names it in CHECK_MSIX.
# A client starts the alias as Claude Code and Codex start a command, and it carries the client's
# hello to this browser's pipe and the answer back.
msix=
if windows && [ -n "${CHECK_MSIX:-}" ]; then
  alias=$(cygpath -w "$(cygpath -u "$LOCALAPPDATA")/Microsoft/WindowsApps/$(cat branding/name)-mcp-bridge.exe")
  python3 -I - "$alias" "$socket" "$token" > "$out/mcp-alias.log" 2>&1 << 'PY' ||
import os, subprocess, sys
alias, socket, token = sys.argv[1:]
hello = (b'{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18",'
         b'"capabilities":{},"clientInfo":{"name":"msix","version":"1"}}}\n')
run = subprocess.run([alias, socket], input=hello, capture_output=True, timeout=30,
                     env={**os.environ, "BROWSER_AGENT_TOKEN": token})
print(run.stdout.decode(errors="replace"), run.stderr.decode(errors="replace"))
sys.exit(b'"protocolVersion":"2025-06-18"' not in run.stdout)
PY
    fail "the MSIX's bridge, by its alias, didn't answer: $(tr '\n' ' ' < "$out/mcp-alias.log")"
  await_log '^browser: agent mcp-client hello from msix$' "the app didn't hear the hello the MSIX's bridge carried"
  msix=1
fi

# The client: the bridge on two pipes, which stay open until the session ends, its stderr added to
# mcp-bridge.log.
bridge_on() {
  rm -f "$out/mcp-in" "$out/mcp-out"
  mkfifo "$out/mcp-in" "$out/mcp-out"
  if windows; then
    BROWSER_AGENT_TOKEN=$token windows_bridge "$bridge" "$socket" "$out/mcp-in" "$out/mcp-out" \
      2>> "$out/mcp-bridge.log" &
  else
    BROWSER_AGENT_TOKEN=$token "$bridge" "$socket" < "$out/mcp-in" > "$out/mcp-out" \
      2>> "$out/mcp-bridge.log" &
  fi
  bridge_pid=$!
  exec 4> "$out/mcp-in" 5< "$out/mcp-out"
}
rm -f "$out/mcp-bridge.log"
bridge_on
# Prints the bridge's next line within $1 seconds, 30 if not given, and fails after. sh has no read
# -t on Linux, where it's dash, and the Mac has no timeout, so perl reads the line under its own
# alarm, a byte at a time to take that line alone. An alarm doesn't outlive an exec under Git Bash,
# whose exec starts a new process, so perl reads it itself rather than a shell it execs (#3286).
next_answer() {
  perl -e 'alarm shift; my $line = "";
    while (sysread STDIN, my $byte, 1) { $line .= $byte; last if $byte eq "\n" }
    print $line; exit($line =~ /\n\z/ ? 0 : 1)' "${1:-30}" <&5
}
# Sends the JSON-RPC message $1 and, for a request, prints its answer, within 30 s.
id=0
mcp() {
  printf '%s\n' "$1" >&4
  case "$1" in *'"id":'*) answer=$(next_answer) || fail "the browser didn't answer $1"; printf '%s\n' "$answer" ;; esac
}
# Prints the answer on stdin to the call $1: its text, after "error: " if it's an error, or a
# picture's type and size. An answer with no result, as a JSON-RPC error, fails the run with it.
answered() {
  raw=$(cat)
  printf '%s\n' "$raw" | python3 -I -c '
import json, sys
result = json.load(sys.stdin)["result"]
content = result["content"][0]
if content["type"] == "image":
    print(content["mimeType"], len(content["data"]))
else:
    print(("error: " if result["isError"] else "") + content["text"])' 2> /dev/null ||
    fail "the browser answered $1 with: $raw"
}
# The tools/call request for the tool $1 with the arguments $2, as they are: sh's echo would make
# a JSON escape such as \n a line break.
request() {
  id=$((id + 1))
  printf '%s\n' "{\"jsonrpc\":\"2.0\",\"id\":$id,\"method\":\"tools/call\",\"params\":{\"name\":\"$1\",\"arguments\":$2}}"
}
# Calls the tool $1 with the arguments $2, and prints its answer.
call() {
  mcp "$(request "$1" "$2")" | answered "$1"
}

mcp '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}' \
  > "$out/mcp-initialize.json"
grep -q '"protocolVersion":"2025-06-18"' "$out/mcp-initialize.json" ||
  fail "initialize didn't answer in the client's version"
await_log '^browser: agent mcp-client hello from check$' "the app didn't hear the client's hello"
mcp '{"jsonrpc":"2.0","method":"notifications/initialized"}'
mcp '{"jsonrpc":"2.0","id":100,"method":"tools/list"}' > "$out/mcp-tools.json"
grep -q '"name":"read_page"' "$out/mcp-tools.json" || fail "tools/list didn't list read_page"
# A Boost is the user's alone (decision 8), so no tool reads, writes or turns one on.
grep -qi 'boost' "$out/mcp-tools.json" && fail "tools/list named a Boost among the agent's tools: $(grep -o '"name":"[^"]*"' "$out/mcp-tools.json" | tr '\n' ' ')"

listed=$(call list_tabs '{}')
await_log '^browser: made space [1-9][0-9]* for agent mcp-client$' "list_tabs didn't make the agent's Space"
agent=$(grep '^browser: made space' "$log" | tail -1 | cut -d' ' -f4)
# Its Space is the agent's alone, which no window lists (#1080), and list_tabs alone starts no card.
await_log "^browser: task space $agent MCP client\$" "the window didn't keep the agent's Space apart"
grep -q "^browser: added space $agent " "$log" && fail "the agent's Space reached the switcher"
grep -q '^browser: session ' "$log" && fail "list_tabs alone started the client's card"
case "$listed" in
  "You have no tabs yet."*) ;;
  *) fail "list_tabs in a new Space said: $listed" ;;
esac
printf '%s\n' "$listed" | grep -q "^. $user_tab  " && fail "list_tabs listed the user's tab"

refused=$(call read_page "{\"tab\":$user_tab}")
[ "$refused" = "error: The tab isn't in the agent's own Space." ] ||
  fail "reading the user's tab answered: $refused"
await_log "^browser: agent activity mcp-client $user_tab read_page refused: The tab isn't in the agent's own Space\.\$" \
  "the refused read didn't reach the activity log"
grep -q "^browser: tab text $user_tab " "$log" && fail "the core read the user's tab"

# A link to an app, as a Word document's in SharePoint (ms-word:), opens nothing outside the
# browser and asks nothing: the tab stays on its page, which the agent can go on using, and the
# click's answer names the scheme (#1443).
serve
printf '%s' '<!doctype html><title>Documents</title>
<a href="ms-word:ofe|u|http://localhost/report.docx">Report</a>' > "$out/site/documents.html"
opened=$(call open_tab "{\"url\":\"localhost:$port/documents.html\"}")
documents=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
ref=$(printf '%s\n' "$opened" | sed -n 's/.*- link "Report" \[\(e[0-9]*\)\].*/\1/p' | head -1)
clicked=$(call click "{\"tab\":$documents,\"ref\":\"$ref\"}")
case "$clicked" in
  "error: That link opens "*"(ms-word:), outside the browser"*"The tab stayed on “Documents”."*) ;;
  *) fail "clicking a link to an app answered: $clicked" ;;
esac
listed=$(call list_tabs '{}')
printf '%s\n' "$listed" | grep -q "^. $documents  Documents  .*/documents\.html\$" ||
  fail "the link to an app took the tab off its page: $listed"
grep -q "^browser: page $documents dialog [^n]" "$log" && fail "the link to an app asked to open it"
read=$(call read_page "{\"tab\":$documents}")
case "$read" in
  "Tab $documents: Documents"*) ;;
  *) fail "reading the tab a link to an app left answered: $read" ;;
esac
closed=$(call close_tab "{\"tab\":$documents}")
case "$closed" in "Closed tab $documents."*) ;; *) fail "closing the documents tab answered: $closed" ;; esac

# Every answer is 12,000 characters at most, on pages far bigger than that (#3657): an article,
# which navigate answers with its text when asked and read_page reads in parts; a table of 120
# orders, its rows a line each with their refs, which a click that draws it again answers with a
# line; and a JSON response, which gives its start. Each answer's size goes in answer-sizes.txt,
# all of them before any is checked, for the same pages on another build.
chars() { python3 -I -c 'import sys; print(len(sys.stdin.buffer.read().decode()))'; }
sized() { printf '%s %s\n' "$1" "$(printf '%s' "$2" | chars)" | tee -a "$out/answer-sizes.txt"; }
python3 -I - "$out/site" << 'EOF'
import json, sys
site = sys.argv[1]
words = "The river rose in the night and by morning the low road was gone under it. "
with open(f"{site}/article.html", "w") as page:
    page.write("<!doctype html><title>River</title><h1>River</h1>")
    page.write("".join(f"<p>{n}. {words * 9}</p>" for n in range(60)))
rows = "".join(
    f"<tr><td>Order {n}</td><td>Paid</td><td><a href='#{n}'>Refund</a></td></tr>"
    for n in range(120)
)
with open(f"{site}/orders.html", "w") as page:
    page.write(
        "<!doctype html><title>Orders</title><button onclick='redraw()'>Sort</button>"
        f"<table><caption>Orders</caption><tbody id=rows>{rows}</tbody></table>"
        "<script>function redraw() { const rows = document.getElementById('rows');"
        " rows.innerHTML = rows.innerHTML; document.title = 'Sorted' }</script>"
    )
with open(f"{site}/orders.json", "w") as data:
    json.dump({"orders": [{"id": n, "state": "paid", "lines": [1, 2, 3]} for n in range(4000)]}, data)
EOF
rm -f "$out/answer-sizes.txt"
article_opened=$(call open_tab "{\"url\":\"localhost:$port/article.html\"}")
sized open_tab_article "$article_opened"
article=$(printf '%s\n' "$article_opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
article_read=$(call read_page "{\"tab\":$article}")
sized read_page_article "$article_read"
article_text=$(call navigate "{\"tab\":$article,\"url\":\"localhost:$port/article.html?again\",\"answer\":\"text\"}")
sized navigate_article_text "$article_text"
orders_opened=$(call open_tab "{\"url\":\"localhost:$port/orders.html\"}")
sized open_tab_orders "$orders_opened"
orders=$(printf '%s\n' "$orders_opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
sort_ref=$(printf '%s\n' "$orders_opened" | sed -n 's/.*- button "Sort" \[\(e[0-9]*\)\].*/\1/p' | head -1)
orders_sorted=$(call click "{\"tab\":$orders,\"ref\":\"$sort_ref\"}")
sized click_orders_redrawn "$orders_sorted"
json_opened=$(call open_tab "{\"url\":\"localhost:$port/orders.json\"}")
sized open_tab_json "$json_opened"
json=$(printf '%s\n' "$json_opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
while read -r answer size; do
  [ "$size" -le 12000 ] || fail "$answer answered $size characters, over the 12,000 an answer"
done < "$out/answer-sizes.txt"
case "$article_read" in
  *"[The page goes on: read_page with from 10000 reads the next part.]") ;;
  *) fail "read_page of a long article didn't say how to read on: $(printf '%s' "$article_read" | tail -c 300)" ;;
esac
article_next=$(call read_page "{\"tab\":$article,\"from\":10000}")
case "$article_next" in
  # Its 10,000th character is in the 15th paragraph, each about 700.
  *"15. The river rose"*) ;;
  *) fail "read_page from 10000 didn't go on with the article: $(printf '%s' "$article_next" | head -c 300)" ;;
esac
case "$article_text" in
  *"[e"[0-9]*) fail "navigate with the text answer gave refs: $(printf '%s' "$article_text" | head -c 300)" ;;
  *"0. The river rose"*) ;;
  *) fail "navigate with the text answer didn't give the article's text: $(printf '%s' "$article_text" | head -c 300)" ;;
esac
printf '%s\n' "$orders_opened" | grep -q '^ *- row \[e[0-9]*\]: "Order 119" \[e[0-9]*\] · "Paid" \[e[0-9]*\] · link "Refund" \[e[0-9]*\]$' ||
  fail "the table's last order wasn't a line with its refs: $(printf '%s\n' "$orders_opened" | tail -3)"
case "$orders_sorted" in
  *"Most of the outline's elements are new, though it reads much as before"*) ;;
  *) fail "a click that drew the table again answered: $(printf '%s' "$orders_sorted" | head -c 300)" ;;
esac
case "$json_opened" in
  *'- text: {"orders": [{"id": 0,'*"This tab shows a response"*) ;;
  *) fail "a JSON response's tab answered: $(printf '%s' "$json_opened" | head -c 300)" ;;
esac
for tab in "$article" "$orders" "$json"; do
  closed=$(call close_tab "{\"tab\":$tab}")
  case "$closed" in "Closed tab $tab."*) ;; *) fail "closing tab $tab answered: $closed" ;; esac
done

# A tab an agent's tab opens follows the agent's rules from when it's made, as the agent's own tab
# does, whether a link, a page's window.open or a redirect in it leads there (#2660).
if engine_has ../core/space.cc ../core/window.cc ../core/tab.cc ../core/main_delegate.cc; then
  # The tab open_tab made refused the link to an app above where the core decides, as it's the
  # agent's from its first request: open_tab makes it so before its page starts.
  grep -q "browser: agent tab $documents refused ms-word\$" "$log" ||
    fail "the agent's own tab didn't refuse the link to an app; its last agent tab line:
$(grep 'browser: agent tab ' "$log" | tail -1)"
  # window.open by name: a handler's bare open() is the document's, which with one argument
  # replaces the page and opens nothing.
  printf '%s' '<!doctype html><title>Opens</title>
<a href="maps:?q=check" target="_blank">Link</a>
<a href="bounce?to=maps%3A%3Fq%3Dcheck" target="_blank">Redirect</a>
<button onclick="window.open(&quot;maps:?q=check&quot;)">Pop up</button>' > "$out/site/opens.html"
  opened=$(call open_tab "{\"url\":\"localhost:$port/opens.html\"}")
  opens=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  # The core's line carries Chromium's prefix.
  kept='browser: agent tab [0-9]* refused maps$'
  for target in 'link "Link"' 'link "Redirect"' 'button "Pop up"'; do
    ref=$(printf '%s\n' "$opened" | sed -n "s/.*- $target \[\(e[0-9]*\)\].*/\1/p" | head -1)
    before=$(grep -c "$kept" "$log" || true)
    call click "{\"tab\":$opens,\"ref\":\"$ref\"}" > /dev/null
    await_more "$kept" "$before" || fail "the tab the agent's $target opened didn't follow the agent's rules"
    kept_tab=$(grep "$kept" "$log" | tail -1 | sed 's/.*browser: agent tab \([0-9]*\) .*/\1/')
    [ "$kept_tab" != "$opens" ] || fail "the agent's $target was kept in its own tab, not one it opened"
    grep -q "^browser: page $kept_tab dialog [^n]" "$log" &&
      fail "the tab the agent's $target opened asked the user: $(grep "^browser: page $kept_tab dialog" "$log" | tail -1)"
    if call list_tabs '{}' | grep -q "^. $kept_tab  "; then
      closed=$(call close_tab "{\"tab\":$kept_tab}")
      case "$closed" in "Closed tab $kept_tab."*) ;; *) fail "closing tab $kept_tab answered: $closed" ;; esac
    fi
  done
  closed=$(call close_tab "{\"tab\":$opens}")
  case "$closed" in "Closed tab $opens."*) ;; *) fail "closing tab $opens answered: $closed" ;; esac
  opened_said=", each following its rules from when it's made"
else
  opened_said=""
  echo "skip: mcp, a tab an agent's tab opens. This engine is from before such a tab was the agent's from when it's made (#2660)."
fi

# Ten cases of a web toolkit, each the same form on a page of its own, which an agent moves to
# Closed in tabs the user isn't looking at, each step on all ten in one turn: cases.py, on the
# client's own connection, so they're steps of its card. The server answers a turn's calls at once
# (#1476). The form says Ready once
# it has painted (painted.js, site.sh). Its status list opens on the next frame, as an animated
# one does, which a tab drawing no frames never shows, and closes once focus leaves its button for
# anything but the list, as the page losing focus takes it. Save saves on a timer, as a request
# would, which a hidden page may throttle.
serve
printf '%s' '<script src="painted.js"></script>
<h1 id=heading>Case</h1>
<p>Status <button id=pick aria-haspopup=listbox aria-expanded=false>Open</button></p>
<ul id=list role=listbox aria-label=Status hidden>
<li role=option tabindex=-1>Open<li role=option tabindex=-1>Pending<li role=option tabindex=-1>Closed
</ul>
<p><button id=save>Save</button> <span id=state></span></p>
<script>
const n = new URLSearchParams(location.search).get("n");
heading.textContent = "Case " + n;
const show = open => { list.hidden = !open; pick.ariaExpanded = open; };
pick.onclick = () => requestAnimationFrame(() => show(list.hidden));
pick.onblur = event => list.contains(event.relatedTarget) || show(false);
list.onclick = event => { pick.textContent = event.target.textContent; show(false); };
save.onclick = () => setTimeout(() => {
  state.textContent = "Saved " + pick.textContent;
  document.title = "case " + n + " saved " + pick.textContent;
}, 300);
painted(() => { state.textContent = "Ready"; document.title = "case " + n; });
</script>' > "$out/site/case.html"
# Over pipes, as Python on Windows can't use a Git Bash fifo's descriptor as its stdin or stdout:
# perl hands it the connection's answers, a byte at a time so as to take only its own, until the
# answer to its closing ping, and cat sends on what it writes. The pipeline's status is cat's, so
# its last line, there once the cases and their tabs' closing are done, says how it went.
perl -e '$SIG{PIPE} = "IGNORE"; $| = 1; alarm 300; my $line = "";
  while (sysread STDIN, my $byte, 1) {
    $line .= $byte;
    next unless $byte eq "\n";
    exit 0 if $line =~ /"id":"cases-done"/;
    print $line;
    $line = "";
  }
  exit 1' <&5 |
  python3 -I check/cases.py "localhost:$port/case.html" 2> "$out/cases.log" | cat >&4
grep -q '^cases: 10 cases in ' "$out/cases.log" || fail "$(tail -1 "$out/cases.log")"
await_log '^browser: agent mcp-client answered 10 calls at once$' \
  "the server answered a turn's ten calls one after another"
for n in $(seq 10); do
  grep -q "^browser: title [0-9]* case $n saved Closed\$" "$log" || fail "case $n wasn't saved Closed"
done
cases=$(sed -n 's/^cases: \(10 cases in .*\)/\1/p' "$out/cases.log")

# An action waits for its element to take input and for its page to settle (#1436). The Load
# button is enabled 4 s after the page shows, as a form's once its data is in: after open_tab's
# answer, which waits for the page to settle and took 2.5 s on Windows' runner (#2079), and well
# within the 5 s the click waits for it. Its click fetches /slow, which takes a second, then says
# what came: the click's answer has it, with no wait step. Stall's fetch never comes, as a poll's
# doesn't, while the page holds still: its click answers after 5 s saying so, not that the page may
# change yet (#2355).
printf '%s' '<script src="painted.js"></script>
<h1>Settle</h1>
<p><button id=load disabled>Load</button> <button id=stall>Stall</button> <span id=state></span></p>
<script>
painted(() => setTimeout(() => { load.disabled = false; document.title = "settle enabled"; }, 4000));
load.onclick = () => fetch("slow").then(answer => answer.text()).then(text => {
  state.textContent = "Came " + text;
});
stall.onclick = () => fetch("wait");
</script>' > "$out/site/settle.html"
ref_in() { printf '%s\n' "$1" | sed -n "s/.*- $2 \"$3\" \[\(e[0-9]*\)\].*/\1/p" | head -1; }
opened=$(call open_tab "{\"url\":\"localhost:$port/settle.html\"}")
settle_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
load=$(ref_in "$opened" button Load)
stall=$(ref_in "$opened" button Stall)
[ -n "$settle_tab" ] && [ -n "$load" ] && [ -n "$stall" ] || fail "open_tab of the settle page answered: $opened"
grep -q "^browser: title $settle_tab settle enabled\$" "$log" &&
  fail "the settle page enabled Load before open_tab answered, which leaves nothing to wait for"
started=$(date +%s)
clicked=$(call click "{\"tab\":$settle_tab,\"ref\":\"$load\"}")
case "$clicked" in
  *"still busy"*) fail "the click on Load answered busy: $clicked" ;;
  "Clicked button “Load”."*"Came slow"*) ;;
  *) fail "the click on Load didn't wait for it to be enabled and its fetch to come: $clicked" ;;
esac
settled=$(call run_steps "{\"tab\":$settle_tab,\"steps\":[{\"action\":\"wait\",\"until\":\"quiet\"}]}")
case "$settled" in
  "Ran 1 steps."*"1. The page settled."*) ;;
  *) fail "a wait until quiet on a page at rest answered: $settled" ;;
esac
stalled=$(call click "{\"tab\":$settle_tab,\"ref\":\"$stall\"}")
case "$stalled" in
  "Clicked button “Stall”. The page held still, with requests it made still open after 5 s."*) ;;
  *) fail "the click on Stall, whose fetch never comes, answered: $stalled" ;;
esac
case "$stalled" in *"may change yet"*) fail "the click on Stall said the page may change yet" ;; esac
settle="the settle page's click waited $(($(date +%s) - started)) s in all"
closed=$(call close_tab "{\"tab\":$settle_tab}")
case "$closed" in "Closed tab $settle_tab."*) ;; *) fail "closing the settle page answered: $closed" ;; esac

# An app that loads in a frame (#3658): the page's own document is done at once, with a splash,
# and the app is a frame of another site's that shows a progress bar until its content comes after
# its load, by a fetch or by its own script alone. open_tab and navigate answer with the content,
# or say the page was still busy, and never with the splash alone: the fetch is heard from the
# page's commit on, and a frame that shows only a progress bar is a splash. Each answer's time is in
# framed.txt.
printf '%s' "<!doctype html><title>Framed</title><p id=splash>Loading Mail</p>
<iframe title=App></iframe>
<script>
document.querySelector('iframe').src = 'http://127.0.0.1:$port/framed-app.html' + location.search;
addEventListener('message', () => splash.remove());
</script>" > "$out/site/framed.html"
printf '%s' '<!doctype html><title>App</title><div id=app><div role=progressbar aria-label=Loading></div></div>
<script>
const show = () => {
  app.innerHTML = "<h1>Inbox</h1><button>Compose</button>";
  parent.postMessage("ready", "*");
};
if (location.search === "?by=late") app.innerHTML = "<p>Starting</p>";
addEventListener("load", () => location.search === "?by=fetch" ? fetch("slow").then(show)
  : setTimeout(show, location.search === "?by=late" ? 6000 : 1500));
</script>' > "$out/site/framed-app.html"
now_ms() { perl -MTime::HiRes=time -e 'printf "%d\n", time * 1000'; }
framed_tab=
framed_wrong=
for how in "open_tab fetch" "navigate timer" "navigate timer#inbox" "navigate fetch"; do
  set -- $how
  started=$(now_ms)
  case $1 in
    open_tab) answer=$(call open_tab "{\"url\":\"localhost:$port/framed.html?by=$2\"}") ;;
    *) answer=$(call navigate "{\"tab\":$framed_tab,\"url\":\"localhost:$port/framed.html?by=$2\"}") ;;
  esac
  took=$(($(now_ms) - started))
  [ -n "$framed_tab" ] ||
    framed_tab=$(printf '%s\n' "$answer" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  [ -n "$framed_tab" ] || fail "open_tab of the framed app answered: $answer"
  case "$answer" in
    *Inbox*) said=content ;;
    *"still busy"*) said=busy ;;
    *Loading*) said=splash ;;
    *) said=else ;;
  esac
  printf '%s\n' "$1 by $2: $took ms, $said, ${#answer} characters" >> "$out/framed.txt"
  printf '%s\n' "$answer" > "$out/framed-$1-$(printf '%s' "$2" | tr -c 'a-z\n' -).txt"
  case $said in content | busy) ;; *) framed_wrong="$framed_wrong; $1 by $2: $said in $took ms" ;; esac
done
framed="$(paste -sd, - < "$out/framed.txt" | sed 's/,/, /g')"
[ -z "$framed_wrong" ] ||
  fail "the framed app's answers had neither its content nor that it was busy${framed_wrong}: all were $framed"
# One whose content comes after the page has settled, past the 5 s an answer waits, with nothing
# that says it's loading, is answered once wait_for's text shows.
started=$(now_ms)
late=$(call navigate "{\"tab\":$framed_tab,\"url\":\"localhost:$port/framed.html?by=late\",\"wait_for\":\"Inbox\",\"seconds\":15}")
took=$(($(now_ms) - started))
printf '%s\n' "$late" > "$out/framed-navigate-late.txt"
case "$late" in
  "“Inbox” showed."*Compose*) ;;
  *) fail "navigate waiting for the late app's Inbox answered in $took ms: $late" ;;
esac
framed="$framed, navigate waiting for Inbox in $took ms"
closed=$(call close_tab "{\"tab\":$framed_tab}")
case "$closed" in "Closed tab $framed_tab."*) ;; *) fail "closing the framed app answered: $closed" ;; esac

# A page whose first load hasn't committed once an action has waited 15 s for it (kLoadTimeout in
# agent_page.cc) is still loading, which the answer says, naming the tab it opened, and not a page
# agents may not use (#1461). /wait never comes.
waited=$(call open_tab "{\"url\":\"localhost:$port/wait\"}")
case "$waited" in
  "error: Opened tab "*". localhost is still loading after 15 s. Wait, or try again.") ;;
  *) fail "open_tab of a page still loading answered: $waited" ;;
esac
waiting_tab=$(printf '%s\n' "$waited" | sed -n '1s/^error: Opened tab \([0-9]*\)\..*/\1/p')
closed=$(call close_tab "{\"tab\":$waiting_tab}")
case "$closed" in "Closed tab $waiting_tab."*) ;; *) fail "closing the tab still loading answered: $closed" ;; esac

# A tab the agent's page opens, by a link to a new tab or a page's window.open, goes in its Space,
# behind, with the agent's tab as its opener, and the click's answer names it. The user's tab stays
# shown, and their Space gets none (#1470).
printf '%s' '<!doctype html><title>Opener</title><a href="opened.html" target="_blank">Open</a>
<button onclick="open(&quot;opened.html?popup&quot;, &quot;&quot;, &quot;popup&quot;)">Pop up</button>' \
  > "$out/site/opener.html"
printf '%s' '<!doctype html><title>Opened</title><p>Opened.</p>' > "$out/site/opened.html"
opened=$(call open_tab "{\"url\":\"localhost:$port/opener.html\"}")
opener=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
user_today=$(today)
for target in 'link "Open"' 'button "Pop up"'; do
  ref=$(printf '%s\n' "$opened" | sed -n "s/.*- $target \[\(e[0-9]*\)\].*/\1/p" | head -1)
  behind=$(grep -c "^browser: tab [0-9]* opened by $opener in the background\$" "$log" || true)
  clicked=$(call click "{\"tab\":$opener,\"ref\":\"$ref\"}")
  await_more "^browser: tab [0-9]* opened by $opener in the background\$" "$behind" ||
    fail "the agent's $target didn't open a tab behind, from its tab"
  new_tab=$(sed -n "s/^browser: tab \([0-9]*\) opened by $opener in the background\$/\1/p" "$log" | tail -1)
  case "$clicked" in
    "Clicked "*". It opened tab $new_tab among your tabs."*) ;;
    *) fail "the agent's click on its $target answered: $clicked" ;;
  esac
  call list_tabs '{}' | grep -q "^. $new_tab  " || fail "the tab its $target opened isn't in its Space"
  # Beside the agent's tab, it's as quiet as the agent's own (#1080).
  await_log "^browser: page $new_tab audible 0 muted 1\$" "the tab its $target opened has sound"
  grep -q "^browser: selected $new_tab\( in .*\)\{0,1\}\$" "$log" &&
    fail "the tab the agent's $target opened was shown"
  [ "$(today)" = "$user_today" ] || fail "the agent's $target changed the user's Today: $(today)"
  closed=$(call close_tab "{\"tab\":$new_tab}")
  case "$closed" in "Closed tab $new_tab."*) ;; *) fail "closing the opened tab answered: $closed" ;; esac
done
closed=$(call close_tab "{\"tab\":$opener}")
case "$closed" in "Closed tab $opener."*) ;; *) fail "closing the opener answered: $closed" ;; esac

# An agent's navigate starts its tab a new browsing group, so a site it left keeps no hold on the
# tab through a pop-up the site opened: the pop-up's message to its opener and its move of it go
# nowhere (#3595). Before that, while they're related, the same presses in the pop-up do both, which
# is what makes the step's green mean something. 127.0.0.1 is another site than localhost. The
# message and the move are two presses: Blink may hold a message posted in a press until it next
# draws, and a move in the same press can replace the opener's page first, which drops it.
printf '%s' '<!doctype html><title>Steer</title>
<button onclick="open(&quot;steers.html&quot;, &quot;&quot;, &quot;popup&quot;)">Pop up</button>
<script>addEventListener("message", event => document.title = "messaged " + event.data)</script>' \
  > "$out/site/steer.html"
printf '%s' '<!doctype html><title>Steers</title><script>let sends = 0, tries = 0</script>
<button onclick="sends++; try { opener.postMessage(&quot;steered&quot;, &quot;*&quot;) } catch (error) {}
 document.title = &quot;sent &quot; + sends">Message</button>
<button onclick="tries++; try { opener.location = &quot;/steered.html&quot; } catch (error) {}
 document.title = &quot;tried &quot; + tries">Steer</button>' \
  > "$out/site/steers.html"
printf '%s' '<!doctype html><title>Steered</title><p>Steered.</p>' > "$out/site/steered.html"
printf '%s' '<!doctype html><title>Left</title><p>The site the agent went to.</p>
<script>addEventListener("message", event => document.title = "messaged " + event.data)</script>' \
  > "$out/site/left.html"
opened=$(call open_tab "{\"url\":\"localhost:$port/steer.html\"}")
steer_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
[ -n "$steer_tab" ] || fail "open_tab steer.html answered: $opened"
ref=$(printf '%s\n' "$opened" | sed -n 's/.*- button "Pop up" \[\(e[0-9]*\)\].*/\1/p' | head -1)
behind=$(grep -c "^browser: tab [0-9]* opened by $steer_tab in the background\$" "$log" || true)
call click "{\"tab\":$steer_tab,\"ref\":\"$ref\"}" > /dev/null
await_more "^browser: tab [0-9]* opened by $steer_tab in the background\$" "$behind" ||
  fail "the steer page's Pop up didn't open a tab behind"
popup_tab=$(sed -n "s/^browser: tab \([0-9]*\) opened by $steer_tab in the background\$/\1/p" "$log" | tail -1)
await_log "^browser: title $popup_tab Steers\$" "the steer page's pop-up didn't load"
buttons=$(call snapshot "{\"tab\":$popup_tab}")
message=$(printf '%s\n' "$buttons" | sed -n 's/.*- button "Message" \[\(e[0-9]*\)\].*/\1/p' | head -1)
steer=$(printf '%s\n' "$buttons" | sed -n 's/.*- button "Steer" \[\(e[0-9]*\)\].*/\1/p' | head -1)
[ -n "$message" ] && [ -n "$steer" ] ||
  fail "the snapshot of the steer page's pop-up had no Message and Steer buttons: $buttons"
call click "{\"tab\":$popup_tab,\"ref\":\"$message\"}" > /dev/null
await_log "^browser: title $popup_tab sent 1\$" "the pop-up's Message didn't run"
await_log "^browser: title $steer_tab messaged steered\$" \
  "the pop-up's message didn't reach its opener while they were related, so the step can't see one"
call click "{\"tab\":$popup_tab,\"ref\":\"$steer\"}" > /dev/null
await_log "^browser: title $popup_tab tried 1\$" "the pop-up's Steer didn't run"
await_log "^browser: loaded $steer_tab http://localhost:$port/steered.html\$" \
  "the pop-up didn't move its opener while they were related, so the step can't see a move"
went=$(call navigate "{\"tab\":$steer_tab,\"url\":\"http://127.0.0.1:$port/left.html\"}")
case "$went" in "error: "*) fail "the agent's navigate to another site answered: $went" ;; esac
await_log "^browser: loaded $steer_tab http://127.0.0.1:$port/left.html\$" \
  "the agent's navigate to another site didn't load"
messaged=$(grep -c "^browser: title $steer_tab messaged steered\$" "$log" || true)
steered=$(grep -c 'steered\.html' "$out/site.log" || true)
loads=$(grep -c "^browser: loaded $steer_tab " "$log" || true)
call click "{\"tab\":$popup_tab,\"ref\":\"$message\"}" > /dev/null
await_log "^browser: title $popup_tab sent 2\$" "the pop-up's second Message didn't run"
call click "{\"tab\":$popup_tab,\"ref\":\"$steer\"}" > /dev/null
# A frame's messages reach the browser in order, so by the title the browser has had the message
# and the move. The move it let through would start at once, and the control's took well under a
# second, so 5 s is room to see one. A message let through isn't dropped by a move here, since the
# move is refused or fails the step itself.
await_log "^browser: title $popup_tab tried 2\$" "the pop-up's second Steer didn't run"
for _ in $(seq 50); do
  [ "$(grep -c "^browser: title $steer_tab messaged steered\$" "$log")" = "$messaged" ] ||
    fail "the pop-up of the site the agent left messaged the agent's tab"
  [ "$(grep -c 'steered\.html' "$out/site.log")" = "$steered" ] &&
    [ "$(grep -c "^browser: loaded $steer_tab " "$log")" = "$loads" ] ||
    fail "the pop-up of the site the agent left moved the agent's tab: $(grep "^browser: loaded $steer_tab " "$log" | tail -1)"
  sleep 0.1
done
for tab in $popup_tab $steer_tab; do
  closed=$(call close_tab "{\"tab\":$tab}")
  case "$closed" in "Closed tab $tab."*) ;; *) fail "closing the steer step's tab $tab answered: $closed" ;; esac
done

# The client's work is one card at the sidebar's foot, from its first call that acted on a tab, and
# the cases' calls on its connection were steps of the same card (#1080). Its name is the one the
# client gave.
sessions=$(grep -c '^browser: session session-[0-9]* of agent mcp-client as check$' "$log" || true)
[ "$sessions" = 1 ] || fail "the client's connection had $sessions cards, not one"
session=$(sed -n 's/^browser: session \(session-[0-9]*\) of agent mcp-client as check$/\1/p' "$log")
await_log "^browser: task plate task-${session#session-} at " "the client's card wasn't drawn"
# Unnamed, it's titled by the web site it first acted on, the user's page off the web naming
# none; named with title, it's the agent's words, and its line leads with the client's name, with
# no step count. A different name within a minute is refused and changes nothing (#3690).
await_log "^browser: task card $session titled localhost\$" \
  "the unnamed card wasn't titled by its site: $(grep "^browser: task card $session titled " "$log" | tail -1)"
named=$(call title '{"text":"Check the pages"}')
[ "$named" = "The task has its name." ] || fail "title answered: $named"
await_log "^browser: task card $session titled Check the pages\$" "the card didn't take the agent's title"
line=$(grep "^browser: task card $session [A-Z][a-z]* " "$log" | tail -1 | cut -d' ' -f6-)
case "$line" in
  "check · "*) ;;
  *) fail "the card's line didn't lead with the client's name: $line" ;;
esac
case "$line" in *"Step "*) fail "the card's line counted its steps: $line" ;; esac
renamed=$(call title '{"text":"Another name"}')
[ "$renamed" = "error: The task was named under a minute ago; name it again when you move on to another task." ] ||
  fail "a second title within a minute answered: $renamed"
grep -q "^browser: task card $session titled Another name\$" "$log" && fail "a second title within a minute took"
# A second connection of the client's, as a second terminal's, is a card of its own, named as it
# says, whose steps are its own: the first card's line stays as it was. Closing it ends its card
# alone (#3663).
first_line=$(grep "^browser: task card $session [A-Z][a-z]* " "$log" | tail -1)
rm -f "$out/mcp2-in" "$out/mcp2-out"
mkfifo "$out/mcp2-in" "$out/mcp2-out"
if windows; then
  BROWSER_AGENT_TOKEN=$token windows_bridge "$bridge" "$socket" "$out/mcp2-in" "$out/mcp2-out" \
    2>> "$out/mcp-bridge.log" &
else
  BROWSER_AGENT_TOKEN=$token "$bridge" "$socket" < "$out/mcp2-in" > "$out/mcp2-out" \
    2>> "$out/mcp-bridge.log" &
fi
second_pid=$!
# 8 and 9, as 6 and 7 are the notification server's and the other client's.
exec 8> "$out/mcp2-in" 9< "$out/mcp2-out"
# Sends $1 on the second connection and prints its answer, as mcp does on the first.
second() {
  printf '%s\n' "$1" >&8
  perl -e 'alarm 30; my $line = "";
    while (sysread STDIN, my $byte, 1) { $line .= $byte; last if $byte eq "\n" }
    print $line; exit($line =~ /\n\z/ ? 0 : 1)' <&9 || fail "the second connection didn't answer $1"
}
second '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"second","version":"1"}}}' > /dev/null
printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}' >&8
await_log '^browser: agent mcp-client hello from second$' "the app didn't hear the second connection's hello"
printf '%s' '<!doctype html><title>Second page</title><p>A page of the second connection.' \
  > "$out/site/second.html"
opened=$(second "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"open_tab\",\"arguments\":{\"url\":\"localhost:$port/second.html\"}}}" | answered open_tab)
second_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
[ -n "$second_tab" ] || fail "open_tab on the second connection answered: $opened"
await_log '^browser: session session-[0-9]* of agent mcp-client as second$' \
  "the client's second connection had no card of its own: $(grep -c '^browser: session ' "$log") cards"
second_session=$(sed -n 's/^browser: session \(session-[0-9]*\) of agent mcp-client as second$/\1/p' "$log")
await_log "^browser: task card $second_session [A-Z][a-z]* second · " \
  "the second connection's card didn't lead with its own name: $(grep "^browser: task card $second_session " "$log" | tail -1)"
[ "$(grep "^browser: task card $session [A-Z][a-z]* " "$log" | tail -1)" = "$first_line" ] ||
  fail "the second connection's step reached the first card: $(grep "^browser: task card $session [A-Z][a-z]* " "$log" | tail -1)"
closed=$(second "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"close_tab\",\"arguments\":{\"tab\":$second_tab}}}" | answered close_tab)
case "$closed" in "Closed tab $second_tab."*) ;; *) fail "close_tab on the second connection answered: $closed" ;; esac
exec 8>&- 9<&-
for _ in $(seq 100); do kill -0 "$second_pid" 2>/dev/null || break; sleep 0.1; done
await_log "^browser: session $second_session done\$" "closing the second connection didn't end its card"
grep -q "^browser: session $session done\$" "$log" && fail "closing the second connection ended the first's card"
# The agent's own page, with text to read and a link to follow, once a live site's (#3144).
printf '%s' '<!doctype html><title>Agent page</title><h1>Agent page</h1>
<p>A page of the check for the agent to read.</p><p><a href="two.html">Two</a></p>' \
  > "$out/site/agent.html"
opened=$(call open_tab "{\"url\":\"localhost:$port/agent.html\"}")
agent_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
[ -n "$agent_tab" ] || fail "open_tab agent.html answered: $opened"
# An agent's tab opens muted, until the user unmutes it or keeps it (#1080).
await_log "^browser: page $agent_tab audible 0 muted 1\$" "the agent's tab didn't open muted"
# Its page's picture in the card's conversation, once it shows: "x y width height".
picture() {
  await_log "^browser: task picture $1 at " "the conversation didn't picture tab $1"
  grep "^browser: task picture $1 at " "$log" | tail -1 | cut -d' ' -f6-
}
# Watches the agent's tab $1 from its picture, from the conversation.
watch() {
  read -r wx wy ww wh <<PICTURE
$(picture "$1")
PICTURE
  # The rail holds more pictures than it shows once the task has many pages, and scrolls to the
  # page that needs the user, so the one wanted may be past its top, where the first one drawn
  # stood, or past the window's foot. The user's scroll over the rail brings it back.
  rail_top=$(grep '^browser: task picture [0-9]* at ' "$log" | head -1 | cut -d' ' -f7)
  tall=$(sed -n 's/^browser: layout content [0-9]*x\([0-9]*\),.*/\1/p' "$log" | tail -1)
  if [ -z "$tall" ]; then
    send layout
    tall=$(sed -n 's/^browser: layout content [0-9]*x\([0-9]*\),.*/\1/p' "$log" | tail -1)
  fi
  if [ "$wy" -lt "$rail_top" ] || [ $((wy + wh / 2)) -ge "$tall" ]; then
    places=$(grep -c "^browser: task picture $1 at " "$log" || true)
    send scroll $((wx + ww / 2)) $((rail_top + wh / 2)) $((wy - rail_top)) 1
    await_more "^browser: task picture $1 at " "$places" ||
      fail "a scroll over the rail didn't bring tab $1's picture back into sight"
    read -r wx wy ww wh <<PICTURE
$(picture "$1")
PICTURE
  fi
  # Counted, as a page watched before has its lines already.
  watched=$(grep -c "^browser: task $session page $1\$" "$log" || true)
  placed=$(grep -c "^browser: task page $1 in place\$" "$log" || true)
  send click $((wx + ww / 2)) $((wy + wh / 2))
  await_more "^browser: task $session page $1\$" "$watched" ||
    fail "a click on tab $1's picture didn't watch it"
  await_more "^browser: task page $1 in place\$" "$placed" ||
    fail "tab $1's page didn't come to rest in place"
}
# A click in an agent's page takes it over (#1080). After clicks that are there for the page's
# focus, not to take it, ⌘↩ hands back what one took. $1 is how many presses the log had before
# them. The press reaches the window a turn after the click, which the state waits for.
hand_back_pressed() {
  send state
  [ "$(grep -c '^browser: agent page pressed in ' "$log" || true)" = "$1" ] && return
  pressed=$(sed -n 's/^browser: agent page pressed in \([0-9]*\)$/\1/p' "$log" | tail -1)
  handed=$(grep -c "^browser: agent mcp-client handed back $pressed\$" "$log" || true)
  send key return cmd
  await_more "^browser: agent mcp-client handed back $pressed\$" "$handed" ||
    fail "⌘↩ didn't hand back the page a click took, $pressed"
}
# Focus Agent Question's key (#3869).
if [ "$(uname)" = Darwin ]; then card_key="a alt shift cmd"; else card_key="a ctrl shift"; fi
# Presses the button named $1 on ilo's newest line that offers any, with the keys, as the user does
# (#3955): Focus Agent Question's key rings the line's first button, Tab moves along its buttons as
# they stand, and ↩ presses once the app says a key on the ringed one counts. Sets ilo to the line.
ilo_press() {
  case "$1" in
    "Take Over") ilo_button=TakeOver ;;
    "Give Full access") ilo_button='Raise(Full)' ;;
    "Any site") ilo_button=AnySite ;;
    "Hand Back") ilo_button=HandBack ;;
    "Show Page") ilo_button=ShowPage ;;
    "Not Now") ilo_button=NotNow ;;
    *) fail "the check knows no button of ilo's lines named $1" ;;
  esac
  ons=$(grep -c '^browser: ilo keys on ' "$log" || true)
  readies=$(grep -c '^browser: ilo keys ready on ' "$log" || true)
  send key $card_key
  await_more '^browser: ilo keys on ' "$ons" ||
    fail "Focus Agent Question's key didn't give ilo's line the keyboard, to press $1"
  ilo=$(grep '^browser: ilo keys on ' "$log" | tail -1 | cut -d' ' -f5)
  read -r ix iy <<AT
$(sed -n "s/^browser: ilo line $ilo $1 at \\([0-9-]*\\) \\([0-9-]*\\) .*/\\1 \\2/p" "$log" | tail -1)
AT
  [ -n "$ix" ] || fail "ilo's line $ilo, which Focus Agent Question's key went to, drew no $1"
  # A Tab for each place a button stands before it on its row, each where it was last drawn.
  tabs=$(sed -n "s/^browser: ilo line $ilo \\(.*\\) at \\([0-9-]*\\) \\([0-9-]*\\) [0-9]* [0-9]*\$/\\2 \\3 \\1/p" "$log" |
    awk -v x="$ix" -v y="$iy" '{ name = $3; for (i = 4; i <= NF; i++) name = name " " $i; at[name] = $1; row[name] = $2 }
      END { for (name in at) if (row[name] == y && at[name] + 0 < x + 0) before[at[name]] = 1; n = 0; for (each in before) n++; print n }')
  await_more '^browser: ilo keys ready on ' "$readies" ||
    fail "the app never said a key on ilo's line would press"
  while [ "$tabs" -gt 0 ]; do
    readies=$(grep -c '^browser: ilo keys ready on ' "$log" || true)
    send key tab
    await_more '^browser: ilo keys ready on ' "$readies" ||
      fail "the app never said a key would press after Tab on ilo's line"
    tabs=$((tabs - 1))
  done
  send key return
  expect "^browser: ilo key pressed $ilo_button\$" "↩ on ilo's line didn't press $1"
}
# Agent actions never move the user's focus (decision 8): through all the agent did in its own Space
# since the user clicked into their field, their Space, tab and keyboard stayed, and their next keys
# go into the field.
if [ -n "$yours_step" ]; then
  send state
  [ "$(space)" = "$yours_space" ] || fail "the agent's work moved the user to Space $(space)"
  shown=$(sed -n "/^browser: drive $yours_step: /,\$p" "$log" | grep '^browser: selected ' |
    grep -v "^browser: selected $user_tab\( in .*\)\{0,1\}\$" | head -1)
  [ -z "$shown" ] || fail "the agent's work showed another tab in the user's window: $shown"
  if windows && ! engine_has ../core/window.cc; then
    # An engine from before the core stopped showing the window for an agent's page's tab, which
    # on Windows gave the window itself the keyboard (#2225).
    echo "skip: mcp: the user's keyboard through the agent's work on Windows: the engine is from" \
      "before the core kept the window as it was for an agent's page's tab (#2225)"
  else
    [ "$key $focus" = "$yours_key $yours_focus" ] ||
      fail "the agent's work took the keyboard from $yours_key, on $yours_focus, to $key, on $focus"
    sed -n "/^browser: drive $yours_step: /,\$p" "$log" | grep -q "^browser: title $user_tab yours left\$" &&
      fail "the user's field lost the keyboard while the agent worked"
    send type ok
    expect "^browser: title $user_tab yours typed ok\$" "the user's keys after the agent's work didn't reach their field"
    # What a task's access doesn't cover is refused at once in ilo's own words, with nothing asked
    # of the user (#4086): a change sent to a site without its page is Full access's, so at Work on
    # pages the agent is told so and nothing reaches the site. The user's keyboard stays in their
    # field through it, and Focus Agent Question's key takes it nowhere with no conversation shown.
    stops=$(grep -c '^browser: agent mcp-client stopped ' "$log" || true)
    unsent=$(call request "{\"tab\":$agent_tab,\"requests\":[{\"method\":\"PUT\",\"path\":\"/note\",\"body\":{},\"what\":\"Replace the note.\"}]}")
    case "$unsent" in
      "error: ilo stopped this: "*"It isn't part of this task's access (Work on pages). Only the user can change a task's access, in ilo."*"the same call will go through.") ;;
      *) fail "a change at Work on pages answered: $unsent" ;;
    esac
    await_more '^browser: agent mcp-client stopped Request at Work on pages: localhost$' "$stops" ||
      fail "the change refused at Work on pages isn't logged as ilo's stop"
    grep -q '^PUT ' "$out/site.log" && fail "the change refused at Work on pages reached the site"
    send key $card_key
    sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: ilo keys on ' &&
      fail "Focus Agent Question's key took the keyboard with no conversation shown"
    [ "$key $focus" = "$yours_key $yours_focus" ] ||
      fail "the refused change, or Focus Agent Question's key after it, took the keyboard from $yours_key, on $yours_focus, to $key, on $focus"
  fi
else
  echo "skip: mcp: the user's focus through the agent's work. Browser wasn't the active app."
fi
presses=$(grep -c '^browser: agent page pressed in ' "$log" || true)
send click $page_point
driven=$active
hand_back_pressed "$presses"
if [ "$driven" = 1 ]; then
  # A click on the card opens its conversation, its steps in its agent's place.
  read -r px py pw ph <<PLATE
$(grep "^browser: task plate task-${session#session-} at " "$log" | tail -1 | cut -d' ' -f6-)
PLATE
  send click $((px + pw / 2)) $((py + ph / 2))
  expect "^browser: task $session shown\$" "a click on the client's card didn't show its conversation"
  # Its one page so far stands in the rail beside the conversation, a tile to watch it from.
  picture "$agent_tab" > /dev/null
  ../tools/window-screenshot.sh "$pid" "$out/agent-pages.png" || fail "no window to capture"
  # The change ilo refused stands in the conversation as ilo's line, with Take Over and the step
  # up that would allow it, each where the app says it drew it.
  await_log '^browser: ilo line [0-9]* Take Over at ' "the refused change's line drew no Take Over"
  await_log '^browser: ilo line [0-9]* Give Full access at ' \
    "the refused change's line drew no Give Full access"
  ../tools/window-screenshot.sh "$pid" "$out/agent-stopped.png" || fail "no window to capture"
  # Focus Agent Question's key (#3955), ⌥⇧⌘A on the Mac and Alt+Shift+A elsewhere, gives the line's
  # first button the keyboard and presses nothing, leaving the conversation's area as it is; ↩
  # straight after it is ignored, and the key again gives the keyboard back.
  closed=$(grep -c '^browser: task area [0-9]* closed$' "$log" || true)
  # ↩ goes in the driver's queue with the key, as it handles each command once the last is done:
  # waiting for the key's done line first took Windows' shell past the protection.
  post key $card_key
  chord=$step
  send key return
  expect '^browser: ilo keys on [0-9]* TakeOver$' \
    "Focus Agent Question's key didn't give the refused change's line the keyboard on Take Over" "$chord"
  # The keyboard reaches the ringed button with the next frame, so a ↩ that beats the frame goes
  # where the keyboard was and one after it is ignored by the line: either way nothing is pressed.
  sed -n "/^browser: drive $chord: /,\$p" "$log" | grep -q '^browser: ilo key pressed ' &&
    fail "↩ straight after Focus Agent Question's key pressed the line's button"
  send key $card_key
  expect '^browser: ilo keys back by key$' "Focus Agent Question's key again didn't give the keyboard back"
  [ "$(grep -c '^browser: task area [0-9]* closed$' "$log" || true)" = "$closed" ] ||
    fail "Focus Agent Question's key closed the conversation's area"
  # One of the app's own windows taking the keyboard from the line's keys (#3869): ⌘T's command
  # bar ends them where they are, giving nobody the keyboard under it, and keeps it, a letter typed
  # reaching the bar. Escape closes the bar and the line has no ring: ↩ presses nothing. Then the
  # key rings the line again, and Escape gives the keyboard back, nothing pressed.
  ons=$(grep -c '^browser: ilo keys on ' "$log" || true)
  send key $card_key
  await_more '^browser: ilo keys on ' "$ons" ||
    fail "Focus Agent Question's key didn't give the line the keyboard a second time"
  ons=$(grep -c '^browser: ilo keys on ' "$log" || true)
  send key t cmd
  bar_step=$step
  expect '^browser: command bar shown$' "⌘T from ilo's line didn't open the command bar"
  send type x
  expect '^browser: command bar suggestions "x" in ' "x typed with the command bar open over ilo's line didn't reach the bar"
  send key escape
  expect '^browser: command bar hidden$' "Escape didn't close the command bar opened over ilo's line"
  ended=$(sed -n "/^browser: drive $bar_step: /,\$p" "$log" | grep -E '^browser: ilo keys (back|left) by ' | head -1)
  case "$ended" in
    "") fail "the command bar taking the keyboard didn't end the line's keys" ;;
    "browser: ilo keys back by "*)
      fail "the line's keys gave the keyboard back with the command bar over them: $ended" ;;
  esac
  send key return
  send type y
  # The key after them, once handled, says they were.
  readies=$(grep -c '^browser: ilo keys ready on ' "$log" || true)
  send key $card_key
  await_more '^browser: ilo keys on ' "$ons" ||
    fail "Focus Agent Question's key didn't give the line the keyboard after the bar"
  sed -n "/^browser: drive $bar_step: /,\$p" "$log" | grep "^browser: title $user_tab yours typed " &&
    fail "a letter typed with or after the command bar over ilo's line reached the user's field"
  await_more '^browser: ilo keys ready on ' "$readies" ||
    fail "the app never said a key on ilo's line would press"
  send key escape
  expect '^browser: ilo keys back by escape$' "Escape on ilo's line didn't give the keyboard back"
  sed -n "/^browser: drive $chord: /,\$p" "$log" |
    grep -E '^browser: ilo key pressed |^browser: agent mcp-client taken over |^browser: task [^ ]* access ' &&
    fail "ilo's line was pressed by a key that came too soon, after the command bar or by Escape"
  # ↩ once the app says a key counts presses the ringed button: Take Over makes the tab the
  # user's, by the keys alone.
  taken=$(grep -c "^browser: agent mcp-client taken over in $agent_tab\$" "$log" || true)
  ilo_press "Take Over"
  await_more "^browser: agent mcp-client taken over in $agent_tab\$" "$taken" ||
    fail "Take Over on the refused change's line didn't make tab $agent_tab the user's"
  # Watched, the page they took over has the keyboard, and Focus Agent Question's key is the
  # page's: it keys no line under the page and leaves the page where it is. ⌘↩ hands it back.
  watch "$agent_tab"
  send key $card_key
  held_step=$step
  handed=$(grep -c "^browser: agent mcp-client handed back $agent_tab\$" "$log" || true)
  send key return cmd
  await_more "^browser: agent mcp-client handed back $agent_tab\$" "$handed" ||
    fail "⌘↩ on the page the user took over didn't hand tab $agent_tab back"
  sed -n "/^browser: drive $held_step: /,\$p" "$log" |
    grep -E "^browser: ilo keys on |^browser: focus key leaves task page |^browser: task page $agent_tab back\$" &&
    fail "Focus Agent Question's key on a page the user holds keyed a line or left the page"
  # Handed back it's the agent's, watched with the keyboard the window's: there the key goes back
  # to the conversation, as the strip's Back does, and rings no button, none having been in sight.
  thumbs=$(grep -c '^browser: task page in its thumbnail$' "$log" || true)
  send key $card_key
  back_step=$step
  expect "^browser: focus key leaves task page $agent_tab\$" \
    "Focus Agent Question's key over the agent's page watched didn't go back to the conversation" "$back_step"
  await_more '^browser: task page in its thumbnail$' "$thumbs" ||
    fail "the page Focus Agent Question's key left didn't go back into its picture"
  sed -n "/^browser: drive $back_step: /,\$p" "$log" | grep '^browser: ilo keys on ' &&
    fail "Focus Agent Question's key over a page watched keyed a line"
  watch "$agent_tab"
  ../tools/window-screenshot.sh "$pid" "$out/agent-watched.png" || fail "no window to capture"
  # The agent's navigate in its tab the user watches leaves the keyboard where the user has it, here
  # the sidebar's, not in the page it loads (#3595). Then to its page again, for the steps after.
  send click $sidebar_point
  ui_key=$key ui_focus=$focus
  loaded="^browser: loaded $agent_tab http://localhost:$port/two.html\$"
  loads=$(grep -c "$loaded" "$log" || true)
  went=$(call navigate "{\"tab\":$agent_tab,\"url\":\"localhost:$port/two.html\"}")
  case "$went" in "error: "*) fail "the agent's navigate in its watched tab answered: $went" ;; esac
  await_more "$loaded" "$loads" ||
    fail "the agent's navigate in its watched tab didn't load"
  send state
  [ "$key $focus" = "$ui_key $ui_focus" ] ||
    fail "the agent's navigate in its watched tab took the keyboard from $ui_key, on $ui_focus, to $key, on $focus"
  loaded="^browser: loaded $agent_tab http://localhost:$port/agent.html\$"
  loads=$(grep -c "$loaded" "$log" || true)
  call navigate "{\"tab\":$agent_tab,\"url\":\"localhost:$port/agent.html\"}" > /dev/null
  await_more "$loaded" "$loads" || fail "the agent's navigate back to its page didn't load it"
  # A download the agent's page starts after its call answered is the agent's in the tab the
  # conversation watches too (#3655): downloads are part of Work on pages, so it lands with nothing
  # asked and is logged as the agent's. Its script starts it 2 s and 7 s after the page loads, on a
  # host of its own.
  printf '%s' '<!doctype html><title>Later</title><script>
const q = new URLSearchParams(location.search);
onload = () => setTimeout(() => { location.href = q.get("file"); }, Number(q.get("ms")));
</script>' > "$out/site/later.html"
  later_logged='^browser: agent logged mcp-client download later\.localhost$'
  for ms in 2000 7000; do
    stem=later-${ms%000}
    printf 'later\n' > "$out/site/$stem.docx"
    logged=$(grep -c "$later_logged" "$log" || true)
    went=$(call navigate "{\"tab\":$agent_tab,\"url\":\"later.localhost:$port/later.html?ms=$ms&file=$stem.docx\"}")
    case "$went" in "error: "*) fail "the agent's navigate to the later download's page answered: $went" ;; esac
    await_more "$later_logged" "$logged" ||
      fail "a download the watched tab's page started ${ms%000} s after its navigate answered isn't in the activity log as the agent's"
    for _ in $(seq 50); do
      [ -f "$out/downloads/$stem.docx" ] && break
      sleep 0.1
    done
    [ -f "$out/downloads/$stem.docx" ] || fail "the watched tab's later download isn't in the check's folder"
  done
  call navigate "{\"tab\":$agent_tab,\"url\":\"localhost:$port/agent.html\"}" > /dev/null
fi

# Split views (#1597): the agent puts two tabs of its own side by side, adds a third, lists them
# and separates one, each through the core's split calls and checked by its rules, with its entry
# in the activity log. A tab of the user's is refused, and the split stays as it was. Each page is
# taller than its pane, so the agent's scroll there scrolls rather than answering that nothing did.
for page in a b c; do
  printf '%s' "<title>Split $page</title><p>Split page $page</p>
<button onclick=\"document.title = 'pressed'\">Press $page</button>
<div style=\"height: 300vh\"></div>" > "$out/site/split-$page.html"
done
split_tab() {
  call open_tab "{\"url\":\"localhost:$port/split-$1.html\"}" |
    sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p'
}
split_a=$(split_tab a)
split_b=$(split_tab b)
split_c=$(split_tab c)
[ -n "$split_a" ] && [ -n "$split_b" ] && [ -n "$split_c" ] ||
  fail "open_tab didn't open the split view's pages: $split_a $split_b $split_c"
refused=$(call split_tabs "{\"tabs\":[$split_a,$user_tab]}")
[ "$refused" = "error: The tab isn't in the agent's own Space." ] ||
  fail "a split with the user's tab answered: $refused"
await_log "^browser: agent activity mcp-client $user_tab split_tabs refused: The tab isn't in the agent's own Space\.\$" \
  "the refused split didn't reach the activity log"
listed=$(call list_split "{\"tab\":$split_a}")
[ "$listed" = "Tab $split_a is in no split view." ] ||
  fail "the refused split made one anyway: $listed"
split=$(call split_tabs "{\"tabs\":[$split_a,$split_b]}")
[ "$split" = "Tab $split_a's split view, left to right: $split_a, $split_b." ] ||
  fail "split_tabs answered: $split"
await_log "^browser: agent activity mcp-client $split_b split_tabs\$" \
  "the split didn't reach the activity log"
added=$(call add_to_split "{\"tab\":$split_c,\"split\":$split_a}")
[ "$added" = "Tab $split_c's split view, left to right: $split_a, $split_b, $split_c." ] ||
  fail "add_to_split answered: $added"
await_log "^browser: agent activity mcp-client $split_c add_to_split\$" \
  "adding to the split didn't reach the activity log"
refused=$(call add_to_split "{\"tab\":$user_tab,\"split\":$split_a}")
[ "$refused" = "error: The tab isn't in the agent's own Space." ] ||
  fail "adding the user's tab to the split answered: $refused"
await_log "^browser: agent activity mcp-client $user_tab add_to_split refused: The tab isn't in the agent's own Space\.\$" \
  "the refused add didn't reach the activity log"
listed=$(call list_split "{\"tab\":$split_b}")
[ "$listed" = "Tab $split_b's split view, left to right: $split_a, $split_b, $split_c." ] ||
  fail "list_split answered: $listed"
await_log "^browser: agent activity mcp-client $split_b list_split\$" \
  "listing the split didn't reach the activity log"
separated=$(call separate_tab "{\"tab\":$split_b}")
[ "$separated" = "Tab $split_b is out of its split view." ] || fail "separate_tab answered: $separated"
await_log "^browser: agent activity mcp-client $split_b separate_tab\$" \
  "separating the tab didn't reach the activity log"
listed=$(call list_split "{\"tab\":$split_a}")
[ "$listed" = "Tab $split_a's split view, left to right: $split_a, $split_c." ] ||
  fail "after separate_tab, list_split answered: $listed"
refused=$(call separate_tab "{\"tab\":$user_tab}")
[ "$refused" = "error: The tab isn't in the agent's own Space." ] ||
  fail "separating the user's tab answered: $refused"
await_log "^browser: agent activity mcp-client $user_tab separate_tab refused: The tab isn't in the agent's own Space\.\$" \
  "the refused separate didn't reach the activity log"
# The agent's own click in its page, as the window watches it, isn't the user's press: it comes
# through DevTools, as the user's would reach the page, and takes nothing over (#3243). The same
# for the page beside it in its watched split view. Its next action then isn't refused.
if [ "$driven" = 1 ]; then
  # Escape steps back to Pages from the page watched, $1, which goes back into its picture.
  to_pages() {
    back='^browser: task page [0-9]* back$'
    thumbnail='^browser: task page in its thumbnail$'
    before=$(grep -c "$back" "$log" || true)
    shrunk=$(grep -c "$thumbnail" "$log" || true)
    # A page there can have the keyboard, as the one a press in the split view focused on X11,
    # and keeps Escape: a click on the sidebar gives the UI the keyboard, as the user's would.
    send click $sidebar_point
    send key escape
    await_more "$back" "$before" || fail "Escape didn't step back to Pages from $1"
    await_more "$thumbnail" "$shrunk" || fail "$1 didn't go back into its picture"
  }
  to_pages "tab $agent_tab"
  watch "$split_a"
  presses=$(grep -c '^browser: agent page pressed in ' "$log" || true)
  agent_clicks=$step
  for tab in $split_a $split_c; do
    ref=$(call snapshot "{\"tab\":$tab}" | sed -n 's/.*- button "Press [ac]" \[\(e[0-9]*\)\].*/\1/p' | head -1)
    [ -n "$ref" ] || fail "the snapshot of the split view's tab $tab had no Press button"
    clicked=$(call click "{\"tab\":$tab,\"ref\":\"$ref\"}")
    case "$clicked" in "error: "*) fail "the agent's click in its watched split view's tab $tab answered: $clicked" ;; esac
    await_log "^browser: title $tab pressed\$" "the agent's click in tab $tab didn't reach its page"
    scrolled=$(call scroll "{\"tab\":$tab}")
    case "$scrolled" in
      "error: "*) fail "after its own click in tab $tab, watched in its split view, the agent's scroll answered: $scrolled" ;;
    esac
  done
  send state
  [ "$(grep -c '^browser: agent page pressed in ' "$log" || true)" = "$presses" ] ||
    fail "the agent's own clicks in its watched split view reached the app as the user's press: $(since "$agent_clicks" | grep '^browser: agent' | tail -6 | tr '\n' ' ')"
  since "$agent_clicks" | grep -q -e "^browser: agent mcp-client taken over in $split_a\$" \
    -e "^browser: agent mcp-client taken over in $split_c\$" &&
    fail "the agent's own click in its watched split view took its tab over"
  to_pages "the split view"
  watch "$agent_tab"
fi
# The split views' pages close, so the conversation's grid has the pages the steps after it count on.
for split_tab in $split_a $split_b $split_c; do
  closed=$(call close_tab "{\"tab\":$split_tab}")
  case "$closed" in "Closed tab $split_tab."*) ;; *) fail "closing the split view's page answered: $closed" ;; esac
done
# An agent's tab at Chrome's warning before http, in the agent's split view, watched from the
# conversation, where its split's pages share the watched page's place. The warning shown doesn't
# take the user's keyboard, on the Mac too, where it's a window of its own (#3240). Once the user's
# click gives it the keyboard, and takes the tab over as a press in its page does (#3243), ⌘W
# steps back to Pages from there, a press in the page beside it takes that tab over and the
# keyboard to that page, and never to an agent's dialog: on the Mac that press gives the keyboard
# back to the browser window (#3212). Escape there is the warning's, as Chrome's is.
if [ "$driven" = 1 ]; then
  await '^browser: settings about Version .*, on Chromium [0-9]' || fail "the engine's version wasn't said"
  major=$(sed -n 's/^browser: settings about .*on Chromium \([0-9]*\)\..*/\1/p' "$log" | head -1)
fi
if [ "$driven" = 1 ] && [ "$major" -ge 157 ]; then
  warned=$(call open_tab "{\"url\":\"http://$plain/split-a.html\"}" |
    sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  [ -n "$warned" ] || fail "open_tab didn't open http://$plain/split-a.html"
  await_log "browser: tab $warned error -20\$" \
    "the agent's tab at http://$plain/split-a.html had no warning before http: $(grep "^browser: loaded $warned " "$log" | tail -1)"
  beside=$(split_tab b)
  split=$(call split_tabs "{\"tabs\":[$warned,$beside]}")
  [ "$split" = "Tab $warned's split view, left to right: $warned, $beside." ] ||
    fail "split_tabs with the tab at the warning answered: $split"
  # Tab round the strip's ring never gives the watched agent's page the keyboard: its stop is the
  # strip's words, so the handback into a page, which on Windows gives the shown page's pane the
  # keyboard straight (#3396), never runs for it. Tab from the window's keyboard rings the strip,
  # where F6 would leave the agent's Space for the user's (sidebar_keys.rs). Windows alone, the
  # change's.
  ring_strip() {
    windows || return 0
    ringed=$((step + 1))
    send key tab
    expect '^browser: strip keyed ' "Tab with the agent's split watched didn't ring the strip under it"
    for _ in 1 2 3 4 5 6 7 8; do
      send key tab
    done
    send state
    logged '^browser: strip keyed page$' "$ringed" ||
      fail "Tab round the strip with the agent's split watched never rang its page's stop"
    ! logged '^browser: ring gave the page ' "$ringed" ||
      fail "Tab round the ring with the agent's split watched gave its page the keyboard"
    case "$key $focus" in
      *NativeWidgetMacNSWindow* | *RenderWidgetHostViewCocoa*)
        fail "Tab round the ring with the agent's split watched left the keyboard in $key, on $focus" ;;
    esac
  }
  # Watches $warned, whose warning shows over its pane, at $warning, without the user's keyboard,
  # and has a click on it give it the keyboard. $1 says how the user came to watch it, $2 is a
  # step to take while it's watched, before the click, and $3, "watched", says it's watched already.
  watch_warning() {
    [ "${3:-}" = watched ] || watch "$warned"
    for _ in $(seq 50); do
      send layout
      since | grep -q '^browser: layout child ' && break
      sleep 0.1
    done
    laid=$(since | grep -m1 '^browser: layout')
    warning=$(since | sed -n 's/^browser: layout child //p' | head -1)
    [ -n "$warning" ] || fail "the agent's tab $warned, watched in its split view $1, showed no warning before http: $laid"
    send state
    [ "$key" != NativeWidgetMacNSWindow ] ||
      fail "the agent's warning before http, watched in its split view $1, took the user's keyboard as it showed: $key, $focus"
    [ -z "${2:-}" ] || "$2"
    warning_x=${warning%%,*} warning_y=${warning#*,}
    warning_y=${warning_y%% *} warning_size=${warning#* }
    warning_width=${warning_size%%x*} warning_height=${warning_size#*x}
    send click $((warning_x + warning_width / 2)) $((warning_y + 8))
    warning_clicked=$step
    send state
    [ "$key" = NativeWidgetMacNSWindow ] ||
      fail "a click on the agent's warning before http at $warning, watched in its split view $1, didn't give it the keyboard: $key, $focus"
  }
  # Hands $warned back from its page watched, the UI given the keyboard by a click on the sidebar.
  hand_back_warned() {
    watch "$warned"
    send click $sidebar_point
    handed=$(grep -c "^browser: agent mcp-client handed back $warned\$" "$log" || true)
    send key return cmd
    await_more "^browser: agent mcp-client handed back $warned\$" "$handed" ||
      fail "⌘↩ didn't hand back tab $warned, which the click on its warning took over"
  }
  # Waits for the click on the warning to take $warned over, again; $1 says when.
  took_warned() {
    await_more "^browser: agent mcp-client taken over in $warned\$" "$took" ||
      fail "the click on the agent's warning before http, watched $1, didn't take tab $warned over: $(since "$warning_clicked" | grep -e '^browser: drive' -e '^browser: agent' | tail -6 | tr '\n' ' ')"
  }
  # A click on the split's picture straight after Escape, while the page watched still goes back
  # into its own, watches the split: the page going back lets the click through (#3496). The
  # conversation first draws the split's picture as the page goes back, and the click is posted
  # the moment it logs it, as send's and await's tenths would take most of the zoom's 0.2 s. It goes
  # to the point of the split's picture nearest the agent's, which the page going back covers
  # longest.
  read -r cx cy nw nh <<PICTURE
$(picture "$agent_tab")
PICTURE
  cx=$((cx + nw / 2)) cy=$((cy + nh / 2))
  watched=$(grep -c "^browser: task $session page $warned\$" "$log" || true)
  placed=$(grep -c "^browser: task page $warned in place\$" "$log" || true)
  before=$(grep -c '^browser: task page [0-9]* back$' "$log" || true)
  drawn=$(grep -c "^browser: task picture $warned at " "$log" || true)
  send click $sidebar_point
  post key escape
  escaped=$step
  for _ in $(seq 2000); do
    [ "$(grep -c "^browser: task picture $warned at " "$log")" -gt "$drawn" ] && break
  done
  at=$(grep "^browser: task picture $warned at " "$log" | tail -1)
  [ -n "$at" ] || fail "Escape from tab $agent_tab never drew tab $warned's picture in Pages"
  read -r wx wy ww wh <<PICTURE
${at#browser: task picture $warned at }
PICTURE
  [ "$cx" -ge $((wx + 4)) ] || cx=$((wx + 4))
  [ "$cx" -le $((wx + ww - 5)) ] || cx=$((wx + ww - 5))
  [ "$cy" -ge $((wy + 4)) ] || cy=$((wy + 4))
  [ "$cy" -le $((wy + wh - 5)) ] || cy=$((wy + wh - 5))
  post click "$cx" "$cy"
  await "^browser: drive $step done" || fail "the driver didn't finish the click on tab $warned's picture"
  await_more '^browser: task page [0-9]* back$' "$before" || fail "Escape didn't step back to Pages from tab $agent_tab"
  await_more "^browser: task $session page $warned\$" "$watched" ||
    fail "a click on tab $warned's picture at $cx $cy while tab $agent_tab went back into its own after Escape didn't watch it: $(since "$escaped" | grep -e '^browser: drive' -e '^browser: task' | head -12 | tr '\n' ' ')"
  await_more "^browser: task page $warned in place\$" "$placed" || fail "tab $warned's page didn't come to rest in place"
  since "$escaped" | grep -m1 -e '^browser: drive [0-9]*: click ' -e '^browser: task page in its thumbnail$' |
    grep -q '^browser: task page' &&
    echo "skip: mcp: a click on a picture while the page watched goes back: tab $agent_tab was back in its picture before the click (#3496)."
  presses=$(grep -c '^browser: agent page pressed in ' "$log" || true)
  took=$(grep -c "^browser: agent mcp-client taken over in $warned\$" "$log" || true)
  watch_warning "from Pages" ring_strip watched
  first_click=$warning_clicked
  # Linux's driver lists the pages the shell lays out, and the task area lays out a watched split
  # as one, which the core divides: the capture shows its panes there.
  if [ "$(uname)" = Linux ]; then
    echo "skip: mcp: the watched split view's two pages in the layout, on Linux: the driver lists the shell's one frame for them. $out/agent-split-warning.png shows them."
  else
    # A tab switch keeps the page it left in the window, past the content's right edge, until the
    # page it shows draws (#3769), so only the pages within the content are the split's.
    width=$(echo "$laid" | sed -n 's/^browser: layout content \([0-9]*\)x.*/\1/p')
    panes=$(echo "$laid" | grep -o 'WebContentsViewCocoa [0-9,]* [0-9x]*' | sed 's/^WebContentsViewCocoa //' |
      awk -v width="$width" '{ split($1, at, ","); split($2, size, "x"); if (at[1] + size[1] <= width) print }')
    [ -n "$width" ] && [ "$(echo "$panes" | wc -l)" -eq 2 ] ||
      fail "watching tab $warned didn't show its split view's two pages within the content, ${width:-?} wide: $laid"
  fi
  ../tools/window-screenshot.sh "$pid" "$out/agent-split-warning.png" || fail "no window to capture"
  # The click on the agent's warning, a press on its tab's dialog, took the tab over, as a press
  # in its page does: answering it acts on the tab, which is the user's from then (#3243).
  await_log "^browser: agent page pressed in $warned\$" \
    "the click on the agent's warning before http, watched from Pages, didn't reach the app as a press on tab $warned: $(since "$first_click" | grep -e '^browser: drive' -e '^browser: agent' | tail -6 | tr '\n' ' ')"
  took_warned "from Pages"
  # From the warning with the keyboard, ⌘W steps back to Pages, as it does from its page.
  back='^browser: task page [0-9]* back$'
  thumbnail='^browser: task page in its thumbnail$'
  before=$(grep -c "$back" "$log" || true)
  shrunk=$(grep -c "$thumbnail" "$log" || true)
  send key w cmd
  left=$step
  await_more "$back" "$before" ||
    fail "⌘W on the agent's warning before http, which had the keyboard, didn't step back to Pages: $(since "$left" | grep -e '^browser: drive' -e '^browser: command' -e '^browser: task' | tail -6 | tr '\n' ' ')"
  await_more "$thumbnail" "$shrunk" ||
    fail "the agent's split view didn't go back into its picture after ⌘W"
  # The tab is the user's, and its warning takes the keyboard as Chrome's does, so it's handed back
  # from its page watched, the UI given the keyboard by a click on the sidebar, as the user's would:
  # the warning keeps Return, so ⌘↩ reaches ilo only from there.
  hand_back_warned
  to_pages "tab $warned"
  # Watched again, the agent's warning shows without the keyboard (#3240).
  presses=$(grep -c '^browser: agent page pressed in ' "$log" || true)
  took=$(grep -c "^browser: agent mcp-client taken over in $warned\$" "$log" || true)
  watch_warning "again after ⌘W"
  took_warned "again after ⌘W"
  # The page beside, to the right of the warning's, as split_tabs put them.
  other="$((warning_x + warning_width + 100)) $((warning_y + warning_height + 100))"
  send click $other
  pressed=$step
  send state
  [ "$key" = BrowserCoreNSWindow ] ||
    fail "a press in the page beside the agent's warning before http left the keyboard in $key, on $focus: $(since "$pressed" | grep -e '^browser: drive' -e '^browser: agent' | tail -6 | tr '\n' ' ')"
  ../tools/window-screenshot.sh "$pid" "$out/agent-split-pressed.png" || fail "no window to capture"
  # The press beside takes over the tab pressed, not the one watched (#3243), and ⌘↩ from that page,
  # which has the keyboard, hands it back.
  await_log "^browser: agent page pressed in $beside\$" \
    "a press in the page beside the agent's warning didn't reach the app as a press on tab $beside: $(since "$pressed" | grep -e '^browser: drive' -e '^browser: agent' | tail -6 | tr '\n' ' ')"
  await_log "^browser: agent mcp-client taken over in $beside\$" \
    "a press in the other pane of a watched agent split didn't take over the tab pressed, $beside"
  taken=$(since "$warning_clicked" | sed -n 's/^browser: agent page pressed in \([0-9]*\)$/\1/p' |
    sort -u | tr '\n' ' ')
  [ "$taken" = "$(printf '%s\n' "$warned" "$beside" | sort -u | tr '\n' ' ')" ] ||
    fail "the click on the agent's warning and the press beside it took over tabs $taken, not $warned and $beside"
  hand_back_pressed "$presses"
  # The warning's tab, which its click took, is handed back from its page watched.
  to_pages "the split view"
  hand_back_warned
  # Escape on the warning with the keyboard is the warning's, its Go back, on every OS: a dialog
  # keeps the keys that answer it (dialog_keys.cc). The click that gave it the keyboard took the
  # tab over, so the Escape is the user's act on their own tab, and the strip says they have it
  # before the page leaves. The warning goes, and the tab leaves the http page, here for the new
  # tab page, as it had no page before it. ⌘↩ hands it back after.
  to_pages "tab $warned"
  # The press beside took $beside over, where the agent acted last, which ended its session there.
  # The click on the warning in $warned, its other tab, takes that one over too: whose tab it is
  # decides, not where the agent acted last (#3503).
  last=$(grep '^browser: agent mcp-client \(at work\|done\) in ' "$log" | tail -1)
  [ "$last" = "browser: agent mcp-client done in $beside" ] ||
    fail "the press beside the agent's warning didn't end its session in $beside, where it acted last, before the click on the warning in tab $warned: $last"
  presses=$(grep -c '^browser: agent page pressed in ' "$log" || true)
  took=$(grep -c "^browser: agent mcp-client taken over in $warned\$" "$log" || true)
  driving=$(grep -c "^browser: task strip $warned says You're driving" "$log" || true)
  watch_warning "before Escape"
  took_warned "before Escape"
  await_more "^browser: task strip $warned says You're driving" "$driving" ||
    fail "the strip under tab $warned didn't say the user has it once the click on its warning took it: $(grep "^browser: task strip $warned says" "$log" | tail -1)"
  send key escape
  escaped=$step
  for _ in $(seq 50); do
    since "$escaped" | grep "^browser: page $warned " | grep -qv "$plain" && break
    sleep 0.1
  done
  since "$escaped" | grep "^browser: page $warned " | grep -qv "$plain" ||
    fail "Escape on the agent's warning before http, which had the keyboard, didn't answer it: tab $warned stayed at http://$plain: $(since "$escaped" | grep -e '^browser: drive' -e "^browser: page $warned " -e '^browser: task' | tail -6 | tr '\n' ' ')"
  for _ in $(seq 50); do
    send layout
    none_since '^browser: layout child ' && break
    sleep 0.1
  done
  none_since '^browser: layout child ' ||
    fail "the agent's warning before http stayed after its Escape: $(since | grep '^browser: layout child ')"
  send click $sidebar_point
  hand_back_pressed "$presses"
  to_pages "tab $warned"
  for split_tab in $warned $beside; do
    closed=$(call close_tab "{\"tab\":$split_tab}")
    case "$closed" in "Closed tab $split_tab."*) ;; *) fail "closing the split view's page answered: $closed" ;; esac
  done
  watch "$agent_tab"
else
  echo "skip: mcp: an agent's warning before http in its split view, watched. Browser wasn't the active app, or the engine is before 157."
fi

# The agent's page watched dies, as macOS ends a renderer under memory pressure: its place shows the
# crash card, as the user's own page's does, and a press on its Reload is the user acting on the
# agent's tab, so it takes the tab over, as a press in its page does, and loads the page again
# (#3433). The core hears no press there, as the page has no renderer. ⌘↩ hands it back. The agent
# reads the page first, as a press takes over only a tab the agent is at work in, and closing the
# split view's pages left it at work in none.
if [ "$driven" = 1 ]; then
  at_work=$(grep -c "^browser: agent mcp-client at work in $agent_tab\$" "$log" || true)
  read=$(call read_page "{\"tab\":$agent_tab}")
  case "$read" in
    "Tab $agent_tab: Agent page"*) ;;
    *) fail "reading the agent's tab before its renderer was killed answered: $read" ;;
  esac
  await_more "^browser: agent mcp-client at work in $agent_tab\$" "$at_work" ||
    fail "the agent's tab didn't show it at work once it read the page"
  presses=$(grep -c '^browser: agent page pressed in ' "$log" || true)
  taken=$(grep -c "^browser: agent page pressed in $agent_tab\$" "$log" || true)
  reloads=$(grep -c "^browser: loaded $agent_tab " "$log" || true)
  fine=$(grep -c "^browser: page $agent_tab trouble fine\$" "$log" || true)
  kill_pages || fail "no renderer to kill"
  await_log "^browser: page $agent_tab trouble crashed\$" "the agent's page watched, its renderer killed, wasn't called crashed"
  await_log "^browser: crash card shown $agent_tab\$" \
    "the agent's page watched that died showed no crash card in its place: $(grep -e "^browser: page $agent_tab " -e '^browser: crash ' "$log" | tail -4 | tr '\n' ' ')"
  ../tools/window-screenshot.sh "$pid" "$out/agent-crashed.png" || fail "no window to capture"
  at=$(sed -n "s/^browser: crash reload $agent_tab at \([0-9]* [0-9]* [0-9]* [0-9]*\)$/\1/p" "$log" | tail -1)
  [ -n "$at" ] || fail "the crash card of the agent's page watched drew no Reload"
  read -r rx ry rw rh <<RELOAD
$at
RELOAD
  send click $((rx + rw / 2)) $((ry + rh / 2))
  expect "^browser: crash card reload $agent_tab\$" "a click on the crash card's Reload in the agent's page watched didn't press it"
  await_more "^browser: agent page pressed in $agent_tab\$" "$taken" ||
    fail "a press on the crash card of the agent's page watched didn't take the tab over: $(since | grep -e '^browser: drive' -e '^browser: agent' | tail -6 | tr '\n' ' ')"
  await_more "^browser: loaded $agent_tab " "$reloads" || fail "the crash card's Reload didn't load the agent's page again"
  await_more "^browser: page $agent_tab trouble fine\$" "$fine" || fail "the agent's page loaded again wasn't fine"
  # The tab is the user's until they hand it back, so list_tabs says so and nothing of its page:
  # neither its title nor its address (#3694). Handed back, it's listed as before, below.
  held=$(call list_tabs '{}' | grep "^. $agent_tab  ")
  case "$held" in
    *"  With the user until they hand it back"*) ;;
    *) fail "list_tabs didn't say the tab the user took over is with them: $held" ;;
  esac
  case "$held" in
    *"Agent page"* | *"localhost"*) fail "list_tabs named the page of the tab the user holds: $held" ;;
  esac
  send click $sidebar_point
  hand_back_pressed "$presses"
fi

# upload asks the user for a file for a page's file field (#3656), and the core refuses one before
# any card where it isn't a file field or names no site to give the file to: a folder's field, a
# button, a field in a sandboxed frame, whose origin is opaque, and a field whose document's root
# element frames no document, as an iframe the page put there after the snapshot or an object.
# Each refusal says why, with an activity-log entry, and the core shows no field for a card. A file
# field hidden behind its label is outlined as the label's text alone, with no ref to upload to.
# A page told something to do waits for a file the run puts in the site (told.js).
printf '%s' '// Runs then once the run puts name.go in the site, asked for every 100 ms till then.
function told(name, then) {
  const ask = () => fetch(name + ".go", { cache: "no-store" })
    .then(answer => answer.ok ? then() : setTimeout(ask, 100), () => setTimeout(ask, 100));
  ask();
}' > "$out/site/told.js"
printf '%s' '<input type=file aria-label="Sandboxed file">' > "$out/site/upload-sandboxed.html"
printf '%s' '<title>loading</title>
<input type=file webkitdirectory aria-label="Photos folder">
<button>Send</button>
<label for=cv>Upload your CV</label><input id=cv type=file style="display: none">
<iframe sandbox src=upload-sandboxed.html onload="document.title = &quot;upload refused&quot;"></iframe>' \
  > "$out/site/upload-refused.html"
printf '%s' '<title>upload root</title><script src=told.js></script>
<input type=file aria-label="Root file">
<script>told("upload-root", () => {
  const frame = document.createElement("iframe");
  frame.append(document.querySelector("input"));
  document.replaceChild(frame, document.documentElement);
  fetch("upload-root.done");
});</script>' > "$out/site/upload-root.html"
printf '%s' '<title>upload object root</title>
<input type=file aria-label="Object root file">
<script>
const object = document.createElement("object");
object.data = "missing";
object.append(document.querySelector("input"));
document.replaceChild(object, document.documentElement);
</script>' > "$out/site/upload-object-root.html"
# The fields the core showed in tab $1, as its "file field <id> shown in <tab>" says.
shown_fields() { grep -c "browser: file field [0-9]* shown in $1\$" "$log" || true; }
# Calls upload for the ref $2 in tab $1, whose field is $3, and sets refused to its answer, failing
# if the step was handed to the user, as an upload's is, or the core showed the field.
upload_refused() {
  handed=$(grep -c "^browser: agent mcp-client hands off $1: Choose " "$log" || true)
  shown=$(shown_fields "$1")
  refused=$(call upload "{\"tab\":$1,\"ref\":\"$2\"}")
  [ "$(grep -c "^browser: agent mcp-client hands off $1: Choose " "$log" || true)" = "$handed" ] ||
    fail "upload for $3 handed the tab to the user, as: $refused"
  [ "$(shown_fields "$1")" = "$shown" ] || fail "the core showed $3, though upload answered: $refused"
}
# Opens the page $1 in a tab of the agent's and sets upload_tab to it and opened to the answer.
upload_page() {
  opened=$(call open_tab "{\"url\":\"$1\"}")
  upload_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  [ -n "$upload_tab" ] || fail "open_tab of $1 answered: $opened"
}
folder_why="That field takes a folder. Hand the tab to the user with hand_off."
button_why="That element isn't a file field. Call upload on an input type=file (listed in snapshot as a button, usually “Choose File” or “Browse”)."
no_site_why="That field is in a frame with no site of its own, so ilo can't say which site is asking. Hand the tab to the user with hand_off."
upload_page "localhost:$port/upload-refused.html"
refused_tab=$upload_tab
await_log "^browser: title $refused_tab upload refused\$" "the upload page's sandboxed frame didn't load"
outline=$(call snapshot "{\"tab\":$refused_tab}")
printf '%s\n' "$outline" > "$out/mcp-upload-refused.txt"
for each in "Photos folder" Send "Sandboxed file"; do
  [ -n "$(ref_in "$outline" button "$each")" ] || fail "the upload page's outline had no ref for $each: $outline"
done
upload_refused "$refused_tab" "$(ref_in "$outline" button "Photos folder")" "a folder's field"
[ "$refused" = "error: $folder_why" ] || fail "upload for a folder's field answered: $refused"
await_log "^browser: agent logged mcp-client upload localhost refused: $folder_why\$" \
  "the refused upload for a folder's field isn't in the activity log"
upload_refused "$refused_tab" "$(ref_in "$outline" button Send)" "a button"
[ "$refused" = "error: $button_why" ] || fail "upload for a button answered: $refused"
await_log "^browser: agent logged mcp-client upload localhost refused: $button_why\$" \
  "the refused upload for a button isn't in the activity log"
upload_refused "$refused_tab" "$(ref_in "$outline" button "Sandboxed file")" "a sandboxed frame's field"
[ "$refused" = "error: $no_site_why" ] || fail "upload for a sandboxed frame's field answered: $refused"
await_log "^browser: agent logged mcp-client upload localhost refused: $no_site_why\$" \
  "the refused upload for a sandboxed frame's field isn't in the activity log"
# Chromium leaves a field the page hid out of the tree, and its label names no field the outline
# has, so it's the label's text, with no ref. An agent there has no field to upload to.
label=$(printf '%s\n' "$outline" | grep -F 'Upload your CV')
[ -n "$label" ] || fail "the outline left out the label of the file field hidden behind it: $outline"
case "$label" in
  *'[e'*) fail "the label of a file field hidden behind it was outlined with a ref, which upload's steps don't cover: $label" ;;
esac
# The page puts an iframe in its root element's place after the snapshot, the field inside it, where
# it's never drawn: the field is gone, or is in a document whose root frames none, refused either way.
upload_page "localhost:$port/upload-root.html"
root_tab=$upload_tab
root_ref=$(ref_in "$opened" button "Root file")
[ -n "$root_ref" ] || fail "the page whose root element goes had no ref for its field: $opened"
: > "$out/site/upload-root.go"
served "^GET chromium localhost:$port/upload-root\\.done " "the page didn't put an iframe in its root element's place"
upload_refused "$root_tab" "$root_ref" "a field the page moved into an iframe in its root element's place"
case "$refused" in
  "error: $no_site_why" | "error: The element isn't on the page any more. Take a new snapshot.") ;;
  *) fail "upload for a field the page moved into an iframe in its root element's place answered: $refused" ;;
esac
# An object as the root, its data missing, draws the field inside it as what it falls back to.
upload_page "localhost:$port/upload-object-root.html"
object_tab=$upload_tab
object_ref=
for _ in $(seq 20); do
  object_ref=$(ref_in "$opened" button "Object root file")
  [ -n "$object_ref" ] && break
  sleep 0.25
  opened=$(call snapshot "{\"tab\":$object_tab}")
done
[ -n "$object_ref" ] || fail "the page whose root element is an object had no ref for the field inside it: $opened"
upload_refused "$object_tab" "$object_ref" "a field in an object that's its document's root element"
[ "$refused" = "error: $no_site_why" ] ||
  fail "upload for a field in an object that's its document's root element answered: $refused"
for each in "$refused_tab" "$root_tab" "$object_tab"; do
  closed=$(call close_tab "{\"tab\":$each}")
  case "$closed" in "Closed tab $each."*) ;; *) fail "closing upload's tab $each answered: $closed" ;; esac
done

if [ "$driven" = 1 ]; then
  # The core lists the agent's Space alone: its tab, and not the user's Wikipedia tab in theirs.
  listed=$(call list_tabs '{}')
  case "$listed" in
    *"$agent_tab  Agent page  http://localhost:$port/agent.html"*) ;;
    *) fail "list_tabs didn't list the agent's tab: $listed" ;;
  esac
  printf '%s\n' "$listed" | grep -q "^. $user_tab  " &&
    fail "list_tabs listed the user's tab in another Space: $listed"
  read=$(call read_page "{\"tab\":$agent_tab}")
  case "$read" in
    "Tab $agent_tab: Agent page"*"for the agent to read"*) ;;
    *) fail "reading the agent's tab answered: $read" ;;
  esac
  await_log "^browser: agent activity mcp-client $agent_tab read_page\$" "the read didn't reach the activity log"
  await_log "^browser: tab text $agent_tab [0-9]* [1-9][0-9]*\$" "the core didn't read the page"
  await_log "^browser: agent mcp-client at work in $agent_tab\$" "the agent's tab didn't show it at work"

  # Sessions, never secrets: a page's text has its prose, and no field's value, whether the user
  # typed it, the page filled it in as autofill does, or it's a password.
  serve
  # The page names itself once its fields and their handlers are there, which its title in the
  # head, read before them, wouldn't say, and it has painted (painted.js, site.sh).
  printf '%s' '<script src="painted.js"></script>
<p style="position:relative;z-index:1;pointer-events:none">Fields for the agents check.</p>
<label>Name <input id=typed style="position:fixed;inset:0;width:100%;height:100%;color:#f00"></label>
<label>Draft <textarea id=draft
  style="position:fixed;top:0;right:0;width:25%;height:15%;z-index:2;color:#f00"></textarea></label>
<label>Password <input id=password type=password></label>
<label>Email <input id=email autocomplete=email></label>
<label>Card <input id=card autocomplete=cc-number value=4111111111111111></label>
<label>Note <textarea id=note>note-by-ada</textarea></label>
<div contenteditable>editable-by-ada</div>
<select><option>Pick one<option selected>chosen-by-ada</select>
<script>
password.value = "hunter2-by-ada";
email.value = "ada@example.com";
typed.oninput = () => {
  typed.setAttribute("value", typed.value);
  document.title = "typed";
};
draft.oninput = () => {
  draft.textContent = draft.value;
  document.title = "drafted";
};
// A form'"'"'s Send on ⌘↩ or Ctrl+Enter, which would keep Hand Back'"'"'s key from ilo (#1080).
onkeydown = (e) => {
  if (e.key == "Enter" && (e.metaKey || e.ctrlKey)) {
    e.preventDefault();
    document.title = "sent";
  }
};
note.oninput = () => document.title = "noted";
painted(() => document.title = "fields");
</script>' > "$out/site/fields.html"
  call navigate "{\"tab\":$agent_tab,\"url\":\"localhost:$port/fields.html\"}" > /dev/null
  await_log "^browser: title $agent_tab fields\$" "the fields page didn't load in the agent's tab"
  # A click while the page still loads can land before its field is drawn, and focus no field.
  await_log "^browser: page $agent_tab back 1 forward 0 loading 0 http://localhost:$port/fields\\.html\$" \
    "the fields page didn't finish loading"
  presses=$(grep -c '^browser: agent page pressed in ' "$log" || true)
  send click $page_point
  [ "$focus" = RenderWidgetHostViewCocoa ] || fail "a click on the fields page left focus on $focus"
  send type typed-by-ada
  expect "^browser: title $agent_tab typed\$" "typing didn't reach the page's field"
  send key tab
  send type drafted-by-ada
  expect "^browser: title $agent_tab drafted\$" "typing didn't reach the page's textarea"
  # The page keeps ⌘↩ as its form's Send, so Hand Back's key reaches ilo before it (#1080).
  hand_back_pressed "$presses"
  grep -q "^browser: title $agent_tab sent\$" "$log" &&
    fail "the page's form took ⌘↩, not Hand Back"
  read=$(call read_page "{\"tab\":$agent_tab}")
  printf '%s\n' "$read" > "$out/mcp-fields.txt"
  case "$read" in
    *"Fields for the agents check."*) ;;
    *) fail "reading the fields page didn't give its prose: $read" ;;
  esac
  secrets="typed-by-ada drafted-by-ada hunter2-by-ada ada@example.com 4111111111111111 note-by-ada
    editable-by-ada chosen-by-ada"
  for secret in $secrets; do
    case "$read" in *"$secret"*) fail "reading the fields page gave a field's value, $secret" ;; esac
  done

  # The outline names the fields by ref and the password field off limits. A field shows what it
  # holds only where that's the page's own, as the select's option marked selected is, and of any
  # other only that it's filled: the user's typing, in a field whose page keeps its markup in step
  # with what it holds too, and an input's or a textarea's text from the markup, which the core
  # can't tell from that. A select's line names its options, the page's own (#2355).
  outline=$(call snapshot "{\"tab\":$agent_tab}")
  printf '%s\n' "$outline" > "$out/mcp-outline.txt"
  case "$outline" in
    "Tab $agent_tab: drafted"*"Fields for the agents check."*"(password: off limits)"*) ;;
    *) fail "the fields page's outline was: $outline" ;;
  esac
  for secret in $secrets; do
    case $secret in chosen-by-ada) continue ;; esac
    case "$outline" in *"$secret"*) fail "the fields page's outline gave a field's value, $secret" ;; esac
  done
  ref_of() { printf '%s\n' "$outline" | sed -n "s/.*- $1 \"$2\" \[\(e[0-9]*\)\].*/\1/p" | head -1; }
  line_of() { printf '%s\n' "$outline" | grep -- "- $1 \"$2\" \["; }
  for field in Name Draft Email Card Note; do
    case "$(line_of textbox $field)" in
      *'] (filled)') ;;
      *) fail "the $field field's line didn't say only that it was filled: $(line_of textbox $field)" ;;
    esac
  done
  # Its picture paints over the user's typing in both, whose text is red: none is left.
  mcp "$(request screenshot "{\"tab\":$agent_tab}")" | python3 -I -c '
import base64, json, sys
content = json.load(sys.stdin)["result"]["content"][0]
if content["type"] != "image":
    sys.exit(content.get("text", "no picture"))
open(sys.argv[1], "wb").write(base64.b64decode(content["data"]))' "$out/agent-typed.jpg" ||
    fail "the screenshot of the fields page answered no picture"
  chosen=$(printf '%s\n' "$outline" | grep 'chosen-by-ada')
  case "$chosen" in
    *'- combobox [e'*'] (holds "chosen-by-ada") (options: "Pick one", "chosen-by-ada")') ;;
    *) fail "the select's line didn't hold its option marked selected and name its options: $chosen" ;;
  esac
  password=$(ref_of textbox Password)
  refused=$(call type "{\"tab\":$agent_tab,\"ref\":\"$password\",\"text\":\"by-the-agent\"}")
  case "$refused" in
    "error: Password fields are off limits"*) ;;
    *) fail "typing into the password field answered: $refused" ;;
  esac
  note=$(ref_of textbox Note)
  [ -n "$note" ] || fail "the outline had no ref for the note"
  typed=$(call type "{\"tab\":$agent_tab,\"ref\":\"$note\",\"text\":\"by-the-agent\"}")
  case "$typed" in
    "Typed into “Note”."*) ;;
    *) fail "typing into the note answered: $typed" ;;
  esac
  # What changed in the outline is the note, which holds the agent's typing with the page's text.
  case "$typed" in
    *'- textbox "Note" [e'*'] (holds "'*by-the-agent*'")'*) ;;
    *) fail "typing into the note didn't answer with what the note holds now: $typed" ;;
  esac
  await_log "^browser: title $agent_tab noted\$" "the agent's typing didn't reach the field"
  # The page watched from its card has its pointer: at the note it typed into, ringed and
  # tagged with the note's ref, over the page in the window the mouse goes through. It goes a
  # moment after.
  await_log "^browser: agent element $agent_tab [0-9]* at [0-9.]* [0-9.]* [1-9][0-9.]* [1-9][0-9.]*\$" \
    "the core didn't say where the note was"
  await_log "^browser: agent pointer $agent_tab $note at " "the agent's pointer didn't come to the note"
  ../tools/window-screenshot.sh "$pid" "$out/agent-pointer.png" || fail "no window to capture"
  await_log '^browser: agent pointer gone$' "the agent's pointer didn't go"

  # A page's frames are part of it: the outline has what's in a frame of the page's site and in
  # one of another, which Chromium puts in a process of its own, and the agent acts in both. A
  # password field in a frame is as off limits as one in the page. The page names itself once the
  # other site's frame has painted: until then Chromium hasn't the frame's drawing to find what's
  # under a click in, and gives the click to the page around it. A third site's frame inside the
  # other site's, a frame the page adds after it has loaded and one it fills by posting a form into
  # it, as SharePoint posts Word's editor into its frame, are in the outline and the text too, in
  # the page's order, and an empty helper frame is in neither. A click two frames deep lands
  # (#1444). The posted frame's editor, built as Word for the web's document body is, an editable
  # region of paragraphs and text runs, is a field: the outline names it and neither the outline
  # nor the text has what it holds, as with any field's value.
  printf '%s' '<p>Inner text</p><button>Inner button</button><input aria-label="Inner field">' \
    > "$out/site/inner.html"
  printf '%s' '<style>.skip { position: absolute; top: -2000px } .skip:focus { top: 0 }</style>
<a class=skip href=#cross onclick="document.body.append(&quot;skip clicked&quot;)">Skip to cross</a>
<p id=cross>Cross text</p>
<button onclick="document.body.append(&quot;cross clicked&quot;)">Cross button</button>
<input aria-label="Cross field" oninput="document.body.append(&quot;cross typed&quot;)">
<input type=password aria-label="Cross password"><script src="painted.js"></script>
<script>painted(() => parent.postMessage("painted", "*"));</script>' > "$out/site/cross.html"
  printf '%s' "<iframe src=http://nested.localhost:$port/deep.html></iframe>" \
    >> "$out/site/cross.html"
  printf '%s' '<p>Deep text</p>
<button onclick="document.body.append(&quot;deep clicked&quot;)">Deep button</button>' \
    > "$out/site/deep.html"
  printf '%s' '<p>Late text</p><script>top.postMessage("late", "*")</script>' \
    > "$out/site/late.html"
  printf '%s' '<p>Posted text</p>
<div contenteditable role=textbox aria-multiline=true aria-label="Document"><p class=Paragraph>
<span class=TextRun><span class=NormalTextRun>Editor text</span></span></p></div>
<script>top.postMessage("posted", "*")</script>' > "$out/site/posted.html"
  : > "$out/site/empty.html"
  printf '%s' "<title>loading</title><p>Top text</p><iframe src=inner.html></iframe>
<iframe src=http://127.0.0.1:$port/cross.html></iframe>
<iframe title='Helper frame' src=empty.html width=1 height=1></iframe>
<iframe name=posted></iframe><form target=posted action=http://127.0.0.1:$port/posted.html></form>
<div role=button style='position: absolute; left: -9999px'
  onclick='document.body.append(&quot;reached&quot;)'>Out of reach</div>
<script>const heard = new Set();
addEventListener('message', (event) => {
  if (event.data === 'painted') {
    document.title = 'frames';
    const late = document.createElement('iframe');
    late.src = 'http://127.0.0.1:$port/late.html';
    document.body.append(late);
    document.forms[0].submit();
  }
  if (event.data === 'late' || event.data === 'posted') heard.add(event.data);
  if (heard.size === 2) document.title = 'frames late';
  if (event.data === 'extension frame') document.title = 'frames with extension';
});</script>" > "$out/site/frames.html"
  outline=$(call open_tab "{\"url\":\"localhost:$port/frames.html\"}")
  printf '%s\n' "$outline" > "$out/mcp-frames.txt"
  case "$outline" in
    "Opened tab "*"Top text"*"Inner text"*"Inner button"*"Cross text"*"Cross button"*"(password: off limits)"*) ;;
    *) fail "the page with frames was outlined as: $outline" ;;
  esac
  frames_tab=$(printf '%s\n' "$outline" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  # The agent doesn't wait for that. The core gives a click to the frame it's in, so it lands
  # before the window has drawn the other site's frame, which in a background tab it may not
  # have yet (#1175).
  inner=$(ref_of button "Inner button")
  cross=$(ref_of button "Cross button")
  [ -n "$inner" ] && [ -n "$cross" ] && [ "$inner" != "$cross" ] ||
    fail "the frames' buttons had the refs '$inner' and '$cross'"
  clicked=$(call click "{\"tab\":$frames_tab,\"ref\":\"$cross\"}")
  case "$clicked" in
    *"Top text"*) fail "the click answered with the whole outline: $clicked" ;;
    "Clicked button “Cross button”."*"What changed since"*"cross clicked"*) ;;
    *) fail "clicking in the other site's frame answered: $clicked" ;;
  esac
  await_log "^browser: agent activity mcp-client $frames_tab click\$" \
    "the click in the frame didn't reach the activity log"
  field=$(ref_of textbox "Cross field")
  typed=$(call type "{\"tab\":$frames_tab,\"ref\":\"$field\",\"text\":\"by-the-agent\"}")
  case "$typed" in
    "Typed into “Cross field”."*"cross typed"*) ;;
    *) fail "typing in the other site's frame answered: $typed" ;;
  esac
  password=$(ref_of textbox "Cross password")
  refused=$(call type "{\"tab\":$frames_tab,\"ref\":\"$password\",\"text\":\"by-the-agent\"}")
  case "$refused" in
    "error: Password fields are off limits"*) ;;
    *) fail "typing into the frame's password field answered: $refused" ;;
  esac
  # The extension's frame is the extension's, not the page's: the page's text has its frames'
  # and not that one's.
  await_log "^browser: title $frames_tab frames with extension\$" \
    "the check's extension didn't put its frame in the frames page"
  await_log "^browser: title $frames_tab frames late\$" \
    "the frames added and filled after the load didn't load"
  later=$(call snapshot "{\"tab\":$frames_tab}")
  printf '%s\n' "$later" > "$out/mcp-frames-later.txt"
  case "$later" in
    *"Helper frame"*) fail "the outline had the empty helper frame: $later" ;;
    *"Editor text"*) fail "the outline had what the posted frame's editor holds: $later" ;;
    *"Top text"*"Inner text"*"Cross text"*"Deep text"*"Posted text"*'textbox "Document"'*"Late text"*) ;;
    *) fail "the frames page was outlined later as: $later" ;;
  esac
  read=$(call read_page "{\"tab\":$frames_tab}")
  printf '%s\n' "$read" > "$out/mcp-frames-text.txt"
  case "$read" in
    *"Extension text"*) fail "reading the frames page gave the extension's frame's text: $read" ;;
    *"Editor text"*) fail "reading the frames page gave what the posted frame's editor holds: $read" ;;
    *"Top text"*"Inner text"*"Cross text"*"Deep text"*"Posted text"*"Late text"*) ;;
    *) fail "reading the frames page answered: $read" ;;
  esac
  outline=$later
  clicked=$(call click "{\"tab\":$frames_tab,\"ref\":\"$(ref_of button "Deep button")\"}")
  case "$clicked" in
    "Clicked button “Deep button”."*"deep clicked"*) ;;
    *) fail "clicking two frames deep answered: $clicked" ;;
  esac
  # A skip link, as Office's frame has, is above the page until it's focused, where no click
  # reaches it: the agent's click focuses it first, as a keyboard user's Tab would (#2174). One off
  # the page that can't take focus is refused, rather than pressed where nothing is.
  clicked=$(call click "{\"tab\":$frames_tab,\"ref\":\"$(ref_of link "Skip to cross")\"}")
  case "$clicked" in
    "Clicked link “Skip to cross”."*"skip clicked"*) ;;
    *) fail "clicking a skip link answered: $clicked" ;;
  esac
  refused=$(call click "{\"tab\":$frames_tab,\"ref\":\"$(ref_of button "Out of reach")\"}")
  case "$refused" in
    "error: The element is outside the page"*) ;;
    *) fail "clicking a button off the page that can't take focus answered: $refused" ;;
  esac

  # select chooses an option by its text: in a select through the option's own action, and in a
  # dropdown of the page's own by clicking it open and then the option. A miss chooses nothing and
  # names the options.
  printf '%s' '<script src="painted.js"></script>
<label for=status>Status</label>
<select id=status><option>Open<option>Pending<option>Closed</select>
<span id=priority-label>Priority</span>
<div id=pick role=combobox tabindex=0 aria-labelledby=priority-label aria-controls=list
  aria-expanded=false>Low</div>
<ul id=list role=listbox aria-label=Priorities hidden>
<li role=option>Low<li role=option>High<li role=option>Urgent
</ul>
<input id=city role=combobox aria-label=City aria-owns=cities aria-expanded=true value=Ber-by-ada>
<ul id=cities role=listbox aria-label=Cities>
<li role=option>Berlin<li role=option>Bern
</ul>
<script>
cities.onclick = event => document.title = "city " + event.target.textContent;
const field = document.getElementById("status");
field.onchange = () => document.title = "status " + field.value;
const show = open => { list.hidden = !open; pick.ariaExpanded = open; };
pick.onclick = () => show(list.hidden);
list.onclick = event => {
  pick.textContent = event.target.textContent;
  show(false);
  document.title = "priority " + pick.textContent;
};
painted(() => document.title = "choose");
</script>' > "$out/site/choose.html"
  opened=$(call open_tab "{\"url\":\"localhost:$port/choose.html\"}")
  choose_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  [ -n "$choose_tab" ] || fail "opening the choosing page answered: $opened"
  await_log "^browser: title $choose_tab choose\$" "the choosing page didn't paint"
  outline=$(call snapshot "{\"tab\":$choose_tab}")
  printf '%s\n' "$outline" > "$out/mcp-choose.txt"
  status=$(ref_of combobox Status)
  priority=$(ref_of combobox Priority)
  case "$outline" in
    *"- combobox \"Status\" [$status] (holds \"Open\") (options: \"Open\", \"Pending\", \"Closed\")"*) ;;
    *) fail "the select's line didn't name its options: $(printf '%s\n' "$outline" | grep Status)" ;;
  esac
  [ -n "$status" ] && [ -n "$priority" ] ||
    fail "the choosing page's fields had the refs '$status' and '$priority'"
  # A picker whose list is its child (aria-owns), as Chromium's tree has it, shows its options with
  # refs. Its field is an input, whose text from the markup reads only as filled (#2355).
  city=$(printf '%s\n' "$outline" | grep -- '- combobox "City" \[')
  case "$city" in
    *Ber-by-ada*) fail "the picker's line gave its input's text from the markup: $city" ;;
    *'] (filled)' | *'] (expanded) (filled)') ;;
    *) fail "the picker's line didn't say only that it was filled: $city" ;;
  esac
  bern=$(ref_of option Bern)
  case "$outline" in
    *"- combobox \"City\" [e"*"- listbox \"Cities\" [e"*"- option \"Berlin\" [e"*"- option \"Bern\" [$bern]"*) ;;
    *) fail "the picker's own list wasn't in the outline: $(printf '%s\n' "$outline" | grep -A3 City)" ;;
  esac
  picked=$(call click "{\"tab\":$choose_tab,\"ref\":\"$bern\"}")
  case "$picked" in "Clicked option “Bern”."*) ;; *) fail "clicking the picker's option answered: $picked" ;; esac
  await_log "^browser: title $choose_tab city Bern\$" "clicking the picker's option didn't reach the page"
  chose=$(call select "{\"tab\":$choose_tab,\"ref\":\"$status\",\"option\":\"closed\"}")
  case "$chose" in
    "The combobox “Status” now holds “Closed”."*) ;;
    *) fail "choosing in the select answered: $chose" ;;
  esac
  await_log "^browser: title $choose_tab status Closed\$" \
    "choosing in the select didn't reach the page"
  await_log "^browser: agent activity mcp-client $choose_tab select_option\$" \
    "choosing an option didn't reach the activity log"
  chose=$(call select "{\"tab\":$choose_tab,\"ref\":\"$priority\",\"option\":\"High\"}")
  case "$chose" in
    "The combobox “Priority” now holds “High”."*) ;;
    *) fail "choosing in the page's dropdown answered: $chose" ;;
  esac
  await_log "^browser: title $choose_tab priority High\$" \
    "choosing in the dropdown didn't reach the page"
  missed=$(call select "{\"tab\":$choose_tab,\"ref\":\"$status\",\"option\":\"Missing\"}")
  options="Its options are “Open”, “Pending”, “Closed”."
  [ "$missed" = "error: There's no option “Missing” there. $options" ] ||
    fail "choosing an option the select hasn't answered: $missed"
  last=$(sed -n "s/^browser: title $choose_tab //p" "$log" | tail -1)
  [ "$last" = "priority High" ] || fail "a missed option changed the page, to $last"

  # A name with a no-break space beside a space, or with any other of Unicode's spaces, is one
  # space in the outline, and select takes it as the outline shows it, in a select and in a page's
  # own dropdown. A miss echoes what was asked as it was asked, and its options as the outline
  # shows them (#3190).
  printf '%s' '<script src="painted.js"></script>
<label for=account>Account</label>
<select id=account><option>Alpha 1234<option>Bravo 5678&nbsp; (closed)
<option>Charlie&#x3000;9012&#x2003;&#x202F;(open)</select>
<span id=branch-label>Branch</span>
<div id=branch role=combobox tabindex=0 aria-labelledby=branch-label aria-controls=branches
  aria-expanded=false>None</div>
<ul id=branches role=listbox aria-label=Branches hidden>
<li role=option>Alpha 1234<li role=option>Bravo 5678&nbsp; (closed)
<li role=option>Echo&#x2007;&#x205F;12&#x1680;(open)
</ul>
<script>
account.onchange = () => document.title = "account " + account.selectedIndex;
const show = open => { branches.hidden = !open; branch.ariaExpanded = open; };
branch.onclick = () => show(branches.hidden);
branches.onclick = event => {
  branch.textContent = event.target.textContent;
  show(false);
  document.title = "branch " + [...branches.children].indexOf(event.target);
};
painted(() => document.title = "spaces");
</script>' > "$out/site/spaces.html"
  opened=$(call open_tab "{\"url\":\"localhost:$port/spaces.html\"}")
  spaces_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  [ -n "$spaces_tab" ] || fail "opening the page of names with Unicode spaces answered: $opened"
  await_log "^browser: title $spaces_tab spaces\$" "the page of names with Unicode spaces didn't paint"
  outline=$(call snapshot "{\"tab\":$spaces_tab}")
  printf '%s\n' "$outline" > "$out/mcp-spaces.txt"
  account=$(ref_of combobox Account)
  branch=$(ref_of combobox Branch)
  [ -n "$account" ] && [ -n "$branch" ] ||
    fail "the page of names with Unicode spaces had the refs '$account' and '$branch'"
  shown='"Alpha 1234", "Bravo 5678 (closed)", "Charlie 9012 (open)"'
  # It holds its first option, the page's own with none marked selected, before its options.
  case "$outline" in
    *"- combobox \"Account\" [$account] (holds \"Alpha 1234\") (options: $shown)"*) ;;
    *) fail "the select's line didn't name its options with one space each, as select takes them: $(printf '%s\n' "$outline" | grep Account)" ;;
  esac
  # choose TEXT TITLE REF NAME: select's TEXT in the field REF answers that it holds TEXT, and the
  # page's title becomes TITLE.
  choose() {
    chose=$(call select "{\"tab\":$spaces_tab,\"ref\":\"$3\",\"option\":\"$1\"}")
    case "$chose" in
      "The combobox “$4” now holds “$1”."*) ;;
      *) fail "choosing “$1” in $4, as the outline names it, answered: $chose" ;;
    esac
    await_log "^browser: title $spaces_tab $2\$" "choosing “$1” in $4 didn't reach the page"
  }
  choose "Bravo 5678 (closed)" "account 1" "$account" Account
  choose "Charlie 9012 (open)" "account 2" "$account" Account
  choose "Bravo 5678 (closed)" "branch 1" "$branch" Branch
  choose "Echo 12 (open)" "branch 2" "$branch" Branch
  # Two spaces and a no-break space, which match no option, come back as they were sent.
  nbsp=$(printf '\302\240')
  missed=$(call select "{\"tab\":$spaces_tab,\"ref\":\"$account\",\"option\":\"Bravo  5678\\u00a0(gone)\"}")
  options="Its options are “Alpha 1234”, “Bravo 5678 (closed)”, “Charlie 9012 (open)”."
  [ "$missed" = "error: There's no option “Bravo  5678$nbsp(gone)” there. $options" ] ||
    fail "choosing an option the select hasn't, with two spaces and a no-break space, answered: $missed"
  last=$(sed -n "s/^browser: title $spaces_tab //p" "$log" | tail -1)
  [ "$last" = "branch 2" ] || fail "a missed option changed the page, to $last"
  # Closed, as the conversation pictures each tab the agent keeps, and one more would put the
  # sign-in page's picture below the window's bottom, where the check's click can't reach it.
  call close_tab "{\"tab\":$spaces_tab}" > /dev/null

  # Choice tiles whose radios the page hid, with display: none and named by a label's "for" or
  # inside it, show as those radios, named by the label's text and with which is chosen, and a
  # click on one by its ref chooses it. A label with no control stays text (#2355).
  printf '%s' '<script src="painted.js"></script>
<fieldset><legend>Account type</legend>
<input type=radio name=kind id=personal value=personal hidden checked>
<label for=personal><h3>Personal</h3><p>For you</p></label>
<label><input type=radio name=kind value=business style="display: none"><h3>Business</h3>
<p>For a team</p></label>
<label>No control</label></fieldset>
<script>
for (const radio of document.querySelectorAll("input")) {
  radio.onchange = () => document.title = "kind " + radio.value;
}
painted(() => document.title = "tiles");
</script>' > "$out/site/tiles.html"
  opened=$(call open_tab "{\"url\":\"localhost:$port/tiles.html\"}")
  tiles_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  [ -n "$tiles_tab" ] || fail "opening the tiles page answered: $opened"
  await_log "^browser: title $tiles_tab tiles\$" "the tiles page didn't paint"
  outline=$(call snapshot "{\"tab\":$tiles_tab}")
  printf '%s\n' "$outline" > "$out/mcp-tiles.txt"
  personal=$(ref_of radio "Personal For you")
  business=$(ref_of radio "Business For a team")
  [ -n "$personal" ] && [ -n "$business" ] ||
    fail "the hidden radios' tiles had the refs '$personal' and '$business'"
  case "$outline" in
    *"radio \"Personal For you\" [$personal] (checked)"*) ;;
    *) fail "the chosen tile didn't show as checked" ;;
  esac
  case "$outline" in
    *"radio \"Business For a team\" [$business] (checked)"*) fail "both tiles showed as checked" ;;
    *'"No control" ['*) fail "a label with no control had a ref" ;;
  esac
  clicked=$(call click "{\"tab\":$tiles_tab,\"ref\":\"$business\"}")
  case "$clicked" in
    "Clicked radio “Business For a team”."*) ;;
    *) fail "clicking a tile answered: $clicked" ;;
  esac
  await_log "^browser: title $tiles_tab kind business\$" "clicking a tile didn't choose it"
  outline=$(call snapshot "{\"tab\":$tiles_tab}")
  printf '%s\n' "$outline" >> "$out/mcp-tiles.txt"
  case "$outline" in
    *"radio \"Business For a team\" [$business] (checked)"*) ;;
    *) fail "the tile clicked didn't show as checked" ;;
  esac
  case "$outline" in
    *"radio \"Personal For you\" [$personal] (checked)"*) fail "the tile left still showed as checked" ;;
  esac

  # A console's list page: its table's rows and its side menu's items, which a person clicks, have
  # refs, and a click on a row opens it. Ticking the box enables Continue, and the click's answer
  # says what changed in each, not only that each did (#2355).
  printf '%s' '<script src="painted.js"></script>
<ul aria-label=Menu><li onclick="document.title = this.textContent">Instances<li
 onclick="document.title = this.textContent">Volumes</ul>
<table aria-label=Instances><tr><th>Name<th>State</tr></table>
<p><label><input type=checkbox id=agree> I agree</label> <button id=next disabled>Continue</button>
<script>
const table = document.querySelector("table");
for (let i = 1; i <= 8; i++) {
  const row = table.insertRow();
  row.insertCell().textContent = "web-" + i;
  row.insertCell().textContent = i % 2 ? "Running" : "Stopped";
  row.onclick = () => document.title = "opened web-" + i;
}
agree.onchange = () => next.disabled = !agree.checked;
painted(() => document.title = "console");
</script>' > "$out/site/console.html"
  opened=$(call open_tab "{\"url\":\"localhost:$port/console.html\"}")
  console_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  [ -n "$console_tab" ] || fail "opening the console page answered: $opened"
  await_log "^browser: title $console_tab console\$" "the console page didn't paint"
  outline=$(call snapshot "{\"tab\":$console_tab}")
  printf '%s\n' "$outline" > "$out/mcp-console.txt"
  # A row is a line, its ref first and then its cells' (#3657).
  row=$(printf '%s\n' "$outline" | sed -n 's/.*- row \[\(e[0-9]*\)\]: "web-3" .*/\1/p' | head -1)
  volumes=$(ref_of listitem Volumes)
  agree=$(ref_of checkbox "I agree")
  [ -n "$row" ] && [ -n "$volumes" ] && [ -n "$agree" ] ||
    fail "the console page's row, menu item and box had the refs '$row', '$volumes' and '$agree': $out/mcp-console.txt"
  call click "{\"tab\":$console_tab,\"ref\":\"$row\"}" > /dev/null
  await_log "^browser: title $console_tab opened web-3\$" "a click on a row by its ref didn't open it"
  call click "{\"tab\":$console_tab,\"ref\":\"$volumes\"}" > /dev/null
  await_log "^browser: title $console_tab Volumes\$" "a click on a menu item by its ref didn't reach it"
  ticked=$(call click "{\"tab\":$console_tab,\"ref\":\"$agree\"}")
  case "$ticked" in
    *"Changed:"*'button "Continue" [e'*']: now enabled'*) ;;
    *) fail "ticking the box answered without saying Continue is now enabled: $ticked" ;;
  esac

  # It scrolls in what it names by ref, a panel and another site's frame, as a wheel there does,
  # and once one is at its end the answer says nothing scrolled. The page around them is too short
  # to scroll, so only they can, and a scroll of the page says nothing did (#1445).
  printf '%s' '<p style="height:600px">Tall text</p><script src="painted.js"></script>
<script>
painted(() => parent.postMessage("painted", "*"));
addEventListener("scroll", () => parent.postMessage("tall scrolled", "*"), { once: true });
</script>' > "$out/site/tall.html"
  printf '%s' "<title>loading</title><div role=region aria-label=Panel
 style='height:150px; overflow:auto' onscroll=\"document.title = 'scroll panel moved'\">
<p style='height:400px'>Panel text</p></div>
<iframe title='Tall frame' src=http://127.0.0.1:$port/tall.html height=150></iframe>
<script>addEventListener('message', (event) => {
  if (event.data === 'painted') document.title = 'scroll';
  if (event.data === 'tall scrolled') document.title = 'scroll frame moved';
});</script>" > "$out/site/scroll.html"
  outline=$(call open_tab "{\"url\":\"localhost:$port/scroll.html\"}")
  scroll_tab=$(printf '%s\n' "$outline" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  [ -n "$scroll_tab" ] || fail "open_tab of the scrolling page answered: $outline"
  await_log "^browser: title $scroll_tab scroll\$" "the scrolling page's frame didn't paint"
  outline=$(call snapshot "{\"tab\":$scroll_tab}")
  printf '%s\n' "$outline" > "$out/mcp-scroll.txt"
  panel=$(ref_of region Panel)
  frame=$(ref_of Iframe "Tall frame")
  [ -n "$panel" ] && [ -n "$frame" ] || fail "the scrolling page was outlined as: $outline"
  # Scrolls in <ref>, named <name>, which titles the page <title> once it moves, then on to its end.
  scroll_to_end() {
    scrolled=$(call scroll "{\"tab\":$scroll_tab,\"ref\":\"$1\",\"direction\":\"down\"}")
    case "$scrolled" in
      "Scrolled down in “$2”."*) ;;
      *) fail "scrolling in $2 answered: $scrolled" ;;
    esac
    await_log "^browser: title $scroll_tab $3\$" "scrolling in $2 didn't move it"
    for _ in 1 2 3 4 5 6; do
      scrolled=$(call scroll "{\"tab\":$scroll_tab,\"ref\":\"$1\",\"direction\":\"down\"}")
      case "$scrolled" in
        "error: Nothing scrolled: it's as far down as it goes"*) return ;;
        "Scrolled down in “$2”."*) ;;
        *) fail "scrolling on in $2 answered: $scrolled" ;;
      esac
    done
    fail "scrolling in $2 never reached its end"
  }
  scroll_to_end "$panel" Panel "scroll panel moved"
  scroll_to_end "$frame" "Tall frame" "scroll frame moved"
  scrolled=$(call scroll "{\"tab\":$scroll_tab}")
  case "$scrolled" in
    "error: Nothing scrolled"*) ;;
    *) fail "scrolling the page too short to scroll answered: $scrolled" ;;
  esac
  call close_tab "{\"tab\":$scroll_tab}" > /dev/null

  # Scrolling to a field leaves room around it: one already whole on screen, just under a sticky
  # header that covers its label, comes down far enough for the label to show (#2355).
  printf '%s' '<script src="painted.js"></script>
<header style="position: sticky; top: 0; height: 60px; background: #ddd">Header</header>
<div style="height: 1500px"></div>
<label id=label for=email>Email</label><br><input id=email style="height: 30px">
<div style="height: 1500px"></div>
<script>
// The field whole, just under the header, which covers its label.
scrollBy(0, email.getBoundingClientRect().top - 61);
onscroll = () => {
  const shows = label.getBoundingClientRect().top >= 60;
  const whole = email.getBoundingClientRect().bottom <= innerHeight;
  if (shows && whole) document.title = "label shows";
};
painted(() => document.title = "sticky");
</script>' > "$out/site/sticky.html"
  opened=$(call open_tab "{\"url\":\"localhost:$port/sticky.html\"}")
  sticky_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  [ -n "$sticky_tab" ] || fail "opening the sticky header's page answered: $opened"
  await_log "^browser: title $sticky_tab sticky\$" "the sticky header's page didn't paint"
  outline=$(call snapshot "{\"tab\":$sticky_tab}")
  email=$(ref_of textbox Email)
  [ -n "$email" ] || fail "the sticky header's page had no ref for its field: $outline"
  scrolled=$(call scroll "{\"tab\":$sticky_tab,\"ref\":\"$email\"}")
  case "$scrolled" in
    "Scrolled to “Email”."*) ;;
    *) fail "scrolling to the field under the header answered: $scrolled" ;;
  esac
  await_log "^browser: title $sticky_tab label shows\$" \
    "scrolling to the field left its label under the sticky header"
  call close_tab "{\"tab\":$sticky_tab}" > /dev/null

  # Its keys go to the page alone: type with submit and press_key's Enter submit the form, Tab
  # moves the keyboard to the next field, and Escape reaches the page. Escape the page doesn't keep
  # is ilo's Stop while a page loads, and the agent's runs no shortcut of ilo's: the page goes on
  # loading, and the tabs and the app stay as they were.
  printf '%s' '<script src="painted.js"></script>
<form id=form><input id=first aria-label="First field"><input id=second aria-label="Second field">
<button>Send</button></form>
<script>
let sent = 0;
form.onsubmit = event => {
  event.preventDefault();
  document.title = "submitted " + ++sent + " " + first.value;
};
// Tab brings the keyboard here, and a moment later the page that loads until its tab closes.
second.onfocus = () => {
  document.title = "second focused";
  setTimeout(() => location = "keys-loading.html", 2000);
};
painted(() => document.title = "keys");
</script>' > "$out/site/keys.html"
  # Its picture never comes, so the page loads until its tab closes, with its field focused.
  printf '%s' '<title>still loading</title><input id=field aria-label="Loading field">
<script>
field.focus();
// Heard, and not kept.
addEventListener("keydown", event => event.key == "Escape" && (document.title = "escape heard"));
</script><img src="wait">' > "$out/site/keys-loading.html"
  tabs() { call list_tabs '{}' | grep -c '^. [0-9]*  '; }
  outline=$(call open_tab "{\"url\":\"localhost:$port/keys.html\"}")
  keys_tab=$(printf '%s\n' "$outline" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  [ -n "$keys_tab" ] || fail "open_tab of the keys page answered: $outline"
  await_log "^browser: title $keys_tab keys\$" "the keys page didn't paint"
  outline=$(call snapshot "{\"tab\":$keys_tab}")
  first=$(ref_of textbox "First field")
  [ -n "$first" ] || fail "the keys page's outline had no ref for its first field: $outline"
  typed=$(call type "{\"tab\":$keys_tab,\"ref\":\"$first\",\"text\":\"by-the-agent\",\"submit\":true}")
  case "$typed" in
    "Typed into “First field” and pressed Enter."*) ;;
    *) fail "type with submit answered: $typed" ;;
  esac
  await_log "^browser: title $keys_tab submitted 1 by-the-agent\$" "type with submit didn't submit the form"
  pressed=$(call press_key "{\"tab\":$keys_tab,\"key\":\"Enter\"}")
  case "$pressed" in "Pressed Enter."*) ;; *) fail "press_key Enter answered: $pressed" ;; esac
  await_log "^browser: title $keys_tab submitted 2 by-the-agent\$" "press_key Enter didn't submit the form"
  await_log "^browser: agent activity mcp-client $keys_tab press_key\$" \
    "press_key didn't reach the activity log"
  pressed=$(call press_key "{\"tab\":$keys_tab,\"key\":\"Tab\"}")
  case "$pressed" in "Pressed Tab."*) ;; *) fail "press_key Tab answered: $pressed" ;; esac
  await_log "^browser: title $keys_tab second focused\$" "press_key Tab didn't move the keyboard on"
  await_log "^browser: title $keys_tab still loading\$" "the page that keeps loading didn't come"
  await_log "^browser: page $keys_tab back 1 forward 0 loading 1 http://localhost:$port/keys-loading\\.html\$" \
    "the page that keeps loading didn't show as loading"
  stopped="^browser: page $keys_tab back [01] forward [01] loading 0 "
  stops=$(grep -c "$stopped" "$log" || true)
  before=$(tabs)
  # The agent waits out the load before it presses, 15 s (kLoadTimeout in agent_page.cc), and for
  # the page to settle after, 5 s (kBusy), so the answer comes after the 30 s next_answer waits.
  request press_key "{\"tab\":$keys_tab,\"key\":\"Escape\"}" >&4
  await_log "^browser: title $keys_tab escape heard\$" "press_key Escape didn't reach the page"
  pressed=$(next_answer 60) ||
    fail "the browser didn't answer press_key Escape"
  pressed=$(printf '%s\n' "$pressed" | answered "press_key Escape")
  case "$pressed" in "Pressed Escape."*) ;; *) fail "press_key Escape answered: $pressed" ;; esac
  kill -0 "$pid" 2>/dev/null || fail "the app exited as the agent pressed Escape"
  [ "$(grep -c "$stopped" "$log" || true)" = "$stops" ] ||
    fail "the agent's Escape, which the page didn't keep, ran ilo's Stop"
  [ "$(tabs)" = "$before" ] || fail "the agent's Escape changed the tabs in its Space"
  closed=$(call close_tab "{\"tab\":$keys_tab}")
  case "$closed" in "Closed tab $keys_tab."*) ;; *) fail "closing the keys page answered: $closed" ;; esac

  # Its Tab past its page's last control stays in the page: the core asks the app only of a
  # user's tab, so the user's ring and keyboard don't move (#3220).
  printf '%s' '<script src="painted.js"></script><input id=start aria-label="First field">
<input id=last aria-label="Last field">
<script>
let focused = 0;
start.onfocus = () => document.title = "start focused " + ++focused;
last.onfocus = () => document.title = "last focused";
painted(() => document.title = "end");
</script>' > "$out/site/end.html"
  outline=$(call open_tab "{\"url\":\"localhost:$port/end.html\"}")
  end_tab=$(printf '%s\n' "$outline" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  [ -n "$end_tab" ] || fail "open_tab of the end page answered: $outline"
  await_log "^browser: title $end_tab end\$" "the end page didn't paint"
  rings=$(grep -c '^browser: sidebar keyed \|^browser: strip keyed \|^browser: tab [0-9]* focus left its page.*kept by the app$' "$log" || true)
  call press_key "{\"tab\":$end_tab,\"key\":\"Tab\"}" > /dev/null
  await_log "^browser: title $end_tab start focused 1\$" "the agent's Tab didn't reach its page's field"
  call press_key "{\"tab\":$end_tab,\"key\":\"Tab\"}" > /dev/null
  await_log "^browser: title $end_tab last focused\$" "the agent's second Tab didn't reach the last field"
  call press_key "{\"tab\":$end_tab,\"key\":\"Tab\"}" > /dev/null
  await_log "^browser: tab $end_tab focus left its page at the end, kept by the agent's page\$" \
    "the agent's Tab past its page's last control wasn't kept in its page"
  # Kept, the page has the keyboard with nothing in it focused, so its next Tab starts it over.
  call press_key "{\"tab\":$end_tab,\"key\":\"Tab\"}" > /dev/null
  await_log "^browser: title $end_tab start focused 2\$" \
    "the agent's Tab after its page's end didn't give it its first control again"
  [ "$(grep -c '^browser: sidebar keyed \|^browser: strip keyed \|^browser: tab [0-9]* focus left its page.*kept by the app$' "$log" || true)" = "$rings" ] ||
    fail "the agent's Tab past its page's end reached the user's ring"
  call close_tab "{\"tab\":$end_tab}" > /dev/null

  # Its typing is keys, as a keyboard's, so a field's own key handlers filter and format it as they
  # do a user's, and a line break never sends or submits: a chat composer whose Enter sends gets
  # the agent's lines as one unsent message. Text that isn't one key arrives exact, with no
  # composition left open, and a long text loses nothing on a busy page (#2355). Each field says
  # what it holds in the title; the run writes what each came to, then checks them.
  printf '%s' '<meta charset=utf-8><script>
function report(name, text) { document.title = name + " " + text.replace(/\n/g, "|"); }
let composing = false;
addEventListener("compositionstart", () => composing = true);
addEventListener("compositionend", () => composing = false);
const digitsOnly = (event) => {
  if (event.key.length === 1 && !/[0-9]/.test(event.key)) event.preventDefault();
};
let keyups = 0;
function busy() { const until = performance.now() + 2; while (performance.now() < until); }
const expected = "日本語 😀👩‍👩‍👧 é";
</script>
<div contenteditable role=textbox aria-label=Composer
  onkeydown="if (event.key === &quot;Enter&quot; &amp;&amp; !event.shiftKey) { event.preventDefault(); report(&quot;sent&quot;, this.innerText); }"
  onkeypress="if (event.keyCode === 13) { event.preventDefault(); report(&quot;sent&quot;, this.innerText); }"
  oninput="report(&quot;composer&quot;, this.innerText.split(&quot;\n&quot;).filter(Boolean).join(&quot;\n&quot;))"></div>
<textarea aria-label=Notes oninput="report(&quot;notes&quot;, this.value)"></textarea>
<input aria-label=Unicode oninput="report(&quot;unicode&quot;, (this.value === expected ? &quot;exact&quot; : &quot;wrong &quot; + this.value.length) + (composing ? &quot; composing&quot; : &quot; closed&quot;))">
<input aria-label=Digits onkeydown="digitsOnly(event)" oninput="report(&quot;digits&quot;, this.value)">
<input aria-label=Short maxlength=5 oninput="report(&quot;short&quot;, this.value)">
<input aria-label=Card onkeydown="digitsOnly(event)"
  oninput="this.value = this.value.replace(/\D/g, &quot;&quot;).slice(0, 16).replace(/(\d{4})(?=\d)/g, &quot;$1 &quot;); report(&quot;card&quot;, this.value)">
<input aria-label=Expiry onkeydown="digitsOnly(event)"
  oninput="const d = this.value.replace(/\D/g, &quot;&quot;).slice(0, 4); this.value = d.length > 2 ? d.slice(0, 2) + &quot;/&quot; + d.slice(2) : d; report(&quot;expiry&quot;, this.value)">
<input aria-label=Suggest onkeyup="if (event.key.length === 1 &amp;&amp; !event.ctrlKey &amp;&amp; !event.metaKey) report(&quot;suggest&quot;, ++keyups + &quot; &quot; + this.value)">
<input aria-label=Long oninput="if (this.value.length >= 5000) report(&quot;long&quot;, this.value.length + &quot; &quot; + (this.value === &quot;abcdefghij&quot;.repeat(500)))">
<input aria-label=Busy onkeydown="busy()" onkeypress="busy()"
  oninput="busy(); if (this.value.length >= 2000) report(&quot;busy&quot;, this.value.length + &quot; &quot; + (this.value === &quot;0123456789&quot;.repeat(200)))">
<script>document.title = "typing";</script>' > "$out/site/typing.html"
  outline=$(call open_tab "{\"url\":\"localhost:$port/typing.html\"}")
  typing_tab=$(printf '%s\n' "$outline" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  [ -n "$typing_tab" ] || fail "open_tab of the typing page answered: $outline"
  await_log "^browser: title $typing_tab typing\$" "the typing page didn't load"
  outline=$(call snapshot "{\"tab\":$typing_tab}")
  now_ms() { perl -MTime::HiRes=time -e 'printf "%d\n", time * 1000'; }
  : > "$out/mcp-typing.txt"
  typing_wrong=
  # Types $2 into the field named $1 and checks that what its title last said starts with $3.
  type_and_see() {
    field=$(ref_of textbox "$1")
    [ -n "$field" ] || fail "the typing page's outline had no $1 field: $outline"
    started=$(now_ms)
    typed=$(call type "{\"tab\":$typing_tab,\"ref\":\"$field\",\"text\":\"$2\"}")
    took=$(($(now_ms) - started))
    name=$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')
    seen=
    for _ in $(seq 30); do
      seen=$(grep -E "^browser: title $typing_tab ($name|sent) " "$log" | tail -1 |
        sed "s/^browser: title $typing_tab //")
      [ "$seen" = "$name $3" ] && break
      sleep 0.1
    done
    printf '%s: %s in %s ms, answered %s\n' "$1" "${seen:-nothing}" "$took" \
      "$(printf '%s' "$typed" | head -1)" | tee -a "$out/mcp-typing.txt"
    [ "$seen" = "$name $3" ] || typing_wrong="$typing_wrong; $1 came to '${seen:-nothing}', not '$name $3'"
  }
  type_and_see Composer 'line one\nline two\n\nline four' 'line one|line two|line four'
  type_and_see Notes 'first\nsecond\n' 'first|second|'
  type_and_see Unicode '日本語 😀👩‍👩‍👧 é' 'exact closed'
  type_and_see Digits 'a1b2c3' '123'
  type_and_see Short 'abcdefgh' 'abcde'
  type_and_see Card '4242424242424242' '4242 4242 4242 4242'
  type_and_see Expiry '1229' '12/29'
  type_and_see Suggest 'abc' '3 abc'
  type_and_see Long "$(printf 'abcdefghij%.0s' $(seq 500))" '5000 true'
  type_and_see Busy "$(printf '0123456789%.0s' $(seq 200))" '2000 true'
  grep -q "^browser: title $typing_tab sent " "$log" && typing_wrong="$typing_wrong; a line break sent the composer's message"
  [ -z "$typing_wrong" ] || fail "the agent's typing came out wrong${typing_wrong}"
  call close_tab "{\"tab\":$typing_tab}" > /dev/null

  # An extension's controls in a page aren't the page's, and the agent can't act on them (#2355).
  printf '%s' '<title>covered loading</title>
<button style="position: absolute; left: 20px; top: 20px; width: 200px; height: 60px"
  onclick="document.body.append(&quot;covered clicked&quot;)">Covered button</button>
<input aria-label="Page field" style="position: absolute; left: 20px; top: 200px"
  onkeydown="document.title = (document.documentElement.dataset.heard ? &quot;extension heard &quot; : &quot;page heard &quot;) + event.key"
  oninput="document.title = (document.documentElement.dataset.heard ? &quot;extension heard &quot; : &quot;page heard &quot;) + this.value">
<button style="position: absolute; left: 20px; top: 260px"
  onclick="document.querySelector(&quot;iframe:not([title])&quot;).focus(); document.title = &quot;extension focused&quot;">Focus extension</button>
<script>const heard = new Set();
addEventListener("message", (event) => {
  heard.add(event.data);
  if (heard.has("extension frame") && heard.has("inner covered")) document.title = "covered";
});</script>' > "$out/site/covered.html"
  printf '%s' "<iframe title='Inner frame' src=http://127.0.0.1:$port/covered-inner.html
  style='position: absolute; left: 20px; top: 360px; width: 400px; height: 200px'></iframe>" \
    >> "$out/site/covered.html"
  printf '%s' '<p>Inner covered text</p><div style="height: 3000px"></div>
<script>addEventListener("message", (event) => {
  if (event.data === "extension frame") top.postMessage("inner covered", "*");
});</script>' > "$out/site/covered-inner.html"
  outline=$(call open_tab "{\"url\":\"localhost:$port/covered.html\"}")
  covered_tab=$(printf '%s\n' "$outline" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  [ -n "$covered_tab" ] || fail "open_tab of the covered page answered: $outline"
  await_log "^browser: title $covered_tab covered\$" "the check's extension didn't cover the covered page"
  outline=$(call snapshot "{\"tab\":$covered_tab}")
  printf '%s\n' "$outline" > "$out/mcp-covered.txt"
  case "$outline" in
    *"Extension button"*) fail "the outline had the extension's button: $outline" ;;
    *'button "Covered button"'*'textbox "Page field"'*) ;;
    *) fail "the covered page was outlined as: $outline" ;;
  esac
  refused=$(call click "{\"tab\":$covered_tab,\"ref\":\"$(ref_of button "Covered button")\"}")
  case "$refused" in
    "error: An extension's control covers that spot"*) ;;
    *) fail "clicking the button the extension's frame covers answered: $refused" ;;
  esac
  await_log "^browser: agent activity mcp-client $covered_tab click refused: An extension's control covers" \
    "the refused click didn't reach the activity log"
  clicked=$(call click "{\"tab\":$covered_tab,\"ref\":\"$(ref_of textbox "Page field")\"}")
  case "$clicked" in "Clicked"*) ;; *) fail "clicking the page's field answered: $clicked" ;; esac
  pressed=$(call press_key "{\"tab\":$covered_tab,\"key\":\"ArrowDown\"}")
  case "$pressed" in "Pressed ArrowDown."*) ;; *) fail "press_key in the page's field answered: $pressed" ;; esac
  await_log "^browser: title $covered_tab [a-z]* heard ArrowDown\$" "the agent's key didn't reach the page"
  grep -q "^browser: title $covered_tab extension heard " "$log" &&
    fail "the extension's key listener heard the agent's key"
  typed=$(call type "{\"tab\":$covered_tab,\"ref\":\"$(ref_of textbox "Page field")\",\"text\":\"by-the-agent\"}")
  case "$typed" in "Typed into “Page field”."*) ;; *) fail "typing in the page's field answered: $typed" ;; esac
  await_log "^browser: title $covered_tab [a-z]* heard by-the-agent\$" "the agent's typing didn't reach the page"
  grep -q "^browser: title $covered_tab extension heard " "$log" &&
    fail "the extension's listeners heard the agent's typing"
  read=$(call read_page "{\"tab\":$covered_tab}")
  printf '%s\n' "$read" > "$out/mcp-covered-text.txt"
  case "$read" in
    *"Extension button"*) fail "reading the covered page gave the extension's button: $read" ;;
    *"Covered button"*"Inner covered text"*) ;;
    *) fail "reading the covered page answered: $read" ;;
  esac
  # The wheel finds what's under it in every frame it goes into, as a click does.
  refused=$(call scroll "{\"tab\":$covered_tab,\"ref\":\"$(ref_of Iframe "Inner frame")\",\"direction\":\"down\"}")
  case "$refused" in
    "error: An extension's control covers that spot"*) ;;
    *) fail "scrolling where the extension covers the other site's frame answered: $refused" ;;
  esac
  await_log "^browser: agent activity mcp-client $covered_tab scroll refused: An extension's control covers" \
    "the refused scroll didn't reach the activity log"
  clicked=$(call click "{\"tab\":$covered_tab,\"ref\":\"$(ref_of button "Focus extension")\"}")
  case "$clicked" in "Clicked"*) ;; *) fail "clicking Focus extension answered: $clicked" ;; esac
  await_log "^browser: title $covered_tab extension focused\$" "the page didn't focus the extension's frame"
  refused=$(call press_key "{\"tab\":$covered_tab,\"key\":\"Enter\"}")
  case "$refused" in
    "error: An extension's control has the keyboard"*) ;;
    *) fail "press_key while the extension's frame had the keyboard answered: $refused" ;;
  esac
  await_log "^browser: agent activity mcp-client $covered_tab press_key refused: An extension's control has" \
    "the refused key didn't reach the activity log"
  call close_tab "{\"tab\":$covered_tab}" > /dev/null

  # It sends HTTP requests from its tab to the tab's own site, with the tab's session but never
  # seeing it: reads go at once, their answers without the cookie the site sets or the token in its
  # body, and a change isn't part of Work on pages: ilo refuses it and it's never sent.
  sent=$(call request "{\"tab\":$frames_tab,\"requests\":[{\"method\":\"GET\",\"path\":\"/api/me\"},{\"method\":\"GET\",\"path\":\"/api/me\"}]}")
  # The frame's password field, which the outline has, puts a line before the answers: the page
  # asks for a password, so the site may have answered as to someone signed out (#3661).
  case "$sent" in
    "Tab $frames_tab's page asks for a password, as a sign-in page does"*"hand_off"*"1. GET /api/me → 200 application/json"*"Ada"*"2. GET /api/me → 200"*) ;;
    *) fail "request answered: $sent" ;;
  esac
  case "$sent" in *s3cret* | *tok-*) fail "the request's answer held the session: $sent" ;; esac
  served "^GET chromium localhost:$port/api/me cookie .*api=s3cret$port" \
    "the agent's request didn't carry the tab's session" "^GET chromium localhost:$port/api/me "
  await_log "^browser: agent activity mcp-client $frames_tab request\$" \
    "the request didn't reach the activity log"
  stops='^browser: agent mcp-client stopped Request at Work on pages: localhost$'
  unsent="^browser: agent logged mcp-client request localhost refused: Sending changes to a site without its page isn't part of Work on pages\\.\$"
  posted="^POST localhost:$port/api/items cookie .*api=s3cret$port[; ]"
  # A change without the agent's sentence of what it does is refused for that, before the task's
  # access comes into it, and nothing is sent (#3548).
  before=$(grep -c "$stops" "$log" || true)
  unsaid=$(call request "{\"tab\":$frames_tab,\"requests\":[{\"method\":\"POST\",\"path\":\"/api/items\",\"body\":{\"title\":\"Milk\"}}]}")
  case "$unsaid" in
    'error: Request 1 (POST /api/items) may change something, so it needs a `what`'*'Nothing was sent.') ;;
    *) fail "a change without a sentence answered: $unsaid" ;;
  esac
  [ "$(grep -c "$stops" "$log" || true)" = "$before" ] ||
    fail "a change without a sentence was stopped for the task's access, not for its sentence"
  milk='{"method":"POST","path":"/api/items","body":{"title":"Milk"},"what":"Add Milk to the shared shopping list under Dairy, so whoever shops next picks it up; it changes nothing else on the list and sends nothing else."}'
  # Sends the changes $1, $2 of them, from the frames' tab. Sending a change without its page isn't
  # part of Work on pages, so ilo refuses them at once: the call's answer says so and that only the
  # user changes a task's access, the stop is logged, the activity log has each change as not sent,
  # and nothing reaches the site. Sets changed to the answer.
  change_stopped() {
    before=$(grep -c "$stops" "$log" || true)
    noted=$(grep -c "$unsent" "$log" || true)
    reached=$(grep -c '^\(POST\|PUT\|PATCH\|DELETE\) ' "$out/site.log" || true)
    case "$2" in 1) what="a change" ;; *) what="$2 changes" ;; esac
    changed=$(call request "{\"tab\":$frames_tab,\"requests\":[$1]}")
    [ "$changed" = "error: ilo stopped this: make $what on localhost. It isn't part of this task's access (Work on pages). Only the user can change a task's access, in ilo. Don't try another way to the same end; if they change it, the same call will go through." ] ||
      fail "$what at Work on pages answered: $changed"
    await_more "$stops" "$before" || fail "$what refused at Work on pages wasn't logged as ilo's stop"
    await_more "$unsent" "$((noted + $2 - 1))" ||
      fail "the activity log didn't list each of $what as not sent: $(grep '^browser: agent logged mcp-client request ' "$log" | tail -2)"
    [ "$(grep -c '^\(POST\|PUT\|PATCH\|DELETE\) ' "$out/site.log" || true)" = "$reached" ] ||
      fail "$what refused at Work on pages reached the site: $(tail -1 "$out/site.log")"
  }
  change_stopped "$milk" 1
  # A delete is a change as a POST is, whatever its sentence says.
  draft='{"method":"DELETE","path":"/api/items/7","what":"Tidy up the old draft so your list is cleaner."}'
  change_stopped "$draft" 1
  # Settings › Agents lists each request by what it did, and a press opens it to where it went and
  # what it sent, or, for a change ilo refused, that it wasn't sent (#2326). Each row's name holds
  # its entry's id, which counts up, and its method.
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  settings_drew
  if ! grep '^browser: settings shown ' "$log" | tail -1 | grep -q ' Agents$'; then
    send clickkey $(drawn "toolbar Agents")
    expect '^browser: settings tab Agents$' "a click on Agents didn't show its section"
  fi
  settings_drew Agents
  drawn 'request [0-9]* POST' > /dev/null || fail "Settings › Agents drew no change"
  refused_id=$(sed -n 's/^browser: settings request \([0-9]*\) POST at .*/\1/p' "$log" | sort -nu | tail -1)
  read_id=$(sed -n 's/^browser: settings request \([0-9]*\) GET at .*/\1/p' "$log" | sort -nu | tail -1)
  [ -n "$read_id" ] && [ -n "$refused_id" ] ||
    fail "Settings › Agents didn't list the read and the refused change: $(grep '^browser: settings request ' "$log")"
  # Opens the request $1, sent with the method $2, and checks its line says it shows $3. The oldest
  # first, the lowest, so a row opened doesn't move the next one. The name is whole, as in_sight
  # finds a scrolled row by its fixed text.
  open_request() {
    in_sight "request $1 $2" || fail "Settings › Agents didn't draw request $1"
    send clickkey $at
    expect "^browser: settings request $1 opened " "a click on request $1 didn't open it"
    shown=$(sed -n "s/^browser: settings request $1 opened //p" "$log" | tail -1)
    [ "$shown" = "$3" ] || fail "request $1 opened to show \"$shown\", not \"$3\""
  }
  open_request "$read_id" GET "to http://localhost:$port/api/me, sent: "
  open_request "$refused_id" POST "to http://localhost:$port/api/items, not sent: title: Milk"
  # The agent's sentence, whole and first, as its words; the row's head stays its method and path.
  said=$(sed -n "s/^browser: settings request $refused_id said: //p" "$log" | tail -1)
  [ "$said" = "$(printf '%s' "$milk" | python3 -I -c 'import json, sys; print(json.load(sys.stdin)["what"])')" ] ||
    fail "request $refused_id opened to the agent's sentence \"$said\""
  ../tools/window-screenshot.sh "$pid" "$out/settings-agents-requests.png" || fail "no window to capture"
  # Back to General, for the visits below to click Agents.
  send clickkey $(drawn "toolbar General")
  expect '^browser: settings tab General$' "a click on General didn't show its section"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
  # A POST whose override header says GET is a change too, as a server may take either: it goes
  # nowhere, and Settings › Agents names it by the method sent.
  oats='{"method":"POST","path":"/api/items","headers":{"X-HTTP-Method-Override":"GET"},"body":{"title":"Oats"},"what":"Add Oats to the list."}'
  change_stopped "$oats" 1
  grep -q 'Oats' "$out/site.log" && fail "the refused overridden POST reached the site"
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  settings_drew
  if ! grep '^browser: settings shown ' "$log" | tail -1 | grep -q ' Agents$'; then
    send clickkey $(drawn "toolbar Agents")
    expect '^browser: settings tab Agents$' "a click on Agents didn't show its section"
  fi
  settings_drew Agents
  # A frame logs its rows after its drew line, the newest first, so the newest is in once one is.
  expect '^browser: settings request [0-9]* [A-Z]* at ' "Settings › Agents drew no request after the refused overridden POST"
  newest=$(sed -n 's/^browser: settings request \([0-9]*\) \([A-Z]*\) at .*/\1 \2/p' "$log" | sort -n | tail -1)
  [ "${newest#* }" = POST ] && [ "${newest% *}" -gt "$refused_id" ] ||
    fail "Settings › Agents didn't name the overridden POST by the method sent: its newest request is $newest"
  send clickkey $(drawn "toolbar General")
  expect '^browser: settings tab General$' "a click on General didn't show its section"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
  # A long address and body go nowhere either.
  picnic='{"method":"POST","path":"/api/lists/summer-2026/picnics/north-gate/saturday-noon/items/blankets-table-and-cooler/plates-cups-knife-and-bin-bags?draft=1&notify=everyone-on-the-list&note=meet-at-the-north-gate-of-the-park-at-noon","body":{"title":"Picnic","file":"menu\u202etxt.exe","notes":"Meet at the north gate of the park at noon on Saturday. Bring two blankets, the folding table and the blue cooler. Sandwiches: cheese and tomato, egg and cress, and a few plain ones for the children. Fruit: apples, grapes and a melon to cut there. Drinks: lemonade, water and a flask of tea. If it rains we move to the bandstand, and if the bandstand is taken we go to the café by the pond instead. Whoever arrives first saves the spot under the big oak by the lake, with a blanket and a sign.","items":["blankets","table","cooler","plates","cups","knife","bin bags"]},"what":"Save the picnic plan to your list."}'
  change_stopped "$picnic" 1
  grep -q 'Picnic' "$out/site.log" && fail "the refused long change reached the site"
  # Several changes in one call are refused together, each listed, and none of them is sent.
  bread='{"method":"POST","path":"/api/items","body":{"title":"Bread"},"what":"Add Bread to the list."}'
  crumbs='{"method":"DELETE","path":"/api/items/9","what":"Remove the old crumbs entry."}'
  change_stopped "$bread,$crumbs" 2
  grep -q 'Bread' "$out/site.log" && fail "a refused change of several reached the site"
  grep -q '^DELETE ' "$out/site.log" && fail "a refused delete reached the site"

  # The agent's picture of a page withholds what its outline does of a field, by one rule: a field
  # holding what the agent typed and is unchanged shows it, as does the page's own value, a
  # select's option its markup marks selected or a read-only or disabled field's; any other field
  # with something in it is painted over and its line says only that it's filled, so a field filled
  # by the page's script, as the user's typing or a password manager's fill would be, an input's
  # value and a textarea's text from the markup, which the core can't tell from those, and fields
  # whose page keeps their markup in step with what they hold, filled by the check's extension as a
  # fill would be (the user's typing in such fields is the fields page's), one the agent typed into
  # that the page or an extension changed since, which says it's not as the agent typed it, a
  # password field the page made a text field and one it made a password again, a card's field and
  # a one-time code's, even with the agent's own typing, in a frame of another site and half out of
  # view too (#2355). Each field's text is red blocks: a field shown has red in the picture, and one
  # withheld has none. The check's extension puts two frames of its
  # own with red text in the page, one over it and one in its flow below the fields, which the
  # outline leaves out, so the picture has no red there either. The site's frame has a rule of its
  # own, as one for every iframe would place the extension's too.
  printf '%s' '<!doctype html><meta charset=utf-8><style>
body { margin: 0; background: #fff }
input, textarea { position: absolute; box-sizing: border-box; width: 40%; height: 40%; margin: 0;
  padding: 0; border: 2px solid #888; background: #fff; color: #f00; font: 30vh/1 monospace;
  -webkit-text-stroke: 0.1em #f00; resize: none; overflow: hidden }
</style>
<input aria-label="Frame default" value="██████" style="left: 5%; top: 10%">
<input aria-label="Frame filled" style="left: 55%; top: 10%">
<script src="painted.js"></script>
<script>document.querySelector("[aria-label=\"Frame filled\"]").value = "██████";
painted(() => parent.postMessage("painted", "*"));</script>' > "$out/site/withheld-inner.html"
  # The fields keep some of their background inside their border, as most pages' do, and the
  # one-time code's has none, so its text reaches its edges: a withheld field's hatch is the same
  # greys whatever the field holds, and a hatch taken from the field's pixels went red there (#3277).
  printf '%s' "<!doctype html><meta charset=utf-8><title>loading</title><style>
body { margin: 0; background: #fff }
input, textarea { position: absolute; box-sizing: border-box; width: 20vw; height: 12vh; margin: 0;
  padding: 0.5vh 1vw; border: 2px solid #888; background: #fff; color: #f00; font: 9vh/1 monospace;
  -webkit-text-stroke: 0.1em #f00; resize: none; overflow: hidden }
#site { position: absolute; left: 2vw; top: 40vh; width: 60vw; height: 30vh; border: 0 }
button { position: absolute; left: 70vw; top: 50vh }
</style>
<input aria-label=Typed style='left: 2vw; top: 2vh'>
<input aria-label=Changed style='left: 26vw; top: 2vh'>
<input aria-label=Filled style='left: 50vw; top: 2vh'>
<input aria-label=Default value='██████' style='left: 74vw; top: 2vh'>
<input aria-label=Fixed readonly style='left: 2vw; top: 20vh'>
<input aria-label='Shown password' type=password value='██████' style='left: 26vw; top: 20vh'>
<input aria-label=Card autocomplete=cc-number value='██████' style='left: 50vw; top: 20vh'>
<textarea aria-label=Note style='left: 74vw; top: 20vh'>██████</textarea>
<input aria-label='One-time code' autocomplete=one-time-code style='left: 2vw; top: 33vh; height: 5vh; padding: 0; font-size: 4vh'>
<input aria-label='Card typed' autocomplete='cc-number' style='left: 26vw; top: 33vh; height: 5vh; font-size: 4vh'>
<input aria-label='Extension changed' style='left: 50vw; top: 33vh; height: 5vh; font-size: 4vh'>
<select aria-label='Moved select' style='position: absolute; left: 74vw; top: 33vh'><option selected>First<option>Second</select>
<input aria-label=Copied style='left: 2vw; top: 71vh; height: 4vh; font-size: 3vh'>
<textarea aria-label='Copied note' style='left: 26vw; top: 71vh; height: 4vh; font-size: 3vh'></textarea>
<select aria-label='Default select' style='position: absolute; left: 50vw; top: 71vh; width: 20vw; height: 4vh; appearance: none; margin: 0; padding: 0; border: 2px solid #888; background: #fff; color: #f00; font: 3vh/1 monospace'><option>First<option selected>██████</select>
<input aria-label=Disabled disabled value='██████' style='left: 74vw; top: 58vh'>
<input aria-label='Toggled back' type=password value='██████' style='left: 74vw; top: 76vh'>
<iframe id=site src=http://127.0.0.1:$port/withheld-inner.html></iframe>
<button onclick='document.querySelector(\"[aria-label=Changed]\").value += \"█\"; postMessage(\"extension, change\", \"*\")'>Change</button>
<input aria-label=Below style='left: 2vw; top: 94vh'>
<div style='height: 76vh'></div>
<script src=painted.js></script>
<script>
const field = label => document.querySelector('[aria-label=\"' + label + '\"]');
for (const label of ['Filled', 'Fixed', 'Below']) field(label).value = '██████';
field('Shown password').type = 'text';
field('Toggled back').type = 'text';
field('Toggled back').type = 'password';
field('Moved select').selectedIndex = 1;
field('Copied').oninput = event => event.target.setAttribute('value', event.target.value);
field('Copied note').oninput = event => event.target.textContent = event.target.value;
let ready = 0;
const shown = () => ++ready == 4 && (document.title = 'fields');
painted(shown);
addEventListener('message', event =>
  (event.data == 'painted' || event.data == 'extension painted') && shown());
addEventListener('message', event => event.data == 'extension changed' && (document.title = 'changed'));
</script>" > "$out/site/withheld.html"
  outline=$(call open_tab "{\"url\":\"localhost:$port/withheld.html\"}")
  fields_tab=$(printf '%s\n' "$outline" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  await_log "^browser: title $fields_tab fields\$" "the page with fields didn't paint"
  for label in Typed Changed 'One-time code' 'Card typed' 'Extension changed'; do
    ref=$(ref_of textbox "$label")
    typed=$(call type "{\"tab\":$fields_tab,\"ref\":\"$ref\",\"text\":\"██████\"}")
    case "$typed" in "error: "*) fail "typing into $label answered: $typed" ;; esac
  done
  clicked=$(call click "{\"tab\":$fields_tab,\"ref\":\"$(ref_of button Change)\"}")
  case "$clicked" in
    "Clicked button “Change”."*) ;;
    *) fail "clicking Change answered: $clicked" ;;
  esac
  await_log "^browser: title $fields_tab changed\$" "the check's extension didn't change its field"
  outline=$(call snapshot "{\"tab\":$fields_tab}")
  printf '%s\n' "$outline" > "$out/mcp-withheld-outline.txt"
  # Each field's line ends as $2 says, a password's after it says it's off limits.
  seen_line() {
    line=$(printf '%s\n' "$outline" | grep -- "- [a-z]* \"$1\" \[e[0-9]*\]" | head -1)
    case "$line" in
      *"] $2" | *"] (password: off limits) $2") ;;
      *) fail "the outline's line for $1 didn't end with $2: $line" ;;
    esac
  }
  seen_line Typed '(holds "██████")'
  seen_line Changed '(filled, not as you typed it)'
  seen_line 'Extension changed' '(filled, not as you typed it)'
  seen_line Filled '(filled)'
  seen_line Default '(filled)'
  seen_line Fixed '(read-only) (holds "██████")'
  seen_line Disabled '(disabled) (holds "██████")'
  seen_line 'Shown password' '(filled)'
  seen_line 'Toggled back' '(filled)'
  seen_line Card '(filled)'
  seen_line 'Card typed' '(filled)'
  seen_line 'One-time code' '(filled)'
  seen_line Note '(filled)'
  seen_line Copied '(filled)'
  seen_line 'Copied note' '(filled)'
  seen_line 'Frame default' '(filled)'
  seen_line 'Frame filled' '(filled)'
  seen_line Below '(filled)'
  seen_line 'Moved select' '(filled) (options: "First", "Second")'
  seen_line 'Default select' '(holds "██████") (options: "First", "██████")'
  mcp "$(request screenshot "{\"tab\":$fields_tab}")" | python3 -I -c '
import base64, json, sys
content = json.load(sys.stdin)["result"]["content"][0]
if content["type"] != "image":
    sys.exit(content.get("text", "no picture"))
open(sys.argv[1], "wb").write(base64.b64decode(content["data"]))' "$out/agent-fields.jpg" ||
    fail "the screenshot of the page with fields answered no picture"
  # The share of red pixels between ($1, $2) and ($3, $4), as hundredths of the picture's width
  # and height, in %, and their count; pixel leaves the picture as a BMP.
  pixel "$out/agent-fields.jpg" 0 0 > /dev/null
  reds() {
    python3 -I - "$out/pixel.bmp" "$@" << 'PY'
import struct, sys
data = open(sys.argv[1], "rb").read()
offset, = struct.unpack_from("<I", data, 10)
width, height, _, bits = struct.unpack_from("<iiHH", data, 18)
x0, y0, x1, y1 = (float(each) / 100 for each in sys.argv[2:6])
stride = (bits * width + 31) // 32 * 4
red = total = 0
for y in range(int(y0 * abs(height)), int(y1 * abs(height))):
    row = y if height < 0 else abs(height) - 1 - y
    for x in range(int(x0 * width), int(x1 * width)):
        start = offset + row * stride + x * bits // 8
        b, g, r = data[start:start + 3]
        red += r > 160 and g < 100 and b < 100
        total += 1
print(red * 100 // max(total, 1), red)
PY
  }
  # A field's inside, $1 between ($2, $3) and ($4, $5), clear of its border: $6, shown with red,
  # or withheld with none.
  see_field() {
    reddened=$(reds "$2" "$3" "$4" "$5" | cut -d' ' -f1)
    echo "$1: $reddened% red" >> "$out/mcp-pictured.txt"
    if [ "$6" = shown ]; then
      [ "$reddened" -ge 10 ] ||
        fail "the agent's picture didn't show $1, which it may see: $reddened% red"
    else
      [ "$reddened" -eq 0 ] || fail "the agent's picture showed $1, which it may not: $reddened% red"
    fi
  }
  : > "$out/mcp-pictured.txt"
  see_field Typed 3 4 21 12 shown
  see_field Changed 27 4 45 12 withheld
  see_field Filled 51 4 69 12 withheld
  see_field Default 75 4 93 12 withheld
  see_field Fixed 3 22 21 30 shown
  see_field "Shown password" 27 22 45 30 withheld
  see_field Card 51 22 69 30 withheld
  see_field "One-time code" 3 34 21 37 withheld
  see_field "Card typed" 27 34 45 37 withheld
  see_field "Extension changed" 51 34 69 37 withheld
  see_field Disabled 75 60 93 68 shown
  see_field "Toggled back" 75 78 93 86 withheld
  see_field Note 75 22 93 30 withheld
  see_field Copied 3 72 21 74 withheld
  see_field "Copied note" 27 72 45 74 withheld
  see_field "Default select" 51 72 65 74 shown
  see_field "Frame default" 6 44 28 54 withheld
  see_field "Frame filled" 36 44 58 54 withheld
  see_field Below 3 95 21 99 withheld
  see_field "Extension frame over the page" 41 77 64 90 withheld
  see_field "Extension frame in the page" 3 77 26 90 withheld
  # The user's typing on the fields page, in fields whose page keeps their markup in step: no red
  # pixel at all, as a line of text is too small a share of the picture to count as one.
  pixel "$out/agent-typed.jpg" 0 0 > /dev/null
  typed_red=$(reds 0 0 100 100 | cut -d' ' -f2)
  echo "User's typing, copied to the markup: $typed_red red pixels" >> "$out/mcp-pictured.txt"
  [ "$typed_red" -eq 0 ] ||
    fail "the agent's picture showed the user's typing, which it may not: $typed_red red pixels"
  call close_tab "{\"tab\":$fields_tab}" > /dev/null

  # It opens a page in its Space, clicks its link, scrolls a long page and takes a screenshot.
  opened=$(call open_tab "{\"url\":\"localhost:$port/agent.html\"}")
  case "$opened" in
    "Opened tab "*"URL: http://localhost:$port/agent.html"*) ;;
    *) fail "open_tab answered: $opened" ;;
  esac
  opened_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  await_log "^browser: agent page $opened_tab [0-9]* [1-9][0-9]*\$" "the core didn't outline the opened page"
  # A screenshot of a tab out of sight, just opened, answers, with its picture or, if the tab drew
  # none in 15 s, with that, never not at all: call's 30 s would fail it.
  picture=$(call screenshot "{\"tab\":$opened_tab}")
  case "$picture" in
    "image/jpeg "[1-9]*) fresh_picture=a\ picture ;;
    "error: The tab didn't draw a picture in 15 s. Try again, or ask the user to bring it to the front.")
      fresh_picture="the 15 s error" ;;
    *) fail "a screenshot of the tab just opened answered: $picture" ;;
  esac
  # With the UI holding the keyboard, as after a click in the sidebar, nothing in the window changes,
  # and a screenshot of a tab out of sight still answers with its picture, from its page's surface.
  send click 100 600
  picture=$(call screenshot "{\"tab\":$opened_tab}")
  case "$picture" in
    "image/jpeg "[1-9]*) ;;
    *) fail "with the UI holding the keyboard, a screenshot of a tab out of sight answered: $picture" ;;
  esac
  link=$(printf '%s\n' "$opened" | sed -n 's/.*- link "[^"]*" \[\(e[0-9]*\)\].*/\1/p' | head -1)
  clicked=$(call click "{\"tab\":$opened_tab,\"ref\":\"$link\"}")
  case "$clicked" in
    "Clicked link"*"two.html"*) ;;
    *) fail "clicking agent.html's link answered: $clicked" ;;
  esac
  # Back to a page it saw before, whose entry kept its title, the tab's row takes that title again
  # (#1469).
  titles=$(grep -c "^browser: title $opened_tab Agent page\$" "$log" || true)
  went=$(call navigate "{\"tab\":$opened_tab,\"url\":\"back\"}")
  case "$went" in "Went back."*"Agent page"*) ;; *) fail "navigate back answered: $went" ;; esac
  await_more "^browser: title $opened_tab Agent page\$" "$titles" ||
    fail "back on agent.html, the tab kept the title of the page it left: $(grep "^browser: title $opened_tab " "$log" | tail -1)"
  # agent.html is shorter than the view, where a scroll says nothing moved (#1445), so the tab
  # goes on to a page taller than the view, whose heading the steps below wait for, and a scroll
  # down, the steps' scroll up and the one after Hand Back each move it.
  printf '%s' '<title>Long page</title><h1>Example Domain</h1>
<p style="height:3000px">A page taller than the view.</p>' > "$out/site/long.html"
  call navigate "{\"tab\":$opened_tab,\"url\":\"localhost:$port/long.html\"}" > /dev/null
  await_log "^browser: title $opened_tab Long page\$" "the long page didn't load in the agent's tab"
  scrolled=$(call scroll "{\"tab\":$opened_tab}")
  case "$scrolled" in "Scrolled down."*) ;; *) fail "scroll answered: $scrolled" ;; esac
  picture=$(call screenshot "{\"tab\":$opened_tab}")
  case "$picture" in "image/jpeg "[1-9]*) ;; *) fail "screenshot answered: $picture" ;; esac
  # close_tab closes a tab of its own, which stays out of the user's archive (#1408).
  opened=$(call open_tab "{\"url\":\"localhost:$port/agent.html\"}")
  closing=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  archived=$(sed -n 's/^browser: archive \([0-9]*\) tabs$/\1/p' "$log" | tail -1)
  closed=$(call close_tab "{\"tab\":$closing}")
  [ "$closed" = "Closed tab $closing." ] || fail "close_tab answered: $closed"
  await_log "^browser: agent activity mcp-client $closing close_tab\$" \
    "the close didn't reach the activity log"
  # A round trip, by which the core has told the app of an archive the close changed.
  listed=$(call list_tabs '{}')
  printf '%s\n' "$listed" | grep -q "^. $closing  " && fail "list_tabs listed the closed tab"
  [ "$(sed -n 's/^browser: archive \([0-9]*\) tabs$/\1/p' "$log" | tail -1)" = "$archived" ] ||
    fail "the tab the agent closed went in the user's archive"
  # A close asks a page with a beforeunload handler as the user's close does, and the answer
  # waits for it (#3256). Chromium lets a handler ask only in a frame that was clicked or typed in,
  # so the agent's click or typing first, as the user's would. Never a leave dialog, and each close
  # has one activity-log entry, written as the page answered, after the core's line.
  printf '%s\n' '<!doctype html><title>Quiet</title><button>Note</button>
<script>addEventListener("beforeunload", () => {})</script>' > "$out/site/quiet.html"
  printf '%s\n' '<!doctype html><title>Leave</title><p>Unsaved changes.</p><button>Note</button>
<script>addEventListener("beforeunload", event => event.preventDefault())</script>' > "$out/site/leave.html"
  printf '%s\n' '<!doctype html><title>Form</title><label>Note <input></label>
<script>let dirty = false; addEventListener("input", () => dirty = true);
addEventListener("beforeunload", event => { if (dirty) event.preventDefault() })</script>' > "$out/site/form.html"
  # Opens $1 in a tab of the agent's own, and prints its id, then the ref of its "$2" "$3".
  open_with() {
    opened=$(call open_tab "{\"url\":\"localhost:$port/$1\"}")
    printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p'
    printf '%s\n' "$opened" | sed -n "s/.*- $2 \"$3\" \[\(e[0-9]*\)\].*/\1/p" | head -1
  }
  # The close of $1 answered after the core asked its page and heard $2, with its one activity-log
  # entry, "close_tab localhost" and $3, after that, and no leave dialog. The core's "agent
  # activity" line is the permission check's, at the call, before the page is asked.
  asked_page() {
    asked=$(grep -n "browser: agent close $1 asks its page\$" "$log" | tail -1 | cut -d: -f1)
    [ -n "$asked" ] || fail "the close of tab $1 didn't ask its page"
    heard=$(grep -n "browser: agent close $1 answered $2\$" "$log" | tail -1 | cut -d: -f1)
    [ -n "$heard" ] || fail "the core didn't answer the close of tab $1 with $2"
    await_log "^browser: agent logged mcp-client close_tab localhost$3" \
      "the close of tab $1 wasn't logged"
    entry=$(grep -n "^browser: agent logged mcp-client close_tab localhost$3" "$log" | tail -1 |
      cut -d: -f1)
    [ -n "$entry" ] && [ "$entry" -gt "$heard" ] ||
      fail "the close of tab $1 wasn't logged once its page answered"
    [ "$(sed -n "$asked,\$p" "$log" | grep -c '^browser: agent logged mcp-client close_tab ')" = 1 ] ||
      fail "the close of tab $1 was logged more than once"
    grep -q "^browser: page $1 dialog [^n]" "$log" && fail "closing tab $1 showed a dialog"
    return 0
  }
  # A handler that doesn't ask: the first call closes the tab, as the core closes any, out of the
  # agent's list and out of the user's archive.
  set -- $(open_with quiet.html button Note)
  quiet=$1
  call click "{\"tab\":$quiet,\"ref\":\"$2\"}" > /dev/null
  archived=$(sed -n 's/^browser: archive \([0-9]*\) tabs$/\1/p' "$log" | tail -1)
  closed=$(call close_tab "{\"tab\":$quiet}")
  [ "$closed" = "Closed tab $quiet." ] || fail "closing a page whose handler doesn't ask answered: $closed"
  asked_page "$quiet" 0 '$'
  grep -q "^browser: tab $quiet closed\$" "$log" || fail "the core didn't tell of the closed tab"
  call list_tabs '{}' | grep -q "^. $quiet  " && fail "list_tabs listed the closed tab"
  [ "$(sed -n 's/^browser: archive \([0-9]*\) tabs$/\1/p' "$log" | tail -1)" = "$archived" ] ||
    fail "the tab the agent closed went in the user's archive"
  # A handler that asks, in a page no one clicked or typed in, can't: it closes as the user's
  # close of it would, without a dialog.
  set -- $(open_with leave.html button Note)
  untouched=$1
  closed=$(call close_tab "{\"tab\":$untouched}")
  [ "$closed" = "Closed tab $untouched." ] || fail "closing a page no one clicked in answered: $closed"
  asked_page "$untouched" 0 '$'
  # Once the agent clicked in it, it asks: the tab stays, and the answer says why. With discard it's
  # asked again, and the agent's own tab closes without saving (#1447).
  set -- $(open_with leave.html button Note)
  leaving=$1
  call click "{\"tab\":$leaving,\"ref\":\"$2\"}" > /dev/null
  kept=$(call close_tab "{\"tab\":$leaving}")
  case "$kept" in
    "error: Tab $leaving stays open: its page asks before it's left"*"discard: true"*) ;;
    *) fail "closing a page that asks before it's left answered: $kept" ;;
  esac
  asked_page "$leaving" 3 " refused: Tab $leaving stays open"
  call list_tabs '{}' | grep -q "^. $leaving  " || fail "a close that kept the tab took it out"
  closed=$(call close_tab "{\"tab\":$leaving,\"discard\":true}")
  case "$closed" in
    "Closed tab $leaving without saving."*) ;;
    *) fail "discarding the agent's own tab answered: $closed" ;;
  esac
  [ "$(grep -c "browser: agent close $leaving asks its page\$" "$log")" = 2 ] ||
    fail "the discard didn't ask the page first"
  # The activity log has it as a discard, once, after the page was asked again. The permission
  # check's "agent activity" line at the call names the tool, close_tab.
  await_log "^browser: agent logged mcp-client discard_tab localhost\$" \
    "the discard didn't reach the activity log"
  asked=$(grep -n "browser: agent close $leaving asks its page\$" "$log" | tail -1 | cut -d: -f1)
  [ "$(sed -n "$asked,\$p" "$log" | grep -c '^browser: agent logged mcp-client discard_tab localhost$')" = 1 ] ||
    fail "the discard wasn't logged once after its page was asked"
  call list_tabs '{}' | grep -q "^. $leaving  " && fail "list_tabs listed the discarded tab"
  grep -q "^browser: page $leaving dialog [^n]" "$log" && fail "closing the leaving page showed a dialog"
  # A form the agent typed in asks, as one the user typed in would.
  set -- $(open_with form.html textbox Note)
  form=$1
  call type "{\"tab\":$form,\"ref\":\"$2\",\"text\":\"by-the-agent\"}" > /dev/null
  kept=$(call close_tab "{\"tab\":$form}")
  case "$kept" in
    "error: Tab $form stays open: its page asks before it's left"*) ;;
    *) fail "closing a form the agent typed in answered: $kept" ;;
  esac
  asked_page "$form" 3 " refused: Tab $form stays open"
  closed=$(call close_tab "{\"tab\":$form,\"discard\":true}")
  [ "$closed" = "Closed tab $form without saving." ] || fail "discarding the form answered: $closed"
  grep -q "^browser: page $form dialog [^n]" "$log" && fail "closing the form showed a dialog"
  # A page's own dialog in the agent's tab is the user's to answer, and no tool answers it (#3536).
  # A call on the tab answers at once, naming the dialog, and reaches no page, and the activity log
  # says the dialog paused the agent, without its words. close_tab closes the tab without asking
  # its page, which can't answer while its dialog is up. The click arms the alert for later than a
  # click's job can take, whose settling ends within 5 s, so the click has its answer first.
  printf '%s\n' '<!doctype html><title>Asks</title><button>Ask</button>
<script>document.querySelector("button").onclick = () => setTimeout(() => alert("Hold the seats?"), 10000)</script>' > "$out/site/asks.html"
  set -- $(open_with asks.html button Ask)
  asks=$1
  call click "{\"tab\":$asks,\"ref\":\"$2\"}" > /dev/null
  await_log "^browser: page $asks dialog Alert " "the page's alert didn't come in the agent's tab"
  shown_at=$(wc -l < "$log")
  pressed=$(call press_key "{\"tab\":$asks,\"key\":\"Escape\"}")
  [ "$pressed" = "error: Tab $asks's page shows an alert. Nothing more can be done on the tab while it's up, and no tool answers it. close_tab closes the tab, and the dialog with it." ] ||
    fail "a press while the page's alert is up answered: $pressed"
  sed -n "$shown_at,\$p" "$log" | grep -q "^browser: agent page $asks " &&
    fail "the press reached the page under its alert"
  await_log "^browser: agent logged mcp-client page_dialog localhost\$" \
    "the activity log didn't say the page's alert paused the agent"
  grep -q '^browser: agent logged .*Hold the seats' "$log" && fail "the activity log has the alert's words"
  closed=$(call close_tab "{\"tab\":$asks}")
  [ "$closed" = "Closed tab $asks." ] || fail "closing the tab under its alert answered: $closed"
  grep -q "browser: agent close $asks asks its page\$" "$log" && fail "the close asked the page under its alert"
  await_log "^browser: tab $asks closed\$" "the core didn't tell of the tab closed under its alert"

  # It pins a tab of its own, which list_tabs says it pinned, unpins it, pins it again and removes
  # it, to the archive. The user's tab isn't its to pin.
  opened=$(call open_tab "{\"url\":\"localhost:$port/agent.html\"}")
  pinning=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  pinned=$(call pin_tab "{\"tab\":$pinning}")
  case "$pinned" in
    "Pinned tab $pinning at the page it's at."*) ;;
    *) fail "pin_tab answered: $pinned" ;;
  esac
  await_log "^browser: agent activity mcp-client $pinning pin_tab\$" \
    "the pin didn't reach the activity log"
  call list_tabs '{}' | grep -q "^. $pinning  .*  pinned by you\$" ||
    fail "list_tabs didn't say the agent pinned its tab"
  unpinned=$(call unpin_tab "{\"tab\":$pinning}")
  [ "$unpinned" = "Unpinned tab $pinning. It's at the top of your Today tabs." ] ||
    fail "unpin_tab answered: $unpinned"
  call list_tabs '{}' | grep "^. $pinning  " | grep -q '  pinned by ' &&
    fail "list_tabs listed the unpinned tab as pinned"
  call pin_tab "{\"tab\":$pinning}" > /dev/null
  removed=$(call remove_tab "{\"tab\":$pinning}")
  [ "$removed" = "Removed tab $pinning. The user can still find it in the archive." ] ||
    fail "remove_tab answered: $removed"
  await_log "^browser: agent activity mcp-client $pinning remove_tab\$" \
    "the removal didn't reach the activity log"
  call list_tabs '{}' | grep -q "^. $pinning  " && fail "list_tabs listed the removed tab"
  refused=$(call pin_tab "{\"tab\":$user_tab}")
  [ "$refused" = "error: The tab isn't in the agent's own Space." ] ||
    fail "pinning the user's tab answered: $refused"

  # run_steps takes several actions in one call, in order, and waits for a page's text.
  ran=$(call run_steps "{\"tab\":$opened_tab,\"steps\":[{\"action\":\"scroll\",\"direction\":\"up\"},{\"action\":\"snapshot\"},{\"action\":\"wait\",\"text\":\"example domain\"}]}")
  case "$ran" in
    "Ran 3 steps."*"1. Scrolled up."*"2. Tab $opened_tab:"*"3. “example domain” showed."*) ;;
    *) fail "run_steps answered: $ran" ;;
  esac

  # hand_off waits until the user hands the tab back, and nothing is asked in a card (#4086): the
  # step is ilo's line in the conversation, with Hand Back and Not Now, and its page's tile says
  # Needs you. While the user watches another of the task's pages, that one stays where it is.
  back="^browser: task page $agent_tab back\$"
  thumbnail='^browser: task page in its thumbnail$'
  opened_watched="^browser: task page $opened_tab watched\$"
  opens=$(grep -c "$opened_watched" "$log" || true)
  placed=$(grep -c "^browser: task page $opened_tab in place\$" "$log" || true)
  request hand_off "{\"tab\":$opened_tab,\"reason\":\"Sign in for the check\"}" >&4
  await_log "^browser: agent mcp-client hands off $opened_tab: Sign in for the check\$" \
    "hand_off didn't hand the tab to the user"
  send state
  [ "$(grep -c "$opened_watched" "$log" || true)" = "$opens" ] ||
    fail "the page handed to the user took the place of the page they watched"
  # Back in the conversation, the page handed to the user says so under its picture (#1080), and
  # opens by itself, as the user has no other of the task's pages there now.
  before=$(grep -c "$back" "$log" || true)
  send key escape
  await_more "$back" "$before" || fail "Escape didn't step back to the conversation"
  await_log "^browser: task picture $opened_tab says Needs you\$" \
    "the handed page's tile didn't say it needs the user: $(grep "^browser: task picture $opened_tab says" "$log" | tail -1)"
  await_more "$opened_watched" "$opens" || fail "the page handed to the user didn't open by itself"
  await_more "^browser: task page $opened_tab in place\$" "$placed" ||
    fail "the page handed to the user didn't come to rest in place"
  ../tools/window-screenshot.sh "$pid" "$out/agent-handoff.png" || fail "no window to capture"
  # The keyboard stayed the window's as ilo opened the page, so Escape steps back from it, and the
  # step's line stands in the conversation with its buttons.
  before=$(grep -c "^browser: task page $opened_tab back\$" "$log" || true)
  shrunk=$(grep -c "$thumbnail" "$log" || true)
  send key escape
  await_more "^browser: task page $opened_tab back\$" "$before" ||
    fail "Escape didn't step back from the page ilo opened for the user's step"
  await_more "$thumbnail" "$shrunk" || fail "the handed page didn't go back into its picture"
  await_log '^browser: ilo line [0-9]* Not Now at ' "the step's line drew no Not Now"
  ../tools/window-screenshot.sh "$pid" "$out/agent-pages-needs-you.png" || fail "no window to capture"
  # Hand Back, the line's first button, by the keys.
  ilo_press "Hand Back"
  await_log "^browser: agent mcp-client handed back $opened_tab\$" "Hand Back didn't hand the tab back"
  handed=$(next_answer) || fail "the browser didn't answer hand_off"
  handed=$(printf '%s\n' "$handed" | answered hand_off)
  case "$handed" in
    "The user handed tab $opened_tab back."*) ;;
    *) fail "hand_off answered: $handed" ;;
  esac
  watch "$agent_tab"

  # A sign-in the browser asks for, which no agent may answer, hands the agent's tab to the user as
  # hand_off does, its line's step the user's, and the call that met it waits (#3662). The page
  # opens by itself as its tile turns to Needs you, and the dialog shows over it once it's watched,
  # never before. Esc cancels it and the tab stays the user's, as ⌘↩ after the next ask, signed in,
  # hands it back; the call then answers. The agent's calls on the tab wait behind it meanwhile, as
  # behind hand_off's. The throwaway password typed reaches no answer and no log line.
  sign_in_from="^browser: task page $agent_tab back\$"
  before=$(grep -c "$sign_in_from" "$log" || true)
  shrunk=$(grep -c "$thumbnail" "$log" || true)
  send key escape
  await_more "$sign_in_from" "$before" || fail "Escape didn't step back to Pages for the sign-in"
  await_more "$thumbnail" "$shrunk" || fail "the page watched didn't go back into its picture for the sign-in"
  settled=$(grep -c '^browser: dialog settled$' "$log" || true)
  opens=$(grep -c "$opened_watched" "$log" || true)
  # A host as long as a sign-in's can be.
  sign_in_host=sign-in.accounts.a-long-example-host-for-the-card.localhost
  asked="^browser: page $opened_tab dialog SignIn Sign in\$"
  request navigate "{\"tab\":$opened_tab,\"url\":\"$sign_in_host:$port/sign-in?agent\"}" >&4
  await_log "$asked" "the site's sign-in didn't ask in the agent's tab"
  # Its apostrophe is typographic, which a locale reads as one character or three, and the UI's
  # tests hold it to the letter.
  sign_in_why="$(printf '%s' "$sign_in_host" | sed 's/\./\\./g') asks you to sign in\. Hand the tab back when you.*re done\."
  await_log "^browser: agent mcp-client hands off $opened_tab: $sign_in_why\$" \
    "the sign-in didn't hand the agent's tab to the user: $(grep "^browser: agent mcp-client hands off $opened_tab" "$log" | tail -1)"
  await_more "$opened_watched" "$opens" || fail "the page that asks the user to sign in didn't open by itself"
  await_more '^browser: dialog settled$' "$settled" || fail "the agent's sign-in didn't show over its page watched"
  # The dialog's line comes after the page's: it never showed while the page wasn't watched.
  [ "$(grep -n '^browser: dialog settled$' "$log" | tail -1 | cut -d: -f1)" -gt \
    "$(grep -n "$opened_watched" "$log" | tail -1 | cut -d: -f1)" ] ||
    fail "the agent's sign-in showed while the user wasn't watching its page"
  ../tools/window-screenshot.sh "$pid" "$out/agent-sign-in-dialog.png" || fail "no window to capture"
  send key escape
  await_log "^browser: page $opened_tab dialog answered cancel\$" "Esc didn't cancel the agent's sign-in"
  await_log "^browser: title $opened_tab signed out\$" "a cancelled sign-in didn't show what the site sent"
  asks=$(grep -c "$asked" "$log" || true)
  settled=$(grep -c '^browser: dialog settled$' "$log" || true)
  go_from_bar "localhost:$port/sign-in?again"
  await_more "$asked" "$asks" || fail "the site's second sign-in didn't ask"
  await_more '^browser: dialog settled$' "$settled" || fail "the second sign-in didn't show over the page watched"
  send type ada
  send key tab
  send type zq7lovelace
  send key return
  await_log "^browser: page $opened_tab dialog answered accept\$" "↩ didn't sign in in the agent's tab"
  await_log "^browser: title $opened_tab signed in ada\$" "the site didn't take the user's sign-in"
  [ "$(grep -c "^browser: agent mcp-client hands off $opened_tab: $sign_in_why\$" "$log")" = 1 ] ||
    fail "the second sign-in, under the hold, handed the tab off again"
  handed=$(grep -c "^browser: agent mcp-client handed back $opened_tab\$" "$log" || true)
  send key return cmd
  await_more "^browser: agent mcp-client handed back $opened_tab\$" "$handed" ||
    fail "⌘↩ didn't hand the signed-in tab back"
  waited=$(next_answer) || fail "the browser didn't answer the navigate that met the sign-in"
  waited=$(printf '%s\n' "$waited" | answered navigate)
  case "$waited" in
    "A browser sign-in dialog opened on the tab, so the browser handed the tab to the user and this call waited. Nothing they typed is shown. The user handed tab $opened_tab back. Take a snapshot to see where it is now.") ;;
    *) fail "the navigate that met the sign-in answered: $waited" ;;
  esac
  signed_said=$(call snapshot "{\"tab\":$opened_tab}")
  case "$signed_said" in
    "Tab $opened_tab: signed in ada"*) ;;
    *) fail "the agent didn't read the page the user signed in to: $signed_said" ;;
  esac
  for said in "$waited" "$signed_said"; do
    case "$said" in *zq7lovelace*) fail "the password typed in the sign-in reached the agent: $said" ;; esac
  done
  # The driver's own lines say what it typed.
  grep -v '^browser: drive ' "$log" | grep -q zq7lovelace && fail "the password typed in the agent's sign-in reached the log"
  # The steps below scroll the tab, which the signed-in page, shorter than the view, can't do.
  longs=$(grep -c "^browser: title $opened_tab Long page\$" "$log" || true)
  call navigate "{\"tab\":$opened_tab,\"url\":\"localhost:$port/long.html\"}" > /dev/null
  await_more "^browser: title $opened_tab Long page\$" "$longs" || fail "the long page didn't load again after the sign-in"
  before=$(grep -c "^browser: task page $opened_tab back\$" "$log" || true)
  shrunk=$(grep -c "$thumbnail" "$log" || true)
  send key escape
  await_more "^browser: task page $opened_tab back\$" "$before" || fail "Escape didn't step back from the signed-in page"
  await_more "$thumbnail" "$shrunk" || fail "the signed-in page didn't go back into its picture"
  watch "$agent_tab"

  # From the conversation, a press in the page watched takes the tab the agent is at work in over,
  # as the strip has no Take Over (#1080), and the agent's next action there is refused until the
  # user hands it back. The strip then says the user drives.
  before=$(grep -c "$back" "$log" || true)
  shrunk=$(grep -c "$thumbnail" "$log" || true)
  send key escape
  await_more "$back" "$before" || fail "Escape didn't step back to the conversation"
  await_more "$thumbnail" "$shrunk" || fail "the page watched didn't go back into its picture"
  watch "$opened_tab"
  await_log "^browser: task strip $opened_tab at " "the page watched showed no strip"
  ../tools/window-screenshot.sh "$pid" "$out/agent-strip.png" || fail "no window to capture"
  read -r sx sy sw _ <<STRIP
$(grep "^browser: task strip $opened_tab at " "$log" | tail -1 | cut -d' ' -f6-)
STRIP
  # In the page, above the strip, where nothing on the page takes the click.
  send click $((sx + sw / 2)) $((sy - 120))
  await_log "^browser: agent page pressed in $opened_tab\$" "a press in the agent's page didn't reach the strip"
  await_log "^browser: agent mcp-client taken over in $opened_tab\$" "a press in the agent's page didn't take it over"
  ../tools/window-screenshot.sh "$pid" "$out/agent-driving.png" || fail "no window to capture"
  refused=$(call scroll "{\"tab\":$opened_tab}")
  case "$refused" in
    "error: "*"hand it back"*) ;;
    *) fail "acting in a tab the user took over answered: $refused" ;;
  esac
  await_log "^browser: agent activity mcp-client $opened_tab scroll refused: .*hand it back" \
    "the refusal didn't reach the activity log"
  # The agent asks for the tab back, which is ilo's line in the conversation (#4086), and Not Now
  # there keeps it the user's. The page they took has the keyboard, and Escape steps back from it.
  request ask_for_tab "{\"tab\":$opened_tab,\"reason\":\"To finish the check\"}" >&4
  await_log "^browser: agent mcp-client asks for $opened_tab back: To finish the check\$" \
    "ask_for_tab didn't ask"
  before=$(grep -c "^browser: task page $opened_tab back\$" "$log" || true)
  shrunk=$(grep -c "$thumbnail" "$log" || true)
  send key escape
  await_more "^browser: task page $opened_tab back\$" "$before" ||
    fail "Escape didn't step back from the page the user took over"
  await_more "$thumbnail" "$shrunk" || fail "the page the user took over didn't go back into its picture"
  ../tools/window-screenshot.sh "$pid" "$out/agent-ask-for-tab.png" || fail "no window to capture"
  ilo_press "Not Now"
  kept=$(next_answer) || fail "the browser didn't answer ask_for_tab"
  kept=$(printf '%s\n' "$kept" | answered ask_for_tab)
  case "$kept" in
    "error: The user keeps tab $opened_tab."*) ;;
    *) fail "ask_for_tab answered: $kept" ;;
  esac
  # Watched again, ⌘↩ hands it back.
  watch "$opened_tab"
  handed=$(grep -c "^browser: agent mcp-client handed back $opened_tab\$" "$log" || true)
  send key return cmd
  await_more "^browser: agent mcp-client handed back $opened_tab\$" "$handed" || fail "⌘↩ didn't hand the tab back"
  scrolled=$(call scroll "{\"tab\":$opened_tab}")
  case "$scrolled" in "Scrolled down."*) ;; *) fail "after Hand Back, scroll answered: $scrolled" ;; esac

  # Nothing of the user's autofills in the agent's tab. A throwaway login, saved through the offer
  # while the user holds the tab, on this run's own port, fills in neither as the page loads nor
  # from a dropdown once the agent has the tab back. Take Over fills it at once, without a reload,
  # and the dropdown offers it. ⌘↩ on the page watched takes it over and hands it back.
  # Its count has a name of its own: the run's $before, which the run reads after, holds the login
  # page's loads.
  take_over() {
    taken="^browser: agent mcp-client taken over in $opened_tab\$"
    takeovers=$(grep -c "$taken" "$log" || true)
    send key return cmd
    await_more "$taken" "$takeovers" || fail "$1: ⌘↩ didn't take the tab"
  }
  # Sets child_windows to how many child windows the browser window has, Chrome's autofill dropdown
  # among them. It's called in the run's own shell, never in $(...): a send in a subshell loses its
  # step, and every send after it waits on an earlier step's line (#3936).
  children() {
    send layout
    child_windows=$(sed -n "/^browser: drive $step: /,\$p" "$log" |
      grep -c '^browser: layout child' || true)
  }
  logins() {
    grep -c "^browser: title $opened_tab login\$" "$log" || true
  }
  call navigate "{\"tab\":$opened_tab,\"url\":\"localhost:$port/welcome.html\"}" > /dev/null
  await_log "^browser: title $opened_tab welcome\$" "the welcome page didn't load in the agent's tab"
  take_over "to save a login"
  login_loaded="^browser: page $opened_tab back [0-9]* forward 0 loading 0 http://localhost:$port/login\\.html\$"
  login_loads=$(grep -c "$login_loaded" "$log" || true)
  go_from_bar "localhost:$port/login.html"
  expect "^browser: title $opened_tab login\$" "the login page didn't load in the tab the user holds"
  # The page names itself once painted, which can be before its load ends, and a login typed then
  # brought no offer to save it: the click and the keys go once the load has ended, as a person's
  # would come after it.
  await_more "$login_loaded" "$login_loads" ||
    fail "the login page didn't finish loading in the tab the user holds: $(grep "^browser: page $opened_tab " "$log" | tail -2 | tr '\n' ' ')"
  send click 600 60
  send key a cmd
  send type check@example.test
  send key tab
  send key a cmd
  send type hunter2.check
  send key return
  expect "^browser: offer $opened_tab save check@example.test " "the user's login brought no offer to save"
  expect '^browser: offer shown$' "the offer to save the login wasn't shown"
  expect "^browser: offer $opened_tab takes input\$" "the offer to save the login never took input"
  send layout
  child=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: layout child' |
    grep ' 320x180$' | head -1 | sed 's/^browser: layout child //')
  [ -n "$child" ] || fail "the offer's card isn't over the browser window"
  ox=${child%%,*}
  oy=${child#*,}
  oy=${oy%% *}
  # Return in the offer's username field saves.
  send click $((ox + 56)) $((oy + 70))
  send key return
  expect "^browser: offer $opened_tab answered Accept check@example.test\$" "return didn't save the login"
  handed="^browser: agent mcp-client handed back $opened_tab\$"
  before=$(grep -c "$handed" "$log" || true)
  send key return cmd
  await_more "$handed" "$before" || fail "⌘↩ didn't hand the login's tab back"

  before=$(logins)
  call navigate "{\"tab\":$opened_tab,\"url\":\"localhost:$port/login.html\"}" > /dev/null
  await_more "^browser: title $opened_tab login\$" "$before" ||
    fail "the login page didn't load in the agent's tab"
  children
  shown=$child_windows
  # The agent's own click in the username field, where Chrome's dropdown would offer the login. A
  # user's press there would take the tab over (#1080), and the login would be theirs to fill.
  outline=$(call snapshot "{\"tab\":$opened_tab}")
  user_field=$(printf '%s\n' "$outline" | sed -n 's/.*- textbox[^[]*\[\(e[0-9]*\)\].*/\1/p' | head -1)
  [ -n "$user_field" ] || fail "the login page's outline had no username field: $outline"
  pointers=$(grep -c "^browser: agent pointer $opened_tab " "$log" || true)
  gone=$(grep -c '^browser: agent pointer gone$' "$log" || true)
  floats=$(grep -c '^browser: drop targets shown$' "$log" || true)
  hidden=$(grep -c '^browser: drop targets hidden$' "$log" || true)
  call click "{\"tab\":$opened_tab,\"ref\":\"$user_field\"}" > /dev/null
  # Chrome fills and offers a saved login within a moment of the load and the click. Here none
  # may come. The agent's pointer comes to the field and goes 2 s later, longer than that moment,
  # and where its window over the page is a child (the drop targets' float) that goes with it, so
  # the children are counted after both (#1935).
  await_more "^browser: agent pointer $opened_tab " "$pointers" ||
    fail "the agent's pointer didn't come to the username field"
  await_more '^browser: agent pointer gone$' "$gone" || fail "the agent's pointer didn't go"
  if [ "$(grep -c '^browser: drop targets shown$' "$log" || true)" -gt "$floats" ]; then
    await_more '^browser: drop targets hidden$' "$hidden" ||
      fail "the agent's pointer's window stayed over the page"
  fi
  ../tools/window-screenshot.sh "$pid" "$out/agent-login-empty.png" || fail "no window to capture"
  children
  [ "$child_windows" = "$shown" ] || fail "the agent's tab offered the saved login in a dropdown"
  # Nor do the agent's keys, which would open the dropdown and accept its login (#2355).
  call press_key "{\"tab\":$opened_tab,\"key\":\"ArrowDown\"}" > /dev/null
  children
  [ "$child_windows" = "$shown" ] || fail "the agent's ArrowDown offered the saved login in a dropdown"
  grep -q "^browser: title $opened_tab filled\$" "$log" &&
    fail "Chrome filled the saved password in the agent's tab"
  outline=$(call snapshot "{\"tab\":$opened_tab}")
  case "$outline" in *hunter2*) fail "the login page's outline gave the password" ;; esac
  # With nothing offered, its Enter submits the empty form, which goes on to the welcome page.
  welcomes=$(grep -c "^browser: title $opened_tab welcome\$" "$log" || true)
  call press_key "{\"tab\":$opened_tab,\"key\":\"Enter\"}" > /dev/null
  await_more "^browser: title $opened_tab welcome\$" "$welcomes" ||
    fail "the agent's Enter in the login field didn't submit the form"
  grep -q "^browser: title $opened_tab filled\$" "$log" &&
    fail "the agent's Enter accepted the saved login"
  before=$(logins)
  call navigate "{\"tab\":$opened_tab,\"url\":\"localhost:$port/login.html\"}" > /dev/null
  await_more "^browser: title $opened_tab login\$" "$before" ||
    fail "the login page didn't load in the agent's tab again"
  before=$(logins)
  take_over "to fill the login"
  await_log "^browser: title $opened_tab filled\$" "Take Over didn't fill the saved password in"
  [ "$(logins)" = "$before" ] || fail "Take Over reloaded the login page to fill it"
  send click 600 60
  for _ in $(seq 30); do
    children
    [ "$child_windows" -gt "$shown" ] && break
  done
  [ "$child_windows" -gt "$shown" ] || fail "the held tab's username field offered no saved login"
  ../tools/window-screenshot.sh "$pid" "$out/agent-login-filled.png" || fail "no window to capture"
  send key escape
  before=$(grep -c "$handed" "$log" || true)
  send key return cmd
  await_more "$handed" "$before" || fail "⌘↩ didn't hand the filled tab back"

  # open_tab at a file downloads it at Work on pages, with nothing asked of the user (#1446,
  # #4086), and answers with its name and size, its empty tab closed.
  rm -f "$out/downloads/report"*
  printf 'report\n' > "$out/site/report.docx"
  stops=$(grep -c '^browser: agent mcp-client stopped ' "$log" || true)
  downloaded=$(call open_tab "{\"url\":\"files.localhost:$port/report.docx\"}")
  [ "$downloaded" = "Downloaded report.docx (7 bytes) to downloads. Agents can't open files; ask the user if you need it." ] ||
    fail "the download at Work on pages answered: $downloaded"
  [ -f "$out/downloads/report.docx" ] || fail "the download isn't in the check's folder"
  file_tab=$(grep '^browser: agent download [0-9]* [0-9]* [0-9]* state ' "$log" | tail -1 |
    cut -d' ' -f5)
  listed=$(call list_tabs '{}')
  printf '%s\n' "$listed" | grep -q "^. $file_tab  " && fail "the download's empty tab stayed open"
  [ "$(grep -c '^browser: agent mcp-client stopped ' "$log" || true)" = "$stops" ] ||
    fail "ilo stopped a download at Work on pages: $(grep '^browser: agent mcp-client stopped ' "$log" | tail -1)"
  # As does one a tab the agent's tab opened starts (#2660): the tab is the agent's from when it's
  # made, so its download lands at Work on pages too, in the activity log as the agent's.
  if [ -n "$opened_said" ]; then
    rm -f "$out/downloads/opened"*
    printf 'opened\n' > "$out/site/opened.docx"
    printf '%s' "<!doctype html><title>Files</title><a href=\"//files.localhost:$port/opened.docx\" target=\"_blank\">Report</a>" \
      > "$out/site/files.html"
    opened=$(call open_tab "{\"url\":\"localhost:$port/files.html\"}")
    files=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
    ref=$(printf '%s\n' "$opened" | sed -n 's/.*- link "Report" \[\(e[0-9]*\)\].*/\1/p' | head -1)
    [ -n "$files" ] && [ -n "$ref" ] || fail "the page with a link to a file didn't open with its link: $opened"
    landed='^browser: agent logged mcp-client download files\.localhost$'
    before=$(grep -c "$landed" "$log" || true)
    call click "{\"tab\":$files,\"ref\":\"$ref\"}" > /dev/null
    await_more "$landed" "$before" ||
      fail "a download a tab the agent's tab opened started isn't in the activity log as the agent's; the last download lines:
$(grep 'browser: agent .*download' "$log" | tail -3)"
    for _ in $(seq 50); do
      [ -f "$out/downloads/opened.docx" ] && break
      sleep 0.1
    done
    [ -f "$out/downloads/opened.docx" ] ||
      fail "the opened tab's download isn't in the check's folder: $(ls -A "$out/downloads" | tr '\n' ' ')"
    [ "$(grep -c '^browser: agent mcp-client stopped ' "$log" || true)" = "$stops" ] ||
      fail "ilo stopped a download a tab the agent's tab opened started: $(grep '^browser: agent mcp-client stopped ' "$log" | tail -1)"
    # Left open: the agent is at work in it, which the session's end below ends.
  fi
  # A site whose page's download link the agent clicks with another app in front below (#3183). A
  # .docx, which the check's server sends as a file, where a .txt would open as a page. The link
  # opens a tab, which the download closes at once, as many sites' do: before #3104 that tab's
  # window show asked the compositor to bring ilo forward.
  printf 'notes\n' > "$out/site/notes.docx"
  printf '%s' '<!doctype html><title>Notes</title><a href="notes.docx" target=_blank>Notes</a>' \
    > "$out/site/notes.html"
  downloaded=$(call open_tab "{\"url\":\"saved.localhost:$port/notes.docx\"}")
  case "$downloaded" in
    "Downloaded notes.docx "*) ;;
    *) fail "the second site's download answered: $downloaded" ;;
  esac
  saved_page=$(call open_tab "{\"url\":\"saved.localhost:$port/notes.html\"}")
  notes_tab=$(printf '%s\n' "$saved_page" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  notes_ref=$(printf '%s\n' "$saved_page" | sed -n 's/.*- link "Notes" \[\(e[0-9]*\)\].*/\1/p' | head -1)
  [ -n "$notes_tab" ] && [ -n "$notes_ref" ] ||
    fail "the agent's tab of the site it downloads from didn't open with its link: $saved_page"

  # Paying, placing an order, sending a message as the user and a bank's site aren't part of Work
  # on pages, where every task starts: ilo refuses each at once, asking nothing (#4086), the agent
  # hears that only the user changes a task's access, the stop is logged, and nothing is done on the
  # page. Each refusal is ilo's line in the conversation, with the step up that would allow it, and
  # once the user gives Full access there, by the keys, the same steps go through. $level says
  # which the task has.
  level="Work on pages"
  printf '%s' "<title>Checkout</title>
<form onsubmit=\"event.preventDefault(); document.title = 'Paid'\">
<input aria-label=\"Card number\"><button>Pay now</button>
<label><input type=\"checkbox\"> Save this card</label></form>" > "$out/site/checkout.html"
  checkout_url="{\"url\":\"shop.localhost:$port/checkout.html\"}"
  shop_tab=
  # Opens the checkout in a tab of the agent's, closing the last, and sets shop_tab, card_ref and
  # save_ref.
  checkout() {
    [ -z "$shop_tab" ] || call close_tab "{\"tab\":$shop_tab}" > /dev/null
    opened=$(call open_tab "$checkout_url")
    shop_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
    card_ref=$(printf '%s\n' "$opened" |
      sed -n 's/.*- textbox "Card number" \[\(e[0-9]*\)\].*/\1/p')
    save_ref=$(printf '%s\n' "$opened" |
      sed -n 's/.*- checkbox "Save this card" \[\(e[0-9]*\)\].*/\1/p')
    [ -n "$shop_tab" ] && [ -n "$card_ref" ] && [ -n "$save_ref" ] ||
      fail "the checkout didn't open with its fields: $opened"
  }
  # Calls the tool $1 with $2 on the site $3's tab $4, an act ilo names by the words matching $5
  # and refuses at Work on pages as $6, the row of the levels' table it's under; done, the page is
  # titled $7. At Work on pages the answer says ilo stopped it and why, the stop is logged by those
  # words and the page isn't titled so; at Full access the answer has the page titled so. Sets did
  # to the answer.
  does_at() {
    stop="^browser: agent mcp-client stopped $6 at Work on pages: $5\$"
    before=$(grep -c "$stop" "$log" || true)
    did=$(call "$1" "$2")
    case "$level" in
      "Work on pages")
        case "$did" in
          "error: ilo stopped this: "*" on $3"*". It isn't part of this task's access (Work on pages). Only the user can change a task's access, in ilo. Don't try another way to the same end; if they change it, the same call will go through.") ;;
          *) fail "$1 $2 at Work on pages answered: $did" ;;
        esac
        await_more "$stop" "$before" ||
          fail "$1 $2 at Work on pages wasn't logged as ilo's stop by its words: $(grep '^browser: agent mcp-client stopped ' "$log" | tail -1)"
        grep -q "^browser: title $4 $7\$" "$log" && fail "$1 $2, which ilo stopped at Work on pages, was done all the same"
        ;;
      *)
        case "$did" in
          *"Tab $4: $7"*) ;;
          *) fail "$1 $2 at $level didn't do it: $did" ;;
        esac
        [ "$(grep -c "$stop" "$log" || true)" = "$before" ] || fail "$1 $2 was stopped at $level"
        ;;
    esac
    return 0
  }
  # A key press that activates a control is judged as a click on it is, on a site that sends no
  # messages (GHSA-hqw3-jp9p-3rhj): typing in a card's field with submit, Enter after typing in it,
  # Space and Enter on its Pay now button, which Tab brought the keyboard to, and Enter on the box
  # to save the card, which submits its form too, are each a payment. Whether the outline says the
  # button has the keyboard is the stop's to say.
  pay_steps() {
    checkout
    does_at type "{\"tab\":$shop_tab,\"ref\":\"$card_ref\",\"text\":\"4242\",\"submit\":true}" shop.localhost "$shop_tab" \
      'press Enter, which may click “Pay now”, after typing “4242” into “Card number”' Order Paid
    checkout
    typed=$(call type "{\"tab\":$shop_tab,\"ref\":\"$card_ref\",\"text\":\"4242\"}")
    case "$typed" in
      "Typed into “Card number”."*) ;;
      *) fail "typing in the card's field answered: $typed" ;;
    esac
    does_at press_key "{\"tab\":$shop_tab,\"key\":\"Enter\"}" shop.localhost "$shop_tab" \
      'press Enter, which may click “Pay now”' Order Paid
    checkout
    call type "{\"tab\":$shop_tab,\"ref\":\"$card_ref\",\"text\":\"4242\"}" > /dev/null
    call press_key "{\"tab\":$shop_tab,\"key\":\"Tab\"}" > /dev/null
    does_at press_key "{\"tab\":$shop_tab,\"key\":\"Space\"}" shop.localhost "$shop_tab" \
      'press Space\( to click\|, which may click\) “Pay now”' Order Paid
    if [ "$level" = "Work on pages" ]; then
      case "$(grep '^browser: agent mcp-client stopped Order ' "$log" | tail -1)" in
        *"press Space to click "*) space_said="the button it had the keyboard on" ;;
        *) space_said="the page's paying button, its outline not saying what had the keyboard" ;;
      esac
    fi
    checkout
    call type "{\"tab\":$shop_tab,\"ref\":\"$card_ref\",\"text\":\"4242\"}" > /dev/null
    call press_key "{\"tab\":$shop_tab,\"key\":\"Tab\"}" > /dev/null
    does_at press_key "{\"tab\":$shop_tab,\"key\":\"Enter\"}" shop.localhost "$shop_tab" \
      'press Enter\( to click\|, which may click\) “Pay now”' Order Paid
    checkout
    call type "{\"tab\":$shop_tab,\"ref\":\"$card_ref\",\"text\":\"4242\"}" > /dev/null
    saved=$(call click "{\"tab\":$shop_tab,\"ref\":\"$save_ref\"}")
    case "$saved" in
      *"Tab $shop_tab: Paid"*) fail "a click on the box to save the card paid: $saved" ;;
    esac
    does_at press_key "{\"tab\":$shop_tab,\"key\":\"Enter\"}" shop.localhost "$shop_tab" \
      'press Enter, which may click “Pay now”' Order Paid
    # The last checkout stays open: the tab an agent last acted in shows it at work until it goes.
  }

  # A Send is a message on a site that isn't one of the messaging sites once the page holds a
  # message for it, and an order is one in another language (GHSA-6qxg-jpw9-5jrr): a forum's
  # Comment with its form's multi-line field empty is a press as any other and posts at Work on
  # pages, and with the agent's text in it is a message; a chat's Send in no form is one beside a
  # rich-text editor holding text; a Danish shop's Betal is an order.
  printf '%s' "<title>Forum</title>
<form onsubmit=\"event.preventDefault(); document.title = document.title + ' posted'\">
<textarea aria-label=\"Body\"></textarea><button>Comment</button></form>" > "$out/site/forum.html"
  printf '%s' "<title>Chat</title>
<div contenteditable role=\"textbox\" aria-label=\"Message\"></div>
<button onclick=\"document.title = 'Sent'\">Send</button>" > "$out/site/chat.html"
  printf '%s' "<title>Butik</title>
<button onclick=\"document.title = 'Betalt'\">Betal</button>" > "$out/site/butik.html"
  # Opens $1 on the site $2 in a tab of the agent's, closing the last of these, and sets said_tab
  # and said_ref to the ref of its element named $3, of the role $4.
  said_tab=
  saying() {
    [ -z "$said_tab" ] || call close_tab "{\"tab\":$said_tab}" > /dev/null
    opened=$(call open_tab "{\"url\":\"$2.localhost:$port/$1\"}")
    said_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
    said_ref=$(printf '%s\n' "$opened" | sed -n "s/.*- $4 \"$3\" \[\(e[0-9]*\)\].*/\1/p" | head -1)
    [ -n "$said_tab" ] && [ -n "$said_ref" ] || fail "$1 didn't open with its $4 “$3”: $opened"
  }
  say_steps() {
    saying forum.html forum Comment button
    comment_ref=$said_ref
    body_ref=$(printf '%s\n' "$opened" | sed -n 's/.*- textbox "Body" \[\(e[0-9]*\)\].*/\1/p')
    [ -n "$body_ref" ] || fail "the forum didn't open with its Body field: $opened"
    posted=$(call click "{\"tab\":$said_tab,\"ref\":\"$comment_ref\"}")
    case "$posted" in
      *"Tab $said_tab: Forum posted"*) ;;
      *) fail "a Comment with nothing in its form's field was stopped at $level or didn't post: $posted" ;;
    esac
    saying forum.html forum Comment button
    comment_ref=$said_ref
    body_ref=$(printf '%s\n' "$opened" | sed -n 's/.*- textbox "Body" \[\(e[0-9]*\)\].*/\1/p')
    call type "{\"tab\":$said_tab,\"ref\":\"$body_ref\",\"text\":\"Looks good\"}" > /dev/null
    does_at click "{\"tab\":$said_tab,\"ref\":\"$comment_ref\"}" forum.localhost "$said_tab" \
      'click “Comment”' Message "Forum posted"
    saying chat.html chat Send button
    send_ref=$said_ref
    message_ref=$(printf '%s\n' "$opened" | sed -n 's/.*- textbox "Message" \[\(e[0-9]*\)\].*/\1/p')
    [ -n "$message_ref" ] || fail "the chat didn't open with its Message editor: $opened"
    call type "{\"tab\":$said_tab,\"ref\":\"$message_ref\",\"text\":\"On my way\"}" > /dev/null
    does_at click "{\"tab\":$said_tab,\"ref\":\"$send_ref\"}" chat.localhost "$said_tab" \
      'click “Send”' Message Sent
    saying butik.html butik Betal button
    does_at click "{\"tab\":$said_tab,\"ref\":\"$said_ref\"}" butik.localhost "$said_tab" \
      'click “Betal”' Order Betalt
  }
  # A bank's site isn't opened at Work on pages, and at Full access it is, where its Overfør moves
  # money as any order there goes through. Its charset said, as the site's server says none and
  # the page would read as Latin-1.
  bank="{\"url\":\"bank.localhost:$port/fields.html\"}"
  printf '%s' "<meta charset=utf-8><title>Konto</title>
<button onclick=\"document.title = 'Overført'\">Overfør</button>" > "$out/site/move.html"
  bank_steps() {
    stop='^browser: agent mcp-client stopped Bank at Work on pages: bank\.localhost$'
    before=$(grep -c "$stop" "$log" || true)
    allowed=$(call open_tab "$bank")
    case "$level" in
      "Work on pages")
        case "$allowed" in
          "error: ilo stopped this: open bank.localhost. It isn't part of this task's access (Work on pages). Only the user can change a task's access, in ilo."*) ;;
          *) fail "a bank's site at Work on pages answered: $allowed" ;;
        esac
        await_more "$stop" "$before" ||
          fail "the bank's site refused at Work on pages wasn't logged as ilo's stop: $(grep '^browser: agent mcp-client stopped ' "$log" | tail -1)"
        await_log '^browser: agent logged mcp-client open_tab bank\.localhost refused: Using a bank site isn.t part of Work on pages\.$' \
          "the bank's site refused at Work on pages isn't in the activity log in ilo's sentence: $(grep '^browser: agent logged mcp-client .* bank\.localhost' "$log" | tail -1)"
        ;;
      *)
        case "$allowed" in "Opened tab "*) ;; *) fail "a bank's site at $level answered: $allowed" ;; esac
        saying move.html bank Overfør button
        does_at click "{\"tab\":$said_tab,\"ref\":\"$said_ref\"}" bank.localhost "$said_tab" \
          'click “Overfør”' Order Overført
        ;;
    esac
  }
  # The refusals are read in the conversation, with no page of the task's watched: each stands
  # there as ilo's line, the newest with Give Full access, and what's on screen follows the log,
  # the button drawn where the log says it is.
  before=$(grep -c "^browser: task page $opened_tab back\$" "$log" || true)
  shrunk=$(grep -c "$thumbnail" "$log" || true)
  send key escape
  await_more "^browser: task page $opened_tab back\$" "$before" ||
    fail "Escape didn't step back to the conversation for the refusals' lines"
  await_more "$thumbnail" "$shrunk" || fail "the page watched didn't go back into its picture for the refusals' lines"
  ../tools/window-screenshot.sh "$pid" "$out/agent-stops-before.png" || fail "no window to capture"
  # The line drawn last with Give Full access, by its key, which counts up.
  gives() { sed -n 's/^browser: ilo line \([0-9]*\) Give Full access at .*/\1/p' "$log" | sort -n | tail -1; }
  pay_steps
  say_steps
  lined=$(gives)
  bank_steps
  for _ in $(seq 100); do
    newest=$(gives)
    [ "${newest:-0}" -gt "${lined:-0}" ] && break
    sleep 0.1
  done
  [ "${newest:-0}" -gt "${lined:-0}" ] ||
    fail "the bank's refusal drew no line of ilo's with Give Full access: $(grep '^browser: ilo line ' "$log" | tail -2 | tr '\n' ' ')"
  ../tools/window-screenshot.sh "$pid" "$out/agent-stops.png" || fail "no window to capture"
  follows_log "$out/agent-stops-before.png" "$out/agent-stops.png" \
    "$(sed -n "s/^browser: ilo line $newest Give Full access at //p" "$log" | tail -1)" \
    "Give Full access on ilo's newest line"
  # The keys press it once the app says a key on it counts: the task's access goes up, and nothing
  # of the agent's or a page's did it.
  raised='^browser: task [^ ]* access Work on pages to Full access$'
  grep -q "$raised" "$log" && fail "the task had Full access before the user gave it"
  ilo_press "Give Full access"
  await_log "$raised" "Give Full access on ilo's line didn't raise the task's access"
  await_log '^browser: agent logged mcp-client access ' "the change of the task's access isn't in the activity log"
  ../tools/window-screenshot.sh "$pid" "$out/agent-full-access.png" || fail "no window to capture"
  watch "$opened_tab"
  level="Full access"
  pay_steps
  say_steps
  bank_steps

  # Settings › Agents has no answer kept for good to list or revoke (#4086): nothing was asked, and
  # the access the user gave above is that task's alone.
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  settings_drew
  send clickkey $(drawn "toolbar Agents")
  expect '^browser: settings tab Agents$' "a click on Agents didn't show its section"
  settings_drew Agents
  grep -q '^browser: settings revoke ' "$log" &&
    fail "Settings › Agents listed an answer kept for good to revoke: $(grep '^browser: settings revoke ' "$log" | tail -1)"
  # Off limits (#3724): Add Site… opens a field with the keyboard, where Return adds the site
  # typed, which the list then shows with its Remove, and Remove takes it off.
  fence_add() {
    added=$(grep -c '^browser: agents fenced pension\.localhost$' "$log" || true)
    click_in_sight "fence open"
    expect '^browser: settings fence field open$' "Add Site… didn't open Off limits' field"
    expect '^browser: field Site agents can.t open or use has the keyboard$' \
      "Add Site… didn't give Off limits' field the keyboard"
    send type pension.localhost
    send key return
    await_more '^browser: agents fenced pension\.localhost$' "$added" ||
      fail "Return in Off limits' field didn't add the site"
  }
  fence_add
  ../tools/window-screenshot.sh "$pid" "$out/settings-agents-fence.png" || fail "no window to capture"
  click_in_sight "unfence pension.localhost"
  expect '^browser: agents unfenced pension\.localhost$' "Remove didn't take the site off Off limits"
  # Add Site… moves up with the row gone: its place from a frame drawn since, not the one before.
  drawn_after "fence open" '^browser: agents unfenced pension\.localhost$' > /dev/null ||
    fail "Off limits didn't draw again once its site was removed"
  fence_add
  in_sight "unfence pension.localhost" || fail "Off limits didn't list the site added again"
  # At the Settings window's least size, the foot under Add Site… wraps whole.
  send resizekey 720 480
  await_log '^browser: settings drawn at 720x480$' \
    "Settings didn't draw at its least size: $(grep '^browser: settings drawn at ' "$log" | tail -1)"
  in_sight "fence open" || fail "Off limits didn't draw Add Site… at the least size"
  # The foot's last line can be under the card's edge while Add Site… is in sight.
  in_sight "fence foot" || fail "Off limits didn't draw its foot at the least size"
  ../tools/window-screenshot.sh "$pid" "$out/settings-agents-fence-least.png" || fail "no window to capture"
  send resizekey 820 660
  await_log '^browser: settings drawn at 820x660$' "Settings didn't draw at its size again"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"

  # upload for a file field hands its step to the user, as hand_off does (#3656, #4086): the file
  # goes in by the page's own control, so ilo opens no dialog of its own and the agent hears only
  # that the tab came back, never what went in. The step says which field by its name, whichever
  # frame it's in: a frame of a subdomain of the tab's site, an object showing a page of it and
  # another site's frame. The step's page opens by itself, Hand Back on its line gives the tab
  # back, and Not Now leaves the step, the tab staying the user's until they hand it back.
  printf '%s' '<input type=file aria-label="Frame file">' > "$out/site/upload-frame.html"
  printf '%s' '<input type=file aria-label="Object file">' > "$out/site/upload-object.html"
  printf '%s' '<input type=file aria-label="Cross file">' > "$out/site/upload-cross.html"
  printf '%s' "<title>loading</title>
<iframe src=http://x.a.localhost:$port/upload-frame.html></iframe>
<object type=text/html data=http://x.a.localhost:$port/upload-object.html></object>
<iframe src=http://127.0.0.1:$port/upload-cross.html></iframe>
<script>let loading = 3;
for (const each of document.querySelectorAll('iframe, object'))
  each.onload = () => --loading || (document.title = 'upload frames');</script>" \
    > "$out/site/upload-frames.html"
  # Asks for a file for the field named $3, by the ref $2 in tab $1: the step is handed to the
  # user by the field's name, its page opens by itself, and Escape steps back from it to the
  # conversation, where the step's line stands.
  upload_handed() {
    handed="^browser: agent mcp-client hands off $1: Choose a file for “$3”\\.\$"
    before=$(grep -c "$handed" "$log" || true)
    opens=$(grep -c "^browser: task page $1 watched\$" "$log" || true)
    placed=$(grep -c "^browser: task page $1 in place\$" "$log" || true)
    backs=$(grep -c "^browser: task page $1 back\$" "$log" || true)
    shrunk=$(grep -c "$thumbnail" "$log" || true)
    request upload "{\"tab\":$1,\"ref\":\"$2\"}" >&4
    await_more "$handed" "$before" ||
      fail "upload for “$3” didn't hand its step to the user: $(grep 'browser: agent .*\(upload\|hands off\)' "$log" | tail -2 | tr '\n' ' ')"
    await_more "^browser: task page $1 watched\$" "$opens" ||
      fail "the page of the field “$3”, handed to the user, didn't open by itself"
    await_more "^browser: task page $1 in place\$" "$placed" ||
      fail "the page of the field “$3” didn't come to rest in place"
    send key escape
    await_more "^browser: task page $1 back\$" "$backs" ||
      fail "Escape didn't step back from the page of the field “$3”"
    await_more "$thumbnail" "$shrunk" || fail "the page of the field “$3” didn't go back into its picture"
  }
  # The steps' lines are pressed from the conversation, with no page of the task's watched.
  before=$(grep -c "^browser: task page $opened_tab back\$" "$log" || true)
  shrunk=$(grep -c "$thumbnail" "$log" || true)
  send key escape
  await_more "^browser: task page $opened_tab back\$" "$before" ||
    fail "Escape didn't step back to the conversation for the uploads"
  await_more "$thumbnail" "$shrunk" || fail "the page watched didn't go back into its picture for the uploads"
  upload_page "a.localhost:$port/upload-frames.html"
  frames_upload_tab=$upload_tab
  await_log "^browser: title $frames_upload_tab upload frames\$" "the upload page's frames didn't load"
  outline=$(call snapshot "{\"tab\":$frames_upload_tab}")
  printf '%s\n' "$outline" > "$out/mcp-upload-frames.txt"
  dialogs=$(grep -c 'browser: file field [0-9]* dialog over window ' "$log" || true)
  for each in "Frame file|a subdomain's frame" "Object file|an object" "Cross file|another site's frame"; do
    name=${each%%|*} what=${each##*|}
    # Its outline afresh, as a tab handed back says to take: the page was the user's meanwhile.
    outline=$(call snapshot "{\"tab\":$frames_upload_tab}")
    ref=$(ref_in "$outline" button "$name")
    [ -n "$ref" ] || fail "the upload page's outline had no ref for the field in $what: $outline"
    noted=$(grep -c '^browser: agent logged mcp-client upload a\.localhost$' "$log" || true)
    upload_handed "$frames_upload_tab" "$ref" "$name"
    await_more '^browser: agent logged mcp-client upload a\.localhost$' "$noted" ||
      fail "the upload handed to the user for the field in $what isn't in the activity log"
    if [ "$what" = "a subdomain's frame" ]; then
      ../tools/window-screenshot.sh "$pid" "$out/agent-upload.png" || fail "no window to capture"
    fi
    if [ "$what" = "another site's frame" ]; then
      ilo_press "Not Now"
      left=$(next_answer) || fail "the browser didn't answer the upload for the field in $what"
      left=$(printf '%s\n' "$left" | answered upload)
      case "$left" in
        "error: The user won't do the step on tab $frames_upload_tab now. It stays theirs; "*) ;;
        *) fail "Not Now on the upload's step for the field in $what answered: $left" ;;
      esac
      # The tab is the user's still, and ⌘↩ on its page watched hands it back.
      held=$(call snapshot "{\"tab\":$frames_upload_tab}")
      case "$held" in "error: "*) ;; *) fail "after Not Now, the agent read the tab left with the user: $held" ;; esac
      watch "$frames_upload_tab"
      handed=$(grep -c "^browser: agent mcp-client handed back $frames_upload_tab\$" "$log" || true)
      send key return cmd
      await_more "^browser: agent mcp-client handed back $frames_upload_tab\$" "$handed" ||
        fail "⌘↩ didn't hand back the tab left with the user by Not Now"
    else
      handed=$(grep -c "^browser: agent mcp-client handed back $frames_upload_tab\$" "$log" || true)
      ilo_press "Hand Back"
      await_more "^browser: agent mcp-client handed back $frames_upload_tab\$" "$handed" ||
        fail "Hand Back on the upload's step for the field in $what didn't hand the tab back"
      chosen=$(next_answer) || fail "the browser didn't answer the upload for the field in $what"
      chosen=$(printf '%s\n' "$chosen" | answered upload)
      [ "$chosen" = "The user handed tab $frames_upload_tab back. Take a snapshot to see where it is now." ] ||
        fail "the upload for the field in $what, handed back, answered: $chosen"
    fi
  done
  [ "$(grep -c 'browser: file field [0-9]* dialog over window ' "$log" || true)" = "$dialogs" ] ||
    fail "an upload handed to the user opened a file dialog of ilo's"
  # The page the user handed back is watched: back to the one watched before.
  before=$(grep -c "^browser: task page $frames_upload_tab back\$" "$log" || true)
  shrunk=$(grep -c "$thumbnail" "$log" || true)
  send key escape
  await_more "^browser: task page $frames_upload_tab back\$" "$before" ||
    fail "Escape didn't step back from the upload's page"
  await_more "$thumbnail" "$shrunk" || fail "the upload's page didn't go back into its picture"
  closed=$(call close_tab "{\"tab\":$frames_upload_tab}")
  case "$closed" in "Closed tab $frames_upload_tab."*) ;; *) fail "closing upload's tab $frames_upload_tab answered: $closed" ;; esac
  watch "$opened_tab"

  # A site the user put off limits is refused whatever the task's access, Full access here, with
  # nothing asked (#3724, #4086): an open of it, and each call on a tab that comes there by another
  # way, a link here, a read included; the click's answer and a forward's give nothing of the page.
  # Off the site again, the tab is the agent's to use as before. Each refusal is in the activity
  # log.
  printf '%s' '<!doctype html><title>Pension balance</title><p>Balance 1234</p>' \
    > "$out/site/pension.html"
  printf '%s' "<!doctype html><title>To the pension</title>
<p><a href=\"http://pension.localhost:$port/pension.html\">Pension</a></p>" > "$out/site/to-pension.html"
  fence_stops="^browser: agent mcp-client stopped Fenced at $level: pension\\.localhost\$"
  # Whether $1, the answer to $2, is ilo's refusal of $3 on the site, logged as its stop since
  # $before.
  fence_refused() {
    [ "$1" = "error: ilo stopped this: $3 pension.localhost. pension.localhost is off limits to agents, whatever a task's access. Only the user can change that, in ilo. Don't try another way to the same end." ] ||
      fail "$2 answered: $1"
    await_more "$fence_stops" "$before" ||
      fail "$2 wasn't logged as ilo's stop: $(grep '^browser: agent mcp-client stopped ' "$log" | tail -1)"
  }
  before=$(grep -c "$fence_stops" "$log" || true)
  fenced=$(call open_tab "{\"url\":\"pension.localhost:$port/pension.html\"}")
  fence_refused "$fenced" "the open of a site off limits" open
  await_log '^browser: agent logged mcp-client open_tab pension\.localhost refused: pension\.localhost is on your Off limits list\.$' \
    "the refused open of a site off limits isn't in the activity log in ilo's sentence: $(grep '^browser: agent logged mcp-client .* pension\.localhost' "$log" | tail -1)"
  listed=$(call list_tabs '{}')
  case "$listed" in *pension.localhost*) fail "the refused open of a site off limits left a tab on it: $listed" ;; esac
  went=$(call open_tab "{\"url\":\"localhost:$port/to-pension.html\"}")
  fence_tab=$(printf '%s\n' "$went" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  [ -n "$fence_tab" ] || fail "open_tab to-pension.html answered: $went"
  # A read the tab sends from its page, which Settings › Agents lists under the refusals to come.
  sent=$(call request "{\"tab\":$fence_tab,\"requests\":[{\"method\":\"GET\",\"path\":\"/api/me\"}]}")
  case "$sent" in *"GET /api/me → 200"*) ;; *) fail "a read from the tab beside the site off limits answered: $sent" ;; esac
  # Comes to the site by the link.
  link=$(printf '%s\n' "$went" | sed -n 's/.*- link "Pension" \[\(e[0-9]*\)\].*/\1/p' | head -1)
  [ -n "$link" ] || fail "the page with a link to the site off limits answered: $went"
  clicked=$(call click "{\"tab\":$fence_tab,\"ref\":\"$link\"}")
  case "$clicked" in
    "error: "* | *Balance* | *"Pension balance"*) fail "the click that came to the site off limits answered: $clicked" ;;
    *"Tab $fence_tab is now on pension.localhost, which is off limits to agents, so nothing of its page is shown and calls on it are refused."*) ;;
    *) fail "the click that came to the site off limits didn't say where the tab is: $clicked" ;;
  esac
  await_log "^browser: loaded $fence_tab http://pension\.localhost:$port/pension\.html\$" \
    "the link didn't take the tab to the site off limits"
  before=$(grep -c "$fence_stops" "$log" || true)
  fenced=$(call read_page "{\"tab\":$fence_tab}")
  fence_refused "$fenced" "the read of a tab that came to the site off limits" use
  await_log '^browser: agent logged mcp-client came_to pension\.localhost refused: pension\.localhost is on your Off limits list\.$' \
    "the refused read of a tab that came to the site off limits isn't in the activity log: $(grep '^browser: agent logged mcp-client came_to ' "$log" | tail -1)"
  # Back off the site and forward onto it again is refused again, whether the page comes back
  # from the back-forward cache or loads again; which it was goes in mcp-fence.txt. It guards the
  # order the fence rests on: the core's report of the page's address reaches the app before the
  # forward's answer, a restore's too, or the answer would carry the page.
  went=$(call navigate "{\"tab\":$fence_tab,\"url\":\"back\"}")
  case "$went" in "Went back."*'link "Pension"'*) ;; *) fail "back off the site off limits answered: $went" ;; esac
  loads=$(grep -c "^browser: loaded $fence_tab http://pension\.localhost:$port/pension\.html\$" "$log" || true)
  went=$(call navigate "{\"tab\":$fence_tab,\"url\":\"forward\"}")
  case "$went" in
    *Balance* | *"Pension balance"*) fail "forward onto the site off limits answered its page: $went" ;;
    "Went forward."*"Tab $fence_tab is now on pension.localhost, which is off limits to agents"*) ;;
    *) fail "forward onto the site off limits answered: $went" ;;
  esac
  before=$(grep -c "$fence_stops" "$log" || true)
  fenced=$(call read_page "{\"tab\":$fence_tab}")
  fence_refused "$fenced" "the read after forward onto the site off limits" use
  case "$(grep -c "^browser: loaded $fence_tab http://pension\.localhost:$port/pension\.html\$" "$log")" in
    "$loads") echo "forward onto the site off limits: restored from the back-forward cache, no load" ;;
    *) echo "forward onto the site off limits: loaded again" ;;
  esac >> "$out/mcp-fence.txt"
  # Off the site, the tab is the agent's to read again.
  went=$(call navigate "{\"tab\":$fence_tab,\"url\":\"localhost:$port/to-pension.html\"}")
  case "$went" in *'link "Pension"'*) ;; *) fail "off the site off limits again, the tab answered: $went" ;; esac
  # Settings › Agents' log has the refusals at the tab that came there, in the words of using the
  # site, over the read the tab sent before them.
  had=$(sed -n 's/^browser: settings request \([0-9]*\) GET at .*/\1/p' "$log" | sort -n | tail -1)
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings for the refusals' log"
  settings_drew
  # Settings shows the section it was last on, Agents since the foot's capture.
  if ! grep '^browser: settings shown ' "$log" | tail -1 | grep -q ' Agents$'; then
    send clickkey $(drawn "toolbar Agents")
    expect '^browser: settings tab Agents$' "a click on Agents didn't show its section for the log"
  fi
  settings_drew Agents
  # The read's row is the newest that opens, under the refusals: with it in sight, they show over
  # it. They open nothing, so draw no name. Its id is past every read's drawn before.
  for _ in $(seq 100); do
    newest=$(sed -n 's/^browser: settings request \([0-9]*\) GET at .*/\1/p' "$log" | sort -n | tail -1)
    [ "${newest:-0}" -gt "${had:-0}" ] && break
    sleep 0.1
  done
  [ "${newest:-0}" -gt "${had:-0}" ] || fail "Settings › Agents drew no read under the refusals"
  in_sight "request $newest GET" || fail "Settings › Agents didn't draw the read under the refusals"
  ../tools/window-screenshot.sh "$pid" "$out/settings-agents-fence-log.png" || fail "no window to capture"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings after the refusals' log"
  # The tab stays open, as the agent's last at work, which the end of its session reads.
fi
# The client closing its end ends its session, which ends its session with the tab.
exec 4>&-
for _ in $(seq 100); do kill -0 "$bridge_pid" 2>/dev/null || break; sleep 0.1; done
if kill -0 "$bridge_pid" 2>/dev/null; then
  # Who still holds the client's end open, if the bridge didn't hear it close.
  lsof "$out/mcp-in" >&2 || true
  fail "the bridge didn't stop once the client closed its end"
fi
exec 5<&-
if [ "$driven" = 1 ]; then
  await_log "^browser: agent session ended mcp-client $agent_tab disconnected\$" \
    "ending the client's session didn't end its session with the tab"
  # Its card ends on its receipt, which names only what had consequences (#1080): the five key
  # presses that paid at Full access, the Danish Betal and the bank's Overfør are its orders, the
  # Comment and the chat's Send its messages; what ilo stopped at Work on pages is none of them, the
  # bank's site it opened is no order or message, and its pages are Open's to count.
  # Library › Agents keeps it, in its agent's profile (#1080).
  await_log '^browser: agents history keeps session-[0-9]*-[0-9]* in profile [1-9]' \
    "the client's ended session wasn't kept for Library › Agents: $(grep '^browser: agents history' "$log" | tail -1)"
  await_log '^browser: task card session-[0-9]* Done Done · 7 orders placed · 2 messages sent$' \
    "the client's card didn't count the seven orders it placed and two messages it sent: $(grep '^browser: task card session-[0-9]* Done ' "$log" | tail -1)"
  grep '^browser: task card session-[0-9]* Done ' "$log" | grep -q '[0-9] pages\{0,1\}' &&
    fail "the client's receipt counted its pages, which Open counts: $(grep '^browser: task card session-[0-9]* Done ' "$log" | tail -1)"
  # The agent is at work in the tab it last acted in, which is still open.
  at=$(grep '^browser: agent mcp-client at work in ' "$log" | tail -1 | awk '{print $NF}')
  await_log "^browser: agent mcp-client done in $at\$" "the tab still shows a gone agent at work"
  # The card on its receipt, its status rolled in by now.
  ../tools/window-screenshot.sh "$pid" "$out/agent-receipt.png" || fail "no window to capture"
  # Library › Agents lists it under the title the agent gave it (#3690).
  send menu 'View Agents…'
  expect '^browser: library section agents$' "View Agents… didn't open the Library on Agents"
  await '^browser: library shown$' || fail "the Library didn't settle open on Agents"
  ../tools/window-screenshot.sh "$pid" "$out/agent-library.png" || fail "no window to capture"
  send menu 'View Agents…'
  expect '^browser: library hidden$' "View Agents… didn't close the Library"

  # Reconnected, the client's next call on a tab is a card of its own. Its conversation's Stop
  # answers the call under way that the user stopped it, then closes the client's connection,
  # which the bridge ends with, until the user reconnects it (#1080). The click comes while the
  # step under way, the hand_off waiting on the user, shimmers: the shimmer draws on the loading
  # dots' clock, and the driver reads its commands meanwhile, whatever the renderer (#1700).
  hello='^browser: agent mcp-client hello from check$'
  initialize='{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}'
  bridge_on
  hellos=$(grep -c "$hello" "$log" || true)
  mcp "$initialize" > /dev/null
  await_more "$hello" "$hellos" || fail "the app didn't hear the reconnected client's hello"
  sessions=$(grep -c '^browser: session session-[0-9]* of agent mcp-client as check$' "$log" || true)
  # Quiet until it needs you (#1080): with ilo in the background, the handoff posts one
  # notification. On Linux another app's window takes the keyboard, the run's server takes the
  # notification and clicks it, and on X11 the window is marked urgent, which Wayland lets no
  # client do to Chromium's surface. On Windows another app has the foreground and the toast is
  # posted, which no run can click. The Mac's runner can't answer its leave for notifications, so
  # there ilo stays active, and the need only counts on the Dock's badge.
  background=
  if [ "$(uname)" = Linux ] && [ -n "$notes" ] &&
    { [ -z "${BROWSER_WAYLAND:-}" ] || [ -n "${WAYLAND_DISPLAY:-}" ]; }; then
    rm -f "$out/other-in"
    mkfifo "$out/other-in"
    if [ -n "${WAYLAND_DISPLAY:-}" ]; then
      # check/drag's window, as lib.sh's other_app_click opens, which writes to it on fd 4, the
      # bridge's here. sway gives it the keyboard.
      cargo build -q --release --manifest-path check/drag/Cargo.toml --target-dir target ||
        fail "check/drag didn't build"
      : > "$out/other-window.log" # an earlier run's window stays named in it until emptied (#4040)
      target/release/drag 1280x800 window < "$out/other-in" > "$out/other-window.log" 2>&1 &
      other=$!
      exec 7> "$out/other-in"
      for _ in $(seq 100); do grep -q '^drag: ready$' "$out/other-window.log" && break; sleep 0.1; done
      grep -q '^drag: ready$' "$out/other-window.log" ||
        fail "check/drag's window didn't show: $(cat "$out/other-window.log")"
      swaymsg '[app_id=ilo-drag] focus' > /dev/null ||
        fail "sway didn't give check/drag's window the keyboard"
    else
      : > "$out/other-window.log" # an earlier run's window stays named in it until emptied (#4040)
      python3 -I check/other-window.py < "$out/other-in" > "$out/other-window.log" 2>&1 &
      other=$!
      exec 7> "$out/other-in"
      for _ in $(seq 50); do grep -q '^[0-9]' "$out/other-window.log" && break; sleep 0.1; done
      other_window=$(head -1 "$out/other-window.log")
      xdotool windowactivate --sync "$other_window" 2> /dev/null ||
        fail "another app's window didn't open and take the keyboard: $(cat "$out/other-window.log")"
    fi
    # The driver reads the keyboard's window from the X server, as the app does, and under Wayland
    # the app's own.
    send state
    [ "$active" = 0 ] || fail "another app's window left ilo active"
    background=linux
  elif windows; then
    rm -f "$out/cover-front"
    python3 -I -u check/app-windows.py cover "$(cat "/proc/$pid/winpid")" away "$(cygpath -w "$out/cover-front")" \
      > "$out/cover-needs.log" 2>&1 &
    helper=$!
    for _ in $(seq 100); do
      grep -q '^cover ' "$out/cover-needs.log" || ! kill -0 "$helper" 2>/dev/null && break
      sleep 0.1
    done
    grep -q '^cover .*, foreground away$' "$out/cover-needs.log" ||
      fail "another app didn't take the foreground: $(cat "$out/cover-needs.log")"
    background=windows
  fi
  # An agent's download in its own Space, from a click in its page, while another app's window has
  # the keyboard: ilo's window neither asks to come forward nor takes the keyboard (#3183), and the
  # download is in the downloads area and the activity log all the same. Under Wayland sway marks a window that asks to come forward urgent, and says so.
  if [ -n "$background" ]; then
    logged='^browser: agent logged mcp-client download saved\.localhost$'
    logs=$(grep -c "$logged" "$log" || true)
    lists=$(grep -c '^browser: downloads [0-9]*$' "$log" || true)
    flying=$(grep -c '^browser: download flying ' "$log" || true)
    # Not $marked, which send state sets to the driver's marked window.
    urgent_marks=$(grep -c '^browser: window [0-9]* urgent on$' "$log" || true)
    # Its link's ref afresh, as the client reconnected since.
    saved_page=$(call snapshot "{\"tab\":$notes_tab}")
    notes_ref=$(printf '%s\n' "$saved_page" | sed -n 's/.*- link "Notes" \[\(e[0-9]*\)\].*/\1/p' | head -1)
    [ -n "$notes_ref" ] || fail "the agent's tab of the site it downloads from has no link: $saved_page"
    call click "{\"tab\":$notes_tab,\"ref\":\"$notes_ref\"}" > /dev/null
    await_more "$logged" "$logs" ||
      fail "the agent's download with another app in front isn't in the activity log: $(grep 'browser: agent .*download' "$log" | tail -3 | tr '\n' ' ')"
    await_more '^browser: downloads [0-9]*$' "$lists" ||
      fail "the agent's download with another app in front isn't in the downloads area: $(grep '^browser: download' "$log" | tail -3 | tr '\n' ' ')"
    await_log '^browser: download [0-9]* done notes' "the agent's download with another app in front didn't finish"
    if [ "$(grep -c '^browser: download flying ' "$log")" -gt "$flying" ]; then
      await_log '^browser: download landed ' "the agent's download's icon didn't land in the bin"
    fi
    send state
    [ "$active" = 0 ] ||
      fail "the agent's download took the keyboard from the other app's window: it's in $key, on $focus"
    [ "$(grep -c '^browser: window [0-9]* urgent on$' "$log" || true)" = "$urgent_marks" ] ||
      fail "the agent's download marked ilo's window urgent"
    # Sway marks a window urgent when it asks to come forward while another has the keyboard, and
    # keeps the mark until the window has the keyboard, so its tree says whether ilo's asked.
    if [ -n "${WAYLAND_DISPLAY:-}" ]; then
      asked=$(swaymsg -t get_tree | jq -r --argjson pid "$(app_pid)" \
        '.. | objects | select(.pid? == $pid and ((.urgent // false) or (.focused // false)))
          | "\(.app_id) \(.name) urgent \(.urgent) focused \(.focused)"')
      [ -z "$asked" ] ||
        fail "the agent's download asked for ilo's window to come forward, as sway's tree says: $(printf '%s' "$asked" | tr '\n' ' ')"
    fi
  fi
  counted() { grep -c "$1" "$log" || true; }
  ones=$(counted '^browser: needs you 1$')
  urgents=$(counted '^browser: window [0-9]* urgent on$')
  badges=$(counted '^browser: \(dock\|taskbar\) badge 1$')
  opens=$(counted "^browser: task page $opened_tab watched\$")
  placed=$(counted "^browser: task page $opened_tab in place\$")
  backs=$(counted "^browser: task page $opened_tab back\$")
  request hand_off "{\"tab\":$opened_tab,\"reason\":\"Sign in once more\"}" >&4
  await_more '^browser: session session-[0-9]* of agent mcp-client as check$' "$sessions" ||
    fail "the reconnected client's call started no card of its own"
  session=$(sed -n 's/^browser: session \(session-[0-9]*\) of agent mcp-client as check$/\1/p' "$log" | tail -1)
  await_log "^browser: agent mcp-client hands off $opened_tab: Sign in once more\$" \
    "the reconnected client's hand_off didn't hand the tab over"
  await_log "^browser: task card $session Waiting " "the reconnected client's card didn't wait for the user"
  await_log "^browser: task plate task-${session#session-} at " "the reconnected client's card wasn't drawn"
  needing=$session
  await_more '^browser: needs you 1$' "$ones" || fail "the waiting card didn't count as a need"
  case $background in
    linux)
      if [ -z "${WAYLAND_DISPLAY:-}" ]; then
        await_more '^browser: window [0-9]* urgent on$' "$urgents" ||
          fail "the waiting card didn't mark the window urgent"
      fi
      await_log "^browser: needs you notified [0-9]* $needing\$" "the need in the background posted nothing"
      need=$(sed -n "s/^browser: needs you notified \([0-9]*\) $needing\$/\1/p" "$log" | tail -1)
      await_log "^browser: needs you posted $need\$" \
        "the notification didn't reach the desktop: $(grep '^browser: needs you ' "$log" | tail -1)"
      # Named by its client, never by its title or its step, which are the agent's words.
      note=$(grep '^notify [0-9]* check · Needs you$' "$notes" | tail -1 | cut -d' ' -f2)
      [ -n "$note" ] || fail "the desktop's server got no \"check · Needs you\": $(cat "$notes")"
      grep -qx "body $note " "$notes" ||
        fail "the notification says more than its title: $(grep "^body $note " "$notes")"
      grep -x "hints $note .*" "$notes" | grep -q ' suppress-sound' ||
        fail "the notification may sound: $(grep "^hints $note " "$notes")"
      [ "$(grep -c '^notify ' "$notes")" = 1 ] || fail "a need posted more than once: $(cat "$notes")"
      # Its click opens the task's card, the app asking to come forward, which the window manager
      # decides; the run gives the keyboard back, as a click on the window would.
      echo "click $note" >&6
      await_log "^browser: needs you clicked $need $needing\$" "the notification's click reached no task"
      await_log "^browser: task $needing shown\$" "the notification's click didn't open its task's card"
      exec 7>&-
      wait "$other" 2> /dev/null || true
      send window 1
      [ "$active" = 1 ] || fail "the window didn't take the keyboard back"
      ;;
    windows)
      await_more '^browser: taskbar badge 1$' "$badges" ||
        fail "the taskbar button's overlay didn't count the need"
      await_log "^browser: needs you notified [0-9]* $needing\$" "the need in the background posted nothing"
      need=$(sed -n "s/^browser: needs you notified \([0-9]*\) $needing\$/\1/p" "$log" | tail -1)
      await_log "^browser: needs you posted $need" "the toast wasn't posted"
      # Windows shows a toast only for a copy its installer gave a Start menu shortcut, or an MSIX,
      # as Chrome's own, and the run's copy is neither: it's refused as unregistered.
      posted=$(grep "^browser: needs you posted $need" "$log" | tail -1)
      case "$posted" in
        "browser: needs you posted $need") ;;
        "browser: needs you posted $need: Element not found."*) toast_skip=1 ;;
        *) fail "the toast wasn't posted: $posted" ;;
      esac
      touch "$out/cover-front"
      for _ in $(seq 100); do kill -0 "$helper" 2>/dev/null || break; sleep 0.1; done
      helper=
      ;;
    *)
      await_log "^browser: needs you $needing while active\$" "the need with ilo active wasn't held back"
      if [ "$(uname)" = Darwin ]; then
        await_more '^browser: dock badge 1$' "$badges" || fail "the Dock's badge didn't count the need"
      else
        echo "skip: the need with ilo in the background, and its count on a badge: here no other app's window takes the keyboard from ilo, or no notification server takes the notification, which X11's and Wayland's runs have, and Linux has no badge every desktop reads."
      fi
      ;;
  esac
  case $background in
    linux) needs_said=", a handoff while another app's window has the keyboard $([ -n "${WAYLAND_DISPLAY:-}" ] || echo "marks the window urgent and ")posts one notification, \"check needs you\" over its step with no sound, whose click opens its card, and none while ilo is active" ;;
    windows) needs_said=", a handoff with another app in front counts on the taskbar button and asks for one toast, and none while ilo is active" ;;
    *) needs_said=", a handoff with ilo active counts on the Dock's badge and posts nothing" ;;
  esac
  # On Linux the notification's click showed its conversation, which a click on the card would
  # put away.
  if [ "$background" != linux ]; then
    read -r px py pw ph <<PLATE
$(grep "^browser: task plate task-${session#session-} at " "$log" | tail -1 | cut -d' ' -f6-)
PLATE
    send click $((px + pw / 2)) $((py + ph / 2))
    expect "^browser: task $session shown\$" "a click on the new card didn't show its conversation"
  fi
  # The handoff's page opens by itself over the conversation shown, none of the task's being
  # watched there, and Escape steps back from it to the conversation, where the task's Stop is.
  await_more "^browser: task page $opened_tab watched\$" "$opens" ||
    fail "the reconnected client's handoff didn't open its page by itself once its conversation showed"
  await_more "^browser: task page $opened_tab in place\$" "$placed" ||
    fail "the reconnected client's handoff's page didn't come to rest in place"
  send key escape
  await_more "^browser: task page $opened_tab back\$" "$backs" ||
    fail "Escape didn't step back from the reconnected client's handoff's page"
  # With motion reduced, as on GitHub's Windows runner, the step holds still rather than shimmer.
  if ! grep -q '^browser: reduce motion on$' "$log"; then
    await_log "^browser: task $session shimmers\$" "the conversation's step waiting on the user didn't shimmer"
  fi
  await_log '^browser: task stop at ' "the new card's conversation drew no Stop"
  read -r sx sy sw sh <<STOP
$(grep '^browser: task stop at ' "$log" | tail -1 | cut -d' ' -f5-)
STOP
  ../tools/window-screenshot.sh "$pid" "$out/agent-session.png" || fail "no window to capture"
  send click $((sx + sw / 2)) $((sy + sh / 2))
  expect '^browser: command stop-task via conversation$' "the click on Stop while the step shimmered didn't arrive"
  expect '^browser: agent mcp-client stopped by the user$' "Stop didn't stop the client"
  # No task waits once it's stopped, and the count and the urgency go with it.
  # Waits for the last line matching $1 to be $2, for up to 10 s.
  last_is() {
    for _ in $(seq 100); do
      [ "$(grep "$1" "$log" | tail -1)" = "$2" ] && return 0
      sleep 0.1
    done
    return 1
  }
  last_is '^browser: needs you [0-9]*$' "browser: needs you 0" || fail "the count stayed once no task waited"
  # Wayland has no urgency to take back (needs_you.rs).
  if [ "$background" = linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
    window_urgent=$(grep '^browser: window [0-9]* urgent ' "$log" | tail -1 | cut -d' ' -f3)
    last_is '^browser: window [0-9]* urgent ' "browser: window $window_urgent urgent off" ||
      fail "the window stayed urgent once no task waited"
  fi
  stopped=$(next_answer) || fail "the browser didn't answer the hand_off it stopped"
  stopped=$(printf '%s\n' "$stopped" | answered "the hand_off it stopped")
  case "$stopped" in
    "error: The user stopped you."*) ;;
    *) fail "the hand_off stopped answered: $stopped" ;;
  esac
  await_log '^browser: agent mcp-client connection [0-9]* closed, as the user stopped it$' \
    "Stop didn't close the client's connection"
  for _ in $(seq 100); do kill -0 "$bridge_pid" 2>/dev/null || break; sleep 0.1; done
  kill -0 "$bridge_pid" 2>/dev/null && fail "the bridge stayed connected once the user stopped its client"
  exec 4>&- 5<&-

  # Off in Settings › Agents takes the socket away and drops a client connected.
  bridge_on
  hellos=$(grep -c "$hello" "$log" || true)
  mcp "$initialize" > /dev/null
  await_more "$hello" "$hellos" || fail "the app didn't hear the second client's hello"
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  settings_drew
  # Agents, unless Settings came back on it.
  if ! grep '^browser: settings shown ' "$log" | tail -1 | grep -q ' Agents$'; then
    send clickkey $(drawn "toolbar Agents")
    expect '^browser: settings tab Agents$' "a click on Agents didn't show its section"
  fi
  settings_drew Agents
  ../tools/window-screenshot.sh "$pid" "$out/settings-agents.png" || fail "no window to capture"
  click_in_sight "switch agents"
  expect '^browser: agents off$' "a click on the switch didn't turn agents off"
  for _ in $(seq 30); do [ -e "$socket" ] || break; sleep 0.1; done
  [ -e "$socket" ] && fail "the socket stayed once agents were off"
  for _ in $(seq 100); do kill -0 "$bridge_pid" 2>/dev/null || break; sleep 0.1; done
  kill -0 "$bridge_pid" 2>/dev/null && fail "the client stayed connected once agents were off"
  exec 4>&- 5<&-
  if BROWSER_AGENT_TOKEN=$token "$bridge" "$socket" < /dev/null > /dev/null 2>&1; then
    fail "the bridge got in once agents were off"
  fi
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
fi
quit

# A tab's id names it alone across launches (#1449): relaunched, a call with the user's tab's id
# from before is refused as one from before ilo restarted, as the tab it comes back as has another.
# The switch the run turned off goes back on. The launch waits 3 s for a handoff in place of the
# spec's 15 minutes, which the time limit's step below reaches, and ends a task 3 s after its last
# page closed in place of 60.
printf '{"on":true,"agents":[{"client":"mcp-client","token":"%s"}]}' "$token" > "$agents_file"
log="$out/mcp-waits.log"
step=0
BROWSER_DRIVE=1 "$browser" --agent-waits=2,3,3 < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch didn't restore the Spaces"
await "$on" || fail "the relaunch didn't start the server"
socket=$(grep '^browser: agents server on' "$log" | tail -1 | sed 's/^browser: agents server on //')
bridge_on
mcp '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}' \
  > /dev/null
mcp '{"jsonrpc":"2.0","method":"notifications/initialized"}'
stale=$(call read_page "{\"tab\":$user_tab}")
[ "$stale" = "error: ilo restarted, so the tabs you knew are gone. Call list_tabs for the tabs open now." ] ||
  fail "a call with a tab's id from before the relaunch answered: $stale"
# The time limit (#2184): a handoff not handed back answers the agent's call, the tab staying the
# user's.
opened=$(call open_tab "{\"url\":\"localhost:$port/frames.html\"}")
waits_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
[ -n "$waits_tab" ] || fail "open_tab answered: $opened"
request hand_off "{\"tab\":$waits_tab,\"reason\":\"Sign in later\"}" >&4
await_log "^browser: agent mcp-client hands off $waits_tab: Sign in later\$" \
  "hand_off didn't hand the tab to the user"
# done sent while the handoff waits on the user is answered after it, as a call with no tab waits
# for the calls before it, so it never leaves a question behind (#3652).
request done '{}' >&4
handed=$(next_answer) || fail "the browser didn't answer the handoff once its 3 s wait ended"
handed=$(printf '%s\n' "$handed" | answered hand_off)
case "$handed" in
  "error: The user hasn't handed tab $waits_tab back in 15 minutes."*) ;;
  *) fail "the handoff not handed back answered: $handed" ;;
esac
# Then done ends the agent's task: its card is done, the activity log says so, and its next done has
# no task to end.
waits_session=$(sed -n 's/^browser: session \(session-[0-9]*\) of agent mcp-client as check$/\1/p' "$log" | tail -1)
said=$(next_answer) || fail "the browser didn't answer done once the handoff was answered"
said=$(printf '%s\n' "$said" | answered done)
case "$said" in
  "Your task is done"*) ;;
  *) fail "done sent while the handoff waited answered: $said" ;;
esac
await_log "^browser: session $waits_session done\$" "done didn't end the agent's task: $(grep '^browser: session ' "$log" | tail -2)"
await_log '^browser: agent logged mcp-client done $' "done isn't in the activity log: $(grep '^browser: agent logged mcp-client done' "$log" | tail -2)"
again=$(call done '{}')
[ "$again" = "No task of yours is under way." ] || fail "a second done answered: $again"
# Its next call is a new task, which ends once 3 s pass after its close of its last page, as one
# that doesn't call done ends.
opened=$(call open_tab "{\"url\":\"localhost:$port/frames.html\"}")
quiet_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
[ -n "$quiet_tab" ] || fail "open_tab after done answered: $opened"
quiet_session=$(sed -n 's/^browser: session \(session-[0-9]*\) of agent mcp-client as check$/\1/p' "$log" | tail -1)
[ "$quiet_session" != "$waits_session" ] || fail "the call after done didn't start a new task: $quiet_session"
closed=$(call close_tab "{\"tab\":$quiet_tab}")
case "$closed" in "Closed tab $quiet_tab."*) ;; *) fail "closing the new task's page answered: $closed" ;; esac
await_log "^browser: session $quiet_session quiet for 3 s after its last page\$" \
  "the task didn't end once 3 s passed after its last page closed: $(grep '^browser: session ' "$log" | tail -2)"
await_log "^browser: session $quiet_session done\$" "the quiet task's card didn't say it's done"
# A page in the agent's tab that asks for a passkey never gets one for the agent (principle 3).
# The agent's tab is out of the user's sight, its page unfocused, so Blink refuses the request
# itself, as the page doesn't have focus. A page that has the focus reaches Chrome, where the
# core hands the tab to the user before Chrome's dialog asks, with Touch ID's authenticator among
# those found where the Mac has one, and the agent is refused the tab until they hand it back
# (g_browser_core_refuse_passkey). Either way no passkey is made, and the run says which it saw.
printf '%s' '<title>loading</title>
<script>
onload = () => {
  document.title = "asking";
  navigator.credentials.create({ publicKey: {
    rp: { id: "localhost", name: "Check" },
    user: { id: new Uint8Array(16), name: "ada@check.test", displayName: "Ada" },
    challenge: crypto.getRandomValues(new Uint8Array(32)),
    pubKeyCredParams: [{ type: "public-key", alg: -7 }],
  } }).then(() => document.title = "made",
    error => document.title = "failed " + error.name + ": " + error.message);
};
</script>' > "$out/site/passkey-agent.html"
opened=$(call open_tab "{\"url\":\"localhost:$port/passkey-agent.html\"}")
passkey_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
[ -n "$passkey_tab" ] || fail "open_tab passkey-agent.html answered: $opened"
await_log "^browser: \(agent mcp-client hands off $passkey_tab: passkey\|title $passkey_tab \(made\|failed .*\)\)\$" \
  "a passkey request in the agent's tab neither reached the user nor ended"
grep -q "^browser: title $passkey_tab made\$" "$log" && fail "the agent's page made a passkey"
if grep -q "^browser: agent mcp-client hands off $passkey_tab: passkey\$" "$log"; then
  refused=$(call scroll "{\"tab\":$passkey_tab}")
  case "$refused" in
    "error: The user has this tab until they hand it back."*) ;;
    *) fail "acting in the tab its passkey request handed to the user answered: $refused" ;;
  esac
  passkey_said="handed the tab to the user, the agent refused it"
else
  failed=$(grep "^browser: title $passkey_tab failed " "$log" | tail -1 | cut -d' ' -f5-)
  case "$failed" in
    "NotAllowedError: The operation is not allowed at this time because the page does not have focus.") ;;
    *) fail "the agent's page's passkey request failed otherwise than for its unfocused page: $failed" ;;
  esac
  passkey_said="was refused by Blink, the agent's page being unfocused"
fi
exec 4>&- 5<&-
quit
# Nothing of the password typed in the agent's sign-in is kept in the profile, the agents' 30-day
# file in it among it (#3662).
if grep -rlF zq7lovelace "$profile_dir" > "$out/mcp-sign-in-kept.txt" 2> /dev/null; then
  fail "the password typed in the agent's sign-in was kept: $(tr '\n' ' ' < "$out/mcp-sign-in-kept.txt")"
fi
forget_login
# The window quit showing the client's Space, which with no window left stays for the next launch to
# delete (#1676): launched with a page, the window shows the user's Space, with the page in front.
log="$out/mcp-relaunch.log"
step=0
BROWSER_DRIVE=1 BROWSER_URL='data:text/html,<title>front</title>' "$browser" < "$out/drive" \
  > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: title [0-9]* front$' || fail "the page the relaunch opened didn't load"
front_tab=$(grep '^browser: title [0-9]* front$' "$log" | tail -1 | cut -d' ' -f3)
send state
[ "$(grep '^browser: window [0-9]* shows ' "$log" | tail -1 | cut -d' ' -f5)" = "$front_tab" ] ||
  fail "the relaunch's page, tab $front_tab, isn't the one the window shows: $(grep '^browser: window [0-9]* shows ' "$log" | tail -1)"
quit
# An agent's tab has a page size of its own, whatever the user's window shows (#3788): on a new
# profile's Welcome, with no page for Chromium's tab strip to size it by, it's still pictured, and
# with a page in a smaller window it's pictured at the same size, laid out alike. A pop-up its
# page opens has its size too. The page's buttons follow its width, so its outline says which
# layout it has.
own_size_said=
if engine_has ../core/window.cc ../core/agent_page.cc; then
  printf '%s' '<!doctype html><title>Widths</title><style>
#narrow, #middle, #wide { display: none }
@media (max-width: 499.98px) { #narrow { display: block } }
@media (min-width: 500px) and (max-width: 849.98px) { #middle { display: block } }
@media (min-width: 850px) { #wide { display: block } }
</style><button id=narrow>Narrow layout</button><button id=middle>Middle layout</button>
<button id=wide>Wide layout</button>
<button onclick="open(&quot;widths.html?popup&quot;, &quot;&quot;, &quot;popup&quot;)">Pop up</button>' \
    > "$out/site/widths.html"
  printf '%s' '<!doctype html><title>yours</title><script>
const middle = matchMedia("(min-width: 500px) and (max-width: 849.98px)");
const say = () => document.title = "yours " + (middle.matches ? "middle" : "other");
say(); middle.onchange = say;
</script>' > "$out/site/widths-yours.html"
  rm -rf "$profile_dir"
  mkdir -p "$profile_dir/Default"
  printf '{"on":true,"agents":[{"client":"mcp-client","token":"%s"}]}' "$token" > "$agents_file"
  # The size the agent's last picture of |tab| had, as WxH.
  pictured_at() {
    grep "browser: agent picture of tab $1 at " "$log" | tail -1 | sed 's/.* at //'
  }
  # The agent's tab of the widths page, pictured, then the pop-up it opens, as
  # "<size> <layout> <pop-up's size> <answer>".
  own_size() {
    mcp '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}' \
      > /dev/null
    mcp '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    opened=$(call open_tab "{\"url\":\"localhost:$port/widths.html\"}")
    sized_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
    [ -n "$sized_tab" ] || fail "open_tab of the widths page answered: $opened"
    picture=$(call screenshot "{\"tab\":$sized_tab}")
    size=$(pictured_at "$sized_tab")
    # Its first word alone, as the size and the answer around it are split on spaces.
    layout=$(call snapshot "{\"tab\":$sized_tab}" | grep -o '"[A-Z][a-z]* layout"' | tr -d '"' |
      head -1 | cut -d' ' -f1)
    ref=$(printf '%s\n' "$opened" | sed -n 's/.*- button "Pop up" \[\(e[0-9]*\)\].*/\1/p' | head -1)
    behind=$(grep -c "^browser: tab [0-9]* opened by $sized_tab in the background\$" "$log" || true)
    call click "{\"tab\":$sized_tab,\"ref\":\"$ref\"}" > /dev/null
    await_more "^browser: tab [0-9]* opened by $sized_tab in the background\$" "$behind" ||
      fail "the widths page's Pop up, which the agent clicked, opened no tab behind"
    popup=$(sed -n "s/^browser: tab \([0-9]*\) opened by $sized_tab in the background\$/\1/p" "$log" |
      tail -1)
    call screenshot "{\"tab\":$popup}" > /dev/null
    popup_size=$(pictured_at "$popup")
    echo "${size:-no-size} ${layout:-no-layout} ${popup_size:-no-size} $picture"
  }
  log="$out/mcp-own-size.log"
  step=0
  BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_log '^browser: welcome shown$' "a new profile's launch didn't show the Welcome"
  await_log "$on" "the new profile's launch didn't start the server"
  socket=$(grep '^browser: agents server on' "$log" | tail -1 | sed 's/^browser: agents server on //')
  bridge_on
  welcome=$(own_size)
  exec 4>&- 5<&-
  quit
  log="$out/mcp-own-size-page.log"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="http://localhost:$port/widths-yours.html" "$browser" < "$out/drive" \
    > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_log '^browser: title [0-9]* yours other$' "the user's page didn't load"
  # Narrower than the window a new profile opens, so the user's page takes its middle layout.
  send resize 900 640
  await_log '^browser: title [0-9]* yours middle$' \
    "the user's page didn't take its middle layout in the window made 900 by 640"
  await_log "$on" "the relaunch didn't start the server"
  socket=$(grep '^browser: agents server on' "$log" | tail -1 | sed 's/^browser: agents server on //')
  bridge_on
  paged=$(own_size)
  exec 4>&- 5<&-
  quit
  echo "Welcome: $welcome" > "$out/mcp-own-size.txt"
  echo "A page in a 900 by 640 window: $paged" >> "$out/mcp-own-size.txt"
  case "$welcome" in
    *" image/jpeg "[1-9]*) ;;
    *) fail "the agent's tab opened while the user's window showed the Welcome wasn't pictured: $welcome" ;;
  esac
  # Each picture's size, from the core's line, and each layout, from the outline, was read.
  for said in "$welcome" "$paged"; do
    for field in 1 3; do
      echo "$said" | cut -d' ' -f$field | grep -q '^[1-9][0-9]*x[1-9][0-9]*$' ||
        fail "no size of the agent's page above 0 by 0 was read from its pictures (line \"browser: agent picture of tab <id> at WxH\"): $(cat "$out/mcp-own-size.txt")"
    done
    case "$(echo "$said" | cut -d' ' -f2)" in
      Narrow | Middle | Wide) ;;
      *) fail "the agent's widths page gave no layout in its outline: $(cat "$out/mcp-own-size.txt")" ;;
    esac
  done
  for said in "$welcome" "$paged"; do
    [ "$(echo "$said" | cut -d' ' -f1)" = "$(echo "$said" | cut -d' ' -f3)" ] ||
      fail "the pop-up the agent's page opened took another size than its opener's: $(cat "$out/mcp-own-size.txt")"
  done
  [ "${welcome%% *}" = "${paged%% *}" ] ||
    fail "the agent's page took another size with a page in the user's smaller window: $(cat "$out/mcp-own-size.txt")"
  [ "$(echo "$welcome" | cut -d' ' -f2)" = "$(echo "$paged" | cut -d' ' -f2)" ] ||
    fail "the agent's page was laid out otherwise with a page in the user's smaller window: $(cat "$out/mcp-own-size.txt")"
  own_size_said="ok: an agent's tab has a page size of its own, ${welcome%% *}, pictured on a new profile's Welcome and laid out alike with a page in the user's smaller window, and a pop-up its page opens has it too"
else
  own_size_said="skip: an agent's tab's own page size (#3788): not run on an engine without it"
fi
trap cleanup EXIT
rm -f "$agents_file" "$out/mcp-in" "$out/mcp-out"
# The closing line, its clauses joined by a space: one a line, so two changes that each add a step's
# clause merge by themselves, as they did not when the line was one (#3727).
clauses() {
  (IFS=' '; printf '%s\n' "$*")
}
if [ "$driven" = 1 ]; then
  echo "ok: $(clauses \
    "an MCP client's bridge gets in with its token only," \
    "on the user's own socket," \
    "is offered no Boost tool," \
    "gets a Space of its own," \
    "is refused the user's tab and reads its own," \
    "is answered that a link to an app opens nothing," \
    "its tab staying on its page," \
    "moves ten cases to Closed in tabs the user isn't looking at a step a turn," \
    "its calls on the ten answered at once ($cases)," \
    "the tabs its page opens kept in its Space," \
    "behind$opened_said," \
    "without its fields' values," \
    "acts by ref but never in a password field," \
    "presses keys that reach the page alone," \
    "its Escape running no Stop of ilo's," \
    "closes its own tab," \
    "pins," \
    "unpins and removes its own tab but not the user's," \
    "sends requests to its tab's site with the tab's session it never sees," \
    "a change refused at Work on pages and never sent," \
    "refused for its missing sentence before that," \
    "a delete's too," \
    "each opening in Settings › Agents to the agent's sentence," \
    "where it went and what it sent or didn't," \
    "asks a page's beforeunload handler first," \
    "with no dialog," \
    "closing one that doesn't ask at the first call and keeping one the agent clicked or typed in that asks until it discards it," \
    "runs steps in one call," \
    "waits on a handoff until Hand Back on ilo's line," \
    "its page opening by itself unless the user watches another," \
    "has its tab handed to the user for a sign-in the browser asks for," \
    "which shows over its page watched," \
    "its call waiting and the password never reaching it," \
    "works on one card for its connection," \
    "a second connection of the client being a card of its own that ends alone when it closes," \
    "whose conversation watches its page," \
    "opens its tabs muted," \
    "is refused a tab the user took over with a press in its page until they hand it back and Not Now answers its ask for it," \
    "has a tab the user holds listed as with them and nothing of its page," \
    "is stopped by its conversation's Stop while its waiting step shimmers," \
    "which closes its connection once its call is answered," \
    "gets no saved login filled in or offered in its tab until the user takes it over," \
    "when it fills without a reload," \
    "its download at Work on pages answers with the file," \
    "its upload hands the field's step to the user by the field's name," \
    "in a subdomain's frame," \
    "an object and another site's frame," \
    "ilo opening no dialog," \
    "Not Now leaving the tab theirs," \
    "and is refused unasked for a folder's field," \
    "a button," \
    "a sandboxed frame's field and a document whose root frames none," \
    "a key press that pays is stopped at Work on pages as a click on Pay now is," \
    "typing with submit," \
    "Enter in the card's field," \
    "Space after Tab stopped by $space_said," \
    "Enter after Tab," \
    "and Enter on the box to save the card," \
    "a Send off the messaging sites is stopped once its page holds a message," \
    "a forum's Comment and a chat's Send in no form," \
    "and not with its field empty," \
    "a Danish Betal and a bank's site are stopped," \
    "each stop ilo's line with Give Full access on screen as the log draws it ($follows% of its place changed)," \
    "which the keys press," \
    "and each goes through at Full access," \
    "with no answer kept for Settings › Agents to list," \
    "a site off limits refused at Full access too," \
    "its open and each call on a tab that came there$needs_said," \
    "and off takes the socket away;" \
    "an action waits for its element and its page to settle ($settle)," \
    "an app in a frame answers with its content or as busy ($framed)," \
    "a tab's id from before a relaunch is refused as such," \
    "done ends its task once its handoff is answered and not before," \
    "its next task ends 3 s after its last page closed," \
    "a passkey its page asks for $passkey_said," \
    "and a launch after the window quit in its Space shows the page it opens;" \
    "a screenshot of a tab just opened out of sight answers ($fresh_picture)," \
    "and with the UI holding the keyboard with its picture")"
else
  echo "ok: $(clauses \
    "an MCP client's bridge gets in with its token only," \
    "on the user's own socket," \
    "is offered no Boost tool," \
    "gets a Space of its own and is refused the user's tab," \
    "is answered that a link to an app opens nothing," \
    "its tab staying on its page," \
    "and moves ten cases to Closed in tabs the user isn't looking at a step a turn," \
    "its calls on the ten answered at once ($cases)," \
    "and its actions wait for their element and their page to settle ($settle)," \
    "an app in a frame answering with its content or as busy ($framed)," \
    "and the tabs its page opens are kept in its Space," \
    "behind$opened_said. Its upload is refused unasked for a folder's field," \
    "a button," \
    "a sandboxed frame's field and a document whose root frames none. A tab's id from before a relaunch is refused as such," \
    "done ends its task once its handoff is answered and not before," \
    "its next task ends 3 s after its last page closed," \
    "and a passkey its page asks for $passkey_said. skip: reading a page in its Space. Browser wasn't the active app," \
    "so the page ignored the click.")"
fi
echo "$own_size_said"
if [ -n "${toast_skip:-}" ]; then
  echo "skip: the needs' toast showing, as Windows shows toasts only for an installed copy, which the run's isn't: it counted on the taskbar button and asked for the toast."
fi
if [ -n "$msix" ]; then
  echo "ok: a Store copy's bridge, run by the alias its MSIX declares in the user's WindowsApps folder, carries a client's hello to the browser and its answer back."
elif windows; then
  echo "skip: a Store copy's bridge, by its alias. No MSIX of this build is installed here; CHECK_MSIX names one."
fi
