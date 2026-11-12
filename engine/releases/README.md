# ilo-engine

Prebuilt engines for [ilo](https://github.com/MartinPTielemans/ilo), the browser: the Chromium it runs on, built as Chrome's framework so the app can skip a build of its own. They're here, apart from ilo's repository, so its Releases hold only ilo's.

Each release is one engine, named for the ilo commit it was built from:

- `engine-mac-<commit>`: `Chromium-mac-arm64.tar.xz`, for Macs with Apple silicon.
- `engine-linux-<commit>`: the engine for x86-64 Linux.
- `engine-windows-<commit>`: the engine for x64 Windows, cross-compiled on Linux.

Each file has its SHA-256 beside it. ilo's `engine/fetch.sh` downloads the one built from the nearest commit in a checkout's history. Each release's notes link its commit.

## Source

These are binaries of GPL software, and their source is:

- [ilo](https://github.com/MartinPTielemans/ilo) at the commit a release names, whose `engine/` builds them: `engine/pins.ini` pins the versions, and `engine/series` lists the patches it applies, from `engine/patches/`.
- [ungoogled-chromium](https://github.com/ungoogled-software/ungoogled-chromium), at the tag `engine/pins.ini` names, with its platform repositories, and the Chromium source it patches.

ilo is GPL-3.0, and Chromium and ungoogled-chromium keep their own licenses. This repository holds no source of its own.
