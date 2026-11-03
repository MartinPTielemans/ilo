# A tab in a call says so on its row, as Chrome's tab strip does: the camera while its page uses the
# camera, the microphone alone, or the screen while it shares a screen, a window or a tab, and the
# tab it shares shows the screen too. While it shares a tab, another tab's menu has Share This Tab
# Instead, which moves the share there. The marks go once the call ends. A call floats in Picture in
# Picture while its tab is hidden, and comes back with it, unless its Site Controls block that, or
# Settings turns it off for the sites they don't allow. A tab playing music out of sight has a
# player at the sidebar's foot, whose buttons pause it and float it in Picture in Picture, whose
# click shows the tab, and whose × puts it away. A private window's music has a player in that
# window alone. View › Cast… and the page menu's Cast… open the tab's Cast card, and × closes it.
# A video's page menu floats it in Picture in Picture, the Mini Player, which a scroll or a drag
# moves, ⌘ or Ctrl with a scroll sizes, and a double-click puts back in its tab, shown.
# Chromium's fake camera and microphone stand in for real ones, and the share takes the tab titled
# Shared without its picker; the question for the camera is answered with Allow, as the user
# would, since the fake question Chromium can put in its place never tells the browser what's
# captured.

serve
# "c" calls with the camera and microphone, "s" shares the tab titled Shared, "e" ends both, "o"
# opens that tab, the same page with ?Shared, whose title is what follows the ?, "p" one titled
# Other, and "m" plays ten seconds of silence over and over, as music titled Quiet with a next
# track: long enough for Chrome to give it controls, and no sound. Asked to float in Picture in
# Picture through its media session, it opens a floating window, which closes as its tab shows.
printf '%s' '<title>loading</title><body style="background:#333">
<script>
const name = decodeURIComponent(location.search.slice(1));
requestAnimationFrame(() => setTimeout(() => document.title = name || "ready"));
let streams = [];
onkeydown = async e => {
  try {
    if (e.key == "c") {
      streams.push(await navigator.mediaDevices.getUserMedia({video: true, audio: true}));
      document.title = "in a call";
    } else if (e.key == "s") {
      streams.push(await navigator.mediaDevices.getDisplayMedia({video: true}));
      document.title = "sharing";
    } else if (e.key == "e") {
      streams.forEach(stream => stream.getTracks().forEach(track => track.stop()));
      streams = [];
      document.title = "ended";
    } else if (e.key == "m") {
      const rate = 8000, n = rate * 10, wav = new DataView(new ArrayBuffer(44 + n));
      const text = (at, s) => [...s].forEach((c, i) => wav.setUint8(at + i, c.charCodeAt(0)));
      text(0, "RIFF"); wav.setUint32(4, 36 + n, true); text(8, "WAVEfmt ");
      wav.setUint32(16, 16, true); wav.setUint16(20, 1, true); wav.setUint16(22, 1, true);
      wav.setUint32(24, rate, true); wav.setUint32(28, rate, true); wav.setUint16(32, 1, true);
      wav.setUint16(34, 8, true); text(36, "data"); wav.setUint32(40, n, true);
      for (let i = 0; i < n; i++) {
        wav.setUint8(44 + i, 128);
      }
      const music = new Audio(URL.createObjectURL(new Blob([wav], {type: "audio/wav"})));
      music.loop = true;
      music.onplaying = () => document.title = "playing";
      music.onpause = () => document.title = "paused";
      navigator.mediaSession.metadata = new MediaMetadata({title: "Quiet", artist: "Check"});
      navigator.mediaSession.setActionHandler("nexttrack", () => document.title = "next track");
      navigator.mediaSession.setActionHandler("enterpictureinpicture", async () => {
        const floating = await documentPictureInPicture.requestWindow({width: 320, height: 180});
        floating.onpagehide = () => document.title = "landed";
        document.onvisibilitychange = () => document.hidden || floating.close();
        document.title = "floating";
      });
      await music.play();
    } else if (e.key == "o" || e.key == "p") {
      window.open(e.key == "o" ? "call.html?Shared" : "call.html?Other");
    }
  } catch (error) {
    document.title = "refused " + error.name;
  }
};
</script>' > "$out/site/call.html"
# The same pages over https, for the floating call: Chrome floats a call on https or a file alone,
# and its page info, the Site Controls, lists no permissions for a file. The certificate is made
# for this run, and the browser takes it by its key alone, so no other site's errors are let by.
# Git Bash would pass the subject to Windows' openssl as a path under its own folder (#872).
MSYS2_ARG_CONV_EXCL=/CN= openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=127.0.0.1 \
  -addext subjectAltName=IP:127.0.0.1 -keyout "$out/tls.key" -out "$out/tls.pem" 2> /dev/null ||
  fail "openssl didn't make the floating call's certificate"
spki=$(openssl x509 -in "$out/tls.pem" -pubkey -noout | openssl pkey -pubin -outform der |
  openssl dgst -sha256 -binary | base64)
rm -f "$out/tls.port"
mkfifo "$out/tls.port"
python3 -I -u -c '
import functools, http.server, ssl, sys
handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=sys.argv[1])
server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
tls.load_cert_chain(sys.argv[2], sys.argv[3])
server.socket = tls.wrap_socket(server.socket, server_side=True)
# As bytes: text stdout on Windows ends a line with \r, which the driver typed as return (#1196).
sys.stdout.buffer.write(b"%d\n" % server.server_address[1])
sys.stdout.close()
server.serve_forever()
' "$out/site" "$out/tls.pem" "$out/tls.key" > "$out/tls.port" 2> "$out/tls.log" 3>&- 4>&- 5>&- &
tls=$!
# Its port, once it's listening, or nothing if it ended first.
read -r tls_port < "$out/tls.port" || true
[ -n "$tls_port" ] || fail "the https server didn't start: $(cat "$out/tls.log")"
secure="https://127.0.0.1:$tls_port/"
log="$out/media.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}call.html" "$browser" --use-fake-device-for-media-stream \
  --auto-select-tab-capture-source-by-title=Shared --ignore-certificate-errors-spki-list="$spki" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the call page didn't draw"
a=$(today | cut -d' ' -f1)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: media. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi

# The tab the call shares and the one it shares instead, opened from it, then the call's tab
# shown again. Each opens under it, so the call's tab is found by its place in Today.
# shown <tab>: shows <tab> by its ⌘ number. shown_again <tab>: and gives its page the keyboard.
shown() {
  n=$(today | tr ' ' '\n' | grep -nx "$1" | cut -d: -f1)
  switch "$((n + $(pinned | wc -w))) cmd" "$1"
}
shown_again() {
  shown "$1"
  send click $page_point
}
send key o
expect "^browser: today \($a [0-9][0-9]*\|[0-9][0-9]* $a\)\$" \
  "the page didn't open the tab to share"
b=$(today | tr ' ' '\n' | grep -vx "$a")
expect "^browser: title $b Shared\$" "the tab to share didn't draw"
shown_again "$a"
send key p
expect "^browser: today .* .* .*\$" "the page didn't open the tab to share instead"
c=$(today | tr ' ' '\n' | grep -vx "$a" | grep -vx "$b")
expect "^browser: title $c Other\$" "the tab to share instead didn't draw"
shown_again "$a"

send key c
expect "^browser: page $a asks ${site%/} to " "the call didn't ask for the camera and microphone"
expect "^browser: question $a at " "the call's question wasn't drawn"
expect "^browser: question $a takes input\$" "the call's question never took input"
send layout
read -r qx qy qw qh <<EOF
$(grep "^browser: question $a at " "$log" | tail -1 | cut -d' ' -f5-)
EOF
send click $((qx + qw * 3 / 4)) $((qy + qh - 26))
expect "^browser: title $a in a call\$" "Allow didn't give the call the camera and microphone"
expect "^browser: page $a camera 1 microphone 1 sharing 0 shared 0\$" \
  "the call's tab didn't show it uses the camera"
../tools/window-screenshot.sh "$pid" "$out/media-call.png" || fail "no window to capture"

send click $page_point
send key s
expect "^browser: title $a sharing\$" "the call didn't share the tab titled Shared"
expect "^browser: page $a camera 1 microphone 1 sharing 1 shared 0\$" \
  "the call's tab didn't show it shares"
expect "^browser: page $b camera 0 microphone 0 sharing 0 shared 1\$" \
  "the tab shared didn't show it's shared"
expect "^browser: page $c share instead 1\$" "the other tab wasn't offered to share instead"
# Chrome shows the tab a page starts sharing, unless the page asks it not to, once, up to a second
# later, and whichever tab is shared by then. So it does before the share moves, or it shows the
# other tab under the call's "e" (#1166).
expect "^browser: selected $b\( in .*\)\{0,1\}\$" "Chrome didn't show the tab shared"
../tools/window-screenshot.sh "$pid" "$out/media-sharing.png" || fail "no window to capture"

# The other tab's menu, from its row, moves the share there.
post rightclick $(row "$(($(today | tr ' ' '\n' | grep -nx "$c" | cut -d: -f1) - 1))")
expect "^browser: menu shown .*$(cased 'Share This Tab Instead')" \
  "the other tab's menu didn't offer Share This Tab Instead"
send choose "$(cased 'Share This Tab Instead')"
expect "^browser: page $c camera 0 microphone 0 sharing 0 shared 1\$" \
  "Share This Tab Instead didn't share the other tab"
expect "^browser: page $b camera 0 microphone 0 sharing 0 shared 0\$" \
  "Share This Tab Instead left the first tab shared"

# The call's tab is shown again for its "e".
shown "$a"
send click $page_point
send key e
expect "^browser: page $a camera 0 microphone 0 sharing 0 shared 0\$" \
  "the call's tab kept its mark once the call ended"
expect "^browser: page $c camera 0 microphone 0 sharing 0 shared 0\$" \
  "the tab shared kept its mark once the share ended"

# A call floats once its tab is hidden, as Arc's does: Chrome's automatic Picture in Picture for a
# page using the camera that asks for it with its media session, on https or a file alone. So a
# page from the https server, from the command bar, whose "c" calls and which says when Chrome
# asks it to float, when its window floats and lands or why it was refused, and whose "k" puts how
# often it was asked to float in its title. Showing its tab again brings the call back.
printf '%s' '<title>loading</title><body style="background:#333">
<script>
requestAnimationFrame(() => setTimeout(() => document.title = "ready"));
let asked = 0;
navigator.mediaSession.setActionHandler("enterpictureinpicture", async () => {
  asked++;
  document.title = "asked to float";
  try {
    const floating = await documentPictureInPicture.requestWindow({width: 320, height: 180});
    document.title = "floating";
    floating.addEventListener("pagehide", () => document.title = "landed");
  } catch (error) {
    document.title = "not floating: " + error.name;
  }
});
onkeydown = async e => {
  if (e.key == "c") {
    window.call = await navigator.mediaDevices.getUserMedia({video: true, audio: true});
    document.title = "in a call";
  } else if (e.key == "e") {
    call.getTracks().forEach(track => track.stop());
    document.title = "ended";
  } else if (e.key == "k") {
    document.title = "asked " + asked;
  }
};
</script>' > "$out/site/float.html"
send key t cmd
expect '^browser: command bar shown$' "the command bar didn't open for the floating call"
send type "${secure}float.html"
send key return
expect "^browser: loaded [0-9]* ${secure}float.html\$" "the floating call's page didn't load"
f=$(grep "^browser: loaded [0-9]* ${secure}float.html$" "$log" | tail -1 | cut -d' ' -f3)
expect "^browser: title $f ready\$" "the floating call's page didn't draw"
send click $page_point
send key c
expect "^browser: question $f takes input\$" "the floating call didn't ask for the camera"
send layout
read -r qx qy qw qh <<EOF
$(grep "^browser: question $f at " "$log" | tail -1 | cut -d' ' -f5-)
EOF
send click $((qx + qw * 3 / 4)) $((qy + qh - 26))
expect "^browser: title $f in a call\$" "Allow didn't give the floating call the camera"
shown "$b"
expect "^browser: title $f floating\$" "the call didn't float with its tab hidden"
# It lands as its tab is shown, so before the click that gives its page the keyboard.
shown "$f"
expect "^browser: title $f landed\$" "the floating call didn't come back with its tab"
send click $page_point

# The call's Site Controls list the Automatic picture-in-picture it asked for. Blocked there, the
# call stays in its tab as the tab hides, and allowed, it floats again. Back at Ask, Settings'
# switch turns it off for every site the user hasn't allowed, and on again. The panel hangs from
# the URL field's button at 206 64, its first row's middle at y 262, the rows 43 apart; Safari's
# engine's row comes before the permissions.
# floats <value> [capture]: sets the call's Automatic picture-in-picture to <value> in its Site
# Controls, capturing them first as <capture>.
floats() {
  # Out first, as a move back within the tools' fade out draws them shown again with no new line.
  away
  send move $sidebar_point
  expect '^browser: sidebar tools shown$' "the URL field's buttons didn't fade in"
  send click 206 64
  expect '^browser: site controls .*Automatic picture-in-picture [A-Z]' \
    "the floating call's Site Controls didn't list Automatic picture-in-picture"
  expect '^browser: site controls settled$' "the floating call's Site Controls didn't open"
  [ -z "${2:-}" ] || ../tools/window-screenshot.sh "$pid" "$out/$2" || fail "no window to capture"
  line=$(grep '^browser: site controls .*Automatic picture-in-picture ' "$log" | tail -1)
  i=$(echo "$line" | sed 's/^[^;]*; //;s/;.*//' | tr ',' '\n' | sed 's/^ //' |
    grep -n '^Automatic picture-in-picture ' | cut -d: -f1)
  case "$line" in *"; Safari's engine "*) i=$((i + 1)) ;; esac
  post click 207 $((262 + 43 * (i - 1)))
  expect "^browser: menu shown .*$1" "Automatic picture-in-picture's menu didn't offer $1"
  send choose "$1"
  expect "^browser: site controls set Automatic picture-in-picture $1\$" "$1 in the menu didn't run"
  send key escape
  expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
  send click $page_point
}
# mini_player: Settings' Advanced switch for it, which says it went <on or off>.
mini_player() {
  send key , cmd
  expect '^browser: settings shown ' "⌘, from the page didn't show Settings"
  # A click before its first frame finds no tabs to land on.
  expect '^browser: settings drew ' "Settings didn't draw"
  # Settings opens at the tab it last showed.
  if ! grep '^browser: settings shown ' "$log" | tail -1 | grep -q ' Advanced$'; then
    send clickkey $(drawn "toolbar Advanced")
    expect '^browser: settings tab Advanced$' "a click on Advanced didn't show its tab"
    settings_drew Advanced
  fi
  send clickkey 592 208
  expect "^browser: mini player $1\$" "a click on the Mini Player's switch didn't turn it $1"
  [ "$1" = on ] || ../tools/window-screenshot.sh "$pid" "$out/settings-mini-player.png" ||
    fail "no window to capture"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
}
floats Block site-controls-pip.png
shown "$b"
shown_again "$f"
send key k
expect "^browser: title $f asked 1\$" "blocked in its Site Controls, the call still floated"
floats Allow
shown "$b"
expect "^browser: title $f floating\$" "allowed in its Site Controls, the call didn't float"
shown "$f"
expect "^browser: title $f landed\$" "the call allowed to float didn't come back with its tab"
send click $page_point
floats Ask
mini_player off
shown "$b"
shown_again "$f"
send key k
expect "^browser: title $f asked 2\$" "with the Mini Player off in Settings, the call still floated"
send click $page_point
mini_player on
shown "$b"
expect "^browser: title $f floating\$" "with the Mini Player on again, the call didn't float"
shown "$f"
expect "^browser: title $f landed\$" "the call floating again didn't come back with its tab"
send click $page_point
send key e
expect "^browser: title $f ended\$" "e didn't end the floating call"
shown_again "$a"

# Music in the call's tab, then another tab shown: the tab playing out of sight has a player.
send key m
expect "^browser: title $a playing\$" "m in the page didn't play its silence"
expect "^browser: page $a media 1 playing 1 next 1 pip 1 Quiet\$" "the page's music had no controls"
none_since "^browser: media players [0-9]" || fail "the tab shown had a player"
shown "$b"
expect "^browser: media players $a\$" "the tab playing out of sight had no player"
expect "^browser: media player $a at " "the player wasn't drawn"
read -r mx my mw mh <<EOF
$(grep "^browser: media player $a at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
../tools/window-screenshot.sh "$pid" "$out/media-player.png" || fail "no window to capture"
# Its buttons from the right, 8 in, 24 wide and 6 apart: ×, the next track, pause, then Picture in
# Picture, as Arc's Back to PiP, there while the page can float and doesn't.
button_x() { echo $((mx + mw - 8 - 12 - $1 * 30)); }
send click "$(button_x 2)" $((my + mh / 2))
expect '^browser: command media-pause via player$' "the player's pause didn't take the click"
expect "^browser: title $a paused\$" "the player's pause didn't pause the page"
expect "^browser: page $a media 1 playing 0 next 1 pip 1 Quiet\$" "the player didn't show it paused"
send click "$(button_x 3)" $((my + mh / 2))
expect '^browser: command media-picture-in-picture via player$' \
  "the player's Picture in Picture didn't take the click"
expect "^browser: title $a floating\$" "the player's Picture in Picture didn't float the page"
expect "^browser: page $a media 1 playing 0 next 1 pip 0 Quiet\$" \
  "the player kept Picture in Picture while the page floated"
send click $((mx + 60)) $((my + mh / 2))
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "a click on the player didn't show its tab"
expect "^browser: title $a landed\$" "the floating page didn't land as its tab showed"
expect '^browser: media players $' "the tab shown kept its player"
shown "$b"
expect "^browser: media players $a\$" "the paused tab out of sight had no player"
send click "$(button_x 0)" $((my + mh / 2))
expect '^browser: command media-dismiss via player$' "the player's × didn't take the click"
expect '^browser: media players $' "× didn't put the player away"

# A private window's music out of sight has a player there alone. Every window hears every tab,
# but the regular window's players are its Spaces' tabs, so its music, playing again, is its one.
# The private window logs a Today of its own, so the regular one's keys are taken first.
key_a=$(($(today | tr ' ' '\n' | grep -nx "$a" | cut -d: -f1) + $(pinned | wc -w)))
key_b=$(($(today | tr ' ' '\n' | grep -nx "$b" | cut -d: -f1) + $(pinned | wc -w)))
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
private=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
expect "^browser: window opened $private\$" "the private window didn't open"
send window 2
# private_page <name>: opens the call page named <name> in the private window, as $p.
private_page() {
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar in the private window"
  send insert "${site}call.html?$1"
  send key return
  expect "^browser: title [0-9]* $1\$" "the private window's $1 page didn't draw"
  p=$(sed -n "s/^browser: title \([0-9]*\) $1\$/\1/p" "$log" | tail -1)
}
private_page Private
music=$p
send click $page_point
send key m
expect "^browser: title $music playing\$" "m in the private page didn't play its silence"
played=$step
# Its player comes as ⌘T's new tab hides the page, before the next page is shown.
private_page Hidden
players=$(sed -n "/^browser: drive $played: /,\$p" "$log" | grep '^browser: media players' |
  tail -1)
[ "$players" = "browser: media players $music" ] ||
  fail "the private window's music out of sight had no player alone: ${players:-none}"
expect "^browser: media player $music at " "the private window's player wasn't drawn" "$played"
../tools/window-screenshot.sh "$pid" "$out/media-player-private.png" || fail "no window to capture"
send window 1
switch "$key_a cmd" "$a"
send click $page_point
send key m
expect "^browser: title $a playing\$" "m in the page didn't play its silence again"
switch "$key_b cmd" "$b"
expect "^browser: media players $a\$" "the regular window's players weren't its music alone"
expect "^browser: media player $a at " "the regular window's player wasn't drawn again"
../tools/window-screenshot.sh "$pid" "$out/media-player-own.png" || fail "no window to capture"
send click "$(button_x 2)" $((my + mh / 2))
expect "^browser: title $a paused\$" "the player's pause didn't pause the page"
send click "$(button_x 0)" $((my + mh / 2))
expect '^browser: media players $' "× didn't put the player away"
send window 2
send key w cmd shift
expect "^browser: window closed $private\$" "⇧⌘W didn't close the private window"
send window 1

# The Cast card, from View › Cast… and from the page menu, with whatever devices the network has,
# none clicked. Linux alone: the Mac asks for the local network on the screen as Chrome looks with
# DIAL, and Windows' firewall may.
if [ "$(uname)" = Linux ]; then
  send menu "Cast…"
  expect "^browser: cast $b .* devices [0-9]*\$" "View › Cast… didn't open the tab's Cast dialog"
  expect "^browser: cast $b at " "the Cast card wasn't drawn"
  read -r kx ky kw kh <<EOF
$(grep "^browser: cast $b at " "$log" | tail -1 | cut -d' ' -f5-)
EOF
  ../tools/window-screenshot.sh "$pid" "$out/media-cast.png" || fail "no window to capture"
  # Its ×, 7 inside its top right corner and 22 square.
  send click $((kx + kw - 18)) $((ky + 18))
  expect '^browser: command cast-close via card$' "the Cast card's × didn't take the click"
  expect "^browser: cast $b closed\$" "× didn't close the Cast dialog"
  post rightclick $page_point
  expect '^browser: menu shown .*Cast…' "the page's menu had no Cast…"
  send choose "Cast…"
  expect "^browser: cast $b .* devices [0-9]*\$" "the page menu's Cast… didn't open the Cast dialog"
  send menu "Cast…"
  expect "^browser: cast $b closed\$" "View › Cast… again didn't close the Cast dialog"
else
  echo "skip: the Cast card on $(uname). Chrome looking for devices would ask for the network."
fi

# A video's page menu has Chrome's Picture in Picture, which floats the video; "x" in the page
# brings it back. The video plays a canvas's colours, with no sound.
printf '%s' '<title>loading</title><body style="margin:0;background:#333">
<video id=v muted autoplay style="width:100vw;height:100vh"></video>
<script>
const canvas = Object.assign(document.createElement("canvas"), {width: 320, height: 180});
const paint = canvas.getContext("2d");
const frame = time => {
  paint.fillStyle = `hsl(${time / 20 % 360} 50% 40%)`;
  paint.fillRect(0, 0, 320, 180);
  requestAnimationFrame(frame);
};
requestAnimationFrame(frame);
v.srcObject = canvas.captureStream();
v.onplaying = () => document.title = "playing";
v.onenterpictureinpicture = () => document.title = "floating";
v.onleavepictureinpicture = () => document.title = "landed";
onkeydown = e => e.key == "x" && document.exitPictureInPicture();
</script>' > "$out/site/video.html"
send key t cmd
expect '^browser: command bar shown$' "the command bar didn't open for the video"
send type "${site}video.html"
send key return
expect "^browser: loaded [0-9]* ${site}video.html\$" "the video's page didn't load"
v=$(grep "^browser: loaded [0-9]* ${site}video.html\$" "$log" | tail -1 | cut -d' ' -f3)
expect "^browser: title $v playing\$" "the video didn't play"
post rightclick $page_point
expect "^browser: menu shown .*$(cased 'Picture in Picture')" \
  "the video's menu had no Picture in Picture"
../tools/window-screenshot.sh "$pid" "$out/video-menu.png" || fail "no window to capture"
send choose "$(cased 'Picture in Picture')"
expect "^browser: title $v floating\$" "the video menu's Picture in Picture didn't float the video"

# The floating video, the Mini Player, moves with a scroll or a drag, and ⌘-scroll, Ctrl-scroll
# off the Mac, sizes it about its middle. A double-click on it, with another tab shown, shows its
# tab and lands it. Linux alone: drive.mm doesn't find the Mini Player yet.
# mini_player_at <condition> <failure>: waits for the Mini Player logged since the last command at
# a frame, x, y, w and h, for which awk's <condition> holds of the last px, py, pw and ph, then
# makes it the last.
mini_player_at() {
  for _ in $(seq 100); do
    at=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep '^browser: pip at ' | tail -1 |
      cut -d' ' -f4-)
    if [ -n "$at" ] && echo "$at" | awk -v px="$px" -v py="$py" -v pw="$pw" -v ph="$ph" \
      "{ x = \$1; y = \$2; w = \$3; h = \$4; exit !($1) }"; then
      read -r px py pw ph <<EOF
$at
EOF
      return 0
    fi
    sleep 0.1
  done
  fail "$2"
}
if [ "$(uname)" = Linux ]; then
  px=0 py=0 pw=0 ph=0
  send pip
  mini_player_at 1 "the floating video had no Mini Player"
  # Two notches down move it up as far as the content would go, 120 a notch.
  send pip wheel $((pw / 2)) $((ph / 3)) 2
  mini_player_at "x == px && y == py - 240 && w == pw && h == ph" \
    "a scroll over the Mini Player didn't move it with the content"
  # Off its controls, left of the 10 seconds back by more than its width, and away from the
  # screen's right edge, which would push it in as it grows.
  # The window manager moves it with the pointer, as by a title bar, from when the core asks it,
  # past the drag's threshold, and tells it where it is at the release. The core's lines carry
  # Chromium's prefix.
  send pip press $((pw / 8)) $((ph / 2))
  expect 'browser: mini player pressed at ' "a press off its controls didn't reach the Mini Player"
  send pip dragto $((pw / 8 - 10)) $((ph / 2))
  expect 'browser: mini player moving from ' "a drag didn't start moving the Mini Player"
  send pip dragto $((pw / 8 - 200)) $((ph / 2))
  send pip release $((pw / 8 - 200)) $((ph / 2))
  mini_player_at "x == px - 200 && y == py && w == pw && h == ph" \
    "a drag didn't move the Mini Player"
  send pip wheel $((pw / 2)) $((ph / 3)) -2 cmd
  mini_player_at "w > pw * 1.15 && h > ph * 1.15 && (2 * x + w - 2 * px - pw) ^ 2 <= 16 &&
    (2 * y + h - 2 * py - ph) ^ 2 <= 16" "Ctrl with a scroll didn't grow the Mini Player in place"
  ../tools/window-screenshot.sh "$pid" "$out/mini-player.png" || fail "no window to capture"
  # By its row: ⌘ and a number in the page whose video floats shows no tab on Linux yet.
  send click $(row $(($(today | tr ' ' '\n' | grep -nx "$b" | cut -d: -f1) - 1)))
  expect "^browser: selected $b in " "a click on its row didn't show tab $b of \"$(today)\""
  send pip doubleclick $((pw / 2)) $((ph / 4))
  expect "^browser: selected $v\( in .*\)\{0,1\}\$" \
    "a double-click on the Mini Player didn't show its tab"
  expect "^browser: title $v landed\$" "a double-click on the Mini Player didn't land it"
  expect '^browser: pip gone$' "the Mini Player stayed after a double-click"
else
  echo "skip: moving, sizing and double-clicking the Mini Player on $(uname): drive.mm has no pip."
  send click $page_point
  send key x
  expect "^browser: title $v landed\$" "the floating video didn't come back"
fi
exec 3>&-
quit

echo "ok: a tab in a call marks its row with the camera, then the screen while it shares a tab, the tab it shares shows the screen too, another tab's Share This Tab Instead moves the share there, and the marks go when the call ends; a call floats while its tab is hidden, unless its Site Controls or Settings turn that off; music playing out of sight has a player that pauses it, floats it in Picture in Picture, shows its tab and goes on ×, and a private window's music has one there alone; the Cast card opens from View and the page menu and closes on ×; a video's menu floats it in Picture in Picture, where a scroll or a drag moves it, Ctrl with a scroll sizes it and a double-click shows its tab."
