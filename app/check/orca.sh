# check.sh runs this at 306, only when named.
# What Orca says as a screen-reader user moves through ilo on Linux, from a new profile's first
# launch (#3226): the onboarding's steps, Tab and ⇧Tab in them, then Tab and the arrows in the
# sidebar of the Space it leaves. It records and judges nothing yet. Orca follows the focus AT-SPI
# reports, so what it says after each key is what ilo told it had the keyboard.
# Orca writes what it would say to its debug log, "SPEECH OUTPUT: …", with or without a speech
# server, and where it takes the focus to be, "FOCUS MANAGER: …". It buffers that log, so the run
# marks the time of each step and splits the log by them once Orca has quit: orca-speech.txt is
# what each step made Orca say and where its focus went, and orca-debug.log all of it.
# Orca comes from the distribution's package: on a hosted runner without it the run installs it
# with apt, and elsewhere without it it skips. It reads ilo's tree over AT-SPI on buses of the
# run's own, as access does, so it never reads the desktop's other apps.

if [ "$(uname)" != Linux ]; then
  echo "skip all: Orca reads Linux only; screen-reader runs NVDA on Windows"
  return 0
fi
orca=$(command -v orca || true)
if [ -z "$orca" ] && [ "${RUNNER_ENVIRONMENT:-}" = github-hosted ] && sudo -n true 2> /dev/null; then
  # Again after an update, where the runner's package lists are older than the mirror's.
  { sudo -n env DEBIAN_FRONTEND=noninteractive timeout 900 apt-get install -y -q orca ||
    { sudo -n timeout 600 apt-get update -q &&
      sudo -n env DEBIAN_FRONTEND=noninteractive timeout 900 apt-get install -y -q orca; }; } \
    > "$out/orca-apt.log" 2>&1 || fail "Orca didn't install: $(tail -5 "$out/orca-apt.log")"
  orca=$(command -v orca || true)
fi
if [ -z "$orca" ]; then
  echo "skip all: no Orca: install the orca package; a hosted runner installs it"
  return 0
fi

# The app's session bus, whose at-spi-bus-launcher starts AT-SPI's, and AT-SPI's registry, as in
# access: GPUI publishes its tree only while that session bus says AT-SPI is on.
started=$(dbus-daemon --session --fork --print-address=1 --print-pid=1 2> /dev/null | tr '\n' ' ')
session=$(echo "$started" | cut -d' ' -f1)
buses=$(echo "$started" | cut -d' ' -f2)
a11y=
[ -z "$session" ] || a11y=$(DBUS_SESSION_BUS_ADDRESS=$session busctl --user call \
  org.a11y.Bus /org/a11y/bus org.a11y.Bus GetAddress 2> /dev/null | sed -n 's/^s "\(.*\)"$/\1/p')
registry=
for each in /usr/libexec/at-spi2-registryd /usr/lib/at-spi2-registryd \
  /usr/lib/at-spi2-core/at-spi2-registryd; do
  [ -x "$each" ] && registry=$each && break
done
if [ -z "$a11y" ] || [ -z "$registry" ]; then
  kill $buses 2> /dev/null || true
  buses=
  echo "skip all: no AT-SPI bus answers, as dbus-daemon or at-spi2-core isn't installed"
  return 0
fi
AT_SPI_BUS_ADDRESS=$a11y "$registry" > "$out/orca-registry.log" 2>&1 &
buses="$buses $!"
for _ in $(seq 50); do
  busctl --address="$a11y" status org.a11y.atspi.Registry > /dev/null 2>&1 && break
  sleep 0.1
done
DBUS_SESSION_BUS_ADDRESS=$session busctl --user set-property org.a11y.Bus /org/a11y/bus \
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
DBUS_SESSION_BUS_ADDRESS=$session AT_SPI_BUS_ADDRESS=$a11y XDG_CONFIG_HOME="$out/orca-config" \
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
' "$orca" -u "$out/orca-prefs" --debug-file "$orca_log" > "$out/orca.log" 2>&1 &
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

marks="$out/orca-marks.txt"
: > "$marks"
# Until Orca's log has stopped growing for 0.6 s, up to 5 s, so the next key comes after what the
# last one made Orca say. A record's pace, not a judge: the log is buffered, and the split after
# Orca quits is by time.
settled() {
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
mark() {
  echo "$(date '+%H:%M:%S.%6N') $1" >> "$marks"
}
press() {
  mark "key $*"
  send key "$@"
  settled
}

log="$out/orca-ilo.log"
step=0
rm -f "$saved" "$profile_dir/Account"
# An empty home for the import step, as a new user's may be, and never the runner's own browsers.
mkdir -p "$out/orca-home"
mark launch
DBUS_SESSION_BUS_ADDRESS=$session BROWSER_IMPORT_HOME="$out/orca-home" BROWSER_DRIVE=1 \
  BROWSER_ONBOARDING=light "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: onboarding step intro settled$' || fail "the onboarding's intro didn't draw"
settled
await '^browser: onboarding left intro ' || fail "the intro didn't go on by itself"
await '^browser: onboarding step [a-z-]* settled$' || fail "no step came after the intro"
mark "$(grep '^browser: onboarding step [a-z-]* settled$' "$log" | tail -1 | cut -d' ' -f4) step"
settled
press tab
press tab
press tab shift
press down
# On through the steps as Esc goes, each one's speech recorded as it comes.
for _ in $(seq 10); do
  grep -q '^browser: onboarding done$' "$log" && break
  press escape
done
expect '^browser: command bar shown$' "the command bar didn't come up after the onboarding"
mark "command bar after the onboarding"
settled
press escape
# The Space the onboarding leaves, its sidebar from Tab.
press tab
press tab
press tab shift
press down
press down
press up
mark end
exec 3>&-
quit
orca_quit
run_undo=
kill $buses 2> /dev/null || true
buses=

# Each step and, until the next, what Orca said and where it took the focus to be.
speech="$out/orca-speech.txt"
awk -v marks="$marks" '
  BEGIN { while ((getline line < marks) > 0) { n++; at[n] = substr(line, 1, 15); what[n] = substr(line, 17) } }
  /^[0-9][0-9]:[0-9][0-9]:[0-9][0-9]\.[0-9]* - / {
    time = substr($0, 1, 15)
    while (i < n && time >= at[i + 1]) { i++; print "== " what[i] }
    if ($0 ~ / - (SPEECH OUTPUT:|SPEECH: Not available|SPEECH: Using speech server|FOCUS MANAGER: Request to set locus of focus)/) print substr($0, 19)
  }
  END { while (i < n) { i++; print "== " what[i] } }' "$orca_log" > "$speech"
said=$(grep -c '^SPEECH OUTPUT:' "$speech" || true)
echo "ok: recorded what Orca said through the onboarding and the sidebar, $said utterances: $speech"
