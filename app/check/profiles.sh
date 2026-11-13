# check.sh runs this at 90.
# The New Space form's Profile row makes a profile, asks for its name, and puts the Space in it, and its Choose a Theme
# row colours the Space. The Space's tabs open in the new profile, without the first profile's
# cookies, from a command bar opened in the first profile's Space too, which follows the Space shown. While it's empty the Space's menu moves it between profiles, and with a tab it keeps its
# own. The first Space keeps its cookie and its favorite, which shows only in the Spaces of its
# profile, and a new Space made from a Space is in that Space's profile. After a relaunch each Space
# is in its profile still. A tab moved to the other profile's Space loads there with that profile's
# cookie, after a warning the first time. Settings › Profiles renames a profile, which the Space
# menu then shows, and makes and deletes one. Each profile there has its own search engine, which
# the command bar searches with in its Spaces, its own archive time, its own download folder, which
# its Space's downloads go in, and Clear… for its browsing data, as ⇧⌘⌫ opens it for the Space
# shown. Little ilo's bar picks the other profile for its page, which is kept for the site, and a
# link from another app to the site opens in Little ilo in that profile. Each profile picks the
# service New Note opens a new document in, under Settings › Features, and ⌃⇧N, the command bar's
# New Note, the footer's + and New Note Split open one in the Space's profile's service.
# Deleting the Space leaves its profile with only a favorite, and Delete in the menu deletes it,
# favorite and all, once the user confirms.

# The cookie page's title says whether it finds its cookie, p, never a value, since the app logs
# every title, and "c" sets it. The cookie outlives the run, so its value is this run's start and an
# earlier run's doesn't count. It's on localhost, as clear.sh's cookie is on 127.0.0.1. A key
# sent before the page first paints can miss it, so the page draws text and says when it has
# drawn. Its title also shows each click and key it gets, to show where one went.
serve
printf '%s' '<title>loading</title>
<p>Cookie page</p>
<a id=d download="profile.txt" href="profile.html"></a>
<script>
const mine = "p='"$(date +%s)"'";
const seen = "cookie " + (document.cookie.split("; ").includes(mine) ? "p" : "none");
document.title = seen;
requestAnimationFrame(() => setTimeout(() => document.title = seen + ", drawn"));
onmousedown = () => document.title = "mousedown";
onkeydown = e => {
  document.title = "keydown " + e.key;
  if (e.key == "c") {
    document.cookie = mine + "; max-age=3600";
    document.title = "cookie set";
  }
  if (e.key == "d") d.click();
};
</script>' > "$out/site/profile.html"
cookie_page="$(echo "$site" | sed 's/127\.0\.0\.1/localhost/')profile.html"
log="$out/profiles.log"
step=0
# Links from other apps go to Little ilo, the rules' default, whatever a links run left.
rm -f "$saved" "$launch_frame" "$(dirname "$saved")/Little ilo profiles" "$(dirname "$saved")/Link rules"
BROWSER_DRIVE=1 BROWSER_URL="$cookie_page" "$browser" \
  < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
first=$(space)
a=$(today)
await "^browser: title $a cookie none\$" ||
  fail "the cookie page didn't load, or found an earlier run's cookie"
await "^browser: title $a cookie none, drawn\$" || fail "the cookie page didn't draw"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: profiles. Browser wasn't the active app, so the page ignored the click."
  exit 0
fi
default=$(sed -n "s/^browser: profiled space $first \([0-9]*\)\$/\1/p" "$log" | tail -1)
grep -q "^browser: added profile $default Default\$" "$log" ||
  fail "the first Space isn't in the profile named Default"
send key c
expect "^browser: title $a cookie set\$" "c in the cookie page didn't set a cookie"
send key r cmd
expect "^browser: title $a cookie p\$" "the cookie page lost its cookie on ⌘R"
# Dragged onto the Space's name, the tab becomes the first profile's favorite, which leaves the
# first Space empty.
send press $(row 0)
send dragto 100 160
send dragto 114 112
expect '^browser: drag shape tile$' "the row dragged onto the Space's name didn't turn into a tile"
send release 114 112
expect "^browser: favorites $a\$" "the row dragged onto the Space's name didn't make a favorite"
# In the empty Space, the command bar's New Profile asks for the new profile's name in the dialog,
# as the Space menu's does, and Cancel makes none.
bar_command change-profile "New Profile" "new profile"
expect '^browser: dialog shown New Profile$' "the command bar's New Profile didn't ask for its name"
../tools/window-screenshot.sh "$pid" "$out/bar-new-profile.png" || fail "no window to capture"
send key escape
expect '^browser: dialog cancelled$' "escape didn't cancel the new profile's dialog"
expect '^browser: dialog hidden$' "the new profile's dialog didn't go"
none_since 'browser: create profile' || fail "cancelling the dialog made a profile"

# A new Space is in the profile of the Space it's made from. Its form's Profile row puts it in a
# new profile, whose name Chrome's numbering gives, and its favorites aren't the first profile's.
send menu New Space…
expect "^browser: spaces $first [0-9]*\$" "New Space didn't make a second Space"
second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
expect "^browser: space $second in " "the window didn't show the new Space"
expect "^browser: profiled space $second $default\$" \
  "the new Space isn't in the profile of the Space it was made from"
expect "^browser: settled space $second\$" "the new Space's page didn't settle"
# The form's Profile and Choose a Theme rows, under its name field.
post click 100 255
expect "^browser: menu shown $(cased 'Default (checked), New Profile')\$" \
  "the form's Profile row didn't show the profiles"
../tools/window-screenshot.sh "$pid" "$out/profile-popup.png" || fail "no window to capture"
send choose "$(cased 'New Profile')"
expect '^browser: create profile [0-9]* Profile 2$' "New Profile didn't make a profile"
work=$(sed -n 's/^browser: create profile \([0-9]*\) .*/\1/p' "$log" | tail -1)
expect "^browser: added profile $work Profile 2\$" "the new profile wasn't listed"
expect "^browser: profiled space $second $work\$" "the form's Space didn't go into the new profile"
expect '^browser: favorites $' "the new profile's Space showed the first profile's favorite"
# The row asks for the profile's name at once, and escape keeps the numbered one.
expect "^browser: rename profile $work\$" "the new profile's name didn't take the keyboard"
../tools/window-screenshot.sh "$pid" "$out/profile-name.png" || fail "no window to capture"
send type personal
send key escape
expect '^browser: rename cancelled$' "escape didn't leave the new profile's name"
none_since '^browser: name profile' || fail "escape named the new profile"
send click 100 299
expect "^browser: theme picker for $second\$" "the form's Choose a Theme row didn't open the picker"
expect '^browser: theme picker shown$' "the form's Choose a Theme row didn't show the picker"
expect '^browser: theme picker drawn$' "the theme picker didn't draw itself on screen"
send clickkey "$(swatch_x 6)" $swatch_y
expect "^browser: colored space $second [0-9a-f]*\$" "the green swatch didn't colour the Space"
../tools/window-screenshot.sh "$pid" "$out/profile-form.png" || fail "no window to capture"
send key escape
expect '^browser: theme picker hidden$' "escape didn't close the theme picker"
send type work
send key return
expect "^browser: create space $second work\$" "return didn't name the Space"
expect '^browser: space form gone$' "the form didn't leave after return"
../tools/window-screenshot.sh "$pid" "$out/profile-made.png" || fail "no window to capture"
# Empty, its menu takes it back to the first profile, whose favorite shows again, and to the new
# one again. The Space's name is under the URL field, and 47 lower under a row of tiles.
name_point="100 112"
tiled_name="100 159"
post rightclick $name_point
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $themed, Set Profile › Default, \
Set Profile › Profile 2 (checked), Set Profile › New Profile, New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "the empty Space's menu didn't offer its profiles"
../tools/window-screenshot.sh "$pid" "$out/profile-menu.png" || fail "no window to capture"
send choose Default
expect "^browser: profiled space $second $default\$" "Default didn't take the Space back"
expect "^browser: favorites $a\$" "the first profile's favorite didn't show in its Space again"
post rightclick $tiled_name
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $themed, Set Profile › Default (checked), \
Set Profile › Profile 2, Set Profile › New Profile, Set Profile › Delete “Profile 2”, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "the menu didn't offer the unused profile back"
send choose Profile 2
expect "^browser: profiled space $second $work\$" "Profile 2 didn't take the Space again"
expect '^browser: favorites $' "the first profile's favorite stayed in the other profile's Space"
# The bar opened in the first profile's Space follows another Space shown while it's open, as
# Arc's does: ⌃2 shows this one, the bar asks this profile again for what's typed, and ↩ opens the
# page here, in this profile, never in the first one, whose cookie the page would find.
send key 1 ctrl
expect "^browser: space $first in " "⌃1 didn't show the first profile's Space"
send key t cmd
expect '^browser: command bar shown$' "⌘T in the first profile's Space didn't open the command bar"
send type "$cookie_page"
expect "^browser: command bar suggestions \"$cookie_page" "the command bar didn't suggest for the cookie page"
send key 2 ctrl
expect '^browser: command bar follows the Space$' "⌃2 with the command bar open didn't give it the Space"
expect "^browser: space $second in " "⌃2 with the command bar open didn't show the new profile's Space"
expect "^browser: command bar suggestions \"$cookie_page" \
  "the command bar didn't ask the new profile again for what was typed"
none_since '^browser: command bar hidden$' || fail "⌃2 closed the command bar"
send key return
expect '^browser: today [0-9][0-9]*$' "the cookie page didn't open in the new profile's Space"
b=$(today)
expect "^browser: title $b cookie none\$" \
  "the new profile's tab saw the first profile's cookie: $(grep "^browser: title $b" "$log" |
  tail -1)"
../tools/window-screenshot.sh "$pid" "$out/profile-space.png" || fail "no window to capture"
# Held partway back, the first profile's Space comes in beside this one's whole: its URL field
# with the tab it would show, its favorite and its rows at its offset, as this one's go out. X11
# sends a touchpad's scrolling with no start or end, so two fingers never page there (#554).
if [ "$(uname)" = Linux ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  echo "skip: two fingers held partway between two profiles' Spaces: X11 sends no touch phases (#554)."
else
  post swipe 110 500 10 0 10 1500
  await '^browser: swipe across$' || fail "two fingers across the sidebar didn't page"
  ../tools/window-screenshot.sh "$pid" "$out/profile-swipe.png" || fail "no window to capture"
  await "^browser: drive $step done" || fail "the driver didn't finish the swipe"
  none_since '^browser: space [0-9]' || fail "a swipe let go short of halfway showed another Space"
  expect "^browser: settled space $second\$" "the Space's page didn't settle back"
fi
# Its menu, with a tab, can't change its profile.
post rightclick $name_point
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $themed, Set Profile › Default (disabled), \
Set Profile › Profile 2 (checked), Set Profile › New Profile (disabled), \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space")\$" \
  "the menu of a Space with a tab offered another profile"
own=$((step + 1))
send choose Profile 2
# A Space made from this one is in its profile. Cancel takes it away again.
send menu New Space…
expect "^browser: spaces $first $second [0-9]*\$" "New Space didn't make a third Space"
none_since "^browser: profiled space $second " "$own" || fail "choosing its own profile changed the Space's"
third=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f5)
expect "^browser: profiled space $third $work\$" \
  "a Space made from the new profile's Space isn't in it"
expect "^browser: settled space $third\$" "the third Space's page didn't settle"
send layout
content=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  sed 's/^browser: layout content \([0-9]*x[0-9]*\),.*/\1/')
send click 114 $((${content#*x} - 74))
expect "^browser: cancel space $third\$" "a click on Cancel didn't cancel"
expect "^browser: spaces $first $second\$" "Cancel didn't delete the third Space"
# The first profile's Space has its favorite and its cookie still.
send key 1 ctrl
expect "^browser: space $first in " "⌃1 didn't show the first Space"
expect "^browser: favorites $a\$" "the first Space didn't show its profile's favorite"
expect "^browser: settled space $first\$" "the first Space's page didn't settle"
send click $page_point
send key r cmd
expect "^browser: title $a cookie p\$" "the first profile's cookie went"
# Little ilo's bar, with two profiles, has a profile picker, whose other profile opens the page in a
# Little ilo window of its own, without the first profile's cookie, and is kept for the site.
send key n cmd alt
expect "^browser: little window opened [0-9]* profile $default home $first\$" \
  "⌥⌘N didn't open a Little ilo window in the first profile"
little=$(grep '^browser: little window opened ' "$log" | tail -1 | cut -d' ' -f5)
# A new window goes in front of the others, so it logs no "key window"; the page ⌘L loads below
# being the one this window reopens shows it's key.
send key l cmd
expect '^browser: command bar shown$' "⌘L didn't open the Little ilo window's command bar"
send insert "$cookie_page"
send key return
expect "^browser: loaded [0-9]* $cookie_page\$" "the cookie page didn't load in the Little ilo window"
# The picker comes before Copy Link and Open in <Space> ⌘O | ⌄, whose words set where it is, so
# the bar logs its place, in the window's own points as Settings' are.
for _ in $(seq 100); do
  picker=$(sed -n 's/^browser: little profile picker at //p' "$log" | tail -1)
  [ -n "$picker" ] && break
  sleep 0.1
done
[ -n "$picker" ] || fail "Little ilo's bar didn't draw its profile picker"
post clickkey $picker
expect "^browser: menu shown $(cased 'Default (checked), Profile 2')\$" \
  "Little ilo's profile picker didn't list the profiles, the page's checked"
../tools/window-screenshot.sh "$pid" "$out/little-profile.png" all || fail "no window to capture"
send choose Profile 2
expect "^browser: little profile $work via bar\$" "picking Profile 2 didn't reach the app"
expect "^browser: little window $little reopened $cookie_page in profile $work as [0-9]*\$" \
  "picking Profile 2 didn't reopen the page in it"
moved=$(sed -n "s|^browser: little window $little reopened .* as \([0-9]*\)\$|\1|p" "$log" | tail -1)
expect "^browser: window closed $little\$" "the first profile's Little ilo window stayed open"
expect "^browser: little window opened $moved profile $work home $second\$" \
  "the new Little ilo window isn't in Profile 2, for its Space"
expect "^browser: title [0-9]* cookie none\$" "the page in Profile 2 saw the first profile's cookie"
grep -q "^localhost	Profile 2\$" "$(dirname "$saved")/Little ilo profiles" ||
  fail "Profile 2 wasn't kept for the site: $(cat "$(dirname "$saved")/Little ilo profiles" 2>&1)"
# New too, so ⌘W closing it shows it's key.
send key w cmd
expect "^browser: window closed $moved\$" "the Profile 2 Little ilo window didn't come to the front: ⌘W didn't close it"
# A link to the site from another app, as one clicked in Mail, opens in Little ilo in the profile
# kept for the site, Profile 2, in its Space, though the window in front shows the first's (#693).
# It's sent as the system sends one: on Windows by the command ilo.iss registers, on Linux by a
# second launch, which hands it to the browser running, and on the Mac by open.
send window 1
linked="${cookie_page}?from-another-app"
if [ "$(uname)" = Darwin ]; then
  open -a "$app" "$linked"
elif windows; then
  ("$browser" --single-argument "$linked" < /dev/null)
else
  "$browser" "$linked"
fi >> "$out/profiles-link.log" 2>&1 || fail "the link from another app didn't reach the browser"
expect_late "^browser: little ilo opens $linked in profile $work\$" \
  "the link from another app to the kept site didn't open in Profile 2: $(grep '^browser: link from another app' "$log" | tail -1)"
expect "^browser: link from another app $linked: tab [0-9]* in little window [0-9]* of space $second\$" \
  "the link from another app to the kept site didn't open in Profile 2's Space"
linked_window=$(sed -n "s|^browser: link from another app $linked: tab [0-9]* in little window \([0-9]*\) .*|\1|p" "$log" | tail -1)
expect "^browser: little window opened $linked_window profile $work home $second\$" \
  "the link's Little ilo window isn't in Profile 2"
expect "^browser: title [0-9]* cookie none\$" "the link's page in Profile 2 saw the first profile's cookie"
../tools/window-screenshot.sh "$pid" "$out/little-kept-profile.png" all || fail "no window to capture"
send window "$(grep -c '^browser: window opened ' "$log")"
send key w cmd
expect "^browser: window closed $linked_window\$" "⌘W didn't close the link's Little ilo window"
send window 1
exec 3>&-
quit

# Launched again, each Space is in its profile, and each tab loads in its own.
log="$out/profiles-restored.log"
step=0
# The folder as Windows names it, D:\a\..., which Chrome can move downloads into (#872), and
# which the app logs as given.
work_downloads="$(native_path "$PWD/$out/work-downloads")"
mkdir -p "$work_downloads"
rm -f "$work_downloads/profile.txt"
BROWSER_DOWNLOAD_FOLDER="$work_downloads" BROWSER_DRIVE=1 "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch didn't restore the Spaces"
await '^browser: spaces [0-9]* [0-9]*$' || fail "the relaunch didn't bring back two Spaces"
first=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f3)
second=$(grep '^browser: spaces' "$log" | tail -1 | cut -d' ' -f4)
default=$(sed -n 's/^browser: added profile \([0-9]*\) Default$/\1/p' "$log")
work=$(sed -n 's/^browser: added profile \([0-9]*\) Profile 2$/\1/p' "$log")
[ -n "$work" ] || fail "the new profile wasn't listed after the relaunch"
grep -q "^browser: profiled space $first $default\$" "$log" ||
  fail "the first Space came back in another profile"
grep -q "^browser: profiled space $second $work\$" "$log" ||
  fail "the second Space came back in another profile"
await "^browser: favorites [0-9][0-9]*\$" || fail "the first profile's favorite didn't come back"
a=$(grep '^browser: favorites' "$log" | tail -1 | cut -d' ' -f3)
await "^browser: title $a cookie p\$" ||
  fail "the favorite came back without its profile's cookie"
send click $page_point
send key 2 ctrl
expect "^browser: space $second in " "⌃2 after the relaunch didn't show the second Space"
expect '^browser: favorites $' "the second Space showed the first profile's favorite"
b=$(today)
expect "^browser: title $b cookie none\$" "the second Space's tab came back in the first profile"
# Its menu moves it to the first profile's Space, after a warning the first time, as a tab
# whose page loads there with that profile's cookie. Moved back, from under the favorite's tile,
# it asks nothing.
post rightclick $(row 0)
expect '^browser: menu shown .*, Move to › Space 1, Move to › work (checked), ' \
  "the tab's menu didn't offer the other profile's Space"
post choose Space 1
expect '^browser: dialog shown Move this tab to the “Default” profile?$' \
  "moving the tab to another profile's Space didn't warn first"
expect '^browser: dialog settled$' "the dialog didn't settle"
../tools/window-screenshot.sh "$pid" "$out/profile-move.png" || fail "no window to capture"
send key return
expect "^browser: tab $b closed\$" "the tab moved to another profile kept its old page"
expect "^browser: space $first\$" "the window didn't go with the moved tab"
expect '^browser: today [0-9][0-9]*$' "the moved tab isn't in the first Space"
c=$(today)
expect "^browser: title $c cookie p\$" "the moved tab didn't load with the first profile's cookie"
post rightclick 100 $(($(row 0 | cut -d' ' -f2) + 55))
expect '^browser: menu shown .*, Move to › Space 1 (checked), Move to › work, ' \
  "the moved tab's menu didn't offer its old Space"
send choose work
expect "^browser: tab $c closed\$" "the tab moved back kept its page in the first profile"
none_since '^browser: dialog shown' || fail "the second move to another profile warned again"
expect "^browser: space $second\$" "the window didn't go with the tab moved back"
expect '^browser: today [0-9][0-9]*$' "the tab moved back isn't in the second Space"
d=$(today)
expect "^browser: title $d cookie none\$" "the tab moved back still saw the first profile's cookie"
# In Every Space, the favorite has a twin in the other profile's Space. Taken away from its URL and
# removed from the favorites, its twin goes too, and ⌘Z brings back both, each going back to the
# favorite's URL in its own profile.
printf '%s' '<title>away</title>' > "$out/site/away.html"
printf '%s' '<title>elsewhere</title>' > "$out/site/elsewhere.html"
away_page="$(echo "$site" | sed 's/127\.0\.0\.1/localhost/')away.html"
elsewhere_page="$(echo "$site" | sed 's/127\.0\.0\.1/localhost/')elsewhere.html"
tile="114 110"
send menu Settings…
expect '^browser: settings shown General$' "Settings… didn't show Settings on its General section"
settings_drew General
send clickkey $(drawn "toolbar Tabs & Sidebar")
expect '^browser: settings tab Tabs & Sidebar$' "a click on Tabs & Sidebar didn't show its section"
settings_drew 'Tabs & Sidebar'
in_sight "share favorites option In every Space" || fail "Settings didn't draw In Every Space's drawing"
send clickkey $at
expect "^browser: favorites scope In Every Space\$" "a click on In Every Space's drawing didn't set it"
expect '^browser: favorites [0-9][0-9]*$' "the favorite has no twin in the other profile's Space"
twin=$(grep '^browser: favorites' "$log" | tail -1 | cut -d' ' -f3)
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
[ "$twin" != "$a" ] || fail "the other profile's Space shows the first profile's favorite itself"
send key 1 ctrl
expect "^browser: space $first in " "⌃1 didn't show the first Space"
expect "^browser: settled space $first\$" "the first Space's page didn't settle"
send click $tile
expect "^browser: selected $a\( in .*\)\{0,1\}\$" "a click on the tile didn't show the favorite"
go_from_bar "$away_page"
expect "^browser: title $a away\$" "the favorite didn't go to another page"
expect "^browser: page $a away 1\$" "the favorite away from its URL didn't say so"
post rightclick $tile
expect "^browser: menu shown $(cased 'Back to Favorite URL, Replace Favorite URL with Current, Duplicate Tab, Remove from Favorites, ')" \
  "the favorite's menu didn't offer Remove from Favorites"
send choose "$(cased 'Remove from Favorites')"
expect '^browser: favorites $' "Remove from Favorites left the tile"
send key z cmd
expect '^browser: command undo via \(ui\|page\)$' "⌘Z didn't undo Remove from Favorites"
expect "^browser: favorites $a\$" "⌘Z didn't put the favorite back"
# Back to Favorite URL takes each twin to the URL the favorite had, in its own profile.
post rightclick $tile
expect "^browser: menu shown $(cased 'Back to Favorite URL, ')" "⌘Z put the favorite back without its URL"
send choose "$(cased 'Back to Favorite URL')"
expect "^browser: title $a cookie p\$" "Back to Favorite URL didn't load the favorite's URL"
send key 2 ctrl
expect "^browser: space $second in " "⌃2 after Back to Favorite URL didn't show the second Space"
expect '^browser: favorites [0-9][0-9]*$' "⌘Z didn't bring back the favorite's twin"
twin=$(grep '^browser: favorites' "$log" | tail -1 | cut -d' ' -f3)
[ "$twin" != "$a" ] || fail "the other profile's Space shows the first profile's favorite itself"
expect "^browser: settled space $second\$" "the second Space's page didn't settle"
send click $tile
expect "^browser: selected $twin\( in .*\)\{0,1\}\$" "a click on the twin's tile didn't show it"
# The twin opens where the favorite is, so it goes to a page of its own.
go_from_bar "$elsewhere_page"
expect "^browser: title $twin elsewhere\$" "the twin didn't go to another page"
post rightclick $tile
expect "^browser: menu shown $(cased 'Back to Favorite URL, ')" "the twin away from its URL didn't offer it back"
send choose "$(cased 'Back to Favorite URL')"
expect "^browser: title $twin cookie none\$" \
  "the twin's Back to Favorite URL didn't load the favorite's URL in its own profile"
# Back to In Each Profile's Spaces, each profile keeps the site as its own favorite. Removed and
# closed, the second Space has only its tab again.
send menu Settings…
expect '^browser: settings shown Tabs & Sidebar$' "Settings… didn't show Tabs & Sidebar again"
settings_drew 'Tabs & Sidebar'
in_sight "share favorites In Every Space" || fail "Share favorites didn't show In Every Space chosen"
in_sight "share favorites option In each profile's Spaces" ||
  fail "Settings didn't draw In Each Profile's Spaces' drawing"
send clickkey $at
expect "^browser: favorites scope In Each Profile's Spaces\$" \
  "a click on In Each Profile's Spaces' drawing didn't set it"
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
[ "$(grep '^browser: favorites' "$log" | tail -1)" = "browser: favorites $twin" ] ||
  fail "the second profile didn't keep the twin as its own favorite: $(grep '^browser: favorites' "$log" | tail -1)"
post rightclick $tile
expect "^browser: menu shown $(cased 'Back to Favorite URL (disabled), Replace Favorite URL with Current (disabled), Duplicate Tab, Remove from Favorites, ')" \
  "the second profile's favorite's menu didn't offer Remove from Favorites"
send choose "$(cased 'Remove from Favorites')"
expect '^browser: favorites $' "Remove from Favorites left the second profile's tile"
expect "^browser: today $twin $d\$" "the removed favorite isn't at the top of Today: Today is \"$(today)\""
send click $page_point
send key w cmd
expect "^browser: today $d\$" "⌘W didn't close the removed favorite: Today is \"$(today)\""
expect "^browser: selected $d\( in .*\)\{0,1\}\$" "⌘W didn't show the second Space's tab"
# ⇧⌘⌫ opens Chromium's Clear browsing data in a tab of the Space shown, so in its profile, and ⌘W
# closes it.
send click $page_point
send key backspace shift cmd
expect '^browser: command clear-browsing-data via \(ui\|page\)$' "⇧⌘⌫ didn't run Clear Browsing Data"
expect '^browser: loaded [0-9]* chrome://settings/clearBrowserData$' \
  "⇧⌘⌫ didn't open Clear browsing data: $(grep '^browser: loaded' "$log" | tail -1)"
expect "^browser: today [0-9]* $d\$" "Clear browsing data didn't open in the Space shown"
../tools/window-screenshot.sh "$pid" "$out/clear-browsing-data.png" || fail "no window to capture"
send key w cmd
expect "^browser: today $d\$" "⌘W didn't close Clear browsing data: Today is \"$(today)\""
# Settings opens only once the tab ⌘W leaves is shown: its page, shown later, would take the
# keyboard back from Settings, and the click on its section would go to the browser window.
expect "^browser: selected $d\( in .*\)\{0,1\}\$" "⌘W didn't show the second Space's tab again"
# Clear Browsing Data run from the bar is a pick, kept in the profile's folder. The dialog's Delete,
# with the history it ticks, deletes the history of its time, which takes the pick: Cancel has the
# keyboard, and Delete is after it.
bar_command clear-browsing-data "Clear Browsing Data" "clear browsing"
expect '^browser: command bar picks kept in ' "a pick in the command bar wasn't kept"
picks=$(sed -n 's/^browser: command bar picks kept in //p' "$log" | tail -1)
[ -e "$picks" ] || fail "the command bar's picks aren't where it said: $picks"
expect '^browser: loaded [0-9]* chrome://settings/clearBrowserData$' \
  "the command bar didn't open Clear browsing data: $(grep '^browser: loaded' "$log" | tail -1)"
# The dialog takes the keyboard a moment after its page loads, which the app's log can't tell
# (clear.sh asks DevTools, which this run hasn't): Tab and Space sent before then follow the
# settings page's link instead, and ⌘← brings the dialog back for another try.
deleted=
for _ in 1 2 3; do
  send key tab
  send key space
  missed=
  for _ in $(seq 100); do
    logged '^browser: history deleted ' && deleted=1 && break
    logged '^browser: page [0-9]* back 1 forward 0 loading [01] chrome://settings/' && missed=1 && break
    sleep 0.1
  done
  [ -z "$deleted" ] && [ -n "$missed" ] || break
  history_key left back page
  expect '^browser: page [0-9]* back 0 forward 1 loading 0 chrome://settings/clearBrowserData$' \
    "⌘← didn't bring Clear browsing data back"
done
[ -n "$deleted" ] || fail "Delete in Clear browsing data didn't delete the history"
# A pattern of the path, whose backslashes on Windows grep would read as its own, \b in \browser-app.
expect "^browser: command bar picks none in $(printf '%s\n' "$picks" | sed 's/[][\\.*^$]/\\&/g')\$" \
  "deleting the history kept the bar's picks"
[ ! -e "$picks" ] || fail "deleting the history left the bar's picks: $picks"
../tools/window-screenshot.sh "$pid" "$out/clear-browsing-data-deleted.png" || fail "no window to capture"
send key w cmd
expect "^browser: today $d\$" "⌘W didn't close Clear browsing data: Today is \"$(today)\""
# A profile's own settings sit in the sections they belong to, a row per profile: its search
# engine and download folder in General, its archive time in Tabs & Sidebar. Settings › Profiles
# lists the profiles, with New Profile under them. Settings logs where it draws each control the
# run clicks. Change… sends the second profile's downloads to the folder the run named.
send menu Settings…
expect '^browser: settings shown Tabs & Sidebar$' "Settings… didn't show Tabs & Sidebar"
settings_drew 'Tabs & Sidebar'
send clickkey $(drawn "toolbar General")
expect '^browser: settings tab General$' "a click on General didn't show its section"
settings_drew 'General'
in_sight "download folder $work" || fail "Settings didn't draw the second profile's Change…"
../tools/window-screenshot.sh "$pid" "$out/settings-profile-boxes.png" || fail "no window to capture"
send clickkey $at
expect "^browser: download folder of profile $work $(printf '%s\n' "$work_downloads" | sed 's/[][\\.*^$]/\\&/g') set\$" \
  "Change… didn't set the second profile's download folder: $(grep '^browser: download folder' "$log" | tail -1)"
# Each profile picks its search engine and archive time, the first keeping its own.
post clickkey $(drawn "search engine $work")
expect '^browser: menu shown .*DuckDuckGo (checked)' \
  "the second profile's Search engine didn't show the engines, with DuckDuckGo chosen"
send choose Microsoft Bing
expect "^browser: profile $work searches with Microsoft Bing\$" "choosing Bing didn't set the second profile's"
send clickkey $(drawn "toolbar Tabs & Sidebar")
expect '^browser: settings tab Tabs & Sidebar$' "a click on Tabs & Sidebar didn't show its section"
settings_drew 'Tabs & Sidebar'
post clickkey $(drawn "archive $work")
expect '^browser: menu shown 12 hours (default) (checked), 24 hours, 7 days, 30 days, Never$' \
  "the second profile's Archive Today tabs after didn't show the times, with 12 hours chosen"
send choose 7 days
expect "^browser: profile $work archive tabs after 7 days\$" "choosing 7 days didn't set the second profile's"
post clickkey $(drawn "archive $default")
expect '^browser: menu shown 12 hours (default) (checked), 24 hours, 7 days, 30 days, Never$' \
  "the second profile's time changed the first's"
send key escape
expect '^browser: menu closed$' "escape didn't close the first profile's times"
post clickkey $(drawn "archive $work")
expect '^browser: menu shown 12 hours (default), 24 hours, 7 days (checked), 30 days, Never$' \
  "the second profile didn't keep 7 days"
send key escape
expect '^browser: menu closed$' "escape didn't close the second profile's times"
../tools/window-screenshot.sh "$pid" "$out/settings-profile-settings.png" || fail "no window to capture"
# Each profile picks the service New Note opens a new document in, Google Docs until it picks one
# (#698): the first Notion, the second Word.
send clickkey $(drawn "toolbar Features")
expect '^browser: settings tab Features$' "a click on Features didn't show its section"
settings_drew Features
in_sight "notes $work" || fail "Settings didn't draw the second profile's New Note service"
post clickkey $(drawn "notes $default")
expect '^browser: menu shown Notion, Google Docs (checked), Word, Confluence$' \
  "the first profile's New Note didn't offer the services, with Google Docs chosen"
send choose Notion
expect "^browser: notes service $default Notion\$" "choosing Notion didn't set the first profile's service"
post clickkey $(drawn "notes $work")
expect '^browser: menu shown Notion, Google Docs (checked), Word, Confluence$' \
  "the first profile's service changed the second's"
send choose Word
expect "^browser: notes service $work Word\$" "choosing Word didn't set the second profile's service"
../tools/window-screenshot.sh "$pid" "$out/settings-new-note.png" || fail "no window to capture"
send clickkey $(drawn "toolbar Profiles")
expect '^browser: settings tab Profiles$' "a click on Profiles didn't show its section"
settings_drew 'Profiles'
# Rename, or a double-click on a name, puts it in a field, where escape leaves it and return
# keeps a new one.
click_in_sight "rename profile $work"
expect "^browser: rename profile $work\$" "Rename on Profile 2 didn't put its name in a field"
send type personal
send key escape
expect '^browser: rename cancelled$' "escape didn't cancel the renaming"
none_since '^browser: renamed profile' || fail "escape renamed the profile"
click_in_sight "profile $work" doubleclickkey
expect "^browser: rename profile $work\$" "a double-click on Profile 2 didn't put it in a field"
send type personal
send key return
expect "^browser: renamed profile $work personal\$" "return didn't rename the profile"
# New Profile makes one, with its name in a field, which takes Japanese input. Its Delete asks
# first, and Cancel keeps it.
click_in_sight "new profile"
expect '^browser: create profile [0-9]* Profile 3$' "New Profile didn't make a profile"
spare=$(sed -n 's/^browser: create profile \([0-9]*\) .*/\1/p' "$log" | tail -1)
expect "^browser: rename profile $spare\$" "the new profile's name didn't take the keyboard"
send type spare
send key space
send ime on
send type nihongo
send key space
[ "$marked" = 6,3 ] || fail "the profile's field didn't take the conversion: marked $marked"
../tools/window-screenshot.sh "$pid" "$out/settings-profile-name.png" || fail "no window to capture"
send key return
send ime off
send key return
expect "^browser: renamed profile $spare spare 日本語\$" \
  "the new profile wasn't named: $(grep '^browser: renamed profile' "$log" | tail -1)"
# What the app keeps for a profile goes in its folder, as the command bar's picks do, so it goes
# with the profile. A Space in it would have made them; the run puts them there itself.
spare_picks="$(sed -n "s/^browser: profile $spare folder //p" "$log" | tail -1)/Command bar picks"
[ "$spare_picks" != "/Command bar picks" ] || fail "the app didn't log the new profile's folder"
mkdir -p "$(dirname "$spare_picks")"
printf 'New Window\t1\n' > "$spare_picks"
../tools/window-screenshot.sh "$pid" "$out/settings-profiles.png" || fail "no window to capture"
# Settings asks in an alert, and off the Mac, which has none, in its dialog, where escape cancels
# and return deletes. The dialog covers Settings, taking its clicks, until it has gone.
asks=alert
[ "$(uname)" = Linux ] && asks=dialog
settings_answer() {
  if [ "$asks" = dialog ]; then
    expect '^browser: dialog settled$' "Settings' dialog didn't settle"
    case $1 in Cancel) send key escape ;; *) send key return ;; esac
    expect '^browser: dialog hidden$' "Settings' dialog didn't go"
  else
    send choose "$1"
  fi
}
post clickkey $(drawn "delete profile $spare")
expect "^browser: $asks shown Delete “spare 日本語”?\$" "Delete in Settings didn't ask first"
settings_answer Cancel
none_since '^browser: removed profile' || fail "Cancel deleted the profile"
post clickkey $(drawn "delete profile $spare")
expect "^browser: $asks shown Delete “spare 日本語”?\$" "Delete in Settings didn't ask again"
settings_answer Delete
expect "^browser: removed profile $spare spare 日本語\$" "Delete in Settings didn't delete it"
for _ in $(seq 300); do
  [ -e "$spare_picks" ] || break
  sleep 0.1
done
[ ! -e "$spare_picks" ] || fail "the deleted profile left its command bar picks: $spare_picks"
# The first profile's Clear… in Privacy opens Clear browsing data in its Space, leaving Settings.
send clickkey $(drawn "toolbar Privacy")
expect '^browser: settings tab Privacy$' "a click on Privacy didn't show its section"
settings_drew Privacy
click_in_sight "clear $default"
expect "^browser: clear browsing data of profile $default in space $first\$" \
  "the first profile's Clear… didn't open Clear browsing data in its Space"
expect '^browser: settings hidden$' "Clear… didn't hide Settings"
expect "^browser: space $first\( in .*\)\{0,1\}\$" "Clear… didn't show the first profile's Space"
expect '^browser: loaded [0-9]* chrome://settings/clearBrowserData$' \
  "Clear… didn't open Clear browsing data: $(grep '^browser: loaded' "$log" | tail -1)"
send key w cmd
# The command bar searches with the engine of the profile shown: the first's DuckDuckGo, the
# second's Bing. Neither search waits for its page, and ⌘W closes its tab once it's shown: sent
# before, on GitHub's hosted Mac, it closed the tab shown before it (#2205).
search_shown() {
  expect "^browser: page [0-9]* back 0 forward 0 loading 1 $1" "the search didn't open its tab"
  searched=$(grep "^browser: page [0-9]* back 0 forward 0 loading 1 $1" "$log" | tail -1 | cut -d' ' -f3)
  expect "^browser: selected $searched\( in .*\)\{0,1\}\$" "the search's tab $searched wasn't shown"
}
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type ilo profiles
send key return
expect '^browser: command bar go https://duckduckgo\.com/?q=ilo' \
  "the first profile's Space didn't search with DuckDuckGo: $(grep '^browser: command bar go' "$log" | tail -1)"
search_shown 'https://duckduckgo\.com/?q=ilo'
send key w cmd
send key 2 ctrl
expect "^browser: space $second in " "⌃2 after ⌘W closed the first Space's search didn't show the second Space"
expect "^browser: settled space $second\$" "the second Space's page didn't settle"
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type ilo profiles
send key return
expect '^browser: command bar go https://www\.bing\.com/search?q=ilo' \
  "the second profile's Space didn't search with Bing: $(grep '^browser: command bar go' "$log" | tail -1)"
search_shown 'https://www\.bing\.com/search?q=ilo'
send key w cmd
expect "^browser: selected $d\( in .*\)\{0,1\}\$" "⌘W didn't show the second Space's tab again"
# ⌃⇧N opens a new document in the second profile's service, Word, at the top of Today.
send key n ctrl shift
expect '^browser: new note https://word\.new$' "⌃⇧N in the second profile's Space didn't open a new Word document"
expect "^browser: today [0-9][0-9]* $d\$" "⌃⇧N's note didn't open at the top of Today: Today is \"$(today)\""
send key w cmd
expect "^browser: selected $d\( in .*\)\{0,1\}\$" "⌘W didn't close the second profile's note"
# The second profile's page downloads into the folder Change… gave it.
send click $page_point
send type d
expect '^browser: download [0-9]* done profile.txt$' "the second profile's page didn't download"
[ -f "$work_downloads/profile.txt" ] ||
  fail "the second profile's download isn't in the folder Change… gave it"
# Its icon flies to the Library button first, over the sidebar the drag below starts in.
flight=$(grep '^browser: download [0-9]* done profile.txt$' "$log" | tail -1 | cut -d' ' -f3)
expect "^browser: download landed $flight\$" "the second profile's download didn't land"
# Its tab made the profile's favorite, the Space is empty, and deleted at once it leaves the
# profile with a favorite no Space shows. The first Space's menu then deletes the profile, favorite
# and all, once the user confirms (#1524).
send press $(row 0)
send dragto 100 160
send dragto 114 112
expect '^browser: drag shape tile$' "the second Space's row dragged onto its name didn't turn into a tile"
send release 114 112
expect "^browser: favorites $d\$" "the second Space's row dragged onto its name didn't make a favorite"
post rightclick $tiled_name
expect "^browser: menu shown $(cased 'Change Space Icon…'), " \
  "a right-click on the Space's name didn't show its menu"
post choose Delete Space
expect "^browser: spaces $first\$" "Delete didn't delete the second Space"
expect "^browser: space $first\( in .*\)\{0,1\}\$" "deleting the second Space didn't show the first"
expect "^browser: settled space $first\$" "the first Space's page didn't settle"
post rightclick $tiled_name
# The Space is now the lone one, and unnamed, so its menu also offers to hide its name row.
expect "^browser: menu shown $(cased "Change Space Icon…, Rename Space…, $colors, Hide Space Header, \
Set Profile › Default (checked), Set Profile › personal, Set Profile › New Profile, Set Profile › Delete “personal”, \
New Folder…, ${into}Export as Markdown…, Manage Spaces…, Delete Space (disabled)")\$" \
  "the menu didn't offer to delete the unused profile by the name Settings gave it"
post choose Delete “personal”
expect '^browser: dialog shown Delete “personal”?$' "Delete didn't ask before deleting the profile"
expect '^browser: dialog settled$' "the dialog didn't settle"
../tools/window-screenshot.sh "$pid" "$out/profile-delete.png" || fail "no window to capture"
send key return
expect "^browser: removed profile $work personal\$" "the profile wasn't deleted"
# The first profile's New Note opens a Notion page from the command bar and from the footer's +,
# each at the top of Today, and New Note Split opens one beside the page shown (#698).
# Picks the command bar's row $1, typed, with ↓ and ↩, once the core's rows are in among the bar's.
bar_runs() {
  send key t cmd
  expect '^browser: command bar shown$' "⌘T didn't open the command bar"
  send type "$1"
  expect "^browser: command bar lists \"$1\": .*https://duckduckgo.com/" "the core's rows for \"$1\" never came"
  index=$(grep "^browser: command bar lists \"$1\": " "$log" | tail -1 |
    sed 's/^browser: command bar lists "[^"]*": //' |
    awk -v want="$1" '{ n = split($0, rows, ", "); for (i = 1; i <= n; i++) if (rows[i] == want) { print i - 1; exit } }')
  [ -n "$index" ] || fail "the command bar didn't list $1: $(grep "^browser: command bar lists \"$1\": " "$log" | tail -1)"
  down=0
  while [ "$down" -lt "$index" ]; do
    send key down
    down=$((down + 1))
  done
  send key return
  expect "^browser: command bar runs $1\$" "↩ on $1 ran another row"
}
before=$(today)
# The note goes first in Today, ahead of what was there; with Today empty it's alone.
atop="^browser: today [0-9][0-9]*${before:+ $before}\$"
bar_runs "New Note"
expect '^browser: new note https://notion\.new$' "the command bar's New Note didn't open a new Notion page"
expect "$atop" "the bar's note didn't open at the top of Today: Today is \"$(today)\""
send key w cmd
expect "^browser: today $before\$" "⌘W didn't close the bar's note"
send layout
height=$(sed -n "/^browser: drive $step: /,\$p" "$log" | grep -m1 '^browser: layout' |
  sed 's/^browser: layout content [0-9]*x\([0-9]*\),.*/\1/')
post click 204 $((height - 25))
expect "^browser: menu shown $(cased 'New Space…, New Folder…, New Split View (disabled), New Note'), " \
  "the footer's + didn't offer New Note"
send choose "$(cased 'New Note')"
expect '^browser: new note https://notion\.new$' "the footer's New Note didn't open a new Notion page"
expect "$atop" "the footer's note didn't open at the top of Today: Today is \"$(today)\""
# New Note Split opens beside the page shown, as the bar offers it only with one, so the footer's
# note stays for it.
shown=$(sed -n 's/^browser: selected \([0-9]*\).*/\1/p' "$log" | tail -1)
bar_runs "New Note Split"
expect '^browser: new note https://notion\.new split$' "New Note Split didn't open a new Notion page"
expect "^browser: splits\(.* \)\{0,1\}$shown+[0-9][0-9]*" "New Note Split's note isn't beside the page"
../tools/window-screenshot.sh "$pid" "$out/new-note-split.png" || fail "no window to capture"
# Off in Settings › Features, as its neighbours can be, New Note leaves the + menu and ⌃⇧N does
# nothing; on again, ⌃⇧N opens one (#1636). Its switch: Settings' Features section, then ⌘W.
# Settings opens again on the section it showed last, so Features is clicked only the first time.
new_note_switch() {
  send menu Settings…
  expect '^browser: settings shown ' "Settings… didn't show Settings for New Note's switch"
  if [ "$1" = off ]; then
    send clickkey $(drawn "toolbar Features")
    expect '^browser: settings tab Features$' "a click on Features didn't show its section"
  fi
  settings_drew Features
  click_in_sight "switch new note"
  expect "^browser: new note $1\$" "a click on New Note's switch didn't turn it $1"
  [ "$1" = on ] || ../tools/window-screenshot.sh "$pid" "$out/settings-new-note-off.png" ||
    fail "no window to capture"
  send key w cmd
  expect '^browser: settings hidden$' "⌘W didn't hide Settings"
}
new_note_switch off
# The app takes keys in order, so ⌘T's bar says ⌃⇧N has been handled.
pressed=$((step + 1))
send key n ctrl shift
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar after ⌃⇧N"
none_since '^browser: new note https' "$pressed" || fail "⌃⇧N opened a note with New Note off"
send key escape
expect '^browser: command bar hidden$' "escape didn't close the command bar"
post click 204 $((height - 25))
expect "^browser: menu shown $(cased 'New Space…, New Folder…, New Split View'), " \
  "the footer's + didn't show its menu with New Note off"
grep '^browser: menu shown ' "$log" | tail -1 | grep -q "$(cased 'New Note')" &&
  fail "the footer's + offered New Note while it's off: $(grep '^browser: menu shown ' "$log" | tail -1)"
send key escape
expect '^browser: menu closed$' "escape didn't close the footer's menu"
new_note_switch on
send key n ctrl shift
expect '^browser: new note https://notion\.new$' "⌃⇧N didn't open a note once New Note was on again"
exec 3>&-
quit
echo "ok: the New Space form's Profile row makes a profile and puts the Space in it, where its tabs open without the first profile's cookie, from a command bar that followed it from the first profile's Space too, and its Choose a Theme row colours it; an empty Space's menu moves it between profiles, and a Space with tabs keeps its profile; held partway between two profiles' Spaces, each page has its own URL field and favorites; a new Space is in the profile of the Space it's made from; favorites show only in their profile's Spaces; after a relaunch each Space is in its profile and each tab loads in it; a tab moved to another profile's Space loads there with its cookies, after a warning only the first time; in Every Space a favorite has a twin in the other profile, which goes with it when it’s removed, comes back with ⌘Z, and goes back to the favorite’s URL in its own profile, and stays the other profile’s own favorite after; ⇧⌘⌫ opens Clear browsing data in the Space shown; Clear Browsing Data from the command bar is a pick, kept in the profile's folder, which the dialog's Delete takes with the history; Settings › Profiles sets the second profile's download folder, which its page downloads into, its search engine, which the command bar then searches with in its Space while the first's keeps DuckDuckGo, and its archive time, leaving the first's, opens Clear browsing data for the first in its Space, renames a profile on a double-click, which the Space menu shows, and makes one, named with Japanese input, and deletes it once the user confirms, with what the app kept in its folder; a profile no Space is in is deleted from the menu, its favorite with it, once the user confirms; Little ilo's bar reopens its page in the other profile, without the first's cookie, and keeps that profile for the site, where a link from another app to the site then opens, in that profile's Space; each profile's New Note opens its own service's page, by ⌃⇧N, the command bar, the footer's + and New Note Split beside the page, and with its switch in Settings off, the + menu has no New Note and ⌃⇧N does nothing until it's on again. Windows: $out/profile-popup.png, $out/little-profile.png, $out/little-kept-profile.png, $out/profile-form.png, $out/profile-made.png, $out/profile-menu.png, $out/profile-space.png, $out/profile-swipe.png, $out/profile-move.png, $out/clear-browsing-data.png, $out/clear-browsing-data-deleted.png, $out/settings-profile-boxes.png, $out/settings-profile-settings.png, $out/settings-profile-name.png, $out/settings-profiles.png, $out/profile-delete.png, $out/settings-new-note.png, $out/new-note-split.png, $out/settings-new-note-off.png"
