# check.sh runs this at 360.
# A new profile's first launch plays the onboarding in a window of its own, the only one on screen
# until it ends, captured at each step in a light and a dark appearance: the intro, the welcome,
# waits, where Return goes on and Skip Setup skips the setup (#4093); the import step lists the
# made-up Arc in src/import/fixture (a made-up Chrome on Linux), never the Mac's own browsers, with
# what comes from it as chips, which a click leaves out and another brings back, and ↩ there
# imports it at once; Skip goes on from a step, where Esc is Back, the colour step's Return
# gives the first Space the swatch picked, and the last step, the keys, has no Skip: its Return
# leaves the flow for the first Space's welcome with the command bar up and a line under it. On the Mac, Transparency's step comes after
# the colour's, as it shows only over the window's glass. The default browser's step shows only while
# ilo isn't the default, and is skipped everywhere but a Linux CI VM, where its button makes ilo
# the default for a moment, so the run never changes a desk's. The search engine's comes after it
# with the profile's list: Skip sets none, and the dark pass's ↓ and Return set the engine after the
# default, as Settings' pop-up does. The opt-ins' shows the daily count's switch, off, as the run
# names an endpoint, and no agents' until outside apps' have a setting of their own (#4260); the
# dark pass turns the count's on and off again from the keyboard. After the colour come the tidy
# step, whose bar Skip leaves and the dark pass's → and Return set a stop on, and the private
# step, whose ad blocking the dark pass turns off and on again from the keyboard. The account's
# comes between them and the opt-ins while
# the build names a sync server (branding/sync-server), and Skip leaves it signed out: the run never
# signs in to the hosted server, as sync-account signs in on a server of its own. The assistant's
# comes right after the welcome, and Skip leaves it with no assistant signed in. Help › Welcome to
# ilo plays it again in first run's window, whether the browser's shows a page or not. A
# launch that brings Spaces back never starts it. Every other run sets BROWSER_ONBOARDING=skip
# (check.sh); this one sets the appearance instead.

serve
# The import step reads the fixture's home, set for each launch so the runs after keep their own.
# Arc has no Linux build, so there the home has a made-up Chrome where Linux keeps it, and on
# Windows where Windows does.
fixture="$PWD/src/import/fixture"
import_home=$fixture
found=Arc
if [ "$(uname)" != Darwin ]; then
  # From the root, as chrome.exe moves into its own folder as it starts.
  fixture="$PWD/$out/onboarding-home"
  import_home=$fixture
  found=Chrome
  chrome="$fixture/.config/google-chrome"
  if windows; then
    chrome="$fixture/AppData/Local/Google/Chrome/User Data"
    import_home=$(cygpath -w "$fixture")
  fi
  rm -rf "$fixture"
  mkdir -p "$chrome/Default"
  printf '%s' '{"profile":{"info_cache":{"Default":{"name":"Me"}}}}' > "$chrome/Local State"
  printf '%s' '{}' > "$chrome/Default/Preferences"
  # The import lists a browser only once it has bookmarks or history to bring in.
  printf '%s' '{"roots":{}}' > "$chrome/Default/Bookmarks"
fi
# Captures the window as onboarding-$1.png.
capture_step() {
  ../tools/window-screenshot.sh "$pid" "$out/onboarding-$1.png" || fail "no window to capture"
}
# The import's list says what comes from the browser used last as chips: a click on History leaves
# it out and another brings it back, keeping the step. Captured as onboarding-$1-import-chips.png
# with History left out.
import_choices() {
  local at
  at=$(drawn "import-chip History") || fail "$1: the import's list drew no History chip"
  send click $at
  expect '^browser: import chip History false$' "$1: a click on History didn't leave it out"
  send state
  capture_step "$1-import-chips"
  send click $at
  expect '^browser: import chip History true$' "$1: a second click on History didn't bring it back"
  none_since '^browser: onboarding left import' || fail "$1: a click on a chip left the import step"
}
# The account's step, while the build names a server to sign in to.
account=
[ ! -s branding/sync-server ] || account="account "
# Transparency's step, only over the window's glass, which only the Mac has.
glass=
[ "$(uname)" != Darwin ] || glass="transparency "
# Launches a new profile, whose saved Spaces are gone and which isn't signed in, in appearance $1,
# and reads the steps it shows into $steps.
launch_new() {
  log="$out/onboarding-$1.log"
  step=0
  rm -f "$saved" "$profile_dir/Account"
  # A count endpoint nothing listens on, so the opt-ins offer the count's switch, as the prod
  # build's do, while the switch stays off and nothing is sent (count.sh turns it on).
  BROWSER_IMPORT_HOME="$import_home" BROWSER_DRIVE=1 BROWSER_ONBOARDING=$1 \
    "$browser" --daily-count-url=http://127.0.0.1:9/v1/daily < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await '^browser: onboarding step intro settled$' || fail "$1: the intro didn't draw"
  steps=$(grep -m1 '^browser: onboarding steps ' "$log" | cut -d' ' -f4-)
  case "$steps" in
    "intro assistant import "*"color ${glass}tidy private ${account}opt-ins keys") ;;
    *) fail "$1: the steps were '$steps', not the intro, the assistant, the import, the rest, the color, ${glass}the tidy and private steps, ${account}the opt-ins, the keys" ;;
  esac
  # BROWSER_ONBOARDING gives the sheet its look but leaves the window's material the system's, so
  # on a system of the other look the captures would show a sheet on glass no user can reach.
  # ilo's own Appearance, picked as a user picks it, gives the window the same look (#4093).
  look=Light
  [ "$1" = light ] || look=Dark
  send menu "$(cased "Appearance › $look")"
  expect "^browser: appearance $1\$" "$1: View › Appearance › $look didn't set ilo's look"
}

# Light: the welcome waits once drawn, Return goes on from it, and Skip from every step after.
launch_new light
grep -q '^browser: onboarding started$' "$log" || fail "a new profile didn't start the onboarding"
capture_step light-intro
if [ "$(uname)" = Darwin ]; then
  # A reopen, which a click on the Dock's icon sends, brings first run's window to the front and
  # makes no window: the browser's stays held and Chromium makes none of its own.
  send reopen
  expect '^browser: reopen brought first run to the front$' "light: a reopen didn't come to first run"
  send front
  grep '^browser: front ' "$log" | tail -1 | grep -q '^browser: front GPUIWindow .*, key GPUIWindow ' ||
    fail "light: a reopen put a window in front of first run's: $(grep '^browser: front ' "$log" | tail -1)"
  ! grep -q '^browser: window on screen after first run$' "$log" ||
    fail "light: a reopen showed the browser's window while first run showed"
  # A link from another app opens in a Little window over first run's, as an assistant's sign-in
  # page comes. Closing it gives first run's window the keyboard back at the welcome, which Return
  # goes on from below, and the browser's window stays held.
  open -a "$app" "${site}index.html"
  little_link="^browser: link from another app ${site}index.html: tab [1-9][0-9]* in little window [0-9]* of space [0-9]*\$"
  expect "$little_link" "light: a link while first run showed didn't open in a Little window"
  tab=$(grep "$little_link" "$log" | tail -1 | sed 's/.*: tab \([0-9]*\) .*/\1/')
  little=$(grep "$little_link" "$log" | tail -1 | sed 's/.* little window \([0-9]*\) .*/\1/')
  expect "^browser: loaded $tab ${site}index.html\$" "light: the link didn't load in its Little window"
  send key w cmd
  expect "^browser: window closed $little\$" "light: ⌘W didn't close the link's Little window"
  # As AppKit may hand the keyboard on a moment after.
  for _ in $(seq 20); do
    send front
    case "$(grep '^browser: front ' "$log" | tail -1)" in *", key GPUIWindow "*) break ;; esac
  done
  grep '^browser: front ' "$log" | tail -1 | grep -q ', key GPUIWindow ' ||
    fail "light: first run's window didn't get the keyboard back: $(grep '^browser: front ' "$log" | tail -1)"
  ! grep -q '^browser: onboarding left ' "$log" ||
    fail "light: the link's Little window took first run off its welcome"
  ! grep -q '^browser: window on screen after first run$' "$log" ||
    fail "light: a link showed the browser's window while first run showed"
fi
send key return
expect '^browser: onboarding left intro via return$' "light: Return didn't go on from the welcome"
for each in $steps; do
  [ "$each" = intro ] && continue
  expect "^browser: onboarding step $each settled\$" "light: the $each step didn't come"
  [ "$each" != import ] || grep -q "^browser: import found $found\$" "$log" ||
    fail "light: the import step didn't find the fixture's $found"
  capture_step "light-$each"
  [ "$each" != import ] || import_choices light
  # The keys' step has no Skip: its Return is the way in.
  if [ "$each" = keys ]; then
    send key return
    expect '^browser: onboarding left keys via return$' "light: Return didn't leave the keys' step"
    continue
  fi
  skip_step "$each"
  expect "^browser: onboarding left $each via skip\$" "light: Skip didn't leave the $each step"
done
expect '^browser: onboarding done$' "light: the flow didn't end after its last step"
# First run is the only window until it ends, when the browser's, held off screen, comes on.
grep -q '^browser: window held for first run$' "$log" ||
  fail "light: the browser's window came on screen while first run showed"
# The Mac's Chromium puts the window on screen as it makes it, so the host takes it off again.
[ "$(uname)" != Darwin ] || grep -q '^browser: window held off screen$' "$log" ||
  fail "light: the Mac left the browser's window on screen while first run showed"
expect '^browser: window on screen after first run$' \
  "light: the browser's window didn't come on screen as first run ended"
grep -q '^browser: onboarding picked color ' "$log" && fail "light: Skip at the colour step gave the Space one"
grep -q '^browser: profile [0-9]* searches with ' "$log" && fail "light: Skip at the search engine step set one"
grep -q '^browser: onboarding tidy set ' "$log" && fail "light: Skip at the tidy step set its time"
expect '^browser: command new-tab via onboarding$' "light: the flow didn't end with the command bar"
expect '^browser: command bar shown$' "light: the command bar didn't come up after the flow"
capture_step light-in
exec 3>&-
quit

# A launch that brings the Spaces back, as the light one left them, never starts it, even with
# the run not skipping it.
log="$out/onboarding-relaunch.log"
step=0
BROWSER_DRIVE=1 BROWSER_ONBOARDING=light "$browser" < "$out/drive" \
  > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: restored in ' || fail "the relaunch didn't bring the Spaces back"
await '^browser: UI shown at ' || fail "the relaunch didn't show its UI"
grep -q '^browser: onboarding' "$log" &&
  fail "a launch that brought Spaces back started the onboarding"
exec 3>&-
quit

# Dark: Return goes on from the welcome, Skip in the sheet's foot leaves the import, and
# the colour step's → and Return give the first Space the first swatch.
# With the system's animations off the app reduces its motion, read at launch, and follows them
# back on. Only on a CI VM, as the setting is the desk's: on Windows "Show animations in Windows",
# and on Linux GTK's settings file, the last word where, as in the CI session, no portal answers.
# Nothing but System Settings changes the Mac's.
reduced=
if windows && [ -n "${GITHUB_ACTIONS:-}" ]; then
  windows_animations 0 || fail "PowerShell didn't turn Windows' animations off"
  reduced=windows
elif [ "$(uname)" = Linux ] && [ -n "${GITHUB_ACTIONS:-}" ]; then
  gtk="${XDG_CONFIG_HOME:-$HOME/.config}/gtk-4.0/settings.ini"
  rm -f "$out/gtk-settings.ini"
  [ ! -f "$gtk" ] || cp "$gtk" "$out/gtk-settings.ini"
  mkdir -p "$(dirname "$gtk")"
  printf '[Settings]\ngtk-enable-animations=0\n' > "$gtk"
  reduced=linux
fi
launch_new dark
case $reduced in
  windows)
    await '^browser: reduce motion on$' ||
      fail "dark: with Windows' animations off, the app didn't reduce its motion"
    ;;
  linux)
    await '^browser: reduce motion on$' ||
      fail "dark: with GTK's animations off, the app didn't reduce its motion"
    if [ -f "$out/gtk-settings.ini" ]; then
      mv "$out/gtk-settings.ini" "$gtk"
    else
      rm -f "$gtk"
    fi
    ;;
esac
capture_step dark-intro
send key return
expect '^browser: onboarding left intro via return$' "dark: Return didn't go on from the welcome"
# Back on once the still welcome is left.
if [ "$reduced" = windows ]; then
  windows_animations 1 || fail "PowerShell didn't turn Windows' animations back on"
  expect '^browser: reduce motion off$' \
    "dark: Windows' animations turned back on didn't bring the app's motion back"
fi
expect '^browser: onboarding step assistant settled$' "dark: the assistant's step didn't come after the welcome"
capture_step dark-assistant
skip_step assistant
expect '^browser: onboarding left assistant via skip$' "dark: Skip didn't leave the assistant's step"
expect '^browser: onboarding step import settled$' "dark: the import step didn't come"
capture_step dark-import
import_choices dark
# The sheet's Skip takes a click where the app last said it's drawn, as it logs where once the step
# has come, before the choices above, and again only if it moves (#4093).
await '^browser: onboarding skip at ' || fail "dark: the import step didn't draw its Skip"
send click $(grep '^browser: onboarding skip at ' "$log" | tail -1 | cut -d' ' -f5-6)
expect '^browser: onboarding left import via skip$' \
  "dark: a click on Skip didn't leave the import step"
case " $steps " in
  *" default-browser "*)
    expect '^browser: onboarding step default-browser settled$' "dark: the default browser's step didn't come"
    capture_step dark-default-browser
    # Only a Linux CI VM, where no one browses, lets Return make ilo the default: xdg-settings
    # reads it back, and the VM's own lists of defaults come back after, for the replay's steps.
    # xdg-settings takes a desktop entry only for a program it can run, so this one runs the build.
    if [ "$(uname)" = Linux ] && [ -n "${GITHUB_ACTIONS:-}" ] && command -v xdg-settings > /dev/null
    then
      apps="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
      lists="${XDG_CONFIG_HOME:-$HOME/.config}/mimeapps.list $apps/mimeapps.list"
      at=0
      for list in $lists; do
        at=$((at + 1))
        rm -f "$out/mimeapps-$at.list"
        [ ! -f "$list" ] || cp "$list" "$out/mimeapps-$at.list"
      done
      entry="$apps/$(cat branding/id).desktop"
      mkdir -p "$apps"
      # With the types xdg-settings sets already listed: it adds each one missing to the entry and
      # sleeps 4 s after, which put http's past the wait below on a quick runner.
      printf '[Desktop Entry]\nType=Application\nName=%s\nExec=%s %%U\nMimeType=%s\n' \
        "$app_name" "$browser" \
        'text/html;x-scheme-handler/http;x-scheme-handler/https;x-scheme-handler/about;x-scheme-handler/unknown;' \
        > "$entry"
      send key return
      expect '^browser: onboarding left default-browser via return$' \
        "dark: Return didn't leave the default browser's step"
      for _ in $(seq 50); do
        made=$(xdg-settings get default-web-browser 2> /dev/null || true)
        [ "$made" = "${entry##*/}" ] && break
        sleep 0.1
      done
      [ "$made" = "${entry##*/}" ] ||
        fail "dark: the default browser's button left the default '$made', not ${entry##*/}"
      # xdg-settings sets each type with its own xdg-mime, each reading the lists and writing them
      # back: one still going once http reads as ilo's writes ilo back over the lists put back
      # below, and the replay found ilo the default (#2773). On the VM only the app's runs it.
      for _ in $(seq 100); do
        pgrep -x xdg-settings > /dev/null || break
        sleep 0.1
      done
      ! pgrep -x xdg-settings > /dev/null ||
        fail "dark: the app's xdg-settings was still setting the default 10 s after it read as ilo"
      at=0
      for list in $lists; do
        at=$((at + 1))
        if [ -f "$out/mimeapps-$at.list" ]; then
          mv "$out/mimeapps-$at.list" "$list"
        else
          rm -f "$list"
        fi
      done
      rm -f "$entry"
    else
      skip_step default-browser
      expect '^browser: onboarding left default-browser via skip$' \
        "dark: Skip didn't leave the default browser's step"
      [ "$(uname)" != Linux ] || [ -z "${GITHUB_ACTIONS:-}" ] ||
        echo "skip: onboarding's default browser button. The VM has no xdg-settings."
    fi
    ;;
esac
case " $steps " in
  *" search-engine "*)
    expect '^browser: onboarding step search-engine settled$' "dark: the search engine step didn't come"
    capture_step dark-search-engine
    send key down
    send key return
    expect '^browser: profile [0-9]* searches with ' "dark: ↓ and Return didn't set the engine after the default"
    expect '^browser: onboarding left search-engine via return$' "dark: Return didn't leave the search engine step"
    ;;
esac
expect '^browser: onboarding step color settled$' "dark: the colour step didn't come"
send key right
capture_step dark-color
send key return
expect '^browser: onboarding picked color f0eae5$' "dark: → and Return didn't pick the first swatch"
expect '^browser: onboarding left color via return$' "dark: Return didn't leave the colour step"
if [ -n "$glass" ]; then
  expect '^browser: onboarding step transparency settled$' "dark: Transparency's step didn't come after the colour's"
  capture_step dark-transparency
  skip_step transparency
  expect '^browser: onboarding left transparency via skip$' "dark: Skip didn't leave Transparency's step"
fi
# The bar rests at a new profile's 12 hours; → moves it a stop, which Return sets.
expect '^browser: onboarding step tidy settled$' "dark: the tidy step didn't come"
capture_step dark-tidy
send key right
expect '^browser: onboarding tidy at 1 day$' "dark: → didn't move the tidy step's bar from 12 hours"
send key return
expect '^browser: onboarding tidy set 1 day$' "dark: Return didn't set the tidy step's time"
expect '^browser: onboarding left tidy via return$' "dark: Return didn't leave the tidy step"
# From Continue, where it rests, ⇥ round to ad blocking's switch, which Space turns off and on.
expect '^browser: onboarding step private settled$' "dark: the private step didn't come"
capture_step dark-private
expect '^browser: onboarding ad-blocking at ' "dark: the private step didn't draw ad blocking's switch"
send key tab
send key space
expect '^browser: onboarding ad-blocking off$' "dark: ⇥ then Space didn't turn ad blocking off"
send key space
expect '^browser: onboarding ad-blocking on$' "dark: Space again didn't turn ad blocking back on"
skip_step private
expect '^browser: onboarding left private via skip$' "dark: Skip didn't leave the private step"
if [ -n "$account" ]; then
  expect '^browser: onboarding step account settled$' "dark: the account step didn't come"
  capture_step dark-account
  skip_step account
  expect '^browser: onboarding left account via skip$' "dark: Skip didn't leave the account step"
  ! grep -q '^browser: onboarding sign in via ' "$log" || fail "dark: Skip at the account step signed in"
fi
expect '^browser: onboarding step opt-ins settled$' "dark: the opt-ins step didn't come"
capture_step dark-opt-ins
expect '^browser: onboarding daily-count at ' \
  "dark: the opt-ins step didn't draw the count's switch, though the run names an endpoint"
# The keyboard reaches the switches: from Continue, the last of the sheet's foot, where the step
# rests (#3498), ⇥ round to the count's, the first, where Space turns it on, ⇥ on and ⇧⇥ back to
# it, and Space turns it off again, so Skip still leaves it as it was (#2985).
send key tab
send key space
expect '^browser: onboarding daily-count on$' \
  "dark: ⇥ from Continue, round to the first switch, then Space didn't turn the count's switch on"
# Captured once the knob has slid all the way on.
expect '^browser: onboarding daily-count slid$' "dark: the count's switch didn't finish its slide on"
capture_step dark-opt-ins-keys
send key tab
send key tab shift
send key space
expect '^browser: onboarding daily-count off$' \
  "dark: ⇥ then ⇧⇥ didn't come back to the count's switch for Space to turn it off"
skip_step opt-ins
expect '^browser: onboarding left opt-ins via skip$' "dark: Skip didn't leave the opt-ins step"
grep -q '^browser: onboarding daily-count set ' "$log" && fail "dark: Skip set the count, left off"
expect '^browser: onboarding step keys settled$' "dark: the keys' step didn't come after the opt-ins"
capture_step dark-keys
send key return
expect '^browser: onboarding left keys via return$' "dark: Return didn't leave the keys' step"
expect '^browser: onboarding done$' "dark: the flow didn't end after its last step"
expect '^browser: command bar shown$' "dark: the command bar didn't come up after the flow"
capture_step dark-in
send key escape
expect '^browser: command bar hidden$' "dark: Esc didn't close the command bar the flow left up"

# Help › Welcome to ilo plays it again in first run's window, recorded from before it starts
# through its loop, as its motion's proof. Esc there, with no step to go back to, does nothing,
# and Skip Setup skips the setup: the flow ends with no other step.
record onboarding-intro 5
send menu "Welcome to $app_name"
expect '^browser: command welcome via ui$' "Welcome to $app_name didn't run"
expect '^browser: onboarding step intro settled$' "Welcome to $app_name didn't play the intro again"
recorded
send key escape
skip_step intro
expect '^browser: onboarding left intro via skip$' "Skip Setup didn't leave the replayed welcome"
expect '^browser: onboarding skipped setup$' "Skip Setup at the replayed welcome didn't skip the setup"
expect '^browser: onboarding done$' "the replay didn't end after Skip Setup"
! grep -q '^browser: onboarding left intro via escape$' "$log" ||
  fail "Esc at the replayed welcome left it, though it has no step to go back to"
! sed -n '/^browser: onboarding left intro via skip$/,$p' "$log" |
  grep -q '^browser: onboarding step [a-z-]*$' ||
  fail "Skip Setup at the replayed welcome went on to another step, not out of the flow"
expect '^browser: command bar shown$' "the replay didn't end with the command bar"
send key escape
expect '^browser: command bar hidden$' "Esc didn't close the command bar the replay left up"

# With a page shown it plays in first run's window too, and the page stays where it was.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type "${site#http://}index.html"
send key return
expect "^browser: loaded [0-9]* ${site}index.html\$" "the page from the command bar didn't load"
send menu "Welcome to $app_name"
expect '^browser: first run shown$' "Welcome to $app_name over a page didn't show first run's window"
expect '^browser: onboarding step intro settled$' "Welcome to $app_name over a page didn't play it"
send key return
expect '^browser: onboarding left intro via return$' "Return didn't go on from the replayed welcome"
for each in $steps; do
  [ "$each" = intro ] && continue
  expect "^browser: onboarding step $each settled\$" "the replay's $each step didn't come"
  [ "$each" != import ] || capture_step replay
  # The keys' step has no Skip: its Return is the way in.
  if [ "$each" = keys ]; then
    send key return
    continue
  fi
  skip_step "$each"
done
expect '^browser: onboarding done$' "the replay over a page didn't end"
expect '^browser: command bar shown$' "the replay over a page didn't end with the command bar"
exec 3>&-
quit

# Suggested favorites: a first launch on a made-up Chrome with tabs open, whose history visits two
# sites three times and more, brings them in with ↩ at the browser, and the step after the import
# is Your favorites, offering those two, ticked.
fav_home="$PWD/$out/onboarding-favorites-home"
rm -rf "$fav_home"
if [ "$(uname)" = Darwin ]; then
  fav_chrome="$fav_home/Library/Application Support/Google/Chrome"
elif windows; then
  fav_chrome="$fav_home/AppData/Local/Google/Chrome/User Data"
else
  fav_chrome="$fav_home/.config/google-chrome"
fi
mkdir -p "$fav_chrome/Default"
printf '%s' '{"profile":{"info_cache":{"Default":{"name":"Me"}}}}' > "$fav_chrome/Local State"
printf '%s' '{}' > "$fav_chrome/Default/Preferences"
# Three bookmarks on its bar, which come in as pinned tabs, and 55 tabs open in its last session,
# which come in as Today tabs: placing those reads the first window's Shell, and the import step
# is pressed inside that Shell's update, which aborted the launch (#2428).
printf '%s' '{"roots":{"bookmark_bar":{"type":"folder","name":"Bookmarks bar","children":[
  {"type":"url","name":"Docs","url":"https://docs.example.org/"},
  {"type":"url","name":"Wiki","url":"https://wiki.example.org/"},
  {"type":"url","name":"Calendar","url":"https://calendar.example.org/"}]}},"version":1}' \
  > "$fav_chrome/Default/Bookmarks"
mkdir -p "$fav_chrome/Default/Sessions"
perl -e '
sub cmd { my ($id, $body) = @_; pack("S<C", length($body) + 1, $id) . $body }
sub nav { my ($tab, $index, $url) = @_;
  my $pickle = pack("l<3", $tab, $index, length $url) . $url;
  $pickle .= "\0" x (-length($pickle) % 4);
  $pickle .= pack("l<", 0);
  pack("l<", length $pickle) . $pickle }
print "SNSS", pack("l<", 3), cmd(9, pack("l<2", 1, 0));
for my $tab (2 .. 56) {
  print cmd(0, pack("l<2", 1, $tab)), cmd(2, pack("l<2", $tab, $tab - 2)),
    cmd(6, nav($tab, 0, "https://open.example.org/" . ($tab - 1)));
}
print cmd(255, "")' > "$fav_chrome/Default/Sessions/Session_13400000000000000" ||
  fail "couldn't write the made-up Chrome's session"
# Chromium counts microseconds from 1601: an hour ago.
python3 -I -c '
import sqlite3, sys, time
db = sqlite3.connect(sys.argv[1])
now = int((time.time() + 11644473600) * 1000000)
db.executescript("""CREATE TABLE urls(id INTEGER PRIMARY KEY AUTOINCREMENT, url LONGVARCHAR,
  title LONGVARCHAR, visit_count INTEGER DEFAULT 0 NOT NULL, typed_count INTEGER DEFAULT 0 NOT NULL,
  last_visit_time INTEGER NOT NULL, hidden INTEGER DEFAULT 0 NOT NULL);""")
db.executemany("INSERT INTO urls(url, title, visit_count, typed_count, last_visit_time) "
               "VALUES (?, ?, ?, 1, ?)",
               [("https://news.example.org/", "News", 5, now - 3600000000),
                ("https://mail.example.org/", "Mail", 4, now - 3600000000)])
db.commit()
' "$fav_chrome/Default/History" || fail "couldn't write the made-up Chrome's history"
fav_import_home=$fav_home
! windows || fav_import_home=$(cygpath -w "$fav_home")
log="$out/onboarding-favorites.log"
step=0
rm -f "$saved" "$profile_dir/Account"
BROWSER_IMPORT_HOME="$fav_import_home" BROWSER_DRIVE=1 BROWSER_ONBOARDING=dark \
  "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: onboarding step intro settled$' || fail "favorites: the intro didn't draw"
# Return, as the welcome waits for it and Skip Setup there skips the whole setup (#4093).
send key return
expect '^browser: onboarding left intro via return$' "favorites: Return didn't go on from the welcome"
expect '^browser: onboarding step assistant settled$' "favorites: the assistant's step didn't come"
skip_step assistant
expect '^browser: onboarding step import settled$' "favorites: the import step didn't come"
send key return
expect '^browser: import pick Chrome$' "favorites: ↩ didn't bring the made-up Chrome"
expect '^browser: import read Chrome: ' "favorites: picking Chrome didn't read it"
expect '^browser: import suggests https://news\.example\.org/ https://mail\.example\.org/$' \
  "favorites: the import didn't work out the sites its history visited most: $(grep 'import suggests' "$log")"
expect '^browser: import from Chrome: [0-9]* Spaces, 3 pinned tabs, 55 open tabs, ' \
  "favorites: the import didn't plan Chrome's bookmarks and open tabs: $(grep 'import from Chrome' "$log")"
expect '^browser: import done: ' "favorites: the import with open tabs didn't finish"
await '^browser: onboarding step favorites settled$' ||
  fail "favorites: the step after the import wasn't Your favorites"
capture_step favorites
skip_step favorites
expect '^browser: onboarding left favorites via skip$' "favorites: Skip didn't leave the step"
# The look goes back to the system's, as the light and dark passes set ilo's own, for the runs after.
send menu "$(cased 'Appearance › Automatic')"
expect '^browser: appearance automatic$' \
  "favorites: View › Appearance › Automatic didn't give the look back to the system"
exec 3>&-
quit

echo "ok: a new profile's first launch plays the intro, in a window of its own, which waits for Return and skips the setup at Skip Setup, then the assistant's step, the import of the fixture's $found, the default browser's step while ilo isn't the default, the colour, Transparency on the Mac, the tidy step, its time set by → and Return in dark, the private step, its ad blocking turned off and on from the keyboard, the account while the build names a sync server and the opt-ins, each left by Skip and the colour kept by Return, and the keys, left by Return, for the welcome with the command bar up, in light and dark; Help › Welcome to $app_name plays it again in first run's window, over a page too; a launch that brings Spaces back doesn't. Captures: $out/onboarding-*.png, and the replayed intro's motion in $out/onboarding-intro.mp4"
