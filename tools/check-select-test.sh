#!/bin/bash
# Proves tools/check-select.sh on check.sh's real runs: a change to the tokens alone is a look build
# of the smoke set and the runs that read the tokens; a change to the core is a full build of every
# run, the engine's own too; a path the map doesn't know is every run a look build does. It also
# proves each run the map names is one check.sh does. It runs in a scratch repository holding the
# runs' first lines, and changes nothing here.
# Usage: tools/check-select-test.sh
set -uo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/tools" "$scratch/app/check" "$scratch/app/design" "$scratch/core/public"
cp "$here/tools/check-select.sh" "$here/tools/check-select.map" "$scratch/tools/"
for file in "$here"/app/check/*.sh; do
  head -1 "$file" > "$scratch/app/check/${file##*/}"
done
echo base > "$scratch/app/design/tokens.toml"
echo base > "$scratch/core/public/browser_core.h"
cd "$scratch" || exit 1
git init -q
git add -A
git -c user.name=check -c user.email=check@example.test commit -q -m base
base=$(git rev-parse HEAD)
suite=$(sh "$here/app/check.sh" --runs default)
smoke=$(awk -F'\t' '$1 == "@smoke" { print $2 }' tools/check-select.map)
engine_only=$(awk -F'\t' '$1 == "@engine-only" { print $2 }' tools/check-select.map)
failed=0
has() { case " $1 " in *" $2 "*) ;; *) return 1 ;; esac; }
# The runs of the suite, in its order, that the list $1 has.
in_suite_order() {
  local run out=
  for run in $suite; do
    ! has "$1" "$run" || out="$out $run"
  done
  echo "${out# }"
}

# Commits a change to each of the paths $3..., on the base, and expects the selection's kind $1 and
# runs $2.
try() {
  local want_kind=$1 want_runs=$2 path said
  shift 2
  git checkout -q -f "$base"
  for path in "$@"; do
    mkdir -p "$(dirname "$path")"
    echo changed >> "$path"
  done
  git add -A
  git -c user.name=check -c user.email=check@example.test commit -q -m change
  if ! said=$(tools/check-select.sh "$base" HEAD 2> /dev/null); then
    echo "FAIL: check-select.sh failed for $*"
    failed=1
    return
  fi
  local want="kind=$want_kind
lib=cargo test --release --lib
runs=$want_runs"
  if [ "$said" = "$want" ]; then
    echo "ok: $* gives a $want_kind build"
  else
    echo "FAIL: $* gave"
    echo "$said" | sed 's/^/  /'
    echo "  where it should give"
    echo "$want" | sed 's/^/  /'
    failed=1
  fi
}

design=$(awk -F'\t' '$1 == "app/design/tokens.toml" { print $2 }' tools/check-select.map)
try look "$(in_suite_order "$smoke $design")" app/design/tokens.toml
try full "$(echo $suite)" core/tab.cc
try full "$(echo $suite)" core/public/browser_core.h app/design/tokens.toml
try full "$(echo $suite)" engine/series
not_engine=
for run in $suite; do has "$engine_only" "$run" || not_engine="$not_engine $run"; done
try look "${not_engine# }" app/src/a_file_no_rule_knows.rs
try look "${not_engine# }" app/design/tokens.toml somewhere/new.txt
try look "$(in_suite_order "$smoke")" docs/roadmap.md app/src/agents/tests.rs
try look "$(in_suite_order "$smoke find")" app/check/find.sh
# A run's own file on a look build never brings an engine-only run.
try look "$(in_suite_order "$smoke")" app/check/idle.sh

# Every run the map names is one of check.sh's.
while IFS=$'\t' read -r pattern what; do
  case $pattern in '' | '#'*) continue ;; esac
  case $what in full | all | none | self) continue ;; esac
  for run in $what; do
    has "$suite" "$run" || { echo "FAIL: the map's $pattern names $run, which check.sh doesn't do"; failed=1; }
  done
done < tools/check-select.map
[ "$failed" = 1 ] || echo "ok: every run the map names is one check.sh does"
exit "$failed"
