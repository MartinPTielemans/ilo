#!/bin/bash
# Captures every candidate material of the window-material spike (#4087) over white, mid grey, black
# and a busy picture, in dark and light, and prints the table of grounds and contrasts. It builds
# <out>/material first if it isn't there. The captures go in <out>/captures, the table in
# <out>/table.md, and what it read from each capture in <out>/reads.txt.
#
# The windows are the spike's own, two of them: a plain one, the backdrop, and the candidate's over
# it, 160 pt in from its edges, more than the blur's reach (the ring shots check it). The app puts
# the backdrop where no window that says by its bounds where it draws meets it (apps' windows,
# alerts, desktop widgets), out of the top right where banners come, and refuses each shot while
# one does, naming it. The Dock's and Notification Center's windows cover the whole screen,
# see-through, so the spot is also proved by pixels, before the shots and at the end: the
# backdrop alone in a flat colour no one meets, twice, captured as composed, every pixel that
# colour, or the run fails naming the rectangle that isn't. The pointer is never in a capture:
# screencapture leaves it out without -C. Dark and light are the candidate window's own
# appearance; the system's stays as it is.
#
# A capture is the screen as composed, at the candidate window's frame (screencapture -R): the
# window server blends what's behind into the material there, which a capture of the app's windows
# alone (tools/window-screenshot.sh) leaves out. That's a capture of the screen, so the script
# refuses to run anywhere but a VM, as the check VM and GitHub's hosted Macs are, by what the
# hardware says (kern.hv_vmm_present, hw.model), never an environment variable: it never runs on
# the owner's Mac, or anyone's.
#
# --reduce-transparency also takes the chosen candidates with Reduce Transparency on. It's the
# system's setting, so it's turned on with
#   defaults write com.apple.universalaccess reduceTransparency -bool true
# for those shots alone, and put back as it was as soon as they're taken, or when the script ends
# any other way: the value it had is written back, or the key deleted if it had none. Without the
# flag nothing of the system's is changed.
#
# Usage: spikes/window-material/capture.sh <out> [--reduce-transparency]
set -u
usage() { echo "usage: $0 <out> [--reduce-transparency]" >&2; exit 2; }
{ [ $# -eq 1 ] || { [ $# -eq 2 ] && [ "$2" = --reduce-transparency ]; }; } || usage
model=$(sysctl -n hw.model 2> /dev/null)
case $model in
VirtualMac*) ;;
*)
  [ "$(sysctl -n kern.hv_vmm_present 2> /dev/null)" = 1 ] || {
    echo "capture: this captures the screen, so it runs only in a VM, such as the check VM or a" \
      "GitHub runner, and this Mac (${model:-of no model}) isn't one" >&2
    exit 1
  }
  ;;
esac
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$1"
out=$(cd "$1" && pwd)
bin=$out/material
[ -x "$bin" ] || "$here/build.sh" "$out" || exit 1
shots=$out/captures
mkdir -p "$shots"
: > "$out/reads.txt"
: > "$out/notes.txt"
work=$(mktemp -d "${TMPDIR:-/tmp}/window-material.XXXXXX")
pid=
reduce_set=
reduce_before=
# A write to the app after it's gone fails, and says so, rather than ending the script.
trap '' PIPE

note() { echo "$*" | tee -a "$out/notes.txt"; }

stop() {
  [ -n "$pid" ] || return 0
  echo quit >&3 2> /dev/null
  exec 3>&- 4<&-
  tries=0
  while kill -0 "$pid" 2> /dev/null && [ $tries -lt 50 ]; do
    sleep 0.1
    tries=$((tries + 1))
  done
  kill "$pid" 2> /dev/null
  wait "$pid" 2> /dev/null
  pid=
}

# Reduce Transparency as it was before this script.
restore() {
  [ -n "$reduce_set" ] || return 0
  if [ -z "$reduce_before" ]; then
    defaults delete com.apple.universalaccess reduceTransparency 2> /dev/null
  elif [ "$reduce_before" = 1 ]; then
    defaults write com.apple.universalaccess reduceTransparency -bool true
  else
    defaults write com.apple.universalaccess reduceTransparency -bool false
  fi
  reduce_set=
  note "Reduce Transparency was put back as it was (${reduce_before:-not set})."
}

finish() {
  stop
  restore
  rm -rf "$work"
}
trap finish EXIT

fail() {
  echo "capture: $*" >&2
  exit 1
}

# Starts the spike's app, which answers each line on fd 3 with one on fd 4.
start() {
  rm -f "$work/in" "$work/out"
  mkfifo "$work/in" "$work/out"
  "$bin" serve < "$work/in" > "$work/out" 2>> "$out/serve.log" &
  pid=$!
  exec 3> "$work/in" 4< "$work/out"
  IFS= read -r -t 30 said <&4 || fail "the spike's app didn't start; see $out/serve.log"
  case $said in
  "serving reduce "*) reduce_seen=${said#serving reduce } ;;
  *) fail "the spike's app said: $said" ;;
  esac
}

# shot <material> <tint> <dark|light> <behind> [<active|inactive> <plain|glow> <name's end>]
shot() {
  file="$1-$2-$3-$4${7:+-$7}.png"
  echo "show $1 $2 $3 $4 ${5:-active} ${6:-plain}" >&3 || fail "the spike's app is gone"
  IFS= read -r -t 30 said <&4 || fail "the spike's app didn't answer for $file"
  case $said in
  "shown reduce "*" at "*) rect=${said##* at } ;;
  *) fail "$file: $said" ;;
  esac
  # One more try: a capture has failed once for a reason of the runner's (#2963).
  screencapture -x -R"$rect" "$shots/$file" || screencapture -x -R"$rect" "$shots/$file" ||
    fail "couldn't capture $file"
  echo "$file $("$bin" read "$shots/$file")" >> "$out/reads.txt"
  echo "$file"
}

# The backdrop's spot shows it and nothing else: proved by pixels, in two colours, |when|.
prove() {
  for colour in 13a37f d0158e; do
    echo "proof $colour" >&3 || fail "the spike's app is gone"
    IFS= read -r -t 30 said <&4 || fail "the spike's app didn't answer the proof"
    case $said in
    "proved at "*) rect=${said#proved at } ;;
    *) fail "proof: $said" ;;
    esac
    screencapture -x -R"$rect" "$work/proof.png" || screencapture -x -R"$rect" "$work/proof.png" ||
      fail "couldn't capture the proof"
    said=$("$bin" check "$work/proof.png" "$colour") || {
      cp "$work/proof.png" "$out/proof-$colour.png"
      fail "the backdrop's spot at $rect isn't clear $1: $said (capture in $out/proof-$colour.png)"
    }
  done
  note "The backdrop's spot at $rect (pt from the screen's top-left) showed the backdrop alone $1."
}

behinds="white grey black picture"
materials="underWindowBackground sidebar hudWindow fullScreenUI windowBackground"

start
note "macOS $(sw_vers -productVersion); Reduce Transparency read as $reduce_seen."
prove "before the shots"

# Each public material untinted, and today's: the dark material AppKit doesn't name under black at
# 0.25, and the HUD window's under white at 0.8 in light.
for appearance in dark light; do
  for material in $materials; do
    for behind in $behinds; do shot "$material" none "$appearance" "$behind"; done
  done
done
for behind in $behinds; do
  shot today none dark "$behind"
  shot today 000000a250 dark "$behind"
  shot hudWindow ffffffa800 light "$behind"
done

# Each with the tint that brings it nearest its targets, from the untinted grounds: in dark, of the
# brief's colour and of black.
"$bin" fit "$shots" > "$out/fit.txt" || fail "couldn't fit the tints"
while read -r material appearance tint off; do
  case $tint in *a000) continue ;; esac
  [ "$material-$tint-$appearance" != today-000000a250-dark ] || continue
  for behind in $behinds; do shot "$material" "$tint" "$appearance" "$behind"; done
done < "$out/fit.txt"

# The window lane's candidates, beside the fitted ones: the HUD window's material under black in
# dark, and under white at 0.83 in light.
named="hudWindow 000000a640 dark
hudWindow 000000a660 dark
hudWindow ffffffa830 light"
while read -r material tint appearance; do
  for behind in $behinds; do
    [ -f "$shots/$material-$tint-$appearance-$behind.png" ] ||
      shot "$material" "$tint" "$appearance" "$behind"
  done
done << EOF
$named
EOF

# The chosen candidate in each appearance, the public material nearest its targets once tinted, the
# named ones and today's: inactive, with the Space's glow, and over the ring.
chosen() { grep -v '^today ' "$out/fit.txt" | awk -v a="$1" '$2 == a' | sort -n -k4 | head -1; }
dark=$(chosen dark)
light=$(chosen light)
note "Chosen in dark: ${dark:-none}; in light: ${light:-none} (material, appearance, tint, off)."
extras="today 000000a250 dark
hudWindow ffffffa800 light
$named"
[ -z "$dark" ] || extras="$extras
$(echo "$dark" | awk '{ print $1, $3, $2 }')"
[ -z "$light" ] || extras="$extras
$(echo "$light" | awk '{ print $1, $3, $2 }')"
echo "$extras" | sort -u > "$work/extras"
while read -r material tint appearance; do
  shot "$material" "$tint" "$appearance" black inactive plain inactive
  shot "$material" "$tint" "$appearance" white inactive plain inactive
  shot "$material" "$tint" "$appearance" picture active glow glow
  # Black with white in the margin's outer half, 80 pt out from the window and more from the
  # patch: the same ground as over black says nothing past the backdrop reaches the material.
  shot "$material" "$tint" "$appearance" ring
done < "$work/extras"
while read -r material tint appearance; do
  over=$(awk -v f="$material-$tint-$appearance-black.png" '$1 == f { print $3 }' "$out/reads.txt")
  ring=$(awk -v f="$material-$tint-$appearance-ring.png" '$1 == f { print $3 }' "$out/reads.txt")
  if [ -n "$over" ] && [ "$over" = "$ring" ]; then same=same; else same=NOT the same; fi
  note "Margin 160 pt, $material $tint $appearance: over black ${over:-unread}, with the ring" \
    "${ring:-unread}: $same."
done < "$work/extras"

if [ $# -eq 2 ]; then
  stop
  reduce_before=$(defaults read com.apple.universalaccess reduceTransparency 2> /dev/null) ||
    reduce_before=
  reduce_set=1
  if defaults write com.apple.universalaccess reduceTransparency -bool true; then
    start
    prove "with Reduce Transparency on"
    if [ "$reduce_seen" = 1 ]; then
      while read -r material tint appearance; do
        shot "$material" "$tint" "$appearance" black active plain reduce
        shot "$material" "$tint" "$appearance" white active plain reduce
      done < "$work/extras"
    else
      note "Reduce Transparency was written, but the app didn't see it on, so no shots with it."
    fi
    stop
  else
    note "Reduce Transparency couldn't be written, so no shots with it."
  fi
  restore
  start
fi
prove "at the end"
stop

{
  echo "## Window material (#4087)"
  echo
  "$bin" table "$shots"
  echo "### Notes"
  echo
  sed 's/^/- /' "$out/notes.txt"
  echo
  echo "Fitted tints (material, appearance, tint, worst channel off its targets once tinted):"
  echo
  echo '```'
  cat "$out/fit.txt"
  echo '```'
} > "$out/table.md" || fail "couldn't make the table"
cat "$out/table.md"
