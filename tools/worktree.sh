#!/bin/bash
# Makes a worktree ready to work in (#3917): the branch checked out at <dir>, then the app's setup,
# which shares the GPUI tree, the archives and the prebuilt engine through ~/.cache/ilo-setup, so
# only the first worktree downloads them. --from names the base of a branch neither this repository
# nor origin has, origin/main by default. --engine sets up the engine too, which the Mac's builds
# need and the unit tests don't. --test builds and runs the unit tests, at the cost of a cold
# compile and a target of its own. Each step says how long it took.
# Usage: tools/worktree.sh <dir> <branch> [--from <ref>] [--engine] [--test]
set -uo pipefail
usage() { echo "usage: tools/worktree.sh <dir> <branch> [--from <ref>] [--engine] [--test]" >&2; exit 2; }
[ $# -ge 2 ] || usage
dir=$1 branch=$2 from=origin/main engine= test=
shift 2
while [ $# -gt 0 ]; do
  case $1 in
  --from) [ $# -ge 2 ] || usage; from=$2; shift 2 ;;
  --engine) engine=1; shift ;;
  --test) test=1; shift ;;
  *) usage ;;
  esac
done
repo=$(cd "$(dirname "$0")/.." && pwd -P)
# The repository is shared with every other worktree, which a gc or maintenance started here would
# repack under them.
git() { command git -C "$repo" -c gc.auto=0 -c maintenance.auto=false "$@"; }

[ ! -e "$dir" ] || { echo "worktree: $dir is there already; pick a new folder" >&2; exit 1; }
parent=$(cd "$(dirname "$dir")" 2>/dev/null && pwd -P) ||
  { echo "worktree: $(dirname "$dir") doesn't exist" >&2; exit 1; }
if projects=$(cd "$HOME/Projects" 2>/dev/null && pwd -P); then
  case $parent/ in
  "$projects"/*)
    echo "worktree: $dir is inside ~/Projects, a managed workspace where no folder is made (AGENTS.md's hurt 9)" >&2
    exit 1 ;;
  esac
fi
dir=$parent/$(basename "$dir")
nl='
'
case "$nl$(git worktree list --porcelain)$nl" in
*"${nl}branch refs/heads/$branch$nl"*)
  echo "worktree: $branch is checked out in another worktree already; work there, or pick a new branch" >&2
  exit 1 ;;
esac

step() {
  local at=$SECONDS
  "$@" || {
    echo "worktree: $* failed${made:+; the worktree stays at $dir: git worktree remove --force $dir}" >&2
    exit 1
  }
  echo "worktree: $* took $((SECONDS - at)) s"
}
add() {
  if git show-ref -q --verify "refs/heads/$branch"; then
    git worktree add -q "$dir" "$branch"
  elif git ls-remote -q --exit-code --heads origin "$branch" > /dev/null; then
    git fetch -q origin "+refs/heads/$branch:refs/remotes/origin/$branch" &&
      git worktree add -q --track -b "$branch" "$dir" "origin/$branch"
  else
    case $from in origin/*) git fetch -q origin "+refs/heads/${from#origin/}:refs/remotes/$from" ;; esac &&
      git worktree add -q --no-track -b "$branch" "$dir" "$from"
  fi
}
setup() { (cd "$dir/app" && if [ -n "$engine" ]; then ./setup.sh; else ./setup.sh ui; fi); }
tests() { (cd "$dir/app" && cargo test --release --lib); }

made=
step add
made=1
step setup
[ -z "$test" ] || step tests
echo "worktree: ready at $dir on $branch. To remove it, and its target with it: git worktree remove $dir"
