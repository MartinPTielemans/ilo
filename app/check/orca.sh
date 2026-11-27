# check.sh runs this at 306, only when named.
# What Orca says as a screen-reader user moves through ilo on Linux, from a new profile's first
# launch (#3226): the onboarding's steps, Tab and ⇧Tab in them, then Tab and the arrows in the
# sidebar of the Space it leaves, and F6 and the arrows along two pages' rows. It records and
# judges nothing yet. Orca follows the focus AT-SPI
# reports, so what it says after each key is what ilo told it had the keyboard.
# Orca writes what it would say to its debug log, "SPEECH OUTPUT: …", with or without a speech
# server, and where it takes the focus to be, "FOCUS MANAGER: …". The run marks the time of each
# step and splits the log by them once Orca has quit: orca-speech.txt is what each step made Orca
# say and where its focus went, and orca-debug.log all of it.
# Runs named together share one shell, so its names start with orca_, but for buses, which lib.sh's
# cleanup reads to stop the buses as it does access's.
# Orca comes from the distribution's package: on a hosted runner without it the run installs it
# with apt, and elsewhere without it it skips. It reads ilo's tree over AT-SPI on buses of the
# run's own, as access does, so it never reads the desktop's other apps.

if [ "$(uname)" != Linux ]; then
  echo "skip all: Orca reads Linux only; screen-reader runs NVDA on Windows"
  return 0
fi
orca_bin=$(command -v orca || true)
if [ -z "$orca_bin" ] && [ "${RUNNER_ENVIRONMENT:-}" = github-hosted ] && sudo -n true 2> /dev/null; then
  # Again after an update, where the runner's package lists are older than the mirror's.
  { sudo -n env DEBIAN_FRONTEND=noninteractive timeout 900 apt-get install -y -q orca ||
    { sudo -n timeout 600 apt-get update -q &&
      sudo -n env DEBIAN_FRONTEND=noninteractive timeout 900 apt-get install -y -q orca; }; } \
    > "$out/orca-apt.log" 2>&1 || fail "Orca didn't install: $(tail -5 "$out/orca-apt.log")"
  orca_bin=$(command -v orca || true)
fi
if [ -z "$orca_bin" ]; then
  echo "skip all: no Orca: install the orca package; a hosted runner installs it"
  return 0
fi

# The app's session bus, whose at-spi-bus-launcher starts AT-SPI's, and AT-SPI's registry, as in
# access: GPUI publishes its tree only while that session bus says AT-SPI is on.
orca_started=$(dbus-daemon --session --fork --print-address=1 --print-pid=1 2> /dev/null | tr '\n' ' ')
orca_session=$(echo "$orca_started" | cut -d' ' -f1)
buses=$(echo "$orca_started" | cut -d' ' -f2)
orca_a11y=
[ -z "$orca_session" ] || orca_a11y=$(DBUS_SESSION_BUS_ADDRESS=$orca_session busctl --user call \
  org.a11y.Bus /org/a11y/bus org.a11y.Bus GetAddress 2> /dev/null | sed -n 's/^s "\(.*\)"$/\1/p')
orca_registry=
for orca_each in /usr/libexec/at-spi2-registryd /usr/lib/at-spi2-registryd \
  /usr/lib/at-spi2-core/at-spi2-registryd; do
  [ -x "$orca_each" ] && orca_registry=$orca_each && break
done
if [ -z "$orca_a11y" ] || [ -z "$orca_registry" ]; then
  kill $buses 2> /dev/null || true
  buses=
  echo "skip all: no AT-SPI bus answers, as dbus-daemon or at-spi2-core isn't installed"
  return 0
fi
AT_SPI_BUS_ADDRESS=$orca_a11y "$orca_registry" > "$out/orca-registry.log" 2>&1 &
buses="$buses $!"
for _ in $(seq 50); do
  busctl --address="$orca_a11y" status org.a11y.atspi.Registry > /dev/null 2>&1 && break
  sleep 0.1
done
DBUS_SESSION_BUS_ADDRESS=$orca_session busctl --user set-property org.a11y.Bus /org/a11y/bus \
  org.a11y.Status IsEnabled b true || fail "AT-SPI's bus didn't turn on"

# Orca with settings of its own, its whole debug log in the run's folder. Orca opens that log
# block-buffered, and what was still in the buffer when it was stopped never reached the file
# (44431039438's ended before the onboarding's first step), so its open gives the log line
# buffering.
# No speech server either: on the hosted runner, with no sound, the speech-dispatcher Orca started
# never answered, and Orca waited on it from the first thing it said (44433125069). Without one,
# Orca still writes each thing it would say, so its own config refuses to start one.
orca_log="$out/orca-debug.log"
rm -rf "$out/orca-prefs" "$out/orca-config" "$orca_log"
mkdir -p "$out/orca-prefs" "$out/orca-config/speech-dispatcher"
echo DisableAutoSpawn > "$out/orca-config/speech-dispatcher/speechd.conf"
DBUS_SESSION_BUS_ADDRESS=$orca_session AT_SPI_BUS_ADDRESS=$orca_a11y XDG_CONFIG_HOME="$out/orca-config" \
  SPEECHD_ADDRESS="unix_socket:$out/orca-config/speechd.sock" python3 -I -c '
import builtins, runpy, sys
opened = builtins.open
def line_buffered(file, mode="r", *args, **kwargs):
    if file == sys.argv[-1] and "w" in mode:
        return opened(file, mode, 1)
    return opened(file, mode, *args, **kwargs)
builtins.open = line_buffered
path = sys.argv[1]
sys.argv = [path] + sys.argv[2:]
runpy.run_path(path, run_name="__main__")
' "$orca_bin" -u "$out/orca-prefs" --debug-file "$orca_log" > "$out/orca.log" 2>&1 &
orca_pid=$!
orca_quit() {
  [ -n "${orca_pid:-}" ] || return 0
  kill "$orca_pid" 2> /dev/null || true
  for _ in $(seq 50); do
    kill -0 "$orca_pid" 2> /dev/null || break
    sleep 0.1
  done
  kill -9 "$orca_pid" 2> /dev/null || true
  orca_pid=
}
run_undo=orca_quit
for _ in $(seq 300); do
  grep -q 'SPEECH: Initialized' "$orca_log" 2> /dev/null && break
  kill -0 "$orca_pid" 2> /dev/null || fail "Orca quit as it started: $(tail -5 "$out/orca.log")"
  sleep 0.1
done

orca_marks="$out/orca-marks.txt"
: > "$orca_marks"
# Until Orca's log has stopped growing for 0.6 s, up to 5 s, so the next key comes after what the
# last one made Orca say. That's a record's pace only: a step that judges waits for a line of
# Orca's or ilo's own instead.
orca_settled() {
  local size last=-1 still=0
  for _ in $(seq 50); do
    size=$(wc -c < "$orca_log" 2> /dev/null || echo 0)
    if [ "$size" = "$last" ]; then
      still=$((still + 1))
      [ "$still" -lt 6 ] || break
    else
      still=0
      last=$size
    fi
    sleep 0.1
  done
}
# Step $1 starts now, in Orca's clock.
orca_mark() {
  echo "$(date '+%H:%M:%S.%6N') $1" >> "$orca_marks"
}
orca_press() {
  orca_mark "key $*"
  send key "$@"
  orca_settled
}

log="$out/orca-ilo.log"
step=0
rm -f "$saved" "$profile_dir/Account"
# An empty home for the import step, as a new user's may be, and never the runner's own browsers.
mkdir -p "$out/orca-home"
orca_mark launch
# A count endpoint nothing listens on, so the last step offers Help improve's switch, as the prod
# build's does, while it stays off and nothing is sent.
DBUS_SESSION_BUS_ADDRESS=$orca_session BROWSER_IMPORT_HOME="$out/orca-home" BROWSER_DRIVE=1 \
  BROWSER_ONBOARDING=light "$browser" --daily-count-url=http://127.0.0.1:9/v1/daily \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: onboarding started$' || fail "the onboarding didn't start"
# On through the card's steps, each left the way that takes nothing it offers (pass_step), each
# one's speech recorded as it comes, marked once Orca is quiet on the step before: the next thing is
# the key or click leaving it, and Orca speaks within milliseconds of that, sooner than a wait for
# the app's line sees it. Your first Space (4), which every run shows, rests on its swatches, where
# ⇥ goes on to Back and Next, ⇧⇥ back, and → picks a colour; step 6, the last, rests on
# Start Browsing, where ⇥ goes round its row's button and Help improve's switch, and ⇧⇥ back. Each
# goes back as far as it went, so the step's ↩ is still its main button's.
for orca_step in $(onboarding_steps); do
  step_settled "$orca_step" "the onboarding's step $orca_step didn't draw"
  orca_settled
  case $orca_step in
    4)
      orca_press tab
      orca_press tab
      orca_press tab shift
      orca_press tab shift
      orca_press right
      ;;
    7)
      orca_press tab
      orca_press tab
      orca_press tab
      orca_press tab shift
      orca_press tab shift
      orca_press tab shift
      ;;
  esac
  orca_mark "leave $orca_step"
  pass_step "$orca_step"
done
expect '^browser: onboarding done$' "the onboarding didn't end after its last step"
orca_settled
expect '^browser: command bar shown$' "the command bar didn't come up after the onboarding"
orca_mark "command bar after the onboarding"
orca_settled
orca_press escape
# The Space the onboarding leaves, its sidebar from Tab.
orca_press tab
orca_press tab
orca_press tab shift
orca_press down
orca_press down
orca_press up
# Two pages in the Space, so F6 and the arrows have rows to move along: the empty Space's arrows
# said nothing, having none (#3226).
orca_mark "two pages opened"
for orca_title in First Second; do
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar for the $orca_title page"
  send type "data:text/html,<title>$orca_title</title><p>$orca_title</p>"
  send key return
  expect '^browser: loaded [0-9]* data:text/html' "the $orca_title page didn't load"
done
orca_settled
# From the page to the sidebar's rows and along them, and out again.
orca_press f6
orca_press down
orca_press down
orca_press up
orca_press escape
# F10 from the page rings the footer's menu control (#4030), and F10 again leaves it. The window is
# captured before and with the ring, for the PR's record of how it looks on Linux.
../tools/window-screenshot.sh "$pid" "$out/f10-before.png" || fail "no window to capture"
orca_press f10
expect '^browser: menu ring at ' "F10 from the page didn't draw the menu control's ring"
../tools/window-screenshot.sh "$pid" "$out/f10-ring.png" || fail "no window to capture"
orca_press f10
orca_mark end
exec 3>&-
quit
orca_quit
run_undo=
kill $buses 2> /dev/null || true
buses=

# Each step and, until the next, what Orca said and where it took the focus to be.
orca_speech="$out/orca-speech.txt"
awk -v marks="$orca_marks" '
  BEGIN { while ((getline line < marks) > 0) { n++; at[n] = substr(line, 1, 15); what[n] = substr(line, 17) } }
  /^[0-9][0-9]:[0-9][0-9]:[0-9][0-9]\.[0-9]* - / {
    time = substr($0, 1, 15)
    while (i < n && time >= at[i + 1]) { i++; print "== " what[i] }
    if ($0 ~ / - (SPEECH OUTPUT:|SPEECH: Not available|SPEECH: Using speech server|FOCUS MANAGER: Request to set locus of focus)/) print substr($0, 19)
  }
  END { while (i < n) { i++; print "== " what[i] } }' "$orca_log" > "$orca_speech"
orca_said=$(grep -c '^SPEECH OUTPUT:' "$orca_speech" || true)
echo "ok: recorded what Orca said through the onboarding and the sidebar, $orca_said utterances: $orca_speech"
