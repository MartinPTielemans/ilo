# check.sh runs this at 677.
# On Windows, the four panels the host floats in a window of their own size, the command bar, the
# tab switcher, the find bar and the offer, draw their shadow themselves, in a margin round them in
# that window, as the menus do: Windows' own shadow came only with its own corner, which left a
# sliver of the window round the panel's (#2988). Each panel, over a white page and a dark one, in
# Light and in Dark, at the display's 100% and 150%: the margin never shows, as every pixel of the
# page on its window's outer edge is as it was before the panel opened, and the page under the
# panel's bottom edge is darker, its shadow. Each closed leaves no shadow behind: the page under
# where the panel and its shadow were is as it was. The command bar is closed every way a user
# leaves it, with Escape, a click further out, a click in its margin over a link, another app,
# Win+D and minimizing, and the click in its margin is one outside it: the bar closes and the link
# opens, as a click further out reaches the page. Captured as panel-shadows-<page>-<appearance>-
# <scale>-<panel>.png, and panel-shadows-out-<way>.png after each way out. Past the browser window's
# frame, over another app's window under it, each panel's margin is clipped and its panel isn't:
# panel-shadows-edge.txt says how far each reaches past each edge and how much of that is margin,
# that the panel shows whole, that a press in the margin there reaches the other app and not ilo,
# and that the screen there is the same with the panel open as closed.
# Real input needs CI's hosted desktop, and the display's scale is the runner's own to change.

if ! windows; then
  echo "skip: panel-shadows. The Mac's and Linux's hosts draw these panels' shadows in a window under the panel, not in the panel's own (#2988)."
  exit 0
fi
if [ -z "${CI:-}" ]; then
  echo "skip: panel-shadows. Win+D, another app and the display's scale need CI's desktop, which is the check's own."
  exit 0
fi

serve
# Each page's colour fills it, and nothing else on it shows: a link over all but its bottom 80
# pixels, to its twin, and under that a strip that counts presses. Its twin and the page a login
# lands on look the same, so a capture of one is a capture of the other. The login page posts to
# its folder's login, which the site answers with that folder's welcome.html.
for colour in white:#ffffff dark:#1e1e1e; do
  name=${colour%%:*} fill=${colour#*:}
  mkdir -p "$out/site/$name"
  for page in page:twin.html:ready twin:page.html:twin welcome:page.html:welcome; do
    file=${page%%:*} to=${page#*:} title=${page##*:}
    to=${to%%:*}
    printf '%s' "<title>loading</title>
<style>html, body { margin: 0; height: 100%; background: $fill }</style>
<a href=\"$to\" style=\"position:fixed;left:0;top:0;width:100%;height:calc(100% - 80px)\"></a>
<div id=press style=\"position:fixed;left:0;bottom:0;width:100%;height:80px\"></div>
<script>
let presses = 0;
press.onmousedown = () => document.title = \"pressed \" + ++presses;
requestAnimationFrame(() => requestAnimationFrame(() => document.title = \"$title\"));
</script>" > "$out/site/$name/$file.html"
  done
  printf '%s' "<title>loading</title>
<style>html, body { margin: 0; height: 100%; background: $fill }
input { position: fixed; left: 0; width: 100%; height: 50%; border: 0; background: transparent;
  font: 24px system-ui; color: gray }</style>
<form method=post action=login>
<input name=username autocomplete=username style=\"top:0\">
<input name=password type=password autocomplete=current-password style=\"top:50%\">
<button style=\"position:fixed;right:0;bottom:0;width:1px;height:1px\"></button>
</form>
<script>requestAnimationFrame(() => requestAnimationFrame(() => document.title = \"login\"));</script>" \
    > "$out/site/$name/login.html"
done

log="$out/panel-shadows.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="${site}white/page.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the white page didn't draw"
m=$(today | awk '{ print $1 }')
# The page's rectangle in points, from the layout: $press_point in the strip that counts presses,
# $user_point in a login page's username field, and $page_top and $page_bottom.
page_points() {
  send layout
  set -- $(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' | sed -n \
    's/^browser: layout content \([0-9]*\)x\([0-9]*\),.*WebContentsViewCocoa \(-*[0-9]*\),\(-*[0-9]*\) \([0-9]*\)x\([0-9]*\).*/\1 \2 \3 \4 \5 \6/p')
  [ $# = 6 ] || fail "the layout named no page"
  # The layout's y counts from the content's bottom.
  page_top=$(($2 - $4 - $6)) page_bottom=$(($2 - $4))
  press_point="$(($3 + $5 / 2)) $((page_bottom - 40))"
  user_point="$(($3 + $5 / 2)) $((page_top + $6 / 4))"
}
# Shows the tab $1, wherever Today has it, unless it's shown.
show_tab() {
  local i=0 id
  [ "$(grep '^browser: selected ' "$log" | tail -1 | cut -d' ' -f3)" != "$1" ] || return 0
  for id in $(today); do
    [ "$id" = "$1" ] && break
    i=$((i + 1))
  done
  send click $(row $i)
  expect "^browser: selected $1\( in \|\$\)" "a click on row $i of Today didn't show its tab $1"
}
# Has the tab shown, the run's or $4, show $1's page $2, whose title says $3 once it's drawn.
go_to() {
  go_from_bar "${site}$1/$2.html"
  expect "^browser: title ${4:-$m} $3\$" "the $1 $2 page didn't draw"
}
# The panel $1's line since the step $opened: "X Y W H in its window X Y W H".
panel_line() {
  sed -n "/^browser: drive $opened: /,\$p" "$log" | grep "^browser: $1 panel " | tail -1 |
    sed "s/^browser: $1 panel //"
}
shot() {
  ../tools/window-screenshot.sh "$pid" "$1" all || fail "no window to capture"
}
# Rests the pointer on the page's press strip, and waits for a tab's hover card, which a click on
# a sidebar row can leave, to be off the screen, so a capture before a panel opens has only the page
# where the panel and its shadow go.
rest_pointer() {
  send move $press_point
  for _ in $(seq 20); do
    send stack
    sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: stack ' | grep -q 'hover card' ||
      return 0
    sleep 0.1
  done
  fail "a tab's hover card stayed on screen with the pointer on the page: $(grep '^browser: stack ' "$log" | tail -1)"
}
# Captures the panel $1, opened from the step $opened over $before, as $2, until its margin and
# shadow read as they should, its shadow $3 levels darker than the page under its bottom edge at
# least, for up to 10 captures, as the panel's first frames may still be on their way.
shadow_shown() {
  local said=
  for _ in $(seq 10); do
    line=$(panel_line "$1")
    if [ -n "$line" ]; then
      shot "$2"
      said=$(python3 -I check/panel-shadow.py shown "$before" "$2" "${line%% in its window *}" \
        "${line##* in its window }" "$3") && return 0
    else
      sleep 0.2
    fi
  done
  fail "the $1's margin showed, or its shadow didn't, in $2: ${said:-it logged no panel line since step $opened}"
}
# Captures what's left once the panel $1 has closed, as $2, until the page where it and its shadow
# were is as it was in $before, for up to 10 captures.
shadow_gone() {
  local said=
  for _ in $(seq 10); do
    shot "$2"
    said=$(python3 -I check/panel-shadow.py gone "$before" "$2" "${line%% in its window *}" \
      "${line##* in its window }") && return 0
  done
  fail "after $3, the $1 left its shadow on the page, in $2: $said"
}
# The page hears a click in its strip, as it does once ilo's window is back and active.
page_hears() {
  local pressed
  pressed=$(grep -c "^browser: title $m pressed " "$log" || true)
  send click $press_point
  for _ in $(seq 100); do
    [ "$(grep -c "^browser: title $m pressed " "$log" || true)" -gt "$pressed" ] && return 0
    sleep 0.1
  done
  fail "the page didn't hear a click $1"
}
offers=0
# Opens the panel $1 over the page $2 as a user does, having captured the page as $3.before.png.
open_panel() {
  case "$1" in
    'command bar')
      before="$3.before.png"
      shot "$before"
      opened=$((step + 1))
      send key t cmd
      expect '^browser: command bar shown$' "⌘T didn't open the command bar"
      ;;
    'find bar')
      send click $press_point
      before="$3.before.png"
      shot "$before"
      opened=$((step + 1))
      post key f cmd
      expect '^browser: find bar shown$' "⌘F didn't open the find bar"
      ;;
    # ⌃Tab twice, ⌃ held, shows it at once.
    'tab switcher')
      send click $press_point
      before="$3.before.png"
      shot "$before"
      opened=$((step + 1))
      send key tab ctrl
      pressed=$step
      send key tab ctrl
      expect '^browser: tab switcher shown$' "⌃Tab twice with ⌃ held didn't show the tab switcher" "$pressed"
      ;;
    # Over the page a login lands on, which before is taken of first. Each login has a username of
    # its own, as Chrome stops offering for a site and username once a few offers went unanswered.
    offer)
      go_to "$2" welcome welcome
      before="$3.before.png"
      shot "$before"
      go_to "$2" login login
      send click $user_point
      offers=$((offers + 1))
      opened=$((step + 1))
      send key a cmd
      send type "check-shadow-$offers@example.test"
      send key tab
      send key a cmd
      send type "shadow-$offers"
      send key return
      expect '^browser: offer shown$' "a login on the $2 login page brought no offer" "$opened"
      ;;
  esac
}
# Closes the panel $1, opened by open_panel, as a user does.
close_panel() {
  case "$1" in
    'command bar' | 'find bar')
      send key escape
      expect "^browser: $1 hidden\$" "Escape didn't close the $1"
      ;;
    'tab switcher')
      send flags
      expect '^browser: tab switcher hidden$' "letting go of Ctrl didn't close the tab switcher"
      show_tab $m
      ;;
    # The offer doesn't take the keyboard: another tab shown closes it, as passwords' does.
    offer)
      show_tab $other
      expect '^browser: offer hidden$' "showing the other tab didn't close the offer"
      show_tab $m
      ;;
  esac
}
# Opens the panel $1 over the page $2, captures it as $3 and checks its margin and shadow, closes
# it, and checks no shadow is left.
panel() {
  rest_pointer
  open_panel "$@"
  case "$2" in white) darken=10 ;; *) darken=2 ;; esac
  shadow_shown "$1" "$3.png" "$darken"
  close_panel "$1"
  shadow_gone "$1" "$3.gone.png" "closing it"
  [ "$1" != offer ] || go_to "$2" page ready
}

# The tab switcher's other tab, $other.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send insert "${site}white/twin.html"
send key return
expect '^browser: title [0-9]* twin$' "the second tab didn't draw"
other=$(today | tr ' ' '\n' | grep -vx "$m" | head -1)
show_tab $m
page_points
send click $press_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: panel-shadows. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
logged '^browser: drive: real press' || fail "CI's desktop gave no real input, which the run's events need"

read -r scale_at scale_least scale_most << EOF
$(python3 -I check/scale.py get)
EOF
scales=100
# 150% is two steps up from 100%, the display's own on the runner.
if [ -n "$scale_most" ] && [ "$scale_most" -ge $((scale_at + 2)) ]; then
  scales="100 150"
else
  echo "SKIP: panel-shadows at 150%, as the display goes from step $scale_least to $scale_most, at $scale_at"
fi
rounds=0
for scale in $scales; do
  if [ "$scale" = 150 ]; then
    page_points
    content_was="$page_top $page_bottom"
    run_undo="python3 -I check/scale.py set $scale_at"
    dpi=$(python3 -I check/scale.py set $((scale_at + 2)))
    # The helper's own window still at 100%'s 96 is the runner's display not changing its scale,
    # as on windows-2025's, which has no GPU; ilo has no part in it.
    if [ "$dpi" = 96 ]; then
      python3 -I check/scale.py set "$scale_at" > /dev/null
      run_undo=
      scales=100
      echo "SKIP: panel-shadows at 150%, as two steps up left this runner's display at 96 DPI"
      break
    fi
    [ "$dpi" = 144 ] || fail "two steps up the display's scale made it $dpi DPI, not 150%'s 144"
    # The UI lays out again once it has heard the scale: the page in points moves.
    for _ in $(seq 20); do
      page_points
      [ "$page_top $page_bottom" != "$content_was" ] && break
      sleep 0.5
    done
    [ "$page_top $page_bottom" != "$content_was" ] ||
      fail "the page's layout in points didn't change at the display's 150%"
  fi
  page_points
  for page in white dark; do
    # ⌃Tab shows the other tab behind the tab switcher, which shows the page's colour's twin, so
    # what's behind the switcher's margin is the colour it was before.
    show_tab $other
    go_to $page twin twin $other
    show_tab $m
    go_to $page page ready
    for appearance in Light Dark; do
      light=false
      [ $appearance = Dark ] || light=true
      if ! grep '^browser: window light ' "$log" | tail -1 | grep -q " $light\$"; then
        send menu "$(cased "Appearance › $appearance")"
        expect "^browser: window light $light\$" "View › Appearance › $appearance didn't change the window"
      fi
      for name in 'command bar' 'find bar' 'tab switcher' offer; do
        panel "$name" $page "$out/panel-shadows-$page-$(echo $appearance | tr A-Z a-z)-$scale-$(echo "$name" | tr ' ' -)"
        rounds=$((rounds + 1))
      done
    done
  done
done
if [ "$scales" = "100 150" ]; then
  python3 -I check/scale.py set "$scale_at" > /dev/null
  run_undo=
fi

# The command bar closed each way a user leaves it, over the white page in Light at 100%. Each opens
# it over $before and leaves $line its panel's.
send menu "$(cased "Appearance › Light")"
expect '^browser: window light true$' "View › Appearance › Light didn't change the window"
go_to white page ready
# Back at 100%, once the UI has laid out at it again.
if [ -n "${content_was:-}" ]; then
  for _ in $(seq 20); do
    page_points
    [ "$page_top $page_bottom" = "$content_was" ] && break
    sleep 0.5
  done
  [ "$page_top $page_bottom" = "$content_was" ] ||
    fail "back at the display's 100%, the page's layout in points was $page_top to $page_bottom, not $content_was"
fi
bar_out() {
  before="$out/panel-shadows-out-$1.before.png"
  shot "$before"
  opened=$((step + 1))
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar before $2"
  shadow_shown 'command bar' "$out/panel-shadows-out-$1.shown.png" 10
}
# A click further out reaches the page, and the bar closes.
bar_out further "a click further out"
pressed=$(grep -c "^browser: title $m pressed " "$log" || true)
send click $press_point
expect '^browser: command bar hidden$' "a click on the page further out than the bar's margin didn't close it"
for _ in $(seq 50); do
  [ "$(grep -c "^browser: title $m pressed " "$log" || true)" -gt "$pressed" ] && break
  sleep 0.1
done
[ "$(grep -c "^browser: title $m pressed " "$log" || true)" -gt "$pressed" ] ||
  fail "a click on the page further out than the command bar's margin didn't reach the page"
shadow_gone 'command bar' "$out/panel-shadows-out-further.png" "a click further out"

# A click in the margin under the bar, where its shadow is, over the page's link: the host hit-tests
# the margin as outside the panel (HTTRANSPARENT), so the click goes to the browser window under it,
# and the link opens its twin, which looks the same.
bar_out margin "a click in its margin"
set -- ${line%% in its window *}
margin_point="$(($1 + $3 / 2)) $(($2 + $4 + 40))"
[ $(($2 + $4 + 40)) -gt "$page_top" ] && [ $(($2 + $4 + 40)) -lt $((page_bottom - 80)) ] ||
  fail "the command bar's margin, 40 points under its panel at $line, isn't over the page's link, from $page_top to $((page_bottom - 80))"
send click $margin_point
expect '^browser: command bar hidden$' "a click in the command bar's margin, at $margin_point, didn't close it as a click outside does"
expect "^browser: title $m twin\$" "a click in the command bar's margin, at $margin_point, didn't reach the page's link under it"
shadow_gone 'command bar' "$out/panel-shadows-out-margin.png" "a click in its margin"
go_to white page ready

# Another app's window over ilo's, with the foreground in a window of that app's.
bar_out app "another app"
covered=$step
python3 -I -u check/app-windows.py cover "$(cat "/proc/$pid/winpid")" away \
  > "$out/panel-shadows-cover.log" 2>&1 &
helper=$!
for _ in $(seq 100); do
  grep -q '^cover ' "$out/panel-shadows-cover.log" || ! kill -0 "$helper" 2>/dev/null && break
  sleep 0.1
done
grep -q '^cover ' "$out/panel-shadows-cover.log" ||
  fail "another app's window didn't open over ilo's: $(cat "$out/panel-shadows-cover.log")"
expect '^browser: command bar hidden$' "another app taking the foreground didn't close the command bar" "$covered"
# By its Windows id, as Git Bash's kill may not end a native process.
taskkill //F //PID "$(cat "/proc/$helper/winpid")" > /dev/null 2>&1 || true
for _ in $(seq 50); do
  send stack
  grep '^browser: stack ' "$log" | tail -1 | grep -q 'other cover' || break
  sleep 0.1
done
page_hears "once the other app had gone"
shadow_gone 'command bar' "$out/panel-shadows-out-app.png" "another app"

# Win+D and back, real keys.
bar_out win-d "Win+D"
send realkey d win
expect '^browser: command bar hidden$' "Win+D didn't close the command bar"
send realkey d win
page_hears "once the window was back from Win+D"
shadow_gone 'command bar' "$out/panel-shadows-out-win-d.png" "Win+D"

# Minimized and back, as #2616's left the bar standing alone where the window was.
bar_out minimize "minimizing"
send minimize
expect '^browser: command bar hidden$' "minimizing the window didn't close the command bar"
send minimize
page_hears "once the window was back from minimized"
shadow_gone 'command bar' "$out/panel-shadows-out-minimize.png" "minimizing"

# Past the browser window's edge. HTTRANSPARENT hands a press on a panel's margin only to this
# thread's windows under it, so past the browser window's frame, over another app's, the margin
# would take the press: the host clips each panel's margin to the frame, and leaves the panel whole.
# With another app's window under ilo's, reaching past its frame, each panel says how far its
# window reaches past each edge, and how much of that is margin; its panel's left and right edges
# show; past the side it reaches furthest, a press in the margin reaches the other app and not ilo,
# and the screen there is as it is once the panel has closed, with no shadow past the edge.
send place 300 150 1100 700
was_bottom=$page_bottom
for _ in $(seq 20); do
  page_points
  [ "$page_bottom" != "$was_bottom" ] && break
  sleep 0.2
done
[ "$page_bottom" != "$was_bottom" ] || fail "the window placed at 1100 by 700 kept its page's layout"
under_log="$out/panel-shadows-under.log"
# Opens the other app's window under ilo's, which reads the frame's size, the browser window's own
# rectangle round it and its content's top left, from the frame's top left.
open_under() {
  # Emptied here, not by the helper's own redirect, which Git Bash opens some time after the fork:
  # until then the loop can read the line the helper before this one left (#4035).
  : > "$under_log"
  python3 -I -u check/under-window.py open "$(cat "/proc/$pid/winpid")" > "$under_log" 2>&1 &
  under=$!
  for _ in $(seq 100); do
    grep -q '^under ' "$under_log" || ! kill -0 "$under" 2>/dev/null && break
    sleep 0.1
  done
  set -- $(grep -m1 '^under ' "$under_log" |
    sed 's/^under [^,]*, frame \(.*\), window \(.*\), client \(.*\)$/\1 \2 \3/')
  [ $# = 10 ] || fail "another app's window didn't open under ilo's: $(cat "$under_log")"
  frame_width=$(($3 - $1)) frame_height=$(($4 - $2))
  own_left=$5 own_top=$6 own_right=$7 own_bottom=$8 client_x=$9 client_y=${10}
}
open_under
edges="$out/panel-shadows-edge.txt"
: > "$edges"
# The panel $1, at $2 $3, $4 by $5 from the frame's top left, shows whole: its left and right edges,
# across its middle rows, which its corners leave straight, are none of the other app's colour,
# and where the panel itself reaches past the frame, all of that part shows the panel too. Only
# its margin is cut at the frame.
panel_whole() {
  local rows_top=$(($3 + $5 / 2 - 2)) rows_bottom=$(($3 + $5 / 2 + 2)) from to said side
  for side in left right; do
    case $side in
      left) from=$(($2 + 1)) to=$(($2 < 0 ? 0 : $2 + 5)) ;;
      right) from=$(($2 + $4 > frame_width ? frame_width : $2 + $4 - 5)) to=$(($2 + $4 - 1)) ;;
    esac
    # Read again until the panel has drawn there, as its line comes as it's placed.
    for _ in $(seq 20); do
      said=$(python3 -I check/under-window.py strip "$(cat "/proc/$pid/winpid")" \
        $from $rows_top $to $rows_bottom "$out/panel-shadows-edge-panel.raw") ||
        fail "couldn't read the $1's panel at its $side edge: $said"
      case $said in *", 0 the other app's colour") break ;; esac
      sleep 0.1
    done
    case $said in
      *", 0 the other app's colour") ;;
      *) fail "the $1's panel, at $2 $3, $4 by $5 in a frame $frame_width by $frame_height, is cut at its $side edge, which shows the other app's window: $said" ;;
    esac
    echo "the $1's panel at its $side edge, $said" >> "$edges"
  done
}
# The panel $1, open since the step $opened: how far its window reaches past each edge of the frame,
# and past the side it reaches furthest, a press, which the other app hears and ilo doesn't, unless
# $3 says why not, with the screen there read as $2.shown.raw. False where it reaches past none.
edge() {
  local x y w h left right above below most side box
  local name=$1 file=$2 why=${3:-}
  line=
  for _ in $(seq 20); do
    line=$(panel_line "$1")
    [ -n "$line" ] && break
    sleep 0.1
  done
  [ -n "$line" ] || fail "the $1 logged no panel line since step $opened"
  local px py pw ph
  set -- ${line%% in its window *}
  px=$1 py=$2 pw=$3 ph=$4
  set -- "$name" "$file" "$why" ${line##* in its window }
  local nopress=$3
  x=$4 y=$5 w=$6 h=$7
  pressed_at=
  left=$((-x)) right=$((x + w - frame_width)) above=$((-y)) below=$((y + h - frame_height))
  [ $left -gt 0 ] || left=0
  [ $right -gt 0 ] || right=0
  [ $above -gt 0 ] || above=0
  [ $below -gt 0 ] || below=0
  # How much of each reach is the panel's own, not its margin's.
  local panel_left=$((-px)) panel_right=$((px + pw - frame_width)) panel_above=$((-py))
  local panel_below=$((py + ph - frame_height))
  [ $panel_left -gt 0 ] || panel_left=0
  [ $panel_right -gt 0 ] || panel_right=0
  [ $panel_above -gt 0 ] || panel_above=0
  [ $panel_below -gt 0 ] || panel_below=0
  echo "the $1's window, at $x $y, $w by $h, in a frame $frame_width by $frame_height, reaches past its left by $left, of which margin $((left - panel_left)), its right by $right, of which margin $((right - panel_right)), its top by $above, of which margin $((above - panel_above)), its bottom by $below, of which margin $((below - panel_below)); its panel is at $px $py, $pw by $ph, in a margin of $((px - x)) at the left, $((x + w - px - pw)) at the right, $((py - y)) above and $((y + h - py - ph)) below" >> "$edges"
  panel_whole "$1" $px $py $pw $ph
  most=$left side=left
  [ $right -le $most ] || { most=$right side=right; }
  [ $above -le $most ] || { most=$above side=top; }
  [ $below -le $most ] || { most=$below side=bottom; }
  [ $most -gt 0 ] || return 1
  # The part of its window past that side, and a point in its middle, in pixels from the frame.
  # Past the browser window's own rectangle too: the clear border Windows leaves round its frame is
  # the browser window's, which takes a press there to resize it, panel or not. In the content's
  # points, as the driver clicks, at 100%.
  local from to
  case $side in
    left)
      to=$((own_left < 0 ? own_left : 0))
      to=$((px < to ? px : to))
      box="$x $y $((px < 0 ? px : 0)) $((y + h))" from=$x
      press="$(((from + to) / 2 - client_x)) $((y + h / 2 - client_y))"
      ;;
    right)
      from=$((px + pw > frame_width ? px + pw : frame_width))
      box="$from $y $((x + w)) $((y + h))" to=$((x + w))
      from=$((own_right > from ? own_right : from))
      press="$(((from + to) / 2 - client_x)) $((y + h / 2 - client_y))"
      ;;
    top)
      to=$((own_top < 0 ? own_top : 0))
      to=$((py < to ? py : to))
      box="$x $y $((x + w)) $((py < 0 ? py : 0))" from=$y
      press="$((x + w / 2 - client_x)) $(((from + to) / 2 - client_y))"
      ;;
    bottom)
      from=$((py + ph > frame_height ? py + ph : frame_height))
      box="$x $from $((x + w)) $((y + h))" to=$((y + h))
      from=$((own_bottom > from ? own_bottom : from))
      press="$((x + w / 2 - client_x)) $(((from + to) / 2 - client_y))"
      ;;
  esac
  [ $from -lt $to ] || nopress="all of it is over the browser window's own border round its frame, from $from to $to, which takes a press there to resize it, panel or not"
  # Beside the frame: along that side, it's kept to the frame's length.
  set -- "$1" "$2" "$3" $box
  case $side in
    left | right)
      [ $5 -ge 0 ] || set -- "$1" "$2" "$3" $4 0 $6 $7
      [ $7 -le $frame_height ] || set -- "$1" "$2" "$3" $4 $5 $6 $frame_height
      ;;
    *)
      [ $4 -ge 0 ] || set -- "$1" "$2" "$3" 0 $5 $6 $7
      [ $6 -le $frame_width ] || set -- "$1" "$2" "$3" $4 $5 $frame_width $7
      ;;
  esac
  strip_box="$4 $5 $6 $7"
  set -- "$name" "$file"
  python3 -I check/under-window.py strip "$(cat "/proc/$pid/winpid")" $strip_box "$2.shown.raw" \
    >> "$edges" || fail "couldn't read the screen past the $1's frame: $(tail -1 "$edges")"
  if [ -n "$nopress" ]; then
    echo "no press past the $1's frame: $nopress" >> "$edges"
    return 0
  fi
  local pressed
  pressed=$(grep -c '^pressed ' "$under_log" || true)
  send click $press
  pressed_at=$step
  for _ in $(seq 50); do
    [ "$(grep -c '^pressed ' "$under_log" || true)" -gt "$pressed" ] && break
    sleep 0.1
  done
  [ "$(grep -c '^pressed ' "$under_log" || true)" -gt "$pressed" ] ||
    fail "a press past the frame's $side edge at $press in the content's points, in the $1's margin over the other app's window, didn't reach that app: ilo heard $(logged '^browser: drive: real press in' && sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: drive: real press in' || echo nothing)"
  ! logged '^browser: drive: real press in' "$pressed_at" ||
    fail "ilo heard the press past the frame's $side edge at $press, which the other app also heard: $(sed -n "/^browser: drive $pressed_at: /,\$p" "$log" | grep -m1 '^browser: drive: real press in')"
  echo "a press at $press in the content's points, past the frame's $side edge in the $1's margin, reached the other app, $(tail -1 "$under_log") from the frame, and not ilo" >> "$edges"
}
# The screen past the edge, read as $2.shown.raw while the panel $1 showed, is as it is now it's
# closed: its shadow didn't draw past the frame.
edge_gone() {
  python3 -I check/under-window.py strip "$(cat "/proc/$pid/winpid")" $strip_box "$2.gone.raw" \
    "$2.shown.raw" >> "$edges" ||
    fail "the $1's shadow drew past the browser window's frame, over the other app's window: $(tail -1 "$edges")"
}
go_to white page ready
for name in 'command bar' 'find bar' 'tab switcher' offer; do
  file="$out/panel-shadows-edge-$(echo "$name" | tr ' ' -)"
  rest_pointer
  open_panel "$name" white "$file"
  why=
  [ "$name" != 'tab switcher' ] || why="the driver lets go of the Ctrl that keeps the tab switcher open before a click"
  if ! edge "$name" "$file" "$why"; then
    close_panel "$name"
    [ "$name" != offer ] || go_to white page ready
    continue
  fi
  # The other app took the foreground with a press: ilo's window back in front, the panel closed
  # if it's still open, as a user would.
  [ -n "$pressed_at" ] || name="$name unpressed"
  case "$name" in
    *' unpressed')
      name=${name% unpressed}
      close_panel "$name"
      ;;
    'command bar')
      expect '^browser: command bar hidden$' "a press on another app's window past the frame didn't close the command bar, as a click further out does" "$pressed_at"
      page_hears "once the other app had the press"
      ;;
    'find bar')
      page_hears "once the other app had the press"
      if ! logged '^browser: find bar hidden$' "$pressed_at"; then
        post key f cmd
        send key escape
        expect '^browser: find bar hidden$' "Escape didn't close the find bar" "$pressed_at"
      fi
      ;;
    offer)
      page_hears "once the other app had the press"
      logged '^browser: offer hidden$' "$pressed_at" || close_panel offer
      ;;
  esac
  edge_gone "$name" "$file"
  [ "$name" != offer ] || go_to white page ready
done
# The command bar over the URL field, a click on which opens it there, at its field's left.
rest_pointer
opened=$((step + 1))
send click 90 50
expect '^browser: command bar opens over the URL field$' "a click on the URL field at 90,50 didn't open the command bar over the field"
expect '^browser: command bar shown$' "a click on the URL field didn't open the command bar"
file="$out/panel-shadows-edge-field-bar"
if edge 'command bar' "$file" && [ -n "$pressed_at" ]; then
  expect '^browser: command bar hidden$' "a press on another app's window past the frame didn't close the command bar over the URL field" "$pressed_at"
  page_hears "once the other app had the press"
else
  send key escape
  expect '^browser: command bar hidden$' "Escape didn't close the command bar over the URL field"
fi
[ ! -f "$file.shown.raw" ] || edge_gone 'command bar' "$file"
# The command bar in the middle of a window narrower than it and its margin.
send place 300 150 760 520
for _ in $(seq 20); do
  page_points
  [ "$page_bottom" != "$was_bottom" ] && [ "$page_bottom" -lt 520 ] && break
  sleep 0.2
done
taskkill //F //PID "$(cat "/proc/$under/winpid")" > /dev/null 2>&1 || true
open_under
rest_pointer
file="$out/panel-shadows-edge-narrow-command-bar"
open_panel 'command bar' white "$file"
# The bar is wider than this window's frame: the capture shows its panel whole past it.
shot "$file.png"
if edge 'command bar' "$file" && [ -n "$pressed_at" ]; then
  expect '^browser: command bar hidden$' "a press on another app's window past the narrow frame didn't close the command bar" "$pressed_at"
  page_hears "once the other app had the press"
else
  close_panel 'command bar'
fi
[ ! -f "$file.shown.raw" ] || edge_gone 'command bar' "$file"
taskkill //F //PID "$(cat "/proc/$under/winpid")" > /dev/null 2>&1 || true
cat "$edges"

exec 3>&-
quit
echo "ok: panel-shadows. The command bar, find bar, tab switcher and offer, in $rounds rounds over a white and a dark page, in Light and Dark, at ${scales// /% and }%: the margin never showed and the shadow did, and none was left once each closed. The command bar closed with Escape, a click further out, a click in its margin, which opened the link under it, another app, Win+D and minimizing, its shadow gone each time. Past the browser window's frame, each panel showed whole, a press in its margin reached the other app's window under it and not ilo, and no shadow drew there."
