#!/bin/bash
# Proves tools/core-same.sh on the core's own header (#2195): edits to its comments, spacing and
# blank lines leave it the same to the compiler, so the prebuilt engine is kept, and a changed
# declaration (a field, an argument, an enum value, a #define's value) makes it differ, so the
# engine is refused. Each edit is tried against a commit, as the nightlies compare, and against
# the working tree, as engine/fetch.sh does. It runs in a scratch repository and changes nothing
# here. CI's Changes job runs it on every push.
# Usage: tools/core-same-test.sh
set -uo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
header=core/public/browser_core.h
mkdir -p "$scratch/tools" "$scratch/core/public"
cp "$here/tools/core-same.sh" "$scratch/tools/"
cp "$here/$header" "$scratch/$header"
cd "$scratch" || exit 1
git init -q
git -c user.name=check -c user.email=check@example.test commit -q --allow-empty -m empty
git add -A
git -c user.name=check -c user.email=check@example.test commit -q -m header
base=$(git rev-parse HEAD)
failed=0

# Applies the perl substitution $2 to the header, then expects core-same.sh to answer $1 (same or
# differs) against the base commit, from the working tree and from a commit of the edit.
try() {
  local want=$1 edit=$2 what=$3 got status
  git checkout -q -f "$base"
  perl -0777 -i -pe "$edit" "$header"
  if git diff --quiet; then
    echo "core-same-test: the edit for $what changed nothing:" \
      "the header no longer has what it edits"
    failed=1
    return
  fi
  for side in - commit; do
    if [ "$side" = commit ]; then
      git -c user.name=check -c user.email=check@example.test commit -q -am "$what"
      side=HEAD
    fi
    status=0
    tools/core-same.sh "$base" "$side" "$header" 2> /dev/null || status=$?
    case $status in 0) got=same ;; 1) got=differs ;; *) got="can't tell ($status)" ;; esac
    if [ "$got" = "$want" ]; then
      echo "ok: $what: $got ($side)"
    else
      echo "core-same-test: $what: $got against ${side}, not $want"
      failed=1
    fi
  done
}

# Each edit takes the first place in the header it fits. The header has no enums: its constants,
# such as the BC_MODIFIER_ flags, are #defines.
try same 's#//#// A reworded comment:#' "a line comment reworded"
try same 's#\n(typedef )#\n/* A block comment\n   over two lines. */\n$1#' "a block comment added"
try same 's#\n#\n\n\n#' "blank lines added"
try same 's#\n(typedef )#\n  $1#' "a declaration indented"
try same 's#(\S) (\S)#$1    $2#' "spaces between words widened"
try same 's#^//[^\n]*\n##m' "a comment line removed"
try same 's#(;)\n#$1 // A trailing comment.\n#' "a trailing comment added"
try differs 's#(typedef struct \w+ \{\n)#$1  int added_field;\n#' "a field added to a struct"
try differs 's#(typedef struct \w+ \{\n\s*)double#$1float#' "a field's type changed"
try differs 's#(\bbc_[a-z0-9_]+\()#$1int added_argument, #' "an argument added to a function"
try differs 's#(\#define BC_MODIFIER_SHIFT )1#${1}3#' "a #define's value changed"
try differs 's#\n(typedef )#\n/* unclosed\n$1#' "a block comment left unclosed"
try differs 's#visibility\("default"\)#visibility("default  //")#' \
  "a string's spacing, and a // in it, which is no comment"

[ "$failed" = 0 ] || exit 1
echo "ok: core-same.sh keeps the engine for comment edits and refuses it for changed declarations."
