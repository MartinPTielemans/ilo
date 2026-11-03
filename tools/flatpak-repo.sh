#!/bin/bash
# Puts a build of ilo's Flatpak, the bundle tools/flatpak.sh made, into ilo's Flatpak repository, a
# static OSTree repository at <URL>, and makes a release's Linux files from it in <out>:
#   ilo-linux-x86_64.flatpak  the same build as a bundle that names the repository and its key, so an
#                             install from it updates through the software centre and the app's own
#                             updater, as one from Flathub does
#   ilo.flatpakref            the repository's entry, which a software centre installs in one click
# <repo> is a copy of the published repository, or a new folder for the first: the build goes in
# signed with the key in FLATPAK_GPG_KEY (ASCII-armoured, without a passphrase), builds older than
# the last three are pruned, static deltas are made and the summary is signed. release.yml syncs it
# with the bucket <URL> serves (docs/releasing.md). The key stays in a keyring of this run's own.
# Usage: FLATPAK_GPG_KEY=… tools/flatpak-repo.sh <ilo.flatpak> <repo> <URL> <out>
set -euo pipefail
fail() {
  echo "flatpak-repo: $*" >&2
  exit 1
}
[ $# -eq 4 ] || fail "usage: tools/flatpak-repo.sh <ilo.flatpak> <repo> <URL> <out>"
bundle=$1
repo=$2
url=${3%/}/
out=$4
[ -n "${FLATPAK_GPG_KEY:-}" ] || fail "no FLATPAK_GPG_KEY, the key the repository is signed with."
for tool in flatpak ostree gpg; do
  command -v "$tool" >/dev/null || fail "needs flatpak, ostree and gpg: no $tool."
done
id=app.ilo.browser
name=$(cat "$(dirname "$0")/../app/branding/name")
flathub=https://dl.flathub.org/repo/flathub.flatpakrepo

gpghome=$(mktemp -d)
trap 'rm -rf "$gpghome"' EXIT
printf '%s\n' "$FLATPAK_GPG_KEY" | gpg --homedir "$gpghome" --batch --quiet --import 2>/dev/null ||
  fail "FLATPAK_GPG_KEY isn't a GPG key."
key=$(gpg --homedir "$gpghome" --batch --with-colons --list-secret-keys | awk -F: '$1 == "fpr" { print $10; exit }')
[ -n "$key" ] || fail "FLATPAK_GPG_KEY holds no private key."
gpg --homedir "$gpghome" --batch --export "$key" >"$gpghome/public.gpg"
sign=(--gpg-sign="$key" --gpg-homedir="$gpghome")

[ -f "$repo/config" ] || ostree init --repo="$repo" --mode=archive-z2
# A repository synced back from the bucket comes without ostree's empty directories, which object
# storage doesn't keep, and build-update-repo then fails listing refs/remotes.
mkdir -p "$repo"/refs/{heads,mirrors,remotes} "$repo"/{objects,state,tmp/cache,extensions}
flatpak build-import-bundle "${sign[@]}" --update-appstream --no-update-summary "$repo" "$bundle"
# The build and the two before it, so a copy one or two releases old updates by a delta.
flatpak build-update-repo "${sign[@]}" --title="$name" --prune --prune-depth=2 \
  --generate-static-deltas "$repo"
ref=$(ostree refs --repo="$repo" | grep -x "app/$id/x86_64/[^/]*") ||
  fail "$repo has no x86_64 build of $id."
branch=${ref##*/}

mkdir -p "$out"
flatpak build-bundle --repo-url="$url" --gpg-keys="$gpghome/public.gpg" --runtime-repo="$flathub" \
  "$repo" "$out/ilo-linux-x86_64.flatpak" "$id" "$branch"
cat >"$out/ilo.flatpakref" <<EOF
[Flatpak Ref]
Name=$id
Branch=$branch
Title=$name
Url=$url
RuntimeRepo=$flathub
IsRuntime=false
SuggestRemoteName=$name
GPGKey=$(base64 -w0 "$gpghome/public.gpg")
EOF
echo "flatpak-repo: $ref, $(ostree rev-parse --repo="$repo" "$ref"), in $repo for $url" >&2
