#!/bin/bash
# Packs the Linux engine build.sh built into <output>.tar.xz, a Chromium directory that fetch.sh
# unpacks: chrome with the core in it, and the files it reads beside it, as
# ungoogled-chromium-portablelinux's scripts/package.sh lists them. The app's library goes beside
# chrome (platform/app_library.h in core/).
set -euo pipefail
[ $# -eq 2 ] || { echo "usage: package.sh <work directory> <output.tar.xz>" >&2 && exit 2; }
out=$1/src/out/Default
archive=$(realpath -m "$2")

fail() {
  echo "package: $*" >&2
  exit 1
}

# The app calls into chrome through the C API, so a chrome that doesn't export it can't run one.
# Read whole before it's searched: grep -q stops at the first match, and under pipefail nm's
# broken pipe would fail the check.
nm=$out/../../third_party/llvm-build/Release+Asserts/bin/llvm-nm
exports=$("$nm" -D --defined-only "$out/chrome")
grep -q ' T bc_main$' <<<"$exports" ||
  fail "chrome doesn't export the core's C API (chrome.map, core/BUILD.gn's linux_exports)"
# And libwayland's, which the app's UI calls on Chromium's display.
grep -q ' T wl_display_dispatch_queue_pending$' <<<"$exports" ||
  fail "chrome doesn't export libwayland's client API (chromium-core-wayland.patch, linux_exports)"

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir "$stage/Chromium"
for file in chrome chrome_100_percent.pak chrome_200_percent.pak chrome_crashpad_handler icudtl.dat \
  libEGL.so libGLESv2.so libvk_swiftshader.so libvulkan.so.1 locales resources.pak \
  v8_context_snapshot.bin vk_swiftshader_icd.json; do
  cp -r "$out/$file" "$stage/Chromium/"
done
# Chromium's version, which Settings › About reads beside chrome (app/src/build_info.rs), as
# Windows's build names a manifest after it.
version=$(sed -n 's/^\(MAJOR\|MINOR\|BUILD\|PATCH\)=//p' "$1/src/chrome/VERSION" | paste -sd.)
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]] ||
  fail "src/chrome/VERSION gave no version: $version"
echo "$version" >"$stage/Chromium/VERSION"
# Qt's shims, for the system's file dialogs on KDE, where the build made them.
for file in libqt5_shim.so libqt6_shim.so; do
  [ ! -e "$out/$file" ] || cp "$out/$file" "$stage/Chromium/"
done
XZ_OPT=-T0 tar -cJf "$archive" -C "$stage" Chromium
sha256sum "$archive" | cut -d' ' -f1 >"$archive.sha256"
echo "$archive"
