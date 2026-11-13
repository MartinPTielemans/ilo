# check.sh runs this at 710, only when named.
# It installs the release ILO_OLD names as ilo's Flatpak, on Linux under Xvfb, so it runs only
# when named.
# An older ilo beside this one on one account (#2108): this build is A, and B is the release
# ILO_OLD names, installed as ilo's Flatpak, app.ilo.browser, which the run drives in its sandbox,
# on an ilo-server of the run's own (ilo-server.sh) and a throwaway address. A signs in first and
# copies its recovery words, and B recovers with them. ilo 0.1.13 and older read kind TODAY,
# which they don't know, as SPACE, so a Today tab whose id they took would become a Space there,
# written back over A's. A Today tab A opens makes no Space on B, nor on A after; B's sidebar
# never shows it. Pinned on A it shows pinned on B, and unpinned, which deletes it under its bare
# id and puts it back as "today:" and its id, B closes it, and makes no Space of it either.
# Linux's alone, under Xvfb, where the Flatpak runs; named only, as it needs the release installed
# (check-linux.yml installs it when the run is named).

if [ -z "${ILO_SERVER:-}" ] || [ -z "${ILO_OLD:-}" ]; then
  echo "skip: sync-versions. It needs ILO_SERVER, an ilo-server to sign in to, and ILO_OLD, the" \
    "version of an older ilo installed as its Flatpak."
  return 0
fi
if [ "$(uname)" != Linux ] || [ "${BROWSER_HEADLESS:-xvfb}" != xvfb ]; then
  echo "skip: sync-versions. It runs under Xvfb on Linux, where ilo's Flatpak runs."
  return 0
fi
# The Flatpak installed is ILO_OLD, never another, such as one a user keeps there.
installed=$(flatpak info --user app.ilo.browser 2> /dev/null | sed -n 's/^ *Version: //p')
[ "$installed" = "$ILO_OLD" ] ||
  fail "ilo's Flatpak is ${installed:-not installed}, not $ILO_OLD as ILO_OLD says"
rm -f "$saved" "$launch_frame" "$profile_dir/Account"
. check/ilo-server.sh

# B, the older ilo in its sandbox, given the run's BROWSER_ settings and its profile, as check.sh's
# flatpak_browser does for the run's own Flatpak.
old_browser() {
  set -- app.ilo.browser "$@"
  for name in $(env | sed -n 's/^\(BROWSER_[A-Z0-9_]*\)=.*/\1/p'); do
    eval "set -- \"--env=$name=\${$name}\" \"\$@\""
  done
  count_off "$BROWSER_PROFILE_DIR"
  set -- flatpak run --user --cwd="$PWD" --filesystem="$PWD" \
    --filesystem="$BROWSER_PROFILE_DIR:create" "$@"
  # flatpak run proxies the session bus into the sandbox: the job's steps have none of their own.
  [ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ] || set -- dbus-run-session -- "$@"
  exec "$@"
}
# Drives device $1: A, this build, or B in its sandbox, whose windows carry its pid there
# (window_pid), as the run's own Flatpak's do.
drive() {
  device "$1"
  if [ "$1" = b ]; then
    BROWSER_FLATPAK=1
  else
    BROWSER_FLATPAK=
  fi
  send window 1
}
# The Spaces the device's window has, and the rows of the Space it shows, by the last list of each
# it logged.
spaces() {
  grep '^browser: spaces ' "$log" | tail -1 | awk '{ print NF - 2 }'
}
# An empty list is logged with a space after its name.
rows() {
  grep -E "^browser: $1( [0-9]+)* ?\$" "$log" | tail -1 | cut -s -d' ' -f3-
}
# Whether the device's sync has committed since its Spaces last sent a change: the core's lines
# carry Chromium's prefix.
committed() {
  awk '/browser: sync spaces sent /{ c = 0 } /browser: sync engine Commit Response/{ c = 1 }
    END { exit !c }' "$log"
}
# When the log $2 last had a core line matching $1, by Chromium's prefix, MMDD/HHMMSS.micro, which
# sorts as text: the copies share the machine's clock.
stamp() {
  grep "$1" "$2" | tail -1 | sed -n 's|^\[[0-9]*:[0-9]*:\([0-9]*/[0-9.]*\):.*|\1|p'
}
# Whether B has fetched updates since A's last commit.
fetched() {
  awk -v a="$(stamp 'browser: sync engine Commit Response' "$a_log")" \
    -v b="$(stamp 'browser: sync engine GetUpdates Response' "$b_log")" \
    'BEGIN { exit !(a != "" && b > a) }'
}
# Waits up to 30 s for the device's last list of $1 to be $2, failing with $3.
await_rows() {
  for _ in $(seq 300); do
    [ "$(rows "$1")" = "$2" ] && return 0
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.1
  done
  fail "$3: $1 $(rows "$1")"
}
await_spaces() {
  for _ in $(seq 300); do
    [ "$(spaces)" = "$1" ] && return 0
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.1
  done
  fail "$2: $(spaces) Spaces"
}
others=

start a "$profile_dir" 5
open_sign_in
sign_in first
await '^browser: account FirstDevice None$' ||
  fail "signing in didn't make A the account's first device"
settings_keyboard
at=$(drawn "sync copy") || fail "Settings › Sync didn't draw Copy with the words"
send clickkey $at
words=
for _ in $(seq 30); do
  words=$(timeout 5 xclip -selection clipboard -o 2> /dev/null || true)
  [ "$(echo "$words" | wc -w)" = 24 ] && break
  sleep 0.1
done
[ "$(echo "$words" | wc -w)" = 24 ] || fail "Copy didn't put 24 words on the clipboard"
at=$(drawn "sync done") || fail "Settings › Sync didn't draw Done with the words"
send clickkey $at
await 'browser: sync service active$' || fail "A's sync didn't become active"

browser_was=$browser
browser=old_browser
start b "$PWD/$out/sync-versions-b" 6
browser=$browser_was
BROWSER_FLATPAK=1
open_sign_in
sign_in
await '^browser: account NeedsSecret None$' || fail "B, signed in, didn't need the account's secret"
settings_keyboard
at=$(drawn "sync use-words") || fail "B's Settings › Sync didn't offer the words"
send clickkey $at
send type $words
send key return
await '^browser: account On None$' || fail "B didn't turn sync on with the recovery words"
await 'browser: sync service active$' || fail "B's sync didn't become active"
# B's own first Space, untouched, gives way to A's, as Arc's does on a new device.
await_spaces 1 "B, synced, doesn't have A's one Space"
drive b
b_today=$(rows today)

# A Today tab A opens is the account's as "today:" and its id, which B drops.
serve
drive a
sends=$(grep -c 'browser: sync spaces sent ' "$log" || true)
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open A's command bar"
send insert "${site}login.html?versions"
send key return
expect "^browser: loaded [0-9]* ${site}login.html?versions\$" "A's Today tab didn't load"
tab=$(sed -n "s|^browser: loaded \([0-9]*\) ${site}login.html?versions\$|\1|p" "$log" | tail -1)
# B fetches the Today tab before A pins it: pinned, it's another entity now, but was the same one,
# under the same id, before 0.1.14, which the account would have had as a pin alone.
for _ in $(seq 50); do
  [ "$(grep -c 'browser: sync spaces sent ' "$log")" -gt "$sends" ] && committed && break
  sleep 0.1
done
[ "$(grep -c 'browser: sync spaces sent ' "$log")" -gt "$sends" ] && committed ||
  fail "A didn't commit its Today tab to the account"
drive b
for _ in $(seq 50); do
  fetched && break
  sleep 0.1
done
fetched || fail "B fetched nothing after A committed its Today tab"
[ "$(spaces)" = 1 ] || fail "B, $ILO_OLD, made a Space of A's Today tab: $(spaces) Spaces"
[ "$(rows today)" = "$b_today" ] ||
  fail "B's Today tabs changed with A's: $(rows today), from $b_today"
drive a

# Pinned on A, it's B's pinned tab, which says B has had what A sent before it too.
send key d cmd
expect '^browser: command toggle-pin via ' "⌘D didn't pin A's Today tab"
drive b
for _ in $(seq 300); do
  [ -n "$(rows pinned)" ] && break
  sleep 0.1
done
b_pinned=$(rows pinned)
[ -n "$b_pinned" ] || fail "the tab A pinned didn't reach B, so B's sync never had A's changes"
[ "$(spaces)" = 1 ] || fail "B, $ILO_OLD, made a Space of A's Today tab: $(spaces) Spaces"
[ "$(rows today)" = "$b_today" ] ||
  fail "B's Today tabs changed with A's: $(rows today), from $b_today"

# Unpinned on A, it's deleted under its bare id and put back as "today:" and its id: B closes it
# and makes nothing of the Today tab.
drive a
send key d cmd
expect '^browser: command toggle-pin via ' "⌘D didn't unpin A's tab"
for _ in $(seq 50); do
  rows today | tr ' ' '\n' | grep -qx "$tab" && break
  sleep 0.1
done
rows today | tr ' ' '\n' | grep -qx "$tab" ||
  fail "A's unpinned tab isn't a Today tab: $(rows today)"
drive b
await_rows pinned "" "B kept the tab A unpinned pinned"
# What B would write back reaches A within 2.5 s, as the account's other changes do (sync-devices).
sleep 5
[ "$(spaces)" = 1 ] || fail "B, $ILO_OLD, made a Space of A's unpinned tab: $(spaces) Spaces"
[ "$(rows today)" = "$b_today" ] ||
  fail "B's Today tabs changed with A's: $(rows today), from $b_today"
../tools/window-screenshot.sh "$(app_pid)" "$out/sync-versions-b.png" || fail "no window to capture"
drive a
[ "$(spaces)" = 1 ] || fail "A has $(spaces) Spaces: B wrote a Space back"
rows today | tr ' ' '\n' | grep -qx "$tab" || fail "A's Today tab went: $(rows today)"
../tools/window-screenshot.sh "$pid" "$out/sync-versions-a.png" || fail "no window to capture"

device b
exec 3>&-
flatpak kill app.ilo.browser 2> /dev/null || true
for _ in $(seq 100); do
  kill -0 "$pid" 2> /dev/null || break
  sleep 0.1
done
exec 6>&-
others=
BROWSER_FLATPAK=
device a
exec 5>&- 3>&-
quit
# The runs after start signed out.
rm -f "$profile_dir/Account"
kill "$ilo_server" 2> /dev/null || true
ilo_server=
echo "ok: ilo $ILO_OLD, recovered with A's words on one account, made no Space of the Today tab" \
  "A opened, kept its own Today tabs, showed the tab pinned while A had it pinned and closed it" \
  "once A unpinned it, and A kept its one Space and the Today tab, on a local ilo-server as" \
  "$email. Captures: $out/sync-versions-a.png $out/sync-versions-b.png"
