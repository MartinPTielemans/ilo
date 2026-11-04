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
# that hasn't come once an action has waited for it is still loading, as the answer says. A
# page with fields gives its prose and none of their values: typed, filled in or a password, in its
# text and in its outline, and the agent can't type into the password field. It types into another.
# A frame nested two sites deep and one the page adds after its load are in the page's outline
# and text too, and an empty helper frame is in neither.
# A page's frames, of its own site and of another, are in its outline, where the agent clicks and
# types, each answered with what changed in the outline alone, and not into a frame's password
# field. Its text leaves out a frame an extension puts in
# the page, which isn't the page's. Its keys, typed or pressed, go to the page alone, and Escape
# the page doesn't keep runs no Stop of ilo's. Where it acts in the page watched, its pointer and a
# ring with the element's ref show it. It opens a page, clicks its link, goes back, where the tab
# takes the page's title again, scrolls, takes a screenshot and closes a tab, and a bank's site asks first, in a card whose Allow Once opens it. Always Allow
# opens it from then on, as Settings › Agents lists with its Revoke. A login the
# user saved fills in neither as the agent's tab loads it nor from a dropdown, and fills once the
# user takes the tab over, without a reload. Closing the client's end ends its session with the tab.
# Reconnected, its next call is a card of its own, whose ✕ answers the call under way and closes
# the client's connection. Turning Settings › Agents off takes the socket away and drops a client
# connected.

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
# page, as a password manager puts its own in pages. The frame tells the page it's there.
extension="$PWD/$out/frame-extension"
mkdir -p "$extension"
cat > "$extension/manifest.json" << 'EOF'
{
  "manifest_version": 3,
  "name": "Frame check",
  "version": "1",
  "content_scripts": [{"matches": ["http://localhost/frames.html*"], "js": ["content.js"]}],
  "web_accessible_resources": [{"resources": ["frame.html"], "matches": ["http://localhost/*"]}]
}
EOF
printf '%s' 'const frame = document.createElement("iframe");
frame.src = chrome.runtime.getURL("frame.html");
document.body.append(frame);' > "$extension/content.js"
printf '%s' '<p>Extension text</p><script src=frame.js></script>' > "$extension/frame.html"
printf '%s' 'parent.postMessage("extension frame", "*");' > "$extension/frame.js"
BROWSER_DRIVE=1 BROWSER_URL='https://en.wikipedia.org/wiki/Arc_(web_browser)' \
  "$browser" --load-extension="$(app_path "$extension")" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
user_tab=$(today | cut -d' ' -f1)
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
# Prints a tool's answer on stdin: its text, after "error: " if it's an error, or a picture's type
# and size.
answered() {
  python3 -I -c '
import json, sys
result = json.load(sys.stdin)["result"]
content = result["content"][0]
if content["type"] == "image":
    print(content["mimeType"], len(content["data"]))
else:
    print(("error: " if result["isError"] else "") + content["text"])'
}
# The tools/call request for the tool $1 with the arguments $2.
request() {
  id=$((id + 1))
  echo "{\"jsonrpc\":\"2.0\",\"id\":$id,\"method\":\"tools/call\",\"params\":{\"name\":\"$1\",\"arguments\":$2}}"
}
# Calls the tool $1 with the arguments $2, and prints its answer.
call() {
  mcp "$(request "$1" "$2")" | answered
}

mcp '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}' \
  > "$out/mcp-initialize.json"
grep -q '"protocolVersion":"2025-06-18"' "$out/mcp-initialize.json" ||
  fail "initialize didn't answer in the client's version"
await_log '^browser: agent mcp-client hello from check$' "the app didn't hear the client's hello"
mcp '{"jsonrpc":"2.0","method":"notifications/initialized"}'
mcp '{"jsonrpc":"2.0","id":100,"method":"tools/list"}' | grep -q '"name":"read_page"' ||
  fail "tools/list didn't list read_page"

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
  send click $((wx + ww / 2)) $((wy + wh / 2))
  await_log "^browser: task $session page $1\$" "a click on tab $1's picture didn't watch it"
  await_log "^browser: task page $1 in place\$" "tab $1's page didn't come to rest in place"
}
send click $page_point
driven=$active
if [ "$driven" = 1 ]; then
  # A click on the card opens its conversation, its steps in its agent's place.
  read -r px py pw ph <<PLATE
$(grep "^browser: task plate task-${session#session-} at " "$log" | tail -1 | cut -d' ' -f6-)
PLATE
  send click $((px + pw / 2)) $((py + ph / 2))
  expect "^browser: task $session shown\$" "a click on the client's card didn't show its conversation"
  watch "$agent_tab"
  ../tools/window-screenshot.sh "$pid" "$out/agent-watched.png" || fail "no window to capture"
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
note.oninput = () => document.title = "noted";
painted(() => document.title = "fields");
</script>' > "$out/site/fields.html"
  call navigate "{\"tab\":$agent_tab,\"url\":\"localhost:$port/fields.html\"}" > /dev/null
  await_log "^browser: title $agent_tab fields\$" "the fields page didn't load in the agent's tab"
  # A click while the page still loads can land before its field is drawn, and focus no field.
  await_log "^browser: page $agent_tab back 1 forward 0 loading 0 http://localhost:$port/fields\\.html\$" \
    "the fields page didn't finish loading"
  send click $page_point
  [ "$focus" = RenderWidgetHostViewCocoa ] || fail "a click on the fields page left focus on $focus"
  send type typed-by-ada
  expect "^browser: title $agent_tab typed\$" "typing didn't reach the page's field"
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

  # The outline names the fields by ref, the password field off limits, and gives no value.
  outline=$(call snapshot "{\"tab\":$agent_tab}")
  printf '%s\n' "$outline" > "$out/mcp-outline.txt"
  case "$outline" in
    "Tab $agent_tab: typed"*"Fields for the agents check."*"(password: off limits)"*) ;;
    *) fail "the fields page's outline was: $outline" ;;
  esac
  for secret in $secrets; do
    case "$outline" in *"$secret"*) fail "the fields page's outline gave a field's value, $secret" ;; esac
  done
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
  # other site's, and a frame the page adds after it has loaded, are in the outline and the text
  # too, in the page's order, and an empty helper frame is in neither (#1444).
  printf '%s' '<p>Inner text</p><button>Inner button</button><input aria-label="Inner field">' \
    > "$out/site/inner.html"
  printf '%s' '<p>Cross text</p>
<button onclick="document.body.append(&quot;cross clicked&quot;)">Cross button</button>
<input aria-label="Cross field" oninput="document.body.append(&quot;cross typed&quot;)">
<input type=password aria-label="Cross password"><script src="painted.js"></script>
<script>painted(() => parent.postMessage("painted", "*"));</script>' > "$out/site/cross.html"
  printf '%s' "<iframe src=http://nested.localhost:$port/deep.html></iframe>" \
    >> "$out/site/cross.html"
  printf '%s' '<p>Deep text</p>' > "$out/site/deep.html"
  printf '%s' '<p>Late text</p><script>top.postMessage("late", "*")</script>' \
    > "$out/site/late.html"
  : > "$out/site/empty.html"
  printf '%s' "<title>loading</title><p>Top text</p><iframe src=inner.html></iframe>
<iframe src=http://127.0.0.1:$port/cross.html></iframe>
<iframe title='Helper frame' src=empty.html width=1 height=1></iframe>
<script>addEventListener('message', (event) => {
  if (event.data === 'painted') {
    document.title = 'frames';
    const late = document.createElement('iframe');
    late.src = 'http://127.0.0.1:$port/late.html';
    document.body.append(late);
  }
  if (event.data === 'late') document.title = 'frames late';
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
  await_log "^browser: title $frames_tab frames late\$" "the frame added after the load didn't load"
  later=$(call snapshot "{\"tab\":$frames_tab}")
  printf '%s\n' "$later" > "$out/mcp-frames-later.txt"
  case "$later" in
    *"Helper frame"*) fail "the outline had the empty helper frame: $later" ;;
    *"Top text"*"Inner text"*"Cross text"*"Deep text"*"Late text"*) ;;
    *) fail "the frames page was outlined later as: $later" ;;
  esac
  read=$(call read_page "{\"tab\":$frames_tab}")
  printf '%s\n' "$read" > "$out/mcp-frames-text.txt"
  case "$read" in
    *"Extension text"*) fail "reading the frames page gave the extension's frame's text: $read" ;;
    *"Top text"*"Inner text"*"Cross text"*"Deep text"*"Late text"*) ;;
    *) fail "reading the frames page answered: $read" ;;
  esac

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
  # The agent waits out the load before it presses and again after, 15 s each (kLoadTimeout in
  # agent_page.cc), so the answer comes after the 30 s next_answer waits.
  request press_key "{\"tab\":$keys_tab,\"key\":\"Escape\"}" >&4
  await_log "^browser: title $keys_tab escape heard\$" "press_key Escape didn't reach the page"
  pressed=$(perl -e 'alarm 60; exec @ARGV' sh -c 'read -r line && printf "%s\n" "$line"' <&5) ||
    fail "the browser didn't answer press_key Escape"
  pressed=$(printf '%s\n' "$pressed" | answered)
  case "$pressed" in "Pressed Escape."*) ;; *) fail "press_key Escape answered: $pressed" ;; esac
  kill -0 "$pid" 2>/dev/null || fail "the app exited as the agent pressed Escape"
  [ "$(grep -c "$stopped" "$log" || true)" = "$stops" ] ||
    fail "the agent's Escape, which the page didn't keep, ran ilo's Stop"
  [ "$(tabs)" = "$before" ] || fail "the agent's Escape changed the tabs in its Space"
  closed=$(call close_tab "{\"tab\":$keys_tab}")
  case "$closed" in "Closed tab $keys_tab."*) ;; *) fail "closing the keys page answered: $closed" ;; esac

  # It opens a page in its Space, clicks its link, scrolls and takes a screenshot.
  opened=$(call open_tab '{"url":"example.com"}')
  case "$opened" in
    "Opened tab "*"URL: https://example.com/"*) ;;
    *) fail "open_tab answered: $opened" ;;
  esac
  opened_tab=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  await_log "^browser: agent page $opened_tab [0-9]* [1-9][0-9]*\$" "the core didn't outline the opened page"
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
  # Hand Back, at the right end of the card's buttons.
  send click $((qx + qw - 16)) $((qy + qh - 26))
  await_log "^browser: agent mcp-client handed back $opened_tab\$" "Hand Back didn't hand the tab back"
  handed=$(next_answer) || fail "the browser didn't answer hand_off"
  handed=$(printf '%s\n' "$handed" | answered)
  case "$handed" in
    "The user handed tab $opened_tab back."*) ;;
    *) fail "hand_off answered: $handed" ;;
  esac

  # From the conversation, the strip under the page watched takes the tab the agent is at work
  # in over with ⌘↩, and the agent's next action there is refused until the user hands it back.
  send key escape
  await_log "^browser: task page $agent_tab back\$" "Escape didn't step back to the conversation"
  await_log '^browser: task page in its thumbnail$' "the page watched didn't go back into its picture"
  watch "$opened_tab"
  await_log "^browser: task strip $opened_tab at " "the page watched showed no strip"
  ../tools/window-screenshot.sh "$pid" "$out/agent-strip.png" || fail "no window to capture"
  send key return cmd
  await_log "^browser: agent mcp-client taken over in $opened_tab\$" "⌘↩ didn't take the tab"
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
  kept=$(printf '%s\n' "$kept" | answered)
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
  take_over() {
    taken="^browser: agent mcp-client taken over in $opened_tab\$"
    before=$(grep -c "$taken" "$log" || true)
    send key return cmd
    await_more "$taken" "$before" || fail "$1: ⌘↩ didn't take the tab"
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
  # A click in the username field, where Chrome's dropdown would offer the login.
  send click 600 60
  # Chrome fills and offers a saved login within a moment of the load and the click. Here none
  # may come, so this waits out that moment.
  sleep 1
  ../tools/window-screenshot.sh "$pid" "$out/agent-login-empty.png" || fail "no window to capture"
  [ "$(children)" = "$shown" ] || fail "the agent's tab offered the saved login in a dropdown"
  grep -q "^browser: title $opened_tab filled\$" "$log" &&
    fail "Chrome filled the saved password in the agent's tab"
  outline=$(call snapshot "{\"tab\":$opened_tab}")
  case "$outline" in *hunter2*) fail "the login page's outline gave the password" ;; esac
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
    allowed=$(printf '%s\n' "$allowed" | answered)
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
  # The agent is at work in the tab it last acted in, the bank's it opened.
  at=$(grep '^browser: agent mcp-client at work in ' "$log" | tail -1 | awk '{print $NF}')
  await_log "^browser: agent mcp-client done in $at\$" "the tab still shows a gone agent at work"
  await_log "^browser: agent question $pending gone\$" "a gone agent's question stayed open"

  # Reconnected, the client's next call on a tab is a card of its own. Its ✕, which stops it as
  # Stop does, answers the call under way that the user stopped it, then closes the client's
  # connection, which the bridge ends with, until the user reconnects it (#1080). It's used from
  # the foot, not the conversation, whose step under way shimmers while it waits on the user: on
  # CI's software rendering those frames starve the driver's idle callbacks.
  hello='^browser: agent mcp-client hello from check$'
  initialize='{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}'
  # children sends its layout from a subshell, whose steps the count here didn't keep.
  step=$(sed -n 's/^browser: drive \([0-9]*\)[: ].*/\1/p' "$log" | sort -n | tail -1)
  bridge_on
  hellos=$(grep -c "$hello" "$log" || true)
  mcp "$initialize" > /dev/null
  await_more "$hello" "$hellos" || fail "the app didn't hear the reconnected client's hello"
  sessions=$(grep -c '^browser: session session-[0-9]* of agent mcp-client as check$' "$log" || true)
  request hand_off "{\"tab\":$opened_tab,\"reason\":\"Sign in once more\"}" >&4
  await_more '^browser: session session-[0-9]* of agent mcp-client as check$' "$sessions" ||
    fail "the reconnected client's call started no card of its own"
  session=$(sed -n 's/^browser: session \(session-[0-9]*\) of agent mcp-client as check$/\1/p' "$log" | tail -1)
  await_log "^browser: agent mcp-client hands off $opened_tab: Sign in once more\$" \
    "the reconnected client's hand_off didn't hand the tab over"
  await_log "^browser: task card $session Waiting " "the reconnected client's card didn't wait for the user"
  await_log "^browser: task plate task-${session#session-} at " "the reconnected client's card wasn't drawn"
  read -r px py pw ph <<PLATE
$(grep "^browser: task plate task-${session#session-} at " "$log" | tail -1 | cut -d' ' -f6-)
PLATE
  # ✕ shows on the hovered plate, at its right end.
  send move $((px + pw / 2)) $((py + ph / 2))
  ../tools/window-screenshot.sh "$pid" "$out/agent-session.png" || fail "no window to capture"
  send click $((px + pw - 20)) $((py + ph / 2))
  expect '^browser: command dismiss-task via sidebar$' "a click on the card's ✕ didn't close it"
  expect '^browser: agent mcp-client stopped by the user$' "closing the card didn't stop the client"
  stopped=$(next_answer) || fail "the browser didn't answer the hand_off it stopped"
  stopped=$(printf '%s\n' "$stopped" | answered)
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
  send clickkey $(drawn "switch agents")
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
forget_login
trap cleanup EXIT
rm -f "$agents_file" "$out/mcp-in" "$out/mcp-out"
if [ "$driven" = 1 ]; then
  echo "ok: an MCP client's bridge gets in with its token only, on the user's own socket, gets a Space of its own, is refused the user's tab and reads its own, is answered that a link to an app opens nothing, its tab staying on its page, moves ten cases to Closed in tabs the user isn't looking at a step a turn, its calls on the ten answered at once ($cases), without its fields' values, acts by ref but never in a password field, presses keys that reach the page alone, its Escape running no Stop of ilo's, closes its own tab, runs steps in one call, waits on a handoff until Hand Back, works on one card for both its connections, whose conversation watches its page, is refused a tab the user took over with the strip's ⌘↩ until they hand it back and Keep answers its ask, is stopped by its card's ✕, which closes its connection once its call is answered, gets no saved login filled in or offered in its tab until the user takes it over, when it fills without a reload, a bank's site asks first until Always Allow, which Settings › Agents lists, and off takes the socket away"
else
  echo "ok: an MCP client's bridge gets in with its token only, on the user's own socket, gets a Space of its own and is refused the user's tab, is answered that a link to an app opens nothing, its tab staying on its page, and moves ten cases to Closed in tabs the user isn't looking at a step a turn, its calls on the ten answered at once ($cases). skip: reading a page in its Space. Browser wasn't the active app, so the page ignored the click."
fi
if [ -n "$msix" ]; then
  echo "ok: a Store copy's bridge, run by the alias its MSIX declares in the user's WindowsApps folder, carries a client's hello to the browser and its answer back."
elif windows; then
  echo "skip: a Store copy's bridge, by its alias. No MSIX of this build is installed here; CHECK_MSIX names one."
fi
