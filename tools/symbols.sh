#!/bin/bash
# Uploads a release's crash report symbols to the collector app/branding/crash-reports names
# (docs/decisions.md, 9) with sentry-cli: the files and folders given, which sentry-cli searches for
# debug files, and on Windows the engine's PDBs, which its release keeps apart from it
# (engine/fetch.sh symbols). SENTRY_URL, SENTRY_AUTH_TOKEN, SENTRY_ORG and SENTRY_PROJECT name the
# collector's project: for ilo's, https://de.sentry.io, the EU region, though an organization
# token's own URL wins over SENTRY_URL. Without the token, the owner's to make, or with no
# collector, as in a fork's build, it says so and uploads nothing; with the token but not the rest
# it stops. Given nothing, it checks only that, so a release stops before it builds.
# Usage: tools/symbols.sh [file or folder]...
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)

if [ -z "${SENTRY_AUTH_TOKEN:-}" ]; then
  echo "symbols: no SENTRY_AUTH_TOKEN (docs/releasing.md), so no symbols were uploaded." >&2
  exit 0
fi
missing=
for name in SENTRY_URL SENTRY_ORG SENTRY_PROJECT; do
  [ -n "${!name:-}" ] || missing="$missing $name"
done
if [ -n "$missing" ]; then
  echo "symbols: uploading crash reports' symbols needs$missing as well (docs/releasing.md)." >&2
  exit 1
fi
if [ -z "$(tr -d '[:space:]' <"$root/app/branding/crash-reports")" ]; then
  echo "symbols: app/branding/crash-reports names no collector, so no symbols are uploaded." >&2
  exit 0
fi
[ $# -gt 0 ] || exit 0

# sentry-cli's own build for the OS, checked against the digest GitHub has for its release.
version=3.8.2
case $(uname -sm) in
"Darwin arm64")
  asset=sentry-cli-Darwin-arm64
  sha256=c80d92b6d34eb64fe7402f56cc7968fbb15aab1c763254ad428b55cc98ec7dcf
  ;;
"Linux x86_64")
  asset=sentry-cli-Linux-x86_64
  sha256=c958865f9d4000183509ea21b1f04925d5390de6d8d87386b9e69f2b305070c3
  ;;
MINGW*" x86_64")
  asset=sentry-cli-Windows-x86_64.exe
  sha256=50009f23c49d6e9ff7624204c7035855303b49c5e326708ee783c1fdf783ac83
  engine=$("$root/engine/fetch.sh" symbols)
  set -- "$@" "$engine"
  ;;
*)
  echo "symbols: there's no sentry-cli here for $(uname -sm)." >&2
  exit 1
  ;;
esac
bin=$(mktemp -d)
trap 'rm -rf "$bin"' EXIT
cli=$bin/$asset
curl -fsSL -o "$cli" "https://github.com/getsentry/sentry-cli/releases/download/$version/$asset"
# shasum on the Mac, sha256sum on Linux and in Git Bash.
if command -v sha256sum >/dev/null; then sum=$(sha256sum "$cli"); else sum=$(shasum -a 256 "$cli"); fi
[ "${sum%% *}" = "$sha256" ] || { echo "symbols: $asset doesn't match its sha256." >&2 && exit 1; }
chmod +x "$cli"
"$cli" debug-files upload --wait "$@"
