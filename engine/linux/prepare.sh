#!/bin/bash
# Prepares Chromium for Linux in <work>/src, the way ungoogled-chromium-portablelinux's x64
# releases are built, at the tag pins.ini names: its source tarball, ungoogled-chromium's patches
# and domain substitution, its GN args and toolchain. Then links the core in as //browser_core and
# applies our series. Prints the tree's path. check-linux.sh and build.sh build from it, and
# .github/workflows/engine-linux.yml keeps one on its runner. Needs about 40 GB, and as much again
# while a new version comes in.
#
# A new tag is prepared whole in <work>/next, then copied over the tree by content (#299), so a
# point release leaves every file it doesn't change as it was and the next build recompiles only
# what changed. With the tag already in the tree, only the series and the GN args are brought up
# to date.
set -euo pipefail
[ $# -eq 1 ] || { echo "usage: prepare.sh <work directory>" >&2 && exit 2; }
work=$(mkdir -p "$1" && cd "$1" && pwd)
. "$(dirname "$0")/../tree.sh"
src=$work/src
# Their scripts log to stdout; this prints only the tree's path there.
exec 3>&1 1>&2

fail() {
  echo "prepare: $*" >&2
  exit 1
}

for tool in ninja node gperf go clang-format rsync; do
  command -v "$tool" >/dev/null || fail "$tool is missing"
done
# Where portablelinux's scripts expect npm's global esbuild.
[ -d /usr/lib/node_modules/esbuild ] || fail "esbuild is missing: sudo npm install -g --prefix /usr esbuild"

# setup_toolchain points the compiler flags at the tree it set up; these are where they started.
flags=("${CFLAGS:-}" "${CXXFLAGS:-}" "${CPPFLAGS:-}")

tag=$(git config -f "$engine/pins.ini" ungoogled-chromium-portablelinux.tag)
ucl=$work/ucl
if [ "$(git -C "$ucl" describe --tags --exact-match 2>/dev/null)" != "$tag" ]; then
  rm -rf "$ucl"
  git clone -q -c advice.detachedHead=false --depth 1 --branch "$tag" --recurse-submodules \
    --shallow-submodules https://github.com/ungoogled-software/ungoogled-chromium-portablelinux.git "$ucl"
fi
check_ungoogled "$ucl/ungoogled-chromium" "ungoogled-chromium-portablelinux $tag"

. "$ucl/scripts/shared.sh"
setup_paths

if [ "$(cat "$src/.portablelinux-tag" 2>/dev/null)" != "$tag" ]; then
  # Their scripts/build.sh, up to its build, in the scratch tree. Each step leaves a stamp there,
  # so a run that stopped goes on where it was.
  _src_dir=$work/next
  _out_dir=$_src_dir/out/Default
  fetch_sources false false
  apply_patches
  apply_domsub
  write_gn_args
  fix_tool_downloading
  setup_toolchain
  remove_agent_instructions "$_src_dir"
  apply_series "$_src_dir"
  echo "$tag" >"$_src_dir/.portablelinux-tag"
  if [ -d "$src" ]; then
    # Without the scratch tree's dates: a changed file gets the time it's written, so ninja sees
    # it as newer than what was built from it. An unchanged one isn't touched.
    rsync -rlpD --checksum --delete --exclude=/out/ "$_src_dir/" "$src/"
    rm -rf "$_src_dir" "$src/out/Default/gn"
  else
    mv "$_src_dir" "$src"
  fi
  rm -rf "$_dl_cache"
fi

_src_dir=$src
_out_dir=$src/out/Default
remove_agent_instructions "$src"
apply_series "$src"
# Rewritten only when they change, as a newer args.gn makes ninja run gn again.
args=$(mktemp -d)
trap 'rm -rf "$args"' EXIT
_out_dir=$args write_gn_args >/dev/null
# Google's Widevine module downloads at the user's yes (core/widevine.cc), with Chromium's Widevine
# support, which ungoogled-chromium's flags.gn turns on. Only a Chrome-branded build bundles it.
grep -qx 'enable_widevine=true' "$args/args.gn" || fail "ungoogled-chromium's GN args no longer turn Widevine on"
cmp -s "$args/args.gn" "$src/out/Default/args.gn" || cp "$args/args.gn" "$src/out/Default/args.gn"

# The toolchain as setup_toolchain gives it to gn's bootstrap, in the tree it's now in.
clang=$src/third_party/llvm-build/Release+Asserts/bin
export CC=$clang/clang CXX=$clang/clang++ AR=$clang/llvm-ar NM=$clang/llvm-nm LLVM_BIN=$clang
resource="-resource-dir=$("$CC" --print-resource-dir) -B$clang"
export CFLAGS="${flags[0]} $resource" CXXFLAGS="${flags[1]} $resource" CPPFLAGS="${flags[2]} $resource"

cd "$src"
[ -x out/Default/gn ] || ./tools/gn/bootstrap/bootstrap.py -o out/Default/gn --skip-generate-buildfiles
echo "$src" >&3
