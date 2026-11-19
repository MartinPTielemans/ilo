#!/bin/bash
# Makes Blocking/Rules, the content-blocking rules the app comes with: downloads the filter lists
# lists.ini and app/ublock.ini pin into .deps/lists, each checked against its sha256, converts
# them with convert/ (Brave's adblock-rust, the desktop's engine), has WebKit compile each rule
# list as the phone will (check.swift), and writes them compressed beside a manifest naming their
# version. Run it once in a new checkout, before xcodegen, and after changing a pin or convert/.
# It needs cargo, curl, unzip and Xcode. Nothing in a download runs: the lists are only read.
set -euo pipefail
cd "$(dirname "$0")"

fail() {
  echo "Blocking/build.sh: $*" >&2
  exit 1
}

# Downloads $1 to $2 unless it's there, and checks it against the sha256 $3 either way.
fetch() {
  [ -f "$2" ] || curl -fsSL --retry 3 -o "$2.part" "$1" && { [ -f "$2" ] || mv "$2.part" "$2"; }
  echo "$3  $2" | shasum -a 256 -c --status || { rm -f "$2"; fail "$1 doesn't match its sha256."; }
}

lists=$PWD/../.deps/lists
mkdir -p "$lists"

desktop=../../../app/ublock.ini
url=$(git config -f "$desktop" ublock.url)
sha256=$(git config -f "$desktop" ublock.sha256)
fetch "$url" "$lists/ublock-$sha256.zip" "$sha256"
ublock=$lists/ublock-$sha256
if [ ! -d "$ublock" ]; then
  rm -rf "$ublock.part"
  unzip -q "$lists/ublock-$sha256.zip" 'uBlock0.chromium/assets/*' -d "$ublock.part"
  mv "$ublock.part/uBlock0.chromium/assets" "$ublock"
  rm -r "$ublock.part"
fi

commit=$(git config -f lists.ini cookies.commit)
mirror=https://raw.githubusercontent.com/uBlockOrigin/uAssetsCDN/$commit
cookies=$lists/cookies-$commit
mkdir -p "$cookies"
fetch "$mirror/thirdparties/easylist-cookies.txt" "$cookies/easylist-cookies.txt" "$(git config -f lists.ini cookies.easylist)"
fetch "$mirror/filters/annoyances-cookies.txt" "$cookies/annoyances-cookies.txt" "$(git config -f lists.ini cookies.ublock)"

export CARGO_TARGET_DIR=$PWD/../.deps/rules-target
cargo build --release --locked --quiet --manifest-path convert/Cargo.toml

out=$PWD/../.deps/rules.part
rm -rf "$out"
"$CARGO_TARGET_DIR/release/ilo-content-rules" "$out" \
  "$ublock/ublock/filters.min.txt" "$ublock/ublock/badware.min.txt" "$ublock/ublock/privacy.min.txt" \
  "$ublock/ublock/unbreak.min.txt" "$ublock/ublock/quick-fixes.min.txt" \
  "$ublock/thirdparties/easylist/easylist.txt" "$ublock/thirdparties/easylist/easyprivacy.txt" \
  "$ublock/thirdparties/urlhaus-filter/urlhaus-filter-online.txt" "$ublock/thirdparties/pgl.yoyo.org/as/serverlist" \
  "$cookies/easylist-cookies.txt" "$cookies/annoyances-cookies.txt" > "$out.counts"
swift check.swift "$out" || fail "WebKit refused a rule list."

# The version names what the phone compiled, so it compiles again only when the rules changed.
version=$(cat "$out"/rules-*.json | shasum -a 256 | cut -c 1-12)
{
  echo "{\"version\": \"$version\", \"lists\": ["
  while read -r name count; do
    compression_tool -encode -a lzfse -i "$out/$name" -o "$out/$name.lzfse"
    rm "$out/$name"
    echo "  {\"file\": \"$name.lzfse\", \"rules\": $count},"
  done < "$out.counts" | sed '$ s/,$//'
  echo "]}"
} > "$out/manifest.json"
rm "$out.counts"
rm -rf Rules
mv "$out" Rules
echo "Blocking/Rules: version $version, $(du -sh Rules | cut -f 1)"
