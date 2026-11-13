# check.sh runs this at 350.
# Quitting through the menu and with ⌘Q, answering Quit when the app asks first, closes the window
# after its tabs and ends every Chromium process with status 0. So does the menu's Quit as the
# window comes on screen, before the UI has made the dialog that asks, which once never quit
# (#905). So does each with a Little ilo window open over the window, its page loaded, and on
# Windows each window's close button: all of them at once, as Windows asks each window to close
# as it shuts down, the regular window's alone, which quits as the last regular window closes, or
# the last window's with no Little ilo. One of those crashed (#1425). So, on Linux, does a quit by
# signal just after a switch to a coloured Space, ten times, which crashed one launch in two
# (#1478). So does a quit with a page playing music, and one with a link preview open. None
# leaves a crash report, where on Linux a crash as the window closed once left one at every quit
# (#474). A quit by signal, as the other runs' is, never closes the window as the user does, so
# only a quit the user makes finds that one.

serve
# The reports Crashpad keeps until they're sent, in the profile's Crashpad folder, as crash.sh
# finds them: on Windows its reports folder, which holds them all, as its handler sends none.
waiting="$(dirname "$saved")/Crashpad/pending"
! windows || waiting="$(dirname "$saved")/Crashpad/reports"
reports() {
  find "$waiting" -name '*.dmp' 2>/dev/null | wc -l | tr -d ' '
}
# Starts the app for the quit $1 with the driver, its log quit-$1.log, on a profile with no Spaces
# saved, opening the page $2, or with none the link from another app in $3. The rest are
# Chromium's switches.
start() {
  way=$1
  page=$2
  shift 2
  log="$out/quit-$way.log"
  step=0
  before=$(reports)
  rm -f "$saved" "$(dirname "$saved")/Link rules"
  if [ -n "$page" ]; then
    BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" "$@" < "$out/drive" > "$log" 2>&1 &
  else
    BROWSER_DRIVE=1 "$browser" "$@" < "$out/drive" > "$log" 2>&1 &
  fi
  pid=$!
  exec 3> "$out/drive"
}
# Quits through $1, menu, key or launch (the menu's), and answers Quit in the dialog if it asks,
# which an earlier run may have turned off. Quit is the driver's last command.
quit_through() {
  if [ "$1" != key ]; then
    post menu "Quit $app_name"
  else
    post key q cmd
  fi
  await '^browser: command quit via ' || fail "$way: Quit through the $1 didn't quit"
  for _ in $(seq 50); do
    kill -0 "$pid" 2>/dev/null || break
    if grep -q '^browser: dialog settled$' "$log"; then
      post key return
      break
    fi
    sleep 0.1
  done
  exec 3>&-
}
# The app quit as the user asked: Chromium stopped as a quit does, every process ended with
# status 0, and there's no new crash report.
quit_cleanly() {
  exited 20
  grep -q '^browser: stopping$' "$log" || fail "$way: the app ended without stopping as a quit does"
  [ "$(reports)" = "$before" ] || fail "$way: the quit left a crash report"
}
# The Little ilo window that opened the link $1 from another app has loaded it.
opened_little() {
  await "^browser: link from another app $1: tab [1-9][0-9]* in little window [0-9]* of space " ||
    fail "$way: the link $1 didn't open in Little ilo: $(grep '^browser: link' "$log" | tail -1)"
  await "^browser: loaded [0-9]* $1\$" || fail "$way: Little ilo didn't load $1"
}
# Sends the link $1 as another app does, which opens it in a Little ilo window of its own while no
# rule says otherwise, and waits for the window to load it. Off the Mac it's a second launch on the
# run's profile, which hands it to the running browser and exits, in a shell of its own, as on
# Windows the launch replaces the shell it runs in.
little() {
  if [ "$(uname)" = Darwin ]; then
    open -a "$app" "$1"
  else
    ("$browser" "$1" < /dev/null >> "$out/quit-sent.log" 2>&1) ||
      fail "$way: the second launch with $1 failed"
  fi
  opened_little "$1"
}
# Asks each of the app's browser windows to close, as its close button does, or with front the
# one in front alone.
close_windows() {
  exec 3>&-
  python3 -I check/app-windows.py close "$(cat "/proc/$pid/winpid")" ${1:+"$1"} ||
    fail "$way: the windows couldn't be asked to close"
}

for way in menu key launch; do
  start "$way" 'data:text/html,<h1>Quit</h1>'
  if [ $way = launch ]; then
    await '^browser: window on screen at ' || fail "the window didn't come on screen"
  else
    await_load
  fi
  quit_through "$way"
  quit_cleanly
done

# With a Little ilo window open, through the menu and the key.
for through in menu key; do
  start "little-$through" "$site"
  await_load
  little "${site}?little-$through"
  quit_through "$through"
  quit_cleanly
done

if windows; then
  # Every window's close button at once, with the Little ilo window a link opened at launch, as
  # #1425's did.
  start little-launch "" "${site}?little-launch"
  opened_little "${site}?little-launch"
  close_windows
  quit_cleanly
  # The regular window's close button alone, with Little ilo over it.
  start little-close-regular "$site"
  await_load
  little "${site}?little-close-regular"
  send window 1
  close_windows front
  quit_cleanly
  # The last window's close button, with no Little ilo.
  start close-last "$site"
  await_load
  close_windows
  quit_cleanly
fi

# On Linux, quits by signal as a switch to a coloured Space cross-fades the window's colours: as
# Chromium quits it closes every window before the core lets its window go, and the app setting
# the colours of the closed one crashed about one launch in two (#1478), so ten launches.
if [ "$(uname)" = Linux ]; then
  printf '%s' '<title>two</title><p style="font:24px system-ui">Space two</p>' \
    > "$out/site/quit-two.html"
  start space-switch "$site"
  await_load
  first=$(space)
  send menu New Space…
  expect "^browser: spaces $first [0-9]*\$" "$way: New Space didn't make a second Space"
  second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
  send type two
  send key return
  expect "^browser: create space $second two\$" "$way: return didn't name the Space"
  send key t cmd
  expect '^browser: command bar shown$' "$way: ⌘T in the new Space didn't open the command bar"
  send type "${site}quit-two.html"
  send key return
  expect '^browser: today [0-9][0-9]*$' "$way: the page didn't open in the new Space"
  expect "^browser: loaded $(today) ${site}quit-two.html\$" "$way: the page didn't load in the new Space"
  post rightclick 100 112
  expect '^browser: menu shown ' "$way: the Space's menu didn't show"
  send choose "$(cased 'Edit Theme Color…')"
  expect '^browser: theme picker drawn$' "$way: the theme picker didn't draw itself on screen"
  send clickkey "$(swatch_x 1)" $swatch_y
  expect "^browser: colored space $second [0-9a-f]*\$" "$way: the pink swatch didn't colour the Space"
  send key escape
  expect '^browser: theme picker hidden$' "$way: escape didn't close the theme picker"
  quit_app
  quit_cleanly
  for launch in $(seq 10); do
    way="space-switch, launch $launch"
    log="$out/quit-space-switch-$launch.log"
    step=0
    before=$(reports)
    BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
    pid=$!
    exec 3> "$out/drive"
    # A launch numbers the Spaces it brings back afresh, and the first launch's may have given a
    # number to a download the profile kept, as import's extension, so the ids are this launch's.
    await '^browser: added space [0-9]* two$' || fail "$way: the second Space didn't come back"
    first=$(sed -n 's/^browser: added space \([0-9]*\) Space 1$/\1/p' "$log" | head -1)
    second=$(sed -n 's/^browser: added space \([0-9]*\) two$/\1/p' "$log" | head -1)
    await "^browser: space $second\$" || fail "$way: the second Space didn't come back"
    send key 1 ctrl
    expect "^browser: space $first in " "$way: ⌃1 didn't show the first Space"
    send key 2 ctrl
    expect "^browser: space $second in " "$way: ⌃2 didn't show the second Space"
    exec 3>&-
    quit_app
    quit_cleanly
  done
fi

# Music playing, which Chromium lets a page start without the user's click under this switch: the
# media.sh page's ten seconds of silence, over and over, as music Chrome gives controls.
printf '%s' '<title>loading</title><script>
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
navigator.mediaSession.metadata = new MediaMetadata({title: "Quiet", artist: "Check"});
music.play().catch(error => document.title = "refused " + error.name);
</script>' > "$out/site/quit-music.html"
start music "${site}quit-music.html" --autoplay-policy=no-user-gesture-required
await_load
await '^browser: title [0-9]* playing$' ||
  fail "music: the page didn't play: $(grep '^browser: title ' "$log" | tail -1)"
await '^browser: page [0-9]* media 1 playing 1 ' || fail "music: the page's music had no controls"
quit_through menu
quit_cleanly

# A link preview open over the window: a pinned tab's link to another site opens in one.
target="http://localhost:$port/quit-target.html"
printf '%s' "<title>loading</title>
<a href=\"$target\" style=\"position:fixed;left:0;top:0;width:100%;height:40%;font:24px system-ui\">
A link to another site</a><p style=\"position:fixed;top:50%;font:24px system-ui\">Page</p>
<script>requestAnimationFrame(() => requestAnimationFrame(() => document.title = \"ready\"));</script>" \
  > "$out/site/quit-preview.html"
printf '%s' '<title>target</title><p style="font:24px system-ui">Another site</p>' \
  > "$out/site/quit-target.html"
previewed=
start preview "${site}quit-preview.html"
await_load
await '^browser: title [0-9]* ready$' || fail "preview: the page didn't draw"
send click $page_point
if [ "$active" = 1 ]; then
  send key d cmd
  expect '^browser: pinned [0-9]' "preview: ⌘D didn't pin the page's tab"
  send click 600 200
  expect "^browser: link preview [1-9][0-9]* $target\$" "preview: the link didn't open the preview"
  expect '^browser: link preview settled$' "preview: the preview didn't settle"
  quit_through menu
  quit_cleanly
  previewed=", and with a link preview open"
else
  quit_through menu
  quit_cleanly
  echo "skip: the quit with a link preview open. Browser wasn't the active app, so the page ignored the click."
fi

closed=
! windows || closed=", and on Windows the close buttons of every window with Little ilo open, of the regular window alone under it, and of the last window"
switched=
[ "$(uname)" != Linux ] || switched=", and on Linux ten quits by signal just after a switch to a coloured Space"
echo "ok: quitting through the menu and with ⌘Q, answering Quit, through the menu as the window comes on screen, and through both with a Little ilo window open$closed$switched, with music playing$previewed, ends every Chromium process with status 0 after Chromium stops, and leaves no crash report"
