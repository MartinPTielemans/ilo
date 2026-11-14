#!/bin/bash
# Proves tools/nightly-commit.sh on the shapes of main's history it meets: an engine on main's line,
# a core change after it, and a batch's engine, built on a branch that main merged, both when the
# merge keeps the engine's core and when the branch had merged a newer core of main's first, which
# failed the nightly (#2697). Each builds its history in a scratch repository, whose engines are
# tags in a bare one beside it, and changes nothing here. CI's Changes job runs it on every push.
# Usage: tools/nightly-commit-test.sh
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
  mkdir -p tools core engine
  cp "$here/tools/nightly-commit.sh" "$here/tools/core-same.sh" tools/
  printf '[prebuilt]\n\trepository = %s\n' "$dir-engines" > engine/prebuilt.ini
  echo 'int a = 1;' > core/a.cc
  echo 'int b = 1;' > core/b.cc
}

# Commits everything as $1, after writing $3 to the file $2 when given, and names the commit $1.
commit() {
  [ $# -lt 3 ] || echo "$3" > "$2"
  git add -A
  git commit -q -m "$1"
  eval "$1=$(git rev-parse HEAD)"
}

# Tags an engine for OS $1 built from commit $2, as the engine repository does.
engine() {
  git push -q "$PWD-engines.git" "$2:refs/tags/engine-$1-$(git rev-parse --short=12 "$2")"
}

# Runs nightly-commit.sh at HEAD and expects it to name commit $1 and windows=$2, for $3.
expect() {
  local out got
  out=$(tools/nightly-commit.sh 2> "$scratch/why") || {
    echo "nightly-commit-test: $3: it failed: $(cat "$scratch/why")"
    failed=1
    return
  }
  got=$(printf '%s\n' "$out" | sed -n 's/^commit=//p')/$(printf '%s\n' "$out" | sed -n 's/^windows=//p')
  if [ "$got" != "$1/$2" ]; then
    echo "nightly-commit-test: $3: it named ${got%/*} with windows=${got#*/}, not $1 with windows=$2:" \
      "$(cat "$scratch/why")"
    failed=1
  fi
}

history
commit c0
engine mac "$c0"
engine windows "$c0"
commit c1 app.txt app
expect "$c1" true "an engine on main's line, its core unchanged since"

history
commit c0
engine mac "$c0"
commit c1 app.txt app
commit c2 core/a.cc 'int a = 2;'
commit c3 app.txt app2
expect "$c1" false "a core change after the engine, which no Windows engine has"

# The batch's branch changes core/b.cc and builds its engine, and main merges it.
history
commit c0
engine mac "$c0"
git checkout -q -b batch
commit b1 core/b.cc 'int b = 2;'
engine mac "$b1"
git checkout -q main
commit c1 app.txt app
git merge -q --no-ff -m merge batch
commit c3 app.txt app2
expect "$c3" false "a batch's engine, whose merge keeps its core"

# As #2697's: the batch's branch has main's core/a.cc from c1, and main changes it again at c2
# before it merges the branch, so no commit of main's line has the batch's core, and c1 has none
# of the engines. The night is c0, whose engine fits it.
history
commit c0
engine mac "$c0"
commit c1 core/a.cc 'int a = 2;'
git checkout -q -b batch
commit b1 core/b.cc 'int b = 2;'
engine mac "$b1"
git checkout -q main
commit c2 core/a.cc 'int a = 3;'
git merge -q --no-ff -m merge batch
commit c3 app.txt app
expect "$c0" false "a batch's engine whose branch had merged a newer core of main's"

[ "$failed" = 0 ] || exit 1
echo "ok: nightly-commit.sh names the newest commit of main's that a Mac engine fits."
