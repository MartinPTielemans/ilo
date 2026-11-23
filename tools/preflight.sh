#!/bin/bash
# The static checks CI spreads over its workflows, in one run of a few seconds before a push (#3917):
#
#   rustfmt      app's Rust as UI on Linux's fmt job checks it: rustfmt --check on the files git tracks
#   lints        tools/check-lints.py, as Check scripts' lints job runs it
#   scripts      each shell script changed from the base parses: app/check's as Check scripts' bash
#                3.2 job does (bash --posix and dash for sh, no bash 4 features, and bash 3.2 itself
#                when $BASH32 names one), any other by the shell its first line names
#
# Every check runs, each says ok or FAIL, and the script fails if any did. CI keeps its own jobs:
# this is a quicker read of the same rules, not a stand-in for them.
#
#   tools/preflight.sh [<base>]   checks the tree; scripts changed since <base> (default: where
#                                 HEAD left origin/main), committed or not
#   tools/preflight.sh --all      every script, not only the changed ones
#   tools/preflight.sh --hook     runs it before each git push from this clone and its worktrees,
#                                 which share hooks; `git push --no-verify` skips it once
#   tools/preflight.sh --unhook   takes that hook out again
set -u
top=$(git rev-parse --show-toplevel) || exit 2
cd "$top" || exit 2

hook=$(git rev-parse --git-path hooks/pre-push)
mark="# ilo: tools/preflight.sh"
case ${1:-} in
--hook)
  if [ -e "$hook" ] && ! grep -qF "$mark" "$hook"; then
    echo "preflight: $hook is another hook; it stays as it is" >&2
    exit 1
  fi
  printf '#!/bin/sh\n%s\nexec "$(git rev-parse --show-toplevel)/tools/preflight.sh"\n' "$mark" > "$hook" &&
    chmod +x "$hook" && echo "preflight: runs before each push, from $hook"
  exit
  ;;
--unhook)
  if [ -e "$hook" ] && grep -qF "$mark" "$hook"; then
    rm "$hook" && echo "preflight: no longer runs before a push"
  else
    echo "preflight: there's no preflight hook in $hook"
  fi
  exit
  ;;
--all) base= ;;
-*)
  echo "usage: tools/preflight.sh [<base> | --all | --hook | --unhook]" >&2
  exit 2
  ;;
*)
  base=${1:-$(git merge-base HEAD origin/main 2> /dev/null)} || base=
  if [ -z "$base" ]; then
    echo "preflight: no base to find changed scripts from; name one, or say --all" >&2
    exit 2
  fi
  ;;
esac

failed=
ok() { echo "ok: $1"; }
fail() {
  echo "FAIL: $1"
  failed="$failed $2"
}

# rustfmt, as app-linux.yml's fmt job runs it, in four batches at once and beside the checks below:
# alone, one file after another, it takes as long as all the rest.
out=$(mktemp "${TMPDIR:-/tmp}/preflight.XXXXXX") || exit 2
trap 'rm -f "$out"' EXIT
if command -v rustfmt > /dev/null; then
  git ls-files -z 'app/*.rs' | xargs -0 -n 25 -P 4 rustfmt --check > "$out" 2>&1 &
  formatting=$!
else
  formatting=
fi

# The lints, as check-scripts.yml's lints job runs them.
if python3 -I tools/check-lints.py; then
  ok "lints"
else
  fail "lints: the findings above" lints
fi

if [ -z "$formatting" ]; then
  fail "rustfmt isn't installed: rustup component add rustfmt" rustfmt
elif wait "$formatting"; then
  ok "rustfmt"
else
  sed -n '1,40p' "$out"
  fail "rustfmt: the lines above aren't formatted; cargo fmt in app/ formats them" rustfmt
fi

# The scripts: app/check's, check.sh and the Sparkle proof's as check-scripts.yml's bash 3.2 job
# parses them; any other by its own shell.
if [ -z "$base" ]; then
  scripts=$(git ls-files -- '*.sh')
  said="every script"
else
  scripts=$( (git diff --name-only --diff-filter=d "$base" -- '*.sh'; git ls-files --others --exclude-standard -- '*.sh') | sort -u)
  said="scripts changed since $(git rev-parse --short "$base")"
fi
bad= n=0
for file in $scripts; do
  [ -f "$file" ] || continue
  n=$((n + 1))
  first=$(head -n 1 "$file")
  case $file in
  app/check/git-bash.sh) # Sourced on Windows alone, by Git Bash's bash 5.
    bash -n "$file" || bad="$bad $file" ;;
  app/check.sh | app/check/*.sh | .github/sparkle-mac-proof/*.sh)
    if [ "$first" = '#!/bin/bash' ]; then mode=; else mode=--posix; fi
    bash $mode -n "$file" || bad="$bad $file"
    if [ -n "${BASH32:-}" ]; then "$BASH32" $mode -n "$file" || bad="$bad $file(bash-3.2)"; fi
    if [ -n "$mode" ] && command -v dash > /dev/null; then dash -n "$file" || bad="$bad $file(dash)"; fi
    if grep -nE '^[^#]*(declare -A|local -A|typeset -A|\<mapfile\>|\<readarray\>|\<coproc\>|\$\{[A-Za-z_][A-Za-z0-9_]*(\[[^]]*\])?(,|\^))' "$file"; then
      bad="$bad $file(bash-4)"
    fi
    ;;
  *)
    case $first in
    '#!/bin/sh' | '#!/usr/bin/env sh') sh -n "$file" || bad="$bad $file" ;;
    *zsh*) command -v zsh > /dev/null && { zsh -n "$file" || bad="$bad $file"; } ;;
    *) bash -n "$file" || bad="$bad $file" ;;
    esac
    ;;
  esac
done
if [ -n "$bad" ]; then
  fail "scripts: these don't parse, or use what the Mac's bash 3.2 lacks:$bad" scripts
else
  ok "scripts: $n parse ($said)"
fi

if [ -n "$failed" ]; then
  echo "preflight: failed:$failed"
  exit 1
fi
echo "preflight: all passed"
