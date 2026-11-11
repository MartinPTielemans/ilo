#!/bin/bash
# Builds Chromium.app, with Chrome's framework in it, from the tree setup.sh prepared. The first
# build takes hours; after that a change recompiles what it touches and relinks the framework.
# `build.sh dev` builds the dev build in out/Dev instead. Other arguments are ninja targets, such
# as one object file to check a change compiles; with none, it builds the app.
set -euo pipefail
cd "$(dirname "$0")"
export PATH=$PWD/.deps/bin:$PATH
out=.deps/src/out/Release
if [ "${1:-}" = dev ]; then
  out=.deps/src/out/Dev
  shift
fi
# Ninja's flags aren't taken: with no target named, ninja would build every test in the tree.
case ${1:-} in -*) echo "usage: build.sh [dev] [target ...]" >&2 && exit 2 ;; esac
[ $# -gt 0 ] || set -- chrome
# The checkout's series and core, whichever the tree had last: a switched checkout built without
# setup.sh kept the patches of the series it left. Unchanged, this only compares the kept copies.
. ./tree.sh
apply_series .deps/src mac
# GN's scripts read the core count too, into every link command, so a BUILD.gn change is
# regenerated before the cap below: under it, the commands differ from setup.sh's, the code
# generators relink, and everything including their output recompiles.
ninja -C "$out" build.ninja >/dev/null
# Chromium's Python generators start a worker per core, Blink's at about 200 MB each. Six of
# Blink's at once took 21 GB and swapped a 24 GB Mac until macOS's watchdog panicked it. So two
# workers each, and a job per 3 GB of memory, or BUILD_JOBS jobs.
export PYTHON_CPU_COUNT=2
jobs=${BUILD_JOBS:-$(($(sysctl -n hw.memsize) / (3 << 30)))}
# The app binds to its own checkout's core/public/browser_core.h, and its build checks that the
# engine was built from the same one (app/build.rs): the copy kept beside Chromium.app. It's taken
# before the build, so an edit made during it isn't recorded as built.
case " $* " in
*" chrome "*)
  cp .deps/src/browser_core/public/browser_core.h "$out/browser_core.h.part"
  ninja -C "$out" -j "$jobs" "$@"
  mv "$out/browser_core.h.part" "$out/browser_core.h.built"
  ;;
*) exec ninja -C "$out" -j "$jobs" "$@" ;;
esac
