#!/bin/bash
# Proves tools/agent-context.sh --source on made-up files (#2565): a source file over 2,000 lines is
# listed with its count, largest first, and one at 2,000 isn't. Files nobody here writes by hand
# (lockfiles, *.gen.*, patches, .deps, vendor, third_party, node_modules, binary files, Markdown)
# aren't listed at any size, and the report never fails on what it lists. It runs in a scratch
# repository and changes nothing here.
# Usage: tools/agent-context-test.sh
set -uo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
cd "$scratch" || exit 1
git init -q
commit() { git -c user.name=check -c user.email=check@example.test commit -q "$@"; }

# Writes $2 lines to the file $1.
lines() {
  mkdir -p "$(dirname "$1")"
  seq "$2" | sed 's/^/line /' > "$1"
}

lines app/src/big.rs 2001
lines core/bigger.cc 2500
lines "app/src/a dir/spaced name.rs" 2100
lines app/src/edge.rs 2000
lines app/src/small.rs 10
lines app/Cargo.lock 3000
lines site/pnpm-lock.yaml 3000
lines site/package-lock.json 3000
lines site/src/routeTree.gen.ts 3000
lines app/patches/gpui-linux-embedded.patch 3000
lines docs/long.md 3000
lines app/.deps/zed/crates/big.rs 3000
lines vendor/lib.c 3000
lines app/third_party/lib.c 3000
lines site/node_modules/pkg/index.js 3000
mkdir -p app/branding
{ printf '\0'; seq 3000; } > app/branding/blob.car
git add -A
commit -m files
lines app/src/later.rs 2200
git add -A
commit -m later

failed=0
fail() {
  echo "agent-context-test: $1"
  echo "The report was:"
  sed 's/^/  /' "$scratch/out"
  failed=1
}

"$here/tools/agent-context.sh" --source > "$scratch/out" 2>&1
status=$?
[ "$status" = 0 ] || fail "the report exited $status with files over the line, where it only reports"
want="Source files over 2000 lines at HEAD, largest first:
    2500  core/bigger.cc
    2200  app/src/later.rs
    2100  app/src/a dir/spaced name.rs
    2001  app/src/big.rs
    8801  total, 4 files"
[ "$(cat "$scratch/out")" = "$want" ] ||
  fail "the report isn't the four files over 2000 lines, largest first, then their total; wanted:
$want"

# A commit before the file that crossed the line doesn't list it.
"$here/tools/agent-context.sh" --source HEAD^ > "$scratch/out" 2>&1
grep -q 'later.rs' "$scratch/out" && fail "HEAD^ listed app/src/later.rs, which HEAD^ doesn't have"
grep -q '6601  total, 3 files' "$scratch/out" || fail "HEAD^'s total isn't its three files' 6601 lines"

# With every file under the line, it says so and still passes.
git rm -q -r app core
commit -m "under the line"
"$here/tools/agent-context.sh" --source > "$scratch/out" 2>&1
status=$?
[ "$status" = 0 ] || fail "the report exited $status with no file over the line"
[ "$(cat "$scratch/out")" = "No source file is over 2000 lines at HEAD." ] ||
  fail "with no file over the line, the report didn't say so in one line"

# A revision that isn't a commit is an error, not an empty report.
"$here/tools/agent-context.sh" --source no-such-rev > "$scratch/out" 2>&1 &&
  fail "a revision that isn't a commit passed"

[ "$failed" = 0 ] && echo "agent-context-test: every case passed."
exit "$failed"
