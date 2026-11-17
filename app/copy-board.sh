#!/bin/bash
# Copies a Board's page (#3073) into $1, the folder chrome://board serves it from (core/board.cc):
# app/board/'s files, tokens.css from the design tokens, and the Lucide icons board.css names, flat
# as icon-<name>.svg, since the host serves names, never paths. build.sh and the Windows package
# call it where they put uBlock Origin.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
dest=$1
rm -rf "$dest"
mkdir -p "$dest"
cp "$here"/board/*.html "$here"/board/*.js "$here"/board/*.css "$dest/"
# tokens.toml's [board] table as custom properties, as its header says.
awk '
  /^\[/ { inside = ($0 == "[board]"); next }
  !inside || /^#/ || !/=/ { next }
  {
    key = $1; value = $3
    if (value ~ /^0x/) value = "#" tolower(substr(value, 3))
    else if (key ~ /-share(-dark)?$/) value = (value * 100) "%"
    else value = value "px"
    if (key ~ /-dark$/) dark = dark "    --" substr(key, 1, length(key) - 5) ": " value ";\n"
    else light = light "  --" key ": " value ";\n"
  }
  END {
    printf "/* Written by copy-board.sh from app/design/tokens.toml [board]. */\n"
    printf ":root {\n%s}\n@media (prefers-color-scheme: dark) {\n  :root {\n%s  }\n}\n", light, dark
  }
' "$here/design/tokens.toml" > "$dest/tokens.css"
grep -q -- "--canvas:" "$dest/tokens.css" || {
  echo "copy-board: tokens.toml has no [board] table with a canvas" >&2
  exit 1
}
# board/icons/ holds the Lucide glyphs only a Board draws, since every file in assets/icons/ is
# built into the app.
for name in $(sed -n 's/.*url(\/icon-\([a-z0-9-]*\)\.svg).*/\1/p' "$here/board/board.css"); do
  icon=$here/board/icons/$name.svg
  [ -f "$icon" ] || icon=$here/assets/icons/$name.svg
  [ -f "$icon" ] || {
    echo "copy-board: board.css names icon-$name.svg, which neither board/icons/ nor assets/icons/ has" >&2
    exit 1
  }
  cp "$icon" "$dest/icon-$name.svg"
done
