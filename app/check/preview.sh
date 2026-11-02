# The link preview. A link to another site in a pinned tab's page opens in a card over the window,
# its page in a tab of no Space, and the pinned tab stays on its page. Esc closes it and its page.
# ⌘↩ keeps it as the top Today tab, shown, without loading it again. A click on the dim closes it,
# and the page behind doesn't get the click, and so does Settings taking the keyboard. ⌘W with its
# page focused closes it, and not the tab behind. ⌘Z reopens a closed one over its tab at its page,
# and so does ⇧⌘T while no tab has closed since. A favorite's link opens it too. A call's link, such
# as Google Meet's, opens as a tab instead. With the Settings switch off, the link goes in the
# pinned tab. Back on, a link rule to another Space sends the link there as a tab instead. Reports
# how long the first preview took to draw its first frame. Captures it at rest in a light and a dark
# Space theme, and records it opening and growing into a tab, for its look (#1373).

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
  kill "$server"
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

# The pinned tab's row, 47 down once the favorite's row of tiles is over the Space's rows.
tiles=0
home_row() {
  echo "100 $(($(pinned_row 0 | cut -d' ' -f2) + tiles))"
}

# Where Settings last drew what $1 names, as the platforms' fonts move its tabs and rows.
settings_at() {
  await "^browser: settings $1 at " || fail "Settings didn't draw its $1"
  grep "^browser: settings $1 at " "$log" | tail -1 | sed "s/^browser: settings $1 at //"
}

# Pinned, the tab's links to other sites open in the preview, which grows from the link.
send key d cmd
expect '^browser: command toggle-pin via page$' "⌘D with the page focused didn't reach the UI"
expect "^browser: pinned $a\$" "⌘D didn't pin the test page's tab: the pinned tabs are \"$(pinned)\""
open_preview "from the pinned tab"
first_frame=$(grep -m1 '^browser: link preview first frame in ' "$log" | sed 's/.* in //')
[ -n "$first_frame" ] || fail "the preview didn't log its first frame"
../tools/window-screenshot.sh "$pid" "$out/preview-open.png" || fail "no window to capture"

# Esc in its page closes it, and its page once it has left.
send key escape
in_order "after Esc" '^browser: link preview closing (escape)$' '^browser: link preview released$' \
  '^browser: link preview hidden$' "^browser: tab $p closed\$"
still_home "after Esc"

# Its look in a light and a dark Space theme: the theme picker's first swatch, then the sun, and
# later the moon, each 13 right of its appearance-x token, as cards.sh and little.sh pick them.
peek_look() { # <light|dark> <the appearance button's x> [swatch]
  post rightclick 100 112
  expect "^browser: menu shown $(cased "Rename Space, Change Icon…, $colors")" \
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
tiles=47
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

# With the switch in Settings › Advanced off, the link goes in the pinned tab.
send click $page_point
expect "^browser: title $a pressed [0-9]*\$" "a click on the test page's lower part didn't reach it"
send key , cmd
expect '^browser: settings shown General$' "⌘, didn't show Settings on its General tab"
# Settings takes no click until it has drawn, nor a tab's rows until the tab has (#1101, #1133).
expect '^browser: settings drew General$' "Settings didn't draw General"
send clickkey $(settings_at "toolbar Advanced")
expect '^browser: settings tab Advanced$' "a click on Advanced didn't show its tab"
settings_drew Advanced
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

# Back on, a link rule to another Space sends the link there as a tab, as it sends one from another
# app, and the preview doesn't open (#692). Settings' Links tab adds the rule, as ./check.sh links
# does: + at 76 173, and the first rule's Space pop-up at 500 129.
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
expect '^browser: settings shown Advanced$' "⌘, didn't show Settings on its Advanced tab"
expect '^browser: settings drew Advanced$' "Settings didn't draw Advanced"
send clickkey $(settings_at "switch link previews")
expect '^browser: link previews on$' "a click on the link previews switch didn't turn them on"
send clickkey $(settings_at "toolbar Links")
expect '^browser: settings tab Links$' "a click on Links didn't show its tab"
settings_drew Links
expect '^browser: link rules drawn ' "the Links tab didn't draw its rules"
send clickkey 76 173
expect '^browser: link rule 0 field has the keyboard$' "+ didn't give the new rule's field the keyboard"
send type preview-target
post clickkey 500 129
expect '^browser: menu shown [^,]* (checked), work$' "the rule's Space pop-up didn't list the user's Spaces"
send choose work
expect '^browser: link rules drawn URL contains "preview-target" → work; ' \
  "the rule's pop-up didn't set its Space"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
send click $link_point
expect "^browser: link rule 0 sends $target to space $second\$" "the rule didn't send the pinned tab's link"
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
echo "ok: a link to another site in a pinned tab or a favorite opens in a preview over the window, which Esc, a click on the dim and ⌘W close, with its page, leaving the tab and the page behind as they were, ⇧⌘C copies the address of, and ⌘↩ keeps as the top Today tab, shown, without loading it again. ⌘Z reopens a closed one over its tab, and so does ⇧⌘T. A Google Meet link opens as a tab of the Space shown instead. With the Settings switch off, the link goes in the pinned tab, and back on, a link rule to another Space sends it there as a tab instead. Windows: $out/preview-open.png, $out/preview-light.png, $out/preview-dark.png, $out/preview-kept.png, $out/preview-reopened.png, $out/preview-meeting.png, $out/preview-ruled.png"
echo "  The first preview's first frame came $first_frame after the click reached the UI."
[ -z "$recording" ] || echo "  Its motion, opening and kept: $recording"
