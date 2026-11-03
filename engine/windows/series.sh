#!/bin/bash
# What setup.sh does to the tree after ungoogled-chromium's stages, for build.py on Windows
# (prepare.sh): drops Chromium's agent instructions, puts the core in as //browser_core, applies
# our series and puts ilo's icon in Chromium's place. Git Bash's ln copies, so the tree has the core
# as it was at this commit.
set -euo pipefail
src=$(cygpath -u "$1")
. "$(dirname "$0")/../tree.sh"
remove_agent_instructions "$src"
apply_series "$src"
windows_icons "$src"
