#!/bin/bash
# Prepares Chromium for Windows in <work>/src, cross-compiled on x64 Linux as Chromium's
# docs/win_cross.md says, at the tags pins.ini names: the source tarball as
# ungoogled-chromium-portablelinux unpacks it, with ungoogled-chromium's patches and
# ungoogled-chromium-windows's, the Windows-only sources and GN args the latter adds, and Chrome's
# win64 optimization profile. ungoogled-chromium-windows builds on Windows with its own LLVM and
# Rust; here the tools are portablelinux's (Google's Linux clang and Rust, and the patches it needs
# for the host), so the Windows patches that only serve its host or its toolchain are left out.
# Microsoft's compiler, linker and SDK come from the zip in $ILO_WIN_TOOLCHAIN (engine/README.md,
# Windows). Then links the core in as //browser_core, applies our series and puts ilo's icon in
# Chromium's place (tree.sh). Prints the tree's path. build.sh builds from it, and
# .github/workflows/engine-windows-cross.yml keeps one on its runner. Needs about 40 GB, and as much
# again while a new version comes in.
#
# As in linux/prepare.sh, a new tag is prepared whole in <work>/next and copied over the tree by
# content, so the next build recompiles only what changed.
set -euo pipefail
[ $# -eq 1 ] || { echo "usage: prepare.sh <work directory>" >&2 && exit 2; }
work=$(mkdir -p "$1" && cd "$1" && pwd)
. "$(dirname "$0")/../../tree.sh"
src=$work/src
# Their scripts log to stdout; this prints only the tree's path there.
exec 3>&1 1>&2

fail() {
  echo "prepare: $*" >&2
  exit 1
}

# Microsoft's toolchain, checked before anything is downloaded: the zip
# depot_tools/win_toolchain/package_from_installed.py made, named by its hash, and vs_files beside
# it, a case-insensitive mount (ciopfs) for the SDK, whose headers include each other in any case.
toolchain=${ILO_WIN_TOOLCHAIN:-}
[ -n "$toolchain" ] || fail "ILO_WIN_TOOLCHAIN isn't set: the directory with the Windows toolchain's zip (engine/README.md, Windows)"
zips=("$toolchain"/*.zip)
[ ${#zips[@]} -eq 1 ] && [ -f "${zips[0]}" ] ||
  fail "$toolchain should hold one zip, the Windows toolchain (engine/README.md, Windows)"
toolchain_hash=$(basename "${zips[0]}" .zip)
mountpoint -q "$toolchain/vs_files" ||
  fail "$toolchain/vs_files isn't mounted case-insensitively (engine/README.md, Windows)"

for tool in ninja node gperf go clang-format rsync tsc 7z; do
  command -v "$tool" >/dev/null || fail "$tool is missing"
done
[ -d /usr/lib/node_modules/esbuild ] || fail "esbuild is missing: sudo npm install -g --prefix /usr esbuild"

flags=("${CFLAGS:-}" "${CXXFLAGS:-}" "${CPPFLAGS:-}")

clone() {
  local name=$1 tag=$2 dir=$3
  [ "$(git -C "$dir" describe --tags --exact-match 2>/dev/null)" = "$tag" ] && return
  rm -rf "$dir"
  git clone -q -c advice.detachedHead=false --depth 1 --branch "$tag" --recurse-submodules \
    --shallow-submodules "https://github.com/ungoogled-software/$name.git" "$dir"
}
ucw_tag=$(git config -f "$engine/pins.ini" ungoogled-chromium-windows.tag)
ucl_tag=$(git config -f "$engine/pins.ini" ungoogled-chromium-portablelinux.tag)
ucw=$work/ucw
ucl=$work/ucl
clone ungoogled-chromium-windows "$ucw_tag" "$ucw"
clone ungoogled-chromium-portablelinux "$ucl_tag" "$ucl"
check_ungoogled "$ucw/ungoogled-chromium" "ungoogled-chromium-windows $ucw_tag"
[ "$(git -C "$ucw/ungoogled-chromium" rev-parse HEAD)" = "$(git -C "$ucl/ungoogled-chromium" rev-parse HEAD)" ] ||
  fail "ungoogled-chromium-windows $ucw_tag and ungoogled-chromium-portablelinux $ucl_tag build different ungoogled-chromium commits"

. "$ucl/scripts/shared.sh"
setup_paths

# The patches after ungoogled-chromium's. Each one left out is named, so a tag that drops or renames
# one stops here to be looked at again.
windows_skipped=(
  # Its host's Microsoft rc.exe, in place of rc.py, which runs here with Google's Linux rc.
  windows-disable-rcpy windows-fix-rc windows-fix-rc-terminating-error
  # Building gn and finding Python on Windows.
  windows-fix-building-gn windows-fix-python
  # Its host runs midl and mc again; here, as in Chromium's cross builds, their outputs are copied.
  windows-disable-win-build-output
  # Its own LLVM, Rust and Windows TypeScript.
  windows-fix-building-with-rust windows-fix-complex-builtins windows-disable-clang-version-check
  windows-fix-unsupported-llvm-flags windows-fix-typescript-lib-dom
)
# portablelinux's patches for a Linux host's Python, Node, esbuild and TypeScript.
host_patches=(
  chromium-138-drop-nodejs-version-check chromium-153-revert-hermetic-python
  chromium-154-use-system-tsc chromium-155-use-system-esbuild chromium-156-revert-use-typescript-go
)
patches=$work/patches
rm -rf "$patches"
mkdir -p "$patches/ungoogled-chromium"
cp -r "$ucw/patches/ungoogled-chromium/windows" "$ucl/patches/ungoogled-chromium/portablelinux" \
  "$patches/ungoogled-chromium/"
for name in "${windows_skipped[@]}"; do
  grep -qx "ungoogled-chromium/windows/$name.patch" "$ucw/patches/series" ||
    fail "ungoogled-chromium-windows $ucw_tag has no $name.patch; look at what it leaves out again"
done
for name in "${host_patches[@]}"; do
  grep -qx "ungoogled-chromium/portablelinux/$name.patch" "$ucl/patches/series" ||
    fail "ungoogled-chromium-portablelinux $ucl_tag has no $name.patch; look at what it takes again"
done
{
  grep -vxF -f <(printf 'ungoogled-chromium/windows/%s.patch\n' "${windows_skipped[@]}") "$ucw/patches/series"
  printf 'ungoogled-chromium/portablelinux/%s.patch\n' "${host_patches[@]}"
} >"$patches/series"

# ungoogled-chromium prunes the outputs of Microsoft's midl and mc that Chromium keeps in
# win_build_output, and the import libraries the windows-rs crates link, as binaries. Its Windows
# build makes them again with Microsoft's tools and downloads the libraries again; neither runs on
# Linux, where Chromium's cross build copies them from the tree, so they stay.
grep -vE '(^|/)win_build_output/|^chrome/app/etw_events/|^third_party/rust/chromium_crates_io/vendor/windows_[a-z0-9_]+_msvc-v0_52/lib/' \
  "$_main_repo/pruning.list" >"$work/pruning.list"

# update.py downloads clang's Windows runtime when the .gclient beside src targets Windows.
echo "target_os = ['win']" >"$work/.gclient"

# ungoogled-chromium's GN args and ungoogled-chromium-windows's, less its LLVM's version, and with
# portablelinux's sysroot and V8 metagen settings for the Linux host, which builds the build's
# tools. V8's snapshot goes into chrome.dll: a cross build has no context snapshot, as making one
# would build Blink for Linux too, and the plain snapshot_blob.bin it would load instead is in
# neither FILES.cfg nor mini_installer's dependencies, which stops gn gen. PlayReady, which Chromium
# turns on only for Chrome's own builds, plays protected video on Windows (decision 1).
write_args() {
  mkdir -p "$1"
  {
    awk 1 "$_main_repo/flags.gn"
    grep -vE '^(clang_version|use_sysroot)=' "$ucw/flags.windows.gn"
    printf '%s\n' 'target_os="win"' 'use_sysroot=true' 'v8_use_metagen_instance_types=false' \
      'v8_use_external_startup_data=false' 'enable_playready=true'
  } >"$1/args.gn"
}

# The Windows target's tools Google builds for Linux hosts, which the tarball lacks: rc, the
# resource compiler rc.py runs, checked against the hash in the tree; and the win64 optimization
# profile (chrome_pgo_phase=2), from where ungoogled-chromium's git checkout gets it.
target_tools() {
  local tree=$1 rc sha pgo
  rc=$tree/build/toolchain/win/rc/linux64/rc
  sha=$(cat "$rc.sha1")
  if ! { [ -f "$rc" ] && [ "$(sha1sum <"$rc" | cut -d' ' -f1)" = "$sha" ]; }; then
    curl -fsSL --retry 3 -o "$rc" "https://storage.googleapis.com/chromium-browser-clang/rc/$sha"
    [ "$(sha1sum <"$rc" | cut -d' ' -f1)" = "$sha" ] || fail "rc doesn't match $rc.sha1"
    chmod +x "$rc"
  fi
  pgo=$(cat "$tree/chrome/build/win64.pgo.txt")
  if [ ! -f "$tree/chrome/build/pgo_profiles/$pgo" ]; then
    mkdir -p "$tree/chrome/build/pgo_profiles"
    curl -fsSL --retry 3 -o "$tree/chrome/build/pgo_profiles/$pgo.partial" \
      "https://storage.googleapis.com/chromium-optimization-profiles/pgo_profiles/$pgo"
    mv "$tree/chrome/build/pgo_profiles/$pgo.partial" "$tree/chrome/build/pgo_profiles/$pgo"
  fi
}

tags="$ucw_tag $ucl_tag"
if [ "$(cat "$src/.windows-tags" 2>/dev/null)" != "$tags" ]; then
  # portablelinux's scripts/build.sh, with the Windows patches and downloads, in the scratch tree.
  # Each step leaves a stamp there, so a run that stopped goes on where it was.
  _src_dir=$work/next
  _out_dir=$_src_dir/out/Default
  fetch_sources false false
  if [ ! -f "$_src_dir/.patched.stamp" ]; then
    "$_main_repo/utils/prune_binaries.py" "$_src_dir" "$work/pruning.list"
    components=(directx-headers webauthn multimedia-redirection)
    "$_main_repo/utils/downloads.py" retrieve -i "$ucw/downloads.ini" -c "$_dl_cache" --components "${components[@]}"
    rm -rf "$_src_dir"/third_party/{microsoft_dxheaders,microsoft_webauthn,microsoft_multimedia_redirection}/src
    "$_main_repo/utils/downloads.py" unpack -i "$ucw/downloads.ini" -c "$_dl_cache" "$_src_dir" --components "${components[@]}"
    "$_main_repo/utils/patches.py" apply "$_src_dir" "$_main_repo/patches" "$patches"
    touch "$_src_dir/.patched.stamp"
  fi
  apply_domsub
  write_args "$_out_dir"
  fix_tool_downloading
  setup_toolchain
  target_tools "$_src_dir"
  remove_agent_instructions "$_src_dir"
  apply_series "$_src_dir"
  windows_icons "$_src_dir"
  echo "$tags" >"$_src_dir/.windows-tags"
  if [ -d "$src" ]; then
    rsync -rlpD --checksum --delete --exclude=/out/ "$_src_dir/" "$src/"
    rm -rf "$_src_dir" "$src/out/Default/gn"
  else
    mv "$_src_dir" "$src"
  fi
  rm -rf "$_dl_cache"
fi

_src_dir=$src
_out_dir=$src/out/Default
target_tools "$src"
remove_agent_instructions "$src"
apply_series "$src"
windows_icons "$src"
# Rewritten only when they change, as a newer args.gn makes ninja run gn again.
args=$(mktemp -d)
trap 'rm -rf "$args"' EXIT
write_args "$args"
cmp -s "$args/args.gn" "$src/out/Default/args.gn" || cp "$args/args.gn" "$src/out/Default/args.gn"

# The toolchain unpacked into the mount, and build/win_toolchain.json pointing at it, which is how
# GN's Windows toolchain finds it. depot_tools hashes what it unpacked and remembers it, so this is
# quick once done. vs_toolchain.py would mount ciopfs itself, which the runner can't; it calls
# nothing here as long as the toolchain's Visual Studio is the one Chromium packages.
vs_wanted=$(sed -n "s/^ *('\([0-9]*\)', '[0-9.]*'), *# The VS version in our packaged toolchain.*/\1/p" "$src/build/vs_toolchain.py")
sdk_wanted=$(sed -n "s/^SDK_VERSION = '\(.*\)'$/\1/p" "$src/build/vs_toolchain.py")
[ -n "$vs_wanted" ] && [ -n "$sdk_wanted" ] ||
  fail "build/vs_toolchain.py no longer says which Visual Studio and SDK Chromium packages"
DEPOT_TOOLS_WIN_TOOLCHAIN_BASE_URL=$toolchain DEPOT_TOOLS_WIN_TOOLCHAIN_ROOT=$toolchain \
  python3 "$src/third_party/depot_tools/win_toolchain/get_toolchain_if_necessary.py" --force \
  --output-json "$src/build/win_toolchain.json" "$toolchain_hash"
vs=$(cat "$toolchain/vs_files/$toolchain_hash/VS_VERSION")
[ "$vs" = "$vs_wanted" ] ||
  fail "the Windows toolchain is Visual Studio $vs; Chromium $(git config -f "$engine/pins.ini" chromium.version) needs $vs_wanted (engine/README.md, Windows)"
[ -d "$toolchain/vs_files/$toolchain_hash/Windows Kits/10/Include/$sdk_wanted" ] ||
  fail "the Windows toolchain lacks the Windows SDK $sdk_wanted (engine/README.md, Windows)"

# The toolchain as setup_toolchain gives it to gn's bootstrap, in the tree it's now in.
clang=$src/third_party/llvm-build/Release+Asserts/bin
export CC=$clang/clang CXX=$clang/clang++ AR=$clang/llvm-ar NM=$clang/llvm-nm LLVM_BIN=$clang
resource="-resource-dir=$("$CC" --print-resource-dir) -B$clang"
export CFLAGS="${flags[0]} $resource" CXXFLAGS="${flags[1]} $resource" CPPFLAGS="${flags[2]} $resource"

cd "$src"
[ -x out/Default/gn ] || ./tools/gn/bootstrap/bootstrap.py -o out/Default/gn --skip-generate-buildfiles
echo "$src" >&3
