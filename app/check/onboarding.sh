# A new profile's first launch plays the onboarding over the window, captured at each step in a
# light and a dark appearance: the intro draws ilo's wordmark and its line and goes on by itself, or
# at any key; the import step lists the made-up Arc in src/import/fixture (a made-up Chrome on
# Linux), never the Mac's own browsers; Esc or Skip goes on from a step, the colour step's Return
# gives the first Space the swatch picked, and the last step leaves the flow for the first Space's
# welcome with the command bar up and a line under it. The default browser's step shows only while
# ilo isn't the default, and is skipped everywhere but a Linux CI VM, where its button makes ilo
# the default for a moment, so the run never changes a desk's. The opt-ins' shows always, with
# the agents' switch, as every build has their server, and the dark pass turns it on in its new
# profile and sees the server start. The account's comes between the colour and the opt-ins while
# the build names a sync server (branding/sync-server), and Esc leaves it signed out: the run never
# signs in to the hosted server, as sync-account signs in on a server of its own. Help › Welcome to
# ilo plays it again, in the window while it shows no page and in a new window while it shows one. A
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
# The account's step, while the build names a server to sign in to.
account=
[ ! -s branding/sync-server ] || account="account "
# Launches a new profile, whose saved Spaces are gone and which isn't signed in, in appearance $1,
# and reads the steps it shows into $steps.
launch_new() {
  log="$out/onboarding-$1.log"
  step=0
  rm -f "$saved" "$profile_dir/Account"
  BROWSER_IMPORT_HOME="$import_home" BROWSER_DRIVE=1 BROWSER_ONBOARDING=$1 \
    "$browser" < "$out/drive" > "$log" 2>&1 &
  pid=$!
  exec 3> "$out/drive"
  await '^browser: onboarding step intro settled$' || fail "$1: the intro didn't draw"
  steps=$(grep -m1 '^browser: onboarding steps ' "$log" | cut -d' ' -f4-)
  case "$steps" in
    "intro import "*"color ${account}opt-ins") ;;
    *) fail "$1: the steps were '$steps', not the intro, the import, the rest, ${account}the opt-ins" ;;
  esac
}

# Light: the intro holds once drawn and goes on by itself, and Esc goes on from every step.
launch_new light
grep -q '^browser: onboarding started$' "$log" || fail "a new profile didn't start the onboarding"
capture_step light-intro
await '^browser: onboarding left intro via time$' || fail "light: the intro didn't go on by itself"
await '^browser: onboarding step import settled$' || fail "light: the import step didn't come"
grep -q "^browser: import found $found\$" "$log" ||
  fail "light: the import step didn't find the fixture's $found"
for each in $steps; do
  [ "$each" = intro ] && continue
  # The import came before any key, so it's been waited for already.
  [ "$each" = import ] ||
    expect "^browser: onboarding step $each settled\$" "light: the $each step didn't come"
  capture_step "light-$each"
  send key escape
  expect "^browser: onboarding left $each via escape\$" "light: Esc didn't leave the $each step"
done
expect '^browser: onboarding done$' "light: the flow didn't end after its last step"
grep -q '^browser: onboarding picked color ' "$log" && fail "light: Esc at the colour step gave the Space one"
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

# Dark: a key skips the intro, Skip at the window's bottom right leaves the import, and the colour
# step's → and Return give the first Space the first swatch. The key goes in as soon as the intro
# settles, with no capture first, as the intro goes on by itself after 3.4 s; the light pass
# captures it.
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
send key a
expect '^browser: onboarding left intro via key$' "dark: a key didn't skip the intro"
# Back on once the still intro, which goes on by itself at 2 s, is left.
if [ "$reduced" = windows ]; then
  windows_animations 1 || fail "PowerShell didn't turn Windows' animations back on"
  expect '^browser: reduce motion off$' \
    "dark: Windows' animations turned back on didn't bring the app's motion back"
fi
expect '^browser: onboarding step import settled$' "dark: the import step didn't come"
capture_step dark-import
# Its words and chip end 24 in from the window's right and bottom, 20 tall.
# Only the Mac's window frame is its content, so elsewhere the driver's layout gives its size: Linux
# logs no frame (host_window_frame), and Windows' takes in the title bar and borders.
frame=
if [ "$(uname)" = Darwin ]; then
  frame=$(grep -m1 '^browser: window [0-9]* frame ' "$log" | cut -d' ' -f7-8)
fi
if [ -z "$frame" ]; then
  send layout
  frame=$(grep -m1 '^browser: layout content ' "$log" |
    sed 's/^browser: layout content \([0-9]*\)x\([0-9]*\).*/\1 \2/')
fi
width=${frame% *}
height=${frame#* }
[ -n "$height" ] || fail "the window didn't log its frame"
send click $((width - 24 - 15)) $((height - 24 - 10))
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
      printf '[Desktop Entry]\nType=Application\nName=%s\nExec=%s %%U\n' "$app_name" "$browser" \
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
      send key escape
      expect '^browser: onboarding left default-browser via escape$' \
        "dark: Esc didn't leave the default browser's step"
      [ "$(uname)" != Linux ] || [ -z "${GITHUB_ACTIONS:-}" ] ||
        echo "skip: onboarding's default browser button. The VM has no xdg-settings."
    fi
    ;;
esac
expect '^browser: onboarding step color settled$' "dark: the colour step didn't come"
send key right
capture_step dark-color
send key return
expect '^browser: onboarding picked color f0eae5$' "dark: → and Return didn't pick the first swatch"
expect '^browser: onboarding left color via return$' "dark: Return didn't leave the colour step"
if [ -n "$account" ]; then
  expect '^browser: onboarding step account settled$' "dark: the account step didn't come"
  capture_step dark-account
  send key escape
  expect '^browser: onboarding left account via escape$' "dark: Esc didn't leave the account step"
  ! grep -q '^browser: onboarding sign in via ' "$log" || fail "dark: Esc at the account step signed in"
fi
expect '^browser: onboarding step opt-ins settled$' "dark: the opt-ins step didn't come"
capture_step dark-opt-ins
# The agents' row takes a click on its switch, where the app says it's drawn once the step has
# come, the second row of two with a crash collector and the only one without (#1286).
expect '^browser: onboarding agents at ' "dark: the opt-ins step didn't draw the agents' switch"
send click $(grep '^browser: onboarding agents at ' "$log" | tail -1 | cut -d' ' -f5-6)
expect '^browser: onboarding agents on$' "dark: a click on the agents' row didn't turn them on"
expect '^browser: agents server on ' "dark: the agents' switch didn't start their server"
send key escape
expect '^browser: onboarding left opt-ins via escape$' "dark: Esc didn't leave the opt-ins step"
expect '^browser: onboarding done$' "dark: the flow didn't end after its last step"
expect '^browser: command bar shown$' "dark: the command bar didn't come up after the flow"
capture_step dark-in
send key escape
expect '^browser: command bar hidden$' "dark: Esc didn't close the command bar the flow left up"

# Help › Welcome to ilo plays it again here, as the window shows no page, recorded from before it
# starts until after the intro has gone on by itself to the import, as its motion's proof.
record onboarding-intro 5
send menu "Welcome to $app_name"
expect '^browser: command welcome via ui$' "Welcome to $app_name didn't run"
expect '^browser: onboarding step intro settled$' "Welcome to $app_name didn't play the intro again"
recorded
expect '^browser: onboarding left intro via time$' "the replayed intro didn't go on by itself"
for each in $steps; do
  [ "$each" = intro ] && continue
  send key escape
  expect "^browser: onboarding left $each via escape\$" "Esc didn't leave the replayed $each step"
done
expect '^browser: onboarding done$' "the replay didn't end after its last step"
expect '^browser: command bar shown$' "the replay didn't end with the command bar"
send key escape
expect '^browser: command bar hidden$' "Esc didn't close the command bar the replay left up"

# With a page shown it plays in a new window, which shows none, and the page stays where it was.
send key t cmd
expect '^browser: command bar shown$' "⌘T didn't open the command bar"
send type "${site#http://}index.html"
send key return
expect "^browser: loaded [0-9]* ${site}index.html\$" "the page from the command bar didn't load"
# The windows by their ids in the log, as Linux logs no frame for one.
count_windows() { grep -o '^browser: window [0-9]* ' "$log" | sort -u | wc -l; }
windows=$(count_windows)
send menu "Welcome to $app_name"
expect '^browser: onboarding step intro settled$' "Welcome to $app_name over a page didn't play it"
[ "$(count_windows)" -gt "$windows" ] ||
  fail "Welcome to $app_name over a page didn't open a window for it"
send key escape
expect '^browser: onboarding left intro via key$' "Esc didn't skip the replayed intro"
expect '^browser: onboarding step import settled$' "the replay's import step didn't come"
capture_step replay
for each in $steps; do
  [ "$each" = intro ] || send key escape
done
expect '^browser: onboarding done$' "the replay in its own window didn't end"
expect '^browser: command bar shown$' "the replay in its own window didn't end with the command bar"
exec 3>&-
quit

echo "ok: a new profile's first launch plays the intro, which goes on by itself or at a key, then the import of the fixture's $found, the default browser's step while ilo isn't the default, the colour, the account while the build names a sync server and the opt-ins, each left by Esc or Skip and the colour kept by Return, for the welcome with the command bar up, in light and dark; Help › Welcome to $app_name plays it again, in a new window over a page; a launch that brings Spaces back doesn't. Captures: $out/onboarding-*.png, and the replayed intro's motion in $out/onboarding-intro.mp4"
