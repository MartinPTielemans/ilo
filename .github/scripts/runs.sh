#!/bin/bash
# The runs a Mac or Windows check does, from check.sh --runs. They take longer than the check job's
# 120 minutes there, so they're dealt across several jobs (#2271, #3917), as check-linux.yml's plan
# deals Linux's.
# `runs.sh all` prints check.sh's runs, then the ones it does only when named.
# `runs.sh shard <n> <of> [runs]` prints job n's share of check.sh's runs, of <of> jobs, or all of
# them for 0: each run is in exactly one job's (check-scripts.yml checks so for each list and job
# count the workflows use). The runs the check does, those given or else all of them, are dealt in
# turn in check.sh's order. The rest, which the jobs only list as not asked for, are job 1's.
# `runs.sh missing <dir> [runs]` prints each of check.sh's runs, and each run named, that no
# results file in <dir> has a line for (<run>, a tab, its result), and fails if there's one.
set -euo pipefail
check=$(dirname "$0")/../../app/check.sh
all=$(sh "$check" --runs)
# The long runs, when done, have the last job to themselves: about 58 minutes of them on 11-21.
long="adblock-parity adblock-cost adblock-native perf perf-native"
# Job 1's, each in its turn: the runs that compose with Windows' Japanese and Korean IMEs, which a
# job installs in 40 to 50 s, and those that compile a check copy of their own on Windows (build.sh
# updates) with the toolchain a job sets up, so that one job alone pays for each.
first="dialogs keys board updates nightly update-checks"
# Whether the list $1 has the run $2.
has() { case " $1 " in *" $2 "*) ;; *) return 1 ;; esac; }
case ${1:-} in
all) echo $all ;;
shard)
  n=$2 of=$3
  shift 3
  todo=$(echo ${*:-$all})
  dealt=$of
  if [ "$of" -gt 1 ]; then
    for run in $long; do
      ! has "$todo" "$run" || dealt=$((of - 1))
    done
  fi
  turn=0
  for run in $all; do
    job=1
    if has "$todo" "$run"; then
      if [ "$dealt" != "$of" ] && has "$long" "$run"; then
        job=$of
      else
        has "$first" "$run" || job=$((turn % dealt + 1))
        turn=$((turn + 1))
      fi
    fi
    [ "$n" != 0 ] && [ "$n" != "$job" ] || printf '%s ' "$run"
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
*) echo "usage: runs.sh all | shard <n> <of> [runs] | missing <dir> [runs]" >&2 && exit 2 ;;
esac
