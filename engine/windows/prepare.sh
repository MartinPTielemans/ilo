#!/bin/bash
# Prepares ungoogled-chromium-windows at the tag pins.ini names, in the directory its CI builds in,
# to build our engine: its build.py with three hooks, and package.py with one. After its patches and
# domain substitution, it runs series.sh (the core and our series). It builds what ILO_TARGETS
# names: chrome, or with ILO_CORE_ONLY the core's own objects, the compile check. And beside its
# package it leaves chrome.dll.lib, which the app links. Run by .github/workflows/engine-windows.yml,
# in Git Bash.
set -euo pipefail
[ $# -eq 1 ] || { echo "usage: prepare.sh <directory>" >&2 && exit 2; }
ucw=$1
engine=$(cd "$(dirname "$0")/.." && pwd)

ucw_tag=$(git config -f "$engine/pins.ini" ungoogled-chromium-windows.tag)
git clone -q -c advice.detachedHead=false -c core.autocrlf=false --depth 1 --branch "$ucw_tag" \
  --recurse-submodules --shallow-submodules \
  https://github.com/ungoogled-software/ungoogled-chromium-windows.git "$ucw"
. "$engine/tree.sh"
check_ungoogled "$ucw/ungoogled-chromium" "ungoogled-chromium-windows $ucw_tag"
# PlayReady, which Chromium turns on only for Chrome's own builds, plays protected video on
# Windows (decision 1). build.py writes this file's args after ungoogled-chromium's.
printf '\nenable_playready=true\n' >>"$ucw/flags.windows.gn"

# Each hook replaces lines a script has at this tag, build.py unless it names another, and fails if
# they're gone.
hook() {
  local anchor=$1 replacement=$2 script=${3:-build.py}
  ANCHOR=$anchor REPLACEMENT=$replacement SCRIPT=$script perl -0pi -e \
    '$n = s/\Q$ENV{ANCHOR}\E/$ENV{REPLACEMENT}/; die "$ENV{SCRIPT} has no \"$ENV{ANCHOR}\"\n" unless $n' \
    "$ucw/$script"
}
hook "    # Check if rust-toolchain folder has been populated" \
"    # ilo: the core and our series, once (engine/windows/prepare.sh).
    if not (source_tree / 'browser_core').exists():
        subprocess.run([os.environ['ILO_BASH'], os.environ['ILO_ENGINE'] + '/windows/series.sh',
                        str(source_tree)], check=True)

    # Check if rust-toolchain folder has been populated"
hook "    ninja_commandline.append('chrome')
    ninja_commandline.append('chromedriver')
    ninja_commandline.append('mini_installer')" \
"    if os.environ.get('ILO_CORE_ONLY'):
        # Only the core's own objects, those its BUILD.gn has on Windows, as ninja knows no other.
        # Its thread count is the int argparse gave it.
        graph = subprocess.run([str(arg) for arg in ninja_commandline] + ['-t', 'targets', 'all'],
                               capture_output=True, text=True, check=True).stdout
        targets = [line.split(': ')[0] for line in graph.splitlines()]
        core = [t for t in targets if t.startswith('obj/browser_core/') and t.endswith('.obj')]
        # With no target, ninja would build its default, the whole browser.
        if not core:
            raise RuntimeError('ninja has no objects of the core')
        ninja_commandline += core
    else:
        ninja_commandline.append('chrome')"
hook "        subprocess.run([sys.executable, 'package.py', '--cpu-arch', '32bit' if args.x86 else 'arm' if args.arm else '64bit'])" \
"        if not os.environ.get('ILO_CORE_ONLY'):
            subprocess.run([sys.executable, 'package.py', '--cpu-arch', '64bit'], check=True)
            shutil.copy2('build/src/out/Default/chrome.dll.lib', 'build/ungoogled-chromium_chrome.dll.lib')"
# package.py copies the installer out first, but only chrome is built, and the package it zips
# leaves the installer out anyway, so without this the last stage fails after the build.
hook "    shutil.copyfile('build/src/out/Default/mini_installer.exe',
        'build/ungoogled-chromium_{}-{}.{}_installer_{}.exe'.format(
            get_chromium_version(), _get_release_revision(),
            _get_packaging_revision(), _get_target_cpu(build_outputs)))
" "" package.py
