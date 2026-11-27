#!/bin/bash
# What a build of <head> needs checked, from what changed since <base>: a look build, UI alone on the
# engine the last full build published, or a full build, a new engine and every run. Prints
#   kind=look|full
#   lib=<the lib tests' command>
#   runs=<check.sh's runs, in its order>
# A change to core/ or engine/ is a full build (check-select.map says which paths). A look build does
# the map's smoke set and the runs each changed path selects; a path the map doesn't know selects
# every run, so the selection fails safe. The runs whose subject is the engine alone (@engine-only)
# run only on a full build. Each changed path and what it selected goes to stderr.
# The runs are those check.sh does unnamed at <head>, as its runs_of reads them, so an old cut's
# selection is from its own runs. A look build of a dev/rework commit ends with
# tools/look-build-mark.sh <commit> <run-id>..., which marks the commit for the rework copy's refresh
# once every run the selection started is green on it.
set -euo pipefail
[ $# = 2 ] || { echo "usage: $0 <base> <head>" >&2; exit 2; }
base=$1 head=$2
map=${CHECK_SELECT_MAP:-$(dirname "$0")/check-select.map}

# check.sh's unnamed runs at $head, in its order: its runs_of on git's copy of the files.
suite=$(git grep '^# check\.sh runs this at [0-9]*\.$' "$head" -- 'app/check/*.sh' |
  sed 's|^[^:]*:app/check/\([^:]*\)\.sh:# check\.sh runs this at \([0-9]*\)\.$|\2 \1|' |
  sort -k1,1n -k2,2 | cut -d' ' -f2 | tr '\n' ' ')
[ -n "$suite" ] || { echo "$0: no runs found in app/check at $head" >&2; exit 1; }

smoke=$(awk -F'\t' '$1 == "@smoke" { print $2 }' "$map")
engine_only=$(awk -F'\t' '$1 == "@engine-only" { print $2 }' "$map")
has() { case " $1 " in *" $2 "*) ;; *) return 1 ;; esac; }

# What the first rule $1 matches selects.
select_for() {
  local pattern what
  while IFS=$'\t' read -r pattern what; do
    case $pattern in '' | '#'* | '@'*) continue ;; esac
    # shellcheck disable=SC2053 # the pattern is a glob
    if [[ $1 == $pattern ]]; then
      echo "$what"
      return
    fi
  done < "$map"
  echo unknown
}

kind=look all=0 picked=" $smoke "
while IFS= read -r path; do
  what=$(select_for "$path")
  case $what in
  full) kind=full ;;
  all | unknown) all=1 ;;
  none) ;;
  self)
    run=$(basename "$path" .sh)
    # A run check.sh does only when named isn't in the suite, so its file selects nothing.
    if has "$suite" "$run"; then picked="$picked$run "; else what="none (named run)"; fi
    ;;
  *) picked="$picked$what " ;;
  esac
  echo "$path: $what" >&2
done < <(git diff --name-only --no-renames "$base" "$head")

runs=
for run in $suite; do
  if [ "$kind" = full ]; then
    runs="$runs $run"
  elif ! has "$engine_only" "$run" && { [ "$all" = 1 ] || has "$picked" "$run"; }; then
    runs="$runs $run"
  fi
done
echo "kind=$kind"
echo "lib=cargo test --release --lib"
echo "runs=${runs# }"
