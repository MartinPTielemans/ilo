#!/bin/bash
# The verdicts of the full checks (check-linux.yml, check-windows.yml, check-mac.yml), which run each
# night and when the integrator dispatches them on main after a merge train (#871).
#
# `main-check.sh seen` writes seen=true to GITHUB_OUTPUT when another run of this workflow on this
# branch already checked this commit, so a night with no new merge costs no runner. A run checked
# it when its job named "check", the one with the default runs, failed past its "Engine" step, or
# passed with its "Runs" step done: one with no engine for the commit, or cancelled, checked nothing.
#
# `main-check.sh rerun`, in a re-run's check job, fails when an earlier attempt of this run failed
# past its "Engine" step. A run's page, summary and conclusion are its last attempt's, and each
# attempt's artifact has the same name, so a re-run's pass would show over that failure, as run
# 41574039797's attempt 2 did over attempt 1 failing spaces (#1589). A new run checks again instead.
#
# `main-check.sh report <result> <found>` keeps one open issue per workflow, "<workflow> fails on
# main": a run that fails opens it, or comments on it if it's open, and one that passes with an
# engine found (<found> true) closes it. Other results, such as cancelled, leave it as it is.
# A run of check.sh that fails this verdict and the one before it on main, the night's or a
# train's, gets an issue of its own, "<workflow>: the <run> run fails on main twice running": fixed,
# the next verdict that passes closes it, and otherwise app/check/quarantine.<os> names it within a
# day, or Check scripts fails (app/check/quarantine.sh lint) (#2197). A quarantined failure is a
# warning, not an error, so it counts for neither.
# MAIN_CHECK_DRY=1 prints the issues it would write instead.
set -euo pipefail
repo=$GITHUB_REPOSITORY
runs=$GITHUB_SERVER_URL/$repo/actions/runs
run=$runs/$GITHUB_RUN_ID
gh() {
  if [ -n "${MAIN_CHECK_DRY:-}" ] && [ "$1" = issue ] && [ "$2" != list ]; then
    printf 'would run: gh'
    printf ' %q' "$@"
    echo
  else
    command gh "$@"
  fi
}

# The jobs with the default runs: the one named "check", or Windows' "check 1/2" and "check 2/2",
# which they're dealt across (#2618).
defaults='[.jobs[] | select(.name == "check" or (.name | test("^check [0-9]+/[0-9]+$")))]'

# Whether run $1 checked its commit: one of its default jobs failed past its "Engine" step, or each
# passed with its "Runs" step done.
checked() {
  gh api "repos/$repo/actions/runs/$1/jobs" --jq "$defaults"' as $jobs | ($jobs | length > 0) and
    (any($jobs[]; .conclusion == "failure"
        and (any(.steps[]; .name == "Engine" and .conclusion == "failure") | not))
      or all($jobs[]; .conclusion == "success" and any(.steps[]; .name == "Runs" and .conclusion == "success")))'
}

# The runs that run $1 failed, from its default jobs' errors, a line each: "tabs", or "tabs@wayland"
# as app/check/quarantine.linux names a session's.
failed_runs() {
  local job
  for job in $(gh api "repos/$repo/actions/runs/$1/jobs" --jq "$defaults"' | .[].id'); do
    gh api "repos/$repo/check-runs/$job/annotations" --jq '.[] | select(.annotation_level == "failure") | .message'
  done |
    sed -n -e 's/^The \([a-z0-9-]*\) run failed on \([a-z]*\)\..*/\1@\2/p' \
      -e 's/^The \([a-z0-9-]*\) run failed\..*/\1/p' \
      -e 's/^Drops under Wayland failed\..*/drops/p' -e 's/^Input methods failed\..*/ime/p' | sort -u
}

case ${1:-} in
seen)
  workflow=$(gh api "repos/$repo/actions/runs/$GITHUB_RUN_ID" --jq .workflow_id)
  for id in $(gh api "repos/$repo/actions/workflows/$workflow/runs?head_sha=$GITHUB_SHA&branch=$GITHUB_REF_NAME&status=completed&per_page=100" \
    --jq ".workflow_runs[] | select(.id != $GITHUB_RUN_ID) | .id"); do
    if [ "$(checked "$id")" = true ]; then
      echo "$runs/$id already checked ${GITHUB_SHA:0:12} on $GITHUB_REF_NAME." | tee -a "$GITHUB_STEP_SUMMARY"
      echo "seen=true" >> "$GITHUB_OUTPUT"
      exit 0
    fi
  done
  echo "No run has checked ${GITHUB_SHA:0:12} on $GITHUB_REF_NAME yet."
  echo "seen=false" >> "$GITHUB_OUTPUT"
  ;;
rerun)
  attempt=1
  while [ "$attempt" -lt "$GITHUB_RUN_ATTEMPT" ]; do
    failed=$(gh api "repos/$repo/actions/runs/$GITHUB_RUN_ID/attempts/$attempt/jobs" --jq 'any(.jobs[];
      (.name | startswith("check")) and .conclusion == "failure"
        and (any(.steps[]; .name == "Engine" and .conclusion == "failure") | not))')
    if [ "$failed" = true ]; then
      echo "::error::Attempt $attempt of this run failed, so a pass here would show over it: start a new run to check again. $run/attempts/$attempt"
      printf 'Failed: [attempt %s](%s) of this run failed, so a pass here would show over it. Start a new run to check again.\n' \
        "$attempt" "$run/attempts/$attempt" >> "$GITHUB_STEP_SUMMARY"
      exit 1
    fi
    attempt=$((attempt + 1))
  done
  ;;
report)
  result=$2 found=$3
  title="$GITHUB_WORKFLOW fails on main"
  issue=$(gh issue list --repo "$repo" --state open --search "in:title \"$title\"" --json number,title \
    --jq "map(select(.title == \"$title\"))[0].number // empty")
  case $result in
  failure)
    # What each failed job said went wrong, or else its failed steps.
    failed=
    for job in $(gh api "repos/$repo/actions/runs/$GITHUB_RUN_ID/jobs" --jq '.jobs[] | select(.conclusion == "failure") | .id'); do
      said=$(gh api "repos/$repo/check-runs/$job/annotations" --jq '.[] | select(.annotation_level == "failure"
        and (.message | startswith("Process completed with exit code") | not)) | "- " + .message')
      # shellcheck disable=SC2016 # $job is jq's.
      [ -n "$said" ] || said=$(gh api "repos/$repo/actions/jobs/$job" --jq '.name as $job | .steps[]
        | select(.conclusion == "failure") | "- \($job): the \(.name) step failed"')
      failed+=$said$'\n'
    done
    body="[A run]($run) ($GITHUB_EVENT_NAME) failed on main at $GITHUB_SHA.

$failed"
    if [ -n "$issue" ]; then
      gh issue comment "$issue" --repo "$repo" --body "$body"
    else
      gh issue create --repo "$repo" --title "$title" --body "$body
The next run of $GITHUB_WORKFLOW on main that passes closes this issue (.github/scripts/main-check.sh)."
    fi
    ;;
  success)
    if [ "$found" = true ] && [ -n "$issue" ]; then
      gh issue close "$issue" --repo "$repo" --comment "[It passes]($run) on main at $GITHUB_SHA."
    fi
    ;;
  esac
  case $GITHUB_WORKFLOW in
  *macOS) os=mac ;;
  *Windows) os=windows ;;
  *) os=linux ;;
  esac
  twice=" fails on main twice running"
  open=$(gh issue list --repo "$repo" --state open --search "in:title \"$GITHUB_WORKFLOW: the\" in:title \"$twice\"" \
    --json number,title --jq ".[] | select(.title | startswith(\"$GITHUB_WORKFLOW: the \")) | \"\(.number)\t\(.title)\"")
  case $result in
  failure)
    # This verdict's failures that the one before it on main had too.
    read -r workflow created < <(gh api "repos/$repo/actions/runs/$GITHUB_RUN_ID" --jq '"\(.workflow_id) \(.created_at)"')
    before=
    for id in $(gh api "repos/$repo/actions/workflows/$workflow/runs?branch=$GITHUB_REF_NAME&status=completed&created=<$created&per_page=30" \
      --jq '.workflow_runs[].id'); do
      if [ "$(checked "$id")" = true ]; then
        before=$id
        break
      fi
    done
    [ -n "$before" ] || exit 0
    comm -12 <(failed_runs "$GITHUB_RUN_ID") <(failed_runs "$before") | while read -r what; do
      on=
      [ "${what#*@}" = "$what" ] || on=" on ${what#*@}"
      title="$GITHUB_WORKFLOW: the ${what%@*} run$on$twice"
      body="[This run]($run) and [the one before it]($runs/$before) on main both failed the ${what%@*} run$on, at $GITHUB_SHA."
      number=$(printf '%s\n' "$open" | awk -F '\t' -v t="$title" '$2 == t { print $1 }')
      if [ -n "$number" ]; then
        gh issue comment "$number" --repo "$repo" --body "$body"
      else
        gh issue create --repo "$repo" --title "$title" --body "$body

Fix it today: the next run of $GITHUB_WORKFLOW on main that passes closes this issue. Or quarantine it under this issue, with a line in app/check/quarantine.$os that starts \`$what #<this issue> <today>\` (app/check/quarantine.sh says how). A day after this issue opens with neither done, Check scripts fails (#2197)."
      fi
    done
    ;;
  success)
    # Passed, unless quarantined: a quarantined run's issue stays open while its line names it.
    [ "$found" = true ] || exit 0
    printf '%s\n' "$open" | while IFS=$'\t' read -r number title; do
      [ -n "$number" ] || continue
      ! grep -q "[[:space:]]#$number[[:space:]]" app/check/quarantine.$os 2> /dev/null || continue
      gh issue close "$number" --repo "$repo" --comment "[It passes]($run) on main at $GITHUB_SHA."
    done
    ;;
  esac
  ;;
*) echo "usage: main-check.sh seen | rerun | report <result> <found>" >&2 && exit 2 ;;
esac
