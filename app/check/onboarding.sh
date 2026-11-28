# check.sh runs this at 360.
# A new profile's first launch plays the onboarding, one card in a window of its own, the only one
# on screen until it ends, captured at each step in a light and a dark appearance. The card names
# its steps by number alone: the Welcome (1) while the build names a sync server (branding/
# sync-server) and the browser is signed out, whose Not Now goes on without signing in, as the run
# never signs in to the hosted server (sync-account signs in on a server of its own); Your
# assistants (2) while the build has the threads' setting, with Just the Browser where nobody is
# signed in; Bring your stuff (3), which lists the made-up Arc in src/import/fixture (a made-up
# Chrome on Linux), never the Mac's own browsers, with what comes from it as chips, which a click
# leaves out and another brings back, once a click on its icon has picked it, as the step picks
# none for the user; and Skip; Your favorites (3b), once an import offers sites;
# Your first Space (4), whose → picks the next palette; Other apps (5) while an agent program is
# found, whose command's Install a launch in a made-up home presses, writing the command's skill
# there as the picture under the cards brings its sidebar card (#4451); and step 6, always last,
# whose Help improve the dark pass turns on and off from the keyboard, and whose Start Browsing
# leaves the card for the first Space with the command bar up. ↩ presses a step's main
# button; ⌘[ goes back a step, which the dark pass checks at the last; Esc and ⌘W leave the step as
# it is. Make Default makes ilo the default for a moment only on
# a Linux CI VM, so the run never changes a desk's. Help › Welcome to ilo plays it again in first
# run's window, whether the browser's shows a page or not. A launch that brings Spaces back never
# starts it. Every other run sets BROWSER_ONBOARDING=skip (check.sh); this one sets the appearance
# instead.

serve
# The import step reads the fixture's home, set for each launch so the runs after keep their own.
# Arc has no Linux build, so there the home has a made-up Chrome where Linux keeps it, and on
# Windows where Windows does.
fixture="$PWD/src/import/fixture"
import_home=$fixture
found=Arc
# A kind the fixture's browser has something of, as the import draws only those: Arc's Spaces.
chip=Spaces
if [ "$(uname)" != Darwin ]; then
  # From the root, as chrome.exe moves into its own folder as it starts.
  fixture="$PWD/$out/onboarding-home"
  import_home=$fixture
  found=Chrome
  chip=Bookmarks
  chrome="$fixture/.config/google-chrome"
  if windows; then
    chrome="$fixture/AppData/Local/Google/Chrome/User Data"
    import_home=$(cygpath -w "$fixture")
  fi
  rm -rf "$fixture"
  mkdir -p "$chrome/Default"
  printf '%s' '{"profile":{"info_cache":{"Default":{"name":"Me"}}}}' > "$chrome/Local State"
  printf '%s' '{}' > "$chrome/Default/Preferences"
  # The import lists a browser only once it has bookmarks or history to bring in, and draws a chip
  # only for a kind with something in it: one bookmark.
  printf '%s' '{"roots":{"bookmark_bar":{"type":"folder","name":"Bookmarks bar","children":[{"type":"url","name":"Example","url":"https://example.org/"}]}}}' > "$chrome/Default/Bookmarks"
fi
# Captures the window as onboarding-$1.png.
capture_step() {
  ../tools/window-screenshot.sh "$pid" "$out/onboarding-$1.png" || fail "no window to capture"
}
# The import step picks no browser for the user (#4449): a click on the fixture's $found picks it,
# and its list says what comes from it as chips: a click on $chip leaves it out and another
# brings it back, keeping the step. Captured as onboarding-$1-import-chips.png with it left out.
import_choices() {
  local at
  ! grep -q '^browser: import pick ' "$log" || fail "$1: the import step picked a browser before a click"
  at=$(drawn "import-browser $found") || fail "$1: the import step drew no $found"
  send click $at
  expect "^browser: import pick $found\$" "$1: a click on $found didn't pick it"
  await "^browser: import read $found: " || fail "$1: picking $found didn't read it"
  at=$(drawn "import-chip $chip") || fail "$1: the import's list drew no $chip chip"
  send click $at
  expect "^browser: import chip $chip false\$" "$1: a click on $chip didn't leave it out"
  send state
  capture_step "$1-import-chips"
  send click $at
  expect "^browser: import chip $chip true\$" "$1: a second click on $chip didn't bring it back"
  none_since '^browser: onboarding left 3 ' || fail "$1: a click on a chip left the import step"
}
# The last line saying where the card is, of its steps coming and settling, a step left and its
# end, which ⌘W and Esc leave as it was. Not where it draws a control, which a frame can say again.
last_move() {
  grep -E '^browser: onboarding (step |left |done$)' "$log" | tail -1
}
# Launches a new profile, whose saved Spaces are gone and which isn't signed in, in appearance $1,
# and reads the steps it shows into $steps.
launch_new() {
  log="$out/onboarding-$1.log"
  step=0
  rm -f "$saved" "$profile_dir/Account"
  # A count endpoint nothing listens on, so the last step offers Help improve's switch, as the
  # prod build's does, while it stays off and nothing is sent (count.sh turns it on).
  BROWSER_IMPORT_HOME="$import_home" BROWSER_DRIVE=1 BROWSER_ONBOARDING=$1 \
    "$browser" --daily-count-url=http://127.0.0.1:9/v1/daily < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await '^browser: onboarding started$' || fail "$1: a new profile didn't start the onboarding"
  steps=$(onboarding_steps)
  # The import, Your first Space and the last always come here, as the fixture has a browser; the
  # Welcome only with a sync server; the rest where the build and the desk have them.
  echo "$steps" | grep -Eq '^(1 )?(2 )?3 4 (5 )?6$' ||
    fail "$1: the steps were '$steps', not the import, Your first Space and the last, in order, with only the others that may come"
  case "$steps" in
    "1 "*) [ -s branding/sync-server ] || fail "$1: the Welcome came in a build that names no sync server" ;;
    *) [ ! -s branding/sync-server ] || fail "$1: the Welcome didn't come, though the build names a sync server" ;;
  esac
  first=${steps%% *}
  step_settled "$first" "$1: the first step, $first, didn't draw"
  # BROWSER_ONBOARDING gives the card its look but leaves the window's material the system's, so
  # on a system of the other look the captures would show a card on glass no user can reach.
  # ilo's own Appearance, picked as a user picks it, gives the window the same look (#4093).
  look=Light
  [ "$1" = light ] || look=Dark
  send menu "$(cased "Appearance › $look")"
  expect "^browser: appearance $1\$" "$1: View › Appearance › $look didn't set ilo's look"
}

# On the Mac the exit grows the window, and the host logs each shape it gives the card's glass with
# the frame the UI was drawing as Core Animation committed it. The glass goes out with the frame
# that first draws the card in its shape, never one before or after, even while another window's
# frame keeps the main thread busy (exit item 3). $1 starts each failure's message.
glass_follows_card() {
  [ "$(uname)" = Darwin ] || return 0
  expect '^browser: onboarding exit grows$' \
    "$1the Mac's host didn't grow first run's window for its exit, so its glass can't be read"
  local last lag
  last=$(sed -n 's/^browser: onboarding card frame [0-9]* drawn \(.*\)/\1/p' "$log" | tail -1)
  expect "^browser: first run glass $last committed at frame " \
    "$1the host didn't log the glass of the card's last frame, $last, as committed"
  # Only a commit that changes the glass's shape is read: the shape it had through the steps is
  # set again as the grow starts, which changes nothing on screen.
  lag=$(awk '
    /^browser: onboarding exit (grows|fades)$/ { n = 0; split("", card); split("", went); exit_ = 1; next }
    /^browser: onboarding card frame / {
      shape = $7 " " $8
      if (exit_ && !(shape in card)) card[shape] = $5
    }
    /^browser: first run glass .* committed at frame / {
      shape = $5 " " $6
      if (shape == had) next
      had = shape
      if (exit_) went[++n] = shape " " $10
    }
    END {
      for (i = 1; i <= n; i++) {
        split(went[i], g, " ")
        shape = g[1] " " g[2]
        # A shape the card never drew in the exit puts glass on screen that no frame of the card matches.
        if (!(shape in card)) {
          printf "glass %s went out with frame %d, a shape the card never drew in the exit; ", shape, g[3]
          continue
        }
        read++
        if (g[3] != card[shape])
          printf "glass %s went out with frame %d, the card drew it at frame %d; ", shape, g[3], card[shape]
      }
      if (!read) printf "no glass shape of the grow matched a frame the card drew in it"
    }' "$log")
  # With the UI's calls that shaped the glass in the exit, and the host's commits between them.
  [ -z "$lag" ] || fail "$1the grow's glass didn't go out with the card's frame: $lag
$(sed -n '/^browser: onboarding exit grows$/,$p' "$log" |
    grep -e '^browser: first run shaped ' -e '^browser: first run glass ' \
      -e '^browser: onboarding card frame ' | head -40)"
}

# Light: each step captured once settled and left the way that takes nothing it offers.
launch_new light
capture_step "light-$first"
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
  # page comes. Closing it gives first run's window the keyboard back at its first step, which the
  # walk below goes on from, and the browser's window stays held.
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
    fail "light: the link's Little window took first run off its first step"
  ! grep -q '^browser: window on screen after first run$' "$log" ||
    fail "light: a link showed the browser's window while first run showed"
fi
for each in $steps; do
  step_settled "$each" "light: the step $each didn't come"
  [ "$each" = "$first" ] || capture_step "light-$each"
  if [ "$each" = 3 ]; then
    grep -q "^browser: import found $found\$" "$log" ||
      fail "light: the import step didn't find the fixture's $found"
    import_choices light
  fi
  pass_step "$each" "light: "
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
glass_follows_card "light: "
! grep -q '^browser: onboarding sign in via ' "$log" || fail "light: Not Now at the Welcome signed in"
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

# Dark: each step captured, the import's Skip clicked where it says it's drawn, Your first Space's
# → picking the next palette, and Esc and ⌘W there leaving the step as it is; Other apps as it
# shows, and at the last Help improve's switch from the keyboard, then ⌘[ back a step and on again
# to Start Browsing.
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
# The step before the last, which ⌘[ there goes back to.
before_last=$(echo "$steps" | awk '{ print $(NF - 1) }')
for each in $steps; do
  step_settled "$each" "dark: the step $each didn't come"
  case $each in
    3)
      capture_step dark-3
      import_choices dark
      ;;
    4)
      send key right
      capture_step dark-4
      # Esc cancels only an import or a sign-in under way, and ⌘W closes nothing, as the card has
      # no close: neither passes the step or goes back.
      send key escape
      send key w cmd
      # On Windows Alt+F4, ⌃F4 to the driver, asks first run's window to close, as its close
      # button and the taskbar's Close window do: the card has no close, so it stays, and the
      # browser window waits hidden behind it.
      if windows; then
        send key f4 ctrl
      fi
      # A last command, handled after the keys, so by its end they have done what they do.
      send state
      [ "$(last_move)" = "browser: onboarding step 4 settled" ] ||
        fail "dark: Esc, ⌘W or Alt+F4 at Your first Space moved the card: $(last_move)"
      ! grep -q '^browser: first run ended without its exit$' "$log" ||
        fail "dark: Esc, ⌘W or Alt+F4 at Your first Space closed first run's window"
      ;;
    5)
      # Either switch here would write into the run's own home, so only the skill launch below,
      # in a made-up one, clicks them.
      capture_step dark-5
      ;;
    6)
      capture_step dark-6
      await '^browser: onboarding help-improve at ' ||
        fail "dark: the last step didn't draw Help improve's switch, though the run names an endpoint"
      # The keyboard reaches the switch: from Start Browsing, where the step rests, ⇧⇥ back to it,
      # the stop before, where Space turns it on and again off, so it's left as it was and nothing
      # is set (#2985), and ⇥ on to Start Browsing again.
      send key tab shift
      send key space
      expect '^browser: onboarding switch slid$' \
        "dark: ⇧⇥ from Start Browsing then Space didn't turn Help improve's switch"
      capture_step dark-6-keys
      send key space
      expect '^browser: onboarding switch slid$' "dark: Space again didn't turn Help improve's switch back"
      send key tab
      ! grep -q '^browser: onboarding left 6 ' "$log" || fail "dark: the keys at Help improve left the last step"
      # Only a Linux CI VM, where no one browses, lets Make Default make ilo the default: xdg-settings
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
        # Make Default is the row's button, the stop before the switch: ⇧⇥ twice from Start
        # Browsing, and ↩, which a bare button with the keyboard takes as its press.
        send key tab shift
        send key tab shift
        send key return
        ! grep -q '^browser: onboarding left 6 ' "$log" ||
          fail "dark: ⇧⇥ twice from Start Browsing and ↩ left the last step, not pressing Make Default"
        for _ in $(seq 50); do
          made=$(xdg-settings get default-web-browser 2> /dev/null || true)
          [ "$made" = "${entry##*/}" ] && break
          sleep 0.1
        done
        [ "$made" = "${entry##*/}" ] ||
          fail "dark: Make Default left the default '$made', not ${entry##*/}"
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
      elif [ "$(uname)" = Linux ] && [ -n "${GITHUB_ACTIONS:-}" ]; then
        echo "skip: onboarding's Make Default. The VM has no xdg-settings."
      fi
      # ⌘[ goes back a step, and that step's way on comes back to the last, resting again on Start
      # Browsing, whose ↩ ends the card.
      send key '[' cmd
      expect '^browser: onboarding left 6$' "dark: ⌘[ didn't go back from the last step"
      step_settled "$before_last" "dark: ⌘[ at the last step didn't come back to step $before_last"
      pass_step "$before_last" "dark: "
      step_settled 6 "dark: the last step didn't come again after step $before_last"
      ;;
    *) capture_step "dark-$each" ;;
  esac
  pass_step "$each" "dark: "
  [ "$each" != 1 ] || ! grep -q '^browser: onboarding sign in via ' "$log" ||
    fail "dark: Not Now at the Welcome signed in"
  # Back on once the still first step is left.
  if [ "$each" = "$first" ] && [ "$reduced" = windows ]; then
    windows_animations 1 || fail "PowerShell didn't turn Windows' animations back on"
    expect '^browser: reduce motion off$' \
      "dark: Windows' animations turned back on didn't bring the app's motion back"
  fi
done
expect '^browser: onboarding done$' "dark: the flow didn't end after its last step"
expect '^browser: command bar shown$' "dark: the command bar didn't come up after the flow"
capture_step dark-in
send key escape
expect '^browser: command bar hidden$' "dark: Esc didn't close the command bar the flow left up"

# Help › Welcome to ilo plays it again in first run's window, recorded from before it starts
# through its entry, as its motion's proof. Esc and ⌘W there leave its first step as it is, and
# each step's way on goes through to the end.
record onboarding-entry 5
send menu "Welcome to $app_name"
expect '^browser: command welcome via ui$' "Welcome to $app_name didn't run"
expect '^browser: onboarding started$' "Welcome to $app_name didn't play the card again"
steps=$(onboarding_steps)
first=${steps%% *}
step_settled "$first" "Welcome to $app_name didn't settle on its first step, $first"
recorded
send key escape
send key w cmd
send state
[ "$(last_move)" = "browser: onboarding step $first settled" ] ||
  fail "Esc and ⌘W at the replay's first step moved the card: $(last_move)"
walk_steps "" "the replay: "
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
expect '^browser: onboarding started$' "Welcome to $app_name over a page didn't play the card"
step_settled "$(onboarding_steps | cut -d' ' -f1)" "Welcome to $app_name over a page didn't settle on its first step"
capture_step replay
walk_steps "" "the replay over a page: "
expect '^browser: command bar shown$' "the replay over a page didn't end with the command bar"
exec 3>&-
quit

# The command's skill (first run's step 5), on the Mac and Linux, where the command's Add writes
# one: a new profile in a made-up home of the run's own, with only Claude Code's folder, which the
# app gets by HOME, as it does the command it links. The left card's Install writes the command's
# skill there, one folder named as the skill holding a file only the user may read, and Settings ›
# Other apps' Remove, after the card, takes it and the skills folder it made away again. The skill's
# name isn't spelled here: it is the command's, which the build names. The right card's Connect
# isn't pressed: it would run Claude Code's own setup. Nothing looks into the run's own home.
if ! windows; then
  skill_home="$PWD/$out/onboarding-skill-home"
  rm -rf "$skill_home"
  mkdir -p "$skill_home/.claude"
  skills="$skill_home/.claude/skills"
  skill_file=
  # Waits up to 30 s for the skill's file to be $1: there, the one file in a folder under skills,
  # which it keeps in skill_file; or gone.
  skill_is() {
    for _ in $(seq 300); do
      case $1 in
        there)
          set -- "$skills"/*/SKILL.md
          [ $# -eq 1 ] && [ -f "$1" ] && skill_file=$1 && return 0
          set -- there
          ;;
        gone) [ ! -e "$skill_file" ] && return 0 ;;
      esac
      kill -0 "$pid" 2>/dev/null || return 1
      sleep 0.1
    done
    return 1
  }
  log="$out/onboarding-skill.log"
  step=0
  rm -f "$saved" "$profile_dir/Account"
  HOME="$skill_home" BROWSER_IMPORT_HOME="$import_home" BROWSER_DRIVE=1 BROWSER_ONBOARDING=dark \
    "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await '^browser: onboarding started$' || fail "skill: the first launch didn't start the onboarding"
  walk_steps 5 "skill: "
  await '^browser: onboarding command at ' || fail "skill: Other apps didn't draw the command's Install"
  logged '^browser: onboarding command Install$' || fail "skill: the command's button didn't read Install"
  [ ! -e "$skill_home/.claude/skills" ] || fail "skill: .claude/skills was there before a click"
  send click $(grep '^browser: onboarding command at ' "$log" | tail -1 | cut -d' ' -f5-6)
  expect_late '^browser: agent ilo-command \(connected\|couldn.t connect: .*\)$' \
    "skill: Install didn't finish installing the command"
  logged '^browser: agent ilo-command connected$' || fail "skill: Install didn't install the command"
  expect '^browser: onboarding command Installed$' "skill: the command's button didn't read Installed once it was"
  skill_is there ||
    fail "skill: Install wrote no SKILL.md in a folder of .claude/skills"
  entries=$(set -- "$skills"/*; echo $#)
  [ "$entries" -eq 1 ] || fail "skill: .claude/skills holds $entries entries, not one folder"
  skill_name=$(basename "$(dirname "$skill_file")")
  mode=$(stat -c %a "$skill_file" 2>/dev/null || stat -f %Lp "$skill_file")
  [ "$mode" = 600 ] || fail "skill: .claude/skills/$skill_name/SKILL.md is mode $mode, not 600"
  grep -Fqx "name: $skill_name" "$skill_file" ||
    fail "skill: .claude/skills/$skill_name/SKILL.md doesn't name its folder, $skill_name"
  # The picture under the cards plays as Install finishes: the sidebar card comes, and the step is
  # captured with it there.
  expect '^browser: onboarding sidebar card came$' "skill: Installed brought no sidebar card under the knot"
  capture_step skill-5-on
  # Settings › Other apps' Remove takes away exactly what Install wrote: the skill and the skills
  # folder it made, keeping .claude. First run's Installed presses nothing (#4451), so the card is
  # left first. (The Settings lane makes it ⋯ › Uninstall.)
  pass_step 5 "skill: "
  step_settled 6 "skill: first run's last step didn't come"
  pass_step 6 "skill: "
  expect '^browser: onboarding done$' "skill: first run's card didn't end after its last step"
  glass_follows_card "skill: "
  # The flow ends with the command bar up; it closes first, or its closing as Settings takes the
  # keyboard hands the browser's window main again and Settings' clicks land there.
  expect '^browser: command bar shown$' "skill: the command bar didn't come up after the flow"
  send key escape
  expect '^browser: command bar hidden$' "skill: Esc didn't close the command bar the flow left up"
  send menu Settings…
  settings_on_general "skill: Settings… didn't show Settings"
  send clickkey $(drawn "toolbar Other apps")
  expect '^browser: settings tab Other apps$' "skill: a click on Other apps didn't show its section"
  settings_drew 'Other apps'
  click_in_sight "ilo-command remove"
  expect_late '^browser: agent ilo-command disconnected$' \
    "skill: a click on Settings › Other apps' Remove didn't remove the ilo command"
  skill_is gone ||
    fail "skill: Remove left .claude/skills/$skill_name/SKILL.md"
  for _ in $(seq 300); do
    [ -e "$skill_home/.claude/skills" ] || break
    sleep 0.1
  done
  [ ! -e "$skill_home/.claude/skills" ] || fail "skill: Remove left the .claude/skills it made"
  [ -d "$skill_home/.claude" ] || fail "skill: Remove took .claude away"
  exec 3>&-
  quit
  rm -rf "$skill_home"
fi

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
# And the step the skill launch's quit kept, so first run plays from its start.
rm -f "$saved" "$profile_dir/Account" "$profile_dir/first-run-step"
BROWSER_IMPORT_HOME="$fav_import_home" BROWSER_DRIVE=1 BROWSER_ONBOARDING=dark \
  "$browser" < "$out/drive" > "$log" 2>&1 &
pid=$!
exec 3> "$out/drive"
await '^browser: onboarding started$' || fail "favorites: the first launch didn't start the onboarding"
walk_steps 3 "favorites: "
# A click on the made-up Chrome picks it, as the step picks none (#4449), and ↩ imports what its
# chips have on.
at=$(drawn "import-browser Chrome") || fail "favorites: the import step drew no Chrome"
send click $at
expect '^browser: import pick Chrome$' "favorites: a click on the made-up Chrome didn't pick it"
await '^browser: import read Chrome: ' || fail "favorites: picking Chrome didn't read it"
send key return
expect '^browser: onboarding import button via return$' "favorites: ↩ at the import didn't press its button"
await '^browser: import suggests https://news\.example\.org/ https://mail\.example\.org/$' ||
  fail "favorites: the import didn't work out the sites its history visited most: $(grep 'import suggests' "$log")"
expect '^browser: import from Chrome: [0-9]* Spaces, 3 pinned tabs, 55 open tabs, ' \
  "favorites: the import didn't plan Chrome's bookmarks and open tabs: $(grep 'import from Chrome' "$log")"
expect '^browser: import done: ' "favorites: the import with open tabs didn't finish"
await '^browser: onboarding left 3$' || fail "favorites: the import didn't go on from its step"
step_settled 3b "favorites: the step after the import wasn't Your favorites"
capture_step favorites
skip_step 3b
expect '^browser: onboarding left 3b$' "favorites: Skip didn't leave the step"
# The look goes back to the system's, as the light and dark passes set ilo's own, for the runs after.
send menu "$(cased 'Appearance › Automatic')"
expect '^browser: appearance automatic$' \
  "favorites: View › Appearance › Automatic didn't give the look back to the system"
exec 3>&-
quit

echo "ok: a new profile's first launch plays the onboarding's card in a window of its own, the only one on screen until it ends, through the steps it shows, $steps in the replay: the Welcome's Not Now while the build names a sync server, Just the Browser at the assistants', the import of the fixture's $found and its Skip, Your first Space, with → picking the next palette and Esc and ⌘W leaving it as it is, Other apps where it comes, the command's Install writing its skill into a made-up home off Windows as its sidebar card comes and Settings' Remove taking it away, and the last, its Help improve turned on and off from the keyboard and ⌘[ going back from it, ended by Start Browsing for the first Space with the command bar up, in light and dark; the import's ↩ brings in a made-up Chrome, after which Your favorites offers its sites; Help › Welcome to $app_name plays it again in first run's window, over a page too; a launch that brings Spaces back doesn't. Captures: $out/onboarding-*.png, and the replayed entry's motion in $out/onboarding-entry.mp4"
