#!/bin/bash
# Builds the files wall's launcher (wall_launcher.c) at $1. The one line that compiles it, for
# build.rs, whose tests start it, and for build.sh, which puts it in the app: libSystem alone, and
# no call the compiler makes up for a loop, so its imports stay the list a test holds them to.
set -euo pipefail
[ $# -eq 1 ] || { echo "usage: wall_launcher.sh <out>" >&2; exit 2; }
here=$(cd "$(dirname "$0")" && pwd)
exec cc -std=c11 -O2 -Wall -Wextra -Werror -fno-builtin -fstack-protector-strong \
  -D_FORTIFY_SOURCE=2 -mmacosx-version-min="${MACOSX_DEPLOYMENT_TARGET:-15.0}" \
  -o "$1" "$here/wall_launcher.c"
