# Add Split View, from the menu, ⌃+ from the page, ⌥-click on "+ New Tab" and the tab menu, narrows
# the page at once for a pane on its right, with the command bar over it, which takes typing and
# escape takes back. ⌥-click on a link, and the link menu's Open Link in Split View, open it in a
# pane on the right; ⌥⌘-click opens it in Little ilo (check/little.sh).
# The link menu is Arc's, and its Open Link in New Window and in Incognito Window open the link in a
# window as ⌘N's and in a private one as ⇧⌘N's. A link dragged onto the page's right target opens in
# a pane there. A URL typed in the command bar opens in the pane, focused. The panes share the card
# 10 apart, the focused one ringed, a click focuses a pane and gives its page the keyboard, ⌘L edits
# that pane's URL, and a toast shows at its corner. The split is one row in the sidebar, which ⌘1–⌘9
# count once and show with the pane focused last, and a click on a segment focuses its pane. The gap
# between the panes drags their widths, or stacked their heights, down to Chrome's least and
# snapping to even, and a double-click on it evens them.
# Expand Current Split gives the focused pane the card and the other 220 wide through a resize, and
# again does nothing. Change Orientation stacks the panes, with the find bar at the focused one's
# corner. Launched again, the app brings the split back stacked with both pages at the heights they
# were dragged to, and Change Orientation puts them side by side again, as each pane's split
# controls do. Separate Page from Split View shows the page alone, ⌥↩ in the command bar opens a URL
# beside the page, and ⌘W closes the focused pane, leaving the page beside it alone, which ⇧⌘T puts
# back. The ✕ on a split's row closes all its panes, and the window shows the tab above them. ⇧⌘T
# brings them back together. A row dragged onto the page brings a target to each of the card's
# sides, which go when it's back over the sidebar, and let go on the left one, the tab joins the
# page shown on its left. The command bar's Flip Split View, Move Pane Right and Separate All Pages
# reorder and end a split. A tab rested 0.4 s on another's row joins it, which ⌘Z takes back to its
# row and ⇧⌘Z joins again, ⌥-click on a split's row takes its tab out and on another tab's joins it
# to the page shown, and a split's menu has Mute under each tab and a Duplicate that copies the
# whole split. A row let go with ⌥ held leaves a copy of its tab there, which ⌘Z closes, and
# ⌥-click on the shown tab's own row splits it with a copy.

# Split view, from the served page with example.org beside it.
serve
log="$out/split.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL="$site" "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* ready$' || fail "the split view test page didn't draw"
a=$(today)
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  kill "$server"
  echo "skip: split view. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
# The Space's splits the app last showed, each as its tabs joined by "+".
splits() {
  grep '^browser: splits' "$log" | tail -1 | sed 's/^browser: splits *//'
}
# Sets $pages to the frames of the pages shown, left to right, then the focused one's ring. Not in
# a subshell, where send's count of the commands sent would be lost.
read_pages() {
  send layout
  pages=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
    grep -o '\(WebContentsViewCocoa\|BrowserCoreFocusRing\) [0-9,]* [0-9x]*' | tr '\n' ' ' |
    sed 's/ $//')
}
# Sets $toast to the left edge of the toast's window, from the content's, and $keyboard to the frame
# of the page that has the keyboard, as read_pages has them, or none.
read_toast() {
  send layout
  toast=$(sed -n "/^browser: drive $step: /,\$p" "$log" |
    grep -m1 '^browser: layout child .* 420x114$' |
    sed 's/^browser: layout child \([0-9-]*\),.*/\1/')
}
read_keyboard() {
  send layout
  keyboard=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout keyboard' |
    sed 's/^browser: layout keyboard //')
}
# Two panes share the page's frame, 10 apart, as Arc's do.
read_pages
whole=$pages
frame=${whole#WebContentsViewCocoa }
x=${frame%%,*}
y=${frame#*,}
y=${y%% *}
w=${frame#* }
h=${w#*x}
w=${w%x*}
half=$(((w + 10) / 2))
left="$x,$y $((half - 10))x$h"
right="$((x + half)),$y $((w - half))x$h"
# Add Split View narrows the page at once to make room on the right, where the command bar opens,
# and escape gives the page its frame back.
send menu Add Split View
expect '^browser: command add-split via ui$' "Add Split View didn't run"
expect '^browser: command bar shown$' "Add Split View didn't open the command bar"
read_pages
[ "$pages" = "WebContentsViewCocoa $left" ] ||
  fail "while a pane is added, the page isn't at $left but at $pages"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
read_pages
[ "$pages" = "$whole" ] || fail "escape didn't give the page its frame back: $pages"
# From the command bar, which moves over the pane without going, keeping the keyboard, so escape
# still closes it.
bar_command add-split "Add Split View" "add split"
expect '^browser: command bar at ' "Add Split View from the command bar didn't open the bar there"
# Where the bar goes over the pane: left, top, width and height, as the host logs them.
over_pane=$(grep '^browser: command bar at ' "$log" | tail -1 | cut -d' ' -f5-)
read_pages
[ "$pages" = "WebContentsViewCocoa $left" ] ||
  fail "Add Split View from the command bar didn't narrow the page to $left but to $pages"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
# ⌥-click on "+ New Tab" adds one too, as Dia's does.
send click 100 154 alt
expect '^browser: command add-split via sidebar$' \
  "⌥-click on \"+ New Tab\" didn't run Add Split View"
expect '^browser: command bar shown$' "⌥-click on \"+ New Tab\" didn't open the command bar"
read_pages
[ "$pages" = "WebContentsViewCocoa $left" ] ||
  fail "⌥-click on \"+ New Tab\" didn't narrow the page to $left but to $pages"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
send click $page_point
# So does the tab menu's, from the tab's row with the page focused: the command bar opens over the
# pane where the bar's own Add Split View put it, takes typing, and escape closes it, giving the
# page its frame back.
post rightclick $(row 0)
expect "^browser: menu shown .*$(cased 'Add Split View')" \
  "a right-click on the page's row didn't show Add Split View: $(grep '^browser: menu shown' "$log" | tail -1)"
send choose "$(cased 'Add Split View')"
expect "^browser: menu chose $(cased 'Add Split View')\$" "the tab menu didn't choose Add Split View"
expect '^browser: command bar shown$' "the tab menu's Add Split View didn't open the command bar"
read_pages
[ "$pages" = "WebContentsViewCocoa $left" ] ||
  fail "the tab menu's Add Split View didn't narrow the page to $left but to $pages"
bar=$(grep '^browser: command bar at ' "$log" | tail -1 | cut -d' ' -f5-)
[ "$bar" = "$over_pane" ] ||
  fail "the tab menu's command bar opened at $bar, not over the pane at $over_pane as the bar's own does"
send type exa
# With example.org in the history, from a run before this one in the same check, it completes in
# place as typed.
expect '^browser: command bar drew "exa[^"]*"$' "the tab menu's command bar didn't take typing"
../tools/window-screenshot.sh "$pid" "$out/split-tab-menu.png" || fail "no window to capture"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the tab menu's command bar"
read_pages
[ "$pages" = "$whole" ] || fail "escape after the tab menu's Add Split View left the page at $pages"
send click $page_point
# ⌥-click on a link opens it in a pane on the right, focused, as Arc's does, not Chrome's download.
send click $((x + 20)) $((y + 20)) alt
expect "^browser: splits $a+[0-9][0-9]*\$" \
  "a click on a link with ⌥ didn't open it beside the page: \"$(splits)\""
n=$(splits | cut -d+ -f2)
expect "^browser: selected $n\( in .*\)\{0,1\}\$" "the link's pane isn't focused"
expect "^browser: panes $a $n\$" "the window doesn't show the link beside the page"
# Before read_keyboard's layout, which expect would read past: the page loads within it.
expect "^browser: loaded $n ${site}notes.txt\$" "the link didn't load in its pane"
read_keyboard
[ "$keyboard" = "$right" ] ||
  fail "the link's pane didn't get the keyboard at once: ${keyboard:-none} has it"
send key w cmd
expect "^browser: tab $n closed\$" "⌘W didn't close the link's pane"
expect "^browser: panes $a\$" "⌘W on the link's pane didn't leave the page alone"
send click $page_point
# A link dragged over the page brings the targets, which the page doesn't see, and let go on the
# right one it opens in a pane there, as a tab let go there does. The link comes from another app,
# as the driver hands it to the page's view: a drag the page starts is AppKit's, which follows the
# real mouse, so the driver's events would end it at once. Both reach the same drag destination.
right_target="$((x + w - 100)) $((y + h / 2))"
send dragover $((x + 40)) $((y + 30)) url "${site}notes.txt"
send dragover $((x + 60)) $((y + 40))
# The first update brings them: AppKit's comes with the second dragover, after the one that
# enters, and Aura's with the first, as XDND's first position enters and updates at once.
await "^browser: drop targets for $a\$" || fail "a link dragged over the page didn't bring the targets"
send dragover $right_target
expect '^browser: drop target right$' "the right target isn't under the dragged link"
expect '^browser: drag over [A-Za-z]* taken$' "the page's view didn't take the link on the target"
../tools/window-screenshot.sh "$pid" "$out/split-link-targets.png" || fail "no window to capture"
send dragdrop $right_target
expect '^browser: link dropped right$' "the link let go on the right target didn't open"
expect '^browser: drag dropped$' "the page's view didn't take the link let go on the target"
expect "^browser: splits $a+[0-9][0-9]*\$" "the dropped link isn't beside the page: \"$(splits)\""
n=$(splits | cut -d+ -f2)
expect "^browser: loaded $n ${site}notes.txt\$" "the dropped link didn't load in its pane"
send key w cmd
expect "^browser: panes $a\$" "⌘W on the dropped link's pane didn't leave the page alone"
send click $page_point
# The link menu is Arc's: Open Link in New Tab, in New Window, in Split View and in Incognito
# Window, then Save Link As… and Copy Link Address, before the menu's other items. Off the Mac
# they're in sentence case, as Chrome's are there.
if windows || [ "$(uname)" = Linux ]; then
  in_window='Open link in new window' in_private='Open link in incognito window'
  in_split='Open link in split view'
  link_items="Open link in new tab, $in_window, $in_split, $in_private, Save link as…, Copy link address"
else
  in_window='Open Link in New Window' in_private='Open Link in Incognito Window'
  in_split='Open Link in Split View'
  link_items="Open Link in New Tab, $in_window, $in_split, $in_private, Save Link As…, Copy Link Address"
fi
link_menu() {
  post rightclick $((x + 20)) $((y + 20))
  expect "^browser: menu shown $link_items\(, .*\)\{0,1\}\$" \
    "a right-click on the page's link didn't show Arc's link items in order: \
$(grep '^browser: menu shown' "$log" | tail -1)"
}
# The tab that loaded the link last.
link_tab() {
  sed -n "s|^browser: loaded \([0-9]*\) ${site}notes.txt\$|\1|p" "$log" | tail -1
}
# Open Link in New Window opens it in a window as ⌘N's, showing it.
link_menu
send choose $in_window
expect '^browser: window opened [0-9]' "Open Link in New Window didn't open a window"
link_window=$(grep '^browser: window opened ' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: loaded [0-9]* ${site}notes.txt\$" "the link didn't load in its window"
n=$(link_tab)
expect "^browser: window $link_window shows $n\$" "the link's window didn't show it"
send window 2
send key w cmd
expect "^browser: tab $n closed\$" "⌘W didn't close the link's tab in its window"
send key w cmd shift
expect "^browser: window closed $link_window\$" "⇧⌘W didn't close the link's window"
send window 1
send click $page_point
# Open Link in Incognito Window opens it in a private window as ⇧⌘N's, whose session ends with it.
link_menu
send choose $in_private
expect '^browser: private window opened [0-9]* profile [0-9]*$' \
  "Open Link in Incognito Window didn't open a private window"
link_window=$(grep '^browser: private window opened ' "$log" | tail -1 | cut -d' ' -f5)
expect "^browser: loaded [0-9]* ${site}notes.txt\$" "the link didn't load in the private window"
n=$(link_tab)
expect "^browser: window $link_window shows $n\$" "the private window didn't show the link"
# The driver counts windows in the order they opened, closed ones too.
send window 3
send key w cmd shift
expect "^browser: window closed $link_window\$" "⇧⌘W didn't close the private window"
expect '^browser: private session ended [0-9]*$' "closing the private window didn't end its session"
send window 1
send click $page_point
# Open Link in Split View opens it beside the page, as ⌥-click does.
link_menu
send choose $in_split
expect "^browser: splits $a+[0-9][0-9]*\$" \
  "Open Link in Split View didn't open the link beside the page: \"$(splits)\""
n=$(splits | cut -d+ -f2)
expect "^browser: selected $n\( in .*\)\{0,1\}\$" "the link's pane isn't focused"
expect "^browser: loaded $n ${site}notes.txt\$" "the link didn't load in its pane"
send key w cmd
expect "^browser: panes $a\$" "⌘W on the link's pane didn't leave the page alone"
send click $page_point
# ⌃+ from the page adds one too, and a URL typed there opens in the new pane, focused.
send key + ctrl
expect '^browser: command add-split via page$' "⌃+ with the page focused didn't reach the UI"
expect '^browser: command bar shown$' "⌃+ didn't open the command bar"
send type example.org
send key return
expect "^browser: splits $a+[0-9][0-9]*\$" "example.org didn't open beside the page: \"$(splits)\""
b=$(splits | cut -d+ -f2)
expect "^browser: selected $b\$" "the new pane isn't focused"
expect "^browser: panes $a $b\$" "the window doesn't show both panes"
expect "^browser: loaded $b https://example.org/\$" "example.org didn't load in the new pane"
read_keyboard
[ "$keyboard" = "$right" ] ||
  fail "the new pane's page doesn't have the keyboard: ${keyboard:-none} has it"
two="WebContentsViewCocoa $left WebContentsViewCocoa $right"
read_pages
[ "$pages" = "$two BrowserCoreFocusRing $right" ] ||
  fail "the panes aren't side by side with the ring on the right one: $pages"
../tools/window-screenshot.sh "$pid" "$out/split.png" || fail "no window to capture"
# A click in a pane focuses it, its page taking the keyboard, and the ring goes with it.
send click $((x + 100)) 400
expect "^browser: selected $a\$" "a click in the left pane didn't focus it"
expect "^browser: panes $a $b\$" "focusing a pane changed the panes shown"
[ "$focus" = RenderWidgetHostViewCocoa ] || fail "the left pane's page doesn't have the keyboard"
read_keyboard
[ "$keyboard" = "$left" ] ||
  fail "the click gave ${keyboard:-nothing} the keyboard, not the left pane"
read_pages
[ "$pages" = "$two BrowserCoreFocusRing $left" ] ||
  fail "the ring didn't go to the left pane: $pages"
../tools/window-screenshot.sh "$pid" "$out/split-left.png" || fail "no window to capture"
# ⌘L edits the focused pane's URL, as Arc's address bar is the panel's, and the other pane keeps its
# page. Back puts the left pane's page back for the steps after.
send key l cmd
expect '^browser: command edit-url via page$' "⌘L with the left pane focused didn't reach the UI"
expect '^browser: command bar shown$' "⌘L didn't open the command bar"
send type "${site}notes.txt"
send key return
expect "^browser: loaded $a ${site}notes.txt\$" "the URL typed after ⌘L didn't load in the focused pane"
# The panes are logged when they change, so the last line still names both.
[ "$(grep '^browser: panes ' "$log" | tail -1)" = "browser: panes $a $b" ] ||
  fail "the URL typed after ⌘L changed the panes shown: $(grep '^browser: panes ' "$log" | tail -1)"
grep -q "^browser: loaded $b ${site}notes.txt\$" "$log" && fail "⌘L's URL loaded in the other pane"
send key left cmd
# A page Back restores from the back/forward cache logs no load, only its state.
expect "^browser: page $a back 0 forward 1 loading 0 $site\$" "Back didn't take the left pane back to its page"
# A toast shows over its pane, the focused one, as Arc's does: ⇧⌘C's toast sits at the right pane's
# corner while it's focused, and the next, once a click focuses the left one, at that one's, as far
# left as its right edge is.
send click $((x + half + 100)) 400
expect "^browser: selected $b\$" "a click in the right pane didn't focus it"
send key c cmd shift
expect '^browser: toast shown Copied Current URL$' "⇧⌘C in the right pane showed no toast"
expect '^browser: toast window shown$' "⇧⌘C's toast didn't come on screen"
read_toast
over_right=$toast
expect '^browser: toast hidden$' "⇧⌘C's toast stayed"
send click $((x + 100)) 400
expect "^browser: selected $a\$" "a click in the left pane didn't focus it again"
send key c cmd shift
expect '^browser: toast shown Copied Current URL$' "⇧⌘C in the left pane showed no toast"
expect '^browser: toast window shown$' "⇧⌘C's toast over the left pane didn't come on screen"
read_toast
over_left=$toast
[ -n "$over_right" ] && [ -n "$over_left" ] &&
  [ "$((over_right - over_left))" = "$((w - half + 10))" ] ||
  fail "the toast went from ${over_right:-nowhere} to ${over_left:-nowhere}, not to the left pane's corner"
../tools/window-screenshot.sh "$pid" "$out/split-toast.png" || fail "no window to capture"
# In the sidebar the split is one row, which ⌘2 shows under a tab opened above it, with the pane
# focused last, and a click on its right segment shows that pane focused.
open_from_bar example.net
c=$tab
expect "^browser: today $c $a $b\$" "the new tab isn't above the split: Today is \"$(today)\""
send key 2 cmd
expect "^browser: selected $a\( in .*\)\{0,1\}\$" \
  "⌘2 didn't show the split with its pane focused last"
expect "^browser: panes $a $b\$" "⌘2 didn't show both panes"
send click 170 $(row 1 | cut -d' ' -f2)
expect "^browser: selected $b\( in .*\)\{0,1\}\$" \
  "a click on the split's right segment didn't focus it"
read_keyboard
[ "$keyboard" = "$right" ] ||
  fail "the right segment's pane didn't get the keyboard: ${keyboard:-none} has it"
# The gap between the panes drags their widths, as Chrome's does, each kept a tenth of the card
# wide, at least 50 and at most 200, and the gap snaps to even within 15. A double-click on it
# evens them.
gap=$((x + half - 5))
least=$(((w + 5) / 10))
[ "$least" -lt 50 ] && least=50
[ "$least" -gt 200 ] && least=200
wider="WebContentsViewCocoa $x,$y $((half + 90))x$h"
narrower="$((x + half + 100)),$y $((w - half - 100))x$h"
send drag $gap 400 $((gap + 100)) 400
read_pages
[ "$pages" = "$wider WebContentsViewCocoa $narrower BrowserCoreFocusRing $narrower" ] ||
  fail "dragging the gap 100 right didn't widen the left pane by 100: $pages"
../tools/window-screenshot.sh "$pid" "$out/split-resized.png" || fail "no window to capture"
send drag $((gap + 100)) 400 $((x + w - 20)) 400
most="WebContentsViewCocoa $x,$y $((w - least - 10))x$h"
fewest="$((x + w - least)),$y ${least}x$h"
read_pages
[ "$pages" = "$most WebContentsViewCocoa $fewest BrowserCoreFocusRing $fewest" ] ||
  fail "dragged far right, the right pane isn't $least wide: $pages"
send drag $((x + w - least - 5)) 400 $((gap + 8)) 400
read_pages
[ "$pages" = "$two BrowserCoreFocusRing $right" ] ||
  fail "dragged to 8 from even, the gap didn't snap there: $pages"
send drag $gap 400 $((gap + 100)) 400
read_pages
[ "$pages" = "$wider WebContentsViewCocoa $narrower BrowserCoreFocusRing $narrower" ] ||
  fail "dragging the gap 100 right again didn't widen the left pane: $pages"
send doubleclick $((gap + 100)) 400
read_pages
[ "$pages" = "$two BrowserCoreFocusRing $right" ] ||
  fail "a double-click on the gap didn't even the panes: $pages"
# Expand Current Split gives the focused pane, the right one, the card in one frame and leaves the
# left 220 wide, also as the window is resized, and doing it again changes nothing. A double-click
# on the gap, now 220 in, evens them again.
expanded="WebContentsViewCocoa $x,$y 220x$h WebContentsViewCocoa $((x + 230)),$y $((w - 230))x$h"
expanded="$expanded BrowserCoreFocusRing $((x + 230)),$y $((w - 230))x$h"
send menu Expand Current Split
expect '^browser: command expand-split via ui$' "Expand Current Split didn't run from the menu"
read_pages
[ "$pages" = "$expanded" ] || fail "Expand Current Split didn't leave the left pane 220 wide: $pages"
../tools/window-screenshot.sh "$pid" "$out/split-expanded.png" || fail "no window to capture"
send menu Expand Current Split
read_pages
[ "$pages" = "$expanded" ] || fail "Expand Current Split again changed the panes: $pages"
# The left pane stays 220 wide as the window gets narrower and as it gets back to its size, the
# right one taking the rest.
send layout
size=$(grep '^browser: layout content' "$log" | tail -1 |
  sed 's/^browser: layout content \([0-9]*\)x\([0-9]*\),.*/\1 \2/')
send resize 1000 700
expect '^browser: drawn at 1000x700$' "the window didn't draw at its new size"
read_pages
set -- $pages
[ "${3%x*}" = 220 ] && [ "${5%%,*}" = "$((x + 230))" ] && [ "${6%x*}" -lt "$((w - 230))" ] ||
  fail "resized, the left pane isn't 220 wide, with the right one beside it: $pages"
send resize $size
expect "^browser: drawn at ${size% *}x${size#* }\$" "the window didn't draw back at its size"
read_pages
[ "$pages" = "$expanded" ] || fail "back at its size the panes aren't as they were: $pages"
send doubleclick $((x + 225)) 400
read_pages
[ "$pages" = "$two BrowserCoreFocusRing $right" ] ||
  fail "a double-click on the expanded split's gap didn't even the panes: $pages"
# Change Orientation stacks the panes top to bottom, 10 apart, each as tall as the other, the ring
# on the focused one, and the find bar goes to its top right corner. The driver's frames count
# from the bottom.
tall=$(((h + 10) / 2))
top="$x,$((y + h - tall + 10)) ${w}x$((tall - 10))"
bottom="$x,$y ${w}x$((h - tall))"
stacked="WebContentsViewCocoa $top WebContentsViewCocoa $bottom"
bar_command change-orientation "Change Orientation" "change orientation"
expect "^browser: splits $a/$b\$" "Change Orientation didn't stack the split: \"$(splits)\""
read_pages
[ "$pages" = "$stacked BrowserCoreFocusRing $bottom" ] ||
  fail "the panes aren't stacked with the ring on the bottom one: $pages"
send key f cmd
expect '^browser: find bar shown$' "⌘F in the bottom pane didn't show the find bar"
send layout
child=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout child' |
  sed 's/^browser: layout child //')
content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout content' |
  sed 's/^browser: layout content \([0-9]*x[0-9]*\).*/\1/')
height=${content#*x}
content=${content%x*}
[ "$child" = "$((content - 320)),$((20 + tall)) 300x32" ] ||
  fail "the find bar is at ${child:-nowhere}, not at the bottom pane's corner"
../tools/window-screenshot.sh "$pid" "$out/split-stacked.png" || fail "no window to capture"
send key escape
expect '^browser: find bar hidden$' "escape didn't close the find bar"
# Stacked, the gap drags their heights, the top one taller by 50 here, which the next launch keeps.
gap=$((height - (y + h - tall + 5)))
taller="$x,$((y + h - tall - 40)) ${w}x$((tall + 40))"
shorter="$x,$y ${w}x$((h - tall - 50))"
resized="WebContentsViewCocoa $taller WebContentsViewCocoa $shorter BrowserCoreFocusRing $shorter"
send drag 600 $gap 600 $((gap + 50))
read_pages
[ "$pages" = "$resized" ] ||
  fail "dragging the stacked gap 50 down didn't make the top pane taller: $pages"
exec 3>&-
quit

# Launched again, the split comes back stacked with both its pages, and the pane focused last
# focused. Change Orientation puts them side by side again.
log="$out/split-restored.log"
step=0
BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch didn't restore the split"
await '^browser: splits [0-9][0-9]*/[0-9][0-9]*$' || fail "the split didn't come back stacked"
a=$(splits | cut -d/ -f1)
b=$(splits | cut -d/ -f2)
# The tab above the split, whose ID the relaunch may have changed too.
c=$(today | cut -d' ' -f1)
await "^browser: selected $b\$" || fail "the relaunch didn't focus the pane focused last"
await "^browser: panes $a $b\$" || fail "the relaunch didn't show both panes"
await "^browser: loaded $a $site\$" || fail "the split's top page didn't load"
await "^browser: loaded $b https://example.org/\$" || fail "the split's bottom page didn't load"
read_pages
[ "$pages" = "$resized" ] ||
  fail "the relaunch didn't show the panes stacked at the heights they were dragged to: $pages"
send doubleclick 600 $((gap + 50))
read_pages
[ "$pages" = "$stacked BrowserCoreFocusRing $bottom" ] ||
  fail "a double-click on the stacked gap didn't even the panes: $pages"
bar_command change-orientation "Change Orientation" "change orientation"
expect "^browser: splits $a+$b\$" "Change Orientation didn't put the panes side by side again"
read_pages
[ "$pages" = "$two BrowserCoreFocusRing $right" ] ||
  fail "the panes aren't side by side again with the ring on the right one: $pages"
# Each pane's split controls, in the window's margin over it, show the split's menu, whose Change
# Orientation stacks the split and, from the top pane's, puts it side by side again.
controls=$(cased 'Change Orientation, Separate Page from Split View, Close this Split Pane')
post click $((x + (half - 10) / 2)) 5
expect "^browser: menu shown $controls\$" "a click on the left pane's split controls didn't show their menu"
send choose "$(cased 'Change Orientation')"
expect '^browser: command change-orientation via split controls$' \
  "the split controls' Change Orientation didn't run"
expect "^browser: splits $a/$b\$" "the split controls didn't stack the panes"
post click $((x + w / 2)) 5
expect "^browser: menu shown $controls\$" "a click on the top pane's split controls didn't show their menu"
send choose "$(cased 'Change Orientation')"
expect "^browser: splits $a+$b\$" "the top pane's split controls didn't put the panes side by side"
send click $((x + w - 100)) 400
# Separate Page from Split View shows the focused pane alone.
send menu Separate Page from Split View
expect '^browser: splits $' "Separate Page from Split View didn't end the split"
../tools/window-screenshot.sh "$pid" "$out/split-separated.png" || fail "no window to capture"
expect "^browser: panes $b\$" "the separated page isn't shown alone"
read_pages
[ "$pages" = "$whole" ] || fail "the separated page doesn't fill the card: $pages"
# ⌘Z puts the page back in the split beside the one it left, and ⇧⌘Z takes it out again.
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo Separate Page from Split View"
expect "^browser: splits $a+$b\$" "⌘Z didn't put the page back in its split: \"$(splits)\""
send key z cmd shift
expect '^browser: command redo via \(ui\|page\)$' "⇧⌘Z didn't redo Separate Page from Split View"
expect '^browser: splits $' "⇧⌘Z didn't end the split again"
expect "^browser: panes $b\$" "⇧⌘Z didn't show the separated page alone"
# ⌥↩ in the command bar opens its URL in a pane beside the page shown, and ⌘W closes the focused
# pane, leaving the card to the page beside it.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type example.net
send key return alt
expect "^browser: splits $b+[0-9][0-9]*\$" \
  "⌥↩ didn't open example.net beside the page: \"$(splits)\""
d=$(splits | cut -d+ -f2)
expect "^browser: panes $b $d\$" "⌥↩ didn't show the page and its new pane"
expect "^browser: loaded $d https://example.net/\$" "example.net didn't load in the new pane"
send key w cmd
expect "^browser: tab $d closed\$" "⌘W didn't close the focused pane"
expect "^browser: panes $b\$" "⌘W on a pane didn't leave the page beside it alone"
expect '^browser: splits $' "the split outlived its pane"
read_pages
[ "$pages" = "$whole" ] || fail "the page left doesn't fill the card: $pages"
# ⇧⌘T brings the closed pane back into its split, at its place, focused.
send key t cmd shift
expect "^browser: splits $b+[0-9][0-9]*\$" \
  "⇧⌘T didn't bring the pane back into its split: \"$(splits)\""
e=$(splits | cut -d+ -f2)
expect "^browser: selected $e\( in .*\)\{0,1\}\$" "the pane ⇧⌘T brought back isn't focused"
expect "^browser: panes $b $e\$" "the window doesn't show the pane ⇧⌘T brought back"
expect "^browser: loaded $e https://example.net/\$" "the pane ⇧⌘T brought back didn't load"
# Flip Split View swaps the panes, and Move Pane Right moves the focused one back to the right.
bar_command flip-split "Flip Split View" "flip split"
expect "^browser: splits $e+$b\$" "Flip Split View didn't swap the split's pages: \"$(splits)\""
expect "^browser: panes $e $b\$" "Flip Split View didn't swap the pages shown"
bar_command move-pane-right "Move Pane Right" "move pane right"
expect "^browser: splits $b+$e\$" "Move Pane Right didn't move the focused page: \"$(splits)\""
expect "^browser: panes $b $e\$" "Move Pane Right didn't move the page shown"
# The ✕ on a split's row closes all its panes, and with the left one focused the window shows the
# tab its Space picks after them.
send click $((x + 100)) 400
expect "^browser: selected $b\( in .*\)\{0,1\}\$" "a click on the left pane didn't focus it"
send move 202 $(row 2 | cut -d' ' -f2)
expect '^browser: hover tab [0-9][0-9]* close 1 favicon none$' "the split's row didn't draw its ✕"
send click 202 $(row 2 | cut -d' ' -f2)
expect '^browser: command close-tab via sidebar$' "a click on the split's ✕ didn't run Close Tab"
expect "^browser: tab $b closed\$" "the split's ✕ didn't close its left pane"
expect "^browser: tab $e closed\$" "the split's ✕ didn't close its right pane"
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "closing the split didn't show the tab above it"
# ⇧⌘T twice brings them back, the second into a split with the first, and the ✕ closes them again.
send key t cmd shift
send key t cmd shift
expect "^browser: splits [0-9][0-9]*+[0-9][0-9]*\$" \
  "⇧⌘T twice didn't bring the split's panes back together: \"$(splits)\""
send click $((x + 100)) 400
send move 202 $(row 2 | cut -d' ' -f2)
expect '^browser: hover tab [0-9][0-9]* close 1 favicon none$' "the split's row didn't draw its ✕"
send click 202 $(row 2 | cut -d' ' -f2)
expect '^browser: command close-tab via sidebar$' "a click on the split's ✕ didn't run Close Tab"
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "closing the split again didn't show $a"
# A row dragged onto the page becomes a picture of a window, and a target slides in at each of the
# card's sides. Back over the sidebar it's a row again, and let go over the left target it joins
# the page shown on its left, focused.
target="$((x + 140)) $((y + h / 2))"
send press $(row 0)
send dragto 110 $(row 0 | cut -d' ' -f2)
send dragto $target
none_since '^browser: detent$' || fail "a row dragged onto the page clicked the trackpad"
expect "^browser: drop targets for $a\$" "a row dragged onto the page didn't bring the targets"
expect '^browser: drop targets shown$' "the targets' window didn't show"
expect '^browser: drop target left$' "the left target isn't under the tab"
../tools/window-screenshot.sh "$pid" "$out/split-targets.png" || fail "no window to capture"
send dragto 100 300
expect '^browser: drop targets gone$' "the targets stayed with the tab back over the sidebar"
expect '^browser: drop targets hidden$' "the targets' window stayed"
send dragto $target
expect '^browser: drop target left$' "the left target isn't under the tab again"
send release $target
expect "^browser: splits $c+$a\$" "the tab let go on the left target didn't join the page: \"$(splits)\""
expect "^browser: selected $c\( in .*\)\{0,1\}\$" "the tab let go on a target isn't focused"
expect "^browser: panes $c $a\$" "the window doesn't show the tab let go left of the page"
# Separate All Pages ends the split, with its pages in their order, and the focused one alone.
bar_command separate-split "Separate All Pages" "separate all"
expect '^browser: splits $' "Separate All Pages didn't end the split: \"$(splits)\""
expect "^browser: panes $c\$" "Separate All Pages didn't show the focused page alone"
expect "^browser: today.* $c $a\( \|\$\)" "Separate All Pages lost the pages' order: $(today)"
# A page alone has targets at the card's top and bottom too, as Arc stacks a split, and let go on
# the top one the tab joins the page above it.
top="$((x + w / 2)) $((y + 80))"
send press $(row 0)
send dragto 110 $(row 0 | cut -d' ' -f2)
send dragto $top
expect '^browser: drop target top$' "the top target isn't under the tab"
../tools/window-screenshot.sh "$pid" "$out/split-targets-top.png" || fail "no window to capture"
send release $top
expect '^browser: splits [0-9][0-9]*/[0-9][0-9]*$' \
  "the tab let go on the top target didn't stack the split: \"$(splits)\""
# The sidebar row of tab $1, a split's tabs one row.
row_of() {
  n=0
  seen=
  for id in $(today); do
    key=$id
    for each in $(splits); do
      case "+$(echo "$each" | tr / +)+" in *"+$id+"*) key=$each ;; esac
    done
    case " $seen " in
      *" $key "*) [ "$id" = "$1" ] && echo $((n - 1)) && return ;;
      *)
        [ "$id" = "$1" ] && echo $n && return
        seen="$seen $key"
        n=$((n + 1))
        ;;
    esac
  done
}
# A tab dragged onto another's row and rested 0.4 s on its middle half joins it on its right, the
# drop line going (ours: Arc wasn't recorded doing it).
bar_command separate-split "Separate All Pages" "separate all"
expect '^browser: splits $' "Separate All Pages didn't end the stacked split: \"$(splits)\""
open_from_bar example.net
f=$tab
onto=$(today | cut -d' ' -f2)
dragged=$(today | cut -d' ' -f3)
rows=$(today)
send press $(row 2)
send dragto 110 $(row 2 | cut -d' ' -f2)
send dragto $(row 1)
expect "^browser: drop onto $onto\$" "a tab rested on another's row didn't take it as the drop"
send release $(row 1)
expect "^browser: dragged $dragged onto $onto\$" "the tab let go on another's row didn't join it"
expect "^browser: splits $onto+$dragged\$" "the rested tab isn't on the right of a split: \"$(splits)\""
# ⌘Z takes the tab back out to its row, the page shown still shown, and ⇧⌘Z joins it again.
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo the split"
expect '^browser: splits $' "⌘Z didn't take the rested tab out of the split: \"$(splits)\""
expect "^browser: today $rows\$" "⌘Z didn't put the rested tab back in its row: \"$(today)\""
send key z cmd shift
expect '^browser: command redo via \(ui\|page\)$' "⇧⌘Z didn't redo the split"
expect "^browser: splits $onto+$dragged\$" "⇧⌘Z didn't join the tab again: \"$(splits)\""
# ⌥-click on a split's row takes the tab under it out, and on another tab's row joins that tab to
# the page shown, on its right, as Arc's does.
send click $(row "$(row_of "$onto")") alt
expect '^browser: command split via sidebar$' "⌥-click on the split's row didn't reach the sidebar"
expect '^browser: splits $' "⌥-click on the split's row didn't take its tab out: \"$(splits)\""
send click $(row "$(row_of "$f")")
expect "^browser: selected $f\( in .*\)\{0,1\}\$" "a click on $f's row didn't show it"
send click $(row "$(row_of "$dragged")") alt
expect "^browser: splits $f+$dragged\$" "⌥-click on a tab's row didn't join it to the page shown"
expect "^browser: panes $f $dragged\$" "⌥-click on a tab's row didn't show it beside the page"
# A split's menu has Mute under each of its tabs' names, and Duplicate copies the whole split.
post rightclick $(row "$(row_of "$f")")
expect "^browser: menu shown .* › $(cased Mute)[^,]*, .* › $(cased Mute)[^,]*, .*$(cased Duplicate)" \
  "a split's menu doesn't have Mute under each tab and Duplicate: \
$(grep '^browser: menu shown' "$log" | tail -1)"
send choose "$(cased Duplicate)"
expect "^browser: splits \(.* \)\{0,1\}$f+$dragged\( .*\)\{0,1\}\$" "Duplicate lost the split"
expect '^browser: splits [0-9][0-9]*+[0-9][0-9]* [0-9][0-9]*+[0-9][0-9]*$' \
  "Duplicate on a split's menu didn't copy the whole split: \"$(splits)\""
# A row dragged with ⌥ held as it's let go stays, and a copy of its tab goes where it's let go,
# shown, which ⌘Z closes (#690; ours: Arc's wasn't recorded).
open_from_bar example.com
c=$tab
rows=$(today)
y=$(row 2 | cut -d' ' -f2)
send press $(row 0) alt
send dragto 110 $(row 0 | cut -d' ' -f2) alt
send dragto 100 $((y - 15)) alt
send release 100 $((y - 15)) alt
expect "^browser: dragged $c copy [0-9][0-9]* to today 0 [0-9][0-9]*\$" \
  "a row let go with ⌥ held didn't drop a copy: $(grep '^browser: dragged' "$log" | tail -1)"
copy=$(grep "^browser: dragged $c copy " "$log" | tail -1 | cut -d' ' -f5)
expect "^browser: today $c .*$copy" "the ⌥-dragged row didn't stay, or its copy isn't under it: \
\"$(today)\""
expect "^browser: selected $copy\( in .*\)\{0,1\}\$" "the ⌥-dragged tab's copy wasn't shown"
../tools/window-screenshot.sh "$pid" "$out/alt-drag-copy.png" || fail "no window to capture"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo the ⌥-drag"
expect "^browser: today $rows\$" "⌘Z didn't close the ⌥-dragged tab's copy: \"$(today)\""
# ⌥-click on the row of the page shown, alone, splits it with a copy of itself on its right, as
# Arc's does (#694).
expect '^browser: rows settled$' "Today's rows didn't settle after ⌘Z"
send click $(row 0)
expect "^browser: selected $c\( in .*\)\{0,1\}\$" "a click on $c's row didn't show it"
send click $(row 0) alt
expect '^browser: command split via sidebar$' \
  "⌥-click on the shown tab's row didn't reach the sidebar"
expect "^browser: splits \(.* \)\{0,1\}$c+[0-9][0-9]*\( .*\)\{0,1\}\$" \
  "⌥-click on the shown tab's row didn't split it with a copy: \"$(splits)\""
twin=$(splits | tr ' ' '\n' | grep "^$c+" | cut -d+ -f2)
case " $rows " in *" $twin "*) fail "⌥-click split $c with $twin, not a copy" ;; esac
expect "^browser: panes $c $twin\$" "⌥-click didn't show the copy beside the page"
../tools/window-screenshot.sh "$pid" "$out/alt-click-copy.png" || fail "no window to capture"
exec 3>&-
quit
echo "ok: Add Split View narrows the page at once for a pane on the right, with the command bar over it, which escape takes back, and ⌃+ from the page, ⌥-click on \"+ New Tab\" and the tab menu's, its bar taking typing, do it too. ⌥-click on a link and the link menu's Open Link in Split View open it in a pane on the right. The link menu is Arc's, and its Open Link in New Window and in Incognito Window open the link in a window and a private window. A link dragged onto the page's right target opens in a pane there. A URL there opens in the pane, focused. The panes share the card 10 apart, the focused one ringed, a click focuses a pane and gives its page the keyboard, ⌘L edits that pane's URL, and a toast shows at that pane's corner. The split is one row in the sidebar, which ⌘1–⌘9 count once and show with the pane focused last, and a click on a segment focuses that pane. The gap between the panes drags their widths, or stacked their heights, down to Chrome's least and snapping to even, and a double-click evens them. Expand Current Split gives the focused pane the card and the other 220 wide, also as the window is resized, and again changes nothing. Change Orientation stacks the panes, with the find bar at the focused one's corner, and the split comes back stacked after a quit with both pages at the heights they were dragged to, until Change Orientation puts them side by side again. Separate Page from Split View shows the page alone, ⌥↩ in the command bar opens a URL beside the page, ⌘W closes the focused pane, which ⇧⌘T puts back in its split, and the ✕ on a split's row closes all its panes, which ⇧⌘T brings back together. A row dragged onto the page brings a target to each of the card's sides, which go when it's back over the sidebar, and let go on the left one the tab joins the page on its left. Flip Split View and Move Pane Right in the command bar reorder a split's pages, and Separate All Pages ends it. A page alone has a target at the card's top too, where the tab joins it in a stacked split. A tab rested on another's row joins it in a split, which ⌘Z takes back to its row and ⇧⌘Z joins again, ⌥-click on a split's row takes its tab out and on another tab's row joins it to the page shown, each pane's split controls change the split's orientation, and a split's menu mutes each of its tabs and duplicates the whole split. A row let go with ⌥ held stays, and a copy of its tab goes where it was let go, which ⌘Z closes, and ⌥-click on the shown tab's own row splits it with a copy of itself. ⌘Z after Separate Page from Split View puts the page back in its split, and ⇧⌘Z takes it out again. Windows: $out/split-tab-menu.png, $out/split.png, $out/split-left.png, $out/split-toast.png, $out/split-resized.png, $out/split-expanded.png, $out/split-stacked.png, $out/split-targets.png, $out/split-separated.png, $out/split-targets-top.png, $out/split-link-targets.png, $out/alt-drag-copy.png, $out/alt-click-copy.png"
