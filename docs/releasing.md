# Releasing

ilo has two tracks, and both are built by `.github/workflows/release.yml` on GitHub's hosted runners.

- **Releases** are built from an annotated tag, `v<version>`, and read by every copy from the feed `app/branding/update-feed`: release `updates`' `appcast.xml`.
- **The nightly** is main's build each night, read from release `nightly`'s `appcast.xml` by a copy with Get nightly builds on (Settings › About).

Both count builds the same way, by `git rev-list --count` of the commit, and an updater installs only a higher build. Main's builds are ahead of a release branch's, so a nightly with the switch turned off keeps its build until a release passes it, which is the next minor's, cut from main. Nothing ever goes back a build.

This page says how to cut a release, how fixes reach one, how the nightly runs, and what the runs need.

## Where a release comes from

Until the nightly track is on, releases come from main, as 0.1.3 does. From then on, each minor has a branch, `release/0.<minor>`, and main carries the next minor:

- **A fix** merges to main first, as any change does. Then it's cherry-picked to the branch with `git cherry-pick -x`, so the branch's commit names main's.
- **The branch's version** moves to the fix release's in a commit of its own on the branch, as `chore(app): version 0.1.4` does on main: `app/Cargo.toml`, `app/Cargo.lock`, and the release's line in `app/platform/linux/flatpak/app.ilo.browser.metainfo.xml`.
- **A Chromium security point release** on the branch's major reaches the branch the same way: its pin's commit is cherry-picked.
- **The next minor's first release**, such as 0.2.0, is cut from main. Its branch, `release/0.2`, is cut from that tag when its first fix needs one.

## Turning the nightly track on

This is one step, done once, right before the first nightly publishes. Do it in this order:

1. **Cut the last release from main**, as before. That's 0.1.3.
2. **Cut `release/0.1` from that tag:** `git push origin v0.1.3^{commit}:refs/heads/release/0.1`.
3. **Move main to 0.2.0** in one commit, `chore(app): version 0.2.0`, which changes `app/Cargo.toml` and `app/Cargo.lock`. The Flatpak metainfo gains a release only when 0.2.0 is cut. This way a nightly never reads as a 0.1.x.
4. **Name the nightly feed** in a second commit: `app/branding/update-feed-nightly` holds `https://github.com/MartinPTielemans/ilo/releases/download/nightly/appcast.xml`, a default service URL that a fork changes. A build that carries it shows Get nightly builds where its updater can follow it. The nightly mode publishes only once the night's commit carries it.
5. **Publish the first nightly:** `gh workflow run release.yml --ref main -f tag=nightly`, or wait for the next night at 06:00 UTC. The run makes release `nightly`.
6. **Let 0.1.x copies opt in** by cherry-picking step 4's commit to `release/0.1`. From the next 0.1.x on, a release copy offers the switch.

Turn the track off by removing `app/branding/update-feed-nightly` from main. The night then builds nothing and says why. Copies already on a nightly keep their build until a release passes it.

## Cutting a release

A release's core batch lands first, as one squash PR from `batch/<version>`, or `batch/<version>-<N>` when it's replayed. The branch holds one signed commit on main, whose tree is the trial merge, so the agent context budget accepts it without the label, as its folded PRs each passed it already (#2203).

From that squash until the release's branch is cut at its version commit, main takes only PRs that change checks, CI or docs, plus the fixes the orchestrator names for the release. A PR that changes `app/src`, `app/platform`, `core` or `engine` waits until the branch is cut, so the release is the tree its gate proved, plus fixes each proven on their own.

1. **Move the version** on the release's branch, or on main until the nightly track is on. That's `app/Cargo.toml`, `app/Cargo.lock` and the metainfo's release line, merged as any change is.
2. **Make sure each OS has an engine for that commit.** Each OS's job stops without one, or packs an older core.
   - **On main:** Linux and Windows build nightly, so dispatch both on it: `gh workflow run engine-linux.yml --ref main` and `gh workflow run engine-windows-cross.yml --ref main`, with main at that commit. Then wait for their releases `engine-linux-<commit>` and `engine-windows-<commit>`. The Mac engine must exist for the commit too.
   - **On a release branch:** dispatch the same two with `--ref release/0.<minor>`. The Mac's is built on the owner's MacBook from the branch's commit, with `engine/build.sh`, then `tools/mac-engine-publish.sh`. `tools/mac-nightly.sh` builds main only.
3. **Check the commit: the gate is green outside the quarantine list.** Check on Windows and Linux, and Check on macOS's default runs, must pass on the release's commit.
   - **The Mac's other runs:** the ones Check on macOS leaves out of its default runs are `VM_RUNS` in `.github/workflows/check-mac.yml`, and they must pass on the owner's MacBook's check VM at that commit. The workflow's header gives each one's reason (#2205).
   - **The only reds allowed** are the runs, or steps of runs, in `app/check/quarantine.<os>`. They still run, but they don't fail a check, and each check's summary lists them under its runs.
   - **Any other red stops the release.** Fix it, or quarantine it under an open issue (`app/check/quarantine.sh` says how), before tagging. Never accept it at the gate.
   - **A quarantine line lasts 14 days.** Check scripts fails on a line older than that, or on one whose issue has closed.
4. **Write the release's changelog entry** in [ilo-site](https://github.com/MartinPTielemans/ilo-site)'s `src/changelog.ts` from the notes, and merge it before tagging. Each update's notes and its first launch open `/changelog#<version>` on the site (`tools/appcast.sh`).
5. **Tag the commit and push the tag.** The release's notes are the tag's message, written in Markdown with no title line, since GitHub shows the release's name above them:

   ```bash
   git tag -a --cleanup=verbatim -F notes.md v0.1.4 <commit>
   git push origin v0.1.4
   ```

   `--cleanup=verbatim` keeps the notes' `##` headings, which git's default cleanup strips as comment lines.

   A quarantined line that is a bug a user can meet goes in the notes' Known gaps, and a check's own flake doesn't. The release run's summary lists both.

   A lightweight tag fails the run, since the notes come from the message. Running the workflow by hand before the tag exists tries the release: it builds everything, its notes say it isn't for publishing, and it makes no release.
6. **Check the draft release's files, then publish it.**
7. **Open a pull request** to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs) with the manifests in the run's `winget` artifact.

Publish each draft before building the next: a Linux build doesn't follow a draft that isn't live yet.

## The nightly

`release.yml` runs at 06:00 UTC, after the night's engines and checks. Run it by hand with `gh workflow run release.yml --ref main -f tag=nightly`, for example after a security point release merges, so the fix ships that day.

- **The night's commit** is the newest on main whose core and engine are those of the Mac engine that `engine/fetch.sh` takes there (`tools/nightly-commit.sh`).
  - Windows joins when its engine fits that commit too. Otherwise the feed keeps Windows's previous build.
  - The appcast is signed beside the Mac's build, so while the Mac engine is behind main's core (the MacBook slept), the night builds nothing new.
  - A night whose commit the feed has already builds nothing.
- **What it builds** is the release's own jobs, so each night rehearses a release:
  - the Mac DMG, signed and notarized;
  - the Windows installer, with no MSIX and no winget;
  - the Flatpak bundle, as a check only, since a Flatpak follows the branch it's installed from.

  A nightly is marked as one (`APP_TRACK=nightly`), so About says "Nightly 0.2.0 (1688)".
- **What it publishes**, to release `nightly`, a prerelease made with `--latest=false`, so the site's buttons never offer it:
  - `ilo-mac-arm64-<build>.dmg`, its deltas from the three nightlies before it, and `ilo-windows-x64-setup-<build>.exe`, uploaded before `appcast.xml`, which names each entry "0.2.0 (<build>)";
  - each OS's newest three builds stay, and packages the appcast no longer names go.
- **A failure** opens or comments on one issue, "Release fails on main", and the next night that publishes closes it (`.github/scripts/main-check.sh`).
- **To try it** from a branch, `gh workflow run release.yml --ref <branch> -f tag=nightly-<name>` publishes the same to a release `nightly-<name>`. It needs no feed file. Delete the release and its tag when done. `-f tag=nightly` runs only from main.

The nightly needs no secret of its own: it signs with the Mac's.

## What a release's run makes

A draft release whose files keep their names, so `releases/latest/download/<name>` always links the newest:

- `ilo-mac-arm64.dmg`: signed and notarized.
- `ilo-windows-x64.msix`: unsigned; the Store signs it.
- `ilo-windows-x64-setup.exe`: unsigned, for winget and direct downloads.
- `ilo-linux-x86_64.flatpak` and `ilo.flatpakref`.
- `SHA256SUMS` and `appcast.xml`.
- `ilo-mac-arm64-<a>-to-<b>.delta`: Sparkle's deltas to the DMG's app from the feed's three Mac builds before it, made from their releases' DMGs. A copy on one downloads a few MB instead of the DMG; any other copy, or one a delta doesn't match, takes the DMG.
- `ilo-demo.tar.gz`, the site's live demo built from the same commit (`.github/workflows/demo.yml`), when the commit has one.

Once the draft is published:

- The `publish` job puts its `appcast.xml` in the release `updates`, the feed the Mac and Windows builds read (`app/branding/update-feed`).
- The Linux build, already in the Flatpak repository unseen, goes live.
- [ilo-site](https://github.com/MartinPTielemans/ilo-site)'s release workflow puts the demo on [ilobrowser.com](https://ilobrowser.com), links its source to the tag and deploys the site, whose download buttons link the newest release. It checks every half hour, or at once with `SITE_DISPATCH_TOKEN`.
- Martin's Grok bot announces it: it records clips and a film of the new features on the published build, adds the clips to the site's changelog through its own ilo-site pull request, and drafts the post. With `RELEASE_WEBHOOK_URL` and `RELEASE_WEBHOOK_KEY` set, the `announce` job tells it at once; without them, it finds the release at one of its three daily check-ins, at 10:00, 16:00 and 21:00 Copenhagen time. The release never waits for it.

Every other release in this repository, such as `updates` and `nightly`, is made with `--latest=false`, or the site's buttons would link it. The prebuilt engines live in [ilo-engine](https://github.com/MartinPTielemans/ilo-engine), the repository `engine/prebuilt.ini` names, as `engine-mac-`, `engine-linux-` and `engine-windows-<commit>`, also with `--latest=false`.

## Secrets and variables

The signing, notarization, update, Flatpak, R2, site and announcement secrets live in the `release` environment, not the repository, so a workflow on a lane's branch can't read them. Only release tags (`v*`), `release/*` branches, where the cut's dry runs go, and `main`, the nightly's, may deploy to it, and only the jobs that sign or publish name it: `mac`, `appcast`, `flatpak-repo`, `publish`, `site` and `announce`. `SENTRY_AUTH_TOKEN` stays a repository secret, as branches' Flatpak builds upload symbols too.

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

**Outside this repository.**

| Secret | What |
|---|---|
| `SITE_DISPATCH_TOKEN` | A fine-grained token with Contents read and write on `MartinPTielemans/ilo-site` alone, so the site takes a release at once |
| `ENGINE_RELEASE_TOKEN` | A fine-grained token with Contents read and write on `MartinPTielemans/ilo-engine` alone, for the engine workflows |
| `RELEASE_WEBHOOK_URL` | The announcement bot's webhook, which a published release POSTs `{"tag", "url"}` to, so the bot hears of it at once |
| `RELEASE_WEBHOOK_KEY` | The bearer key that webhook takes |

## The Flatpak repository, once

Before the first release:

1. Make the key: `gpg --quick-gen-key "ilo Flatpak repository" rsa4096 sign never`, with an empty passphrase. Put `gpg --armor --export-secret-keys <fingerprint>` in `FLATPAK_GPG_KEY` and keep a copy offline forever. Every installed copy checks it, and a new key strands them.
2. In Cloudflare, create the R2 bucket, connect a custom domain to it for public reads, and create the API token.
3. Add the secrets and the variable. The address is permanent too, as installs keep it.
