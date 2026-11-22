#!/bin/sh
# Makes the hero's stills from the captures tools/capture.ts left in DIR. For each Space:
# public/media/space-NAME.webp, the window at 2×, space-NAME-1x.webp, the same at 1×, and
# space-NAME-s.webp, the window's corner at 2×, which a phone shows at its real size. The pages
# name all three already, so retaking changes no markup, only each still's `alt` in
# src/components/spaces.tsx and the credits under the home page, where what a still shows changed.
# Usage: tools/stills.sh DIR [OUT]
set -eu
dir=${1:?usage: tools/stills.sh DIR [OUT]}
out=${2:-"$(cd "$(dirname "$0")/.." && pwd)/public/media"}
mkdir -p "$out"
for name in work lisbon reading; do
  png="$dir/space-$name.png"
  [ -f "$png" ] || { echo "no $png: run tools/capture.ts first" >&2; exit 1; }
  # A 1× screen gives a window of half the size, which would only be the soft still again.
  size=$(sips -g pixelWidth -g pixelHeight "$png" | awk '/pixel/ { printf "%s ", $2 }')
  if [ "$size" != "2560 1600 " ]; then
    echo "$png is ${size}pixels, not 2560 1600: capture on a retina screen" >&2
    exit 1
  fi
  cwebp -quiet -q 80 "$png" -o "$out/space-$name.webp"
  cwebp -quiet -q 80 -resize 1280 800 "$png" -o "$out/space-$name-1x.webp"
  # The phone's frame is 4:3 and shows 520 points of the window's width.
  cwebp -quiet -q 80 -crop 0 0 1040 780 "$png" -o "$out/space-$name-s.webp"
  ls -l "$out/space-$name.webp" "$out/space-$name-1x.webp" "$out/space-$name-s.webp"
done
