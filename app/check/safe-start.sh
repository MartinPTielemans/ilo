# check.sh runs this at 655.
# Safe start (#2345): a crash at launch, the browser killed in its first minute, changes nothing by
# itself, as the next launch loads the profile's extension and Boost as before. A second in a row
# starts the third launch safe: the core holds the extension and the ad blocker, the Extensions menu
# then doesn't list the extension, the site's Boost doesn't show, and the card at the sidebar's foot
# says so. Its Turn Back On restarts the browser, into a normal launch, and the next launch loads
# them both again, the profile's own switches as they were. The mark is one line in the app's
# folder, "Last launch", which check.sh takes away before each run. That a crash after the first
# minute counts nothing is the unit tests' (safe_start.rs), as waiting it out costs a minute.

serve
printf '%s' '<!doctype html><meta charset="utf-8"><title>Safe</title>
<body style="margin:0;background:rgb(51, 51, 51)">
<script>
let said;
setInterval(() => {
  const now = "body " + getComputedStyle(document.body).backgroundColor;
  if (now !== said) document.title = said = now;
}, 100);
</script>' > "$out/site/safe.html"
host=${site#http://}
host=${host%%:*}
# A profile of the run's own, new each time, with one extension and the site's Boost.
safe_dir="$PWD/$out/safe-start"
rm -rf "$safe_dir"
mkdir -p "$safe_dir/extension" "$safe_dir/profile/Default/Boosts"
profile_dir="$safe_dir/profile"
cat > "$safe_dir/extension/manifest.json" << 'JSON'
{"manifest_version": 3, "name": "Check Safe Start", "version": "1", "action": {}}
JSON
printf '%s\n' 'body { background: rgb(0, 0, 128) !important; }' > "$profile_dir/Default/Boosts/$host.css"
mark="$profile_dir/Last launch"
ublock=oghkpboejbohceocapipmnjifoklonij
boosted="^browser: title [0-9]* body rgb(0, 0, 128)\$"

# Launches the app on the run's profile with the extension, logging to $1.
safe_launch() {
  log="$out/$1.log"
  step=0
  BROWSER_DRIVE=1 BROWSER_URL="${site}safe.html" BROWSER_PROFILE_DIR="$profile_dir" \
    "$browser" --load-extension="$(app_path "$safe_dir/extension")" \
    < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await_load
}
# The launch counted $1 crashes at launch before it, and said whether it's safe.
launch_said() {
  await "^browser: safe start $2 after $1 crashes at launch\$" ||
    fail "the launch after $1 crashes at launch didn't say safe start $2: $(grep '^browser: safe start' "$log")"
  [ "$(cat "$mark")" = "launching $1" ] || fail "the launch mark says \"$(cat "$mark")\", not launching $1"
}
# Whether the launch's log has a line matching $1 anywhere, as these launches send no command before.
in_log() {
  grep -q "$1" "$log"
}
# A launch that loaded the extension and the Boost, as normal. The ad blocker is Settings › Ad
# blocking, never an extension list's (#2113), so the menu lists the check's extension alone.
loaded_all() {
  await '^browser: extensions menu [a-p]\{32\}$' ||
    fail "the Extensions menu didn't list Check Safe Start: $(grep '^browser: extensions menu' "$log")"
  in_log "^browser: extensions menu .*$ublock" &&
    fail "the Extensions menu listed the ad blocker: $(grep '^browser: extensions menu' "$log" | tail -1)"
  await "$boosted" || fail "the site's Boost didn't show: $(grep '^browser: title' "$log" | tail -1)"
  in_log 'browser: safe start holds' && fail "the core held extensions in a normal launch"
  in_log '^browser: safe start card shown$' && fail "a normal launch showed the safe-start card"
  return 0
}
# Kills the browser in its first minute, as a crash at launch ends it, and waits for its processes.
crash_at_launch() {
  exec 3>&-
  if windows; then
    taskkill //F //T //PID "$(cat "/proc/$pid/winpid")" > /dev/null
  else
    kill -KILL "$(app_pid)"
  fi
  wait "$pid" 2> /dev/null || true
  for _ in $(seq 20); do
    pgrep -f "$procs" > /dev/null || return 0
    sleep 0.5
  done
  fail "Chromium's processes outlived the browser killed at launch"
}

# The first launch is a normal one, and so is the one after a single crash at launch.
safe_launch safe-start-first
launch_said 0 off
loaded_all
crash_at_launch
safe_launch safe-start-one-crash
launch_said 1 off
loaded_all
crash_at_launch

# The second crash at launch in a row starts the third launch safe.
safe_launch safe-start-held
launch_said 2 on
held_log=$log
await '^browser: safe start card shown$' || fail "the safe-start card didn't show"
await '^browser: safe start button 0 at ' || fail "the safe-start card's Turn Back On wasn't drawn"
await '^browser: boosts of profile [0-9]* read: 1$' || fail "the app didn't read the profile's Boost"
in_log '^browser: boost shown ' && fail "a Boost showed in a safe start: $(grep '^browser: boost shown' "$log")"
await '^browser: title [0-9]* body rgb(51, 51, 51)$' ||
  fail "the page wasn't its own in a safe start: $(grep '^browser: title' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/safe-start.png" || fail "no window to capture"

# Turn Back On restarts it, into a normal launch.
read -r bx by bw bh << EOF
$(grep '^browser: safe start button 0 at ' "$log" | tail -1 | cut -d' ' -f7-)
EOF
# Posted: the app quits before the driver could say it's done.
post click $((bx + bw / 2)) $((by + bh / 2))
expect '^browser: command turn-back-on via sidebar$' "Turn Back On didn't take the click"
exec 3>&-
old=$pid
for _ in $(seq 300); do
  kill -0 "$old" 2> /dev/null || break
  sleep 0.1
done
kill -0 "$old" 2> /dev/null && fail "Turn Back On didn't quit the browser"
wait "$old" || fail "the browser Turn Back On quit exited with status $?"
in_log '^browser: stopping$' || fail "Turn Back On ended the browser without quitting it"
relaunched=
for _ in $(seq 300); do
  relaunched=$(pgrep -n -f "^$browser" || true)
  [ -n "$relaunched" ] && [ "$relaunched" != "$old" ] && break
  relaunched=
  sleep 0.1
done
[ -n "$relaunched" ] || fail "the browser didn't come back after Turn Back On"
# It logs where the restart sends it, not here: its mark says it's a normal launch.
for _ in $(seq 300); do
  [ "$(cat "$mark" 2> /dev/null)" = "launching 0" ] && break
  sleep 0.1
done
[ "$(cat "$mark")" = "launching 0" ] ||
  fail "the launch Turn Back On started says \"$(cat "$mark")\", not launching 0"
pid=$relaunched
quit_app
for _ in $(seq 100); do
  kill -0 "$pid" 2> /dev/null || break
  sleep 0.1
done
kill -0 "$pid" 2> /dev/null && fail "the browser Turn Back On started didn't quit"
[ "$(cat "$mark")" = "quit" ] || fail "the quit left the mark \"$(cat "$mark")\""

# The next launch loads them all again.
safe_launch safe-start-back
launch_said 0 off
loaded_all
exec 3>&-
quit

# What the safe launch held, checked last, from its whole log: the core holds none of the command
# line's extensions, the run's and the ad blocker, until #2631 (#2629), and these steps are
# quarantined on the Mac (quarantine.mac) while the rest of the run is checked as before.
grep -q 'browser: safe start holds 2 extensions$' "$held_log" ||
  fail "the core didn't hold Check Safe Start and the ad blocker: $(grep 'safe start holds' "$held_log")"
grep '^browser: extensions menu' "$held_log" | grep -q '[a-p]' &&
  fail "the Extensions menu listed an extension in a safe start: $(grep '^browser: extensions menu' "$held_log" | tail -1)"
echo "ok: safe start held the extension, the ad blocker and the Boost after two crashes at launch," \
  "showed its card ($out/safe-start.png), and Turn Back On restarted into a normal launch"
