# ilo

a good tool.

<img alt="ilo: a window with the sidebar of Spaces and tabs on the left and a web page on the right" src="docs/images/ilo-light.webp">

ilo is an open-source web browser. Your tabs live in a sidebar, in Spaces: each Space keeps its own pinned tabs and today's tabs, and today's tabs you leave alone move to the archive. A command bar opens pages, tabs and commands, and two pages can share a window side by side. Underneath is Chromium, by way of ungoogled-chromium, so websites work as they should, and extensions install from the Chrome Web Store.

ilo brings your tabs, bookmarks and history over from the other browsers on your computer, and your passwords from a file you export, and syncs with an ilo account, end-to-end encrypted. It saves your passwords and fills them in, and blocks ads and trackers from the first page. Protected video plays on the Mac through macOS's own engine and on Linux with Widevine; on Windows it's untested on real hardware.

ilo is also the browser AI agents work in. Claude Code, Codex or any MCP client can work in your logged-in sessions, in a Space of its own, with only the permissions you give it, and it never reads your passwords or cookies. Agents are off until you turn them on.

ilo is free and licensed under the GPL. It has no paid features, no license checks and no ads, and it doesn't sell your data. Services that hold your data, such as sync, can be self-hosted.

## Install

Every release is on [GitHub Releases](https://github.com/MartinPTielemans/ilo/releases), with its notes. Once installed, ilo keeps itself up to date.

### macOS

macOS 15 (Sequoia) or later, on Apple silicon. Download [ilo-mac-arm64.dmg](https://github.com/MartinPTielemans/ilo/releases/latest/download/ilo-mac-arm64.dmg), open it and drag ilo into Applications.

### Windows

Windows 10 (1809) or later, on x64. Download [ilo-windows-x64-setup.exe](https://github.com/MartinPTielemans/ilo/releases/latest/download/ilo-windows-x64-setup.exe) and run it. It isn't signed yet, so Windows may warn you before it runs.

ilo will also be on winget (`winget install ilo.ilo`) and in the Microsoft Store, once each has accepted it.

### Linux

x86-64, as a Flatpak:

```bash
flatpak install --user https://github.com/MartinPTielemans/ilo/releases/latest/download/ilo.flatpakref
```

Then start ilo from your apps. It updates with your other Flatpaks.

## Build from source

The app builds on a prebuilt engine that `app/setup.sh` downloads, so you don't need to build Chromium:

```bash
cd app
./setup.sh
./build.sh
```

- **macOS:** this makes `app/build/Browser.app`.
- **Linux:** this makes `app/build/Browser`, and `tools/flatpak.sh` packs it as a Flatpak.
- **Windows:** run `./setup.sh` in Git Bash, then `cargo build --release --manifest-path windows/Cargo.toml` builds `Browser.dll`, which the engine's `chrome.exe` loads.

## Documentation

- [Decisions](docs/decisions.md): why ilo is built the way it is.
- [Roadmap](docs/roadmap.md): what comes next.
- [Building the engine](engine/README.md): Chromium and ilo's patches.
- [Releasing](docs/releasing.md): how a release is made.

## Contributing

ilo is in beta. Please report bugs in [Issues](https://github.com/MartinPTielemans/ilo/issues/new/choose); in ilo, Help › Report a Problem… fills in the details for you. [CONTRIBUTING.md](CONTRIBUTING.md) says what's likely to land and how to send it. Contributions use the [Developer Certificate of Origin](CONTRIBUTING.md#sign-off), not a CLA.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE), and [NOTICE](NOTICE) for the other projects ilo uses and carries; in the app, Settings › About opens both. The name ilo and its icon are the project's trademarks and aren't covered by the license. Forks are welcome under their own name: replace `app/branding/` and recompile the icon with `app/icon.sh`. On Windows the engine carries the name too, so a fork builds its own Windows engine ([engine/README.md](engine/README.md)).
