# An MCP client reaches the browser through the bridge beside it once Settings › Agents is on and
# it's connected, as the saved settings say here. The socket is the user's alone, and the bridge
# gets nowhere without the agent's token. Its first call makes its Space, and it may list and read
# only the tabs there: its list leaves out the user's tab, a read of the user's tab is refused, with
# an activity-log entry, and a read of example.com, opened in its Space, gives the page's text. A
# page with fields gives its prose and none of their values: typed, filled in or a password, in its
# text and in its outline, and the agent can't type into the password field. It types into another.
# A page's frames, of its own site and of another, are in its outline, where the agent clicks and
# types, and not into a frame's password field. Its text leaves out a frame an extension puts in
# the page, which isn't the page's. Its keys, typed or pressed, go to the page alone, and Escape
# the page doesn't keep runs no Stop of ilo's. Where it acts in the page shown, its pointer and a
# ring with the element's ref show it. It opens a page, clicks its link, scrolls, takes a screenshot
# and closes a tab, and a bank's site asks first, in a card whose Allow Once opens it. Always Allow
# opens it from then on, until Revoke Permission in the Space's menu takes that back. A login the
# user saved fills in neither as the agent's tab loads it nor from a dropdown, and fills once the
# user takes the tab over, without a reload. Closing the client's end ends its session with the tab,
# and turning Settings › Agents off takes the socket away and drops a client connected.

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
await_log "^browser: added space $agent MCP client\$" "the agent's Space didn't reach the switcher"
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

send click $page_point
driven=$active
if [ "$driven" = 1 ]; then
  send key 2 ctrl
  expect "^browser: space $agent\( in .*\)\{0,1\}\$" "⌃2 didn't show the agent's Space"
  open_from_bar example.com
  agent_tab=$tab
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
  go_from_bar "localhost:$port/fields.html"
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
  # The page shown in the agent's Space has its pointer: at the note it typed into, ringed and
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
  # under a click in, and gives the click to the page around it.
  printf '%s' '<p>Inner text</p><button>Inner button</button><input aria-label="Inner field">' \
    > "$out/site/inner.html"
  printf '%s' '<p>Cross text</p>
<button onclick="document.body.append(&quot;cross clicked&quot;)">Cross button</button>
<input aria-label="Cross field" oninput="document.body.append(&quot;cross typed&quot;)">
<input type=password aria-label="Cross password"><script src="painted.js"></script>
<script>painted(() => parent.postMessage("painted", "*"));</script>' > "$out/site/cross.html"
  printf '%s' "<title>loading</title><p>Top text</p><iframe src=inner.html></iframe>
<iframe src=http://127.0.0.1:$port/cross.html></iframe>
<script>addEventListener('message', (event) => {
  if (event.data === 'painted') document.title = 'frames';
  if (event.data === 'extension frame') document.title = 'frames with extension';
});</script>" > "$out/site/frames.html"
  outline=$(call open_tab "{\"url\":\"localhost:$port/frames.html\"}")
  printf '%s\n' "$outline" > "$out/mcp-frames.txt"
  case "$outline" in
    "Opened tab "*"Top text"*"Inner text"*"Inner button"*"Cross text"*"Cross button"*"(password: off limits)"*) ;;
    *) fail "the page with frames was outlined as: $outline" ;;
  esac
  frames_tab=$(printf '%s\n' "$outline" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  await_log "^browser: title $frames_tab frames\$" "the other site's frame didn't paint"
  inner=$(ref_of button "Inner button")
  cross=$(ref_of button "Cross button")
  [ -n "$inner" ] && [ -n "$cross" ] && [ "$inner" != "$cross" ] ||
    fail "the frames' buttons had the refs '$inner' and '$cross'"
  clicked=$(call click "{\"tab\":$frames_tab,\"ref\":\"$cross\"}")
  case "$clicked" in
    "Clicked button “Cross button”."*"cross clicked"*) ;;
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
  read=$(call read_page "{\"tab\":$frames_tab}")
  printf '%s\n' "$read" > "$out/mcp-frames-text.txt"
  case "$read" in
    *"Extension text"*) fail "reading the frames page gave the extension's frame's text: $read" ;;
    *"Top text"*"Inner text"*"Cross text"*) ;;
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
  scrolled=$(call scroll "{\"tab\":$opened_tab}")
  case "$scrolled" in "Scrolled down."*) ;; *) fail "scroll answered: $scrolled" ;; esac
  picture=$(call screenshot "{\"tab\":$opened_tab}")
  case "$picture" in "image/jpeg "[1-9]*) ;; *) fail "screenshot answered: $picture" ;; esac
  # close_tab closes a tab of its own, which goes to the archive.
  opened=$(call open_tab '{"url":"example.com"}')
  closing=$(printf '%s\n' "$opened" | sed -n '1s/^Opened tab \([0-9]*\)\..*/\1/p')
  closed=$(call close_tab "{\"tab\":$closing}")
  [ "$closed" = "Closed tab $closing. The user can still find it in the archive." ] ||
    fail "close_tab answered: $closed"
  await_log "^browser: agent activity mcp-client $closing close_tab\$" \
    "the close didn't reach the activity log"
  listed=$(call list_tabs '{}')
  printf '%s\n' "$listed" | grep -q "^. $closing  " && fail "list_tabs listed the closed tab"

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

  # The bar in the agent's Space takes the tab it's at work in over, and the agent's next action
  # there is refused until the user hands it back from the bar.
  # The bar logs its place as it changes, which a new tab doesn't.
  await_log '^browser: agent bar [0-9]* at ' "the agent's Space showed no bar"
  read -r bx by bw bh <<BAR
$(grep '^browser: agent bar [0-9]* at ' "$log" | tail -1 | cut -d' ' -f6-)
BAR
  ../tools/window-screenshot.sh "$pid" "$out/agent-bar.png" || fail "no window to capture"
  # Stop's tooltip shows its key, as Take Over's does Back to Pinned URL's.
  hidden=$(grep -c '^browser: tooltip hidden$' "$log" || true)
  faded=$(grep -c '^browser: tooltip window shown$' "$log" || true)
  send move $((bx + bw / 4)) $((by + bh - 26))
  await_log '^browser: tooltip shown Stop \(⌘\.\|Ctrl+\.\)$' "resting on Stop didn't show its key"
  await_more '^browser: tooltip window shown$' "$faded" || fail "Stop's tooltip didn't fade in"
  ../tools/window-screenshot.sh "$pid" "$out/agent-bar-tooltip.png" || fail "no window to capture"
  send move $((bx + bw / 2)) $((by - 40))
  await_more '^browser: tooltip hidden$' "$hidden" || fail "leaving Stop didn't hide its tooltip"
  send click $((bx + bw - 16)) $((by + bh - 26))
  await_log "^browser: agent mcp-client taken over in $opened_tab\$" "Take Over didn't take the tab"
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
  # Hand Back, the held tab's bar's one button, at its left.
  send click $((bx + 30)) $((by + bh - 26))
  await_log "^browser: agent mcp-client handed back $opened_tab\$" "the bar's Hand Back didn't"
  scrolled=$(call scroll "{\"tab\":$opened_tab}")
  case "$scrolled" in "Scrolled down."*) ;; *) fail "after Hand Back, scroll answered: $scrolled" ;; esac

  # Nothing of the user's autofills in the agent's tab. A throwaway login, saved through the offer
  # while the user holds the tab, on this run's own port, fills in neither as the page loads nor
  # from a dropdown once the agent has the tab back. Take Over fills it at once, without a reload,
  # and the dropdown offers it. The bar logs its place as it changes.
  bar() {
    read -r bx by bw bh <<BAR
$(grep '^browser: agent bar [0-9]* at ' "$log" | tail -1 | cut -d' ' -f6-)
BAR
  }
  # Take Over, at the right end of the bar's buttons.
  take_over() {
    taken="^browser: agent mcp-client taken over in $opened_tab\$"
    before=$(grep -c "$taken" "$log" || true)
    bar
    send click $((bx + bw - 16)) $((by + bh - 26))
    await_more "$taken" "$before" || fail "$1: Take Over didn't take the tab"
  }
  # How many child windows the browser window has, Chrome's autofill dropdown among them.
  children() {
    send layout
    sed -n "/^browser: drive $step: /,\$p" "$log" | grep -c '^browser: layout child' || true
  }
  logins() {
    grep -c "^browser: title $opened_tab login\$" "$log" || true
  }
  # Waits until no tooltip is up, its window faded out too, so it isn't counted as a child window.
  no_tooltip() {
    for _ in $(seq 50); do
      case "$(grep '^browser: tooltip \(shown\|hidden\)' "$log" | tail -1)" in
        "browser: tooltip shown "*) sleep 0.1 ;;
        # Its window fades out in 270 ms (tooltip.rs FADE_OUT).
        *) sleep 0.4; return 0 ;;
      esac
    done
    return 1
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
  # Hand Back, the held tab's bar's one button, at its left.
  bar
  send click $((bx + 30)) $((by + bh - 26))
  await_more "$handed" "$before" || fail "the bar's Hand Back didn't hand the login's tab back"
  # Off the bar, which comes back with Stop under where Hand Back was, and Stop's tooltip would
  # count below as a dropdown's child window.
  send move $((bx + bw / 2)) $((by - 40))
  tips=$(grep -c '^browser: tooltip shown ' "$log" || true)

  before=$(logins)
  call navigate "{\"tab\":$opened_tab,\"url\":\"localhost:$port/login.html\"}" > /dev/null
  await_more "^browser: title $opened_tab login\$" "$before" ||
    fail "the login page didn't load in the agent's tab"
  no_tooltip || fail "a tooltip stayed up with the pointer off the agent bar"
  shown=$(children)
  # A click in the username field, where Chrome's dropdown would offer the login.
  send click 600 60
  # Chrome fills and offers a saved login within a moment of the load and the click. Here none
  # may come, so this waits out that moment.
  sleep 1
  ../tools/window-screenshot.sh "$pid" "$out/agent-login-empty.png" || fail "no window to capture"
  [ "$(children)" = "$shown" ] || fail "the agent's tab offered the saved login in a dropdown"
  [ "$(grep -c '^browser: tooltip shown ' "$log" || true)" = "$tips" ] ||
    fail "a tooltip showed with the pointer off the agent bar: $(grep '^browser: tooltip shown ' "$log" | tail -1)"
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
  bar
  send click $((bx + 30)) $((by + bh - 26))
  await_more "$handed" "$before" || fail "the bar's Hand Back didn't hand the filled tab back"

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

  # The Space's menu lists the permission, and revoking it asks again.
  post rightclick $(row 9)
  expect "^browser: menu shown .*$(cased 'Revoke Permission › Always Allow') on bank\\.localhost" \
    "the agent's Space's menu didn't list its permission on the bank's site"
  send choose "$(cased 'Revoke Permission › Always Allow') on bank.localhost"
  await_log '^browser: agent mcp-client revoked bank\.localhost$' "Revoke didn't revoke the permission"
  request open_tab "$bank" >&4
  await_more "$asks" "$before" || fail "a revoked permission didn't ask again"
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

  # Off in Settings › Agents takes the socket away and drops a client connected.
  bridge_on
  hello='^browser: agent mcp-client hello from check$'
  hellos=$(grep -c "$hello" "$log" || true)
  mcp '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}' \
    > /dev/null
  await_more "$hello" "$hellos" || fail "the app didn't hear the second client's hello"
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings"
  settings_drew
  # The seventh tab, and the switch at the end of the first row.
  send clickkey $(drawn "toolbar Agents")
  expect '^browser: settings tab Agents$' "a click on Agents didn't show its tab"
  settings_drew Agents
  ../tools/window-screenshot.sh "$pid" "$out/settings-agents.png" || fail "no window to capture"
  send clickkey 592 134
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
  echo "ok: an MCP client's bridge gets in with its token only, on the user's own socket, gets a Space of its own, is refused the user's tab and reads its own, without its fields' values, acts by ref but never in a password field, presses keys that reach the page alone, its Escape running no Stop of ilo's, closes its own tab, runs steps in one call, waits on a handoff until Hand Back, is refused a tab the user took over until they hand it back and Keep answers its ask, its bar's Stop naming ⌘. in its tooltip, gets no saved login filled in or offered in its tab until the user takes it over, when it fills without a reload, a bank's site asks first until Always Allow and again once that's revoked, and off takes the socket away"
else
  echo "ok: an MCP client's bridge gets in with its token only, on the user's own socket, gets a Space of its own and is refused the user's tab. skip: reading a page in its Space. Browser wasn't the active app, so the page ignored the click."
fi
