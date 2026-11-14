#!/bin/bash
# Gives the one engine runner to a release batch's engine runs ahead of lanes' (#2332). The runner
# takes any queued job its labels match, not the oldest, and GitHub has no priority for self-hosted
# jobs. So a lane's build job that gets the runner while a release waits hands the runner back.
# engine-linux.yml, engine-windows-cross.yml and core-tests.yml fetch this through the contents API
# before any checkout, so asking costs the runner seconds.
# Usage:
#   engine-yield.sh yield: the first step of a lane's build job. With a release waiting, it
#     dispatches the run's workflow again on its ref with yielded=true, and cancels its own run.
#   engine-yield.sh wait: the Changes job of that retry. It waits on GitHub's runner until no
#     release waits, so the retry's build job joins the runner's queue only once the release has
#     the runner. A lane gives way once for each release, never each time the runner frees.
# A release is a run on engine-build/batch-<v>, the integrator's batches. It waits from when it's
# queued until its build job starts: held in its concurrency group, deciding in Changes, or queued
# for the runner. When the API can't be read, nothing waits, and the lane builds as it did before.
set -u
repo=$GITHUB_REPOSITORY

# The first release waiting for the runner, by its run id, or nothing.
waiting() {
  runs=$(for workflow in engine-linux.yml engine-windows-cross.yml; do
    gh api "repos/$repo/actions/workflows/$workflow/runs?per_page=50" --jq '.workflow_runs[]
      | select(.status != "completed" and (.head_branch | startswith("engine-build/batch-"))) | .id'
  done)
  for run in $runs; do
    building=$(gh api "repos/$repo/actions/runs/$run/jobs" --jq '[.jobs[]
      | select(.name == "build" and (.status == "in_progress" or .status == "completed"))] | length')
    [ "$building" = 0 ] && echo "$run" && return
  done
}

case ${1:-} in
yield)
  release=$(waiting)
  [ -n "$release" ] || { echo "engine-yield: no release waits for the runner"; exit 0; }
  workflow=${GITHUB_WORKFLOW_REF%@*}
  workflow=${workflow##*/}
  # Without the retry it builds now, rather than leave the ref with no build at all.
  # The run's other inputs go to the retry too, as name=value lines in ENGINE_YIELD_INPUTS, such as
  # core-tests.yml's filter.
  inputs=(-f yielded=true)
  while IFS= read -r input; do
    [ -z "$input" ] || inputs+=(-f "$input")
  done <<EOF
${ENGINE_YIELD_INPUTS:-}
EOF
  gh workflow run "$workflow" -R "$repo" --ref "$GITHUB_REF_NAME" "${inputs[@]}" || exit 0
  echo "Gave the engine runner to release run [$release]($GITHUB_SERVER_URL/$repo/actions/runs/$release), and dispatched $workflow on $GITHUB_REF_NAME again, which builds once the release has the runner." |
    tee -a "$GITHUB_STEP_SUMMARY"
  gh run cancel "$GITHUB_RUN_ID" -R "$repo"
  # Until the cancel lands, no step after this one may start.
  sleep 120
  exit 1
  ;;
wait)
  # Five hours at most, beyond a release's wait for the one lane build it lets finish; after that
  # the retry joins the queue anyway, so a release that never starts can't hold lanes for good.
  for _ in $(seq 300); do
    release=$(waiting)
    [ -n "$release" ] || { echo "engine-yield: no release waits for the runner"; exit 0; }
    echo "engine-yield: release run $release waits for the runner"
    sleep 60
  done
  echo "::notice::Release run $release still waits for the engine runner after 5 hours, so this build joins the runner's queue anyway."
  ;;
*)
  echo "usage: engine-yield.sh yield|wait" >&2
  exit 2
  ;;
esac
