# Settings › Import finds the made-up Arc in src/import/fixture and a made-up Chrome and Firefox
# beside it, never the Mac's own browsers. From Arc it brings its two Spaces, one in a new profile
# named Work as Arc's Local State names it, their pinned tabs and folders, its favorites, and the
# three of its link rules that can be ours, each to the Space its Arc Space became; from
# Chrome and from Firefox their bookmarks as a Space named after each and their history, which the
# core adds, with Chrome's open tabs, but for the one a bookmark pins, as that Space's Today tabs,
# and its saved address, which the core adds too, never its card. Run again, each adds nothing. A
# bookmarks file, made up here in the format browsers export, comes in as a Space named Bookmarks,
# and a second time adds nothing. Then a passwords file, made up here, comes in through Chrome's
# password importer: the view says how many came in, deletes the file when asked, and a second file
# with a login's password changed says it conflicted and kept the saved one. Settings' Passwords
# tab lists what came in, and the app menu's Import from Other Browsers… shows Import again. With
# the sidebar hidden, Arc's Spaces say so in a toast. Each step is captured. Arc has no Linux build,
# so off the Mac the fixture's Arc isn't found, its steps are left out, and Chrome and Firefox are
# where Linux keeps them.

serve
printf '%s' '<title>import ready</title>' > "$out/site/import.html"
log="$out/import.log"
step=0
rules_file="$(dirname "$saved")/Link rules"
rm -f "$saved" "$rules_file"
# A copy of the fixture's home, with Chrome's History and Web Data and Firefox's places.sqlite
# written as SQLite here rather than kept in the repo as binary files.
home="$out/import-home"
rm -rf "$home"
cp -R src/import/fixture "$home"
if [ "$(uname)" = Darwin ]; then
  arc=1
  chrome="$home/Library/Application Support/Google/Chrome"
  firefox="$home/Library/Application Support/Firefox"
else
  arc=
  chrome="$home/.config/google-chrome"
  firefox="$home/.mozilla/firefox"
fi
mkdir -p "$chrome/Default"
printf '%s' '{"profile":{"info_cache":{"Default":{"name":"Me"}}}}' > "$chrome/Local State"
printf '%s' '{}' > "$chrome/Default/Preferences"
cat > "$chrome/Default/Bookmarks" << 'EOF'
{"roots": {
  "bookmark_bar": {"type": "folder", "name": "Bookmarks bar", "children": [
    {"type": "url", "name": "Mail", "url": "https://mail.example.org/"},
    {"type": "url", "name": "", "url": "https://example.org/"}]},
  "other": {"type": "folder", "name": "Other bookmarks", "children": [
    {"type": "url", "name": "Docs", "url": "https://docs.example.org/"}]},
  "synced": {"type": "folder", "name": "Mobile bookmarks", "children": []}}, "version": 1}
EOF
# Chrome's last session, as its SNSS commands: a browser window with two tabs, the second a
# bookmark's page.
mkdir -p "$chrome/Default/Sessions"
perl -e '
sub cmd { my ($id, $body) = @_; pack("S<C", length($body) + 1, $id) . $body }
sub nav { my ($tab, $index, $url) = @_;
  my $pickle = pack("l<3", $tab, $index, length $url) . $url;
  $pickle .= "\0" x (-length($pickle) % 4);
  $pickle .= pack("l<", 0);
  pack("l<", length $pickle) . $pickle }
print "SNSS", pack("l<", 3), cmd(9, pack("l<2", 1, 0)),
  cmd(0, pack("l<2", 1, 2)), cmd(2, pack("l<2", 2, 0)), cmd(6, nav(2, 0, "https://open.example.org/")),
  cmd(0, pack("l<2", 1, 3)), cmd(2, pack("l<2", 3, 1)), cmd(6, nav(3, 0, "https://mail.example.org/")),
  cmd(255, "")' > "$chrome/Default/Sessions/Session_13400000000000000"
command -v sqlite3 > /dev/null || fail "no sqlite3 to write Chrome's History with"
# Chromium counts microseconds from 1601: an hour and two hours ago.
now=$(( ($(date +%s) + 11644473600) * 1000000 ))
sqlite3 "$chrome/Default/History" "
  CREATE TABLE urls(id INTEGER PRIMARY KEY AUTOINCREMENT, url LONGVARCHAR, title LONGVARCHAR,
    visit_count INTEGER DEFAULT 0 NOT NULL, typed_count INTEGER DEFAULT 0 NOT NULL,
    last_visit_time INTEGER NOT NULL, hidden INTEGER DEFAULT 0 NOT NULL);
  INSERT INTO urls(url, title, visit_count, typed_count, last_visit_time) VALUES
    ('https://history-one.example.org/', 'One', 3, 1, $now - 3600000000),
    ('https://history-two.example.org/', 'Two', 1, 0, $now - 7200000000);"
# Chrome's saved address, field by field as Chromium's FieldType numbers (full name, city, country)
# with each typed by the user, and a made-up card beside it, which the import never reads.
sqlite3 "$chrome/Default/Web Data" "
  CREATE TABLE address_type_tokens(guid VARCHAR, type INTEGER, value VARCHAR,
    verification_status INTEGER DEFAULT 0, observations BLOB);
  INSERT INTO address_type_tokens(guid, type, value, verification_status) VALUES
    ('a', 7, 'Check Example', 4), ('a', 33, 'Utrecht', 4), ('a', 36, 'NL', 4);
  CREATE TABLE credit_cards(guid VARCHAR, card_number_encrypted BLOB);
  INSERT INTO credit_cards VALUES ('card', x'763130');"
mkdir -p "$firefox/Profiles/x.default-release"
printf '[Profile0]\nName=default-release\nIsRelative=1\nPath=Profiles/x.default-release\n' \
  > "$firefox/profiles.ini"
# Firefox counts microseconds from 1970: its two bookmarks never visited, two pages visited an hour
# and two hours ago.
now=$(( $(date +%s) * 1000000 ))
sqlite3 "$firefox/Profiles/x.default-release/places.sqlite" "
  CREATE TABLE moz_places(id INTEGER PRIMARY KEY, url LONGVARCHAR, title LONGVARCHAR,
    visit_count INTEGER DEFAULT 0, hidden INTEGER DEFAULT 0 NOT NULL,
    typed INTEGER DEFAULT 0 NOT NULL, last_visit_date INTEGER, guid TEXT);
  CREATE TABLE moz_bookmarks(id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER DEFAULT NULL,
    parent INTEGER, position INTEGER, title LONGVARCHAR, guid TEXT);
  INSERT INTO moz_places(id, url, title, visit_count, last_visit_date) VALUES
    (1, 'https://fox-mail.example.org/', 'Mail', 0, NULL),
    (2, 'https://fox-docs.example.org/', 'Docs', 0, NULL),
    (3, 'https://history-three.example.org/', 'Three', 2, $now - 3600000000),
    (4, 'https://history-four.example.org/', 'Four', 1, $now - 7200000000);
  INSERT INTO moz_bookmarks(id, type, fk, parent, position, title, guid) VALUES
    (1, 2, NULL, NULL, 0, '', 'root________'), (2, 2, NULL, 1, 0, 'menu', 'menu________'),
    (3, 2, NULL, 1, 1, 'toolbar', 'toolbar_____'), (4, 1, 1, 3, 0, 'Mail', NULL),
    (5, 1, 2, 2, 0, 'Docs', NULL);"
# Arc used last, so it's listed first, then Chrome, then Firefox.
touch -t 200001010000 "$chrome/Default/History" "$chrome/Default/Preferences"
touch -t 199901010000 "$firefox/Profiles/x.default-release/places.sqlite"
touch "$home/Library/Application Support/Arc/StorableSidebar.json"
# The file the open panel would give, which a run can't drive: made-up logins, never real ones.
passwords="$out/passwords.csv"
printf '%s\n' 'name,url,username,password,note' \
  'login,https://login.example.test/,check@example.test,made-up-one,' \
  'mail,https://mail.example.test/,check@example.test,made-up-two,' > "$passwords"
# And the bookmarks file, as a browser exports it.
bookmarks="$out/bookmarks.html"
printf '%s\n' '<!DOCTYPE NETSCAPE-Bookmark-file-1>' '<TITLE>Bookmarks</TITLE>' \
  '<H1>Bookmarks</H1>' '<DL><p>' '<DT><H3 PERSONAL_TOOLBAR_FOLDER="true">Bookmarks bar</H3>' \
  '<DL><p>' '<DT><A HREF="https://bar.example.test/">Bar &amp; Grill</A>' '</DL><p>' \
  '<DT><H3>Reading</H3>' '<DL><p>' '<DT><A HREF="https://read.example.test/">Read</A>' \
  '</DL><p>' '</DL><p>' > "$bookmarks"
# The check's profile keeps the address an earlier run brought in, and the import adds an address
# only once, so the run starts with none; the app isn't running, so Web Data is free.
web_data="$profile_dir/Default/Web Data"
[ ! -e "$web_data" ] || sqlite3 "$web_data" "DELETE FROM addresses; DELETE FROM address_type_tokens;" ||
  fail "the check's profile kept its addresses"
BROWSER_IMPORT_HOME="$home" BROWSER_IMPORT_PASSWORDS="$passwords" \
  BROWSER_IMPORT_BOOKMARKS="$bookmarks" BROWSER_DRIVE=1 BROWSER_URL="${site}import.html" \
  "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await_load
await '^browser: title [0-9]* import ready$' || fail "the import's page didn't draw"
send click $page_point
if [ "$active" != 1 ]; then
  exec 3>&-
  quit
  echo "skip: import. Browser wasn't the active app, so Settings would ignore the clicks."
  exit 0
fi
first=$(space)
# With the sidebar hidden, the Spaces an import makes say so in a toast over the browser window,
# as their icons in the switcher aren't seen. It comes back before the Personal Space's capture.
if [ -n "$arc" ]; then
  send key s cmd
  expect '^browser: sidebar hidden$' "⌘S didn't hide the sidebar before the import"
fi
# The toolbar's tabs, 55 wide or as wide as their names, 1 apart, centred: Import is the fourth of
# eight. Its list has a row for each browser, its Import… button at the row's end, about 62 apart;
# then a browser's choices, a row each, with Back and Import under them. The Passwords box is under
# the browsers' box, its Bookmarks row first.
if [ -n "$arc" ]; then
  found="Arc, Chrome, Firefox" chrome_row=198 firefox_row=258 bookmarks_row=328
else
  found="Chrome, Firefox" chrome_row=134 firefox_row=196 bookmarks_row=266
fi
passwords_row=$((bookmarks_row + 62))
send menu Settings…
expect '^browser: settings shown ' "Settings… didn't show Settings"
# Clicks the import's control $1, import-go for Import or import-done for Done, where the import
# logged it drawn since the last command: each step logs them anew, so a summary that wraps, as
# Chrome's does in the Mac's font, or a taller row can't move the click.
click_drawn() {
  expect "^browser: settings $1 at " "the import didn't draw $1"
  send clickkey $(sed -n "/^browser: drive $step: /,\$p" "$log" |
    sed -n "s/^browser: settings $1 at //p" | tail -1)
}
settings_drew
send clickkey $(drawn "toolbar Import")
expect '^browser: settings tab Import$' "a click on Import didn't show its tab"
settings_drew Import
expect "^browser: import found $found\$" \
  "the import didn't find the fixture's $found, and only them: $(grep 'import found' "$log")"
expect '^browser: import drawn list$' "Settings › Import didn't draw its list"
../tools/window-screenshot.sh "$pid" "$out/import-list.png" || fail "no window to capture"
if [ -n "$arc" ]; then
  send clickkey 575 134
  expect '^browser: import pick Arc$' "Import… on Arc's row didn't show what comes from it"
  expect '^browser: import drawn pick$' "Arc's choices didn't draw"
  ../tools/window-screenshot.sh "$pid" "$out/import-pick.png" || fail "no window to capture"
  click_drawn import-go
  expect '^browser: import from Arc: 2 Spaces, 6 pinned tabs, 0 open tabs, 3 favorites, 0 pages, 0 addresses$' \
    "Import didn't plan Arc's 2 Spaces, 6 pinned tabs and 3 favorites"
  expect '^browser: import from Arc: 3 link rules$' "Import didn't plan Arc's 3 link rules"
  expect '^browser: added profile [0-9]* Work$' "Arc's Work profile didn't come in"
  arc_done='2 Spaces, 6 pinned tabs, 3 favorites and 3 link rules came in from Arc'
  await "^browser: import done: $arc_done\\.\$" ||
    fail "the import didn't finish once the Work profile loaded"
  # The rules go to Arc's Spaces by their places among the user's: Personal second, Work third.
  t=$(printf '\t')
  rules="rule${t}contains${t}3${t}work.example.com
rule${t}is${t}2${t}https://news.ycombinator.com/
rule${t}is${t}3${t}https://docs.example.com/"
  [ "$(cat "$rules_file" 2>/dev/null)" = "$rules" ] ||
    fail "Arc's link rules didn't come in to its Spaces: $(cat "$rules_file" 2>/dev/null)"
  ../tools/window-screenshot.sh "$pid" "$out/import-done.png" || fail "no window to capture"
  expect "^browser: spaces $first [0-9]* [0-9]*\$" "the import didn't add Arc's two Spaces after the first"
  expect '^browser: toast shown Added 2 Spaces \[show\]$' \
    "Arc's two Spaces made with the sidebar hidden had no toast"
  spaces=$(grep '^browser: spaces' "$log" | tail -1)
  personal=$(echo "$spaces" | cut -d' ' -f4)
  work=$(echo "$spaces" | cut -d' ' -f5)
  work_profile=$(sed -n 's/^browser: added profile \([0-9]*\) Work$/\1/p' "$log" | tail -1)
  grep -q "^browser: profiled space $work $work_profile\$" "$log" ||
    fail "Arc's Work Space isn't in the Work profile"
  # Done goes back to the list, and the same import again plans nothing.
  expect '^browser: import drawn done$' "the import's end didn't draw"
  click_drawn import-done
  expect '^browser: import drawn list$' "Done didn't go back to the list"
  send clickkey 575 134
  expect '^browser: import pick Arc$' "Import… didn't show Arc's choices again"
  expect '^browser: import drawn pick$' "Arc's choices didn't draw"
  click_drawn import-go
  expect '^browser: import from Arc: 0 Spaces, 0 pinned tabs, 0 open tabs, 0 favorites, 0 pages, 0 addresses$' \
    "the second import would add something again"
  expect '^browser: import from Arc: 0 link rules$' \
    "the second import would add Arc's link rules again"
  expect '^browser: import done: Everything from Arc is here already\.$' "the second import said it added something"
  ../tools/window-screenshot.sh "$pid" "$out/import-again.png" || fail "no window to capture"
  expect '^browser: import drawn done$' "the import's end didn't draw"
  click_drawn import-done
  expect '^browser: import drawn list$' "Done didn't go back to the list"
fi
# Chrome, the row after Arc's, brings its bookmarks, open tab, history and address, which the
# core adds.
send clickkey 575 $chrome_row
expect '^browser: import pick Chrome$' "Import… on Chrome's row didn't show what comes from it"
expect '^browser: import drawn pick$' "Chrome's choices didn't draw"
click_drawn import-go
expect '^browser: import from Chrome: 1 Spaces, 3 pinned tabs, 1 open tabs, 0 favorites, 2 pages, 1 addresses$' \
  "Import didn't plan Chrome's bookmarks, open tab, history and address"
await '^browser: history added 2 to profile [0-9]*$' || fail "the core didn't add Chrome's 2 pages"
await '^browser: addresses added 1 to profile [0-9]*$' || fail "the core didn't add Chrome's address"
expect '^browser: import done: 1 Space, 3 pinned tabs, 1 open tab, 2 pages of history and 1 address came in from Chrome\.$' \
  "the import didn't say what came from Chrome"
../tools/window-screenshot.sh "$pid" "$out/import-chrome.png" || fail "no window to capture"
expect '^browser: import drawn done$' "the import's end didn't draw"
click_drawn import-done
expect '^browser: import drawn list$' "Done didn't go back to the list"
send clickkey 575 $chrome_row
expect '^browser: import pick Chrome$' "Import… didn't show Chrome's choices again"
expect '^browser: import drawn pick$' "Chrome's choices didn't draw"
click_drawn import-go
await '^browser: history added 0 to profile [0-9]*$' || fail "the core added Chrome's pages again"
await '^browser: addresses added 0 to profile [0-9]*$' || fail "the core added Chrome's address again"
expect '^browser: import done: Everything from Chrome is here already\.$' \
  "the second import from Chrome said it added something"
# Firefox, Chrome's next row, brings its toolbar's bookmark, its menu's in a folder, and its history.
expect '^browser: import drawn done$' "the import's end didn't draw"
click_drawn import-done
expect '^browser: import drawn list$' "Done didn't go back to the list"
send clickkey 575 $firefox_row
expect '^browser: import pick Firefox$' "Import… on Firefox's row didn't show what comes from it"
expect '^browser: import drawn pick$' "Firefox's choices didn't draw"
click_drawn import-go
expect '^browser: import from Firefox: 1 Spaces, 2 pinned tabs, 0 open tabs, 0 favorites, 2 pages, 0 addresses$' \
  "Import didn't plan Firefox's bookmarks and history"
await '^browser: history added 2 to profile [0-9]*$' || fail "the core didn't add Firefox's 2 pages"
expect '^browser: import done: 1 Space, 2 pinned tabs and 2 pages of history came in from Firefox\.$' \
  "the import didn't say what came from Firefox"
../tools/window-screenshot.sh "$pid" "$out/import-firefox.png" || fail "no window to capture"
expect '^browser: import drawn done$' "the import's end didn't draw"
click_drawn import-done
expect '^browser: import drawn list$' "Done didn't go back to the list"
send clickkey 575 $firefox_row
expect '^browser: import pick Firefox$' "Import… didn't show Firefox's choices again"
expect '^browser: import drawn pick$' "Firefox's choices didn't draw"
click_drawn import-go
await '^browser: history added 0 to profile [0-9]*$' || fail "the core added Firefox's pages again"
expect '^browser: import done: Everything from Firefox is here already\.$' \
  "the second import from Firefox said it added something"
# The files' box is under the browsers' box, each Choose File… at its row's end: the bookmarks
# file comes in as a Space named Bookmarks, its bar's tab loose and its folder as one, and a
# second time adds nothing.
expect '^browser: import drawn done$' "the import's end didn't draw"
click_drawn import-done
expect '^browser: import drawn list$' "Done didn't go back to the list"
send clickkey 590 $bookmarks_row
expect '^browser: import bookmarks file bookmarks\.html$' "Choose File… didn't import the bookmarks file"
await '^browser: import done: 1 Space and 2 pinned tabs came in from bookmarks\.html\.$' ||
  fail "the bookmarks file didn't come in: $(grep '^browser: import done' "$log" | tail -1)"
expect '^browser: import drawn done$' "the bookmarks file's end didn't draw"
../tools/window-screenshot.sh "$pid" "$out/import-bookmarks.png" || fail "no window to capture"
click_drawn import-done
expect '^browser: import drawn list$' "Done didn't go back to the list"
send clickkey 590 $bookmarks_row
await '^browser: import done: Everything from bookmarks\.html is here already\.$' ||
  fail "the bookmarks file came in twice: $(grep '^browser: import done' "$log" | tail -1)"
expect '^browser: import drawn done$' "the second bookmarks file's end didn't draw"
rm -f "$bookmarks"
# Then the passwords file. Done's line offers to delete it, with Delete File before Done.
click_drawn import-done
expect '^browser: import drawn list$' "Done didn't go back to the list"
send clickkey 590 $passwords_row
expect '^browser: import passwords$' "Choose File… didn't import the passwords file"
# It names the profile only where there are two, as Arc's import makes Work.
await "^browser: import done: 2 passwords came in${arc:+ to Default} from passwords\\.csv\\.\$" ||
  fail "the import didn't say the file's 2 passwords came in: $(grep '^browser: import done' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/import-passwords.png" || fail "no window to capture"
[ -e "$passwords" ] || fail "the import deleted the passwords file before the user asked"
expect '^browser: import drawn done with file$' "the import's end didn't draw with Delete File"
send clickkey 515 170
expect '^browser: import deleted the passwords file$' "Delete File didn't delete the passwords file"
expect '^browser: import drawn done$' "the import's end didn't draw again without Delete File"
[ ! -e "$passwords" ] || fail "the passwords file is still there after Delete File"
# A second file: the login's password changed, which keeps the saved one, and a new login.
printf '%s\n' 'url,username,password' \
  'https://login.example.test/,check@example.test,made-up-changed' \
  'https://new.example.test/,new@example.test,made-up-three' > "$passwords"
click_drawn import-done
expect '^browser: import drawn list$' "Done didn't go back to the list"
send clickkey 590 $passwords_row
expect '^browser: import passwords$' "Choose File… didn't import the second passwords file"
await "^browser: import done: 1 password came in${arc:+ to Default} from passwords\.csv\. 1 conflicted with a saved password, which was kept\.\$" ||
  fail "the import didn't say 1 came in and 1 conflicted: $(grep '^browser: import done' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/import-passwords-conflict.png" || fail "no window to capture"
rm -f "$passwords"
# Passwords, among Settings' tabs, lists the three logins, not their passwords.
send clickkey $(drawn "toolbar Passwords")
expect '^browser: settings tab Passwords$' "a click on Passwords didn't show its tab"
settings_drew Passwords
await '^browser: passwords [0-9]*: login\.example\.test check@example\.test, mail\.example\.test check@example\.test, new\.example\.test new@example\.test$' ||
  fail "Settings didn't list the imported logins: $(grep '^browser: passwords [0-9]*:' "$log" | tail -1)"
send menu Import from Other Browsers…
expect '^browser: settings tab Import$' "Import from Other Browsers… didn't show Settings' Import tab"
settings_drew Import
send key w cmd
expect '^browser: settings hidden$' "⌘W didn't hide Settings"
if [ -n "$arc" ]; then
  send key s cmd
  expect '^browser: sidebar shown$' "⌘S didn't bring the sidebar back after the import"
  # The Personal Space shows its pinned tabs: HN, then Reading with a, b and c and Deep with d.
  send key 2 ctrl
  expect "^browser: space $personal in " "⌃2 didn't show Arc's Personal Space"
  expect '^browser: pinned [0-9]*\( [0-9]*\)\{6\}$' \
    "Personal's pinned tabs and folders aren't the 7 rows Arc's are: $(pinned)"
  ../tools/window-screenshot.sh "$pid" "$out/import-personal.png" || fail "no window to capture"
fi
quit
[ -n "$arc" ] || echo "skip: import's Arc steps. Arc has no Linux build, so it's never found there."
echo "ok: Settings › Import finds the fixture's Arc, Chrome and Firefox. From Arc come its Spaces,"
echo "  one in a new profile, their pinned tabs and folders, and its favorites; from Chrome and"
echo "  Firefox their bookmarks as a Space and their history, with Chrome's open tab and address; and"
echo "  a second import of each adds nothing."
echo "  A bookmarks file comes in as a Space named Bookmarks, and a second time adds nothing."
echo "  A passwords file comes in, says how many, and is deleted when asked; a second one says its"
echo "  changed login conflicted and kept the saved one; Settings' Passwords tab lists the three,"
echo "  and the app menu's Import from Other Browsers… shows Import again."
echo "  With the sidebar hidden, Arc's two Spaces say so in a toast."
echo "  Windows: $out/import-list.png, $out/import-pick.png, $out/import-done.png,"
echo "  $out/import-again.png, $out/import-chrome.png, $out/import-firefox.png,"
echo "  $out/import-bookmarks.png, $out/import-passwords.png,"
echo "  $out/import-passwords-conflict.png, $out/import-personal.png"
