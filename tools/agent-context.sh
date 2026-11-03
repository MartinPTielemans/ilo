#!/bin/bash
# The size of what an agent reads first: AGENTS.md and the docs in this repo it links to, in bytes,
# at a commit (HEAD by default). agent-context-budget.yml compares a pull request's with main's.
set -euo pipefail
# Scratch only (#1506 proof): holds the budget run open so a label lands while it is in progress.
sleep 60

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
