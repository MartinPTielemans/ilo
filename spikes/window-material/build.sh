#!/bin/bash
# Builds the window-material spike (#4087) into <out>/material, with Xcode's command-line tools
# alone.
# Usage: spikes/window-material/build.sh <out>
set -eu
[ $# -eq 1 ] || { echo "usage: $0 <out>" >&2; exit 2; }
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$1"
xcrun clang -fobjc-arc -fmodules -O2 -Wall -Wno-unused-function \
  -framework AppKit -framework ImageIO -framework QuartzCore \
  -o "$1/material" "$here/material.m"
echo "built $1/material"
