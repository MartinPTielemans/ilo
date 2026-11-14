#!/bin/bash
# Captures the same screens of the preview at 1280 by 800 and lays them side by side in sheet.png,
# so text that fits on the Mac but not in another system's faces or windows shows without anyone
# taking screenshots: on Linux on Wayland and on X11, each in a headless sway session of its own
# (tools/headless.sh), and on Windows, a CI runner's, its desktop (tools/window-capture.ps1).
# The screens: the sidebar with tabs, on the left and on the right, where the window's buttons are
# at the window's right corner (#2375), and with it on the right the Library and its Spaces, on the
# sidebar's side, the window's buttons on its column (#2375), the command bar, a tab's context menu,
# Site Controls, the icon picker, the delete-Space dialog, the sidebar of a Space whose theme has
# three colours, whose dot in the switcher draws all three (#1354), Peek over the page, its card a
# hole the shell's page shows through, and more Spaces than fit (#1401): 16 in the switcher of a
# sidebar 300 wide, and 8 in the Library's Spaces in a window 1100 wide, and Settings › Sync at
# Settings' 720 by 480 least size, scrolled, the section's name held at the card's top, and the URL
# field's translate mark lit under the pointer, light and dark (#2112), the foot bar with two lines
# over the page's bottom, as pointing's, whose keys read as this system's menus write them (#2380),
# Settings › Agents' Model menu for Claude, Sonnet chosen, and for Codex, on Default, Default naming
# its model, light and dark (#2455), and the safe-start card at the sidebar's foot, light and dark,
# and in the narrowest sidebar with ad blocking off (#2345). The preview's Linux host places no
# window, so sway puts Site Controls and the icon picker in the middle rather than where the Mac's
# host would.
# It reports: a screen that won't capture is a gap in the sheet and a line on stderr, never a
# failure.
# Usage: tools/parity.sh <preview> <output directory>
set -uo pipefail
[ $# -eq 2 ] || { echo "usage: $0 <preview> <output directory>" >&2; exit 2; }
preview=$(realpath "$1")
out=$(realpath -m "$2")
here=$(cd "$(dirname "$0")" && pwd)
face=$here/../app/assets/fonts/Inter-Regular.ttf
if command -v magick >/dev/null; then convert=(magick) montage=(magick montage); else convert=(convert) montage=(montage); fi

# Each screen: its name, what it waits for in the preview's log, and the preview's settings, as
# shell words, so a value with spaces is quoted, and for one taken under the pointer, what the
# preview logs ending in the point the pointer goes to.
screens=(
  "sidebar|drawn at|"
  "sidebar-right|drawn at|PREVIEW_SIDEBAR_RIGHT=1"
  "library-right|library shown|PREVIEW_SIDEBAR_RIGHT=1 PREVIEW_LIBRARY=1"
  "library-spaces-right|library shown|PREVIEW_SIDEBAR_RIGHT=1 PREVIEW_LIBRARY=spaces"
  "command-bar|command bar drew|PREVIEW_BAR=tabs"
  "context-menu|menu shown|PREVIEW_MENU=100,247"
  "site-controls|site controls settled|PREVIEW_SITE_CONTROLS=1"
  "icon-picker|icon picker for|PREVIEW_ICON_PICKER="
  "dialog|dialog settled|PREVIEW_DIALOG=1 PREVIEW_MENU=40,205,up,enter"
  "space-theme|drawn at|PREVIEW_THEME='x=0.5 y=0 colors=3'"
  "peek|link preview settled|PREVIEW_PEEK=1"
  "many-spaces|drawn at|PREVIEW_SPACES=16 PREVIEW_SIDEBAR=300"
  "many-spaces-library|library shown|PREVIEW_SPACES=8 PREVIEW_LIBRARY=spaces PREVIEW_SIZE=1100x700"
  "settings-least|settings held Sync|PREVIEW_ACCOUNT=words PREVIEW_SIZE=720x480 PREVIEW_SCROLL=200"
  "translate-hover-dark|translate mark drawn|PREVIEW_TRANSLATE=es PREVIEW_APPEARANCE=dark|translate mark drawn at"
  "translate-hover-light|translate mark drawn|PREVIEW_TRANSLATE=es PREVIEW_APPEARANCE=light|translate mark drawn at"
  "foot-bar|foot bar shown|PREVIEW_FOOT_BAR=point"
  "model-menu-claude-dark|menu shown|PREVIEW_MODEL_MENU=claude,sonnet PREVIEW_APPEARANCE=dark"
  "model-menu-claude-light|menu shown|PREVIEW_MODEL_MENU=claude,sonnet PREVIEW_APPEARANCE=light"
  "model-menu-codex-dark|menu shown|PREVIEW_MODEL_MENU=codex PREVIEW_APPEARANCE=dark"
  "model-menu-codex-light|menu shown|PREVIEW_MODEL_MENU=codex PREVIEW_APPEARANCE=light"
  "safe-start-light|safe start card 0 at|PREVIEW_SAFE_START=1 PREVIEW_APPEARANCE=light"
  "safe-start-dark|safe start card 0 at|PREVIEW_SAFE_START=1 PREVIEW_APPEARANCE=dark"
  "safe-start-narrow|safe start card 0 at|PREVIEW_SAFE_START=unblocked PREVIEW_SIDEBAR=164"
)
case $(uname -s) in
  MINGW* | MSYS*) systems=(windows) drag= ;;
  *)
    systems=(wayland x11)
    cargo build -q --release --manifest-path "$here/../app/check/drag/Cargo.toml" \
      --target-dir "$here/../app/target"
    drag=$here/../app/target/release/drag
    ;;
esac
# Captures the screen, with the preview's settings in the environment, and with a point, again
# with the pointer there, at the window's own point, as the window fills the output. A headless
# sway seat has no pointer, so check/drag lends it one, which lives until the capture's taken; it
# moves twice, as the window may take the seat's pointer only once the first move has given it one.
capture() { # <system> <output.png> <ready> [<point>]
  local then=() point=()
  [ -z "${4:-}" ] || point=(-Point "$4")
  [ -z "${4:-}" ] || then=(--then "set -e; at=\$(grep -m1 '$4' \"\$LOG\" | awk '{ print \$(NF - 1), \$NF }')
    set -- \$at
    { sleep 0.5; echo \"move \$1 \$2\"; sleep 0.5; echo \"move \$((\$1 + 1)) \$2\"; sleep 3; } |
      '$drag' 1280x800 > /dev/null &
    sleep 2; grim -c '$2'; wait")
  case $1 in
    windows) timeout 90 powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$here/window-capture.ps1" \
      "$2" "$3" "$preview" -Moving 400 "${point[@]}" ;;
    x11) timeout 90 "$here/headless.sh" --moving 400 "${then[@]}" --x11 "$2" "$3" "$preview" ;;
    *) timeout 90 "$here/headless.sh" --moving 400 "${then[@]}" "$2" "$3" "$preview" ;;
  esac
}
missing=()
tiles=()
for screen in "${screens[@]}"; do
  IFS='|' read -r name ready settings point <<<"$screen"
  for system in "${systems[@]}"; do
    mkdir -p "$out/$system"
    png=$out/$system/$name.png
    if ! (eval "export PREVIEW_SIZE=1280x800 $settings" && capture "$system" "$png" "$ready" "$point" >/dev/null); then
      missing+=("$system/$name")
      rm -f "$png"
      "${convert[@]}" -size 1280x800 xc:'#301010' -font "$face" -pointsize 28 -fill white \
        -gravity center -annotate +0+0 "no capture: see $system/$name.log" "$out/$system/$name.missing.png"
      png=$out/$system/$name.missing.png
    fi
    tiles+=(-label "$name, $system" "$png")
  done
done
"${montage[@]}" -font "$face" -pointsize 20 -fill white -background '#101010' -tile 2x \
  -geometry +12+12 "${tiles[@]}" "$out/sheet.png"
echo "$out/sheet.png"
for gap in "${missing[@]}"; do
  echo "No capture of $gap: its log is $out/$gap.log." >&2
  [ -z "${GITHUB_ACTIONS:-}" ] || echo "::warning title=Parity captures::No capture of $gap"
done
exit 0
