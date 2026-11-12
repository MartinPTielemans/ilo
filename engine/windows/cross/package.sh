#!/bin/bash
# Packs the Windows engine build.sh built into <output>.zip, a Chromium directory that fetch.sh
# unpacks: the files Chromium's FILES.cfg lists for an official x64 build, as
# ungoogled-chromium-windows packs them, and chrome.dll.lib, which the app links (app/windows).
# Beside it, <output>-symbols.zip has the PDB of each of its executables and libraries, for crash
# reports (tools/symbols.sh). With symbol_level=0 they name functions but hold no lines.
# Nothing of Microsoft's toolchain goes in but what FILES.cfg names from the SDK's redistributable
# files (D3DCompiler_47.dll), which Chrome ships too.
set -euo pipefail
[ $# -eq 2 ] || { echo "usage: package.sh <work directory> <output.zip>" >&2 && exit 2; }
src=$1/src
out=$src/out/Default
archive=$(realpath -m "$2")

fail() {
  echo "package: $*" >&2
  exit 1
}

# The app calls into chrome.dll through the C API, so one that doesn't export it can't run it.
nm=$src/third_party/llvm-build/Release+Asserts/bin/llvm-nm
symbols=$("$nm" "$out/chrome.dll.lib")
grep -qE '(^| )bc_main$' <<<"$symbols" ||
  fail "chrome.dll doesn't export the core's C API (core/BUILD.gn)"

# ilo's icon, not Chromium's, where Windows reads one: chrome.exe's first, for it and its shortcuts,
# its document icons, and chrome.dll's IDR_MAINFRAME, the browser windows' (tree.sh's
# windows_icons).
here=$(cd "$(dirname "$0")" && pwd)
ico=$here/../../../app/platform/windows/package/Assets/ilo.ico
while read -r file group; do
  python3 -I "$here/../../icons.py" group "$out/$file" "$group" "$ico" ||
    fail "$file's icon $group isn't ilo's"
done <<'ICONS'
chrome.exe 0
chrome.exe IDR_X006_HTML_DOC
chrome.exe IDR_X007_PDF_DOC
chrome.dll #101
ICONS

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
# A zip's top directory is named after it.
python3 "$1/ucw/ungoogled-chromium/utils/filescfg.py" -c "$src/chrome/tools/build/win/FILES.cfg" \
  --build-outputs "$out" --cpu-arch 64bit archive -o "$stage/Chromium.zip" -i "$out/chrome.dll.lib"
mv "$stage/Chromium.zip" "$archive"
sha256sum "$archive" | cut -d' ' -f1 >"$archive.sha256"
symbols=${archive%.zip}-symbols.zip
python3 -I - "$archive" "$out" "$symbols" <<'PY' || fail "the engine's symbols weren't packed"
import os, sys, zipfile
archive, out, symbols = sys.argv[1:]
with zipfile.ZipFile(archive) as engine:
    names = [os.path.basename(name) for name in engine.namelist() if name.endswith((".exe", ".dll"))]
pdbs = [name + ".pdb" for name in names if os.path.exists(os.path.join(out, name + ".pdb"))]
if "chrome.dll.pdb" not in pdbs:
    sys.exit("no chrome.dll.pdb in " + out)
with zipfile.ZipFile(symbols, "w", zipfile.ZIP_DEFLATED, allowZip64=True) as zip:
    for pdb in pdbs:
        zip.write(os.path.join(out, pdb), "Chromium/" + pdb)
PY
sha256sum "$symbols" | cut -d' ' -f1 >"$symbols.sha256"
echo "$archive"
