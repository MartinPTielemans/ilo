# The pictures of ilo for its Microsoft Store listing and the README (#454), from the README shot's
# Space and tabs: a Space "ilo" in the default colour, ilobrowser.com, Wikipedia's Web browser and
# ilo's GitHub repository pinned, and Wikipedia's Chromium and ilobrowser.com/ilo in Today, on
# public pages with a new profile and no accounts. The window fills a 1920 by 1080 screen. It
# captures shots-1-page.png (ilobrowser.com/ilo), shots-2-command-bar.png (the bar's suggestions
# for "wiki"), shots-3-pinned.png (the pinned Wikipedia page) and shots-4-settings.png (Settings
# over the window). It needs the network, so it runs only when named, and checks nothing beyond
# getting there: it says the size of each picture.

log="$out/shots.log"
step=0
rm -f "$saved" "$launch_frame"
BROWSER_DRIVE=1 BROWSER_URL='data:text/html,<title>start</title>' "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
send place 0 0 1920 1080

# Waits up to 30 s for tab $1 to load a URL starting with $2: these are pages on the internet.
loaded_in() {
  for _ in $(seq 300); do
    grep -q "^browser: loaded $1 $2" "$log" && return 0
    kill -0 "$pid" 2> /dev/null || gone
    sleep 0.1
  done
  fail "tab $1 didn't load $2"
}
# Opens $1 from ⌘T's command bar in a tab at the top of Today, and sets tab to it.
open_tab() {
  before=$(today)
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send type "$1"
  send key return
  expect "^browser: today [0-9][0-9]*${before:+ $before}\$" "the tab for $1 didn't open at the top of Today"
  tab=$(today | cut -d' ' -f1)
  # Chromium may show the parentheses in a Wikipedia address percent-encoded.
  loaded_in "$tab" "https://${1%%(*}"
}
# Pins the tab shown, tab, with ⌘D.
pin() {
  send key d cmd
  expect "^browser: pinned\\(.* \\)\\{0,1\\}$tab\\( .*\\)\\{0,1\\}\$" "⌘D didn't pin tab $tab"
}
# A picture of the window, once the page's entrance motion has played: the wait is for the
# picture, not for anything checked.
shot() {
  sleep 2
  ../tools/window-screenshot.sh "$pid" "$out/shots-$1.png" all || fail "no window to capture"
  # A PNG's width and height are the big-endian words at bytes 16 and 20.
  size=$(od -An -tu1 -j16 -N8 "$out/shots-$1.png" |
    awk '{ printf "%dx%d", (($1 * 256 + $2) * 256 + $3) * 256 + $4, (($5 * 256 + $6) * 256 + $7) * 256 + $8 }')
  sizes="$sizes shots-$1.png $size"
}

first=$(space)
send menu New Space…
expect '^browser: command new-space via ui$' "New Space in the menu didn't run new-space"
expect "^browser: spaces $first [0-9]*\$" "New Space didn't make a second Space"
second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
send type ilo
send key return
expect "^browser: create space $second ilo\$" "return didn't name the Space ilo"

open_tab ilobrowser.com
pin
open_tab en.wikipedia.org/wiki/Web_browser
wiki=$tab
pin
open_tab github.com/MartinPTielemans/ilo
pin
open_tab "en.wikipedia.org/wiki/Chromium_(web_browser)"
open_tab ilobrowser.com/ilo

sizes=
shot 1-page
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type wiki
expect '^browser: command bar suggestions "wiki"' "the command bar didn't suggest anything for wiki"
shot 2-command-bar
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
send key 2 cmd
expect "^browser: selected $wiki in " "⌘2 didn't show the pinned Wikipedia tab"
shot 3-pinned
send key , cmd
expect '^browser: settings shown General$' "⌘, didn't show Settings"
shot 4-settings
send key escape
exec 3>&-
quit
echo "ok: the Store's pictures:$sizes, in $out"
