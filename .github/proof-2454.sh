#!/bin/bash
# Proof only (#2454): crashes each kind of process of the released Flatpak and counts Crashpad's
# minidumps after each, then tests reading memory across processes in its sandbox without ptrace.
set -u
out=$RUNNER_TEMP/proof
mkdir -p "$out"
summary=$out/summary
say() { echo "$*" | tee -a "$summary"; }
say "kernel $(uname -r), yama ptrace_scope $(cat /proc/sys/kernel/yama/ptrace_scope)"
crashpad=$HOME/.var/app/app.ilo.browser/config/ilo/Crashpad
dumps() { find "$crashpad" -name '*.dmp' 2> /dev/null | wc -l; }

# The app's processes as the host numbers them: kind, namespaces, seccomp.
procs() {
  for p in $(pgrep -f '^/app/ilo/chrome'); do
    # Chromium's processes set their title, which puts their arguments in one string.
    args=$(tr '\0' ' ' < "/proc/$p/cmdline" 2> /dev/null) || continue
    kind=$(printf '%s\n' "$args" | grep -o -- '--type=[a-z-]*' | head -1 | cut -d= -f2)
    case $args in *chrome_crashpad_handler*) kind=handler ;; esac
    case $args in *--extension-process*) kind="$kind(extension)" ;; esac
    echo "$p ${kind:-browser} pidns=$(readlink "/proc/$p/ns/pid") userns=$(readlink "/proc/$p/ns/user")" \
      "$(grep -E '^(Seccomp|NoNewPrivs|PPid|NSpid):' "/proc/$p/status" | tr '\t\n' '  ')"
  done
}
pid_of() {
  procs | awk -v k="$1" '$2 == k { print $1; exit }'
}

# Starts the Flatpak with the arguments after $1, its log $out/$1.log, and waits for its UI and a
# page's renderer.
launch() {
  name=$1
  shift
  flatpak run "$@" app.ilo.browser > "$out/$name.log" 2>&1 &
  fpid=$!
  for _ in $(seq 120); do
    grep -q 'browser: UI shown' "$out/$name.log" && break
    sleep 1
  done
  for _ in $(seq 30); do
    [ -n "$(pid_of renderer)" ] && break
    sleep 1
  done
  sleep 3
  procs > "$out/$name-procs.txt"
  say "$name: UI shown $(grep -c 'browser: UI shown' "$out/$name.log"), processes:"
  sed 's/^/  /' "$out/$name-procs.txt" | tee -a "$summary"
}
stop() {
  flatpak kill app.ilo.browser 2> /dev/null || true
  wait "$fpid" 2> /dev/null || true
  sleep 2
}
# Sends signal $3 to $2, the $1, and counts the minidumps 15 s later.
crash() {
  if [ -z "$2" ]; then
    say "$1: no such process"
    return
  fi
  before=$(dumps)
  kill "-$3" "$2"
  sleep 15
  say "$1 (pid $2) sent SIG$3: minidumps $before -> $(dumps); alive after: $(kill -0 "$2" 2> /dev/null && echo yes || echo no)"
}

page='data:text/html,<title>probe</title><h1>probe</h1>'
launch signals --env=BROWSER_URL="$page"
crash renderer "$(pid_of renderer)" SEGV
crash gpu-process "$(pid_of gpu-process)" SEGV
crash "browser, as a panic's abort ends" "$(pid_of browser)" ABRT
stop

launch browser-segv --env=BROWSER_URL="$page"
crash browser "$(pid_of browser)" SEGV
stop

# Chromium's own crash pages, faults in the process itself rather than a signal from outside.
for url in chrome://crash chrome://gpucrash chrome://inducebrowsercrashforrealz; do
  name=url-${url#chrome://}
  before=$(dumps)
  launch "$name" --env=BROWSER_URL="$url"
  sleep 10
  say "$name: minidumps $before -> $(dumps); loaded: $(grep -c "loaded [0-9]* $url" "$out/$name.log"); crashed: $(grep -c -i 'crash' "$out/$name.log")"
  stop
done

say "Crashpad's lines:"
grep -h -i -e crashpad -e ptrace "$out"/*.log | sort | uniq -c | sed 's/^/  /' | tee -a "$summary"
say "Crashpad's folder:"
find "$crashpad" -type f 2> /dev/null | sed 's/^/  /' | tee -a "$summary"

say "memory without ptrace, in the sandbox:"
flatpak run --filesystem="$PWD/.github:ro" --command=python3 app.ilo.browser \
  "$PWD/.github/proof-2454-mem.py" 2>&1 | tee -a "$summary"

say "memory of a sub-sandboxed child, as a renderer is:"
flatpak run --filesystem="$PWD/.github:ro" --command=python3 app.ilo.browser \
  "$PWD/.github/proof-2454-sub.py" 2>&1 | tee -a "$summary"
exit 0
