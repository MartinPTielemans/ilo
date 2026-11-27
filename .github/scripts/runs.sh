#!/bin/bash
# The runs a Mac or Windows check does, from check.sh --runs. They take longer than the check job's
# 120 minutes there, so they're dealt across several jobs (#2271, #3917), as check-linux.yml's plan
# deals Linux's.
# `runs.sh all` prints check.sh's runs, then the ones it does only when named.
# `runs.sh shard <n> <of> [runs]` prints job n's share of check.sh's runs, of <of> jobs, or all of
# them for 0: each run is in exactly one job's (check-scripts.yml checks so for each list and job
# count the workflows use). The runs the check does, those given or else all of them, are dealt in
# turn in check.sh's order. The rest, which the jobs only list as not asked for, are job 1's.
# With RUN_TIMES naming a file of each run's seconds, as runs-mac.tsv, the runs are dealt by their
# times instead, longest first, each to the job with the least so far, as round the jobs in turn the
# Mac's three were 26, 44 and 52 minutes (45849137595); job 1's and the long runs stay where they
# are dealt above. `runs.sh jobs <max> [runs]` then prints how many jobs the runs take, at up to
# 22 minutes of them a job, as a job sets up in about 3, and at most <max>.
# `runs.sh missing <dir> [runs]` prints each of check.sh's runs, and each run named, that no
# results file in <dir> has a line for (<run>, a tab, its result), and fails if there's one.
set -euo pipefail
check=$(dirname "$0")/../../app/check.sh
all=$(sh "$check" --runs)
# The long runs, when done, have the last job to themselves: about 58 minutes of them on 11-21.
long="adblock-parity adblock-cost adblock-native perf perf-native"
# Job 1's, each in its turn: the runs that compose with Windows' Japanese and Korean IMEs, which a
# job installs in 40 to 50 s, those that compile a check copy of their own on Windows (build.sh
# updates) with the toolchain a job sets up, so that one job alone pays for each, and keychain,
# whose job takes the Mac's prod build for its turn (check-mac.yml, #4000).
first="dialogs keys board updates nightly update-checks keychain"
# Whether the list $1 has the run $2.
has() { case " $1 " in *" $2 "*) ;; *) return 1 ;; esac; }
# Each of check.sh's runs as "<run> <todo> <first> <long> <seconds>", for the dealing by time.
timed() {
  local run
  for run in $all; do
    printf '%s %s %s %s\n' "$run" "$(has "$todo" "$run" && echo 1 || echo 0)" \
      "$(has "$first" "$run" && echo 1 || echo 0)" "$(has "$long" "$run" && echo 1 || echo 0)"
  done | awk 'NR == FNR { if ($0 !~ /^#/ && NF == 2) t[$1] = $2; next }
    { print $0, ($1 in t) ? t[$1] : 120 }' FS='\t' "$RUN_TIMES" FS=' ' -
}
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
  if [ -n "${RUN_TIMES:-}" ]; then
    # Each run's job: longest first to the job with the least, ties to the lower job and to the
    # run first in check.sh's order, so every job deals the same.
    timed | awk -v of="$of" -v dealt="$dealt" '
      { run[NR] = $1; todo[NR] = $2; first[NR] = $3; long[NR] = $4; secs[NR] = $5; n = NR }
      END {
        for (i = 1; i <= n; i++) {
          job[i] = 1
          if (!todo[i]) continue
          if (dealt != of && long[i]) job[i] = of
          else if (first[i]) load[1] += secs[i]
          else { m++; free[m] = i }
        }
        for (a = 2; a <= m; a++) {
          x = free[a]
          for (b = a - 1; b >= 1 && secs[free[b]] < secs[x]; b--) free[b + 1] = free[b]
          free[b + 1] = x
        }
        for (a = 1; a <= m; a++) {
          best = 1
          for (j = 2; j <= dealt; j++) if (load[j] < load[best]) best = j
          job[free[a]] = best
          load[best] += secs[free[a]]
        }
        for (i = 1; i <= n; i++) print run[i], job[i]
      }' | while read -r run job; do
      [ "$n" != 0 ] && [ "$n" != "$job" ] || printf '%s ' "$run"
    done
    echo
    exit 0
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
jobs)
  max=$2
  shift 2
  todo=$(echo ${*:-$all})
  [ -n "${RUN_TIMES:-}" ] || { echo "runs.sh jobs: RUN_TIMES names no file of the runs' times" >&2; exit 2; }
  timed | awk -v max="$max" '$2 { s += $5 }
    END { j = int((s + 1319) / 1320); if (j < 1) j = 1; if (j > max) j = max; print j }'
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
*) echo "usage: runs.sh all | shard <n> <of> [runs] | jobs <max> [runs] | missing <dir> [runs]" >&2 && exit 2 ;;
esac
