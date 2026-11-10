#!/bin/bash
# Whether two versions of the core are the same to the compiler: they may differ in the comments,
# spacing and blank lines of C, C++ and Objective-C files, and in nothing else (#2195). About one
# commit to core/public/browser_core.h in seven changed only its comments, and each made every
# prebuilt engine stale and started an engine build on each OS. Everything that decides whether an
# engine fits the core asks this, so they can't disagree: engine/fetch.sh and app/build.rs, which
# take an engine or refuse it, engine-changed.sh, mac-nightly.sh and nightly-commit.sh, which build
# and name one, and CI's ci-changes, which starts the core's build.
# It compares <path>s at commit <a> with commit <b>, or with the working tree for "-", and exits 0
# when they're the same to the compiler, 1 when they differ, and 2 when it can't tell. A file that
# differs only in comments is named on stderr. With --strip it prints the C on stdin as this
# compares it: comments out, each as one space, as the compiler takes them, runs of spaces as one,
# and no blank lines. String and character literals keep their own spacing.
# Usage: core-same.sh <a> <b>|- <path>...
#        core-same.sh --strip < file
# bash 3.2 runs it on the Mac, where perl is the system's, as on Linux and in Git Bash.
set -uo pipefail

strip() {
  # Lines joined by a backslash first, as the compiler does before it reads comments.
  perl -0777 -ne '
    s/\r\n/\n/g;
    s/\\\n//g;
    my $literal = qr/"(?:\\.|[^"\\\n])*"|\x27(?:\\.|[^\x27\\\n])*\x27/;
    s{($literal)|//[^\n]*|/\*.*?\*/}{defined $1 ? $1 : " "}gse;
    s{($literal)|[ \t\f]+}{defined $1 ? $1 : " "}ge;
    s/^ +| +$//mg;
    s/\n\n+/\n/g;
    s/^\n//;
    print;
  '
}

if [ "${1:-}" = --strip ]; then
  strip
  exit
fi
[ $# -ge 3 ] || {
  echo "usage: core-same.sh <a> <b>|- <path>... or core-same.sh --strip < file" >&2
  exit 2
}
cd "$(dirname "$0")/.." || exit 2
a=$1 b=$2
shift 2
if [ "$b" = - ]; then
  files=$(git diff --name-only --no-renames "$a" -- "$@") || exit 2
else
  files=$(git diff --name-only --no-renames "$a" "$b" -- "$@") || exit 2
fi
# $1 as $2 has it, the working tree for "-".
version() {
  if [ "$2" = - ]; then cat -- "$1"; else git show "$2:$1"; fi
}
same=0
while IFS= read -r file; do
  [ -n "$file" ] || continue
  case $file in
  *.h | *.hh | *.hpp | *.c | *.cc | *.cpp | *.m | *.mm)
    # A file only one side has differs, as git show or cat fails for the other.
    if left=$(version "$file" "$a" 2>/dev/null | strip) &&
      right=$(version "$file" "$b" 2>/dev/null | strip) && [ "$left" = "$right" ]; then
      echo "core-same: $file differs from ${a:0:12}'s only in comments and spacing" >&2
      continue
    fi
    ;;
  esac
  same=1
done <<EOF
$files
EOF
exit $same
