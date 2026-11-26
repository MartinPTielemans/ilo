# check.sh runs this at 605.
# Link cards (#3080), end to end on the run's own servers, with no public site. ⇧ alone on a link,
# with its plate showing, asks the link's page as no page and no user would, and the plate grows
# into a card of the page's title, description and picture; the page still gets the ⇧, and Esc
# takes the card without reaching the page. The run's https server, which the run calls public
# with Chromium's own --ip-address-space-overrides, as handback's sign-in server, asks each
# connection for a client certificate and logs each request: the card's page and picture come
# without a cookie, though the page set one, without a referrer, and without a certificate. A
# second server on 127.0.0.1, which the run leaves private, gets no request from a card: not for a
# link to it, nor for a redirect from the public server to it. Each card's look is captured as
# link-card-<case>.png: a page with its picture, with none, with nothing in its head, still
# loading, and refused; and the plate growing into it as link-card.mp4 where the OS records.
# Off in Settings › Links, and in an agent's Space, ⇧ asks nothing: no card shows, and the server
# gets no request for the link. The agent is an MCP client on the bridge, as mcp.sh's, whose page
# the user watches from its card and clicks in, so the page gets the ⇧. A watched page isn't the
# window's own tab, so its links get no plate, and ⇧ has nothing to ask the core for.
# Needs the core's link card (#3085), which a checkout without core/link_card.cc doesn't have.

if [ ! -f ../core/link_card.cc ]; then
  echo "skip: link cards. This checkout has no link card core (#3085) yet."
  exit 0
fi
log="$out/link-cards.log"
step=0
rm -f "$saved"
forget_appearance
# Settings › Links' switch is kept off as this file, which an earlier run's end may have left.
cards_off="$profile_dir/link-cards-off"
rm -f "$cards_off"
# The agent's token, which Settings › Other apps keeps, on from the launch as mcp.sh's.
agents_file="$(dirname "$saved")/Agents"
token=$(python3 -I -c 'import secrets; print(secrets.token_hex(32))')
printf '{"on":true,"agents":[{"client":"mcp-client","token":"%s"}]}' "$token" > "$agents_file"
chmod 600 "$agents_file"

# The private server, which only logs: a card must never reach it.
: > "$out/link-cards-private.log"
rm -f "$out/link-cards-private.port"
mkfifo "$out/link-cards-private.port"
python3 -I -u -c '
import http.server, socketserver, sys
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        with open(sys.argv[1], "a", encoding="utf-8") as log:
            log.write(self.path + "\n")
        self.send_response(200)
        self.send_header("Content-Type", "text/html")
        self.end_headers()
        self.wfile.write(b"<title>Private</title>")
    def log_message(self, *_):
        pass
# TCPServer, as site.sh binds: HTTPServer looks 127.0.0.1 up by name, 35 s on the hosted Mac.
server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), Handler)
sys.stdout.buffer.write(b"%d\n" % server.server_address[1])
sys.stdout.close()
server.serve_forever()
' "$out/link-cards-private.log" > "$out/link-cards-private.port" 2> "$out/link-cards-private.err" 3>&- 4>&- 5>&- &
private_server=$!
read -r private_port < "$out/link-cards-private.port" || true
[ -n "$private_port" ] || fail "the private server didn't start: $(cat "$out/link-cards-private.err")"
trap 'cleanup; kill $private_server $public_server 2> /dev/null || true; rm -f "$agents_file" "$cards_off"' EXIT

# The public server's certificate, made for this run and taken by its key alone.
MSYS2_ARG_CONV_EXCL=/CN= openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=cards.test \
  -addext "subjectAltName=DNS:cards.test" \
  -keyout "$out/link-cards.key" -out "$out/link-cards.pem" 2> /dev/null ||
  fail "openssl didn't make the public server's certificate"
spki=$(openssl x509 -in "$out/link-cards.pem" -pubkey -noout | openssl pkey -pubin -outform der |
  openssl dgst -sha256 -binary | base64)

# The page with the links, each a band across it: to the card's page, to the private server, and
# to a redirect to the private server. It sets a cookie for its host, and its title says each key
# it gets with a count, so a second ⇧ shows as the first did and the run sees the Esc never comes.
: > "$out/link-cards-public.log"
rm -f "$out/link-cards-slow.go"
rm -f "$out/link-cards-public.port"
mkfifo "$out/link-cards-public.port"
python3 -I -u -c '
import http.server, os, socketserver, ssl, struct, sys, time, zlib
private = int(sys.argv[2])
def png(width, height):
    row = b"\0" + b"\x2a\x6f\xd8" * width
    chunk = lambda kind, data: (struct.pack(">I", len(data)) + kind + data
                                + struct.pack(">I", zlib.crc32(kind + data)))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(row * height)) + chunk(b"IEND", b""))
band = "display:block;height:80px;font:24px system-ui"
pages = {
    "/index.html": ("text/html", f"""<title>loading</title>
<a href="https://cards.test/page.html" style="{band}">The card&#39;s page</a>
<a href="http://127.0.0.1:{private}/direct" style="{band}">A private server</a>
<a href="https://cards.test/hop" style="{band}">A redirect to it</a>
<a href="https://cards.test/plain.html" style="{band}">A page with no picture</a>
<a href="https://cards.test/bare.html" style="{band}">A page that says nothing</a>
<a href="https://cards.test/slow.html" style="{band}">A slow page</a>
<script>
let keys = 0, presses = 0;
onkeydown = (event) => document.title = "key " + event.key + " " + ++keys;
// A press says so, and a click follows no link: the run clicks where the plate window is.
onmousedown = () => document.title = "pressed " + ++presses;
onclick = (event) => event.preventDefault();
requestAnimationFrame(() => requestAnimationFrame(() =>
  document.title = document.cookie.includes("seen=1") ? "ready" : "no cookie"));
</script>""".encode()),
    "/page.html": ("text/html; charset=utf-8", b"""<html><head>
<title>A page that says what it is</title>
<meta property="og:description" content="Its description, from its head.">
<meta property="og:image" content="/share.png">
</head><body>Its body.</body></html>"""),
    "/share.png": ("image/png", png(120, 63)),
    "/plain.html": ("text/html", b"""<title>A page with no picture</title>
<meta name="description" content="Only words.">"""),
    "/bare.html": ("text/html", b"<p>Nothing in its head."),
    "/slow.html": ("text/html", b"<title>A slow page</title>"),
}
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        path = self.path.split("?")[0]
        certificate = self.connection.getpeercert(binary_form=True)
        with open(sys.argv[1], "a", encoding="utf-8") as log:
            log.write("%s cookie=%s referer=%s certificate=%s\n" % (
                path, "yes" if self.headers.get("Cookie") else "no",
                "yes" if self.headers.get("Referer") else "no",
                "yes" if certificate else "no"))
        if path == "/hop":
            self.send_response(302)
            self.send_header("Location", "http://127.0.0.1:%d/hop" % private)
            self.end_headers()
            return
        # The slow page answers once the run says so, so the card stays loading till then.
        while path == "/slow.html" and not os.path.exists(sys.argv[5]):
            time.sleep(0.05)
        if path not in pages:
            self.send_response(404)
            self.end_headers()
            return
        kind, body = pages[path]
        self.send_response(200)
        self.send_header("Content-Type", kind)
        if path == "/index.html":
            self.send_header("Set-Cookie", "seen=1; Secure; SameSite=Lax")
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, *_):
        pass
server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), Handler)
tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
tls.load_cert_chain(sys.argv[3], sys.argv[4])
# Asks every connection for a client certificate, which a card must never give.
tls.load_verify_locations(sys.argv[3])
tls.verify_mode = ssl.CERT_OPTIONAL
server.socket = tls.wrap_socket(server.socket, server_side=True)
sys.stdout.buffer.write(b"%d\n" % server.server_address[1])
sys.stdout.close()
server.serve_forever()
' "$out/link-cards-public.log" "$private_port" "$out/link-cards.pem" "$out/link-cards.key" \
  "$out/link-cards-slow.go" \
  > "$out/link-cards-public.port" 2> "$out/link-cards-public.err" 3>&- 4>&- 5>&- &
public_server=$!
read -r public_port < "$out/link-cards-public.port" || true
[ -n "$public_port" ] || fail "the public server didn't start: $(cat "$out/link-cards-public.err")"

BROWSER_DRIVE=1 BROWSER_URL="https://cards.test/index.html" "$browser" \
  --host-resolver-rules="MAP cards.test 127.0.0.1:$public_port" \
  --ignore-certificate-errors-spki-list="$spki" \
  --ip-address-space-overrides="127.0.0.1:$public_port=public" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* \(ready\|no cookie\)$' || fail "the page with the links didn't draw"
grep -q '^browser: title [0-9]* ready$' "$log" ||
  fail "the page didn't keep its cookie, which the card's requests must go without"
tab=$(grep -m1 '^browser: title [0-9]* ready$' "$log" | cut -d' ' -f3)
# Below the links, which a click would follow.
send click 600 560
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: link cards. Browser wasn't the active app, so the page got no keys."
  exit 0
fi
# Light by choice, since a runner's system can be dark (the hosted Mac's is), and the
# captures before the dark pass are the light ones.
send menu "Appearance › Light"
expect '^browser: appearance light$' "View › Appearance › Light didn't turn the UI light"

# The window's points of the links' bands, each 80 tall, below the page's top inset.
page_link="600 50"
private_link="600 130"
hop_link="600 210"
plain_link="600 290"
bare_link="600 370"
slow_link="600 450"
# card <point> <url> <case>: rests on the link at <point> until its plate shows, then ⇧ alone.
card() {
  send move $1
  # The core says it as the plate shows it, as Chrome does: an https address without its scheme.
  expect "^browser: link target ${2#https://}\$" "$3: resting on the link showed no link target for $2"
  expect '^browser: link target window shown$' "$3: the link target's window didn't show"
  send flags shift
  # The card can answer before the ⇧'s release is sent, so its waits read from the press on.
  shifted=$step
  expect "^browser: title $tab key Shift [0-9]*\$" "$3: the page didn't get the ⇧, which a shift-click needs"
  send flags
}

# The plate growing into the card, the motion's proof where the OS has a recorder.
record link-card 3
card "$page_link" "https://cards.test/page.html" "a page"
shift_step=$step
expect '^browser: link card page with picture$' "a page: ⇧ on its link showed no card with its picture" "$shifted"
recorded
grep -q "^/page.html " "$out/link-cards-public.log" || fail "a page: the server logged no request for the card's page"
grep -q "^/share.png " "$out/link-cards-public.log" || fail "a page: the server logged no request for its picture"
asked=$(grep -E '^/(page.html|share.png) ' "$out/link-cards-public.log")
! printf '%s\n' "$asked" | grep -q 'cookie=yes' ||
  fail "a page: the card's requests carried the page's cookie: $asked"
! printf '%s\n' "$asked" | grep -q 'referer=yes' ||
  fail "a page: the card's requests told the site the page the link was on: $asked"
! printf '%s\n' "$asked" | grep -q 'certificate=yes' ||
  fail "a page: the card's requests gave a client certificate: $asked"
../tools/window-screenshot.sh "$pid" "$out/link-card-page.png" > /dev/null 2>&1 || true
# Esc takes the card and the plate stays; the page gets the key after it, never the Esc.
send key escape
expect '^browser: link card closed$' "a page: Esc didn't take the card"
send key z
expect "^browser: title $tab key z [0-9]*\$" "a page: the key after Esc didn't reach the page"
! sed -n "/^browser: drive $shift_step: /,\$p" "$log" | grep -q "^browser: title $tab key Escape [0-9]*\$" ||
  fail "a page: the page got the Esc that took the card"
! sed -n "/^browser: drive $shift_step: /,\$p" "$log" | grep -q '^browser: link target hidden$' ||
  fail "a page: Esc took the plate with the card"

card "$private_link" "http://127.0.0.1:$private_port/direct" "a private address"
expect '^browser: link card failed$' "a private address: ⇧ on its link didn't show the card's refusal" "$shifted"
[ ! -s "$out/link-cards-private.log" ] ||
  fail "a private address: the private server got a card's request: $(tr '\n' ' ' < "$out/link-cards-private.log")"
../tools/window-screenshot.sh "$pid" "$out/link-card-private.png" > /dev/null 2>&1 || true

card "$hop_link" "https://cards.test/hop" "a redirect"
expect '^browser: link card failed$' "a redirect: ⇧ on its link didn't show the card's refusal" "$shifted"
grep -q '^/hop ' "$out/link-cards-public.log" || fail "a redirect: the public server logged no request for /hop"
[ ! -s "$out/link-cards-private.log" ] ||
  fail "a redirect: the private server got a card's request: $(tr '\n' ' ' < "$out/link-cards-private.log")"


card "$plain_link" "https://cards.test/plain.html" "a page with no picture"
expect '^browser: link card page$' \
  "a page with no picture: ⇧ on its link showed no card of its words" "$shifted"
../tools/window-screenshot.sh "$pid" "$out/link-card-plain.png" > /dev/null 2>&1 || true

card "$bare_link" "https://cards.test/bare.html" "a page that says nothing"
# With no title or description, the card says the page doesn't say what it is, as a refusal does.
expect '^browser: link card failed$' \
  "a page that says nothing: ⇧ on its link didn't show the card saying so" "$shifted"
../tools/window-screenshot.sh "$pid" "$out/link-card-bare.png" > /dev/null 2>&1 || true

# The slow page answers only once link-cards-slow.go is there, so the card waits, loading.
card "$slow_link" "https://cards.test/slow.html" "a slow page"
expect '^browser: link card loading$' \
  "a slow page: ⇧ on its link showed no loading card while its page hadn't come" "$shifted"
../tools/window-screenshot.sh "$pid" "$out/link-card-loading.png" > /dev/null 2>&1 || true
touch "$out/link-cards-slow.go"
expect '^browser: link card page$' "a slow page: its card didn't show the page once it came"

# The plate alone, once Esc took the card, and a click in its window's room above it, which the
# card grows into and nothing paints while the plate shows: the click reaches the page under it,
# as the window lets the pointer through. Where windows can't blend, the window is cut to the plate
# (#3096).
send key escape
expect '^browser: link card closed$' "a slow page: Esc didn't take the card"
../tools/window-screenshot.sh "$pid" "$out/link-card-plate.png" > /dev/null 2>&1 || true
at=$(grep '^browser: link target window at ' "$log" | tail -1 | sed 's/^browser: link target window at //')
[ -n "$at" ] || fail "the UI didn't say where the link target's window is"
room_x=$(printf '%s\n' "$at" | awk '{ printf "%d", $1 + 40 }')
room_y=$(printf '%s\n' "$at" | awk '{ printf "%d", $2 + 40 }')
send click "$room_x" "$room_y"
expect "^browser: title $tab pressed [0-9]*\$" \
  "a click at $room_x $room_y, in the link target's room above its plate (its window at $at), didn't reach the page under it"

# The cards in the dark, a page with its picture and one with none, captured as
# link-card-<case>-dark.png beside the light ones, then the profile's appearance back.
send menu "Appearance › Dark"
expect '^browser: appearance dark$' "View › Appearance › Dark didn't turn the UI dark"
for each in "page:$page_link:https://cards.test/page.html:page with picture" \
  "plain:$plain_link:https://cards.test/plain.html:page"; do
  name=${each%%:*} rest=${each#*:}
  point=${rest%%:*} rest=${rest#*:}
  url=${rest%:*} state=${rest##*:}
  card "$point" "$url" "dark, $name"
  expect "^browser: link card $state\$" "dark, $name: ⇧ on its link showed no card" "$shifted"
  ../tools/window-screenshot.sh "$pid" "$out/link-card-$name-dark.png" > /dev/null 2>&1 || true
  send key escape
  expect '^browser: link card closed$' "dark, $name: Esc didn't take the card"
done
send menu "Appearance › Light"
expect '^browser: appearance light$' "View › Appearance › Light didn't turn the UI light"

# How many requests the public server logged for the path $1.
asked() { grep -c "^$1 " "$out/link-cards-public.log" || true; }

# Off in Settings › Links, ⇧ on a link asks nothing: the app says so as the ⇧ comes, no card
# shows, and the server gets no request for the link's page, which the end of the run reads.
send menu Settings…
expect '^browser: settings shown ' "Settings… didn't show Settings"
settings_drew
send clickkey $(drawn "toolbar Links")
expect '^browser: settings tab Links$' "a click on Links didn't show its tab"
settings_drew Links
click_in_sight "switch link cards"
expect '^browser: link cards off$' "a click on Settings › Links' Link cards didn't turn it off"
../tools/window-screenshot.sh "$pid" "$out/link-cards-off.png" > /dev/null 2>&1 || true
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click 600 560
expect "^browser: title $tab pressed [0-9]*\$" "the cards' page didn't get the click back from Settings"
bare_asked=$(asked /bare.html)
off_step=$step
send move $bare_link
expect '^browser: link target cards.test/bare.html$' "Link cards off: resting on a link showed no link target"
expect '^browser: link target window shown$' "Link cards off: the link target's window didn't show"
send flags shift
off_shifted=$step
expect "^browser: title $tab key Shift [0-9]*\$" "Link cards off: the page didn't get the ⇧"
expect '^browser: link card not asked: off$' \
  "Link cards off: ⇧ on a link with its plate showing didn't say the cards are off" "$off_shifted"
send flags
! sed -n "/^browser: drive $off_step: /,\$p" "$log" | grep '^browser: link card \(loading\|page\|failed\)' ||
  fail "Link cards off: ⇧ on a link showed a card"
# Turned on again, ⇧ shows the card as before.
send menu Settings…
expect '^browser: settings shown ' "Settings… didn't show Settings"
settings_drew
if [ "$(sed -n 's/^browser: settings shown //p' "$log" | tail -1)" != Links ]; then
  send clickkey $(drawn "toolbar Links")
  expect '^browser: settings tab Links$' "a click on Links didn't show its tab"
  settings_drew Links
fi
click_in_sight "switch link cards"
expect '^browser: link cards on$' "a second click on Link cards' switch didn't turn it on"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click 600 560
expect "^browser: title $tab pressed [0-9]*\$" "the cards' page didn't get the click back from Settings"
card "$page_link" "https://cards.test/page.html" "Link cards on again"
expect '^browser: link card page with picture$' "Link cards on again: ⇧ on a link showed no card" "$shifted"
send key escape
expect '^browser: link card closed$' "Link cards on again: Esc didn't take the card"
# Off the link, as the user's pointer leaves it on its way to the sidebar, which a driven click on
# the Mac doesn't, so its plate isn't left over the agent's page.
send move 600 560
expect '^browser: link target none$' "Link cards on again: leaving the link kept its plate"

# An agent's tab with the same page, opened by an MCP client on the bridge.
case "$(uname -s)" in
  MINGW* | MSYS*) on='^browser: agents server on \\\\\.\\pipe\\.*-agents-[0-9a-f]*$' ;;
  *) on='^browser: agents server on /.*/Agents\.sock$' ;;
esac
await "$on" || fail "the saved Agents switch didn't start the agents server"
socket=$(grep '^browser: agents server on' "$log" | tail -1 | sed 's/^browser: agents server on //')
if windows; then bridge="$app/mcp-bridge.exe"; else bridge="$(dirname "$browser")/mcp-bridge"; fi
rm -f "$out/link-cards-in" "$out/link-cards-out"
mkfifo "$out/link-cards-in" "$out/link-cards-out"
if windows; then
  BROWSER_AGENT_TOKEN=$token windows_bridge "$bridge" "$socket" "$out/link-cards-in" \
    "$out/link-cards-out" 2> "$out/link-cards-bridge.log" &
else
  BROWSER_AGENT_TOKEN=$token "$bridge" "$socket" < "$out/link-cards-in" > "$out/link-cards-out" \
    2> "$out/link-cards-bridge.log" &
fi
exec 4> "$out/link-cards-in" 5< "$out/link-cards-out"
# Sends the JSON-RPC request $1 and prints the bridge's answer, a line read under perl's alarm, as
# mcp.sh's next_answer does, within 30 s.
answer() {
  printf '%s\n' "$1" >&4
  perl -e 'alarm shift; my $line = "";
    while (sysread STDIN, my $byte, 1) { $line .= $byte; last if $byte eq "\n" }
    print $line; exit($line =~ /\n\z/ ? 0 : 1)' 30 <&5 ||
    fail "the agents server didn't answer $1; the bridge said: $(cat "$out/link-cards-bridge.log")"
}
answer '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"check","version":"1"}}}' > /dev/null
printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}' >&4
opened=$(answer '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"open_tab","arguments":{"url":"https://cards.test/index.html"}}}')
agent_tab=$(printf '%s\n' "$opened" | sed -n 's/.*Opened tab \([0-9]*\)\..*/\1/p')
[ -n "$agent_tab" ] || fail "the agent's open_tab answered: $opened"
await "^browser: title $agent_tab ready\$" || fail "the agent's tab didn't draw the page with the links"
# Its card, then its conversation's Pages, then its page's picture, which watches it in place.
await '^browser: session session-[0-9]* of agent mcp-client as check$' ||
  fail "the agent's open_tab made no card"
session=$(sed -n 's/^browser: session \(session-[0-9]*\) of agent mcp-client as check$/\1/p' "$log" | tail -1)
await "^browser: task plate task-${session#session-} at " || fail "the agent's card wasn't drawn"
read -r px py pw ph <<PLATE
$(grep "^browser: task plate task-${session#session-} at " "$log" | tail -1 | cut -d' ' -f6-)
PLATE
send click $((px + pw / 2)) $((py + ph / 2))
expect "^browser: task $session shown\$" "a click on the agent's card didn't show its conversation"
await "^browser: task picture $agent_tab at " || fail "the agent's conversation didn't picture its tab"
read -r wx wy ww wh <<PICTURE
$(grep "^browser: task picture $agent_tab at " "$log" | tail -1 | cut -d' ' -f6-)
PICTURE
send click $((wx + ww / 2)) $((wy + wh / 2))
expect "^browser: task $session page $agent_tab\$" "a click on the agent's page's picture didn't watch it"
expect "^browser: task page $agent_tab in place\$" "the agent's page didn't come to rest in place"
# A click in it gives it the keyboard, taking it over where the press reaches the host; the page
# stays the agent's Space's either way. Where the watched page is its tile, the press that takes it
# over goes no further (#4115): the page is the window's once it's in place again, and the click
# after that is the page's.
if grep "^browser: tile $agent_tab \(shown\|ended\)\$" "$log" | tail -1 | grep -q ' shown$'; then
  send click 600 520
  expect "^browser: agent page pressed in $agent_tab\$" "a click on the agent's page's tile wasn't a press in it"
  expect "^browser: agent mcp-client taken over in $agent_tab\$" "a click on the agent's page's tile didn't take it over"
  expect "^browser: task page $agent_tab in place\$" "the page the user took over didn't come to rest in the window"
fi
send click 600 520
expect "^browser: title $agent_tab pressed [0-9]*\$" "the agent's page didn't get the click"
plain_asked=$(asked /plain.html)
agent_step=$step
send move $plain_link
expect '^browser: link target cards.test/plain.html$' \
  "an agent's page: resting on its link didn't report the link"
send flags shift
expect "^browser: title $agent_tab key Shift [0-9]*\$" "an agent's page: the page didn't get the ⇧"
send flags
! sed -n "/^browser: drive $agent_step: /,\$p" "$log" | grep '^browser: link card \(loading\|page\|failed\)' ||
  fail "an agent's page: ⇧ on a link showed a card"
../tools/window-screenshot.sh "$pid" "$out/link-cards-agent.png" > /dev/null 2>&1 || true

exec 4>&- 5<&-
exec 3>&-
quit
forget_appearance
# With the user's link left before, any plate placed since is the agent's page's, read whole now.
! sed -n "/^browser: drive $agent_step: /,\$p" "$log" | grep '^browser: link target window at ' ||
  fail "an agent's page: its link got a plate, which only the window's own tab's links get"
# Long after either ⇧, neither link's page was asked.
[ "$(asked /bare.html)" = "$bare_asked" ] ||
  fail "Link cards off: the server got a request for the link's page: $(grep '^/bare.html ' "$out/link-cards-public.log")"
[ "$(asked /plain.html)" = "$plain_asked" ] ||
  fail "an agent's page: the server got a request for the link's page: $(grep '^/plain.html ' "$out/link-cards-public.log")"
echo "link cards: a page's card with its picture, asked without cookie, referrer or certificate; a private address and a redirect to one refused unasked; a page with no picture, one that says nothing, and one loading, light and dark; a click in the plate's room reaching the page; and none asked off in Settings › Links, back on turned on, or in an agent's Space, its page watched and clicked in"
