#!/bin/sh
# The runs of check.sh, or a step of one, that fail at times on an OS while their issue is open
# (#2197), listed in check/quarantine.mac, quarantine.windows and quarantine.linux. A quarantined
# run still runs and shows in its Check workflow's summary, but its failure doesn't fail the check,
# on main or on a branch, and the summary lists the quarantine under the runs. Each line is:
#
#   <run>[@<session>] #<issue> <YYYY-MM-DD> [<its FAIL line's start, as the summary shows it>]
#
# With a FAIL line's start, only that step's failure is quarantined, and any other fails the check as
# before. On Linux, @xvfb, @sway, @kwin or @wayland keeps the line to that session, and the runs
# drops and ime are check/drops.sh and check/ime.sh. A line lasts 14 days, and its issue must stay
# open: lint, which Check scripts runs on each change and each night, fails otherwise, so a run on
# the list is fixed within two weeks or its line is renewed on purpose.
#
#   quarantine.sh match <os> <run> <session> <FAIL line>   prints the issue of the line that covers
#                                                          that failure, or fails when none does
#   quarantine.sh list <os>    the lines as a Markdown table, for a Check run's summary
#   quarantine.sh lint         fails on a line that's malformed, names no run, is more than 14 days
#                              old or whose issue isn't open, and on an issue main-check.sh opened
#                              for a run red twice running that no line names a day later
set -eu
dir=$(cd "$(dirname "$0")" && pwd)
repo=${GITHUB_REPOSITORY:-MartinPTielemans/ilo}
days=14

# The lines of quarantine.$1, without comments and blank lines.
lines() {
  sed -e '/^[[:space:]]*#/d' -e '/^[[:space:]]*$/d' "$dir/quarantine.$1"
}

# $1 with each run of blanks made one space, and none at its ends.
squeeze() {
  printf '%s\n' "$1" | sed -e 's/[[:space:]][[:space:]]*/ /g' -e 's/^ //' -e 's/ $//'
}

# Seconds since 1970 of the day $1, YYYY-MM-DD, with GNU date or the Mac's.
day() {
  date -u -d "$1" +%s 2> /dev/null || date -u -j -f %Y-%m-%d "$1" +%s 2> /dev/null
}

case ${1:-} in
match)
  [ $# = 5 ] || { echo "usage: quarantine.sh match <os> <run> <session> <FAIL line>" >&2; exit 2; }
  os=$2 run=$3 session=$4
  said=$(squeeze "$5")
  [ -f "$dir/quarantine.$os" ] || { echo "quarantine.sh: no check/quarantine.$os" >&2; exit 2; }
  lines "$os" | {
    while read -r name issue added step; do
      case $name in
      "$run") ;;
      "$run@$session") ;;
      *) continue ;;
      esac
      step=$(squeeze "$step")
      case $said in
      "$step"*)
        echo "$issue"
        exit 0
        ;;
      esac
    done
    exit 1
  }
  ;;
list)
  [ $# = 2 ] || { echo "usage: quarantine.sh list <os>" >&2; exit 2; }
  file=app/check/quarantine.$2
  [ -f "$dir/quarantine.$2" ] || { echo "quarantine.sh: no $file" >&2; exit 2; }
  if [ -z "$(lines "$2")" ]; then
    echo "Nothing is quarantined in $file."
    exit 0
  fi
  echo "Quarantined in $file: these run and show above, but their failures don't fail the check."
  echo
  echo "| Run | Step | Issue | Since |"
  echo "|---|---|---|---|"
  lines "$2" | while read -r name issue added step; do
    step=$(squeeze "$step" | tr '|' '/')
    echo "| $name | ${step:-any} | [$issue](https://github.com/$repo/issues/${issue#\#}) | $added |"
  done
  ;;
lint)
  status=0
  now=$(day "$(date -u +%Y-%m-%d)")
  all=" $(sh "$dir/../check.sh" --runs) "
  named=
  bad() { # <file> <line> <why>
    status=1
    if [ -n "${GITHUB_ACTIONS:-}" ]; then
      echo "::error file=app/check/$1,line=$2::$3"
    else
      echo "app/check/$1:$2: $3" >&2
    fi
  }
  for os in mac windows linux; do
    file=quarantine.$os
    [ -f "$dir/$file" ] || { bad "$file" 1 "there's no $file"; continue; }
    n=0
    while IFS= read -r line || [ -n "$line" ]; do
      n=$((n + 1))
      case $(squeeze "$line") in '' | '#'*) continue ;; esac
      set -f
      # shellcheck disable=SC2086 # Split into its fields.
      set -- $line
      set +f
      name=${1:-} issue=${2:-} added=${3:-}
      run=${name%%@*} session=
      case $name in *@*) session=${name#*@} ;; esac
      case "$os $run" in
      "linux drops" | "linux ime") [ -z "$session" ] || bad "$file" "$n" "$run takes no session" ;;
      *)
        case $all in *" $run "*) ;; *) bad "$file" "$n" "check.sh has no run $run" ;; esac
        case "$os:$session" in
        *: | linux:xvfb | linux:sway | linux:kwin | linux:wayland) ;;
        linux:*) bad "$file" "$n" "Linux has no session $session: xvfb, sway, kwin or wayland" ;;
        *) bad "$file" "$n" "only Linux names a session" ;;
        esac
        ;;
      esac
      number=${issue#\#}
      case "$issue:$number" in
      '#'*:*[!0-9]* | *: | [!#]*) bad "$file" "$n" "the second field is the issue that owns it, such as #2197, not \"$issue\"" ;;
      *)
        named="$named $number"
        state=$(gh issue view "$number" --repo "$repo" --json state -q .state 2>&1) ||
          state="unread: $state"
        [ "$state" = OPEN ] || bad "$file" "$n" "its issue $issue isn't open ($state): a fixed run leaves the list"
        ;;
      esac
      case $added in
      [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]) since=$(day "$added") || since= ;;
      *) since= ;;
      esac
      if [ -z "$since" ]; then
        bad "$file" "$n" "the third field is the day it went in, YYYY-MM-DD, not \"$added\""
      elif [ "$since" -gt "$now" ]; then
        bad "$file" "$n" "it went in on $added, after today"
      elif [ $(((now - since) / 86400)) -gt "$days" ]; then
        bad "$file" "$n" "it has been listed $(((now - since) / 86400)) days, over $days: fix $run, or renew the line with a new day if $issue says why it waits"
      fi
    done < "$dir/$file"
  done
  # A run red on main twice running is fixed that day, which closes its issue (main-check.sh),
  # or quarantined under it.
  gh issue list --repo "$repo" --state open --search 'in:title "fails on main twice running"' \
    --json number,title,createdAt -q '.[] | "\(.number) \(.createdAt[0:10]) \(.title)"' > "${TMPDIR:-/tmp}/twice.$$" ||
    { echo "quarantine.sh: couldn't list the issues of runs red twice running" >&2; status=1; }
  while read -r number opened title; do
    case " $named " in *" $number "*) continue ;; esac
    since=$(day "$opened") || continue
    [ $((now - since)) -lt 86400 ] ||
      bad quarantine.sh 1 "#$number, $title, is open a day on and no quarantine line names it: fix the run or quarantine it"
  done < "${TMPDIR:-/tmp}/twice.$$"
  rm -f "${TMPDIR:-/tmp}/twice.$$"
  exit "$status"
  ;;
*)
  echo "usage: quarantine.sh match <os> <run> <session> <FAIL line> | list <os> | lint" >&2
  exit 2
  ;;
esac
