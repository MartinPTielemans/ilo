# Pages that break show ilo's own state. A page that stops answering for Chrome's hang timeout gets
# the toast's Not responding with Wait and Reload, and a mark on its row: the note goes by itself
# once the page answers, Wait gives it longer, and Reload ends its renderer and loads it again, with
# no card. A page whose renderer dies, killed as macOS kills one under memory pressure, leaves its
# pane to the card's This page stopped, whose Reload, or ⇧⌘R, loads it again, while a tab behind
# whose renderer dies is unloaded instead and loads once it's shown, with no card. A load that fails
# shows the core's error page, with the error's code, and one refused for want of a server loads
# again by itself, through Chromium's auto-reload, once the server is there. Offline, the page
# offers ilo's game, which Space starts and a click doesn't, at chrome://dino too, with ⌘L still
# the browser's over a run; and a page whose network comes back mid-run loads once the run ends.

serve
# Typing w in it keeps its renderer busy for 20 s, past Chrome's 15 s hang timeout, then the title
# says so, counted, since a title that doesn't change logs nothing.
printf '%s' '<title>loading</title><body style="background:#333">
<script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
let wakes = 0;
onkeydown = e => {
  if (e.key != "w") {
    return;
  }
  const end = Date.now() + 20000;
  while (Date.now() < end) {}
  document.title = "woke " + ++wakes;
};
</script>' > "$out/site/busy.html"
log="$out/trouble.log"
step=0
rm -f "$saved" "$launch_frame"
# DevTools' port takes the tab offline later, as a lost network would.
ports="$(dirname "$saved")/DevToolsActivePort"
rm -f "$ports"
BROWSER_DRIVE=1 BROWSER_URL="${site}busy.html" "$browser" --remote-debugging-port=0 \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the busy page didn't draw"
tab=$(today | cut -d' ' -f1)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: trouble. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi

# expect_late, failing at once if the app exits: Chrome calls a page hung only 15 s after the input
# it hasn't answered.
expect_hung() {
  for _ in $(seq 300); do
    sed -n "/^browser: drive $step: /,\$p" "$log" | grep -q "$1" && return 0
    kill -0 "$pid" 2>/dev/null || fail "the app exited"
    sleep 0.1
  done
  fail "$2"
}
# The toast's buttons, from the right of the window's content, $width wide: it ends 18 in from the
# window's edge, its last button 13.5 in from its own, each button 9 + its label + 9 wide, 8 apart,
# all 41 down. Reload's middle is about 45 in from the toast's edge and Wait's about 106.
send layout
content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
  sed 's/^browser: layout content \([0-9]*x[0-9]*\),.*/\1/')
width=${content%x*}
toast_reload="$((width - 18 - 45)) 41"
toast_wait="$((width - 18 - 106)) 41"
hang() {
  send click $page_point
  send key w
  expect_hung "browser: tab $tab page hung" "$1: the busy page wasn't called hung"
  expect "^browser: page $tab trouble hung\$" "$1: the app didn't hear the page hung"
  expect '^browser: toast shown Not responding \[wait, reload\]$' "$1: the toast didn't say Not responding"
  expect '^browser: toast settled$' "$1: the toast didn't settle"
}

# Hung, the note shows with the row's mark, and goes by itself once the page answers.
hang "hung"
../tools/window-screenshot.sh "$pid" "$out/hung.png" || fail "no window to capture"
expect_hung "^browser: title $tab woke 1\$" "the busy page didn't wake"
expect "^browser: page $tab trouble fine\$" "the page answering didn't make it fine"
expect '^browser: toast hidden$' "the note stayed once the page answered"

# Wait gives it Chrome's timeout again, which the page answers within.
hang "wait"
send move $toast_wait
send click $toast_wait
expect '^browser: toast button wait$' "Wait wasn't pressed at $toast_wait"
expect "^browser: page $tab trouble fine\$" "Wait didn't make the page fine"
expect_hung "^browser: title $tab woke 2\$" "the busy page didn't wake after Wait"
none_since "page $tab trouble hung" || fail "the page was called hung again within Wait's time"

# Reload ends the renderer and loads the page again, without the crash card.
hang "reload"
send move $toast_reload
send click $toast_reload
expect '^browser: toast button reload$' "Reload wasn't pressed at $toast_reload"
expect "browser: tab $tab page gone" "Reload didn't end the hung renderer"
expect "^browser: loaded $tab ${site}busy.html\$" "Reload didn't load the hung page again"
none_since "page $tab trouble crashed" || fail "a page Reload ended showed the crash card"

# A second tab, from the command bar. The first is behind it.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send insert "${site}index.html"
send key return
expect "^browser: today [0-9]* $tab\$" "the second tab didn't open over the first"
second=$(today | cut -d' ' -f1)
expect "^browser: loaded $second " "the second tab didn't load"

# Every page's renderer killed: the tab shown shows the card, and the one behind is unloaded,
# with none.
kill_pages || fail "no renderer to kill"
expect "browser: tab $second page gone" "a killed renderer didn't report its page gone"
expect "^browser: page $second trouble crashed\$" "the tab shown wasn't called crashed"
expect "^browser: crash card shown $second\$" "the crash card didn't show"
expect "^browser: page $tab unloaded 1\$" "the tab behind wasn't unloaded"
none_since "page $tab trouble crashed" || fail "the tab behind was called crashed"
# The bundled extension's process lives on, so it stays in the Extensions menu (#1091).
none_since '^browser: extensions menu $' || fail "the pages' renderers took the extension with them"
../tools/window-screenshot.sh "$pid" "$out/crashed.png" || fail "no window to capture"
# Its Reload, where the card says it is.
at=$(sed -n "s/^browser: crash reload $second at \([0-9]*\) \([0-9]*\) \([0-9]*\) \([0-9]*\)$/\1 \2 \3 \4/p" \
  "$log" | tail -1)
[ -n "$at" ] || fail "the crash card's Reload wasn't drawn"
set -- $at
send click $(($1 + $3 / 2)) $(($2 + $4 / 2))
expect "^browser: crash card reload $second\$" "the crash card's Reload wasn't pressed"
expect "^browser: loaded $second ${site}index.html\$" "Reload didn't load the dead page again"
expect "^browser: page $second trouble fine\$" "the page loaded again wasn't fine"

# Killed again, ⇧⌘R loads it, as Arc's does its Sad Tab, and the card goes.
kill_pages || fail "no renderer to kill again"
expect "^browser: page $second trouble crashed\$" "the tab shown wasn't called crashed again"
expect "^browser: crash card shown $second\$" "the crash card didn't show again"
send key r cmd shift
expect '^browser: command force-reload via [a-z]*$' "⇧⌘R didn't run Force Refresh on the crash card"
expect "^browser: loaded $second ${site}index.html\$" "⇧⌘R didn't load the dead page again"
expect "^browser: page $second trouble fine\$" "the page ⇧⌘R loaded wasn't fine"

# The tab behind loads once it's shown, with no card.
send click $(row 1)
expect "^browser: selected $tab in " "the first tab wasn't shown"
expect "^browser: loaded $tab ${site}busy.html\$" "the tab behind didn't load once shown"
none_since "trouble crashed" || fail "the tab behind showed the crash card"

# A host that doesn't resolve shows the core's error page, with no card and no mark. Typed with its
# scheme, a .invalid host, which never resolves, is an address the bar goes to, where Chrome's would
# search for it, and https, since over http Chromium's HTTPS-First warning would cover the page.
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send insert "https://nothing.invalid/"
send key return
expect '^browser: command bar go https://nothing.invalid/$' "the bar didn't go to the address typed with its scheme"
expect "^browser: page $tab trouble network -105\$" "a host that doesn't resolve wasn't a network error"
await '^browser: title [0-9]* nothing.invalid$' || fail "the error page didn't take the host for its title"
../tools/window-screenshot.sh "$pid" "$out/network-error.png" || fail "no window to capture"
# Space on it starts no game, which only the offline page has. A click away from its Reload focuses
# it; the refused step after gives a run that started time to say so.
send click 300 120
send key space

# A port with no server is refused, and loads by itself once a server takes it.
free=$(python3 -I -c 'import socket; s = socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send insert "http://127.0.0.1:$free/"
send key return
expect "^browser: page $tab trouble network -102\$" "a port with no server wasn't refused"
python3 -I -m http.server "$free" --bind 127.0.0.1 --directory "$out/site" > "$out/late-server.log" 2>&1 &
stuck=$!
expect_hung "^browser: loaded $tab http://127.0.0.1:$free/\$" "the refused page didn't load once the server was there"
expect "^browser: page $tab trouble fine\$" "the page that came wasn't fine"
kill "$stuck"
stuck=
grep -q 'browser: tab [0-9]* game running$' "$log" && fail "Space started a game on a page that wasn't offline"

# Offline. A DevTools session on the tab, which DevTools' own Offline is, fails its loads with
# ERR_INTERNET_DISCONNECTED until it says online. Its cache is off, as DevTools offers, since the
# throttle sits under the HTTP cache and a fresh copy of a page loaded earlier would still load. It
# takes "offline" or "online" on its input and says each back once Chromium has it.
cat > "$out/network.py" <<'PY'
import base64, json, os, socket, struct, sys, urllib.request

port = open(sys.argv[1]).read().split()[0]
targets = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list"))
url = next(t["webSocketDebuggerUrl"] for t in targets
           if t["type"] == "page" and t["url"].startswith(sys.argv[2]))
sock = socket.create_connection(("127.0.0.1", int(port)))
key = base64.b64encode(os.urandom(16)).decode()
sock.sendall((f"GET {url.split(port, 1)[1]} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n"
              "Upgrade: websocket\r\nConnection: Upgrade\r\n"
              f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").encode())
head = b""
while not head.endswith(b"\r\n\r\n"):
    head += sock.recv(1)
if b" 101 " not in head.split(b"\r\n")[0]:
    sys.exit(head.decode())

def read(size):
    data = b""
    while len(data) < size:
        chunk = sock.recv(size - len(data))
        if not chunk:
            sys.exit("DevTools closed the session")
        data += chunk
    return data

# A message from the browser: its frames aren't masked, and a long one may come in several.
def receive():
    message = b""
    while True:
        first, second = read(2)
        size = second & 0x7F
        if size == 126:
            size = struct.unpack(">H", read(2))[0]
        elif size == 127:
            size = struct.unpack(">Q", read(8))[0]
        data = read(size)
        if first & 0x0F in (0, 1, 2):
            message += data
            if first & 0x80:
                return message

calls = 0
def call(method, **params):
    global calls
    calls += 1
    body = json.dumps({"id": calls, "method": method, "params": params}).encode()
    mask = os.urandom(4)
    size = bytes([0x80 | len(body)]) if len(body) < 126 else bytes([0xFE]) + struct.pack(">H", len(body))
    sock.sendall(b"\x81" + size + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(body)))
    while True:
        reply = json.loads(receive())
        if reply.get("id") == calls:
            if "error" in reply:
                sys.exit(f"{method}: {reply['error']}")
            return

call("Network.enable")
call("Network.setCacheDisabled", cacheDisabled=True)
for line in sys.stdin:
    state = line.strip()
    call("Network.emulateNetworkConditions", offline=state == "offline", latency=0,
         downloadThroughput=-1, uploadThroughput=-1)
    print(state, flush=True)
PY
rm -f "$out/network"
mkfifo "$out/network"
python3 -I "$out/network.py" "$ports" "http://127.0.0.1:$free/" < "$out/network" > "$out/network.log" 2>&1 &
network=$!
exec 4> "$out/network"
# Takes the tab $1, offline or online.
network() {
  echo "$1" >&4
  for _ in $(seq 100); do
    grep -qx "$1" "$out/network.log" && return 0
    kill -0 "$network" 2>/dev/null || fail "DevTools didn't take the tab $1: $(cat "$out/network.log")"
    sleep 0.1
  done
  fail "DevTools didn't take the tab $1"
}
network offline
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send insert "${site}index.html"
send key return
expect "^browser: page $tab trouble network -106\$" "offline, a load wasn't ERR_INTERNET_DISCONNECTED"
../tools/window-screenshot.sh "$pid" "$out/offline.png" || fail "no window to capture"
# A click focuses the page and starts nothing: the run comes in Space's step, not the click's.
send click $page_point
send key space
expect "browser: tab $tab game running\$" "Space didn't start a run on the offline page"
# The network comes back mid-run. Chromium's auto-reload, due a second after the page or every
# few after, waits for the run, which ends at the first column with no more hops.
network online
../tools/window-screenshot.sh "$pid" "$out/offline-game.png" || fail "no window to capture"
expect "browser: tab $tab game over\$" "the run didn't end at the first column"
sed -n "/browser: tab $tab game running\$/,/browser: tab $tab game over\$/p" "$log" |
  grep -q "^browser: page $tab trouble fine\$\|^browser: loaded $tab ${site}index.html\$" &&
  fail "the page came back mid-run"
expect "^browser: loaded $tab ${site}index.html\$" "the page didn't load once the run ended"
expect "^browser: page $tab trouble fine\$" "the page that came back wasn't fine"
exec 4>&-
wait "$network" || fail "DevTools' session ended badly: $(cat "$out/network.log")"

# chrome://dino opens the offline page, and ⌘L stays the browser's over a run, since the page
# keeps only Space and ↑. The command bar takes the keys, which ends the run while it's open, well
# before an untouched run ends at the first column, 2.4 s in.
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send insert "chrome://dino"
send key return
expect "^browser: page $tab trouble network -106\$" "chrome://dino didn't open the offline page"
send click $page_point
send key space
expect "browser: tab $tab game running\$" "Space didn't start a run at chrome://dino"
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the command bar over a run"
expect "browser: tab $tab game over\$" "the run at chrome://dino didn't end with the command bar open"
# Chromium's prefix has the time of day, hhmmss.micro, after the date.
run="$(grep -E "browser: tab $tab game (running|over)\$" "$log" | tail -2 |
  sed -E 's|^[^/]*/([0-9]{2})([0-9]{2})([0-9.]+):.*|\1 \2 \3|' |
  awk '{ t = $1 * 3600 + $2 * 60 + $3 } NR == 2 { printf "%.2f", t - last } { last = t }')"
awk -v run="$run" 'BEGIN { exit !(run != "" && run < 2) }' ||
  fail "the run at chrome://dino lasted ${run:-?} s: the command bar didn't end it"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"

quit_by_key
exited
echo "ok: a page that stops answering gets Not responding with Wait and Reload and goes by itself once it answers, Wait gives it longer and Reload ends its renderer and loads it again without the crash card, a dead page's pane shows This page stopped, whose Reload loads it again, a tab behind whose renderer dies is unloaded and loads once shown, a host that doesn't resolve shows the core's error page and a refused page loads by itself once its server comes. Offline, Space starts the game and a click doesn't, a page whose network comes back mid-run loads only once the run ends, chrome://dino opens it, and ⌘L opens the bar over a run, which ends it. Windows: $out/hung.png, $out/crashed.png, $out/network-error.png, $out/offline.png, $out/offline-game.png"
