#!/bin/sh
# Renders background.svg to the 1x and 2x PNGs beside it, which tools/dmg.sh joins into one TIFF
# so the window stays sharp on a Retina screen. Run it after changing the SVG, and commit all three.
# It needs resvg (cargo install resvg) and sets the text in the app's Inter, never a system font.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
fonts=$here/../../app/assets/fonts
command -v resvg > /dev/null || { echo "render.sh needs resvg: cargo install resvg" >&2; exit 1; }
for zoom in 1 2; do
  out=$here/background.png
  [ "$zoom" = 1 ] || out=$here/background@${zoom}x.png
  resvg --skip-system-fonts --use-font-file "$fonts/Inter-Regular.ttf" \
    --use-font-file "$fonts/Inter-Medium.ttf" --zoom "$zoom" "$here/background.svg" "$out"
done
