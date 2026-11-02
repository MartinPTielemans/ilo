# Releasing

`.github/workflows/release.yml` builds a release for every OS from one annotated tag. This page says how to cut one and what the run needs.

## Cutting a release

1. Set the version in `app/Cargo.toml` and merge it to main.
2. Make sure the Windows engine exists for that commit. It builds nightly, so run Engine on Windows (cross-compiled) on it, `gh workflow run engine-windows-cross.yml --ref main` with main at that commit, and wait for its release `engine-windows-<commit>`. The Mac and Linux engines must exist for the commit too; each OS's job stops without one.
3. Write the release's entry in [ilo-site](https://github.com/MartinPTielemans/ilo-site)'s `src/changelog.ts` from the notes, and merge it before tagging: each update's notes and its first launch open `/changelog#<version>` on the site (`tools/appcast.sh`).
4. Tag the commit with the release notes as the tag's message, and push the tag:

   ```bash
   git tag -a v0.1.0      # the editor's text is the release's notes
   git push origin v0.1.0
   ```

   A lightweight tag fails the run, since the notes come from the message. Running the workflow by hand before the tag exists tries the release: it builds everything, and its notes say it isn't for publishing.
5. Check the draft release's files, then publish it.
6. Open a pull request to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) with the manifests in the run's `winget` artifact.

Publish each draft before building the next: a Linux build doesn't follow a draft that isn't live yet.

## What the run makes

A draft release whose files keep their names, so `releases/latest/download/<name>` always links the newest:

- `ilo-mac-arm64.dmg`: signed and notarized.
- `ilo-windows-x64.msix`: unsigned; the Store signs it.
- `ilo-windows-x64-setup.exe`: unsigned, for winget and direct downloads.
- `ilo-linux-x86_64.flatpak` and `ilo.flatpakref`.
- `SHA256SUMS` and `appcast.xml`.
- `ilo-demo.tar.gz`, the site's live demo built from the same commit (`.github/workflows/demo.yml`), when the commit has one.

Once the draft is published:

- The `publish` job puts its `appcast.xml` in the release `updates`, the feed the Mac and Windows builds read (`app/branding/update-feed`).
- The Linux build, already in the Flatpak repository unseen, goes live.
- [ilo-site](https://github.com/MartinPTielemans/ilo-site)'s release workflow puts the demo on [ilobrowser.com](https://ilobrowser.com), links its source to the tag and deploys the site, whose download buttons link the newest release. It checks every half hour, or at once with `SITE_DISPATCH_TOKEN`.
- Martin's Grok bot announces it: it records clips and a film of the new features on the published build, adds the clips to the site's changelog through its own ilo-site pull request, and drafts the post. The release never waits for it.

Every other release in this repository, such as `updates`, is made with `--latest=false`, or the site's buttons would link it. The prebuilt engines live in [ilo-engine](https://github.com/MartinPTielemans/ilo-engine), the repository `engine/prebuilt.ini` names, as `engine-mac-`, `engine-linux-` and `engine-windows-<commit>`, also with `--latest=false`.

## Secrets and variables

**Mac.** The DMG is built only with all of these; without them, no DMG, signed or not, is uploaded:

| Secret | What |
|---|---|
| `MAC_CERTIFICATE_P12` | The Developer ID Application certificate and its key, as a base64 `.p12` |
| `MAC_CERTIFICATE_PASSWORD` | The `.p12`'s password |
| `MAC_NOTARY_KEY` | An App Store Connect API key for notarization, as a base64 `.p8` |
| `MAC_NOTARY_KEY_ID`, `MAC_NOTARY_ISSUER_ID` | That key's ID and its issuer's |
| `UPDATE_PRIVATE_KEY` | The updates' signing key: the contents of the file `generate_keys --account ilo -x <file>` writes (`tools/appcast.sh`) |

The Mac and Windows builds carry the key's public half from `app/branding/update-key`, or from an `UPDATE_PUBLIC_KEY` secret, which overrides the file when it's set.

**Crash reports' symbols** go to the collector `app/branding/crash-reports` names only with the secret `SENTRY_AUTH_TOKEN`, an organization auth token that may upload debug files, and the variables `SENTRY_URL`, `SENTRY_ORG` and `SENTRY_PROJECT`: for ilo's, `https://de.sentry.io` (Sentry's EU region), `ilo-rg` and `ilo`. Each OS's job uploads its own (`tools/symbols.sh`), and `app/build.sh prod` uploads the Mac's with the same four in its environment. Without the token the packages are still built, and say no symbols were uploaded.

**Linux.** Installs from GitHub update from ilo's Flatpak repository, a static OSTree repository in a Cloudflare R2 bucket (`tools/flatpak-repo.sh`). The bundle and `ilo.flatpakref` name it and carry its public key. The Linux files are built only with these, and without them nothing goes to the Flatpak repository and the draft isn't made:

| Secret | What |
|---|---|
| `FLATPAK_GPG_KEY` | The repository's signing key, ASCII-armoured, without a passphrase |
| `R2_BUCKET` | The bucket's name |
| `R2_ENDPOINT` | `https://<account-id>.r2.cloudflarestorage.com` |
| `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY` | An R2 API token with Object Read & Write on that bucket alone |

and the repository variable `FLATPAK_REPO_URL`, the bucket's public address, such as `https://flatpak.<domain>/`.

**Other repositories.**

| Secret | What |
|---|---|
| `SITE_DISPATCH_TOKEN` | A fine-grained token with Contents read and write on `MartinPTielemans/ilo-site` alone, so the site takes a release at once |
| `ENGINE_RELEASE_TOKEN` | A fine-grained token with Contents read and write on `MartinPTielemans/ilo-engine` alone, for the engine workflows |

## The Flatpak repository, once

Before the first release:

1. Make the key: `gpg --quick-gen-key "ilo Flatpak repository" rsa4096 sign never`, with an empty passphrase. Put `gpg --armor --export-secret-keys <fingerprint>` in `FLATPAK_GPG_KEY` and keep a copy offline forever. Every installed copy checks it, and a new key strands them.
2. In Cloudflare, create the R2 bucket, connect a custom domain to it for public reads, and create the API token.
3. Add the secrets and the variable. The address is permanent too, as installs keep it.
