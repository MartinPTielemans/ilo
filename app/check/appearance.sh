# check.sh runs this at 410.
# View › Appearance, and Settings' Appearance, give a page its choice as prefers-color-scheme as it
# changes, and Automatic gives back the system's: on the Mac through the app's appearance, which the host sets, and off it
# through the core (bc_set_appearance). Each choice changes the scheme the one before gave, so the
# page says so.
# A Space's theme never reaches a page (#684): what theme.html sees of the browser's colours stays
# the same through a red theme and a blue one of the other look than the system's.

# From a session of its own, on Automatic, so the page it opens is the one the window shows: over
# the Spaces a run before left, the page could open in the agent's Space that run's window showed
# and stay there as the window went back to the user's, where a page doesn't hear the scheme
# change until it's shown (#1791).
serve
log="$out/appearance.log"
step=0
rm -f "$saved" "$launch_frame"
forget_appearance
BROWSER_DRIVE=1 BROWSER_URL="${site}scheme.html" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* scheme ' || fail "the scheme page didn't say its scheme"
system=$(grep '^browser: title [0-9]* scheme ' "$log" | tail -1 | sed 's/.* //')
# It's the tab the window shows, as a page out of sight hears the change only once it's shown.
scheme_tab=$(grep '^browser: title [0-9]* scheme ' "$log" | tail -1 | cut -d' ' -f3)
send state
[ "$(grep '^browser: window [0-9]* shows ' "$log" | tail -1 | cut -d' ' -f5)" = "$scheme_tab" ] ||
  fail "the scheme page, tab $scheme_tab, isn't the one the window shows: $(grep '^browser: window [0-9]* shows ' "$log" | tail -1)"
other=dark
[ "$system" = light ] || other=light
for choice in $other $system $other automatic; do
  case $choice in
  dark) item=Dark scheme=dark ;;
  light) item=Light scheme=light ;;
  *) item=Automatic scheme=$system ;;
  esac
  send menu "Appearance › $item"
  expect "^browser: title [0-9]* scheme $scheme\$" \
    "View › Appearance › $item didn't make the page's prefers-color-scheme $scheme"
done
# Settings › General's Appearance is three drawings (#2429) that pick as View › Appearance does,
# and follow it: a click on the other look's gives the page its scheme, View › Appearance's other
# item moves the ring and gives back the system's, and a click on Automatic's leaves the page's
# scheme as it is. The row is captured in each look.
send menu Settings…
expect '^browser: settings shown General$' "Settings… didn't show General"
settings_drew
was=$system
for choice in $other $system automatic; do
  case $choice in
  dark) item=Dark scheme=dark ;;
  light) item=Light scheme=light ;;
  *) item=Automatic scheme=$system ;;
  esac
  if [ "$choice" = "$system" ]; then
    send menu "Appearance › $item"
  else
    at=$(drawn "appearance option $item") || fail "Settings didn't draw Appearance's $item"
    send clickkey $at
  fi
  expect "^browser: appearance $choice\$" "Appearance's $item didn't change the appearance"
  # The page hears only a change of scheme.
  if [ "$scheme" != "$was" ]; then
    expect "^browser: title [0-9]* scheme $scheme\$" \
      "Appearance's $item didn't make the page's prefers-color-scheme $scheme"
  fi
  was=$scheme
  expect "^browser: settings appearance $item\$" "Settings' Appearance didn't take $item"
  # Light and Dark are each drawn chosen for the first time here, Automatic was at launch.
  if [ "$choice" != automatic ]; then
    expect "^browser: settings appearance $item at " "Settings' Appearance didn't show $item"
    ../tools/window-screenshot.sh "$pid" "$out/settings-appearance-$choice.png" ||
      fail "no window to capture"
  fi
done
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
# On Windows the UI draws Windows' own window buttons (#1244), captured in each appearance at rest,
# with the pointer on maximize, which the frame answers for (Snap layouts) and passes on to the UI,
# and with it on close.
buttons=
if windows; then
  for scheme in light dark; do
    item=Light
    [ $scheme = light ] || item=Dark
    send menu "Appearance › $item"
    expect "^browser: appearance $scheme\$" "View › Appearance › $item didn't change the appearance"
    send move $page_point
    ../tools/window-screenshot.sh "$pid" "$out/window-buttons-$scheme.png" ||
      fail "no window to capture"
    send move 43 24
    expect '^browser: window button Zoom hovered$' "the pointer on maximize didn't reach its button"
    ../tools/window-screenshot.sh "$pid" "$out/window-buttons-$scheme-maximize.png" ||
      fail "no window to capture"
    send move 66 24
    expect '^browser: window button Close hovered$' "the pointer on close didn't reach its button"
    ../tools/window-screenshot.sh "$pid" "$out/window-buttons-$scheme-close.png" ||
      fail "no window to capture"
  done
  send move $page_point
  send menu "Appearance › Automatic"
  buttons="; Windows' window buttons in $out/window-buttons-*.png"
fi
# Pages never read a Space's theme (#684): all a page can see of the browser's colours, its
# prefers-color-scheme and the rest of what theme.html reads, stays as it was when the Space takes a
# red theme, then a blue one whose look is the other of the system's, as a dark theme on a light
# Mac's. The theme picker opens from the command bar's Edit Theme…, with the keyboard, so its
# clicks are in its points: its arrow 13 wide, then the card, whose field, an HSL disc, has its
# middle at 178.5 down, and its appearance buttons at 49 down, light 178.3 and dark 218.7 along.
go_from_bar "${site#http://}theme.html"
expect '^browser: title [0-9]* seen ' "the theme page didn't say what it sees"
seen=$(grep '^browser: title [0-9]* seen ' "$log" | tail -1 | sed 's/^browser: title [0-9]* //')
# Reloads the page and fails with $1 unless it sees what it saw first.
sees_the_same() {
  before=$(grep -c '^browser: title [0-9]* seen ' "$log")
  send key r cmd
  for _ in $(seq 100); do
    [ "$(grep -c '^browser: title [0-9]* seen ' "$log")" -gt "$before" ] && break
    sleep 0.1
  done
  now=$(grep '^browser: title [0-9]* seen ' "$log" | tail -1 | sed 's/^browser: title [0-9]* //')
  [ "$(grep -c '^browser: title [0-9]* seen ' "$log")" -gt "$before" ] ||
    fail "$1: the reloaded page didn't say what it sees"
  [ "$now" = "$seen" ] || fail "$1: the page saw \"$now\", not \"$seen\""
}
open_picker() {
  bar_command edit-theme "Edit Theme…" theme
  expect '^browser: theme picker shown$' "Edit Theme… didn't show the theme picker"
  expect '^browser: theme picker drawn$' "the theme picker didn't draw itself on screen"
}
close_picker() {
  send key escape
  expect '^browser: theme picker hidden$' "escape didn't hide the theme picker"
}
open_picker
send clickkey 276.5 178.5
expect '^browser: themed space ' "a click in the theme picker's field didn't theme the Space"
close_picker
sees_the_same "with a red theme"
look=dark at="231.7 49"
[ "$system" = light ] || look=light at="191.3 49"
# The other look, then the blue swatch, the eighth, which the keyboard reaches, as the field takes
# no click once the Space has a colour: tab to the appearance buttons, to − and +, then to the
# swatches' first, and → to it.
open_picker
send clickkey $at
expect '^browser: themed space ' "the theme picker's $look button didn't change the Space's theme"
for key in tab tab tab right right right right right right right space; do
  send key $key
done
expect '^browser: colored space [0-9]* 7eb6d5$' "the blue swatch didn't give the Space its colour"
close_picker
grep -q "^browser: theme picker set [0-9]* .*appearance=$look\$" "$log" ||
  fail "the theme picker's $look button didn't give the theme a $look look"
sees_the_same "with a $look blue theme in a $system system"
../tools/window-screenshot.sh "$pid" "$out/theme-unseen.png" || fail "no window to capture"
# None of the page's colours is one of the Space's.
for color in $(sed -n 's/^browser: colored space [0-9]* \([0-9a-f]\{6\}\)$/\1/p' "$log"); do
  case " $seen " in *"=$color "*) fail "the page sees the Space's colour $color: $seen" ;; esac
done
exec 3>&-
quit
echo "ok: View › Appearance gives the page its choice as prefers-color-scheme as it changes, and Automatic the system's ($system)," \
  "and Settings' Appearance picks and follows it, captured in $out/settings-appearance-*.png$buttons;" \
  "a red theme and a $look blue one left all the page sees as it was: $seen"
