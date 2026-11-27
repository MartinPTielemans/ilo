# check.sh runs this at 170.
# Resting on a sidebar tab's row shows its hover card beside the rows (#430): a picture of a tab
# not shown, taken once the switches that hid it are quiet and never on a switch, its title and
# host, and its buttons. The tab shown has no picture, as its page is beside the card. The next row
# swaps its card in at once, and leaving the rows, scrolling them or a key lets it go. Its buttons
# each make their call on the first click (#818). It comes in and goes out in Dia's motion, which
# card-motion.mp4 records. The card takes the Space's look, dark and light. Over a hidden sidebar
# out, the card counts as the sidebar's (#830). Reports how long the switches take, beside the tabs
# run's. A row of a pinned GitHub tab's pull requests opens in that tab, shown, ⌘-click behind
# and ⌥-click beside the page (#3576), on the run's own GitHub.

# A page of colour, so its picture shows in the card.
page='data:text/html,<title>loading</title>
<body style="margin:0;height:100vh;background:linear-gradient(135deg,%23e1676f,%237eb6d5)">
<h1 style="font:bold 64px system-ui;color:white;margin:40px">Colours</h1>
<script>requestAnimationFrame(() => setTimeout(() => document.title = "colours"));</script>'
serve
log="$out/cards.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* colours$' || fail "the cards test page didn't draw"
await '^browser: today [0-9][0-9]*$' || fail "the first tab isn't in Today"
a=$(today)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: cards. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
open_from_bar "http://localhost:$port/one.html"
b=$tab
expect_tabs "$b $a" "$b" "after opening one.html"
# The page hidden is pictured once the switch is quiet, drawn unseen for it, never the one shown.
expect "^browser: hover card \(not \)\{0,1\}pictured $a\$" \
  "the core never answered for the tab hidden once the switch was quiet"
none_since "^browser: hover card not pictured $a\$" \
  || fail "the core's picture of the tab hidden came back empty once the switch was quiet"
none_since "^browser: hover card pictured $b\$" || fail "the tab shown was pictured"

# Resting on the hidden tab's row, its card shows with its picture beside the rows, where the UI
# put it, and is drawn before it's on screen. The system's memory-pressure signal drops every card
# picture, so one between the picture and the rest shows the favicon until the core pictures the
# tab again (#3886); without one, the picture taken as the switch went quiet is there at once.
send move $(row 1)
expect "^browser: hover card shown $a pin favorite\( split\)\{0,1\} \(picture\|favicon\)\$" \
  "resting on the hidden tab's row didn't show its card"
dropped=$(awk -v tab="$a" -v move="browser: drive $step: " '
  $0 == "browser: hover card pictured " tab { dropped = 0 }
  /^browser: hover card pictures dropped [1-9]/ { dropped = 1 }
  index($0, move) == 1 { moved = 1 }
  moved && index($0, "browser: hover card shown " tab " ") == 1 { print dropped + 0, $NF; exit }
' "$log")
[ "$dropped" != "0 favicon" ] || fail "the hidden tab's card showed its favicon though no memory \
pressure dropped the picture taken as the switch went quiet: $(grep -n \
  -e "^browser: hover card pictured $a\$" -e '^browser: hover card pictures dropped' \
  -e "^browser: hover card shown $a " "$log" | tail -4 | tr '\n' ' ')"
expect "^browser: hover card drawn $a picture\$" "the hidden tab's card wasn't drawn with its picture"
expect '^browser: hover card window shown$' "the hidden tab's card didn't come on screen"
expect '^browser: hover card settled$' "the hidden tab's card didn't come to rest"
asked=$(grep '^browser: hover card at ' "$log" | tail -1)
placed=$(grep '^browser: hover card window at ' "$log" | tail -1)
echo "$asked $placed" | awk '
  { across = $5 - $14; down = $6 - $15 }
  END { exit !(NR == 1 && NF == 15 && across > -1 && across < 1 && down > -1 && down < 1 && $5 > 200) }' \
  || fail "the card isn't beside the rows where the UI put it: asked \"$asked\", placed \"$placed\""
../tools/window-screenshot.sh "$pid" "$out/card-dark.png" || fail "no window to capture"
# The next row, the tab shown, swaps its card in at once, with no picture. Its Split pairs it with
# the tab shown before it, as the sidebar's split does.
send move $(row 0)
expect "^browser: hover card shown $b pin favorite\( split\)\{0,1\} none\$" "the tab shown's card has a picture"
none_since '^browser: hover card hidden$' || fail "the card went between rows instead of swapping"
../tools/window-screenshot.sh "$pid" "$out/card-shown-tab.png" || fail "no window to capture"
# Leaving the rows lets it go, and so does a scroll over them.
send move $page_point
expect '^browser: hover card hidden$' "leaving the rows didn't hide the card"
send move $(row 1)
expect "^browser: hover card shown $a " "resting on the hidden tab's row again didn't show its card"
send scroll $(row 1) -5 1
expect '^browser: hover card hidden$' "a scroll over the rows didn't hide the card"
send move $page_point
# A key ends it too, as in Dia, here one the page has.
send move $(row 1)
expect "^browser: hover card shown $a " "resting on the hidden tab's row a third time didn't show its card"
expect '^browser: hover card settled$' "the hidden tab's card didn't come to rest before the key"
send key x
expect '^browser: hover card hidden$' "a key typed in the page didn't hide the card"
send move $page_point
# The card coming in and going out, recorded for its motion: it fades in over 100 ms as it grows
# from 0.92 on a spring, and fades out as it springs back (docs/ux-spec.md › Motion › Hover card).
record card-motion 3
send move $(row 1)
expect "^browser: hover card shown $a " "resting on the hidden tab's row didn't show its card for the recording"
expect '^browser: hover card settled$' "the recorded card didn't come to rest"
send move $page_point
expect '^browser: hover card hidden$' "leaving the rows didn't hide the recorded card"
recorded

# A light theme for the Space: the theme panel's first palette, then Light. The card takes the
# Space's look.
post rightclick 100 112
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors, ")" "the Space's menu didn't offer its theme"
send choose "$(cased 'Edit Theme Color…')"
expect '^browser: theme picker shown$' "Edit Theme Color… didn't show the theme picker"
expect '^browser: theme picker drawn$' "the theme picker didn't draw itself on screen"
send clickkey $(panel_at palette 1)
expect '^browser: colored space [0-9]* [0-9a-f]*$' "the first palette didn't colour the Space"
send clickkey $(panel_at look light)
expect '^browser: themed space [0-9]* .*appearance=light' "Light didn't make the Space's theme light"
send key escape
expect '^browser: theme picker hidden$' "escape didn't close the theme picker"
send move $(row 1)
expect "^browser: hover card shown $a .* picture\$" "the hidden tab's card didn't show in the light Space"
expect '^browser: hover card window shown$' "the hidden tab's card didn't come on screen in the light Space"
expect '^browser: hover card settled$' "the hidden tab's card didn't come to rest in the light Space"
../tools/window-screenshot.sh "$pid" "$out/card-light.png" || fail "no window to capture"
send move $page_point
expect '^browser: hover card hidden$' "leaving the rows didn't hide the light card"

# Switches back and forth, each timed, then the one hidden last is pictured and the one shown
# isn't asked for. A picture asked for while it was hidden can still come after the last switch:
# a hidden page draws unseen for it, a few hundred ms on Windows, and switches come faster (#3904).
for _ in 1 2 3 4 5; do
  switch "2 cmd" "$a"
  switch "1 cmd" "$b"
done
expect "^browser: hover card pictured $a\$" "the tab hidden last wasn't pictured after the switches"
none_since "^browser: tab $b picturing " || fail "the tab shown was asked for its picture after the \
switches: $(since | grep "^browser: tab [0-9]* picturing \|^browser: hover card pictured " | tr '\n' ' ')"

# With the sidebar hidden, a row's card is the sidebar's own (#830): the pointer going from the row
# onto it, past the hold zone, leaves the sidebar out, and leaving both sends the sidebar away and
# the card with it. Onto the card at its near edge first, as a hand crosses it.
send key s cmd
expect '^browser: sidebar hidden$' "⌘S didn't hide the sidebar for the card over the sidebar out"
send move 400 300
send move 0 300
send move 0 306
expect '^browser: reveal shown$' "the pointer at the window's edge didn't bring the sidebar out"
expect '^browser: reveal window shown$' "the sidebar out didn't come on screen"
send move $(row 1)
expect "^browser: hover card shown $a " "resting on a row of the sidebar out didn't show its card"
expect '^browser: hover card settled$' "the card over the sidebar out didn't come to rest"
# The card's frame in whole points, as it can be 266.5 tall and the shell's arithmetic is whole.
read -r cl ct cw ch <<EOF
$(grep '^browser: hover card at ' "$log" | tail -1 | cut -d' ' -f5- |
  awk '{ for (i = 1; i <= 4; i++) printf "%d ", int($i + 0.5); print "" }')
EOF
mark=$(wc -l < "$log")
send move $((cl + 6)) $((ct + ch / 2))
send move $((cl + cw / 2)) $((ct + ch / 2))
send move $((cl + cw / 2 + 10)) $((ct + ch / 2))
# The capture outlasts the card's wait after its row is left, so a card that didn't count the
# pointer has gone by the look.
../tools/window-screenshot.sh "$pid" "$out/card-reveal.png" || fail "no window to capture"
tail -n +"$((mark + 1))" "$log" | grep -q '^browser: reveal hidden$' &&
  fail "the pointer going from its row onto the card sent the hidden sidebar away"
tail -n +"$((mark + 1))" "$log" | grep -q '^browser: hover card hidden$' &&
  fail "the pointer going from its row onto the card over the sidebar out let the card go"
send move $page_point
expect '^browser: reveal hidden$' "the pointer off the card and the sidebar out didn't send it away"
expect '^browser: hover card hidden$' "the card stayed after the sidebar out went away"
../tools/window-screenshot.sh "$pid" "$out/card-reveal-away.png" || fail "no window to capture"
send key s cmd
expect '^browser: sidebar shown$' "⌘S didn't bring the sidebar back after the card over it"

# The card's buttons each make their call on the first click, as a hand comes onto the card from
# its row (#818). card_button <name>: the centre of that button on the card shown last, from where
# the UI put the card and the buttons it logged, right to left from its bottom right corner.
card_button() {
  buttons=$(grep '^browser: hover card shown ' "$log" | tail -1 | cut -d' ' -f6-)
  frame=$(grep '^browser: hover card at ' "$log" | tail -1 | cut -d' ' -f5-)
  echo "$buttons|$frame" | awk -F'|' -v want="$1" '{
    n = split($1, b, " ") - 1; split($2, f, " ")
    for (i = 1; i <= n && b[i] != want; i++);
    if (i > n) exit 1
    printf "%d %d\n", f[1] + f[3] - 10 - (n - i) * 30 - 14, f[2] + f[4] - 10 - 14 }'
}
# Rests on the row at $1 $2 till tab $3's card is at rest with the button $4, then clicks it as a
# hand would: onto the card, which lets the card's window take the mouse, then on it again.
click_card_button() {
  send move "$1" "$2"
  expect "^browser: hover card shown $3 \(.* \)\{0,1\}$4 " \
    "tab $3's card didn't show with its $4 button"
  expect '^browser: hover card settled$' "tab $3's card didn't come to rest before its $4 button"
  at=$(card_button "$4") || fail "tab $3's card has no $4 button"
  ../tools/window-screenshot.sh "$pid" "$out/card-$4-before.png" || fail "no window to capture"
  send move $at
  send move $at
  send click $at
  expect "^browser: hover card $4 $3\$" "a click on the card's $4 button didn't reach it"
  expect "^browser: command $4 via hover card\$" "the card's $4 button didn't make its call"
  expect '^browser: hover card hidden$' "the card didn't go after its $4 button"
}
# Pin moves the row up into the pinned tabs, and its card then offers Unpin, which puts it back.
click_card_button $(row 1) "$a" pin
expect_lists "$a" "$b" "after the card's Pin"
../tools/window-screenshot.sh "$pid" "$out/card-pin-after.png" || fail "no window to capture"
click_card_button $(pinned_row 0) "$a" unpin
expect "^browser: pinned \$" "the card's Unpin left the pinned tabs \"$(pinned)\""
../tools/window-screenshot.sh "$pid" "$out/card-unpin-after.png" || fail "no window to capture"
# Favorite makes it a tile, whose card offers Remove from Favorites, which puts it back.
send move $page_point
at=$(today | tr ' ' '\n' | grep -nx "$a" | cut -d: -f1)
click_card_button $(row $((at - 1))) "$a" favorite
expect "^browser: favorites $a\$" "the card's Favorite didn't make tab $a a favorite"
../tools/window-screenshot.sh "$pid" "$out/card-favorite-after.png" || fail "no window to capture"
click_card_button 114 110 "$a" unfavorite
expect "^browser: favorites \$" "the card's Remove from Favorites left a favorite"
../tools/window-screenshot.sh "$pid" "$out/card-unfavorite-after.png" || fail "no window to capture"
# Split puts the hidden tab beside the one shown.
send move $page_point
at=$(today | tr ' ' '\n' | grep -nx "$a" | cut -d: -f1)
click_card_button $(row $((at - 1))) "$a" split
expect "^browser: splits \(.*[ +]\)\{0,1\}$a\([+ ].*\)\{0,1\}\$" \
  "the card's Split didn't put tab $a in a split view"
../tools/window-screenshot.sh "$pid" "$out/card-split-after.png" || fail "no window to capture"
exec 3>&-
quit

app_ms=$(sed -n 's/^browser: selected [0-9]* in \([0-9.]*\) ms$/\1/p' "$log" | summary)
echo "ok: resting on a tab's row shows its card beside the rows, with a picture of its page once it's hidden and the switch quiet, and none for the tab shown; the next row swaps in at once, leaving the rows, a scroll or a key typed in the page lets it go, it comes and goes in Dia's motion, the card takes a light Space's look, a hidden sidebar out stays out while the pointer is on a row's card and goes with it once the pointer leaves both, and its Pin, Unpin, Favorite, Remove from Favorites and Split each make their call on the first click."
echo "  Switch, key to the core's report, with cards pictured: $app_ms. Windows: $out/card-dark.png, $out/card-shown-tab.png, $out/card-light.png, $out/card-reveal.png, $out/card-reveal-away.png, and $out/card-<button>-before.png and -after.png for each button. Motion: $out/card-motion.mp4"

# A row of what's recent on a pinned tab's card opens in the pinned tab itself, which leaves its
# pinned URL and is shown with its page focused (#3576), whether it hasn't been shown since
# launch, as most pinned tabs haven't after a relaunch, or has. Back to Pinned URL takes it home.
# ⌘-click opens the row in a tab behind and ⌥-click beside the page, each keeping the tab shown. GitHub is the run's own server
# over https: its pages, and its search answering signed in with three pull requests asking for
# review and none of the user's own, in the shape the core reads (core/live_folder_rule.cc).
MSYS2_ARG_CONV_EXCL=/CN= openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=github.com \
  -addext subjectAltName=DNS:github.com -keyout "$out/cards-tls.key" \
  -out "$out/cards-tls.pem" 2> /dev/null || fail "openssl didn't make GitHub's certificate"
spki=$(openssl x509 -in "$out/cards-tls.pem" -pubkey -noout | openssl pkey -pubin -outform der |
  openssl dgst -sha256 -binary | base64)
expr "$spki" : '[A-Za-z0-9+/]\{43\}=$' > /dev/null ||
  fail "openssl gave \"$spki\" as GitHub's certificate's key, not a SHA-256 in base64"
rm -f "$out/cards-tls.port"
mkfifo "$out/cards-tls.port"
python3 -I -u -c '
import http.server, json, socketserver, ssl, sys
def row(number):
    return {"number": number, "hl_title": "Pull request %d" % number, "state": "open",
            "repo": {"repository": {"owner_login": "ilo-check", "name": "cards"}}}
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        print("github: GET %s" % self.path, file=sys.stderr, flush=True)
        if self.path.startswith("/search?"):
            rows = [row(n) for n in (7, 8, 9)] if "review-requested" in self.path else []
            search = {"logged_in": True, "results": rows}
            body = json.dumps({"payload": {"blackbirdSearchRoute": search}}).encode()
            kind = "application/json"
        else:
            title = "GitHub" if self.path == "/" else self.path.strip("/").replace("/", " ")
            body = ("<!doctype html><title>%s</title><h1>%s</h1>" % (title, title)).encode()
            kind = "text/html; charset=utf-8"
        self.send_response(200)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, *args):
        pass
# TCPServer, as site.sh binds: HTTPServer looks 127.0.0.1 up by name, 35 s on the hosted Mac.
class Server(socketserver.ThreadingTCPServer):
    # A handshake the browser ends, as for a certificate it refuses, is logged (#2673).
    def get_request(self):
        try:
            return super().get_request()
        except OSError as error:
            print("handshake failed: %s" % error, file=sys.stderr, flush=True)
            raise
server = Server(("127.0.0.1", 0), Handler)
tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
tls.load_cert_chain(sys.argv[1], sys.argv[2])
server.socket = tls.wrap_socket(server.socket, server_side=True)
# As bytes: text stdout on Windows ends a line with \r, which the driver typed as return (#1196).
sys.stdout.buffer.write(b"%d\n" % server.server_address[1])
sys.stdout.close()
server.serve_forever()
' "$out/cards-tls.pem" "$out/cards-tls.key" > "$out/cards-tls.port" 2> "$out/cards-tls.log" \
  3>&- 4>&- 5>&- &
tls=$!
read -r tls_port < "$out/cards-tls.port" || true
[ -n "$tls_port" ] || fail "GitHub's https server didn't start: $(cat "$out/cards-tls.log")"
pull=https://github.com/ilo-check/cards/pull

# recent_row <tab> <n> [modifiers…]: rests on pinned tab <tab>'s row till its card has its rows,
# then clicks row <n>, counting from 0, as a hand would, onto the card first. Under the box's
# padding, one heading, the review requests', is over the rows. A card shown again with the rows
# kept says so as it shows, and draws no new line, as what it holds hasn't changed.
recent_row() {
  local tab=$1 n=$2
  shift 2
  send move $page_point
  send move $(pinned_row 0)
  expect "^browser: hover card \(drawn\|shown\) $tab\( .*\)\{0,1\} recent-rows\$" \
    "the pinned tab's card didn't show its pull requests"
  expect '^browser: hover card settled$' "the pinned tab's card didn't come to rest"
  at=$(grep '^browser: hover card at ' "$log" | tail -1 | cut -d' ' -f5- |
    awk -v n="$n" '{ printf "%d %d\n", $1 + $3 / 2, $2 + 4 + 22 + 44 * n + 22 }')
  ../tools/window-screenshot.sh "$pid" "$out/card-recent-$n.png" || fail "no window to capture"
  send move $at
  send move $at
  send click $at "$@"
}
# followed_from_row <n> <what>: row <n> loaded in the pinned tab, which is the tab shown once it
# has, away from its pinned URL, its page with the keyboard. Then ⌘↩ takes it back to its pinned
# URL.
followed_from_row() {
  local clicked=$step shown
  expect "^browser: command recent-here via hover card\$" "a click on row $1 of $2's card didn't open it"
  expect "^browser: loaded $p $pull/$(($1 + 7))\$" \
    "a click on row $1 of $2's card didn't load it in the pinned tab"
  expect "^browser: page $p away 1\$" \
    "the pinned tab at row $1's address didn't read as away from its pinned URL"
  none_since "^browser: tab [0-9]* opened by $p" "$clicked" ||
    fail "a click on row $1 of $2's card opened a tab of its own"
  # The tab selected last since the click, as the app could select another after it.
  shown=$(since "$clicked" | sed -n 's/^browser: selected \([0-9]*\).*/\1/p' | tail -1)
  [ "$shown" = "$p" ] ||
    fail "a click on row $1 of $2's card loaded it in tab $p, but tab ${shown:-none} is shown"
  send move $page_point
  in_browser RenderWidgetHostViewCocoa "after a click on row $1 of $2's card"
  send key return cmd
  expect '^browser: command back-to-pinned-url via page$' "⌘↩ after row $1 didn't run Back to Pinned URL"
  expect "^browser: loaded $p https://github.com/\$" "⌘↩ after row $1 didn't load the pinned URL"
  expect "^browser: page $p away 0\$" "the pinned tab still read as away at its pinned URL"
}
# GitHub's page pinned, and another shown: a row of the pinned tab, shown since launch. Then
# launched again, so the pinned tab comes back unloaded and never shown.
log="$out/cards-recent.log"
step=0
rm -f "$saved"
BROWSER_DRIVE=1 BROWSER_URL=https://github.com/ "$browser" \
  --host-resolver-rules="MAP github.com 127.0.0.1:$tls_port" \
  --ignore-certificate-errors-spki-list="$spki" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* GitHub$' ||
  fail "GitHub's page from the run's server didn't draw: $(cat "$out/cards-tls.log")"
p=$(today)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  kill "$tls" 2>/dev/null || true
  echo "skip: a pinned tab's recent rows. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
send key d cmd
expect "^browser: pinned $p\$" "⌘D didn't pin GitHub's tab"
open_from_bar "http://localhost:$port/one.html"
b=$tab
recent_row "$p" 1
followed_from_row 1 "the pinned tab shown since launch"
switch "2 cmd" "$b"
quit_by_key
await '^browser: command quit via ' || fail "⌘Q didn't reach the UI"
exited
log="$out/cards-recent-restored.log"
step=0
BROWSER_DRIVE=1 "$browser" \
  --host-resolver-rules="MAP github.com 127.0.0.1:$tls_port" \
  --ignore-certificate-errors-spki-list="$spki" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch didn't restore the Space"
# A relaunch gives the restored tabs new numbers.
await '^browser: pinned [0-9][0-9]*$' || fail "GitHub's tab didn't come back pinned"
await '^browser: today [0-9][0-9]*$' || fail "one.html's tab didn't come back"
p=$(pinned)
b=$(today)
await "^browser: selected $b\$" || fail "the relaunch didn't show one.html's tab"
await "^browser: loaded $b http://localhost:$port/one.html\$" || fail "one.html didn't load"
await "^browser: page $p unloaded 1\$" || fail "the pinned tab didn't come back unloaded"
send click $page_point

recent_row "$p" 0
followed_from_row 0 "the pinned tab never shown since launch"
# ⌘-click: a tab behind, with the tab shown kept.
switch "2 cmd" "$b"
recent_row "$p" 2 cmd
expect '^browser: command recent-behind via hover card$' "⌘-click on a row of the card didn't open it behind"
# A tab opened behind loads once it's shown (open_behind), so it's read at the row's address.
expect "^browser: page [0-9]* back 0 forward 0 loading 0 $pull/9\$" \
  "⌘-click on a row of the card didn't open it in a tab"
behind=$(grep "^browser: page [0-9]* back 0 forward 0 loading 0 $pull/9\$" "$log" | tail -1 | cut -d' ' -f3)
[ "$behind" != "$p" ] || fail "⌘-click on a row of the card opened it in the pinned tab, not a tab behind"
none_since "^browser: selected $p" || fail "⌘-click on a row of the card showed the pinned tab"
none_since "^browser: selected $behind\$" || fail "⌘-click on a row of the card showed the tab it opened"
# ⌥-click: beside the page, with the tab shown kept.
recent_row "$p" 0 alt
expect '^browser: command recent-beside via hover card$' "⌥-click on a row of the card didn't open it beside the page"
expect "^browser: splits \(.*[ +]\)\{0,1\}$b\([+ ].*\)\{0,1\}\$" "⌥-click on a row of the card didn't open it beside one.html"
none_since "^browser: selected $p" || fail "⌥-click on a row of the card showed the pinned tab"
../tools/window-screenshot.sh "$pid" "$out/card-recent-beside.png" || fail "no window to capture"
exec 3>&-
quit
kill "$tls" 2>/dev/null || true
tls=
echo "ok: a click on a row of a pinned tab's pull requests opens it in the pinned tab, shown and away from its pinned URL with its page focused, whether the pinned tab was shown since launch or not, and Back to Pinned URL takes it home; ⌘-click opens it behind and ⌥-click beside the page, keeping the tab shown. Windows: $out/card-recent-<row>.png, $out/card-recent-beside.png"
