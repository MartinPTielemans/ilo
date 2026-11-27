#!/bin/bash
# The four-part version a Windows package carries, the MSIX's identity and the installer's file
# version: 1.0.<build>.0, the build being the count of commits, as the updaters' is (app/build.rs).
# Windows and the Store refuse a package below the one installed, and the shown version may start
# again (0.3.0 to 0.1.0, #4086), so this climbs with the build alone. The 1 puts it above every
# 0.x.y.0 a release had before; the 0 last is the Store's.
# Usage: version.sh [<commit>], in the repository; HEAD by default.
set -euo pipefail
echo "1.0.$(git rev-list --count "${1:-HEAD}").0"
