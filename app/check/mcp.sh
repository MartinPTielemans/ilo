# check.sh runs this at 380.
# An MCP client reaches the browser through the bridge beside it once Settings › Agents is on and
# it's connected, as the saved settings say here. The socket is the user's alone, and the bridge
# gets nowhere without the agent's token. Its first call makes its Space, which no window lists, and
# its first call that acts on a tab starts its card, one for all its connections. It may list and read
# only the tabs there: its list leaves out the user's tab, a read of the user's tab is refused, with
# an activity-log entry, and a read of example.com, opened in its Space, gives the page's text. A
# link to an app (ms-word:) opens nothing and asks nothing, the tab staying on its page, and the
# click's answer names the scheme. Ten
# cases, each the same form in a tab of its own the user isn't looking at, move to Closed a step a
# turn, a turn's ten calls answered at once, which logs the turns and how long they took. A page
# that hasn't come once an action has waited for it is still loading, as the answer says. A tab
# its page opens, by a link or window.open, goes in its Space, behind, and the click's answer names
# it. A page with fields gives its prose and none of their values: typed, filled in or a password, in its
# text and in its outline, and the agent can't type into the password field. It types into another.
# A frame nested two sites deep, one the page adds after its load and one it fills by posting a
# form into it are in the page's outline and text too, an empty helper frame is in neither, and a
# click two frames deep lands. A skip link in a frame, off the page until it's focused, is focused
# and then clicked, as Tab would bring it, and a click on one off the page that can't take focus
# is refused rather than answered as done.
# It puts tabs of its own in a split view, adds one, lists it and separates one, each in the
# activity log, and is refused a tab of the user's.
# A page's frames, of its own site and of another, are in its outline, where the agent clicks and
# types, each answered with what changed in the outline alone, and not into a frame's password
# field. Its text leaves out a frame an extension puts in the page, which isn't the page's. Nor is
# the rest of what an extension puts there (#2355): its button is in neither the outline nor the
# text, a click or the wheel where its frame covers the page, or a frame of the page, is refused,
# and so are keys while its frame has the keyboard, each with an activity-log entry, and its
# listeners don't hear the agent's keys or typing. Neither the agent's click nor its ArrowDown and
# Enter in a login field opens or accepts Chrome's suggestion of a saved login. Its keys, typed or
# pressed, go to the page alone, and Escape the page doesn't keep runs no Stop of ilo's. Where it
# acts in the page watched, its pointer and a
# ring with the element's ref show it. It opens a page, clicks its link, goes back, where the tab
# takes the page's title again, scrolls, takes a screenshot and closes a tab, pins a tab of its own, unpins it and removes it, and a page that asks before it's left stays open, with no
# dialog, until the agent discards it. Its download asks first, and Allow Once downloads the
# file, which the answer names, and Never leaves none. It sends requests to its tab's site, carrying a session it
# never sees, and a change asks every time, in a card whose Allow sends it and Deny doesn't. Settings
# › Agents opens each request to where it went and what it sent, or that the denied one wasn't. A bank's site asks first, in a card whose Allow Once opens it. Always Allow
# opens it from then on, as Settings › Agents lists with its Revoke. A login the
# user saved fills in neither as the agent's tab loads it nor from a dropdown, and fills once the
# user takes the tab over, without a reload. Closing the client's end ends its session with the tab.
# Reconnected, its next call is a card of its own, whose Stop answers the call under way and closes
# the client's connection. Turning Settings › Agents off takes the socket away and drops a client
# connected. Relaunched, a call with a tab's id from before is refused as one from before ilo
# restarted. Its tools offer it nothing of Boosts, which are the user's alone.
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
BROWSER_DRIVE=1 BROWSER_MOTION=full BROWSER_URL="$yours" \
  "$browser" --load-extension="$(app_path "$extension")" < "$out/drive" > "$log" 2>&1 &
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
# Prints the bridge's next line within 30 s. sh has no read -t on Linux, where it's dash, and the
# Mac has no timeout, so perl's alarm, which outlives its exec, bounds a shell that reads the line:
# a shell reads a fifo a byte at a time, so it takes that line alone.
next_answer() {
  perl -e 'alarm 30; exec @ARGV' sh -c 'read -r line && printf "%s\n" "$line"' <&5
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
  "Your Space has no tabs yet."*) ;;
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

# A tab an agent's tab opens follows the agent's rules from when it's made, as the agent's own tab
# does, whether a link, a page's window.open or a redirect in it leads there (#2660).
if engine_has ../core/space.cc ../core/window.cc ../core/tab.cc ../core/main_delegate.cc; then
  # The tab open_tab made refused the link to an app above where the core decides, as it's the
  # agent's from its first request: open_tab makes it so before its page starts.
  grep -q "browser: agent tab $documents refused ms-word\$" "$log" ||
    fail "the agent's own tab didn't refuse the link to an app; its last agent tab line:
$(grep 'browser: agent tab ' "$log" | tail -1)"
  printf '%s' '<!doctype html><title>Opens</title>
<a href="maps:?q=check" target="_blank">Link</a>
<a href="bounce?to=maps%3A%3Fq%3Dcheck" target="_blank">Redirect</a>
<button onclick="const a = navigator.userActivation.isActive; const w = window.open(&quot;maps:?q=check&quot;); document.title = &quot;probe-2667 activation &quot; + a + &quot; window &quot; + !!w">Pop up</button>' > "$out/site/opens.html"
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
# Closed in tabs the user isn't looking at, each step on all ten in one turn: cases.py, through a
# bridge of its own. The server answers a turn's calls at once (#1476). The form says Ready once
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
BROWSER_AGENT_TOKEN=$token python3 -I check/cases.py "$(app_path "$bridge")" "$socket" \
  "localhost:$port/case.html" > "$out/cases.log" 2>&1 || fail "$(tail -1 "$out/cases.log")"
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
    "Clicked "*". It opened tab $new_tab in your Space."*) ;;
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

# The client's work is one card at the sidebar's foot, from its first call that acted on a tab, and
# the cases' calls on their connection of their own were steps of the same card (#1080). Its name
# is the one the client gave.
sessions=$(grep -c '^browser: session session-[0-9]* of agent mcp-client as check$' "$log" || true)
[ "$sessions" = 1 ] || fail "the client's two connections had $sessions cards, not one"
session=$(sed -n 's/^browser: session \(session-[0-9]*\) of agent mcp-client as check$/\1/p' "$log")
await_log "^browser: task plate task-${session#session-} at " "the client's card wasn't drawn"
opened=$(call open_tab '{"url":"example.com"}')
agent_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
[ -n "$agent_tab" ] || fail "open_tab example.com answered: $opened"
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
  # Counted, as a page watched before has its lines already.
  watched=$(grep -c "^browser: task $session page $1\$" "$log" || true)
  placed=$(grep -c "^browser: task page $1 in place\$" "$log" || true)
  send click $((wx + ww / 2)) $((wy + wh / 2))
  await_more "^browser: task $session page $1\$" "$watched" ||
    fail "a click on tab $1's picture didn't watch it"
  await_more "^browser: task page $1 in place\$" "$placed" ||
    fail "tab $1's page didn't come to rest in place"
}
# A click in an agent's page takes it over (#1080), where the driver's click reaches the host as a
# hand's does: the Mac, Windows and Wayland, and not X11's sent click. After clicks that are there
# for the page's focus, not to take it, ⌘↩ hands back what one took. $1 is how many presses the log
# had before them. The press reaches the window a turn after the click, which the state waits for.
hand_back_pressed() {
  send state
  [ "$(grep -c '^browser: agent page pressed in ' "$log" || true)" = "$1" ] && return
  pressed=$(sed -n 's/^browser: agent page pressed in \([0-9]*\)$/\1/p' "$log" | tail -1)
  send key return cmd
  await_log "^browser: agent mcp-client handed back $pressed\$" "⌘↩ didn't hand back the page a click took"
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
  if windows; then
    # Quarantined: there a click of the agent's that opens a tab takes the keyboard (#2225).
    echo "skip: mcp: the user's keyboard through the agent's work on Windows (#2225)"
  else
    [ "$key $focus" = "$yours_key $yours_focus" ] ||
      fail "the agent's work took the keyboard from $yours_key, on $yours_focus, to $key, on $focus"
    sed -n "/^browser: drive $yours_step: /,\$p" "$log" | grep -q "^browser: title $user_tab yours left\$" &&
      fail "the user's field lost the keyboard while the agent worked"
    send type ok
    expect "^browser: title $user_tab yours typed ok\$" "the user's keys after the agent's work didn't reach their field"
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
  # Its one page so far opens on Talk, and the header's Pages shows its grid, each page a picture
  # to watch it from (#1080).
  await_log "^browser: task $session opens on talk\$" "the client's card with one page didn't open on Talk"
  await_log '^browser: task pages at ' "the conversation's header drew no Pages"
  read -r gx gy gw gh <<CHIP
$(grep '^browser: task pages at ' "$log" | tail -1 | cut -d' ' -f5-)
CHIP
  send click $((gx + gw / 2)) $((gy + gh / 2))
  expect "^browser: task $session shows pages\$" "a click on Pages didn't show the grid"
  picture "$agent_tab" > /dev/null
  ../tools/window-screenshot.sh "$pid" "$out/agent-pages.png" || fail "no window to capture"
  watch "$agent_tab"
  ../tools/window-screenshot.sh "$pid" "$out/agent-watched.png" || fail "no window to capture"
fi

# Split views (#1597): the agent puts two tabs of its own side by side, adds a third, lists them
# and separates one, each through the core's split calls and checked by its rules, with its entry
# in the activity log. A tab of the user's is refused, and the split stays as it was.
for page in a b c; do
  printf '%s' "<title>Split $page</title><p>Split page $page</p>" > "$out/site/split-$page.html"
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
# The split views' pages close, so the conversation's grid has the pages the steps after it count on.
for split_tab in $split_a $split_b $split_c; do
  closed=$(call close_tab "{\"tab\":$split_tab}")
  case "$closed" in "Closed tab $split_tab."*) ;; *) fail "closing the split view's page answered: $closed" ;; esac
done

if [ "$driven" = 1 ]; then
  # The core lists the agent's Space alone: its tab, and not the user's Wikipedia tab in theirs.
  listed=$(call list_tabs '{}')
  case "$listed" in
    *"$agent_tab  Example Domain  https://example.com/"*) ;;
    *) fail "list_tabs didn't list the agent's tab: $listed" ;;
  esac
  printf '%s\n' "$listed" | grep -q "^. $user_tab  " &&
    fail "list_tabs listed the user's tab in another Space: $listed"
  read=$(call read_page "{\"tab\":$agent_tab}")
  case "$read" in
    "Tab $agent_tab: Example Domain"*"for use in documentation examples"*) ;;
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
<label>Name <input id=typed style="position:fixed;inset:0;width:100%;height:100%"></label>
<label>Password <input id=password type=password></label>
<label>Email <input id=email autocomplete=email></label>
<label>Card <input id=card autocomplete=cc-number value=4111111111111111></label>
<label>Note <textarea id=note>note-by-ada</textarea></label>
<div contenteditable>editable-by-ada</div>
<select><option>Pick one<option selected>chosen-by-ada</select>
<script>
password.value = "hunter2-by-ada";
email.value = "ada@example.com";
typed.oninput = () => document.title = "typed";
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
  secrets="typed-by-ada hunter2-by-ada ada@example.com 4111111111111111 note-by-ada editable-by-ada
    chosen-by-ada"
  for secret in $secrets; do
    case "$read" in *"$secret"*) fail "reading the fields page gave a field's value, $secret" ;; esac
  done

  # The outline names the fields by ref, the password field off limits, and gives no value. A
  # select's line names its options, the page's own, and never which one is chosen (#2355).
  outline=$(call snapshot "{\"tab\":$agent_tab}")
  printf '%s\n' "$outline" > "$out/mcp-outline.txt"
  case "$outline" in
    "Tab $agent_tab: typed"*"Fields for the agents check."*"(password: off limits)"*) ;;
    *) fail "the fields page's outline was: $outline" ;;
  esac
  for secret in $secrets; do
    [ "$secret" = chosen-by-ada ] && continue
    case "$outline" in *"$secret"*) fail "the fields page's outline gave a field's value, $secret" ;; esac
  done
  chosen=$(printf '%s\n' "$outline" | grep 'chosen-by-ada')
  case "$chosen" in
    *'- combobox [e'*'(options: "Pick one", "chosen-by-ada")') ;;
    *) fail "the select's line didn't name just its options: $chosen" ;;
  esac
  ref_of() { printf '%s\n' "$outline" | sed -n "s/.*- $1 \"$2\" \[\(e[0-9]*\)\].*/\1/p" | head -1; }
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
  case "$typed" in *by-the-agent*) fail "the outline gave what the agent typed" ;; esac
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
    *"- combobox \"Status\" [$status] (options: \"Open\", \"Pending\", \"Closed\")"*) ;;
    *) fail "the select's line didn't name its options: $(printf '%s\n' "$outline" | grep Status)" ;;
  esac
  [ -n "$status" ] && [ -n "$priority" ] ||
    fail "the choosing page's fields had the refs '$status' and '$priority'"
  # A picker whose list is its child (aria-owns), as Chromium's tree has it, shows its options with
  # refs, and still not what's in its field (#2355).
  case "$outline" in *Ber-by-ada*) fail "the picker's line gave what its field holds" ;; esac
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
  row=$(printf '%s\n' "$outline" | sed -n 's/.*- row "web-3[^"]*" \[\(e[0-9]*\)\].*/\1/p' | head -1)
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
  pressed=$(perl -e 'alarm 60; exec @ARGV' sh -c 'read -r line && printf "%s\n" "$line"' <&5) ||
    fail "the browser didn't answer press_key Escape"
  pressed=$(printf '%s\n' "$pressed" | answered "press_key Escape")
  case "$pressed" in "Pressed Escape."*) ;; *) fail "press_key Escape answered: $pressed" ;; esac
  kill -0 "$pid" 2>/dev/null || fail "the app exited as the agent pressed Escape"
  [ "$(grep -c "$stopped" "$log" || true)" = "$stops" ] ||
    fail "the agent's Escape, which the page didn't keep, ran ilo's Stop"
  [ "$(tabs)" = "$before" ] || fail "the agent's Escape changed the tabs in its Space"
  closed=$(call close_tab "{\"tab\":$keys_tab}")
  case "$closed" in "Closed tab $keys_tab."*) ;; *) fail "closing the keys page answered: $closed" ;; esac

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
  # body, and a change asks first, every time, in a card whose Allow sends it and Deny doesn't.
  sent=$(call request "{\"tab\":$frames_tab,\"requests\":[{\"method\":\"GET\",\"path\":\"/api/me\"},{\"method\":\"GET\",\"path\":\"/api/me\"}]}")
  case "$sent" in
    "1. GET /api/me → 200 application/json"*"Ada"*"2. GET /api/me → 200"*) ;;
    *) fail "request answered: $sent" ;;
  esac
  case "$sent" in *s3cret* | *tok-*) fail "the request's answer held the session: $sent" ;; esac
  grep -q "^GET chromium localhost:$port/api/me cookie .*api=s3cret$port" "$out/site.log" ||
    fail "the agent's request didn't carry the tab's session"
  await_log "^browser: agent activity mcp-client $frames_tab request\$" \
    "the request didn't reach the activity log"
  changes='^browser: agent mcp-client asks [0-9]*: make a change on localhost$'
  posted="^POST localhost:$port/api/items cookie .*api=s3cret$port[; ]"
  # Asks for a change and answers $1 with a click on the card's last row, Deny at its left and
  # Allow at its right.
  change() {
    before=$(grep -c "$changes" "$log" || true)
    request request "{\"tab\":$frames_tab,\"requests\":[{\"method\":\"POST\",\"path\":\"/api/items\",\"body\":{\"title\":\"Milk\"}}]}" >&4
    await_more "$changes" "$before" || fail "a change didn't ask first"
    q=$(grep '^browser: agent mcp-client asks' "$log" | tail -1 | cut -d' ' -f5 | tr -d :)
    await_log "^browser: agent question $q at " "the change's question wasn't drawn"
    read -r qx qy qw qh <<CARD
$(grep "^browser: agent question $q at " "$log" | tail -1 | cut -d' ' -f6-)
CARD
    ../tools/window-screenshot.sh "$pid" "$out/agent-request.png" || fail "no window to capture"
    [ "$(grep -c "$posted" "$out/site.log")" = "$2" ] || fail "the change went before the user answered"
    case "$1" in Once) x=$((qw * 3 / 4)) ;; Dismiss) x=$((qw / 4)) ;; esac
    send click $((qx + x)) $((qy + qh - 26))
    await_log "^browser: agent mcp-client answered $q $1\$" "$1 didn't answer the change's question"
    changed=$(next_answer) || fail "the browser didn't answer the change"
    changed=$(printf '%s\n' "$changed" | answered "the change")
  }
  change Once 0
  case "$changed" in
    'POST /api/items → 201 application/json'*'{"id":8,"title":"Milk"}') ;;
    *) fail "the allowed change answered: $changed" ;;
  esac
  # Its body on its own, as the session may hold other cookies of localhost's, such as profiles'.
  grep "$posted" "$out/site.log" | grep -q ' body {"title":"Milk"}$' ||
    fail "the allowed change didn't reach the site with the tab's session"
  change Dismiss 1
  [ "$changed" = "error: The user said no: you can't make a change on localhost. Don't try again." ] ||
    fail "the denied change answered: $changed"
  [ "$(grep -c "$posted" "$out/site.log")" = 1 ] || fail "the denied change reached the site"
  # Settings › Agents lists each request by what it did, and a press opens it to where it went and
  # what it sent, or, for the change the user said no to, that it wasn't sent (#2326). Each row's
  # name holds its entry's id, which counts up, and its method.
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  settings_drew
  if ! grep '^browser: settings shown ' "$log" | tail -1 | grep -q ' Agents$'; then
    send clickkey $(drawn "toolbar Agents")
    expect '^browser: settings tab Agents$' "a click on Agents didn't show its section"
  fi
  settings_drew Agents
  drawn 'request [0-9]* POST' > /dev/null || fail "Settings › Agents drew no change"
  posts=$(sed -n 's/^browser: settings request \([0-9]*\) POST at .*/\1/p' "$log" | sort -nu)
  allowed_id=$(printf '%s\n' "$posts" | head -1)
  denied_id=$(printf '%s\n' "$posts" | tail -1)
  read_id=$(sed -n 's/^browser: settings request \([0-9]*\) GET at .*/\1/p' "$log" | sort -nu | tail -1)
  [ -n "$read_id" ] && [ "$allowed_id" != "$denied_id" ] ||
    fail "Settings › Agents didn't list the read and both changes: $(grep '^browser: settings request ' "$log")"
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
  open_request "$allowed_id" POST "to http://localhost:$port/api/items, sent: title: Milk"
  open_request "$denied_id" POST "to http://localhost:$port/api/items, not sent: title: Milk"
  ../tools/window-screenshot.sh "$pid" "$out/settings-agents-requests.png" || fail "no window to capture"
  # Back to General, for the visits below to click Agents.
  send clickkey $(drawn "toolbar General")
  expect '^browser: settings tab General$' "a click on General didn't show its section"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"

  # The agent's picture of a page withholds what its outline does of a field: a field holding what
  # the agent typed and is unchanged shows it, as does the page's own value, by the markup or in a
  # read-only field; any other field with something in it is painted over, so a field filled by the
  # page's script, as the user's typing or a password manager's fill would be, one the agent typed
  # into that changed since, a password field the page made a text field and a card's field, in a
  # frame of another site and half out of view too. Each field's text is red blocks: a field shown
  # has red in the picture, and one withheld has none. The check's extension puts two frames of its
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
  printf '%s' "<!doctype html><meta charset=utf-8><title>loading</title><style>
body { margin: 0; background: #fff }
input, textarea { position: absolute; box-sizing: border-box; width: 20vw; height: 12vh; margin: 0;
  padding: 0; border: 2px solid #888; background: #fff; color: #f00; font: 9vh/1 monospace;
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
<iframe id=site src=http://127.0.0.1:$port/withheld-inner.html></iframe>
<button onclick='document.querySelector(\"[aria-label=Changed]\").value += \"█\"'>Change</button>
<input aria-label=Below style='left: 2vw; top: 94vh'>
<div style='height: 76vh'></div>
<script src=painted.js></script>
<script>
const field = label => document.querySelector('[aria-label=\"' + label + '\"]');
for (const label of ['Filled', 'Fixed', 'Below']) field(label).value = '██████';
field('Shown password').type = 'text';
let ready = 0;
const shown = () => ++ready == 4 && (document.title = 'fields');
painted(shown);
addEventListener('message', event =>
  (event.data == 'painted' || event.data == 'extension painted') && shown());
</script>" > "$out/site/withheld.html"
  outline=$(call open_tab "{\"url\":\"localhost:$port/withheld.html\"}")
  fields_tab=$(printf '%s\n' "$outline" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  await_log "^browser: title $fields_tab fields\$" "the page with fields didn't paint"
  for label in Typed Changed; do
    ref=$(ref_of textbox "$label")
    typed=$(call type "{\"tab\":$fields_tab,\"ref\":\"$ref\",\"text\":\"██████\"}")
    case "$typed" in "error: "*) fail "typing into $label answered: $typed" ;; esac
  done
  clicked=$(call click "{\"tab\":$fields_tab,\"ref\":\"$(ref_of button Change)\"}")
  case "$clicked" in
    "Clicked button “Change”."*) ;;
    *) fail "clicking Change answered: $clicked" ;;
  esac
  mcp "$(request screenshot "{\"tab\":$fields_tab}")" | python3 -I -c '
import base64, json, sys
content = json.load(sys.stdin)["result"]["content"][0]
if content["type"] != "image":
    sys.exit(content.get("text", "no picture"))
open(sys.argv[1], "wb").write(base64.b64decode(content["data"]))' "$out/agent-fields.jpg" ||
    fail "the screenshot of the page with fields answered no picture"
  # The share of red pixels between ($1, $2) and ($3, $4), as hundredths of the picture's width
  # and height, in %; pixel leaves the picture as a BMP.
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
        b, g, r = data[offset + row * stride + x * bits // 8:][:3]
        red += r > 160 and g < 100 and b < 100
        total += 1
print(red * 100 // max(total, 1))
PY
  }
  # A field's inside, $1 between ($2, $3) and ($4, $5), clear of its border: $6, shown with red,
  # or withheld with none.
  see_field() {
    share=$(reds "$2" "$3" "$4" "$5")
    echo "$1: $share% red" >> "$out/mcp-pictured.txt"
    if [ "$6" = shown ]; then
      [ "$share" -ge 10 ] ||
        fail "the agent's picture didn't show $1, which it may see: $share% red"
    else
      [ "$share" -eq 0 ] || fail "the agent's picture showed $1, which it may not: $share% red"
    fi
  }
  : > "$out/mcp-pictured.txt"
  see_field Typed 3 4 21 12 shown
  see_field Changed 27 4 45 12 withheld
  see_field Filled 51 4 69 12 withheld
  see_field Default 75 4 93 12 shown
  see_field Fixed 3 22 21 30 shown
  see_field "Shown password" 27 22 45 30 withheld
  see_field Card 51 22 69 30 withheld
  see_field Note 75 22 93 30 shown
  see_field "Frame default" 6 44 28 54 shown
  see_field "Frame filled" 36 44 58 54 withheld
  see_field Below 3 95 21 99 withheld
  see_field "Extension frame over the page" 41 77 64 90 withheld
  see_field "Extension frame in the page" 3 77 26 90 withheld
  call close_tab "{\"tab\":$fields_tab}" > /dev/null

  # It opens a page in its Space, clicks its link, scrolls a long page and takes a screenshot.
  opened=$(call open_tab '{"url":"example.com"}')
  case "$opened" in
    "Opened tab "*"URL: https://example.com/"*) ;;
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
    "Clicked link"*"iana.org"*) ;;
    *) fail "clicking example.com's link answered: $clicked" ;;
  esac
  # Back to a page it saw before, whose entry kept its title, the tab's row takes that title again
  # (#1469).
  titles=$(grep -c "^browser: title $opened_tab Example Domain\$" "$log" || true)
  went=$(call navigate "{\"tab\":$opened_tab,\"url\":\"back\"}")
  case "$went" in "Went back."*"Example Domain"*) ;; *) fail "navigate back answered: $went" ;; esac
  await_more "^browser: title $opened_tab Example Domain\$" "$titles" ||
    fail "back on example.com, the tab kept the title of the page it left: $(grep "^browser: title $opened_tab " "$log" | tail -1)"
  # example.com is shorter than the view, where a scroll says nothing moved (#1445), so the tab
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
  opened=$(call open_tab '{"url":"example.com"}')
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
  # A page that asks before it's left stays open, and the answer says why, with no dialog: with
  # discard, the agent's own tab closes without saving (#1447).
  printf '%s\n' '<!doctype html><title>Leave</title><p>Unsaved changes.</p>
<script>addEventListener("beforeunload", event => event.preventDefault())</script>' > "$out/site/leave.html"
  opened=$(call open_tab "{\"url\":\"localhost:$port/leave.html\"}")
  leaving=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  kept=$(call close_tab "{\"tab\":$leaving}")
  case "$kept" in
    "error: Tab $leaving stays open: its page asks before it's left"*"discard: true"*) ;;
    *) fail "closing a page that asks before it's left answered: $kept" ;;
  esac
  call list_tabs '{}' | grep -q "^. $leaving  " || fail "a close that kept the tab took it out"
  closed=$(call close_tab "{\"tab\":$leaving,\"discard\":true}")
  case "$closed" in
    "Closed tab $leaving without saving."*) ;;
    *) fail "discarding the agent's own tab answered: $closed" ;;
  esac
  call list_tabs '{}' | grep -q "^. $leaving  " && fail "list_tabs listed the discarded tab"
  grep -q "^browser: page $leaving dialog [^n]" "$log" && fail "closing the leaving page showed a dialog"

  # It pins a tab of its own, which list_tabs says it pinned, unpins it, pins it again and removes
  # it, to the archive. The user's tab isn't its to pin.
  opened=$(call open_tab '{"url":"example.com"}')
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

  # hand_off waits, in a card in the window, until the user hands the tab back.
  cards=$(grep -c '^browser: agent question [0-9]* at ' "$log" || true)
  request hand_off "{\"tab\":$opened_tab,\"reason\":\"Sign in for the check\"}" >&4
  await_log "^browser: agent mcp-client hands off $opened_tab: Sign in for the check\$" \
    "hand_off didn't hand the tab to the user"
  await_more '^browser: agent question [0-9]* at ' "$cards" || fail "the handoff wasn't drawn"
  q=$(grep '^browser: agent question [0-9]* at ' "$log" | tail -1 | cut -d' ' -f4)
  read -r qx qy qw qh <<CARD
$(grep "^browser: agent question $q at " "$log" | tail -1 | cut -d' ' -f6-)
CARD
  ../tools/window-screenshot.sh "$pid" "$out/agent-handoff.png" || fail "no window to capture"
  # Back in the conversation's Pages, the page handed to the user says so under its picture, with
  # why (#1080). Then its page is watched again, as before.
  back="^browser: task page $agent_tab back\$"
  thumbnail='^browser: task page in its thumbnail$'
  before=$(grep -c "$back" "$log" || true)
  shrunk=$(grep -c "$thumbnail" "$log" || true)
  send key escape
  await_more "$back" "$before" || fail "Escape didn't step back to Pages"
  await_more "$thumbnail" "$shrunk" || fail "the page watched didn't go back into its picture"
  await_log "^browser: task picture $opened_tab says Needs you · Sign in for the check\$" \
    "the handed page's picture didn't say it needs the user: $(grep "^browser: task picture $opened_tab says" "$log" | tail -1)"
  ../tools/window-screenshot.sh "$pid" "$out/agent-pages-needs-you.png" || fail "no window to capture"
  watch "$agent_tab"
  # Hand Back, at the left of the card's buttons, Show Tab the primary at their right.
  send click $((qx + 30)) $((qy + qh - 26))
  await_log "^browser: agent mcp-client handed back $opened_tab\$" "Hand Back didn't hand the tab back"
  handed=$(next_answer) || fail "the browser didn't answer hand_off"
  handed=$(printf '%s\n' "$handed" | answered hand_off)
  case "$handed" in
    "The user handed tab $opened_tab back."*) ;;
    *) fail "hand_off answered: $handed" ;;
  esac

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
  # In the page, above the strip, where nothing on example.com takes the click. On X11 the host
  # hears the display's own presses, which the driver's press makes; elsewhere its click is one.
  if [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
    send handclick $((sx + sw / 2)) $((sy - 120))
  else
    send click $((sx + sw / 2)) $((sy - 120))
  fi
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
  # The agent asks for the tab in a card, and Keep, at the left, keeps it the user's.
  cards=$(grep -c '^browser: agent question [0-9]* at ' "$log" || true)
  request ask_for_tab "{\"tab\":$opened_tab,\"reason\":\"To finish the check\"}" >&4
  await_log "^browser: agent mcp-client asks for $opened_tab back: To finish the check\$" \
    "ask_for_tab didn't ask"
  await_more '^browser: agent question [0-9]* at ' "$cards" || fail "the agent's ask wasn't drawn"
  q=$(grep '^browser: agent question [0-9]* at ' "$log" | tail -1 | cut -d' ' -f4)
  read -r qx qy qw qh <<CARD
$(grep "^browser: agent question $q at " "$log" | tail -1 | cut -d' ' -f6-)
CARD
  ../tools/window-screenshot.sh "$pid" "$out/agent-ask-for-tab.png" || fail "no window to capture"
  send click $((qx + 30)) $((qy + qh - 26))
  await_log "^browser: agent mcp-client answered $q Never\$" "Keep didn't answer"
  kept=$(next_answer) || fail "the browser didn't answer ask_for_tab"
  kept=$(printf '%s\n' "$kept" | answered ask_for_tab)
  case "$kept" in
    "error: The user keeps tab $opened_tab."*) ;;
    *) fail "ask_for_tab answered: $kept" ;;
  esac
  # ⌘↩ again hands it back.
  send key return cmd
  await_log "^browser: agent mcp-client handed back $opened_tab\$" "⌘↩ didn't hand the tab back"
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
  # How many child windows the browser window has, Chrome's autofill dropdown among them.
  children() {
    send layout
    sed -n "/^browser: drive $step: /,\$p" "$log" | grep -c '^browser: layout child' || true
  }
  logins() {
    grep -c "^browser: title $opened_tab login\$" "$log" || true
  }
  call navigate "{\"tab\":$opened_tab,\"url\":\"localhost:$port/welcome.html\"}" > /dev/null
  await_log "^browser: title $opened_tab welcome\$" "the welcome page didn't load in the agent's tab"
  take_over "to save a login"
  go_from_bar "localhost:$port/login.html"
  expect "^browser: title $opened_tab login\$" "the login page didn't load in the tab the user holds"
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
  shown=$(children)
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
  [ "$(children)" = "$shown" ] || fail "the agent's tab offered the saved login in a dropdown"
  # Nor do the agent's keys, which would open the dropdown and accept its login (#2355).
  call press_key "{\"tab\":$opened_tab,\"key\":\"ArrowDown\"}" > /dev/null
  [ "$(children)" = "$shown" ] || fail "the agent's ArrowDown offered the saved login in a dropdown"
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
  for _ in $(seq 30); do [ "$(children)" -gt "$shown" ] && break; done
  [ "$(children)" -gt "$shown" ] || fail "the held tab's username field offered no saved login"
  ../tools/window-screenshot.sh "$pid" "$out/agent-login-filled.png" || fail "no window to capture"
  send key escape
  before=$(grep -c "$handed" "$log" || true)
  send key return cmd
  await_more "$handed" "$before" || fail "⌘↩ didn't hand the filled tab back"

  # open_tab at a file downloads it once the user says yes on the card (#1446), and answers with
  # its name and size, its empty tab closed. Never cancels the next, which leaves no file.
  rm -f "$out/downloads/report"*
  printf 'report\n' > "$out/site/report.docx"
  file="{\"url\":\"files.localhost:$port/report.docx\"}"
  wants='^browser: agent mcp-client asks [0-9]*: download report\.docx from files\.localhost$'
  # Opens the file, which asks, and answers $1 with a click, as answer_bank below does. Sets
  # downloaded to the answer.
  answer_download() {
    before=$(grep -c "$wants" "$log" || true)
    request open_tab "$file" >&4
    await_more "$wants" "$before" || fail "an agent's download didn't ask first"
    q=$(grep '^browser: agent mcp-client asks' "$log" | tail -1 | cut -d' ' -f5 | tr -d :)
    await_log "^browser: agent question $q at " "the download's question wasn't drawn"
    read -r qx qy qw qh <<CARD
$(grep "^browser: agent question $q at " "$log" | tail -1 | cut -d' ' -f6-)
CARD
    # Allow Once and Never share the row above Always Allow, Never at the left.
    case "$1" in Once) x=$((qw * 3 / 4)) ;; Never) x=$((qw / 4)) ;; esac
    send click $((qx + x)) $((qy + qh - 26 - 36))
    await_log "^browser: agent mcp-client answered $q $1\$" "$1 didn't answer the download"
    downloaded=$(next_answer) || fail "the browser didn't answer the download's open_tab"
    downloaded=$(printf '%s\n' "$downloaded" | answered "the download's open_tab")
  }
  answer_download Once
  [ "$downloaded" = "Downloaded report.docx (7 bytes) to downloads. Agents can't open files; ask the user if you need it." ] ||
    fail "the allowed download answered: $downloaded"
  [ -f "$out/downloads/report.docx" ] || fail "the allowed download isn't in the check's folder"
  file_tab=$(grep '^browser: agent download [0-9]* [0-9]* [0-9]* state ' "$log" | tail -1 |
    cut -d' ' -f5)
  listed=$(call list_tabs '{}')
  printf '%s\n' "$listed" | grep -q "^. $file_tab  " && fail "the download's empty tab stayed open"
  answer_download Never
  [ "$downloaded" = "error: The user said no: you can't download report.docx from files.localhost. Don't try again." ] ||
    fail "the refused download answered: $downloaded"
  # Chrome deletes what a cancelled download wrote on its own sequence, a moment after.
  for _ in $(seq 50); do
    [ "$(ls -A "$out/downloads" | grep -c '^report')" = 1 ] && break
    sleep 0.1
  done
  [ "$(ls -A "$out/downloads" | grep -c '^report')" = 1 ] ||
    fail "the refused download left a file: $(ls -A "$out/downloads" | tr '\n' ' ')"
  # As does one a tab the agent's tab opened starts (#2660).
  if [ -n "$opened_said" ]; then
    printf '%s' "<!doctype html><title>Files</title><a href=\"//files.localhost:$port/report.docx\" target=\"_blank\">Report</a>" \
      > "$out/site/files.html"
    opened=$(call open_tab "{\"url\":\"localhost:$port/files.html\"}")
    files=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
    ref=$(printf '%s\n' "$opened" | sed -n 's/.*- link "Report" \[\(e[0-9]*\)\].*/\1/p' | head -1)
    before=$(grep -c "$wants" "$log" || true)
    call click "{\"tab\":$files,\"ref\":\"$ref\"}" > /dev/null
    await_more "$wants" "$before" || fail "a download a tab the agent's tab opened started didn't ask first"
    q=$(grep '^browser: agent mcp-client asks' "$log" | tail -1 | cut -d' ' -f5 | tr -d :)
    await_log "^browser: agent question $q at " "the opened tab's download question wasn't drawn"
    read -r qx qy qw qh <<CARD
$(grep "^browser: agent question $q at " "$log" | tail -1 | cut -d' ' -f6-)
CARD
    send click $((qx + qw / 4)) $((qy + qh - 26 - 36))
    await_log "^browser: agent mcp-client answered $q Never\$" "Never didn't answer the opened tab's download"
    for _ in $(seq 50); do
      [ "$(ls -A "$out/downloads" | grep -c '^report')" = 1 ] && break
      sleep 0.1
    done
    [ "$(ls -A "$out/downloads" | grep -c '^report')" = 1 ] ||
      fail "the opened tab's refused download left a file: $(ls -A "$out/downloads" | tr '\n' ' ')"
    closed=$(call close_tab "{\"tab\":$files}")
    case "$closed" in "Closed tab $files."*) ;; *) fail "closing tab $files answered: $closed" ;; esac
  fi

  # A bank's site asks first, in a card in the window, and Allow Once opens it.
  bank="{\"url\":\"bank.localhost:$port/fields.html\"}"
  asks='^browser: agent mcp-client asks [0-9]*: open bank\.localhost$'
  # Opens the bank's site, which asks, and answers $1 with a click: Always Allow is the card's last
  # row, across its width, and Allow Once the right of the row above, beside Never. Sets allowed
  # to the answer.
  answer_bank() {
    before=$(grep -c "$asks" "$log" || true)
    request open_tab "$bank" >&4
    await_more "$asks" "$before" || fail "a bank's site didn't ask first"
    q=$(grep '^browser: agent mcp-client asks' "$log" | tail -1 | cut -d' ' -f5 | tr -d :)
    await_log "^browser: agent question $q at " "the agent's question wasn't drawn"
    read -r qx qy qw qh <<CARD
$(grep "^browser: agent question $q at " "$log" | tail -1 | cut -d' ' -f6-)
CARD
    ../tools/window-screenshot.sh "$pid" "$out/agent-question.png" || fail "no window to capture"
    # A button is 28 high and 8 from the next row.
    case "$1" in Once) x=$((qw * 3 / 4)) y=$((qh - 26 - 36)) ;; Always) x=$((qw / 2)) y=$((qh - 26)) ;; esac
    send click $((qx + x)) $((qy + y))
    await_log "^browser: agent mcp-client answered $q $1\$" "$1 didn't answer the question"
    allowed=$(next_answer) || fail "the browser didn't answer the bank's open_tab"
    allowed=$(printf '%s\n' "$allowed" | answered "the bank's open_tab")
  }
  answer_bank Once
  case "$allowed" in "Opened tab "*) ;; *) fail "the allowed open_tab answered: $allowed" ;; esac
  answer_bank Always
  case "$allowed" in "Opened tab "*) ;; *) fail "the always allowed open_tab answered: $allowed" ;; esac
  before=$(grep -c "$asks" "$log" || true)
  allowed=$(call open_tab "$bank")
  case "$allowed" in "Opened tab "*) ;; *) fail "after Always Allow, open_tab answered: $allowed" ;; esac
  [ "$(grep -c "$asks" "$log")" = "$before" ] || fail "Always Allow asked again"

  # Settings › Agents lists the permission with its Revoke, which the space menu of the agent's
  # Space did before #1080. Under Xvfb's 800-point screen the button is drawn below its bottom, so
  # the click isn't driven; the Agents' tests revoke.
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  settings_drew
  send clickkey $(drawn "toolbar Agents")
  expect '^browser: settings tab Agents$' "a click on Agents didn't show its section"
  settings_drew Agents
  drawn "revoke bank.localhost" > /dev/null || fail "Settings › Agents didn't list the bank's permission to revoke"
  ../tools/window-screenshot.sh "$pid" "$out/settings-agents-revoke.png" || fail "no window to capture"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
  # Another bank's site asks, as the answer was for bank.localhost alone.
  asks='^browser: agent mcp-client asks [0-9]*: open otherbank\.localhost$'
  request open_tab "{\"url\":\"otherbank.localhost:$port/fields.html\"}" >&4
  await_more "$asks" 0 || fail "another bank's site didn't ask"
  # Left unanswered, for the client to go with.
  pending=$(grep '^browser: agent mcp-client asks' "$log" | tail -1 | cut -d' ' -f5 | tr -d :)
  await_log "^browser: agent question $pending at " "the agent's question wasn't drawn"
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
  # Its card ends on its receipt, which names only what had consequences (#1080): the bank's site
  # it was allowed to open is no order or message, and its pages are Open's to count, so it says
  # when it ended.
  # Library › Agents keeps it, in its agent's profile (#1080).
  await_log '^browser: agents history keeps session-[0-9]*-[0-9]* in profile [1-9]' \
    "the client's ended session wasn't kept for Library › Agents: $(grep '^browser: agents history' "$log" | tail -1)"
  await_log '^browser: task card session-[0-9]* Done Done [0-9]' \
    "the client's card didn't say when it ended: $(grep '^browser: task card session-[0-9]* Done ' "$log" | tail -1)"
  grep '^browser: task card session-[0-9]* Done ' "$log" | grep -q '[0-9] pages\{0,1\}' &&
    fail "the client's receipt counted its pages, which Open counts: $(grep '^browser: task card session-[0-9]* Done ' "$log" | tail -1)"
  # The agent is at work in the tab it last acted in, the bank's it opened.
  at=$(grep '^browser: agent mcp-client at work in ' "$log" | tail -1 | awk '{print $NF}')
  await_log "^browser: agent mcp-client done in $at\$" "the tab still shows a gone agent at work"
  await_log "^browser: agent question $pending gone\$" "a gone agent's question stayed open"
  # The card on its receipt, its status rolled in by now.
  ../tools/window-screenshot.sh "$pid" "$out/agent-receipt.png" || fail "no window to capture"

  # Reconnected, the client's next call on a tab is a card of its own. Its conversation's Stop
  # answers the call under way that the user stopped it, then closes the client's connection,
  # which the bridge ends with, until the user reconnects it (#1080). The click comes while the
  # step under way, the hand_off waiting on the user, shimmers: the shimmer draws on the loading
  # dots' clock, and the driver reads its commands meanwhile, whatever the renderer (#1700).
  hello='^browser: agent mcp-client hello from check$'
  initialize='{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}'
  # children sends its layout from a subshell, whose steps the count here didn't keep.
  step=$(sed -n 's/^browser: drive \([0-9]*\)[: ].*/\1/p' "$log" | sort -n | tail -1)
  bridge_on
  hellos=$(grep -c "$hello" "$log" || true)
  mcp "$initialize" > /dev/null
  await_more "$hello" "$hellos" || fail "the app didn't hear the reconnected client's hello"
  sessions=$(grep -c '^browser: session session-[0-9]* of agent mcp-client as check$' "$log" || true)
  # Quiet until it needs you (#1080): with ilo in the background, the handoff posts one
  # notification. On Linux another app's window takes the keyboard, the run's server takes the
  # notification and clicks it, and on Windows another app has the foreground and the toast is
  # posted, which no run can click. The Mac's runner can't answer its leave for notifications, so
  # there ilo stays active, and the need only counts on the Dock's badge.
  background=
  if [ "$(uname)" = Linux ] && [ -n "$notes" ] && [ -z "${BROWSER_WAYLAND:-}" ]; then
    rm -f "$out/other-in"
    mkfifo "$out/other-in"
    python3 -I check/other-window.py < "$out/other-in" > "$out/other-window.log" 2>&1 &
    other=$!
    exec 7> "$out/other-in"
    for _ in $(seq 50); do grep -q '^[0-9]' "$out/other-window.log" && break; sleep 0.1; done
    other_window=$(head -1 "$out/other-window.log")
    xdotool windowactivate --sync "$other_window" 2> /dev/null ||
      fail "another app's window didn't open and take the keyboard: $(cat "$out/other-window.log")"
    # The driver reads the keyboard's window from the X server, as the app does.
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
  counted() { grep -c "$1" "$log" || true; }
  ones=$(counted '^browser: needs you 1$')
  urgents=$(counted '^browser: window [0-9]* urgent on$')
  badges=$(counted '^browser: \(dock\|taskbar\) badge 1$')
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
      await_more '^browser: window [0-9]* urgent on$' "$urgents" ||
        fail "the waiting card didn't mark the window urgent"
      await_log "^browser: needs you notified [0-9]* $needing\$" "the need in the background posted nothing"
      need=$(sed -n "s/^browser: needs you notified \([0-9]*\) $needing\$/\1/p" "$log" | tail -1)
      await_log "^browser: needs you posted $need\$" \
        "the notification didn't reach the desktop: $(grep '^browser: needs you ' "$log" | tail -1)"
      note=$(grep '^notify [0-9]* check needs you$' "$notes" | tail -1 | cut -d' ' -f2)
      [ -n "$note" ] || fail "the desktop's server got no \"check needs you\": $(cat "$notes")"
      grep -qx "body $note Sign in once more" "$notes" ||
        fail "the notification's body isn't the handoff's step: $(grep "^body $note " "$notes")"
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
      fi
      ;;
  esac
  case $background in
    linux) needs_said=", a handoff while another app's window has the keyboard marks the window urgent and posts one notification, \"check needs you\" over its step with no sound, whose click opens its card, and none while ilo is active" ;;
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
  if [ "$background" = linux ]; then
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
# The switch the run turned off goes back on.
printf '{"on":true,"agents":[{"client":"mcp-client","token":"%s"}]}' "$token" > "$agents_file"
log="$out/mcp-relaunch.log"
step=0
BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
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
exec 4>&- 5<&-
quit
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
trap cleanup EXIT
rm -f "$agents_file" "$out/mcp-in" "$out/mcp-out"
if [ "$driven" = 1 ]; then
  echo "ok: an MCP client's bridge gets in with its token only, on the user's own socket, is offered no Boost tool, gets a Space of its own, is refused the user's tab and reads its own, is answered that a link to an app opens nothing, its tab staying on its page, moves ten cases to Closed in tabs the user isn't looking at a step a turn, its calls on the ten answered at once ($cases), the tabs its page opens kept in its Space, behind$opened_said, without its fields' values, acts by ref but never in a password field, presses keys that reach the page alone, its Escape running no Stop of ilo's, closes its own tab, pins, unpins and removes its own tab but not the user's, sends requests to its tab's site with the tab's session it never sees, a change asking each time, each opening in Settings › Agents to where it went and what it sent or didn't, keeps one whose page asks before it's left until it discards it, with no dialog, runs steps in one call, waits on a handoff until Hand Back, works on one card for both its connections, whose conversation watches its page, opens its tabs muted, is refused a tab the user took over with a press in its page until they hand it back and Keep answers its ask, is stopped by its conversation's Stop while its waiting step shimmers, which closes its connection once its call is answered, gets no saved login filled in or offered in its tab until the user takes it over, when it fills without a reload, its download asks first and answers with the file, a refused one leaving none, a bank's site asks first until Always Allow, which Settings › Agents lists$needs_said, and off takes the socket away; an action waits for its element and its page to settle ($settle), a tab's id from before a relaunch is refused as such, and a launch after the window quit in its Space shows the page it opens; a screenshot of a tab just opened out of sight answers ($fresh_picture), and with the UI holding the keyboard with its picture"
else
  echo "ok: an MCP client's bridge gets in with its token only, on the user's own socket, is offered no Boost tool, gets a Space of its own and is refused the user's tab, is answered that a link to an app opens nothing, its tab staying on its page, and moves ten cases to Closed in tabs the user isn't looking at a step a turn, its calls on the ten answered at once ($cases), and its actions wait for their element and their page to settle ($settle), and the tabs its page opens are kept in its Space, behind$opened_said. A tab's id from before a relaunch is refused as such. skip: reading a page in its Space. Browser wasn't the active app, so the page ignored the click."
fi
if [ -n "${toast_skip:-}" ]; then
  echo "skip: the needs' toast showing, as Windows shows toasts only for an installed copy, which the run's isn't: it counted on the taskbar button and asked for the toast."
fi
if [ -n "$msix" ]; then
  echo "ok: a Store copy's bridge, run by the alias its MSIX declares in the user's WindowsApps folder, carries a client's hello to the browser and its answer back."
elif windows; then
  echo "skip: a Store copy's bridge, by its alias. No MSIX of this build is installed here; CHECK_MSIX names one."
fi
