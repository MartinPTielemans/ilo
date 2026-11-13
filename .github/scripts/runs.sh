#!/bin/bash
# The runs a Mac or Windows check does, from check.sh --runs. runs=all takes longer than the
# check job's 120 minutes there, so it's dealt across three jobs (#2271): the long named runs in a
# job of their own, and the rest in two that 0.1.14's runs (42746639869, 42750992772) timed at
# about 60 minutes each on Windows and 45 on the Mac. A new run goes in the first.
# `runs.sh all` prints check.sh's runs, then the ones it does only when named.
# `runs.sh shard <n> [2]` prints the runs of job n of 3, or all of them for 0. With 2, of two jobs,
# the third's long runs in the first, for the Windows check's default runs, none of which is long
# (#2618).
# `runs.sh missing <dir> [runs]` prints each of check.sh's runs, and each run named, that no
# results file in <dir> has a line for (<run>, a tab, its result), and fails if there's one.
set -euo pipefail
check=$(dirname "$0")/../../app/check.sh
all=$(sh "$check" --runs)
long="adblock-parity adblock-cost perf perf-native"
second="load tabs history spaces profiles find pinned clear split cards extensions adblock adblock-lists
  windows little links trouble dialogs crash onboarding mcp appearance about sync filled notifications
  battery-os everyday count translate boosts sync-extensions adblock-live adblock-native widevine
  nightly shots tab-drag"
case ${1:-} in
all) echo $all ;;
shard)
  for run in $all; do
    in=1
    case " $(echo $long) " in *" $run "*) in=3 ;; esac
    [ "$in" = 3 ] || case " $(echo $second) " in *" $run "*) in=2 ;; esac
    [ "${3:-3}" != 2 ] || [ "$in" != 3 ] || in=1
    [ "$2" != 0 ] && [ "$2" != "$in" ] || printf '%s ' "$run"
  done
  echo
  ;;
missing)
  dir=$2
  shift 2
  missing=
  for run in $all ${*:-}; do
    case " $missing " in *" $run "*) continue ;; esac
    cat "$dir"/*.tsv 2>/dev/null | cut -f1 | grep -qxF -- "$run" || missing="$missing $run"
  done
  [ -z "$missing" ] || { echo "${missing# }"; exit 1; }
  ;;
*) echo "usage: runs.sh all | shard <n> [2] | missing <dir> [runs]" >&2 && exit 2 ;;
esac
