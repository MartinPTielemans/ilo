#!/bin/sh
# Renders the app icon's committed images from the masters in icon/: each size of 64 px and under
# from the master drawn to whole pixels at that size, the larger ones from the large art, so no small
# size is a scaled-down large one. Run it after changing icon/, then commit what it writes:
# ilo.icns, Windows' ilo.ico and logos, the Flatpak's icons, and the site's favicons. app/icon.sh
# compiles ilo.icon into Assets.car on a Mac. It needs resvg (cargo install resvg) and python3 with
# Pillow.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$here/../..
command -v resvg > /dev/null || { echo "render.sh needs resvg: cargo install resvg" >&2; exit 1; }
out=${1:-$(mktemp -d)}
mkdir -p "$out"
# $1's master at $2 px, as $out/$1-$2.png.
png() { resvg -w "$2" -h "$2" "$here/icon/$1.svg" "$out/$1-$2.png"; }

for size in 16 32 64; do png "mac-$size" "$size"; done
for size in 128 256 512 1024; do png mac "$size"; done
for size in 16 20 24 32 40 44 48 50 64; do png "win-$size" "$size"; done
for size in 128 150 256; do png win "$size"; done
png square 180

python3 -I "$here/pack.py" "$out" "$root"
