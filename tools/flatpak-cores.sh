#!/bin/bash
# Keeps where the Flatpak's app died when it dies on a signal in a CI job (#3859), as text, never
# the core itself: a core holds the process's environment and memory, and a public repository's
# artifacts are public.
#
# `catch DIR` has the kernel pipe each core to a handler in DIR, which it runs as root on the host,
# so it reaches the sandboxed app whatever the sandbox's core limit is (core(5)). The handler keeps
# only the app's own processes, which carry no --type= (Chromium's children do), and none while
# DIR/hold exists, for a step that crashes the app on purpose. It needs sudo.
#
# `read DIR OUT` writes each kept core's backtrace to OUT as text, prints its top frames, and
# deletes the core. gdb reads it against the installed runtime and app, whose binaries keep their
# symbol tables, and prints no frame's arguments or locals, so no value from memory reaches the
# text: it has the command line, signal, thread names, frames' functions and addresses.
#
# Usage: tools/flatpak-cores.sh catch DIR | tools/flatpak-cores.sh read DIR OUT
set -euo pipefail

catch() {
  local dir=$1
  mkdir -p "$dir"
  dir=$(cd "$dir" && pwd)
  cat > "$dir/handler" << EOF
#!/bin/sh
# Run by the kernel as root, with the dying process's host pid, the time and the signal.
dir=$dir
umask 077
[ -e "\$dir/hold" ] && exit 0
tr '\\0' ' ' < "/proc/\$1/cmdline" | grep -q -- '--type=' && exit 0
name=\$dir/core-\$2-\$1
cat > "\$name.core"
printf '%s\\n' "\$3" "\$(readlink "/proc/\$1/exe")" > "\$name.about"
chown $(id -u):$(id -g) "\$name.core" "\$name.about"
EOF
  chmod 755 "$dir/handler"
  # A limit above 0 has the kernel wait for the handler, so the process's /proc stays to be read.
  sudo sysctl -q -w kernel.core_pipe_limit=4 "kernel.core_pattern=|$dir/handler %P %t %s"
}

read_cores() {
  local out=$2 about core exe name runtime root signal when x
  cores=$1
  shopt -s nullglob
  # No core outlives the step, read or not.
  trap 'rm -f "$cores"/*.core' EXIT
  mkdir -p "$out"
  if ! command -v gdb > /dev/null; then
    echo "::warning::No gdb to read the app's core with."
    return 0
  fi
  # The sandbox's own tree, as flatpak run lays it out: the runtime in /usr, the app in /app.
  root=$(mktemp -d)
  runtime=$(flatpak info --show-runtime app.ilo.browser)
  ln -s "$(flatpak info --show-location "$runtime")/files" "$root/usr"
  ln -s "$(flatpak info --show-location app.ilo.browser)/files" "$root/app"
  for x in bin sbin lib lib32 lib64; do
    [ ! -e "$root/usr/$x" ] || ln -s "usr/$x" "$root/$x"
  done
  for core in "$cores"/*.core; do
    about=${core%.core}.about
    name=$(basename "${core%.core}")
    signal=$(sed -n 1p "$about")
    exe=$(sed -n 2p "$about")
    when=$(date -u -d "@$(echo "$name" | cut -d- -f2)" +%TZ)
    [ ! -e "$root$exe" ] || exe=$root$exe
    gdb -batch -nx -iex 'set auto-load off' -iex 'set debuginfod enabled off' \
      -iex "set sysroot $root" -iex 'set print frame-arguments none' -iex 'set print entry-values no' \
      -ex 'echo == The thread that died\n' -ex 'bt 40' \
      -ex 'echo == Every thread\n' -ex 'info threads' -ex 'thread apply all bt 40' \
      "$exe" "$core" 2>&1 |
      # Paths as the sandbox has them, without a line for each mapped file gdb has no copy of.
      grep -v 'during file-backed mapping note processing' | sed "s#$root/#/#g" | cat -s \
        > "$out/$name.txt" || true
    rm -f "$core"
    echo "::warning::The Flatpak's ${exe##*/} died on signal $signal at $when. Its backtrace is $name.txt in the artifact ilo-flatpak-traces."
    grep -E '^(Core was generated|Program terminated)' "$out/$name.txt" || true
    sed -n '/^== The thread that died/,/^== Every thread/p' "$out/$name.txt" | grep '^#' | head -15 || true
  done
}

case ${1:-} in
  catch) catch "$2" ;;
  read) read_cores "$2" "$3" ;;
  *) echo "Usage: $0 catch DIR | $0 read DIR OUT" >&2; exit 2 ;;
esac
