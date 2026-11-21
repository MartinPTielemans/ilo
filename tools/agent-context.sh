#!/bin/bash
# The size of what an agent reads first: AGENTS.md and the docs in this repo it links to, in bytes,
# at a commit (HEAD by default). agent-context-budget.yml compares a pull request's with main's.
#
# With --source, the source files over 2,000 lines at that commit instead, largest first, with a
# total: an agent reads such a file whole before changing it, and two changes to it conflict
# (#2565). It only reports, and never fails on them. Left out, as no one writes them by hand
# here: lockfiles and *.gen.* files, which tools write; patches, regenerated from the crates in
# app/.deps (AGENTS.md); anything under a .deps, vendor, third_party or node_modules directory;
# binary files; and Markdown, which the budget above measures in bytes.
# Usage: tools/agent-context.sh [REV] | tools/agent-context.sh --source [REV]
set -euo pipefail

if [ "${1:-}" = --source ]; then
  rev=${2:-HEAD}
  limit=2000
  if ! git rev-parse --verify --quiet "$rev^{commit}" >/dev/null; then
    echo "agent-context.sh: $rev is no commit here." >&2
    exit 1
  fi
  # git grep -c '' counts each text file's lines, and -I passes over binary ones. It answers 1
  # when no file is left, which isn't a failure here.
  {
    git grep -I -c '' "$rev" -- . \
      ':(exclude,glob)**/*.lock' ':(exclude,glob)**/*-lock.yaml' ':(exclude,glob)**/*-lock.json' \
      ':(exclude,glob)**/*.gen.*' ':(exclude,glob)**/*.patch' ':(exclude,glob)**/*.md' \
      ':(exclude,glob)**/.deps/**' ':(exclude,glob)**/vendor/**' \
      ':(exclude,glob)**/third_party/**' ':(exclude,glob)**/node_modules/**' || [ $? = 1 ]
  } | awk -F : -v prefix="$rev:" -v limit="$limit" '
    {
      lines = $NF
      path = substr($0, length(prefix) + 1, length($0) - length(prefix) - length(lines) - 1)
      if (lines > limit) print lines "\t" path
    }' | sort -t "$(printf '\t')" -k1,1nr -k2,2 | awk -F '\t' -v rev="$rev" -v limit="$limit" '
    NR == 1 { printf "Source files over %d lines at %s, largest first:\n", limit, rev }
    { printf "%8d  %s\n", $1, $2; total += $1 }
    END {
      if (NR == 0) printf "No source file is over %d lines at %s.\n", limit, rev
      else printf "%8d  total, %d files\n", total, NR
    }'
  exit 0
fi

rev=${1:-HEAD}
files=$(
  {
    echo AGENTS.md
    git show "$rev:AGENTS.md" | grep -o '](\([^)#]*\.md\)' | sed 's/^](//'
  } | grep -v '://' | awk '!seen[$0]++'
)
total=0
for file in $files; do
  bytes=$(git cat-file -s "$rev:$file" 2>/dev/null || echo 0)
  printf '%8d  %s\n' "$bytes" "$file"
  total=$((total + bytes))
done
printf '%8d  total\n' "$total"
