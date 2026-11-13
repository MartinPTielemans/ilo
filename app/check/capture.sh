# check.sh runs this at 250.
# ⇧⌘2 from the page opens capture mode over it, with the keyboard, and the pointer over an element
# lights the box the core hit tests. A click takes it, and the menu's Copy copies a picture of just
# that box to a pasteboard of the run's own and shows the toast with Save and Share. ⇧⌘-drag on
# the page takes the rectangle dragged at once. Save to
# Library, the menu's first row, saves it into the profile's Media and says so, and the Library's
# Media shows it first, opens it in a tab and deletes it from its menu, and then says it is empty
# in a line that wraps in its list, narrow and wide, light and dark. On Windows the toast's Save
# saves it where the system's save dialog is told, and the page has the keyboard back after.
# Escape and a second ⇧⌘2 leave capture mode and give the page the keyboard back. File › Capture
# Full Page copies a picture of the whole long page, past its view. Capture in Portrait Mode's card
# picks White, and its Capture copies the page on it. A drag takes the rectangle dragged, and a
# click outside capture mode and a resize leave it.

# The line saying the PNG was copied to the run's own pasteboard, and the toast's buttons after it.
# On Linux check.sh runs in a display of its own, whose clipboard is the run's, and there's no share
# menu, so the toast has no Share. Windows has its share sheet, and CI's runner a desktop of its own.
if [ "$(uname)" = Darwin ]; then
  copied='^browser: pasteboard png [0-9]* bytes, private$'
  buttons='save, share'
elif windows; then
  copied='^browser: clipboard png [0-9]* bytes$'
  buttons='save, share'
else
  copied='^browser: clipboard png [0-9]* bytes$'
  buttons=save
fi
# A page longer than any view, with a box and a link at known places in it.
page='data:text/html;charset=utf-8,<title>loading</title>
<body style="margin:0;height:6000px">
<div style="position:absolute;left:300px;top:200px;width:200px;height:100px;background:teal"></div>
<a href="https://example.org/"
  style="position:absolute;left:20px;top:20px;font:24px system-ui">a link</a>
<script>requestAnimationFrame(() => setTimeout(() => document.title = "ready"));</script>'
log="$out/capture.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$page" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the capture test page didn't draw"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: capture. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi

# ⇧⌘2 from the page: capture mode shows over it, with the keyboard. Sets $left and $top to where
# the page is in the window.
capture_mode() {
  send key 2 cmd shift
  expect "^browser: command capture via $1\$" "⇧⌘2 from the $1 didn't run Capture"
  expect '^browser: capture mode at ' "⇧⌘2 from the $1 didn't show capture mode"
  frame=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: capture mode at ' |
    sed 's/^browser: capture mode at //')
  left=$(echo "$frame" | cut -d' ' -f1)
  top=$(echo "$frame" | cut -d' ' -f2)
  # The window shows on the main queue's next turn, after the picture of the page is decoded.
  expect '^browser: capture shown at ' "capture mode's window didn't show"
}
# Points in the window, from $1 and $2 in the page.
in_page() {
  awk -v x="$1" -v y="$2" -v left="$left" -v top="$top" 'BEGIN { print x + left, y + top }'
}
# Moves the pointer onto the box, 200 by 100 at 300, 200 in the page, and waits for it to light,
# else fails with $1. The pointer leaves the box first. wlroots sends no motion to where the pointer
# is already, and the last click on the box left it there (#1578).
light_box() {
  send move $(in_page 100 100)
  send move $(in_page 400 250)
  expect '^browser: capture element at [0-9.]* [0-9.]*: 300 200 200 100$' "$1"
}

capture_mode page
light_box "the pointer over the box didn't light it"
../tools/window-screenshot.sh "$pid" "$out/capture-element.png" || fail "no window to capture"
# The pointer over the page is ilo's camera, as on the Mac (#1279), where the display's own pointer
# shows it. Linux's captures can draw the pointer; the Mac's never do.
if [ "$(uname)" = Linux ]; then
  send layout
  content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
    sed 's/^browser: layout content \([0-9]*x[0-9]*\),.*/\1/')
  # The camera's dark body, left of its lens and below it, in fractions of the window.
  body=$(echo "$(in_page 400 250) $content" |
    awk '{ split($3, size, "x"); print ($1 - 7) / size[1], ($2 + 3) / size[2] }')
  send warp $(in_page 400 250)
  shown=
  for _ in $(seq 20); do
    POINTER=1 ../tools/window-screenshot.sh "$pid" "$out/capture-pointer.png" ||
      fail "no window to capture"
    seen=$(pixel "$out/capture-pointer.png" $body)
    # Within 12 of the body's rgb(30, 41, 48) in each channel, where the lit box is teal. Headless
    # sway draws its pointer into every capture, so there's no capture of the page without it.
    echo "$seen" | awk '{ exit !($1 >= 18 && $1 <= 42 && $2 >= 29 && $2 <= 53 && $3 >= 36 &&
      $3 <= 60) }' && shown=1 && break
  done
  [ -n "$shown" ] || fail "the pointer over the page isn't the camera: $seen (capture-pointer.png)"
  send warp
fi
send click $(in_page 400 250)
expect '^browser: capture chose 300 200 200 100$' "a click on the box didn't take it"
../tools/window-screenshot.sh "$pid" "$out/capture-chosen.png" || fail "no window to capture"
# The menu is 10 in from the ring's left, level with its top; Copy is its second row, 61 to 85
# down, under Send To…, a line and Save to Library.
send click $(in_page 442 269)
expect '^browser: capture run copy$' "the menu's Copy didn't run"
expect '^browser: tab [0-9]* captured [0-9]*: [0-9]*x[0-9]*, [0-9]* bytes$' \
  "Copy didn't get a picture of the box"
size=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: tab [0-9]* captured ' |
  sed 's/.*: \([0-9]*x[0-9]*\),.*/\1/')
case "$size" in
  200x100 | 400x200) ;;
  *) fail "the picture of the 200 by 100 box is $size" ;;
esac
expect "$copied" \
  "Copy didn't copy the PNG to the run's pasteboard"
expect '^browser: capture left (copy)$' "capture mode didn't leave after Copy"
expect "^browser: toast shown Screenshot Copied to Clipboard \\[$buttons\\]\$" \
  "Copy didn't show its toast with $buttons"
expect '^browser: capture hidden$' "capture mode's window didn't hide after Copy"
send state
in_browser RenderWidgetHostViewCocoa "after Copy"

# Windows: the toast's Save asks where in the system's save dialog and saves the picture there.
# The dialog's loop runs the UI's tasks, such as the toast's leaving, which crashed the app while
# the dialog opened inside the click (#1358). Save is the toast's first button: the toast ends 18
# in from the window's edge, Share 13.5 in from its own and 31 wide, and Save, about 46 wide, 8
# before it, all 41 down.
saved_to=
if windows; then
  bytes=$(grep -m1 '^browser: clipboard png ' "$log" |
    sed 's/^browser: clipboard png \([0-9]*\) bytes$/\1/')
  await '^browser: toast settled$' || fail "Copy's toast didn't settle"
  send layout
  width=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
    sed 's/^browser: layout content \([0-9]*\)x.*/\1/')
  toast_save="$((width - 18 - 76)) 41"
  send move $toast_save
  send click $toast_save
  expect '^browser: toast button save$' "Save wasn't pressed at $toast_save"
  expect '^browser: toast hidden$' "the toast didn't leave once its Save was pressed"
  expect '^browser: capture save panel shown$' "the toast's Save didn't ask where to save"
  ../tools/window-screenshot.sh "$pid" "$out/capture-save-dialog.png" || fail "no window to capture"
  picture="$out/capture-saved.png"
  rm -f "$picture"
  send savepanel "$(cygpath -aw "$picture")"
  expect '^browser: save panel answered$' "no save dialog showed for the toast's Save"
  expect '^browser: capture saved ' "the save dialog didn't save the picture"
  [ -f "$picture" ] || fail "the save dialog saved the picture elsewhere: $(grep -m1 \
    '^browser: capture saved ' "$log")"
  [ "$(wc -c < "$picture" | tr -d ' ')" = "$bytes" ] ||
    fail "the save dialog saved $(wc -c < "$picture") bytes, not the $bytes copied"
  send state
  in_browser RenderWidgetHostViewCocoa "after the save dialog"
  saved_to=" The toast's Save saved it to $picture, from $out/capture-save-dialog.png."
fi

# Edit (#673): the menu's Edit shows its bar under the box in place of the menu. A drag on the box
# draws the pen's red line, the bar's Arrow an arrow, which ⌘Z takes back, and Return brings the
# menu back, whose Copy copies the picture with the line on it. Edit is the row after Save, or
# after Messages on the Mac, each 24 down from Save to Library's 37 to 61.
capture_mode page
light_box "the pointer over the box didn't light it for Edit"
send click $(in_page 400 250)
expect '^browser: capture chose 300 200 200 100$' "a click on the box didn't take it for Edit"
if [ "$(uname)" = Darwin ]; then edit_row=341; else edit_row=317; fi
send click $(in_page 442 $edit_row)
expect '^browser: capture run edit$' "the menu's Edit didn't run"
# The bar is at the ring's left, 8 under it: 12 buttons of 28, 4 apart, in 4 of padding.
expect '^browser: capture markup drawn [a-z]* at 296 312 388 36$' \
  "Edit's bar didn't show under the box: $(grep '^browser: capture markup drawn' "$log" | tail -1)"
send move $(in_page 320 250)
send press $(in_page 320 250)
send dragto $(in_page 360 265)
send dragto $(in_page 400 270)
send dragto $(in_page 480 250)
send release $(in_page 480 250)
expect '^browser: capture mark pen ff3b30$' "a drag on the box didn't draw the pen's line"
# The bar's Arrow, its second button, 4 + 32 + 14 in and 4 + 14 down.
send click $(in_page 346 330)
expect '^browser: capture markup chose arrow$' "the bar's Arrow didn't choose it"
send move $(in_page 320 215)
send press $(in_page 320 215)
send dragto $(in_page 400 250)
send dragto $(in_page 470 285)
send release $(in_page 470 285)
expect '^browser: capture mark arrow ff3b30$' "a drag on the box didn't draw an arrow"
../tools/window-screenshot.sh "$pid" "$out/capture-markup.png" || fail "no window to capture"
send key z cmd
expect '^browser: capture mark undone$' "⌘Z didn't take the arrow back"
send key return
expect '^browser: capture markup done, 1 marks$' "Return didn't keep Edit's line"
expect '^browser: capture menu drawn ' "the menu didn't come back after Edit"
send click $(in_page 442 269)
expect '^browser: capture run copy$' "the menu's Copy didn't run after Edit"
expect '^browser: capture marked 1$' "Copy didn't draw Edit's line on the picture"
expect "$copied" "Copy didn't copy the marked PNG to the run's pasteboard"
# The picture itself, to look at, where the run's clipboard is a headless session's that wl-paste
# reads. The Mac's pasteboard is the run's own, and Xvfb's has no reader here.
case "${BROWSER_HEADLESS:-xvfb}" in
  xvfb) ;;
  *) WAYLAND_DISPLAY="$BROWSER_HEADLESS" wl-paste -t image/png > "$out/capture-marked.png" ||
    fail "the clipboard had no PNG after Copy with Edit's line" ;;
esac
expect '^browser: capture left (copy)$' "capture mode didn't leave after Copy with Edit's line"
expect '^browser: capture hidden$' "capture mode's window didn't hide after Copy with Edit's line"

# ⇧⌘-drag on the page, as Arc's, starts capture mode dragging and takes the rectangle dragged at
# once, with the menu at its corner. The page's window keeps the pointer, so the core hands capture
# mode the drag. The app's lines may come at any of the drag's steps.
from=$((step + 1))
send press $(in_page 320 210) cmd shift
send dragto $(in_page 330 220) cmd shift
send dragto $(in_page 480 290) cmd shift
send release $(in_page 480 290) cmd shift
expect 'browser: tab [0-9]* press with modifiers [0-9]* taken$' \
  "the core didn't hand the app the ⇧⌘ press on the page" "$from"
expect '^browser: capture started$' "⇧⌘-drag on the page didn't start capture mode" "$from"
expect '^browser: capture chose 320 210 160 80$' \
  "⇧⌘-drag didn't take the rectangle: $(grep '^browser: capture chose' "$log" | tail -1)" "$from"
expect '^browser: capture shown at ' "capture mode's window didn't show for ⇧⌘-drag" "$from"
../tools/window-screenshot.sh "$pid" "$out/capture-cmd-dragged.png" || fail "no window to capture"
send key escape
expect '^browser: capture left (escape)$' "escape didn't leave capture mode after ⇧⌘-drag"
expect '^browser: capture hidden$' "capture mode's window didn't hide after ⇧⌘-drag"

# Save to Library, 37 to 61 down, saves the box's picture without a question and says so.
capture_mode page
light_box "the pointer over the box didn't light it the second time"
send click $(in_page 400 250)
expect '^browser: capture chose 300 200 200 100$' "a second click on the box didn't take it"
send click $(in_page 442 245)
expect '^browser: capture run save-to-library$' "the menu's Save to Library didn't run"
expect '^browser: capture left (save-to-library)$' "capture mode didn't leave after Save to Library"
expect '^browser: toast shown Saved to Library$' "Save to Library didn't say so"
expect '^browser: saved to library ' "Save to Library didn't save the picture"
library=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: saved to library ' |
  sed 's/^browser: saved to library //')
# The app names it as its OS does, C:\... on Windows, where the check's paths are Unix ones, and
# the log lines naming it are matched with its name as itself, since Windows' has backslashes.
file=$library
! windows || file=$(cygpath -u "$library")
named=$(printf '%s\n' "$library" | sed 's/[][\\.*^$]/\\&/g')
case "$file" in
  "$profile_dir/Media/Capture "*.png) ;;
  *) fail "Save to Library saved to $library, not the profile's Media" ;;
esac
[ -s "$file" ] || fail "Save to Library's $library is empty"
expect '^browser: toast settled$' "Save to Library's toast didn't settle"
../tools/window-screenshot.sh "$pid" "$out/capture-library.png" || fail "no window to capture"

# Window › View Media… opens the Library on Media, the capture its first tile, as the newest. A
# click on the tile, under the first heading, opens it in a tab and closes the Library.
send menu 'View Media…'
expect '^browser: command view-media via ui$' "Window › View Media… didn't run"
expect '^browser: library section media$' "View Media… didn't open the Library on Media"
expect '^browser: library captures [1-9][0-9]*$' "Media didn't list the capture saved"
expect '^browser: library shown$' "the Library didn't settle open on Media"
../tools/window-screenshot.sh "$pid" "$out/library-media.png" || fail "no window to capture"
# Rows from 112 + 8 across and 56 + 27 down, the heading over them; tiles 93.7 square.
send click $((112 + 8 + 47)) $((56 + 27 + 47))
expect "^browser: open media $named\$" "a click on the capture's tile didn't open it"
expect '^browser: library hidden$' "opening the capture didn't close the Library"
expect '^browser: loaded [0-9]* file://.*\.png$' "the capture's tab didn't load it"
media=$(sed -n 's|^browser: loaded \([0-9]*\) file://.*\.png$|\1|p' "$log" | tail -1)

# A right-click on the tile offers Open and Delete, which moves the capture to the Trash, as the
# downloads' Move to Trash does, and out of Media.
send menu 'View Media…'
expect '^browser: library shown$' "View Media… didn't open the Library again"
post rightclick $((112 + 8 + 47)) $((56 + 27 + 47))
expect "^browser: menu shown $(cased 'Open, Delete')\$" \
  "a right-click on the capture's tile didn't show its menu"
../tools/window-screenshot.sh "$pid" "$out/media-menu.png" || fail "no window to capture"
send choose Delete
expect "^browser: delete capture $named\$" "the tile's Delete didn't run"
expect '^browser: library captures [0-9]*$' "Media wasn't listed again after Delete"
[ ! -e "$file" ] || fail "Delete left $library where it was"
# Media also tiles the profile's downloaded pictures and videos, and the captures a run that
# failed kept, which an earlier run can leave in a dev profile (restore's clip.mp4 on a VM). The
# app clears them, as a user would: Downloads' Clear List takes the downloads off the list and
# keeps their files, and each capture left goes by its tile's Delete.
listed=$(grep '^browser: downloads ' "$log" | tail -1 | cut -d' ' -f3)
if [ "${listed:-0}" -gt 0 ]; then
  send key j cmd shift
  expect '^browser: library section downloads$' "⇧⌘J didn't move the Library to Downloads"
  # The first row, under the heading.
  post rightclick 300 111
  expect "^browser: menu shown .*$(cased 'Clear List')\$" \
    "a right-click on the first download's row didn't show its menu"
  send choose "$(cased 'Clear List')"
  expect '^browser: command clear-downloads via download menu$' "Clear List didn't run"
  expect '^browser: downloads 0$' "Clear List didn't take the $listed downloads off the list"
  send menu 'View Media…'
  expect '^browser: library section media$' "View Media… didn't move the Library back to Media"
fi
left=$(grep '^browser: library captures ' "$log" | tail -1 | cut -d' ' -f4)
while [ "$left" -gt 0 ]; do
  post rightclick $((112 + 8 + 47)) $((56 + 27 + 47))
  expect "^browser: menu shown $(cased 'Open, Delete')\$" \
    "a right-click on a capture left in Media didn't show its menu"
  send choose Delete
  expect '^browser: delete capture ' "Delete didn't take a capture left in Media"
  expect "^browser: library captures $((left - 1))\$" "Media wasn't listed again after Delete"
  left=$((left - 1))
done
# Emptied, Media says what fills it in a line that wraps in the list's width, 12 in from its
# edges at 112 and 416 (#828), as the window's light and dark, narrow and wide, show it.
expect '^browser: library empty media at ' "Media emptied didn't draw its line"
send layout
frame=$(grep '^browser: layout content' "$log" | tail -1 |
  sed 's/^browser: layout content \([0-9]*\)x\([0-9]*\),.*/\1 \2/')
# The window's look now, light or dark, from the last line it logged.
tone() {
  case "$(grep '^browser: window light ' "$log" | tail -1)" in
    *true) echo light ;;
    *) echo dark ;;
  esac
}
# The window at $2 by $3, captured in its look and then in the other, as $1-light and $1-dark.
empty_media() {
  send resize $2 $3
  expect "^browser: drawn at $2x$3\$" "the window didn't draw at $2 by $3"
  set -- "$1" $(grep '^browser: library empty media at ' "$log" | tail -1 | cut -d' ' -f6,7,9)
  [ "$2" -ge 124 ] && [ $(($2 + $3)) -le 404 ] ||
    fail "Media's empty line is $3 wide at $2, past the list's 124 to 404"
  empty_lines="$empty_lines$1 $4, "
  was=$(tone)
  ../tools/window-screenshot.sh "$pid" "$out/media-empty-$1-$was.png" || fail "no window to capture"
  if [ "$was" = light ]; then other=Dark light=false; else other=Light light=true; fi
  send menu "$(cased "Appearance › $other")"
  expect "^browser: window light $light\$" "View › Appearance › $other didn't change its look"
  ../tools/window-screenshot.sh "$pid" "$out/media-empty-$1-$(tone).png" ||
    fail "no window to capture"
  send menu "$(cased 'Appearance › Automatic')"
  expect "^browser: window light $([ "$was" = light ] && echo true || echo false)\$" \
    "View › Appearance › Automatic didn't give the window its look back"
}
empty_lines=
empty_media narrow 800 520
empty_media wide 1240 760
send resize $frame
expect "^browser: drawn at ${frame% *}x${frame#* }\$" "the window didn't draw back at its size"
send menu 'View Media…'
expect '^browser: library hidden$' "View Media… didn't close the Library"
# ⌘W closes the capture's tab, back to the long page, which a click gives the keyboard.
send key w cmd
expect "^browser: tab $media closed\$" "⌘W didn't close the capture's tab"
send click $page_point
send state
in_browser RenderWidgetHostViewCocoa "after a click on the long page"

# Escape leaves, and so does ⇧⌘2 again, from capture mode's own window.
capture_mode page
send state
[ "$key" = GPUIPanel ] || fail "capture mode doesn't have the keyboard: $key does"
send key escape
expect '^browser: capture left (escape)$' "escape didn't leave capture mode"
send state
in_browser RenderWidgetHostViewCocoa "after escape in capture mode"
capture_mode page
send key 2 cmd shift
expect '^browser: capture left (toggle)$' "a second ⇧⌘2 didn't leave capture mode"
send state
in_browser RenderWidgetHostViewCocoa "after a second ⇧⌘2"

# Capture mode's pill and menu are ilo's menu's panel in the window's look, the ring and a
# highlighted row its Space's accent: captured in the look the window has and then in the other,
# as capture-pill-light.png, capture-menu-light.png and their dark pair.
tone() {
  case "$(grep '^browser: window light ' "$log" | tail -1)" in
    *true) echo light ;;
    *) echo dark ;;
  esac
}
capture_look() {
  capture_mode page
  expect "^browser: capture pill drawn $1\$" "capture mode's pill didn't draw in the $1 look"
  light_box "the pointer over the box didn't light it in the $1 look"
  ../tools/window-screenshot.sh "$pid" "$out/capture-pill-$1.png" || fail "no window to capture"
  send click $(in_page 400 250)
  expect '^browser: capture chose 300 200 200 100$' \
    "a click on the box didn't take it in the $1 look"
  expect "^browser: capture menu drawn $1\$" "capture's menu didn't draw in the $1 look"
  ../tools/window-screenshot.sh "$pid" "$out/capture-menu-$1.png" || fail "no window to capture"
  send key escape
  expect '^browser: capture left (escape)$' "escape didn't leave capture mode in the $1 look"
}
was=$(tone)
capture_look "$was"
if [ "$was" = light ]; then other=Dark light=false; else other=Light light=true; fi
send menu "$(cased "Appearance › $other")"
expect "^browser: window light $light\$" "View › Appearance › $other didn't change its look"
capture_look "$(tone)"
send menu "$(cased 'Appearance › Automatic')"
expect "^browser: window light $([ "$was" = light ] && echo true || echo false)\$" \
  "View › Appearance › Automatic didn't give the window its look back"

# The whole page, beyond its view: at least its 6000 points high.
send menu "Capture Full Page"
expect '^browser: command capture-full-page via ui$' "File › Capture Full Page didn't run"
expect '^browser: toast shown Capture in Progress$' "Capture Full Page didn't say it's capturing"
expect '^browser: tab [0-9]* captured [0-9]*: [0-9]*x[0-9]*, [0-9]* bytes$' \
  "Capture Full Page didn't get a picture"
full=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: tab [0-9]* captured ' |
  sed 's/.*: \([0-9]*x[0-9]*, [0-9]* bytes\)$/\1/')
height=$(echo "$full" | sed 's/^[0-9]*x\([0-9]*\),.*/\1/')
[ "$height" -ge 6000 ] || fail "the full page's picture is $full, not the page's 6000 points high"
expect "$copied" \
  "Capture Full Page didn't copy the PNG to the run's pasteboard"
expect "^browser: toast shown Screenshot Copied to Clipboard \\[$buttons\\]\$" \
  "Capture Full Page didn't show its toast"

# Portrait Mode, from the command bar: its card under the URL field picks White, and Capture copies
# the page's view on it, as wide again on each side as on the top and bottom.
bar_command capture-portrait "Capture in Portrait Mode" "portrait mode"
expect '^browser: portrait card [0-9]* at ' "Capture in Portrait Mode opened no card"
expect '^browser: portrait swatch white [0-9]* at ' "Portrait Mode's card drew no White"
../tools/window-screenshot.sh "$pid" "$out/capture-portrait-card.png" || fail "no window to capture"
send click $(sed -n 's/^browser: portrait swatch white [0-9]* at //p' "$log" | tail -1 |
  awk '{ printf "%.0f %.0f", $1 + $3 / 2, $2 + $4 / 2 }')
expect '^browser: portrait chose white$' "White on Portrait Mode's card didn't choose it"
# Capture is the right of the card's buttons, a row's height over its foot.
send click $(sed -n 's/^browser: portrait card [0-9]* at //p' "$log" | tail -1 |
  awk '{ printf "%.0f %.0f", $1 + $3 * 0.75, $2 + $4 - 26 }')
expect '^browser: tab [0-9]* captured [0-9]*: [0-9]*x[0-9]*, [0-9]* bytes$' \
  "Portrait Mode's Capture didn't get a picture of the page"
expect '^browser: portrait white [0-9]*x[0-9]*$' "Portrait Mode didn't put the page on white"
view=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: tab [0-9]* captured ' |
  sed 's/.*: \([0-9]*\)x\([0-9]*\),.*/\1 \2/')
portrait=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: portrait white ' |
  sed 's/^browser: portrait white \([0-9]*\)x\([0-9]*\)$/\1 \2/')
echo "$view $portrait" | awk '{ exit !($3 > $1 && $3 - $1 == $4 - $2) }' ||
  fail "Portrait Mode's picture is $portrait, not the page's $view with an even margin"
expect "$copied" "Portrait Mode didn't copy the PNG to the run's pasteboard"
# The picture itself, to look at, where the run's clipboard is a headless session's that wl-paste
# reads. The Mac's pasteboard is the run's own, and Xvfb's has no reader here.
case "${BROWSER_HEADLESS:-xvfb}" in
  xvfb) ;;
  *) WAYLAND_DISPLAY="$BROWSER_HEADLESS" wl-paste -t image/png > "$out/capture-portrait.png" ||
    fail "the clipboard had no PNG after Portrait Mode copied one" ;;
esac
expect "^browser: toast shown Screenshot Copied to Clipboard \\[$buttons\\]\$" \
  "Portrait Mode didn't show its toast"
# The card's buttons left the keyboard with the UI; the page takes it back.
send click 600 400

# Windows: the card offers the desktop's picture too, which Capture reads from the runner's desktop
# and puts the page on (#2682). A solid-colour desktop fills the picture with its colour, so the
# picture's corner is that colour; Windows Spotlight gives none, and the page goes on white.
if windows; then
  bar_command capture-portrait "Capture in Portrait Mode" "portrait mode"
  expect '^browser: portrait swatch desktop [0-9]* at ' \
    "Portrait Mode's card on Windows drew no Desktop Picture swatch"
  ../tools/window-screenshot.sh "$pid" "$out/capture-portrait-card-desktop.png" ||
    fail "no window to capture"
  send click $(sed -n 's/^browser: portrait swatch desktop [0-9]* at //p' "$log" | tail -1 |
    awk '{ printf "%.0f %.0f", $1 + $3 / 2, $2 + $4 / 2 }')
  expect '^browser: portrait chose desktop$' "Desktop Picture on Portrait Mode's card didn't choose it"
  send click $(sed -n 's/^browser: portrait card [0-9]* at //p' "$log" | tail -1 |
    awk '{ printf "%.0f %.0f", $1 + $3 * 0.75, $2 + $4 - 26 }')
  expect '^browser: desktop picture ' \
    "Portrait Mode's Capture on Desktop Picture didn't read the desktop (browser: desktop picture)"
  desktop=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: desktop picture ')
  echo "the runner's ${desktop#browser: }"
  expect '^browser: portrait desktop [0-9]*x[0-9]*$' "Portrait Mode didn't put the page on Desktop Picture"
  expect "$copied" "Portrait Mode on Desktop Picture didn't copy the PNG to the clipboard"
  corner=$(python3 -I check/clipboard-windows.py png "$out/capture-portrait-desktop.png") ||
    fail "the clipboard had no PNG after Portrait Mode copied one on Desktop Picture"
  case "$desktop" in
    *', colour '*) want=${desktop##*, colour } ;;
    *'none, '*) want=ffffff ;;
    *) want= ;;
  esac
  [ -z "$want" ] || [ "$corner" = "$want" ] ||
    fail "Portrait Mode's picture's corner is $corner, not $want, for the runner's ${desktop#browser: }"
  on_desktop=" On the runner's ${desktop#browser: } Portrait Mode's corner is $corner:"
  on_desktop="$on_desktop $out/capture-portrait-card-desktop.png, $out/capture-portrait-desktop.png."
  send click 600 400
fi

# A drag takes the rectangle dragged, and its menu comes as a click's box's does. A click outside
# capture mode, on the sidebar, leaves it, and so does a resize of the window.
capture_mode page
send press $(in_page 50 300)
send dragto $(in_page 150 350)
send dragto $(in_page 250 400)
send release $(in_page 250 400)
expect '^browser: capture chose ' "a drag didn't take a rectangle"
dragged=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: capture chose ' |
  sed 's/^browser: capture chose //' | awk '{ printf "%.0f %.0f %.0f %.0f", $1, $2, $3, $4 }')
[ "$dragged" = "50 300 200 100" ] || fail "a drag took $dragged, not the rectangle dragged, 50 300 200 100"
expect '^browser: capture menu drawn ' "the dragged rectangle's menu didn't draw"
../tools/window-screenshot.sh "$pid" "$out/capture-dragged.png" || fail "no window to capture"
send key escape
expect '^browser: capture left (escape)$' "escape didn't leave the dragged rectangle's menu"
capture_mode page
send click 100 400
expect '^browser: capture left (dismissed)$' "a click on the sidebar didn't leave capture mode"
# The click gives the keyboard to the UI, but on X11 the core leaves it in the page (#708). On
# Wayland GPUI hears the press itself and takes it, as on the Mac.
if [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  capture_mode page
else
  capture_mode ui
fi
send resize 1200 760
expect '^browser: capture left (dismissed)$' "a resize didn't leave capture mode"

# ⇧⌘-click on a link, a press the core takes in case it's a drag, still reaches the page, which
# opens the link in a tab in front as Chrome does.
send click $(in_page 40 30) cmd shift
expect 'browser: tab [0-9]* press ended, handed back$' \
  "the ⇧⌘-click on a link wasn't handed back to the page"
expect '^browser: tab [0-9]* opened by [0-9]* in the foreground$' \
  "⇧⌘-click on a link didn't open it in a tab in front"
none_since '^browser: capture started$' || fail "⇧⌘-click on a link started capture mode"

exec 3>&-
quit
echo "ok: capture. The box's picture is $size, the full page's $full, Portrait Mode's" \
  "$(echo "$portrait" | tr ' ' x) on white.$saved_to Media's empty line, in" \
  "lines: ${empty_lines%, }." \
  "Windows: $out/capture-element.png, $out/capture-chosen.png, $out/capture-library.png," \
  "$out/library-media.png, $out/media-menu.png, $out/media-empty-narrow-light.png," \
  "$out/media-empty-narrow-dark.png, $out/media-empty-wide-light.png," \
  "$out/media-empty-wide-dark.png, $out/capture-pill-light.png, $out/capture-menu-light.png," \
  "$out/capture-pill-dark.png, $out/capture-menu-dark.png, $out/capture-dragged.png, $out/capture-cmd-dragged.png," \
  "$out/capture-portrait-card.png.${on_desktop:-}" \
  "A drag took a rectangle, and a click outside and a resize left capture mode."
