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
# <scale>-<panel>.png, and panel-shadows-out-<way>.png after each way out.
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
# Opens the panel $1 over the page $2 as a user does, captures it as $3 and checks its margin and
# shadow, closes it, and checks no shadow is left.
panel() {
  rest_pointer
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
  case "$2" in white) darken=10 ;; *) darken=2 ;; esac
  shadow_shown "$1" "$3.png" "$darken"
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

exec 3>&-
quit
echo "ok: panel-shadows. The command bar, find bar, tab switcher and offer, in $rounds rounds over a white and a dark page, in Light and Dark, at ${scales// /% and }%: the margin never showed and the shadow did, and none was left once each closed. The command bar closed with Escape, a click further out, a click in its margin, which opened the link under it, another app, Win+D and minimizing, its shadow gone each time."
