# check.sh runs this at 770, only when named.
# Linux only: Google's Widevine module, offered by the protected-media note, downloads at its yes,
# and after a restart a Widevine stream plays (#236). The page is Shaka Player's public Widevine
# demo stream, with its no-account license server, served from the check's server, in a profile of
# its own; nothing signs in anywhere. It needs the network, as the module and the stream come from
# Google, so it runs only when named: ./check.sh widevine.

if [ "$(uname)" != Linux ]; then
  echo "skip: widevine. Only Linux gets Google's Widevine module (decision 1)."
  return 0
fi
serve
# The page says what happened in its title, which the app logs. Playing, it pauses after a second,
# so the capture is of a picture that stands still.
printf '%s' '<!doctype html>
<title>widevine loading</title>
<video id="video" muted width="640"></video>
<script src="https://cdn.jsdelivr.net/npm/shaka-player@4.11.7/dist/shaka-player.compiled.js"></script>
<script>
  const video = document.getElementById("video");
  const said = (title) => { document.title = title; };
  video.addEventListener("timeupdate", () => {
    if (video.currentTime > 1) { video.pause(); said("widevine playing"); }
  });
  shaka.polyfill.installAll();
  const player = new shaka.Player();
  const server = "https://cwip-shaka-proxy.appspot.com/no_auth";
  player.attach(video)
    .then(() => {
      player.configure({ drm: { servers: { "com.widevine.alpha": server } } });
      return player.load("https://storage.googleapis.com/shaka-demo-assets/angel-one-widevine/dash.mpd");
    })
    .then(() => video.play())
    .catch((error) => said("widevine error " + (error.code || error)));
</script>' > "$out/site/widevine.html"
profile_dir="$PWD/$out/widevine/profile"
rm -rf "$profile_dir"
mkdir -p "$profile_dir"
# Starts the app on the page with the log $1, in the run's profile, as the note's Restart would.
launch_widevine() {
  log="$1"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="${site}widevine.html" BROWSER_PROFILE_DIR="$profile_dir" \
    "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
}
# Waits up to $1 seconds for a line matching $2 in the log.
await_long() {
  for _ in $(seq $(($1 * 10))); do
    grep -qE "$2" "$log" && return 0
    kill -0 "$pid" 2>/dev/null || fail "the app exited"
    sleep 0.1
  done
  return 1
}

# The module isn't there yet, so the page's request brings the note, and its button is the yes.
launch_widevine "$out/widevine.log"
await_load
await "^browser: drm note [0-9]* at " || fail "the page asking for Widevine brought no note"
tab=$(grep -oE "^browser: drm note [0-9]+ at " "$log" | tail -1 | cut -d' ' -f4)
grep -q "^browser: page $tab needs com.widevine.alpha\$" "$log" || fail "the page asked for no Widevine"
read -r nx ny nw nh <<EOF
$(grep "^browser: drm note $tab at " "$log" | tail -1 | cut -d' ' -f6-)
EOF
../tools/window-screenshot.sh "$pid" "$out/widevine-note.png" || fail "no window to capture"
send move $((nx + nw / 2)) $((ny + nh - 26))
send click $((nx + nw / 2)) $((ny + nh - 26))
expect "^browser: widevine download\$" "the note's button downloaded nothing"
# Google's module is about 10 MB.
await_long 120 "^browser: widevine state (restart|failed)\$" || fail "the module didn't come"
grep -q "^browser: widevine state restart\$" "$log" || fail "the module didn't download"
[ -n "$(ls "$profile_dir/WidevineCdm" 2>/dev/null)" ] || fail "no module in the profile"
../tools/window-screenshot.sh "$pid" "$out/widevine-restart.png" || fail "no window to capture"
# Settings › Features offers the restart too, beside Remove (#1393). A click would start an app
# outside the check, so the run reads that both are drawn and restarts as the note's run does.
send key , cmd
expect '^browser: settings shown ' "⌘, didn't show Settings"
settings_drew
send clickkey $(drawn "toolbar Features")
expect '^browser: settings tab Features$' "a click on Features didn't show its section"
settings_drew Features
drawn "widevine remove" > /dev/null || fail "Settings' Widevine row offered no Remove"
drawn "widevine restart" > /dev/null || fail "Settings' Widevine row offered no restart"
../tools/window-screenshot.sh "$pid" "$out/widevine-settings.png" all || fail "no screen to capture"
quit

# Launched again, Chromium loads the module and the page plays, with no note. The Spaces saved at
# the quit would bring back the first launch's tab, with its error in its saved title.
rm -f "$profile_dir/Spaces.json"
launch_widevine "$out/widevine-played.log"
await_load
await_long 60 "^browser: title [0-9]+ widevine (playing|error .*)\$" ||
  fail "the page said nothing of playing"
grep -qE "^browser: title [0-9]+ widevine playing\$" "$log" ||
  fail "the page didn't play: $(grep -E '^browser: title [0-9]+ widevine' "$log" | tail -1)"
grep -q "^browser: widevine state ready\$" "$log" || fail "the core didn't say the module loaded"
grep -qE "^browser: drm note [0-9]+ at " "$log" && fail "the note showed with the module loaded"
../tools/window-screenshot.sh "$pid" "$out/widevine-playing.png" || fail "no window to capture"
quit
echo "ok: the protected-media note downloads Google's Widevine module at its yes, Settings' row then offers the restart beside Remove, and after a restart Shaka's Widevine stream plays with no note. Windows: $out/widevine-note.png, $out/widevine-restart.png, $out/widevine-settings.png, $out/widevine-playing.png"
