#!/bin/bash
# Proves which engine engine/fetch.sh takes (#2913): one built at the commit, one built a commit
# behind on the same core, and at a batch's landing, the batch's engine over one built on main since
# the batch branched, which is nearer by commit count but lacks the batch's core. A branch whose
# core no engine has still gets the nearest, and an engine the nearest was built over isn't
# compared, so a core branch doesn't compare every engine since the first. Each builds its history
# in a scratch repository, whose engines are tags in a bare one beside it, and changes nothing here.
# CI's Changes job runs it on every push.
# Usage: tools/engine-fetch-test.sh
set -uo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
# The scratch histories are the same whoever runs it, whatever their git config says.
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export GIT_AUTHOR_NAME=check GIT_AUTHOR_EMAIL=check@example.test
export GIT_COMMITTER_NAME=check GIT_COMMITTER_EMAIL=check@example.test
failed=0

# A new repository, in the current directory from then on, whose engines are $PWD-engines.git's.
history() {
  local dir
  dir=$(mktemp -d "$scratch/history.XXXX")
  git init -q --bare "$dir-engines.git"
  cd "$dir" || exit 1
  git init -q -b main
  mkdir -p tools core engine/patches
  cp "$here/tools/core-same.sh" tools/
  cp "$here/engine/fetch.sh" engine/
  printf '[prebuilt]\n\trepository = %s\n' "$dir-engines" > engine/prebuilt.ini
  echo 'int a = 1;' > core/a.cc
  echo 'int b = 1;' > core/b.cc
  echo 'one.patch' > engine/series
}

# Commits everything as $1, after writing $3 to the file $2 when given, and names the commit $1.
commit() {
  [ $# -lt 3 ] || echo "$3" > "$2"
  git add -A
  git commit -q -m "$1"
  eval "$1=$(git rev-parse HEAD)"
}

# Tags an engine built from commit $1, for each OS, as the engine repository does: fetch.sh asks
# for the one it runs on.
engine() {
  local os
  for os in mac linux windows; do
    git push -q "$PWD-engines.git" "$1:refs/tags/engine-$os-$(git rev-parse --short=12 "$1")"
  done
}

# Runs fetch.sh commit at HEAD and expects it to take the engine built from $1, for $2.
expect() {
  local got
  got=$(engine/fetch.sh commit 2> "$scratch/why") || {
    echo "engine-fetch-test: $2: it failed: $(cat "$scratch/why")"
    failed=1
    return
  }
  if [ "$got" != "$1" ]; then
    echo "engine-fetch-test: $2: it took the engine of ${got:0:12}, not ${1:0:12}."
    failed=1
  fi
}

history
commit c0
engine "$c0"
commit c1 core/a.cc 'int a = 2;'
engine "$c1"
expect "$c1" "an engine built at the commit"

history
commit c0
engine "$c0"
commit c1 engine/series 'two.patch'
engine "$c1"
commit c2 app.txt app
expect "$c1" "an engine a commit behind, on the same core"

# The batch branches, changes core/b.cc and builds its engine. Main goes on three commits and builds
# an engine of its own core. At the landing's merge, an engine is as far as the commits on the other
# side: main's two, the batch's four.
history
commit c0
engine "$c0"
git checkout -q -b batch
commit b1 core/b.cc 'int b = 2;'
engine "$b1"
git checkout -q main
commit m1 app.txt main
commit m2 app2.txt main
commit m3 app3.txt main
engine "$m3"
git merge -q --no-ff -m landing batch
expect "$b1" "a landing's merge, where main's own engine is nearer but lacks the batch's core"

history
commit c0
engine "$c0"
commit c1 app.txt app
engine "$c1"
commit c2 core/a.cc 'int a = 2;'
expect "$c1" "a branch whose core no engine has, which takes the nearest"

# The core goes back to c0's, whose engine the nearest, c1's, was built over: it isn't compared,
# though it has the core, as only a revert brings an older engine's core back.
history
commit c0
engine "$c0"
commit c1 core/a.cc 'int a = 2;'
engine "$c1"
commit c2 core/a.cc 'int a = 1;'
expect "$c1" "an engine the nearest was built over, which isn't compared"

[ "$failed" = 0 ] && echo "engine-fetch-test: fetch.sh takes the engine with the core, and else the nearest."
exit "$failed"
