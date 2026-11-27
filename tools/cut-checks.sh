#!/bin/bash
# A build's functional checks on GitHub's hosted Macs, which the integrator starts at each cut of
# the rework line: it makes sure an engine-mac release fits the cut, the one engine/fetch.sh would
# take, and dispatches .github/workflows/check-mac.yml on the cut's branch. The engine may come from
# Engine on macOS (engine-mac.yml), which publishes its own, or from the Mac, where with the
# checkout on the cut this publishes the tree's build with tools/mac-engine-publish.sh. A cut whose
# core no engine has yet, with engine-mac.yml building it, waits for that run with --wait, and
# otherwise says so. The owner's Mac keeps engine builds, perf, adblock-cost and the live passes.
# Usage: cut-checks.sh [--kind look|full] [--wait] [--watch] [--dry] <branch|commit> [run...]
#        cut-checks.sh times <run id>
# A commit names the branch whose head it is. The runs are check-mac.yml's runs input: none for the
# default ones. --watch follows the check run to its end, then prints its times, as `times` does
# for any run: its wait for its first Mac runner, its time from there to its end, and each job's
# wait and length. --dry finds the engine and says what it would dispatch, and dispatches nothing.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
repo=$(cat "$root/app/branding/repository")
repo=${repo#https://github.com/}
engines=$(git -C "$root" config -f "$root/engine/prebuilt.ini" prebuilt.repository)
engines=${engines#https://github.com/}

fail() {
  echo "cut-checks: $*" >&2
  exit 1
}

# Each job of run $1 with its wait for a runner and its length, and the run's time split in two:
# queued, from its dispatch to its first Mac job's start, and checking, from there to its last
# job's end. A look build's gate takes whichever runner is free first by these.
times() {
  gh api "repos/$repo/actions/runs/$1" --jq '"\(.created_at) \(.status) \(.conclusion // "")"' |
    { read -r made status conclusion
      echo "run $1: $status ${conclusion:-}, made $made"
      gh api --paginate "repos/$repo/actions/runs/$1/jobs?per_page=100" --jq '.jobs[] |
        "\(.name)\t\(.conclusion // .status)\t\(.created_at)\t\(.started_at // "")\t\(.completed_at // "")\t\(.labels | join(" "))"' |
        python3 -I -c '
import sys, datetime
p = lambda s: datetime.datetime.fromisoformat(s.replace("Z", "+00:00")) if s and not s.startswith("0001") else None
made = p(sys.argv[1]); last = None; first = None
m = lambda d: "%d:%02d" % divmod(int(d.total_seconds()) // 60, 60) if d is not None else "-"
for line in sys.stdin:
    name, result, created, started, done, labels = line.rstrip("\n").split("\t")
    created, started, done = p(created), p(started), p(done)
    if done and started and (not last or done > last): last = done
    if started and "macos" in labels and (not first or started < first): first = started
    print("  %-28s %-10s waited %6s  ran %6s" % (name, result, m(started and started - created), m(done and started and done - started)))
# A run still going has no end yet: the last job to end so far may be its gate, before the build.
if sys.argv[2] != "completed": last = None
print("  queued, from dispatch to the first Mac job: %s (h:mm)" % m(first and first - made))
print("  checking, from there to the end of its last job: %s" % m(first and last and last - first))
print("  from dispatch to the end of its last job: %s" % m(last and last - made))
' "$made" "$status"
    }
}

if [ "${1:-}" = times ]; then
  [ -n "${2:-}" ] || fail "times needs a run id."
  times "$2"
  exit 0
fi

kind=look wait= watch= dry=
while [ $# -gt 0 ]; do
  case $1 in
  --kind) kind=${2:-} && shift 2 ;;
  --wait) wait=1 && shift ;;
  --watch) watch=1 && shift ;;
  --dry) dry=1 && shift ;;
  -*) fail "no option $1." ;;
  *) break ;;
  esac
done
case $kind in look | full) ;; *) fail "--kind is look or full." ;; esac
[ $# -ge 1 ] || { echo "usage: cut-checks.sh [--kind look|full] [--wait] [--watch] [--dry] <branch|commit> [run...] | times <run id>" >&2; exit 2; }
target=$1
shift
runs=$(echo "$*")

# The branch and its head: a commit names the branch on GitHub whose head it is, a build's cut
# before any other branch there.
if printf '%s' "$target" | grep -qE '^[0-9a-f]{7,40}$'; then
  branch=$(git ls-remote "https://github.com/$repo.git" 'refs/heads/*' |
    awk -v c="$target" 'index($1, c) == 1 { sub("refs/heads/", "", $2); print ($2 ~ /^scratch\/build-/ ? 0 : 1), $2 }' |
    sort | head -1 | cut -d' ' -f2)
  [ -n "$branch" ] || fail "no branch on $repo has ${target} at its head. Push the cut first."
else
  branch=$target
fi
sha=$(git ls-remote "https://github.com/$repo.git" "refs/heads/$branch" | cut -f1)
[ -n "$sha" ] || fail "$repo has no branch $branch."
git -C "$root" -c gc.auto=0 -c maintenance.auto=false fetch -q "https://github.com/$repo.git" "$sha" ||
  fail "couldn't fetch ${sha:0:12}."
[ "$(git -C "$root" rev-parse --is-shallow-repository)" = false ] ||
  fail "this clone is shallow, so the engines in ${sha:0:12}'s history can't be found. git fetch --unshallow first."

# The engine-mac release engine/fetch.sh takes on the Mac for the cut, when it has the cut's core:
# the nearest in its history whose core is the cut's.
fitting() {
  local ref tagged
  git ls-remote --tags "https://github.com/$engines.git" 'refs/tags/engine-mac-*' |
    while read -r _ ref; do
      case $ref in *"^{}") continue ;; esac
      tagged=$(git -C "$root" rev-parse -q --verify "${ref#refs/tags/engine-mac-}^{commit}" 2>/dev/null) || continue
      git -C "$root" merge-base --is-ancestor "$tagged" "$sha" 2>/dev/null || continue
      echo "$(git -C "$root" rev-list --count "$tagged..$sha") $tagged"
    done | sort -n | {
      nearest=
      while read -r _ tagged; do
        # As fetch.sh: only the nearest, and engines off its history, can have the cut's core.
        if [ -z "$nearest" ]; then
          nearest=$tagged
        elif git -C "$root" merge-base --is-ancestor "$tagged" "$nearest"; then
          continue
        fi
        if (cd "$root" && tools/core-same.sh "$tagged" "$sha" core engine/series engine/patches ':(exclude)*.md') 2>/dev/null; then
          # Its release has the engine itself, not only the tag.
          curl -fsSL --retry 5 -o /dev/null \
            "https://github.com/$engines/releases/download/engine-mac-${tagged:0:12}/Chromium-mac-arm64.tar.xz.sha256" &&
            echo "$tagged"
          break
        fi
      done
    }
}

engine=$(fitting)
while [ -z "$engine" ]; do
  if [ "$(uname -sm)" = "Darwin arm64" ] && [ "$(git -C "$root" rev-parse HEAD)" = "$sha" ]; then
    echo "cut-checks: no engine has ${sha:0:12}'s core, so this Mac publishes its build of it."
    "$root/tools/mac-engine-publish.sh"
  else
    # An Engine on macOS run building the cut, or a commit before it with its core.
    building=
    while read -r id head; do
      [ "$head" = "$sha" ] || {
        git -C "$root" -c gc.auto=0 fetch -q "https://github.com/$repo.git" "$head" 2>/dev/null &&
          git -C "$root" merge-base --is-ancestor "$head" "$sha" &&
          (cd "$root" && tools/core-same.sh "$head" "$sha" core engine/series engine/patches ':(exclude)*.md') 2>/dev/null
      } || continue
      building=$id
      break
    done < <(gh run list -R "$repo" -w engine-mac.yml -L 30 --json databaseId,headSha,status \
      --jq '.[] | select(.status != "completed") | "\(.databaseId) \(.headSha)"')
    [ -n "$building" ] ||
      fail "no engine-mac release has ${sha:0:12}'s core, and no Engine on macOS run is building it. Publish one with tools/mac-engine-publish.sh on the Mac with the checkout on it, or dispatch engine-mac.yml on $branch."
    [ -n "$wait" ] || {
      echo "cut-checks: Engine on macOS run $building is building the engine for ${sha:0:12}. Start again with --wait to dispatch the checks once it's published." >&2
      exit 2
    }
    echo "cut-checks: waiting for Engine on macOS run $building, which builds the engine for ${sha:0:12}."
    gh run watch "$building" -R "$repo" --interval 60 --exit-status > /dev/null ||
      fail "Engine on macOS run $building failed, so ${sha:0:12} has no engine to check on."
  fi
  engine=$(fitting)
  [ -n "$engine" ] || [ -z "${building:-}" ] ||
    fail "Engine on macOS run $building passed, but no engine-mac release has ${sha:0:12}'s core."
  [ -n "$engine" ] || fail "the publish left no engine-mac release with ${sha:0:12}'s core."
done
echo "cut-checks: $branch (${sha:0:12}) runs on engine-mac-${engine:0:12}."

# A cut from before check-mac.yml took a kind has the workflow without it, which refuses the input.
set -- -f "runs=$runs"
if git -C "$root" show "$sha:.github/workflows/check-mac.yml" | grep -q '^      kind:$'; then
  set -- "$@" -f "kind=$kind"
else
  [ "$kind" = look ] || fail "${sha:0:12}'s check-mac.yml has no kind input, so it can't check a full build."
fi
if [ -n "$dry" ]; then
  echo "cut-checks: would run: gh workflow run check-mac.yml -R $repo --ref $branch $*"
  exit 0
fi
# The run the dispatch makes is the branch's one that wasn't there before it.
listed() {
  gh run list -R "$repo" -w check-mac.yml -b "$branch" -e workflow_dispatch -L 10 \
    --json databaseId,headSha --jq ".[] | select(.headSha == \"$sha\") | .databaseId"
}
before=" $(listed | tr '\n' ' ') "
gh workflow run check-mac.yml -R "$repo" --ref "$branch" "$@"
run=
for _ in $(seq 30); do
  # GitHub lists a dispatched run a few seconds after it takes the dispatch.
  sleep 2
  for id in $(listed); do
    case $before in *" $id "*) ;; *) run=$id ;; esac
  done
  [ -z "$run" ] || break
done
[ -n "$run" ] || fail "dispatched check-mac.yml on $branch, but found no run of it. Look in https://github.com/$repo/actions/workflows/check-mac.yml."
echo "cut-checks: Check on macOS run $run, kind $kind, runs ${runs:-default}: https://github.com/$repo/actions/runs/$run"
[ -n "$watch" ] || exit 0
status=0
gh run watch "$run" -R "$repo" --interval 60 --exit-status > /dev/null || status=$?
times "$run"
exit "$status"
