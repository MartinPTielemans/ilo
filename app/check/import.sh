# Settings › Import finds the made-up Arc in src/import/fixture and a made-up Chrome and Firefox
# beside it, never the machine's own browsers. From Arc it brings its two Spaces, one in a new profile
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
# the sidebar hidden, Arc's Spaces say so in a toast. While an import runs, each Space it makes is
# a card under its profile, captured waiting (Arc's, as Work's profile loads, recorded on the Mac)
# and ticked (Chrome's, as its history goes in). Each step is captured, and the list and the
# first browser's choices, read as it's picked and each kind counted, under Import › its name with
# Import in the accent, at Settings' first size and its least, dark and light; Esc goes back from
# the choices to the list. The fixture's Arc is put where the Mac keeps it, or Windows' Store, and
# Chrome and Firefox where each system keeps them. Arc has no Linux build, so on Linux it isn't
# found and its steps are left out.

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
arc_dir="$home/Library/Application Support/Arc"
if [ "$(uname)" = Darwin ]; then
  arc=1
  chrome="$home/Library/Application Support/Google/Chrome"
  firefox="$home/Library/Application Support/Firefox"
elif windows; then
  # Arc on Windows is the Store's, in its package's folder (sources.rs).
  arc=1
  store="$home/AppData/Local/Packages/TheBrowserCompany.Arc_ttt1ap7aakyb4/LocalCache/Local"
  mkdir -p "$store"
  mv "$arc_dir" "$store/Arc"
  arc_dir="$store/Arc"
  chrome="$home/AppData/Local/Google/Chrome/User Data"
  firefox="$home/AppData/Roaming/Mozilla/Firefox"
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
# Runs the SQL $2 on the database $1: with the sqlite3 command where there is one, else with
# Python's own sqlite3 module, as Windows' Git Bash has Python but no sqlite3 (#1724).
sql() {
  if command -v sqlite3 > /dev/null; then
    sqlite3 "$1" "$2"
  else
    python3 -c 'import sqlite3, sys
db = sqlite3.connect(sys.argv[1])
db.executescript(sys.argv[2])
db.commit()' "$1" "$2"
  fi
}
# Chromium counts microseconds from 1601: an hour and two hours ago.
now=$(( ($(date +%s) + 11644473600) * 1000000 ))
sql "$chrome/Default/History" "
  CREATE TABLE urls(id INTEGER PRIMARY KEY AUTOINCREMENT, url LONGVARCHAR, title LONGVARCHAR,
    visit_count INTEGER DEFAULT 0 NOT NULL, typed_count INTEGER DEFAULT 0 NOT NULL,
    last_visit_time INTEGER NOT NULL, hidden INTEGER DEFAULT 0 NOT NULL);
  INSERT INTO urls(url, title, visit_count, typed_count, last_visit_time) VALUES
    ('https://history-one.example.org/', 'One', 3, 1, $now - 3600000000),
    ('https://history-two.example.org/', 'Two', 1, 0, $now - 7200000000);"
# Chrome's saved address, field by field as Chromium's FieldType numbers (full name, city, country)
# with each typed by the user, and a made-up card beside it, which the import never reads.
sql "$chrome/Default/Web Data" "
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
sql "$firefox/Profiles/x.default-release/places.sqlite" "
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
touch "$arc_dir/StorableSidebar.json"
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
[ ! -e "$web_data" ] || sql "$web_data" "DELETE FROM addresses; DELETE FROM address_type_tokens;" ||
  fail "the check's profile kept its addresses"
# The app on Windows takes Windows' paths, and starts in a folder of its own.
BROWSER_IMPORT_HOME="$(native_path "$PWD/$home")" \
  BROWSER_IMPORT_PASSWORDS="$(native_path "$PWD/$passwords")" \
  BROWSER_IMPORT_BOOKMARKS="$(native_path "$PWD/$bookmarks")" BROWSER_DRIVE=1 BROWSER_URL="${site}import.html" \
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
# Import & Export's list has a row for each browser, its Import… button at the row's end; then a
# browser's choices, a row each, with Back and Import under them. The files' box is under the
# browsers' box. Each button is clicked where the import logs it drawn.
if [ -n "$arc" ]; then found="Arc, Chrome, Firefox"; else found="Chrome, Firefox"; fi
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
send clickkey $(drawn "toolbar Import & Export")
expect '^browser: settings tab Import & Export$' \
  "a click on Import & Export didn't show its section"
settings_drew 'Import & Export'
expect "^browser: import found $found\$" \
  "the import didn't find the fixture's $found, and only them: $(grep 'import found' "$log")"
expect '^browser: import drawn list$' "Settings › Import & Export didn't draw its list"
../tools/window-screenshot.sh "$pid" "$out/import-list.png" || fail "no window to capture"
# Settings at its first size and at its least, in its look and then the other, as
# $out/import-settings-$1-<width>-<dark|light>.png, and back at its first size in its own look.
settings_tone() {
  case "$(grep '^browser: settings look ' "$log" | tail -1)" in
    *light) echo light ;;
    *) echo dark ;;
  esac
}
settings_at() {
  for _ in $(seq 100); do
    [ "$(grep '^browser: settings drawn at ' "$log" | tail -1)" = "browser: settings drawn at $1x$2" ] &&
      return 0
    sleep 0.1
  done
  fail "Settings didn't draw at $1 by $2: $(grep '^browser: settings drawn at ' "$log" | tail -1)"
}
settings_shots() {
  for size in "820 660" "720 480"; do
    send resizekey $size
    settings_at $size
    was=$(settings_tone)
    ../tools/window-screenshot.sh "$pid" "$out/import-settings-$1-${size% *}-$was.png" ||
      fail "no window to capture"
    if [ "$was" = light ]; then other=Dark; else other=Light; fi
    send menu "$(cased "Appearance › $other")"
    expect "^browser: settings look $(echo "$other" | tr DL dl)\$" \
      "View › Appearance › $other didn't change Settings' look"
    ../tools/window-screenshot.sh "$pid" "$out/import-settings-$1-${size% *}-$(settings_tone).png" ||
      fail "no window to capture"
    send menu "$(cased 'Appearance › Automatic')"
    expect "^browser: settings look $was\$" \
      "View › Appearance › Automatic didn't give Settings its look back"
  done
  send resizekey 820 660
  settings_at 820 660
}
settings_shots list
# The first browser's choices, under the heading Import › its name, with Import in the accent,
# read as it's picked: each kind with how much of it comes, Arc's Spaces by name, the passwords'
# row and the profiles it goes into. Esc goes back to the list.
if [ -n "$arc" ]; then
  first_found=Arc
  counts='spaces: 2, pinned: 6, folders: 2, link_rules: 3, favorites: 3, bookmarks: 0, bookmark_folders: 0, open_tabs: 0, reading_list: 0, pages: 0, addresses: 0, extensions: 0'
else
  first_found=Chrome
  counts='spaces: 0, pinned: 0, folders: 0, link_rules: 0, favorites: 0, bookmarks: 3, bookmark_folders: 1, open_tabs: 2, reading_list: 0, pages: 2, addresses: 1, extensions: 0'
fi
send clickkey $(drawn "import-from $first_found")
expect "^browser: import pick $first_found\$" "Import… on $first_found's row didn't show its choices"
expect '^browser: import drawn pick$' "$first_found's choices didn't draw"
expect "^browser: import read $first_found: Tally { $counts }\$" \
  "picking $first_found didn't read it: $(grep '^browser: import read' "$log" | tail -1)"
settings_shots choose
send key escape
expect '^browser: import escape$' "Esc in $first_found's choices didn't reach the import"
expect '^browser: import drawn list$' "Esc didn't go back from $first_found's choices to the list"
if [ -n "$arc" ]; then
  send clickkey $(drawn "import-from Arc")
  expect '^browser: import pick Arc$' "Import… on Arc's row didn't show what comes from it"
  expect '^browser: import drawn pick$' "Arc's choices didn't draw"
  ../tools/window-screenshot.sh "$pid" "$out/import-pick.png" || fail "no window to capture"
  # The arrival, recorded on the Mac: a card a Space as the core makes it, under its profile, each
  # waiting while Work's profile loads, then ticking as its tabs go in.
  record import-arrival 2
  click_drawn import-go
  expect '^browser: import cards Personal, Work$' "Arc's Spaces didn't come as cards, Personal then Work"
  # Captured waiting, or at the end where Work's profile came before the window's next frame, as
  # on Windows' runner.
  expect '^browser: import \(drawn running\|done: \)' "Arc's cards didn't draw"
  ../tools/window-screenshot.sh "$pid" "$out/import-arriving.png" || fail "no window to capture"
  expect '^browser: import from Arc: 2 Spaces, 6 pinned tabs, 0 open tabs, 3 favorites, 0 pages, 0 addresses$' \
    "Import didn't plan Arc's 2 Spaces, 6 pinned tabs and 3 favorites"
  expect '^browser: import from Arc: 3 link rules$' "Import didn't plan Arc's 3 link rules"
  expect '^browser: added profile [0-9]* Work$' "Arc's Work profile didn't come in"
  arc_done='2 Spaces, 6 pinned tabs, 3 favorites and 3 link rules came in from Arc'
  await "^browser: import done: $arc_done\\.\$" ||
    fail "the import didn't finish once the Work profile loaded"
  recorded
  # The rules go to Arc's Spaces by their places among the user's: Personal second, Work third.
  t=$(printf '\t')
  # After the Meet rule a profile starts with (#692).
  rules="rule${t}contains${t}front${t}meet.google.com
rule${t}contains${t}3${t}work.example.com
rule${t}is${t}2${t}https://news.ycombinator.com/
rule${t}is${t}3${t}https://docs.example.com/
seeded${t}meet"
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
  send clickkey $(drawn "import-from Arc")
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
send clickkey $(drawn "import-from Chrome")
expect '^browser: import pick Chrome$' "Import… on Chrome's row didn't show what comes from it"
expect '^browser: import drawn pick$' "Chrome's choices didn't draw"
click_drawn import-go
expect '^browser: import from Chrome: 1 Spaces, 3 pinned tabs, 1 open tabs, 0 favorites, 2 pages, 1 addresses$' \
  "Import didn't plan Chrome's bookmarks, open tab, history and address"
# Its card ticks once its tabs are in, as the core adds the history after: captured then, or at
# the end if the core was quicker than the frame.
expect '^browser: import cards Chrome$' "Chrome's Space didn't come as a card"
expect '^browser: import \(drawn placed\|done: \)' "Chrome's tabs didn't go in"
../tools/window-screenshot.sh "$pid" "$out/import-placed.png" || fail "no window to capture"
await '^browser: history added 2 to profile [0-9]*$' || fail "the core didn't add Chrome's 2 pages"
await '^browser: addresses added 1 to profile [0-9]*$' || fail "the core didn't add Chrome's address"
expect '^browser: import done: 1 Space, 3 pinned tabs, 1 open tab, 2 pages of history and 1 address came in from Chrome\.$' \
  "the import didn't say what came from Chrome"
../tools/window-screenshot.sh "$pid" "$out/import-chrome.png" || fail "no window to capture"
expect '^browser: import drawn done$' "the import's end didn't draw"
click_drawn import-done
expect '^browser: import drawn list$' "Done didn't go back to the list"
send clickkey $(drawn "import-from Chrome")
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
send clickkey $(drawn "import-from Firefox")
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
send clickkey $(drawn "import-from Firefox")
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
send clickkey $(drawn import-bookmarks)
expect '^browser: import bookmarks file bookmarks\.html$' "Choose File… didn't import the bookmarks file"
await '^browser: import done: 1 Space and 2 pinned tabs came in from bookmarks\.html\.$' ||
  fail "the bookmarks file didn't come in: $(grep '^browser: import done' "$log" | tail -1)"
expect '^browser: import drawn done$' "the bookmarks file's end didn't draw"
../tools/window-screenshot.sh "$pid" "$out/import-bookmarks.png" || fail "no window to capture"
click_drawn import-done
expect '^browser: import drawn list$' "Done didn't go back to the list"
send clickkey $(drawn import-bookmarks)
await '^browser: import done: Everything from bookmarks\.html is here already\.$' ||
  fail "the bookmarks file came in twice: $(grep '^browser: import done' "$log" | tail -1)"
expect '^browser: import drawn done$' "the second bookmarks file's end didn't draw"
rm -f "$bookmarks"
# Then the passwords file. Done's line offers to delete it, with Delete File before Done.
click_drawn import-done
expect '^browser: import drawn list$' "Done didn't go back to the list"
send clickkey $(drawn import-passwords)
expect '^browser: import passwords$' "Choose File… didn't import the passwords file"
# It names the profile only where there are two, as Arc's import makes Work.
await "^browser: import done: 2 passwords came in${arc:+ to Default} from passwords\\.csv\\.\$" ||
  fail "the import didn't say the file's 2 passwords came in: $(grep '^browser: import done' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/import-passwords.png" || fail "no window to capture"
[ -e "$passwords" ] || fail "the import deleted the passwords file before the user asked"
expect '^browser: import drawn done with file$' "the import's end didn't draw with Delete File"
send clickkey $(drawn import-delete)
expect '^browser: import deleted the passwords file$' "Delete File didn't delete the passwords file"
expect '^browser: import drawn done$' "the import's end didn't draw again without Delete File"
[ ! -e "$passwords" ] || fail "the passwords file is still there after Delete File"
# A second file: the login's password changed, which keeps the saved one, and a new login.
printf '%s\n' 'url,username,password' \
  'https://login.example.test/,check@example.test,made-up-changed' \
  'https://new.example.test/,new@example.test,made-up-three' > "$passwords"
click_drawn import-done
expect '^browser: import drawn list$' "Done didn't go back to the list"
send clickkey $(drawn import-passwords)
expect '^browser: import passwords$' "Choose File… didn't import the second passwords file"
await "^browser: import done: 1 password came in${arc:+ to Default} from passwords\.csv\. 1 conflicted with a saved password, which was kept\.\$" ||
  fail "the import didn't say 1 came in and 1 conflicted: $(grep '^browser: import done' "$log" | tail -1)"
../tools/window-screenshot.sh "$pid" "$out/import-passwords-conflict.png" || fail "no window to capture"
rm -f "$passwords"
# Passwords, among Settings' sections, lists the three logins, not their passwords.
send clickkey $(drawn "toolbar Passwords")
expect '^browser: settings tab Passwords$' "a click on Passwords didn't show its section"
settings_drew Passwords
await '^browser: passwords [0-9]*: login\.example\.test check@example\.test, mail\.example\.test check@example\.test, new\.example\.test new@example\.test$' ||
  fail "Settings didn't list the imported logins: $(grep '^browser: passwords [0-9]*:' "$log" | tail -1)"
send menu Import from Other Browsers…
expect '^browser: settings tab Import & Export$' \
  "Import from Other Browsers… didn't show Settings' Import & Export"
settings_drew 'Import & Export'
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
echo "  Settings' list and $first_found's choices, under Import › $first_found, at 820 by 660 and"
echo "  720 by 480, dark and light, and Esc goes back from the choices to the list."
echo "  Windows: $out/import-settings-{list,choose}-{820,720}-{dark,light}.png,"
echo "  $out/import-arriving.png (with Arc), $out/import-placed.png,"
echo "  $out/import-list.png, $out/import-pick.png, $out/import-done.png,"
echo "  $out/import-again.png, $out/import-chrome.png, $out/import-firefox.png,"
echo "  $out/import-bookmarks.png, $out/import-passwords.png,"
echo "  $out/import-passwords-conflict.png, $out/import-personal.png"
