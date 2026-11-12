#!/bin/bash
# Makes the DMG tools/release.sh made, and the Windows installer --windows names, the update every
# copy of ilo checks for: an entry in the appcast for each, with its release notes and signed with
# the owner's EdDSA key in their Keychain, in app/build/release/appcast/appcast.xml. Sparkle reads
# the Mac's and ilo's Windows updater the Windows one (tools/appcast.py). The installer is built
# from the DMG's commit, so the two have one build number. The Mac's entry names deltas from the
# three builds before it too, beside the appcast. With --publish, the packages and deltas go up as
# the GitHub release v<version>, and the appcast to the release the feed (app/branding/update-feed)
# is served from.
# The notes are Markdown, shown in the Mac's alert. The release's entry in the site's changelog,
# /changelog#<version> on app/branding/site, is what the Mac's first launch of the update opens and
# what a Windows copy still on WinSparkle shows.
#
# Once, on the owner's Mac, before the first release, with Sparkle from app/setup.sh:
#   app/.deps/Sparkle/bin/generate_keys --account ilo                      # the key, in the Keychain
#   app/.deps/Sparkle/bin/generate_keys --account ilo -x ~/ilo-update-key  # its backup, kept offline
#   app/.deps/Sparkle/bin/generate_keys --account ilo -p > app/branding/update-key  # committed
# Every prod build carries the key in app/branding/update-key. Then, for each release:
#   app/build.sh prod
#   SIGN_IDENTITY="Developer ID Application: … (TEAMID)" tools/release.sh
#   tools/appcast.sh notes.md --windows ilo-setup.exe     # then look at the appcast it made
#   tools/appcast.sh notes.md --windows ilo-setup.exe --publish
# In CI (release.yml) the key is UPDATE_PRIVATE_KEY, the backup's contents, which the tools read on
# their standard input, and the Keychain is never read.
# With --local, it makes the updates run's appcast instead (app/check/updates.sh), in DIR, which is
# served at URL: one update past APP, its notes notes.md, signed with the run's own key in KEYFILE,
# or informational, which Sparkle shows and never downloads. It never reads the owner's key or feed.
# With --nightly TAG, it makes the next state of the feed release TAG serves instead, for
# release.yml's nightly mode: each package named by its build, as one release holds several, beside
# the appcast in the same folder, which keeps each OS's newest three. A nightly's notes are in the
# appcast, with no changelog entry to open.
# Usage: tools/appcast.sh notes.md [--windows installer.exe] [--publish | --nightly TAG]
#        tools/appcast.sh notes.md --local APP DIR URL KEYFILE [--informational]
set -euo pipefail
tools="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$tools/../app" && pwd)"
usage="usage: tools/appcast.sh notes.md [--windows installer.exe] [--publish | --nightly TAG]
       tools/appcast.sh notes.md --local APP DIR URL KEYFILE [--informational]"
notes=${1:?$usage}
shift
if [ "${1:-}" = --local ]; then
  { [ $# -eq 5 ] || [ $# -eq 6 ]; } && [ "${6:---informational}" = --informational ] ||
    { echo "$usage" >&2; exit 2; }
  plist=$2/Contents/Info.plist
  version=$(plutil -extract CFBundleShortVersionString raw "$plist")
  build=$(($(plutil -extract CFBundleVersion raw "$plist") + 1))
  mkdir -p "$3"
  if [ -n "${6:-}" ]; then
    python3 -I "$tools/appcast.py" local "$3/appcast.xml" "$version" "$build" "$notes"
  else
    # Sparkle downloads it only for Install, which the run never chooses.
    archive=$3/update-$build.zip
    ditto -c -k "$notes" "$archive"
    signature=$("$root/.deps/Sparkle/bin/sign_update" --ed-key-file "$5" -p "$archive")
    python3 -I "$tools/appcast.py" local "$3/appcast.xml" "$version" "$build" "$notes" \
      "$4$(basename "$archive")" "$(wc -c < "$archive" | tr -d ' ')" "$signature"
    python3 -I "$tools/appcast.py" signed "$3/appcast.xml" "$build"
  fi
  echo "$3/appcast.xml"
  exit 0
fi
windows=
publish=
nightly=
while [ $# -gt 0 ]; do
  case $1 in
    --windows) windows=${2:?$usage} && shift 2 ;;
    --publish) publish=1 && shift ;;
    --nightly) nightly=${2:?$usage} && shift 2 ;;
    *) echo "$usage" >&2 && exit 2 ;;
  esac
done
[ -z "$publish" ] || [ -z "$nightly" ] || { echo "$usage" >&2; exit 2; }
name=$(cat "$root/branding/name")
out=$root/build/release
app=$out/$name.app
dmg=$out/$name.dmg
bin=$root/.deps/Sparkle/bin
feed=$(cat "$root/branding/update-feed")
# The feed is a file in a release of this repository: https://github.com/<owner>/<repo>/releases/…
repo=${feed%%/releases/*}
slug=${repo#https://github.com/}
feed_tag=${feed#"$repo"/releases/download/}
feed_tag=${feed_tag%%/*}
if [ -n "$nightly" ]; then
  feed_tag=$nightly
  feed=$repo/releases/download/$feed_tag/appcast.xml
fi

[ -f "$dmg" ] || { echo "No $dmg. Make it with tools/release.sh." >&2; exit 1; }
[ -z "$windows" ] || [ -f "$windows" ] || { echo "No $windows." >&2; exit 1; }
[ -x "$bin/generate_appcast" ] || { echo "No Sparkle in app/.deps. Run app/setup.sh." >&2; exit 1; }
version=$(plutil -extract CFBundleShortVersionString raw "$app/Contents/Info.plist")
build=$(plutil -extract CFBundleVersion raw "$app/Contents/Info.plist")
# The release's entry in the site's changelog, which names each entry by its version. A nightly's
# packages are beside its appcast, and named by their build.
notes_url=$(cat "$root/branding/site")/changelog#$version
prefix=$repo/releases/download/v$version/
archive=$name.dmg
if [ -n "$nightly" ]; then
  notes_url=
  prefix=$repo/releases/download/$feed_tag/
  archive=$name-mac-arm64-$build.dmg
  installer=$name-windows-x64-setup-$build.exe
fi
# An update signed with another key than the one the app carries is one it refuses.
# With UPDATE_PRIVATE_KEY, generate_appcast leaves an update whose app carries another key unsigned,
# which the check after it catches.
if [ -n "${UPDATE_PRIVATE_KEY:-}" ]; then
  signing=(--ed-key-file -)
elif [ -n "${CI:-}" ]; then
  echo "In CI the signing key is UPDATE_PRIVATE_KEY; the Keychain isn't read." >&2
  exit 1
else
  signing=(--account "$name")
  key=$(plutil -extract SUPublicEDKey raw "$app/Contents/Info.plist")
  if [ "$("$bin/generate_keys" --account "$name" -p)" != "$key" ]; then
    echo "The key in the Keychain isn't the one $app carries." >&2
    exit 1
  fi
fi
# The key, from UPDATE_PRIVATE_KEY, for a tool's standard input, and never in its arguments.
private_key() { [ -z "${UPDATE_PRIVATE_KEY:-}" ] || printf '%s\n' "$UPDATE_PRIVATE_KEY"; }

# generate_appcast adds to the appcast at its output, so it starts from the one published.
work=$out/appcast
rm -rf "$work"
mkdir "$work"
cp "$dmg" "$work/$archive"
cp "$notes" "$work/${archive%.dmg}.md"
# generate_appcast sees only the Mac's entries, as it would take the Windows ones for its own.
aside=$out/appcast-windows.xml
# The builds the deltas are from, the feed's three newest Mac builds, before generate_appcast keeps
# only two of them.
bases=$out/delta-bases.txt
: > "$bases"
before=0
case $(curl -sSL -w '%{http_code}' -o "$work/appcast.xml" "$feed") in
  200)
    before=$(python3 -I "$tools/appcast.py" aside "$work/appcast.xml" "$aside")
    python3 -I "$tools/appcast.py" bases "$work/appcast.xml" "$build" 3 > "$bases"
    ;;
  404) # the first release
    rm "$work/appcast.xml"
    printf '<rss version="2.0"><channel/></rss>\n' > "$aside"
    ;;
  *) echo "Couldn't fetch $feed." >&2 && exit 1 ;;
esac
private_key | "$bin/generate_appcast" "${signing[@]}" \
  --download-url-prefix "$prefix" \
  ${notes_url:+--full-release-notes-url "$notes_url"} \
  --embed-release-notes --maximum-deltas 0 \
  -o "$work/appcast.xml" "$work"
after=$(grep -c '<item>' "$work/appcast.xml")
if ! python3 -I "$tools/appcast.py" signed "$work/appcast.xml" "$build" || [ "$after" -lt "$before" ]; then
  echo "generate_appcast made an appcast without a signature, or lost an update: $work/appcast.xml" >&2
  exit 1
fi
# Deltas to this build from those builds, each from the DMG its own release serves, beside the DMG.
# A copy on one of them downloads a few MB, which Sparkle applies to it in place; any other copy, or
# one whose files a delta doesn't match, takes the DMG. Sparkle's generate_appcast would make them
# too, but it rewrites the URLs of the DMGs it reads with this release's prefix.
base_dir=$out/delta-base
deltas=()
while read -r base url <&3; do
  rm -rf "$base_dir"
  mkdir "$base_dir"
  if ! curl -sSfL --retry 3 -o "$base_dir/base.dmg" "$url"; then
    echo "No delta from build $base: couldn't download $url." >&2
    continue
  fi
  hdiutil attach -quiet -nobrowse -readonly -mountpoint "$base_dir/volume" "$base_dir/base.dmg"
  ditto "$base_dir/volume/$name.app" "$base_dir/$name.app"
  hdiutil detach -quiet "$base_dir/volume"
  delta=$name-mac-arm64-$base-to-$build.delta
  "$bin/BinaryDelta" create "$base_dir/$name.app" "$app" "$work/$delta"
  # A delta goes out only once it has made this build's app, which BinaryDelta checks file by file.
  "$bin/BinaryDelta" apply "$base_dir/$name.app" "$base_dir/applied.app" "$work/$delta"
  signature=$(private_key | "$bin/sign_update" "${signing[@]}" -p "$work/$delta")
  python3 -I "$tools/appcast.py" delta "$work/appcast.xml" "$build" "$base" "$prefix$delta" \
    "$(wc -c < "$work/$delta" | tr -d ' ')" "$signature"
  deltas+=("$work/$delta")
  echo "Delta from build $base: $(wc -c < "$work/$delta" | tr -d ' ') bytes" >&2
done 3< "$bases"
rm -rf "$base_dir" "$bases"
if [ -n "$windows" ]; then
  signature=$(private_key | "$bin/sign_update" "${signing[@]}" -p "$windows")
  python3 -I "$tools/appcast.py" finish "$work/appcast.xml" "$aside" --windows \
    "$prefix${installer:-$(basename "$windows")}" "$(wc -c < "$windows" | tr -d ' ')" \
    "$signature" "$version" "$build" "$notes_url"
else
  python3 -I "$tools/appcast.py" finish "$work/appcast.xml" "$aside"
fi
rm "$aside"
if [ -n "$nightly" ]; then
  python3 -I "$tools/appcast.py" nightly "$work/appcast.xml" "$build" 3
  rm "$work/${archive%.dmg}.md"
  [ -z "$windows" ] || cp "$windows" "$work/$installer"
fi
echo "$work/appcast.xml"
[ -n "$publish" ] || exit 0

# The downloads first, as the appcast sends every copy to them.
gh release create "v$version" "$dmg" ${windows:+"$windows"} ${deltas[@]+"${deltas[@]}"} \
  --repo "$slug" --target "$(git -C "$root" rev-parse HEAD)" --title "$name $version" \
  --notes-file "$notes" --latest
gh release view "$feed_tag" --repo "$slug" > /dev/null 2>&1 ||
  gh release create "$feed_tag" --repo "$slug" --prerelease --latest=false --title "Updates" \
    --notes "The appcast every copy of $name checks for updates (app/branding/update-feed)."
gh release upload "$feed_tag" "$work/appcast.xml" --repo "$slug" --clobber
