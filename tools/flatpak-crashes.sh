#!/bin/bash
# Crashes each kind of process of the installed Flatpak on purpose and checks that each crash leaves
# a minidump in the profile's Crashpad folder (#2454): a renderer, the GPU process and the browser,
# each with SIGSEGV, and the browser with SIGABRT, as a Rust panic ends, since the prod app has no
# way to start one. The signals come from outside, as Chromium's crash pages don't load from a
# launch argument in the prod app. Each minidump must be of that kind of process, as its ptype
# annotation says, and hold its threads' stacks. The profile is new, so Send Crash Reports is off
# and nothing leaves the machine.
# Usage: flatpak-crashes.sh <out>, in a session with a display, as flatpak.yml runs it.
set -eu
out=$1
mkdir -p "$out"
crashpad=$HOME/.var/app/app.ilo.browser/config/ilo/Crashpad
check=$(cd "$(dirname "$0")/../app/check" && pwd)
fail() {
  echo "FAIL: $*" >&2
  exit 1
}
pending() {
  find "$crashpad/pending" -name '*.dmp' 2> /dev/null | wc -l
}

# The app's processes as the host numbers them, each with its kind. Chromium's set their title,
# which puts their arguments in one string.
procs() {
  for p in $(pgrep -f '^/app/ilo/chrome'); do
    args=$(tr '\0' ' ' < "/proc/$p/cmdline" 2> /dev/null) || continue
    kind=$(printf '%s\n' "$args" | grep -o -- '--type=[a-z-]*' | head -1 | cut -d= -f2)
    case $args in *chrome_crashpad_handler*) kind=handler ;; *--extension-process*) kind=extension ;; esac
    echo "$p ${kind:-browser}"
  done
}
pid_of() {
  procs | awk -v k="$1" '$2 == k { print $1; exit }'
}

# Starts the Flatpak on a page of its own, logging to $out/$1.log, and waits for its UI and the
# page's renderer.
page='data:text/html,<title>crash</title><h1>crash</h1>'
launch() {
  log=$out/$1.log
  flatpak run --env=BROWSER_URL="$page" app.ilo.browser > "$log" 2>&1 &
  fpid=$!
  for _ in $(seq 120); do
    grep -q '^browser: UI shown' "$log" && break
    kill -0 "$fpid" 2> /dev/null || fail "the Flatpak exited before its UI showed: $(cat "$log")"
    sleep 1
  done
  grep -q '^browser: UI shown' "$log" || fail "the Flatpak's UI didn't show in 120 s"
  for _ in $(seq 60); do
    [ -n "$(pid_of renderer)" ] && [ -n "$(pid_of gpu-process)" ] && return 0
    sleep 1
  done
  fail "the Flatpak started no renderer and GPU process: $(procs | tr '\n' ' ')"
}
stop() {
  flatpak kill app.ilo.browser 2> /dev/null || true
  wait "$fpid" 2> /dev/null || true
}

# Sends signal $2 to the $1 and checks that a minidump of it comes within 30 s.
crash() {
  kind=$1
  pid=$(pid_of "$kind")
  [ -n "$pid" ] || fail "no $kind to crash: $(procs | tr '\n' ' ')"
  before=$(pending)
  kill "-$2" "$pid"
  for _ in $(seq 60); do
    [ "$(pending)" -gt "$before" ] && break
    sleep 0.5
  done
  [ "$(pending)" -gt "$before" ] || fail "the $kind's SIG$2 left no minidump in $crashpad/pending"
  dump=$(ls -t "$crashpad"/pending/*.dmp | head -1)
  # Its process's kind and how many of its threads have their stack, from the minidump.
  read -r ptype stacks threads <<< "$(python3 -I - "$check" "$dump" << 'EOF'
import struct, sys
sys.path.insert(0, sys.argv[1])
from collector import annotations
dump = open(sys.argv[2], "rb").read()
if not dump.startswith(b"MDMP"):
    print("none 0 0")
    sys.exit()
ptype = dict(annotations(dump)).get("ptype", "none")
streams, directory = struct.unpack_from("<II", dump, 8)
threads = stacks = 0
for i in range(streams):
    kind, _, rva = struct.unpack_from("<III", dump, directory + 12 * i)
    if kind == 3:  # ThreadListStream: a count, then MINIDUMP_THREADs of 48 bytes.
        threads = struct.unpack_from("<I", dump, rva)[0]
        stacks = sum(struct.unpack_from("<I", dump, rva + 4 + 48 * t + 32)[0] > 0
                     for t in range(threads))
print(ptype, stacks, threads)
EOF
)"
  [ "$ptype" = "$kind" ] || fail "the $kind's SIG$2 left a minidump of '$ptype', $dump"
  [ "$stacks" -gt 0 ] || fail "the $kind's minidump has no thread's stack, of $threads threads"
  echo "ok: the $kind's SIG$2 left a minidump, $stacks of its $threads threads' stacks in it"
}

launch crashes
crash renderer SEGV
crash gpu-process SEGV
crash browser SEGV
stop
launch abort
crash browser ABRT
stop
! grep -h '^browser: crash report sent' "$out"/*.log ||
  fail "a report was sent, though Send Crash Reports is off in a new profile"
grep -h 'without ptrace' "$out"/*.log | sort | uniq -c || true
