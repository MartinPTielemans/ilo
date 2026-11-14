# check.sh runs this at 290.
# The link preview. ⇧-click on a link in any tab's page, here a Today tab's, opens it in a card over
# the window, and so does Open Link in Peek in the link's menu, with no tab opened and the tab on its
# page (#1370). A link to another site in a pinned tab's page opens in one too, its page in a tab of
# no Space, and the pinned tab stays on its page. Esc closes it and its page.
# ⌘↩ keeps it as the top Today tab, shown, without loading it again. A click on the dim closes it,
# and the page behind doesn't get the click, and so does Settings taking the keyboard. ⌘W with its
# page focused closes it, and not the tab behind. ⌘Z reopens a closed one over its tab at its page,
# and so does ⇧⌘T while no tab has closed since. A favorite's link opens it too. A call's link, such
# as Google Meet's, opens as a tab instead. With the Settings switch off, the link goes in the
# pinned tab, ⇧-click opens it in a new tab, and the link's menu has no Open Link in Peek. Back on,
# a link rule to another Space sends the link there as a tab instead. Reports how long the first
# preview took to draw its first frame. Captures it at rest in a light and a dark Space theme, and
# records it opening and growing into a tab, for its look (#1373). Resting on its keep button names
# it and its key in the tooltip, over the preview (#1410).

serve
# The test page's top 40% is a link to the same server under another name, localhost, which is
# another site to the browser. Its bottom 60% counts the presses that reach it in its title. Each
# has text, as Chromium ignores a click on a page until it has drawn, and an empty page draws
# nothing. The title says "ready" once a frame is drawn, which the run waits for before a click.
target="http://localhost:$port/preview-target.html"
printf '%s' "<title>loading</title>
<a href=\"$target\" style=\"position:fixed;left:0;top:0;width:100%;height:40%;font:24px system-ui\">
A link to another site</a>
<div id=press style=\"position:fixed;left:0;top:40%;width:100%;height:60%;font:24px system-ui\">
Presses count here</div>
<script>
// M makes the link a Google Meet call's, whose host the run maps to its own server, and M again
// the other site's: a pinned tab's address typed in opens a new tab, so the page changes it.
const link = document.querySelector(\"a\");
onkeydown = (event) => {
  if (event.key != \"m\") return;
  const meeting = link.href == \"$target\";
  link.href = meeting ? \"http://meet.google.com/ilo-check-call\" : \"$target\";
  document.title = meeting ? \"meeting link\" : \"site link\";
};
let presses = 0;
press.onmousedown = () => document.title = \"pressed \" + ++presses;
requestAnimationFrame(() => requestAnimationFrame(() => document.title = \"ready\"));
</script>" > "$out/site/preview.html"
printf '%s' '<title>target</title><p style="font:24px system-ui">Another site</p>' \
  > "$out/site/preview-target.html"
log="$out/preview.log"
step=0
rm -f "$saved"
# Meet's host goes to the run's server, which speaks no TLS, so a call's link never reaches Google.
BROWSER_DRIVE=1 BROWSER_URL="${site}preview.html" "$browser" \
  --host-resolver-rules="MAP meet.google.com 127.0.0.1:$port" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the preview test page didn't draw"
a=$(today)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: link preview. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# That click made the window key, which a page ignores, so the next one is the first it counts.
send click $page_point
expect "^browser: title $a pressed [0-9]*\$" "a click on the test page's lower part didn't reach it"
# The link, in the page's top 40%, and a point of the dim over the page's lower part, left of the
# card, which is 10% of the page's height in from its edges.
link_point="600 200"
dim_point="240 600"
# Clicks the link, and waits for its preview to settle, setting $p to the preview's tab.
open_preview() {
  send click $link_point
  expect "^browser: link preview [1-9][0-9]* $target\$" "$1: a click on the link to another site didn't open the preview"
  p=$(grep "^browser: link preview [1-9][0-9]* $target\$" "$log" | tail -1 | cut -d' ' -f4)
  expect '^browser: link preview shown$' "$1: the preview's window didn't show"
  expect '^browser: link preview settled$' "$1: the preview didn't settle"
  expect "^browser: loaded $p $target\$" "$1: the preview's page didn't load"
}
# The lines matching $2, $3… came in that order since the last command was sent, once the last has.
# $1 says when.
in_order() {
  when=$1
  shift
  for line in "$@"; do
    last_line=$line
  done
  expect "$last_line" "$when: \"$last_line\" didn't come"
  at=0
  for line in "$@"; do
    n=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -n -m1 "$line" | cut -d: -f1)
    [ -n "$n" ] && [ "$n" -gt "$at" ] || fail "$when: \"$line\" didn't come after the line before it"
    at=$n
  done
}
# The pinned tab's page is still the test page.
still_home() {
  grep -q "^browser: loaded $a $target\$" "$log" && fail "$1: the link loaded in the pinned tab"
  [ "$(grep "^browser: loaded $a " "$log" | tail -1)" = "browser: loaded $a ${site}preview.html" ] ||
    fail "$1: the pinned tab isn't at its page"
}

# The pinned tab's row, 55 down once the favorite's row of tiles is over the Space's rows.
tiles=0
home_row() {
  echo "100 $(($(pinned_row 0 | cut -d' ' -f2) + tiles))"
}

# Where Settings last drew what $1 names, as the platforms' fonts move its tabs and rows.
settings_at() {
  await "^browser: settings $1 at " || fail "Settings didn't draw its $1"
  grep "^browser: settings $1 at " "$log" | tail -1 | sed "s/^browser: settings $1 at //"
}

# A link's menu, with Open Link in Peek after Open Link in New Tab. Off the Mac its items are in
# sentence case, as Chrome's are there, and Peek keeps its capital, as ilo's name for the preview.
if windows || [ "$(uname)" = Linux ]; then
  in_tab='Open link in new tab' in_peek='Open link in Peek' in_window='Open link in new window'
else
  in_tab='Open Link in New Tab' in_peek='Open Link in Peek' in_window='Open Link in New Window'
fi
# Waits for the preview the user asked for from the test page's tab to settle, setting $p to its
# tab. $1 says how they asked.
asked_preview() {
  expect "^browser: link preview asked from $a\$" "$1 didn't ask for the link in a preview"
  expect "^browser: link preview [1-9][0-9]* $target\$" "$1 didn't open the link in a preview"
  p=$(grep "^browser: link preview [1-9][0-9]* $target\$" "$log" | tail -1 | cut -d' ' -f4)
  expect '^browser: link preview shown$' "$1: the preview's window didn't show"
  expect '^browser: link preview settled$' "$1: the preview didn't settle"
  expect "^browser: loaded $p $target\$" "$1: the preview's page didn't load"
}
# Asked for, a link opens in the preview from any tab, here a Today tab, whose plain click opens it
# in the tab (#1370): ⇧-click, which Chrome opens in a new window, and Open Link in Peek in the
# link's menu. No tab opens, and the tab stays on its page.
send click $link_point shift
asked_preview "⇧-click on the Today tab's link"
[ "$(today)" = "$a" ] || fail "⇧-click on the link opened a tab too: Today is \"$(today)\""
../tools/window-screenshot.sh "$pid" "$out/preview-shift-click.png" || fail "no window to capture"
send key escape
in_order "after Esc on the ⇧-click's preview" '^browser: link preview closing (escape)$' \
  "^browser: tab $p closed\$"
post rightclick $link_point
expect "^browser: menu shown $in_tab, $in_peek, $in_window, " \
  "a right-click on the link didn't show $in_peek after $in_tab: \
$(grep '^browser: menu shown' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/preview-menu.png" || fail "no window to capture"
send choose "$in_peek"
asked_preview "$in_peek"
[ "$(today)" = "$a" ] || fail "$in_peek opened a tab too: Today is \"$(today)\""
../tools/window-screenshot.sh "$pid" "$out/preview-menu-chosen.png" || fail "no window to capture"
send key escape
in_order "after Esc on $in_peek's preview" '^browser: link preview closing (escape)$' \
  "^browser: tab $p closed\$"
still_home "after the previews asked for"

# Pinned, the tab's links to other sites open in the preview, which grows from the link.
send key d cmd
expect '^browser: command toggle-pin via page$' "⌘D with the page focused didn't reach the UI"
expect "^browser: pinned $a\$" "⌘D didn't pin the test page's tab: the pinned tabs are \"$(pinned)\""
open_preview "from the pinned tab"
first_frame=$(grep -m1 '^browser: link preview first frame in ' "$log" | sed 's/.* in //')
[ -n "$first_frame" ] || fail "the preview didn't log its first frame"
../tools/window-screenshot.sh "$pid" "$out/preview-open.png" || fail "no window to capture"
# Probe (#2781): another app's window raised over Peek's card, and the whole session captured.
if [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  import -window root "$out/probe-0-peek.png"
  rm -f "$out/other-in"
  mkfifo "$out/other-in"
  python3 -I check/other-window.py < "$out/other-in" > "$out/other-window.log" 2>&1 &
  other=$!
  exec 7> "$out/other-in"
  for _ in $(seq 50); do grep -q '^[0-9]' "$out/other-window.log" && break; sleep 0.1; done
  ow=$(head -1 "$out/other-window.log")
  xdotool windowactivate --sync "$ow" 2> /dev/null || echo "probe: windowactivate failed"
  xdotool windowsize --sync "$ow" 900 560 2> /dev/null || echo "probe: windowsize failed"
  xdotool windowmove --sync "$ow" 180 120 2> /dev/null || echo "probe: windowmove failed"
  xdotool windowraise "$ow" 2> /dev/null || true
  for _ in $(seq 50); do [ "$(xdotool getactivewindow 2> /dev/null)" = "$ow" ] && break; sleep 0.1; done
  echo "probe: active $(xdotool getactivewindow 2> /dev/null) other $ow"
  for t in 1 2 3; do
    sleep 0.7
    import -window root "$out/probe-$t-other-over.png"
  done
  xwininfo -root -children > "$out/probe-stack.txt" 2>&1 || true
  xprop -root _NET_ACTIVE_WINDOW _NET_CLIENT_LIST_STACKING >> "$out/probe-stack.txt" 2>&1 || true
  # A click on the other window's middle, from xdotool's own pointer, as a hand's on a window half
  # over ilo's.
  closings=$(grep -c '^browser: link preview closing' "$log")
  xdotool mousemove 250 400 click 1 2> /dev/null || true
  for _ in $(seq 30); do [ "$(grep -c '^browser: link preview closing' "$log")" -gt "$closings" ] && break; sleep 0.1; done
  echo "probe: a click on the dim drawn over the other app closed Peek: $([ "$(grep -c '^browser: link preview closing' "$log")" -gt "$closings" ] && echo yes || echo no), active $(xdotool getactivewindow 2> /dev/null)"
  import -window root "$out/probe-4-after-click.png"
  exec 7>&-
  wait "$other" 2> /dev/null || true
  exec 3>&-
  quit
  echo "probe: done"
  exit 0
fi
# On Windows the card holds Peek's page where the core shows it: the frame the UI draws it in, at its
# window's scale, and the page's in the browser window, in pixels, each edge within 3 of the other's,
# as the core rounds the page's frame in DIPs. The page is under the preview's window there, which
# is made once and hidden between previews (#2541). Either may still be moving, for up to 5 s.
peek_frame() { # <when>
  for _ in $(seq 10); do
    send peek
    expect '^browser: peek page ' "$1: the driver found no page"
    page=$(grep '^browser: peek page ' "$log" | tail -1 | cut -d' ' -f4-)
    card=$(grep '^browser: link preview page frame ' "$log" | tail -1 | cut -d' ' -f6-)
    echo "$card $page" | awk '{ for (i = 1; i <= 4; i++) { d = $i - $(i + 7); if (i > 2) d += $(i - 2) - $(i + 5); if (d > 3 || d < -3) exit 1 } }' &&
      break
    page=
    sleep 0.5
  done
  send dpis
  expect '^browser: dpis ' "$1: the driver named no DPIs"
  dpis=$(grep '^browser: dpis ' "$log" | tail -1 | cut -d' ' -f3-)
  page=${page:-$(grep '^browser: peek page ' "$log" | tail -1 | cut -d' ' -f4-)}
  echo "$1: the card holds the page at $card, and the page is at $page; DPIs: $dpis" >> "$out/preview-frames.txt"
  echo "$card $page" | awk '{ for (i = 1; i <= 4; i++) { d = $i - $(i + 7); if (i > 2) d += $(i - 2) - $(i + 5); if (d > 3 || d < -3) exit 1 } }' ||
    fail "$1: Peek's card holds its page at $card, in pixels, but the page is at $page (DPIs: $dpis)"
}
rm -f "$out/preview-frames.txt"
windows && peek_frame "at launch"

# Esc in its page closes it, and its page once it has left.
send key escape
in_order "after Esc" '^browser: link preview closing (escape)$' '^browser: link preview released$' \
  '^browser: link preview hidden$' "^browser: tab $p closed\$"
still_home "after Esc"

# On Windows, the display's scale goes up a step while the preview's window is hidden, as Settings'
# Scale changes it, and back, and the preview opens at each, and then up a step again while it's
# open: the card still holds the page where the core shows it (#2541). The display goes back to its
# scale even if the run fails.
scaled="its display's scale stayed as it was"
if windows; then
  read -r scale_at scale_least scale_most << EOF
$(python3 -I check/scale.py get)
EOF
  # The browser window's content in points, from the layout, until it isn't $1, for up to 10 s, as
  # the UI lays out once it has heard the scale.
  content_not() {
    for _ in $(seq 20); do
      send layout
      content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' | cut -d' ' -f4 | tr -d ,)
      [ -n "$content" ] && [ "$content" != "$1" ] && return 0
      sleep 0.5
    done
    return 1
  }
  if [ -n "$scale_most" ] && [ "$scale_most" -gt "$scale_at" ]; then
    content_not none
    content_at=$content
    run_undo="python3 -I check/scale.py set $scale_at"
    python3 -I check/scale.py set $((scale_at + 1)) > /dev/null
    content_not "$content_at" || fail "the window's content stayed $content_at points at the display's next scale"
    open_preview "at the display's next scale"
    peek_frame "at the display's next scale"
    send key escape
    expect '^browser: link preview hidden$' "Esc didn't close the preview at the display's next scale"
    python3 -I check/scale.py set "$scale_at" > /dev/null
    run_undo=
    content_not "$content" || fail "the window's content stayed $content points back at the display's scale"
    open_preview "back at the display's scale"
    peek_frame "back at the display's scale"
    # And up a step with the preview open, which lays out again over the window at the new scale.
    content_at=$content
    run_undo="python3 -I check/scale.py set $scale_at"
    python3 -I check/scale.py set $((scale_at + 1)) > /dev/null
    content_not "$content_at" || fail "the window's content stayed $content_at points at the display's next scale, the preview open"
    await '^browser: link preview page frame .* at scale [0-9.]*$' || true
    peek_frame "at the display's next scale, the preview open"
    send key escape
    expect '^browser: link preview hidden$' "Esc didn't close the preview at the display's next scale, opened at the one before"
    python3 -I check/scale.py set "$scale_at" > /dev/null
    run_undo=
    content_not "$content" || fail "the window's content stayed $content points back at the display's scale"
    scaled="it held as the display's scale went a step up and back with the preview hidden, and up with it open"
  fi
fi
# On Windows the preview's window may have heard another scale than its display's: made hidden on a
# display at one scale, at the screen's top left, and moved to the browser window on one at another,
# as the owner's was, drawing its card at 0.8 of the page's frame (#2541). The runner has one
# display, so the driver tells the hidden window alone a scale a step from the display's, as
# Windows tells a window that moved, and the preview opens: the card still holds the page.
if windows; then
  send dpis
  expect '^browser: dpis ' "the driver named no DPIs before the preview's window heard another scale"
  case "$(grep '^browser: dpis ' "$log" | tail -1)" in
    *", window 96,"*) told=125 ;;
    *) told=100 ;;
  esac
  send dpi "$told" link preview
  expect '^browser: dpi [0-9]* told link preview$' "the driver found no preview window to tell $told %"
  open_preview "its window told $told % while hidden"
  ../tools/window-screenshot.sh "$pid" "$out/preview-told.png" || fail "no window to capture"
  peek_frame "its window told $told % while hidden"
  send key escape
  expect '^browser: link preview hidden$' "Esc didn't close the preview whose window was told $told %"
  scaled="$scaled; and the preview's window, told $told % while hidden, drew it at the display's"
fi
# Under Wayland an overlay's surface hears its scale from the compositor only once it's on screen.
# The output's scale goes a step up while the preview is hidden, and back, and at each the preview,
# last shown at the other, draws its first frame at its browser window's scale (#2592). The output
# goes back to its scale even if the run fails.
if [ "$(uname)" = Linux ] && [ -n "${WAYLAND_DISPLAY:-}" ]; then
  # Its mode grows with its scale, as headless.sh makes it, so the window keeps its size in points.
  read -r output width height scale_at << EOF
$(swaymsg -t get_outputs | jq -r '.[0] | "\(.name) \(.current_mode.width) \(.current_mode.height) \(.scale)"')
EOF
  scale_at=$(awk "BEGIN { print $scale_at + 0 }")
  # Sway's command that gives the output the scale $1.
  scale_output() { # <scale>
    echo "swaymsg output $output resolution $(awk "BEGIN { printf \"%dx%d\", $width / $scale_at * $1, $height / $scale_at * $1 }") scale $1"
  }
  # Gives the output the scale $1, read back from sway for up to 10 s. Swaymsg gives up on its
  # answer after 3 s, which a busy runner's sway can take to change the output's mode, though it
  # changes it all the same (#2684).
  give_scale() { # <scale>
    $(scale_output "$1") > /dev/null 2>&1 || true
    for _ in $(seq 20); do
      now=$(swaymsg -t get_outputs 2> /dev/null | jq -r --arg name "$output" '.[] | select(.name == $name) | .scale')
      [ "$(awk "BEGIN { print ${now:-0} + 0 }")" = "$1" ] && return 0
      sleep 0.5
    done
    fail "sway didn't give $output the scale $1 within 10 s: it's ${now:-unread}"
  }
  # The browser window's scale and its overlays' first frames', once the window's is $1, for up to
  # 10 s, as the window hears the output's scale from the compositor.
  window_scaled() { # <scale> <when>
    for _ in $(seq 20); do
      send scales
      scales=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: scales ')
      case "$scales" in "browser: scales window $1,"*) return 0 ;; esac
      sleep 0.5
    done
    fail "$2: the browser window's scale didn't become $1: ${scales:-the driver named none}"
  }
  for to in $(awk "BEGIN { print $scale_at + 1 }") "$scale_at"; do
    run_undo=$(scale_output "$scale_at")
    give_scale "$to"
    window_scaled "$to" "at the output's scale $to"
    open_preview "at the output's scale $to"
    window_scaled "$to" "with the preview open at the output's scale $to"
    for drawn in ${scales#*, overlays }; do
      [ "$drawn" = "$to" ] ||
        fail "at the output's scale $to, an overlay drew its first frame at $drawn: $scales"
    done
    send key escape
    expect '^browser: link preview hidden$' "Esc didn't close the preview at the output's scale $to"
  done
  run_undo=
  scaled="its first frame was at its browser window's scale as the output's went a step up and back with it hidden"
fi

# Its look in a light and a dark Space theme: the theme picker's first swatch, then the sun, and
# later the moon, each 13 right of its appearance-x token, as cards.sh and little.sh pick them.
peek_look() { # <light|dark> <the appearance button's x> [swatch]
  post rightclick 100 112
  expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors")" \
    "the Space's menu didn't offer its theme"
  send choose "$(cased 'Edit Theme Color…')"
  expect '^browser: theme picker shown$' "Edit Theme Color… didn't show the theme picker"
  expect '^browser: theme picker drawn$' "the theme picker didn't draw itself on screen"
  if [ -n "${3:-}" ]; then
    send clickkey "$(swatch_x 0)" $swatch_y
    expect '^browser: colored space [0-9]* [0-9a-f]*$' "the first swatch didn't colour the Space"
  fi
  send clickkey "$2" 49
  expect "^browser: themed space [0-9]* .*appearance=$1" "the Space's theme didn't turn $1"
  send key escape
  expect '^browser: theme picker hidden$' "escape didn't close the theme picker"
  open_preview "in a $1 Space"
  ../tools/window-screenshot.sh "$pid" "$out/preview-$1.png" || fail "no window to capture"
  send key escape
  expect "^browser: tab $p closed\$" "Esc didn't close the preview in a $1 Space"
}
peek_look light 191.3 swatch
peek_look dark 231.7

# Resting on the keep button names it and its key under it, in the tooltip's window, which shows
# over the preview's, so it lies over the card's page; the pointer leaving ends it.
open_preview "for the keep button's tooltip"
keep_at=$(grep '^browser: link preview keep button at ' "$log" | tail -1 | sed 's/.* at //')
[ -n "$keep_at" ] || fail "the preview didn't say where its keep button is"
send move $keep_at
# Off the Mac the keys are named as Chrome names them there.
keep_keys=⌘↩
[ "$(uname)" = Darwin ] || keep_keys=Ctrl+Enter
expect "^browser: tooltip shown Open as Tab $keep_keys\$" \
  "resting on the keep button didn't show Open as Tab and $keep_keys"
expect '^browser: tooltip window shown$' "the keep button's tooltip didn't fade in"
../tools/window-screenshot.sh "$pid" "$out/preview-tooltip.png" || fail "no window to capture"
send move $dim_point
expect '^browser: tooltip hidden$' "the pointer leaving the keep button didn't hide its tooltip"
send key escape
expect "^browser: tab $p closed\$" "Esc didn't close the preview after its keep button's tooltip"

# ⇧⌘C copies the preview's address, not the pinned tab's under it, as Arc's Peek does, and the
# preview stays. ⌘↩ keeps it as the top Today tab, shown, as it is: its page doesn't load again.
# Recorded, from the click on the link to the tab it grows into.
record preview-motion 3
open_preview "for ⌘↩"
kept=$p
send key c cmd shift
expect "^browser: copied $target\$" "⇧⌘C in the preview didn't copy its page's address"
send key return cmd
in_order "after ⌘↩" '^browser: link preview keeping (cmd-enter)$' "^browser: link preview kept $p\$"
expect "^browser: today $p\$" "⌘↩ didn't put the preview's tab at the top of Today: Today is \"$(today)\""
expect "^browser: selected $p\( in .*\)\{0,1\}\$" "⌘↩ didn't show the preview's tab"
expect '^browser: link preview hidden$' "⌘↩ didn't hide the preview"
# A screenshot while the window records ends the recording with ReplayKit's -5814, so it waits.
recorded
check_layout "the kept tab"
../tools/window-screenshot.sh "$pid" "$out/preview-kept.png" || fail "no window to capture"
[ "$(grep -c "^browser: loaded $p " "$log")" = 1 ] || fail "the kept tab's page loaded again"
still_home "after ⌘↩"
away
send click $(home_row)
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "a click on the pinned row didn't show its tab"

# A click on the dim closes it, and the page behind doesn't get the click.
open_preview "for the dim"
send click $dim_point
in_order "after a click on the dim" '^browser: link preview closing (click outside)$' \
  "^browser: tab $p closed\$"
none_since "^browser: title $a pressed" || fail "the click on the dim reached the page behind"

# Settings taking the keyboard is the user leaving it for another window, which closes it, and
# Settings keeps the keyboard.
open_preview "for Settings"
# The preview's page takes no window's keys but its own, so Settings comes from the menu.
send menu Settings…
expect '^browser: settings shown General$' "Settings… didn't show Settings over the preview"
in_order "after Settings took the keyboard" '^browser: link preview released$' \
  '^browser: link preview closing (dismissed)$' "^browser: tab $p closed\$"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings, so it didn't keep the keyboard"
none_since '^browser: command close-tab' || fail "⌘W after Settings left the preview closed a tab"

# ⌘W in its page closes it, not the pinned tab behind.
open_preview "for ⌘W"
send key w cmd
in_order "after ⌘W" '^browser: link preview closing (cmd-w)$' "^browser: tab $p closed\$"
none_since '^browser: command close-tab' || fail "⌘W in the preview closed the tab behind"
none_since "^browser: page $a unloaded 1\$" || fail "⌘W in the preview unloaded the pinned tab"

# ⌘Z reopens the preview closed last over the pinned tab, at its page, growing from its link as it
# draws. Esc closes it again, and ⇧⌘T reopens it too, as no tab has closed since.
reopened() {
  expect "^browser: link preview reopened over $a via $1\$" "$2 didn't reopen the closed preview"
  expect "^browser: link preview [1-9][0-9]* $target\$" "$2 didn't open the preview at its page"
  p=$(grep "^browser: link preview [1-9][0-9]* $target\$" "$log" | tail -1 | cut -d' ' -f4)
  expect '^browser: link preview shown$' "$2: the reopened preview's window didn't show"
  expect '^browser: link preview settled$' "$2: the reopened preview didn't settle"
  expect "^browser: loaded $p $target\$" "$2: the reopened preview's page didn't load"
}
send key z cmd
expect '^browser: command undo via page$' "⌘Z with the page focused didn't reach the UI"
reopened undo "⌘Z"
../tools/window-screenshot.sh "$pid" "$out/preview-reopened.png" || fail "no window to capture"
send key escape
in_order "after Esc on the reopened preview" '^browser: link preview closing (escape)$' \
  "^browser: tab $p closed\$"
send key t cmd shift
reopened reopen-closed-tab "⇧⌘T"
send key escape
expect "^browser: tab $p closed\$" "Esc didn't close the preview ⇧⌘T reopened"
still_home "after reopening the preview"

# A favorite's links open it too: the pinned tab's copy, which Duplicate Tab in its menu puts at the
# top of Today, under the pinned row, the divider and "+ New Tab", dragged onto the Space's name.
away
post rightclick $(home_row)
expect "^browser: menu shown $(cased '.*Duplicate Tab')" "a right-click on the pinned row didn't show its menu"
send choose "$(cased 'Duplicate Tab')"
expect '^browser: command duplicate-tab via tab menu$' "Duplicate Tab didn't run"
expect "^browser: today [1-9][0-9]* $kept\$" "Duplicate Tab didn't put a copy at the top of Today"
b=$(today | cut -d' ' -f1)
expect "^browser: selected $b\( in .*\)\{0,1\}\$" "Duplicate Tab didn't show the copy"
expect "^browser: title $b ready\$" "the copy of the test page didn't draw"
send press 100 $((90 + 44 + 41 + 15 + 41 + 20))
send dragto 100 200
send dragto 114 112
expect '^browser: drag shape tile$' "the copy's row dragged onto the Space's name didn't turn into a tile"
send release 114 112
expect "^browser: favorites $b\$" "the copy's row didn't become a favorite: \"$(grep '^browser: favorites' "$log" | tail -1)\""
tiles=55
open_preview "from the favorite"
send key escape
expect "^browser: tab $p closed\$" "Esc didn't close the favorite's preview"
away
send click $(home_row)
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "a click on the pinned row didn't show its tab"

# A call's link skips the preview and opens as a tab at the top of Today in the Space shown, which
# it shows, and the pinned tab stays on its page.
# M in the page turns its link from one to the other, once a click has given the page the keys.
link_to() {
  send click $page_point
  expect "^browser: title $a pressed [0-9]*\$" "a click on the test page's lower part didn't reach it"
  send key m
  expect "^browser: title $a $1 link\$" "M in the page didn't make its link the $1's"
}
link_to meeting
send click $link_point
expect "^browser: meeting link from $a opens as a tab\$" "a click on a Meet link didn't skip the preview"
expect '^browser: today [1-9]' "the Meet link didn't open at the top of Today: Today is \"$(today)\""
m=$(today | cut -d' ' -f1)
expect "^browser: selected $m\( in .*\)\{0,1\}\$" "the Meet link's tab didn't show"
none_since '^browser: link preview [1-9]' || fail "the Meet link opened in a preview"
../tools/window-screenshot.sh "$pid" "$out/preview-meeting.png" || fail "no window to capture"
grep -q "^browser: loaded $a http://meet.google.com" "$log" && fail "the Meet link loaded in the pinned tab"
away
send click $(home_row)
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "a click on the pinned row didn't show its tab"
link_to site

# With the switch in Settings › Features off, the link goes in the pinned tab.
send click $page_point
expect "^browser: title $a pressed [0-9]*\$" "a click on the test page's lower part didn't reach it"
send key , cmd
expect '^browser: settings shown General$' "⌘, didn't show Settings on its General tab"
# Settings takes no click until it has drawn, nor a tab's rows until the tab has (#1101, #1133).
expect '^browser: settings drew General$' "Settings didn't draw General"
send clickkey $(settings_at "toolbar Features")
expect '^browser: settings tab Features$' "a click on Features didn't show its section"
settings_drew Features
send clickkey $(settings_at "switch link previews")
expect '^browser: link previews off$' "a click on the link previews switch didn't turn them off"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $link_point
expect "^browser: loaded $a $target\$" "with previews off, the link didn't load in the pinned tab"
none_since '^browser: link preview ' || fail "with previews off, the link opened in a preview"
# ⌘← takes the pinned tab back to the page with the link.
send key left cmd
expect "^browser: command back via " "⌘← in the pinned tab didn't reach the UI"
expect "^browser: loaded $a ${site}preview.html\$" "⌘← didn't take the pinned tab back to its page"
# Off, the link's menu has no Open Link in Peek, and ⇧-click opens the link in a new tab, as
# before.
post rightclick $link_point
expect "^browser: menu shown $in_tab, $in_window, " \
  "with previews off, the link's menu didn't go from $in_tab to $in_window: \
$(grep '^browser: menu shown' "$log" | tail -1)"
send key escape
expect '^browser: menu closed$' "Esc didn't close the link's menu"
send click $link_point shift
opened="^browser: loaded [1-9][0-9]* $target\$"
expect "$opened" "with previews off, ⇧-click on the link didn't open it in a tab"
o=$(grep "$opened" "$log" | tail -1 | cut -d' ' -f3)
[ "$o" != "$a" ] || fail "with previews off, ⇧-click on the link loaded it in the pinned tab"
none_since '^browser: link preview ' || fail "with previews off, ⇧-click opened the link in a preview"
expect "^browser: selected $o\( in .*\)\{0,1\}\$" "with previews off, ⇧-click's tab didn't show"
send key w cmd
expect "^browser: tab $o closed\$" "⌘W didn't close the tab ⇧-click opened"
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "⌘W didn't show the pinned tab again"

# Back on, a link rule to another Space sends the link there as a tab, as it sends one from another
# app, and the preview doesn't open (#692). Settings' Links section adds the rule, as ./check.sh
# links does: + and the new rule's Space pop-up, where the view logs them, after the Meet rule a
# profile starts with.
first=$(space)
send menu New Space…
expect "^browser: spaces $first [0-9]*\$" "New Space didn't make a second Space"
second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
send type work
send key return
expect "^browser: create space $second work\$" "return didn't name the second Space"
send key 1 ctrl
expect "^browser: space $first\( in .*\)\{0,1\}\$" "⌃1 didn't show the first Space again"
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "the first Space didn't show the pinned tab again"
send key , cmd
expect '^browser: settings shown Features$' "⌘, didn't show Settings on its Features section"
expect '^browser: settings drew Features$' "Settings didn't draw Features"
send clickkey $(settings_at "switch link previews")
expect '^browser: link previews on$' "a click on the link previews switch didn't turn them on"
send clickkey $(settings_at "toolbar Links")
expect '^browser: settings tab Links$' "a click on Links didn't show its tab"
settings_drew Links
expect '^browser: link rules drawn ' "the Links tab didn't draw its rules"
send clickkey $(settings_at "link add")
# The Meet rule a profile starts with is the first, so the new one is the second.
expect '^browser: link rule 1 field has the keyboard$' "+ didn't give the new rule's field the keyboard"
send type preview-target
post clickkey $(settings_at "link space 1")
front=$(cased "The Space in Front")
expect "^browser: menu shown $front (checked), [^,]*, work, Little ilo\$" \
  "the rule's Space pop-up didn't list the Space in front, the user's Spaces and Little ilo"
send choose work
expect '^browser: link rules drawn URL contains "meet.google.com" → [^,]*, URL contains "preview-target" → work; ' \
  "the rule's pop-up didn't set its Space"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $link_point
expect "^browser: link rule 1 sends $target to space $second\$" "the rule didn't send the pinned tab's link"
routed="^browser: link from $a $target: tab [1-9][0-9]* in space $second of window [0-9]*\$"
expect "$routed" "the rule's link didn't open as a tab in its Space"
r=$(grep "$routed" "$log" | tail -1 | sed 's/.*: tab \([0-9]*\) .*/\1/')
expect "^browser: space $second\( in .*\)\{0,1\}\$" "the window didn't switch to the rule's Space"
expect "^browser: loaded $r $target\$" "the rule's link didn't load in its tab"
none_since '^browser: link preview ' || fail "the link a rule sent opened in a preview too"
none_since "^browser: loaded $a " || fail "the link a rule sent loaded in the pinned tab too"
../tools/window-screenshot.sh "$pid" "$out/preview-ruled.png" || fail "no window to capture"

exec 3>&-
quit
echo "ok: ⇧-click on a Today tab's link and $in_peek in its menu open it in a preview, with no tab opened. A link to another site in a pinned tab or a favorite opens in a preview over the window, which Esc, a click on the dim and ⌘W close, with its page, leaving the tab and the page behind as they were, ⇧⌘C copies the address of, and ⌘↩ keeps as the top Today tab, shown, without loading it again. ⌘Z reopens a closed one over its tab, and so does ⇧⌘T. A Google Meet link opens as a tab of the Space shown instead. With the Settings switch off, the link goes in the pinned tab, ⇧-click opens it in a new tab and its menu has no $in_peek, and back on, a link rule to another Space sends it there as a tab instead. Resting on its keep button shows Open as Tab and its key in a tooltip over it. Windows: $out/preview-shift-click.png, $out/preview-menu.png, $out/preview-menu-chosen.png, $out/preview-open.png, $out/preview-light.png, $out/preview-dark.png, $out/preview-tooltip.png, $out/preview-kept.png, $out/preview-reopened.png, $out/preview-meeting.png, $out/preview-ruled.png"
echo "  The first preview's first frame came $first_frame after the click reached the UI."
! windows || echo "  Its card held its page where the core showed it, and $scaled: $(tr '\n' ' ' < "$out/preview-frames.txt")"
[ "$(uname)" != Linux ] || [ -z "${WAYLAND_DISPLAY:-}" ] || echo "  Under Wayland, $scaled."
[ -z "$recording" ] || echo "  Its motion, opening and kept: $recording"
