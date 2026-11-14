# check.sh runs this at 260.
# A tab in a call says so on its row, as Chrome's tab strip does: the camera while its page uses the
# camera, the microphone alone, or the screen while it shares a screen, a window or a tab, and the
# tab it shares shows the screen too. While it shares a tab, another tab's menu has Share This Tab
# Instead, which moves the share there. The marks go once the call ends. A call floats in Picture in
# Picture while its tab is hidden, and comes back with it, unless its Site Controls block that, or
# Settings turns it off for the sites they don't allow; on X11, and on the Mac in CI, its window
# moves by its top bar and sizes from its corner, and on Windows its hit test says a press there
# would. A tab playing music out of sight has a player at the sidebar's foot, whose buttons pause
# it, skip back and float it in Picture in Picture, whose volume drawer's slider sets the tab's
# volume, whose click shows the tab, and whose × puts it away. A private window's music has a
# player in that window alone. A video's player shows it live while the stack is open, playing or
# paused. View › Cast… and the page menu's Cast… open the tab's Cast card, and × closes it; on the
# Mac the run answers macOS's question for the local network that looking for devices brings, and
# any it asks as the call starts. A video's page menu floats it in Picture in Picture, the Mini
# Player, 10 in from the top right of the screen, which a scroll or a drag moves, ⌘ or Ctrl with a
# scroll sizes, and a double-click puts back in its tab, shown. On the Mac a throw settles it 10 in
# at a corner, and a slow release near a side 10 in along it. The next shows where it was left,
# after a relaunch too, and coming back to its tab's window from another window, or from another
# app with a menu open, puts it back in the tab.
# A page asks to share the screen only from a click or a key: asked with neither, its promise is
# refused, and nothing is shared (decision 12).
# Chromium's fake camera and microphone stand in for real ones, and the share takes the tab titled
# Shared without its picker; the question for the camera is answered with Allow, as the user would,
# since the fake question Chromium can put in its place never tells the browser what's captured.

serve

# On the Mac, macOS's own question, which UserNotificationCenter shows over the app with its
# activation: the user answers it once, and a check VM or a hosted runner, new each time, is asked
# in the run. If the app isn't the active one within <tenths> of a second, the run says what was
# asked, from check/alert.mm, which never says the rest, answers Allow as the user would, and has
# the app's activation back. Usage: answer_macos <tenths> <when>.
answer_macos() {
  [ "$(uname)" = Darwin ] || return 0
  for _ in $(seq "$1"); do
    send state
    [ "$active" = 1 ] || break
    sleep 0.1
  done
  [ "$active" = 0 ] || return 0
  [ -x "$out/alert" ] ||
    clang++ -fobjc-arc -framework Cocoa -o "$out/alert" check/alert.mm 2> "$out/alert-build.log" ||
    fail "check/alert.mm didn't build: $(cat "$out/alert-build.log")"
  # The job on GitHub's hosted Mac answers the local network's question as it comes
  # (check-mac.yml), and ilo has its activation back.
  asked=$("$out/alert" 2>&1) || {
    send state
    [ "$active" = 1 ] && return 0
    fail "ilo lost its activation $2, to no question: $asked"
  }
  echo "macOS asked $2, and the run answers Allow: $asked"
  "$out/alert" "" Allow > "$out/alert.txt" 2>&1 || true
  for _ in $(seq 50); do
    send state
    [ "$active" = 1 ] && break
    sleep 0.1
  done
  [ "$active" = 1 ] ||
    fail "ilo wasn't the active app after macOS asked $2: $asked; $(cat "$out/alert.txt")"
}

# "c" calls with the camera and microphone, "s" shares the tab titled Shared, "u" asks to share it
# once the key's activation has lapsed, as a page asking unasked would, "e" ends both, "o"
# opens that tab, the same page with ?Shared, whose title is what follows the ?, "p" one titled
# Other, and "m" plays ten seconds of silence over and over, as music titled Quiet with a previous
# and a next track: long enough for Chrome to give it controls, and no sound. Asked to float in Picture in
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
    } else if (e.key == "u") {
      while (navigator.userActivation.isActive) {
        await new Promise(next => setTimeout(next, 100));
      }
      streams.push(await navigator.mediaDevices.getDisplayMedia({video: true}));
      document.title = "shared unasked";
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
      // Kept by the window: a paused element nothing refers to is garbage, and its player goes
      // with it, so Chrome ends the session, and the sidebar loses its player.
      const music = window.music = new Audio(URL.createObjectURL(new Blob([wav], {type: "audio/wav"})));
      music.loop = true;
      music.onplaying = () => document.title = "playing";
      music.onpause = () => document.title = "paused";
      navigator.mediaSession.metadata = new MediaMetadata({title: "Quiet", artist: "Check"});
      navigator.mediaSession.setActionHandler("previoustrack", () => document.title = "previous track");
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
import functools, http.server, socketserver, ssl, sys
handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=sys.argv[1])
# TCPServer, as site.sh binds: HTTPServer looks 127.0.0.1 up by name, 35 s on the hosted Mac.
server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), handler)
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
# Now and then a hosted Mac asks as the call starts (#968), before the share's key.
answer_macos 20 "as the call started"

send click $page_point
send key u
expect "^browser: title $a refused InvalidStateError\$" \
  "a share asked for with no click or key wasn't refused"
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
# other tab under the call's "e" (#1166). It decides as the share starts, and only while the window
# sharing is the active one. On the Mac the dialog for what to share, closed at once under the
# check's choice, has the keyboard until it's gone, so a window that has it back only after the
# share started keeps the tab where it is, as Chrome's own would (#2148).
shared_at=$step
for _ in $(seq 100); do
  logged "^browser: selected $b\( in .*\)\{0,1\}\$" "$shared_at" && break
  exited_without "^browser: selected $b" "Chrome didn't show the tab shared" "$shared_at"
  sleep 0.1
done
if ! logged "^browser: selected $b\( in .*\)\{0,1\}\$" "$shared_at"; then
  # The window's keyboard and the share's start, in the order they came.
  started="page $a camera 1 microphone 1 sharing 1 shared 0"
  order=$(sed -n "/^browser: drive $shared_at: /,\$p" "$log" |
    grep -E "^browser: (window (resigned|became) key|$started)\$" | tr '\n' ';')
  case "$order" in
    *"$started;"*"window became key;"*|*"window resigned key;browser: $started;")
      echo "skip: Chrome showing the tab shared: the window had the keyboard back from the" \
        "share's dialog only after the share started, so Chrome found it inactive (#2148):" \
        "$order"
      # Shown as Chrome would have, so the rest goes as it does then.
      shown "$b" ;;
    *) fail "Chrome didn't show the tab shared: $order" ;;
  esac
fi
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

# mini_player_at <condition> <failure>: waits for the Picture in Picture window logged since the
# last command at a frame, x, y, w and h, for which awk's <condition> holds of the last px, py, pw
# and ph, then makes it the last.
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
# pip_driven: whether the driver finds a Picture in Picture window, which it does through X11
# alone: drive.mm has no pip, and under Wayland no client sees another's windows.
pip_driven() {
  [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]
}
# kept_keys [what]: waits for the browser window to have the keyboard once <what> has floated, the
# call as its tab hid if none, as it keeps it on the Mac and X11. Wayland's compositor gives the
# floating window the keyboard as it maps, and the core gives it back (#1961, #2060), in a line
# with Chromium's prefix.
kept_keys() {
  floated_what=${1:-the call floating as its tab hid}
  if [ "$(uname)" = Linux ] && [ -n "${WAYLAND_DISPLAY:-}" ]; then
    expect 'browser: mini player gave the keyboard back$' \
      "$floated_what didn't give the browser window the keyboard back"
  fi
  for _ in $(seq 100); do
    send state
    [ "$key" = BrowserCoreNSWindow ] && return 0
    sleep 0.1
  done
  fail "$floated_what kept the keyboard in $key"
}
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
kept_keys
# It lands as its tab is shown, so before the click that gives its page the keyboard.
shown "$f"
expect "^browser: title $f landed\$" "the floating call didn't come back with its tab"
send click $page_point

# The call's Site Controls list the Automatic picture-in-picture it asked for. Blocked there, the
# call stays in its tab as the tab hides, and allowed, it floats again. Back at Ask, Settings'
# switch turns it off for every site the user hasn't allowed, and on again. The panel hangs from
# the URL field's button at 206 64, and logs where it drew each line.
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
  click_placed post "line Automatic picture-in-picture"
  expect "^browser: menu shown .*$1" "Automatic picture-in-picture's menu didn't offer $1"
  send choose "$1"
  expect "^browser: site controls set Automatic picture-in-picture $1\$" "$1 in the menu didn't run"
  send key escape
  expect '^browser: site controls hidden$' "escape didn't close the Site Controls"
  send click $page_point
}
# mini_player: Settings' Features switch for it, which says it went <on or off>.
mini_player() {
  send key , cmd
  expect '^browser: settings shown ' "⌘, from the page didn't show Settings"
  # A click before its first frame finds no tabs to land on.
  expect '^browser: settings drew ' "Settings didn't draw"
  # Settings opens at the section it last showed.
  if ! grep '^browser: settings shown ' "$log" | tail -1 | grep -q ' Features$'; then
    send clickkey $(drawn "toolbar Features")
    expect '^browser: settings tab Features$' "a click on Features didn't show its section"
    settings_drew Features
  fi
  click_in_sight "switch mini player"
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
kept_keys
# Allowed, it floats without Chrome's prompt, which on X11 is a window of its own over the
# call's bottom right corner while it shows (#1263).
# The call's window moves and sizes as Chrome's does, by its frame, so the page keeps its own
# scroll, pinch and double-click: a drag on its top bar, off the buttons at its ends, moves it, and
# one from its bottom right corner, in the 10 its edges keep for sizing, sizes it. The window
# manager does both, with the display's pointer. The driver through X11 alone, as pip_driven says;
# Windows answers with its hit test below, and a Mac in CI drags it with check/pointer.mm.
if pip_driven; then
  px=0 py=0 pw=0 ph=0
  send pip
  mini_player_at 1 "the floating call had no window"
  # Up and left, as it floats in the screen's bottom right: the window manager moves it with the
  # pointer from where Chromium hands the drag over, at the press or past the drag's threshold.
  send pip press $((pw / 2)) 17
  send pip dragto $((pw / 2 - 10)) 17
  send pip dragto $((pw / 2 - 150)) -23
  send pip release $((pw / 2 - 150)) -23
  mini_player_at "x >= px - 150 && x <= px - 140 && y == py - 40 && w == pw && h == ph" \
    "a drag on the floating call's top bar didn't move it"
  send pip press $((pw - 3)) $((ph - 3))
  send pip dragto $((pw + 7)) $((ph + 7))
  send pip dragto $((pw + 57)) $((ph + 37))
  send pip release $((pw + 57)) $((ph + 37))
  mini_player_at "x == px && y == py && w >= pw + 50 && w <= pw + 60 && h >= ph + 30 &&
    h <= ph + 40" "a drag from the floating call's corner didn't size it"
  ../tools/window-screenshot.sh "$pid" "$out/media-call-floating.png" all ||
    fail "no window to capture"
  # A press on the call's window gives it the keys, as Chrome's does, so a click gives the
  # browser window them back for the tab switch below.
  send click $page_point
elif windows; then
  # Windows moves and sizes a window itself, with the desktop's pointer, from what the window's
  # hit test says is under it, so the driver, which never moves that pointer, asks the hit test.
  px=0 py=0 pw=0 ph=0
  send pip
  mini_player_at 1 "the floating call had no window"
  send pip hit $((pw / 2)) 17
  expect '^browser: pip hit caption$' "the floating call's top bar wasn't one a press moves it by"
  send pip hit $((pw - 3)) $((ph - 3))
  expect '^browser: pip hit bottomright$' \
    "the floating call's bottom right corner wasn't one a press sizes it from"
  ../tools/window-screenshot.sh "$pid" "$out/media-call-floating.png" all ||
    fail "no window to capture"
elif [ "$(uname)" = Darwin ] && [ "${GITHUB_ACTIONS:-}" = true ]; then
  # The window server moves and sizes it with the real pointer, which the driver's events never
  # move, so check/pointer.mm drags it with events posted as a mouse's are. They move the
  # desktop's pointer, so only in a CI job, where nobody's work is on the screen.
  clang++ -fobjc-arc -framework Cocoa -o "$out/pointer" check/pointer.mm 2> "$out/pointer.log" ||
    fail "check/pointer.mm didn't build: $(cat "$out/pointer.log")"
  # The window's frame once it matches $1, as the window server settles it after the let-go.
  pip_is() {
    for _ in $(seq 30); do
      send pip
      at=$(grep '^browser: pip at ' "$log" | tail -1 | cut -d' ' -f4-)
      if [ -n "$at" ] && echo "$at" | awk -v px="$px" -v py="$py" -v pw="$pw" -v ph="$ph" \
        "{ x = \$1; y = \$2; w = \$3; h = \$4; exit !($1) }"; then
        read -r px py pw ph <<EOF
$at
EOF
        return 0
      fi
      sleep 0.1
    done
    fail "$2, at $at: $(cat "$out/pointer.txt")"
  }
  px=0 py=0 pw=0 ph=0
  pip_is 1 "the floating call had no window"
  "$out/pointer" drag $((px + pw / 2)) $((py + 17)) $((px + pw / 2 - 150)) $((py - 23)) \
    > "$out/pointer.txt" 2>&1 || fail "check/pointer.mm didn't drag: $(cat "$out/pointer.txt")"
  pip_is "x == px - 150 && y == py - 40 && w == pw && h == ph" \
    "a drag on the floating call's top bar didn't move it"
  "$out/pointer" drag $((px + pw - 3)) $((py + ph - 3)) $((px + pw + 47)) $((py + ph + 27)) \
    > "$out/pointer.txt" 2>&1 || fail "check/pointer.mm didn't drag: $(cat "$out/pointer.txt")"
  pip_is "x == px && y == py && w >= pw + 45 && w <= pw + 55 && h >= ph + 25 && h <= ph + 35" \
    "a drag from the floating call's corner didn't size it"
  ../tools/window-screenshot.sh "$pid" "$out/media-call-floating.png" all ||
    fail "no window to capture"
  # The press gave the call's window the keys, as on Linux.
  send click $page_point
else
  if [ "$(uname)" = Darwin ]; then
    echo "skip: moving and sizing the floating call on Darwin outside CI: it takes the real" \
      "pointer."
  else
    echo "skip: moving and sizing the floating call on $(uname)${WAYLAND_DISPLAY:+ Wayland}:" \
      "the driver finds no Picture in Picture window there."
  fi
fi
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
kept_keys
shown "$f"
expect "^browser: title $f landed\$" "the call floating again didn't come back with its tab"
send click $page_point
send key e
expect "^browser: title $f ended\$" "e didn't end the floating call"
shown_again "$a"

# Music in the call's tab, then another tab shown: the tab playing out of sight has a player.
send key m
expect "^browser: title $a playing\$" "m in the page didn't play its silence"
expect "^browser: page $a media 1 playing 1 previous 1 next 1 pip 1 Quiet\$" \
  "the page's music had no controls"
none_since "^browser: media players [0-9]" || fail "the tab shown had a player"
shown "$b"
expect "^browser: media players $a\$" "the tab playing out of sight had no player"
# player_at <height> <failure> [<step>]: waits for the front card drawn <height> tall, 33 at rest
# and 66 open, 99 with its volume drawer, since the last command or <step>, and reads where it is
# into mx, my, mw and mh.
player_at() {
  expect "^browser: media player $a at [0-9-]* [0-9-]* [0-9]* $1\$" "$2" "${3:-}"
  read -r mx my mw mh <<EOF
$(grep "^browser: media player $a at [0-9-]* [0-9-]* [0-9]* $1\$" "$log" | tail -1 | cut -d' ' -f6-)
EOF
}
# open_stack: rests the pointer on the front card, which opens the stack.
open_stack() {
  send move $((mx + mw / 2)) $((my + mh - 16))
  expect '^browser: media stack open$' "the stack didn't open under the pointer"
  player_at '6[56]' "the open stack's front card wasn't 65.5 tall"
}
player_at 33 "the player wasn't drawn 33 tall at rest"
expect '^browser: media stack rest$' "the stack didn't rest"
../tools/window-screenshot.sh "$pid" "$out/media-player.png" || fail "no window to capture"
# At rest its controls are fifths of the card: the favicon, the previous track, pause, the next
# track and the volume.
send click $((mx + mw / 2)) $((my + mh / 2))
expect '^browser: command media-pause via player$' "the player's pause didn't take the click"
expect "^browser: title $a paused\$" "the player's pause didn't pause the page"
expect "^browser: page $a media 1 playing 0 previous 1 next 1 pip 1 Quiet\$" \
  "the player didn't show it paused"
# The pointer resting there opens it: the title row's buttons from the right, 4.5 in, 24 wide and
# 2 apart: ×, then Picture in Picture, as Arc's Back to PiP, there while the page can float and
# doesn't.
expect '^browser: media stack open$' "the stack didn't open under the pointer"
player_at '6[56]' "the open stack's front card wasn't 65.5 tall"
../tools/window-screenshot.sh "$pid" "$out/media-player-open.png" || fail "no window to capture"
# control_x <n>: the middle of the controls' <n>th fifth, from 0; control_y their row's, 33 tall at
# the card's foot.
control_x() { echo $((mx + mw * (2 * $1 + 1) / 10)); }
control_y=$((my + mh - 17))
send click "$(control_x 1)" $control_y
expect '^browser: command media-previous via player$' "the player's previous didn't take the click"
expect "^browser: title $a previous track\$" "the player's previous didn't skip back"
# Adjust Tab Volume opens the drawer, which grows the card 33 up from its foot, with a slider whose
# track the log has. A press a quarter along it sets the tab's volume there, which the core reports
# back, and applies to the page's media as it plays again: a paused page's ducking is the audio
# focus manager's to undo.
send click "$(control_x 4)" $control_y
expect '^browser: command media-volume via player$' "the player's volume didn't take the click"
expect "^browser: media volume drawer $a open\$" "the volume drawer didn't open"
player_at '9[89]' "the volume drawer didn't grow the card to 98.5"
read -r vx vy vw vh <<EOF
$(grep "^browser: media volume $a at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
[ -n "$vh" ] || fail "the volume drawer's slider wasn't drawn"
send click $((vx + vw / 4)) $((vy + vh / 2))
expect '^browser: command media-volume 0\.2[3-7] via player$' "the slider didn't take the press"
volume=$(grep '^browser: command media-volume 0' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: page $a volume $volume\$" "the tab's volume didn't come back from the core"
send click "$(control_x 2)" $control_y
resumed=$step
expect "^browser: title $a playing\$" "the player's play didn't play the page again"
# The core's line has Chromium's prefix.
expect "browser: tab $a media at volume $volume\$" \
  "the core didn't turn the page's media to $volume"
../tools/window-screenshot.sh "$pid" "$out/media-player-volume.png" || fail "no window to capture"
send click "$(control_x 2)" $control_y
expect "^browser: title $a paused\$" "the player's pause didn't pause the page again"
# Once as it plays, or again as the audio focus manager settles who ducks, never back and forth.
applied=$(sed -n "/^browser: drive $resumed: /,\$p" "$log" |
  grep -c "browser: tab $a media at volume ")
[ "$applied" -le 10 ] || fail "the core turned the page's media down $applied times as it played"
send click "$(control_x 4)" $control_y
expect '^browser: media volume drawer closed$' \
  "a second click on the volume didn't close its drawer"
player_at '6[56]' "the card didn't shrink to 65.5 as its drawer closed"
button_x() { echo $((mx + mw - 5 - 12 - $1 * 26)); }
title_y=$((my + 19))
send click "$(button_x 1)" $title_y
expect '^browser: command media-picture-in-picture via player$' \
  "the player's Picture in Picture didn't take the click"
expect "^browser: title $a floating\$" "the player's Picture in Picture didn't float the page"
expect "^browser: page $a media 1 playing 0 previous 1 next 1 pip 0 Quiet\$" \
  "the player kept Picture in Picture while the page floated"
# On the Mac the floating window takes the keyboard, and GPUI then reads the pointer from AppKit,
# whose real one the driver's events never move: off the stack, which closes as it's left. On
# Linux and Windows the driver's pointer stays on the open stack. The pointer leaves it here too,
# and the card is pressed once it rests, wherever that is.
floated=$step
send move $page_point
expect '^browser: media stack rest$' "the stack didn't rest once the pointer left it" "$floated"
player_at 33 "the player wasn't drawn at rest once the page floated" "$floated"
send click $((mx + 40)) $((my + mh / 2))
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "a click on the player didn't show its tab"
expect "^browser: title $a landed\$" "the floating page didn't land as its tab showed"
expect '^browser: media players $' "the tab shown kept its player"
send move $page_point
shown "$b"
expect "^browser: media players $a\$" "the paused tab out of sight had no player"
player_at 33 "the paused tab's player wasn't drawn at rest"
open_stack
send click "$(button_x 0)" $title_y
expect '^browser: command media-dismiss via player$' "the player's × didn't take the click"
expect '^browser: media players $' "× didn't put the player away"
send move $page_point

# A private window's music out of sight has a player there alone. Every window hears every tab,
# but the regular window's players are its Spaces' tabs, so its music, playing again, is its one.
# The private window logs a Today of its own, so the regular one's keys are taken first.
key_a=$(($(today | tr ' ' '\n' | grep -nx "$a" | cut -d: -f1) + $(pinned | wc -w)))
key_b=$(($(today | tr ' ' '\n' | grep -nx "$b" | cut -d: -f1) + $(pinned | wc -w)))
send key n cmd shift
expect '^browser: private window opened [0-9]* profile [0-9]*$' "⇧⌘N didn't open a private window"
private=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
expect "^browser: window opened $private\$" "the private window didn't open"
new_window_bar
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
player_at 33 "the regular window's player wasn't drawn again"
../tools/window-screenshot.sh "$pid" "$out/media-player-own.png" || fail "no window to capture"
send click $((mx + mw / 2)) $((my + mh / 2))
expect "^browser: title $a paused\$" "the player's pause didn't pause the page"
expect '^browser: media stack open$' "the stack didn't open under the pointer"
player_at '6[56]' "the open stack's front card wasn't 65.5 tall"
send click "$(button_x 0)" $title_y
expect '^browser: media players $' "× didn't put the player away"
send move $page_point
send window 2
send key w cmd shift
expect "^browser: window closed $private\$" "⇧⌘W didn't close the private window"
send window 1

# Open, the newest card whose page has a video shows it live over its title row (#2100): the page's
# own video, as Picture in Picture shows it, which the page sees go into Picture in Picture and
# leave it as the stack rests. Paused, it shows the frame it stopped on. A test pattern with a
# silent sound track, six seconds, long enough for Chrome to give it controls: ffmpeg -f lavfi -i
# testsrc2=size=160x90:rate=10:duration=6 -f lavfi -i anullsrc=r=48000:cl=mono -t 6 -c:v libvpx
# -b:v 40k -c:a libopus -b:a 6k -shortest card.webm
cp check/card.webm "$out/site/card.webm"
printf '%s' '<title>loading</title><body style="margin:0;background:#333">
<video id=v loop src=card.webm style="width:100vw;height:100vh"></video>
<script>
v.onplaying = () => document.title = "playing";
v.onpause = () => document.title = "paused";
v.onenterpictureinpicture = () => document.title = "floating";
v.onleavepictureinpicture = () => document.title = "landed";
// A handler returning false would keep every other key from the browser, ⌘5 among them.
onkeydown = e => { if (e.key == "m") v.play(); };
</script>' > "$out/site/card.html"
send key t cmd
expect '^browser: command bar shown$' "the command bar didn't open for the card's video"
send type "${site}card.html"
send key return
expect "^browser: loaded [0-9]* ${site}card.html\$" "the card's video page didn't load"
vc=$(grep "^browser: loaded [0-9]* ${site}card.html\$" "$log" | tail -1 | cut -d' ' -f3)
send click $page_point
send key m
expect "^browser: title $vc playing\$" "m didn't play the card's video"
shown "$b"
expect "^browser: media players $vc\$" "the video playing out of sight had no player"
# card_at <failure>: reads where the video's card was drawn last into mx, my, mw and mh.
card_at() {
  expect "^browser: media player $vc at " "$1"
  read -r mx my mw mh <<EOF
$(grep "^browser: media player $vc at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
}
# card_video <failure> <capture>: waits for the core to show the video in the stack just opened,
# and captures it.
card_video() {
  expect "^browser: card video $vc at " "the open card didn't ask for its video"
  expect "browser: card video $vc in its card at " "$1"
  card_at "the open card wasn't drawn"
  ../tools/window-screenshot.sh "$pid" "$out/$2" || fail "no window to capture"
}
card_at "the video's player wasn't drawn"
send move $((mx + mw / 2)) $((my + mh - 16))
expect '^browser: media stack open$' "the stack didn't open over the video's card"
card_video "the core didn't show the video in its open card" media-card-video.png
expect "^browser: title $vc floating\$" "the page didn't see its video go into its card"
send move $page_point
expect '^browser: media stack rest$' "the stack didn't rest once the pointer left it"
expect "browser: card video $vc left its card" "the video stayed as the stack rested"
expect "^browser: title $vc landed\$" "the page didn't see its video leave its card"
card_at "the video's player wasn't drawn at rest"
send click $((mx + mw / 2)) $((my + mh / 2))
expect "^browser: title $vc paused\$" "the player's pause didn't pause the video"
expect '^browser: media stack open$' "the stack didn't open under the pointer on the paused card"
card_video "the core didn't show the paused video in its card" media-card-video-paused.png
# Its title row is under the video, and × is its last button, as without one.
send click $((mx + mw - 5 - 12)) $((my + mh - 47))
expect '^browser: command media-dismiss via player$' "the video card's × didn't take the click"
expect '^browser: media players $' "× didn't put the video's player away"
expect "browser: card video $vc left its card" "the video stayed as its card went"
send move $page_point

# The Cast card, from View › Cast… and from the page menu, with whatever devices the network has,
# none clicked. Chrome looking for them is the first time on the Mac that the app reaches the local
# network, and macOS asks whether it may, seconds later, over the app and with its activation. The
# user answers once; a check VM or a hosted runner is asked each time it's new, so the run answers
# it after the card, through check/alert.mm, and the app has its activation back.
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
# A Mac that has answered before isn't asked again, and the app keeps its activation.
answer_macos 30 "after the Cast card"

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
# The core lays the Mini Player out as it shows, before the commands Linux's keyboard step sends.
floated=$step
expect "^browser: title $v floating\$" "the video menu's Picture in Picture didn't float the video"
# The core's last place for a Mini Player, x, y, w and h, then its screen's work area's.
placed() {
  grep 'browser: mini player placed at ' "$log" | tail -1 | sed 's/.* placed at //; s/ on / /'
}
# The first shows 10 in from the top right of its screen's work area, as Arc's.
expect 'browser: mini player placed at ' "the core didn't place the Mini Player"
read -r fx fy fw fh wx wy ww wh <<PLACED
$(placed)
PLACED
[ "$((fx + fw))" = "$((wx + ww - 10))" ] && [ "$fy" = "$((wy + 10))" ] ||
  fail "the first Mini Player was at $fx $fy $fw $fh, not 10 in from the top right of $wx $wy $ww $wh"
# Chromium shows a video's Mini Player inactive, so the browser window keeps the keyboard. Under
# Wayland the core gives it back as the Mini Player maps (#2060). Linux alone: no run has shown the
# Mac's yet.
if [ "$(uname)" = Linux ]; then
  kept_keys "the video floating from its menu"
fi

# Its look, Dia's: 6 pt corners and a rim, one flat scrim, and its close button 28 × 28 at the top
# left and back to its tab 29 × 28 at the top right, 12 in. An engine older than its core has
# Chrome's.
looked=
if engine_has ../core/mini_player.cc ../engine/patches/ours/chromium-core-mini-player.patch; then
  looked=1
  expect 'browser: mini player look ' "the core didn't give the Mini Player its look" "$floated"
  look=$(grep 'browser: mini player look ' "$log" | tail -1)
  case "$look" in
  *" close 12,12 28x28 back "*",12 29x28 origin "*) ;;
  *) fail "the Mini Player's buttons weren't Dia's: $look" ;;
  esac
fi

# The floating video, the Mini Player, moves with a scroll or a drag, and ⌘-scroll, Ctrl-scroll
# off the Mac, sizes it about its middle. A double-click on it, with another tab shown, shows its
# tab and lands it. X11 alone, as pip_driven says; on the Mac drive.mm throws it, below.
# Proof only (#2803): the display's pointer over the Mini Player, off its controls, so Chrome
# shows them, then every window of the browser captured, with the look line the core logged last.
proof_shot() {
  send pip press $((pw / 8)) $((ph / 2))
  send pip release $((pw / 8)) $((ph / 2))
  sleep 1
  ../tools/window-screenshot.sh "$pid" "$out/$1" all || fail "no window to capture"
  grep 'browser: mini player look ' "$log" | tail -1 > "$out/${1%.png}.look"
}
if pip_driven; then
  px=0 py=0 pw=0 ph=0
  send pip
  mini_player_at "x + w == $((wx + ww - 10)) && y == $((wy + 10))" \
    "the floating video had no Mini Player 10 in from the top right of the screen"
  proof_shot mini-player-narrowest.png
  # Two notches up move it down as far as the content would go, 120 a notch: it starts at the top.
  send pip wheel $((pw / 2)) $((ph / 3)) -2
  mini_player_at "x == px && y == py + 240 && w == pw && h == ph" \
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
  # Proof only (#2803): Ctrl with a scroll until it grows no more, the widest.
  send pip wheel $((pw / 2)) $((ph / 3)) -30 cmd
  last=
  for _ in $(seq 40); do
    sleep 0.3
    send pip
    now=$(grep '^browser: pip at ' "$log" | tail -1)
    [ "$now" = "$last" ] && break
    last=$now
  done
  mini_player_at "w >= pw" "the Mini Player shrank as it grew"
  proof_shot mini-player-widest.png
  # By its row: ⌘ and a number in the page whose video floats shows no tab on Linux yet.
  send click $(row $(($(today | tr ' ' '\n' | grep -nx "$b" | cut -d: -f1) - 1)))
  expect "^browser: selected $b in " "a click on its row didn't show tab $b of \"$(today)\""
  send pip doubleclick $((pw / 2)) $((ph / 4))
  expect "^browser: selected $v\( in .*\)\{0,1\}\$" \
    "a double-click on the Mini Player didn't show its tab"
  expect "^browser: title $v landed\$" "a double-click on the Mini Player didn't land it"
  expect '^browser: pip gone$' "the Mini Player stayed after a double-click"
elif [ "$(uname)" = Darwin ]; then
  # A throw, or a release near a side, settles it 10 in from the screen's visible frame, on Dia's
  # spring: a throw at the corner within 45° of it, a release near a side along that side (#2099).
  px=0 py=0 pw=0 ph=0
  send pip
  mini_player_at "x + w == $((wx + ww - 10)) && y == $((wy + 10))" \
    "the floating video had no Mini Player 10 in from the top right of the screen"
  # Off its controls, as the Linux drag. Down and left at 2000 pt/s from the top right: the bottom
  # left corner is within 45° of it, the top left and bottom right aren't the nearest.
  send pip drag $((pw / 8)) $((ph / 2)) -300 200 150
  expect 'browser: mini player let go at .*, settling at ' "a throw didn't settle the Mini Player"
  mini_player_at "x == $((wx + 10)) && y + h == $((wy + wh - 10))" \
    "a throw down and left didn't settle the Mini Player 10 in at the bottom left of the screen"
  ../tools/window-screenshot.sh "$pid" "$out/mini-player-corner.png" screen ||
    fail "no window to capture"
  # Up at 100 pt/s, let go 30 in from its place along the left side: it's caught there, 10 in.
  send pip drag $((pw / 8)) $((ph / 2)) 30 -300 3000
  expect 'browser: mini player let go at .*, settling at ' \
    "a slow release near the left side didn't settle the Mini Player"
  mini_player_at "x == $((wx + 10)) && (y - (py - 300)) ^ 2 <= 4 && w == pw && h == ph" \
    "a slow release near the left side didn't settle the Mini Player 10 in along it"
  ../tools/window-screenshot.sh "$pid" "$out/mini-player-side.png" screen ||
    fail "no window to capture"
  # And one let go slowly away from the sides stays where it's let go.
  send pip drag $((pw / 8)) $((ph / 2)) 300 0 3000
  expect 'browser: mini player let go at .*, staying$' \
    "a slow release away from the sides didn't leave the Mini Player where it was let go"
  mini_player_at "(x - (px + 300)) ^ 2 <= 4 && y == py" \
    "a slow release away from the sides moved the Mini Player"
  send click $page_point
  send key x
  expect "^browser: title $v landed\$" "the floating video didn't come back"
else
  echo "skip: moving, sizing and double-clicking the Mini Player on" \
    "$(uname)${WAYLAND_DISPLAY:+ Wayland}: the driver doesn't drag a Picture in Picture window" \
    "there."
  # Clear of the page's middle, where sway floats the Mini Player wherever the core asks for it.
  send click 300 200
  send key x
  expect "^browser: title $v landed\$" "the floating video didn't come back"
fi
# The next shows where the last was left, at that one's corner nearest its screen's.
read -r lx ly lw lh <<LEFT
$(grep 'browser: mini player left at ' "$log" | tail -1 | sed 's/.* left at //')
LEFT
[ -n "$lh" ] || fail "the core didn't hear where the Mini Player was left"
post rightclick $page_point
expect "^browser: menu shown .*$(cased 'Picture in Picture')" \
  "the video's menu had no Picture in Picture the second time"
send choose "$(cased 'Picture in Picture')"
expect "^browser: title $v floating\$" "the video didn't float a second time"
expect 'browser: mini player placed at ' "the core didn't place the second Mini Player"
# Whether the last Mini Player placed shows where the last was left, $lx $ly $lw $lh, at that one's
# corner nearest its screen's, with $across and $down saying so for mini_player_at.
placed_where_left() {
  read -r nx ny nw nh wx wy ww wh <<PLACED
$(placed)
PLACED
  ex=$lx across="x == $lx"
  if [ $((lx + lw / 2)) -gt $((wx + ww / 2)) ]; then
    ex=$((lx + lw - nw)) across="x + w == $((lx + lw))"
  fi
  ey=$ly down="y == $ly"
  if [ $((ly + lh / 2)) -gt $((wy + wh / 2)) ]; then
    ey=$((ly + lh - nh)) down="y + h == $((ly + lh))"
  fi
  [ "$nx $ny" = "$ex $ey" ]
}
placed_where_left ||
  fail "the second Mini Player was at $nx $ny, not $ex $ey, where the last was left at $lx $ly $lw $lh"
none_since 'browser: mini player back in its tab$' ||
  fail "the Mini Player went back to its tab as the menu that floated it closed"
if pip_driven; then
  send pip
  mini_player_at "$across && $down" "the second Mini Player didn't show where the last was left"
fi
# The same window shows again, and under Wayland takes the keyboard again as it maps.
if [ "$(uname)" = Linux ]; then
  kept_keys "the video floating from its menu a second time"
fi
# Another window and back to its tab's: the Mini Player goes back in the tab, as Arc's does.
send key n cmd
expect '^browser: window opened [0-9]' "⌘N didn't open a second window"
# The driver numbers windows in the order they were made, the private one that closed too.
second=$(grep -c '^browser: window opened ' "$log")
# The new window becomes the one the user is in a moment after it opens; going back before then
# leaves the first one main all along, and nothing comes back to it. Linux's host doesn't say.
if [ "$(uname)" != Linux ]; then
  opened=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
  expect "^browser: window $opened main true\$" "the window ⌘N opened didn't become the main one"
fi
send window 1
expect 'browser: mini player back in its tab$' \
  "coming back to the video's window didn't put the Mini Player back in its tab"
expect "^browser: title $v landed\$" "the Mini Player didn't land as its window came back"
# The same with a menu open as the user goes to another app: the menu closes as the app's window
# takes the keyboard, and coming back puts the Mini Player back (#1614). On X11, as in mcp.sh, a
# window of another client's takes it from xdotool.
if [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ] && ! sway_xwayland; then
  post rightclick $page_point
  expect "^browser: menu shown .*$(cased 'Picture in Picture')" \
    "the video's menu had no Picture in Picture the third time"
  send choose "$(cased 'Picture in Picture')"
  expect "^browser: title $v floating\$" "the video didn't float a third time"
  post rightclick $page_point
  expect "^browser: menu shown " "the video's page had no menu over its Mini Player"
  rm -f "$out/other-in"
  mkfifo "$out/other-in"
  python3 -I check/other-window.py < "$out/other-in" > "$out/other-window.log" 2>&1 &
  other=$!
  exec 7> "$out/other-in"
  for _ in $(seq 50); do grep -q '^[0-9]' "$out/other-window.log" && break; sleep 0.1; done
  xdotool windowactivate --sync "$(head -1 "$out/other-window.log")" 2> /dev/null ||
    fail "another app's window didn't open and take the keyboard: $(cat "$out/other-window.log")"
  expect '^browser: menu window hidden$' "the menu stayed open as another app took the keyboard"
  send window 1
  expect 'browser: mini player back in its tab$' \
    "coming back from another app with a menu open didn't put the Mini Player back in its tab"
  expect "^browser: title $v landed\$" "the Mini Player didn't land as its window came back"
  exec 7>&-
  wait "$other" 2> /dev/null || true
else
  echo "skip: coming back from another app with a menu open, on $(uname) ${WAYLAND_DISPLAY:+Wayland}: no other app's window to go to."
fi
# Launched again, the next shows where the last was left before the quit, as Arc's does.
read -r lx ly lw lh <<LEFT
$(grep 'browser: mini player left at ' "$log" | tail -1 | sed 's/.* left at //')
LEFT
[ -n "$lh" ] || fail "the core didn't hear where the last Mini Player was left"
# The window ⌘N opened goes, so the relaunch brings back the video's alone, where the driver clicks.
send window "$second"
send key w cmd shift
expect '^browser: window closed [0-9]' "⇧⌘W didn't close the second window"
exec 3>&-
quit
log="$out/media-relaunched.log"
step=0
# Another address, as the Space brings back the video's tab too.
BROWSER_DRIVE=1 BROWSER_URL="${site}video.html?again" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await "^browser: loaded [0-9]* ${site}video.html?again\$" ||
  fail "the video's page didn't load after a relaunch"
v=$(grep "^browser: loaded [0-9]* ${site}video.html?again\$" "$log" | tail -1 | cut -d' ' -f3)
await "^browser: title $v playing\$" || fail "the video didn't play after a relaunch"
post rightclick $page_point
expect "^browser: menu shown .*$(cased 'Picture in Picture')" \
  "the video's menu had no Picture in Picture after a relaunch"
send choose "$(cased 'Picture in Picture')"
expect "^browser: title $v floating\$" "the video didn't float after a relaunch"
expect 'browser: mini player placed at ' "the core didn't place the Mini Player after a relaunch"
placed_where_left ||
  fail "after a relaunch the Mini Player was at $nx $ny, not $ex $ey, where the last was left at $lx $ly $lw $lh"
exec 3>&-
quit

# Chrome's origin, its word on whose video this is, shows beside the ×, on its row, clear of it, of
# back to its tab and of the "–", at each size the Mini Player had, from the smallest it opens at
# (#2802). The core logs a look as "<w>x<h> close <x>,<y> <w>x<h> back … origin … minimize …|none".
if [ -n "$looked" ]; then
  bad=$(grep 'browser: mini player look ' "$log" | sed 's/.*mini player look //' | tr ',x' '  ' |
    awk '{
      right = $4 + $6; ends = $14 + $16
      clear = $13 == "origin" && $16 > 0 && $14 >= right && ends <= $9 && 2 * $15 + $17 == 2 * $5 + $7
      if ($19 != "none") clear = clear && ends <= $19 && $19 >= right && $19 + $21 <= $9
      if (!clear) { print; exit }
    }')
  [ -z "$bad" ] || fail "the Mini Player's origin or its \"–\" was over its buttons: $bad"
fi

echo "ok: a tab in a call marks its row with the camera, then the screen while it shares a tab, the tab it shares shows the screen too, another tab's Share This Tab Instead moves the share there, and the marks go when the call ends; a call floats while its tab is hidden, unless its Site Controls or Settings turn that off, and its window moves by its top bar and sizes from its corner; music playing out of sight has a player that pauses it, skips back, sets its volume from a drawer, floats it in Picture in Picture, shows its tab and goes on ×, and a private window's music has one there alone; the Cast card opens from View and the page menu and closes on ×; a video's menu floats it in Picture in Picture, 10 in from the top right of the screen, where a scroll or a drag moves it, Ctrl with a scroll sizes it and a double-click shows its tab, the next shows where the last was left, after a relaunch too, and coming back to its tab's window from another window, or from another app with a menu open, puts it back in the tab."
