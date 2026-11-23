#!/bin/bash
# Captures every candidate material of the window-material spike (#4087) over white, mid grey, black
# and a busy picture, in dark and light, and prints the table of grounds and contrasts. It builds
# <out>/material first if it isn't there. The captures go in <out>/captures, the table in
# <out>/table.md, and what it read from each capture in <out>/reads.txt.
#
# The windows are the spike's own, two of them: a plain one, the backdrop, and the candidate's over
# it, 60 pt in from its edges. tools/window-screenshot.sh takes the process's windows alone, at the
# backdrop's frame, so nothing else on screen is in a capture. Dark and light are the candidate
# window's own appearance; the system's stays as it is.
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
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
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
  shown*) ;;
  *) fail "$file: $said" ;;
  esac
  # One more try: a capture has failed once for a reason of the runner's (#2963).
  "$repo/tools/window-screenshot.sh" "$pid" "$shots/$file" > /dev/null ||
    "$repo/tools/window-screenshot.sh" "$pid" "$shots/$file" > /dev/null ||
    fail "couldn't capture $file"
  echo "$file $("$bin" read "$shots/$file")" >> "$out/reads.txt"
  echo "$file"
}

behinds="white grey black picture"
materials="underWindowBackground sidebar hudWindow fullScreenUI windowBackground"

start
note "macOS $(sw_vers -productVersion); Reduce Transparency read as $reduce_seen."

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

# Each with the tint that brings it nearest its targets, from the untinted grounds.
"$bin" fit "$shots" > "$out/fit.txt" || fail "couldn't fit the tints"
while read -r material appearance tint off; do
  case $tint in *a000) continue ;; esac
  [ "$material-$tint-$appearance" != today-000000a250-dark ] || continue
  for behind in $behinds; do shot "$material" "$tint" "$appearance" "$behind"; done
done < "$out/fit.txt"

# The chosen candidate in each appearance, the public material nearest its targets once tinted, and
# today's: inactive, and with the Space's glow.
chosen() { grep -v '^today ' "$out/fit.txt" | awk -v a="$1" '$2 == a' | sort -n -k4 | head -1; }
dark=$(chosen dark)
light=$(chosen light)
note "Chosen in dark: ${dark:-none}; in light: ${light:-none} (material, appearance, tint, off)."
extras="today 000000a250 dark
hudWindow ffffffa800 light"
[ -z "$dark" ] || extras="$extras
$(echo "$dark" | awk '{ print $1, $3, $2 }')"
[ -z "$light" ] || extras="$extras
$(echo "$light" | awk '{ print $1, $3, $2 }')"
echo "$extras" | sort -u > "$work/extras"
while read -r material tint appearance; do
  shot "$material" "$tint" "$appearance" black inactive plain inactive
  shot "$material" "$tint" "$appearance" white inactive plain inactive
  shot "$material" "$tint" "$appearance" picture active glow glow
done < "$work/extras"

if [ $# -eq 2 ]; then
  stop
  reduce_before=$(defaults read com.apple.universalaccess reduceTransparency 2> /dev/null) ||
    reduce_before=
  reduce_set=1
  if defaults write com.apple.universalaccess reduceTransparency -bool true; then
    start
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
fi
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
