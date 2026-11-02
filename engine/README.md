# Engine

Chromium built as Chrome's framework (`//chrome:chrome_framework`), from ungoogled-chromium plus our patch series. `./setup.sh` prepares the tree in `.deps/src`, and `./build.sh` builds `Chromium.app`, with the framework in it, in `.deps/src/out/Release`. `./setup.sh dev` and `./build.sh dev` do the same for the dev build in `.deps/src/out/Dev`, which has no link-time optimization, so its framework relinks much faster, and checks Chromium's DCHECKs. Work on the core uses it.

Work that doesn't change the core needs no build: `./fetch.sh` downloads the prebuilt engine built from the nearest commit in the checkout's history, a release `engine-mac-<commit>` of the repository `prebuilt.ini` names, [ilo-engine](https://github.com/MartinPTielemans/ilo-engine), and the app uses it when there's no local build. A prebuilt engine has its commit's core, so `fetch.sh` refuses it once `core/public/browser_core.h` has changed; publish a new one with the API change (The Mac's prebuilt engine, below).

## Layout

| Path | What |
|---|---|
| `pins.ini` | Chromium version and the ungoogled-chromium, ungoogled-chromium-macos, ungoogled-chromium-portablelinux and ungoogled-chromium-windows tags. Bumped together, every Chromium release |
| `series` | Ordered list of patches applied after ungoogled-chromium's own series |
| `patches/upstream/` | Fixes backported from Chromium after the pinned version, header naming the commit; dropped once the pin has it |
| `patches/helium/` | Patches taken from Helium, header naming the upstream path and commit |
| `patches/ardia/` | Patches taken from Ardia, same convention |
| `patches/ours/` | Our own patches |
| `args.gn` | Our GN args, after ungoogled-chromium's `flags.gn` and ungoogled-chromium-macos's `flags.macos.gn` |
| `args.dev.gn` | The dev build's args, after `args.gn` |
| `tree.sh` | What `setup.sh` and `linux/prepare.sh` both do to a prepared tree: remove Chromium's agent instructions, link the core and apply `series` |
| `check-linux.sh` | Compiles the core on Linux, as CI does |
| `linux/` | Prepares, builds and packs the Linux engine |
| `point-release.sh` | Finds a security point release and dry-runs the patches on it, for the daily Action (Point releases, below) |
| `windows/cross/` | Prepares, builds and packs the Windows engine, cross-compiled on Linux |
| `windows/` | ungoogled-chromium-windows's own build with our hooks, for GitHub's Windows runners |
| `prebuilt.ini` | The repository whose releases hold the prebuilt engines `fetch.sh` downloads, apart from ilo's so its Releases hold only ilo's, which a fork changes |
| `releases/README.md` | That repository's README |

## Order

`setup.sh` does this, using ungoogled-chromium-macos's tooling at the pinned tag, which is how its releases are built:

1. Download Chromium's source tarball at the pinned version, checked against its published hashes.
2. Apply ungoogled-chromium's pruning, its patches and ungoogled-chromium-macos's, and domain substitution.
3. Download the toolchain ungoogled-chromium-macos names (LLVM, Rust, Node), in place of Google's.
4. Download Chrome's optimization profile for Apple silicon, which the source tarball lacks, and build `gn` and `bindgen`, each only if it's missing.
5. Link the repository's `core/` into the tree as `//browser_core`, so the next build has every edit to it. Apply `series` in order, with no fuzz. A patch already in the tree is skipped, and one that neither applies nor is in the tree stops the run as drift.
6. Write `out/Release/args.gn` and run `gn gen`.

Steps 1–3 change the tree and run once; a run that stopped in one of them means starting over with a fresh tree. Steps 4–6 run every time, so adding a patch to `series` and running `setup.sh` again applies it. Once `pins.ini` names another version, a plain run stops, and `setup.sh update` takes it instead (Point releases, below).

`setup.sh` names what it needs and isn't installed: Homebrew's `ninja`, `go`, `esbuild`, `typescript`, `coreutils` and `uv`, and Apple's Metal toolchain. It builds with Python 3.13, as ungoogled-chromium-macos does, through `uv`.

It also needs the macOS SDK of Chromium's official builds (`mac_sdk_official_version` in `build/config/mac/mac_sdk.gni`: 26.5 for Chromium 156), from Xcode or the Command Line Tools, even when a newer Xcode is installed. The pinned LLVM's linker can't read newer SDKs.

## The Mac's prebuilt engine

The owner's MacBook builds the Mac engine; no GitHub runner is installed on it, so nothing on GitHub starts code there (#871). `tools/mac-engine-publish.sh`, after `build.sh`, packs the Release `Chromium.app` into `Chromium-mac-arm64.tar.xz` and publishes it with its SHA-256 as the release `engine-mac-<commit>` in ilo-engine, named for the commit it was built from, which must be clean in `engine/` and `core/` and on GitHub. `fetch.sh` on a Mac takes the one nearest the checkout's history, as on Linux and Windows.

`tools/mac-nightly.sh install` starts a launchd agent that runs at 03:00, or when the Mac wakes after a night it slept through. When main's core or engine differ from the newest `engine-mac` release in main's history, it builds main's Release engine in this checkout's tree, from a worktree of main in `~/.cache/ilo-mac-nightly`, and publishes it. It shares the one tree with the integrator's builds and the prod updater, so it skips the night while one is building, when the tree holds a patch no commit of main had, or when the tree's Chromium isn't the one main pins. A point release waits for the integrator's `setup.sh update`. A build started while the night's runs shares the tree with it, and after a night the tree links the checkout's core again but holds main's series, which the integrator's next `build.sh` replaces with their checkout's. It never runs the browser. `status` shows whether it's on and how the last nights went, from `~/Library/Logs/ilo-mac-nightly.log`, and `uninstall` stops it and removes the worktree.

`.github/workflows/check-mac.yml` runs `app/check.sh` on that engine on GitHub's free hosted `macos-26` runners, on main each night after that build, and by hand, which the integrator does on main after each merge train. They're VMs of 3 cores and 7.5 GB with a 1024 by 768 screen, where the driven checks work but scrolling draws 35–41 frames a second, so performance is measured on the owner's Mac only.

## Linux

`linux/prepare.sh <work>` prepares the tree in `<work>/src` with ungoogled-chromium-portablelinux's scripts at the tag `pins.ini` names, links the core and applies `series`. A new tag is unpacked and patched in `<work>/next` first, then copied over the tree with `rsync --checksum`, so a point release leaves unchanged files as they were and the next build recompiles only what it changed. `linux/build.sh <work>` builds `chrome`, with the core linked in, and `linux/package.sh` packs it with what it runs with into `Chromium-linux-x64.tar.xz`.

`linux/test.sh <work>` builds and runs the core's unit tests, `browser_core_unittests`, as the Linux engine's run does. On Linux base's test support loads Chromium's test fonts, which the source tarball leaves out, so it fetches the object DEPS pins for them from Google's `chromium-fonts` bucket, checks its hash, and keeps it in the tree; a Chromium bump updates the pin in `test.sh`. They're only read by the tests, and `package.sh` never packs them.

`check-linux.sh` prepares the same tree and compiles only the core's own objects; the `browser_core` target would compile most of Chrome, and nothing links. CI (`.github/workflows/core-linux.yml`) runs it on pushes that change the core or the engine, in about half an hour and 40 GB of disk.

The engine itself is built by `.github/workflows/engine-linux.yml`, on a self-hosted runner on the owner's machine that keeps the tree in `$ILO_ENGINE_ROOT/linux` between runs. It runs by hand and on pushes to main that change `pins.ini` or the Linux engine's scripts, never on branches or pull requests. Each run publishes a release `engine-linux-<commit>` in ilo-engine, with the secret `ENGINE_RELEASE_TOKEN`, a fine-grained token with Contents read and write on that repository alone. A release's tag is on ilo-engine's own history, which hasn't ilo's commits, so `fetch.sh` reads the commit from the release's name. And `fetch.sh` on Linux downloads the one built from the nearest commit in the checkout's history, unless `core/public/browser_core.h` has changed since. The first build takes 6 to 10 hours and the tree needs about 100 GB, twice the source while a new version comes in.

## Point releases

Chromium's security point releases (same major, such as 156.0.8078.47 → .60) update the engine without asking the owner. A new major waits for the owner's go.

- **Detect.** `.github/workflows/engine-point-release.yml` runs `point-release.sh` daily on GitHub's runners. For each platform section in `pins.ini`, it finds the newest tag on the pinned major. The bump follows the Mac, which never waits on another platform: `[chromium]` moves to the Mac's newest point release, and each section on the same version that has a tag for it moves too. A section whose tag lags stays on the Chromium its tag names, which `check_ungoogled` in `tree.sh` checks alone, and follows in its own PR once its tag is out. It extracts only the files the patches touch from Chromium's source tarball and dry-runs ungoogled-chromium's patches, the platform's, domain substitution and `series`. It then opens one PR bumping `pins.ini`, with "applies cleanly" or the conflicting patches, and the patch lag: the days since Chrome's stable release of that version. A new major gets an issue asking the owner, once.
- **Update in place.** `setup.sh update` (and `setup.sh update dev`) prepares the new version in `.deps/next`, series and all, then copies it over `.deps/src` with `rsync --checksum --delete`, leaving `out/` alone. Unchanged files keep their dates, so ninja rebuilds only what changed. The scratch tree needs about 25 GB while it runs. Afterwards, `setup.sh` for the other build regenerates its build files.
- **Merge.** The integrator builds Dev and Release incrementally, runs the full check, merges, and tags `good/…`. The PR comes from `GITHUB_TOKEN`, so no workflow runs on it. A conflict goes to a lane, as for a major.

## Windows

The Windows engine is cross-compiled on Linux, as Chromium's `docs/win_cross.md` describes, by `.github/workflows/engine-windows-cross.yml` on the runner that builds the Linux engine. It keeps its tree in `$ILO_ENGINE_ROOT/windows`, runs nightly on main, on pushes to main that change the pin, the series or its scripts, and by hand, but not for each core change, which would hold the Linux engines' one runner, and publishes a release `engine-windows-<commit>` in ilo-engine, as the Linux engine's, which `fetch.sh` downloads on Windows and `app/setup.sh` copies into `app/.deps/engine`. Until the next one, Windows takes the nearest engine in its history, up to a day's core changes behind main, and none after a change to `core/public/browser_core.h`.

`windows/cross/prepare.sh <work>` takes ungoogled-chromium-windows's patches and Windows-only sources at the tag `pins.ini` names, on Chromium's source tarball, with ungoogled-chromium-portablelinux's Linux toolchain (the same ungoogled-chromium commit, which the script checks). Of ungoogled-chromium-windows's patches it leaves out the ones that fit its build to a Windows host or its own LLVM, and adds portablelinux's for the Linux host's Python and Node tools. A new tag comes in through `<work>/next` as on Linux. Three exceptions to ungoogled-chromium's pruning stay in the tree: the outputs of Microsoft's `midl` and `mc` that Chromium checks in under `win_build_output/` and `chrome/app/etw_events/`, and windows-rs's import libraries. Those tools don't run on Linux, so a cross build copies the checked-in outputs, as Google's own does. `windows/cross/build.sh` builds `chrome`, and `windows/cross/package.sh` packs what Chromium's `FILES.cfg` lists, with `chrome.dll.lib`, which the app links, into `Chromium-windows-x64.zip`.

Microsoft's compiler, headers and libraries come from a zip of a Visual Studio and Windows SDK install, which Microsoft's licence lets no one redistribute. Never upload it, as a release, an artifact or anywhere else. It sits in `$ILO_ENGINE_ROOT/windows-toolchain`, the only `.zip` there, and unpacks into `vs_files` beside it, which must be a case-insensitive `ciopfs` mount, since the SDK's headers include each other in any case. The runner can't mount one itself, so a service of the owner's keeps it mounted. Its Visual Studio and SDK are the ones `build/vs_toolchain.py` names (Visual Studio 2026 and SDK 10.0.28000.0 for Chromium 156), and `prepare.sh` stops when a new Chromium version names others. To make the zip, on a Windows machine with that Visual Studio (its C++ workload with ATL and MFC) and that SDK, with its debugging tools:

```
git clone https://chromium.googlesource.com/chromium/tools/depot_tools.git
git -C depot_tools checkout <the depot_tools commit in Chromium's DEPS>
cd depot_tools\win_toolchain
python package_from_installed.py <VS version> -w <SDK version> --noarm
```

The zip is named after its contents' hash. The first build takes about as long as the Linux engine's, and the tree, the toolchain and a new version coming in need about 150 GB.

`windows/prepare.sh` is ungoogled-chromium-windows's own build on Windows, which `.github/workflows/engine-windows.yml` runs by hand on GitHub's runners, for when the cross build can't.

Both add `enable_playready=true` to the args. Chromium sets it only in Chrome-branded builds, and nothing else on PlayReady's path checks branding, so with it protected video plays through Windows' own PlayReady (decision 1).

## Releases

A copy for other Macs is the Release engine with the prod app in it. `app/build.sh prod` builds it, and `SIGN_IDENTITY="Developer ID Application: … (TEAMID)" tools/release.sh` signs each part inside out with the hardened runtime, notarizes the app and then a DMG through the Keychain's `notary` profile (`xcrun notarytool store-credentials notary`), and staples both, leaving `app/build/release/ilo.dmg`. Only a Developer ID passes Gatekeeper on another Mac. The entitlements are Chromium's, trimmed to what a browser outside App Sandbox uses, in `app/entitlements/`. `tools/install.sh` signs the owner's own copy instead, on its Apple Development identity.

The copy updates itself with Sparkle 2 (`app/platform/mac/updates.mm`), from the appcast at the default service URL in `app/branding/update-feed`: a file in this repository's release `updates`, which forks change. The version is `app/Cargo.toml`'s and the build number, which Sparkle orders updates by, is the count of commits, so releases come from main. Each update is signed with the owner's EdDSA key, which never leaves their Keychain; `build.sh prod` puts its public half in the app from `app/branding/update-key`, which forks replace (`UPDATE_PUBLIC_KEY` overrides it), and with neither builds a copy with no updater, which `release.sh` refuses. `tools/appcast.sh notes.md --publish` adds the DMG to the appcast with its Markdown notes, publishes it as the release `v<version>` and uploads the appcast. Its header has the one-time key steps and a release's commands.

Windows updates the same way, with WinSparkle (`app/platform/windows/updates.cc`), from the same appcast and key. `tools/appcast.sh notes.md --windows <installer.exe>` signs the installer on the Mac, with the Keychain's key, and adds its entry, marked `windows-x64` while the Mac's are marked `macos`: each updater takes an unmarked entry as its own. The installer is built from the DMG's commit, so the two share a build number. A release build of `Browser.dll` has the feed, the key from `app/branding/update-key` (or `UPDATE_PUBLIC_KEY`) and the version built in, and loads `WinSparkle.dll` from beside it, which `app/setup.sh` fetches and the Windows package carries. A copy from the Microsoft Store has a package identity and leaves its updates to the Store.

On Linux, ilo is the Flatpak on Flathub, and Flathub's builds are its updates; the appcast isn't read. Flatpak's portal (`app/src/flatpak.rs`) tells the running copy when Flathub has a newer build, which it installs beside itself. It then offers to restart, starting the new build through the portal once Chromium has let go of the profile. Outside a Flatpak, as in a dev build, there are no updates and no Check for Updates….

macOS grants a Keychain item to an app's designated requirement, which a Developer ID ties to the team and an Apple Development certificate to the certificate. A copy whose signature changed, such as the owner's moving to a release, asks once for its "ilo Safe Storage" key, the one Chromium encrypts cookies and passwords with. The app reads it before Chromium starts, so a Deny gets an alert that explains it and asks again, rather than a browser signed out of everything. Site permissions macOS keeps (camera, microphone, location) are asked again too; the profile, in Application Support, stays.

## What reaches Google

ungoogled-chromium cuts the browser's calls to Google. Ardia's patches in `patches/ardia/` bring back two of them, for security only (decision 2).

- **Security components.** The component updater checks `https://update.googleapis.com/service/update2/json` for three components only: CRLSet (revoked certificates), PKI Metadata (Chrome Root Store, CT logs, pins) and the SSL error assistant. Every other component stays out of the request and is never downloaded. The first check runs a minute after launch, then one every five hours. Downloads may come only from `dl.google.com`, `*.gvt1.com` or `www.google.com/dl/`, over https, and redirects are held to the same hosts.
  - **What a check sends.** In the body: the protocol version, Chromium's version and channel, the updater's version, macOS's version, the CPU architecture, and a random session id and request id. For each component it sends the id and the installed version. The headers repeat the ids and say whether the check runs in the foreground.
  - **What it doesn't send.** No cookies and no pings. None of the fields Chrome uses to link one check to the next (install id, install date, cohorts, ping day counts). No language, hardware or enterprise state. Google still sees the IP address, and the versions can help it link checks.
  - **How responses are checked.** CUP signatures and the CRX3 publisher key, as in Chrome.
  - **Google's Widevine module, on Linux.** Once the user says yes in the protected-media note or Settings, the checks also ask for the Widevine component (`oimompecagnajdejgnnjijobebaeigek`), the first at once, and it downloads from the same hosts. Removing it in Settings takes it out of the checks again. `patches/ours/chromium-widevine-component.patch` lets the core add it to Ardia's list, whose update URL is already Google's own.
- **Chrome Web Store.** The browser talks to it on three occasions, and with no store extension installed only the first:
  - The store's own page, chromewebstore.google.com, gets Chrome's install API and sees the installed extensions, as in Chrome.
  - An install the user confirms fetches the extension's icon from the address the page gives, Google's image host. It then downloads the extension from clients2.google.com, which redirects to clients2.googleusercontent.com. The download carries the profile's cookies and the store page as referrer, as in Chrome.
  - Installed store extensions are checked for updates at clients2.google.com, soon after launch and every 5 hours (±20%) after. The check sends each one's id and version, and the browser's OS, architecture, version and language, without cookies or usage pings. An update downloads from clients2.googleusercontent.com.
- **Extension kill switch.** The extension update check's `_malware`, `_policy_violation` and `_potentially_uws` answers blocklist or disable the extension, as in Chrome. That check covers only Web Store extensions, which install and update again through `patches/ours/chromium-extensions-web-store.patch`.

## Crash reports

Crashpad writes a minidump of every crash, on every platform, as Chrome's does
(`patches/ours/chromium-crashpad-start.patch` and, on the Mac, `chromium-crashpad-mac-handler.patch`
undo ungoogled-chromium's two switches that stopped it). On the Mac the browser doesn't wait for the
handler to start, which held up launch by about 670 ms (`chromium-crashpad-mac-async-start.patch`),
so a crash of the browser in its first moments isn't caught. Its handler never uploads one:
ungoogled-chromium's hunk that takes out the handler's upload thread and its URL stays, and the core
sends them instead, only if the user says to (`docs/decisions.md`, 9). The reports are in the
profile folder's `Crashpad` on the Mac and Linux (`core/crash_reports.cc`). On Windows,
`chrome_elf` starts Crashpad before the core runs, in the user data folder Chromium names after the
product, which `patches/ours/chromium-windows-product-folder.patch` names after
`app/branding/name`: `%LOCALAPPDATA%\ilo\User Data\Crashpad`, the prod profile's. A dev build's
reports go there too. The core sends, offers and deletes reports only in the profile's own folder,
so a build whose profile is elsewhere, as a dev build's is, does none of that.

- **What's sent.** With Send Crash Reports on, the core sends each report not yet sent to the build's
  collector, the minidump alone, without cookies, and deletes it once the collector has it. One
  that fails waits for the next launch: the handler's own upload thread would wake every 15
  minutes, and no timer replaces it. Turning it off deletes the reports not yet sent. While it's
  off, the launch after the browser crashed offers once to send that crash's report alone, and
  Don't Send deletes it.
- **Pages left out.** Chrome records the page's URL in crash keys; the core's content client
  doesn't. Reports leave out the memory that pointers on the stack lead to, which can hold a
  page's, and Chrome's dumps of processes that didn't crash. The stacks can still hold a URL or
  page text, which is why sending is the user's to turn on.
- **The collector.** `app/branding/crash-reports` holds the minidump endpoint of ilo's Sentry
  project, in Sentry's EU region (organization `ilo-rg`, project `ilo`):
  `https://<host>/api/<project id>/minidump/?sentry_key=<key>`, read off the project's DSN,
  `https://<key>@<host>/<project id>`. The key is public, as every client carries it. Only the
  prod build (`app/build.sh prod`, as every release is) names it, so a dev build or a check never
  sends to it, and Settings › Privacy names it "Sentry (EU region)". Empty, as a fork's starts,
  the build sends nothing and Settings has no Privacy tab. A fork names its own project's
  endpoint, Sentry's or a self-hosted collector's that takes Sentry's minidump endpoint.
- **Symbols.** When the build names a collector, each release job uploads its OS's with
  `tools/symbols.sh`, through `sentry-cli debug-files upload`, to the same project.
  The Mac's are the Release engine's dSYMs and the app's executable; that engine builds with
  ungoogled-chromium-macos's `symbol_level=1` and `blink_symbol_level=0`, so frames have functions
  and lines outside Blink, and Blink's only their library. Linux and Windows build with their
  ungoogled-chromium's `symbol_level=0`, so frames have functions and no lines: on Linux from the
  symbol tables of `chrome` and `libBrowser.so`, which the Flatpak keeps, and on Windows from PDBs,
  the engine's in its release beside it (`windows/cross/package.sh`, `fetch.sh symbols`) and
  `Browser.pdb`. Lines there would take `symbol_level=1`, a full rebuild of each engine.

## Rules

- Keep patches small and separate. New code goes in new files under `//browser_core` where possible. Patches to Chrome's files add hooks; they don't move logic.
- Domain substitution rewrites Google's hosts in the source before the series is applied, so a patch that needs a real host writes it in full, and nothing rewrites it.
- One concern per patch, named `chromium-<area>-<what>.patch`.
- Each release, diff Helium's and Ardia's series against ours for anything worth taking.
- A new Chromium major, or a change to the GN args, means a full build. A point release taken with `setup.sh update` rebuilds only what it changes.
- An Xcode update doesn't: `chromium-xcode-build-define.patch` drops the `CR_XCODE_BUILD` define, which upstream compiles every object with only so that one does.
