# Roadmap

Each phase ends with a check. Work that depends on a phase waits for its check.

## Phase 0: UX spec (2–3 days)

- Import Ardia's Dia spec and parity list as the starting point.
- Install Arc and capture it before 2026-12-22: 60 fps recordings of every interaction, 2× screenshots of every state, measured sizes and timings.
- Merge the two into `docs/ux-spec.md` and a parity checklist with a measured value per item.

**Done when** every Phase 3 feature has a spec entry with numbers.

**State (main, 2026-11-09): partly met.** `docs/ux-spec.md` and the parity checklist (`docs/ux-parity/`, one file per area) hold every Phase 3 feature, from Ardia's spec and the Arc captures so far. 50 of the checklist's 477 MVP rows still say "measure" for an Arc value, 42 of them ticked on an interim value. The spec's last eleven takes need the owner's hands in Arc before it shuts down on 2026-12-22 (#2177).

## Phase 1: architecture spike (2–4 days)

GPUI and prebuilt CEF in windowed mode, in one Mac process. CEF stands in for Chrome's framework here. It has the same `NSApplication` requirements, native page view, and focus and IME behavior.

| Day | Work | Risk it answers |
|---|---|---|
| 0 | Read GPUI's macOS platform code: how it creates the `NSApplication`, whether it can attach to a run loop it doesn't own | Run loop, on paper |
| 1 | Chromium runs `[NSApp run]` and GPUI attaches. A page renders as a native view inside a GPUI window next to a GPUI sidebar | Run loop, native view embedding |
| 2 | Command bar as a borderless child window over the page. ⌘T, ⌘L and ⌘W while the page has focus, focus moving between GPUI and the page, Japanese IME in both, resize, fullscreen, frame timing while scrolling | Overlays, focus and keyboard |
| 3–4 | Buffer, or swap CEF for Ardia's prebuilt engine for a closer match | — |

**Go** if all three risks are solved. **No-go** on the run loop or on focus and keyboard means switching the UI to Chromium Views.

**State: met, go.** `spikes/gpui-cef/` and `spikes/gpui-ardia/` at 91ddfe6, since removed, solved all three risks (decision 4), and `app/` is built on that design.

## Phase 2: engine (1–2 weeks, can overlap Phase 1)

- ungoogled-chromium built as Chrome's framework, from `engine/series`.
- First patches: embedding hooks, Manifest V2 kept, password manager on.
- Prebuilt engine downloads, so UI work never waits on a Chromium build.
- CI compiles the core on Linux.

**Done when** a prebuilt framework downloads and links into the spike.

**State: met.** `engine/series` applies the embedding hooks (`ours/chromium-core-*`) and `ours/chromium-prefs-passwords-autofill-on.patch` over ungoogled-chromium, whose own patch keeps Manifest V2. `engine/fetch.sh` downloads each OS's prebuilt engine from ilo-engine, which the app links when there's no local build. CI compiles the core on Linux (Core on Linux, #2010).

## Phase 3: daily-driver MVP on Mac (about 2–3 months)

Sidebar, Spaces (with the Agent Space kind in the data model), command bar, split view, archive, profiles, extensions (including install prompts and popups), ad blocking on from the first page, passwords and autofill, downloads, find in page, DevTools, import from Arc and Chrome, Sparkle updates, signing and notarization.

**Done when** the parity checklist's MVP items pass and the performance gate holds.

**State (main, 2026-11-09): partly met.** Every feature listed ships in 0.1.12, and on the Mac in a signed, notarized DMG that Sparkle updates, with deltas.

- **Parity:** 434 of the checklist's 477 MVP rows pass. Eight of the 43 open ones have issues of their own (#453, #671, #678, #686, #700, #1400, #1636), and #2178 tracks the rest, most of them built and waiting on a Mac run.
- **Performance gate:** it doesn't hold (Performance gate, below).

## Phase 4: polish, parity and agents

Peek, Little Arc, link rules (Arc's Air Traffic Control), themes, end-to-end encrypted sync (not iCloud-only), Widevine, passkeys.

Agents (decision 8): the built-in local MCP server, Agent Spaces (their hidden tabs keep running unthrottled, as Chrome keeps a tab it captures), engine-native page access, per-agent and per-site permissions with an activity log, and handoff to the human. Then tasks asked from ⌘T, which the user's own `claude` or `codex` CLI runs through the same server. This can move earlier if it becomes the launch pitch. The core API and data model are built for it from Phase 3 either way.

**State (main, 2026-11-09): partly met.** The phase has no exit check of its own. 17 of the checklist's 41 Phase 4 rows pass.

- **Built:**
  - Peek and link rules (#692);
  - Little ilo, with #693 and #1407 open for the rest of Arc's;
  - themes and passkeys;
  - the agents' MCP server, Agent Spaces, permissions, activity, handoff and ⌘T tasks (#1481, #1750, #1858).
- **Open:**
  - sync (#589), with extensions and settings (#1235, #2176) and Today tabs (#2098) on it. The self-hosted server is checked by ilo-server#31.
  - Widevine, which waits on its VMP signature (#649).
- The rows built but not ticked are #2178's.

## Phase 5: Windows, then Linux

Per platform: the platform layer, packaging, updater, signing. Linux runs on X11 and on native Wayland (decision 3). On wlroots' Xwayland its panels can't keep the keys, so use Wayland there (#1646).

**Done when** each platform's build passes the parity checklist's MVP items and the performance gate, and on Linux that holds on native Wayland as well as X11.

**State (main, 2026-11-09): partly met.**

- **Shipping:** Windows and Linux have shipped since 0.1.0 (#454):
  - Windows as an installer, an MSIX and on winget, updating in the background (#1418);
  - Linux as a Flatpak from ilo's own GPG-signed repository, on X11 and native Wayland (#411).
- **Signing:** Windows' installer and app are unsigned (#2180).
- **Parity:** of `check.sh`'s 61 runs, 55 pass on X11, 40 on Wayland and 38 on Windows. The checklist ticks rows against the Mac alone (#2179).
- **Performance gate:** it doesn't hold on either OS (below).

## Performance gate

Measured on every release against Chrome, Helium and Ardia, following the method in Ardia's `docs/perf/vs-browsers.md`.

**State (main, 2026-11-09): not held, and only partly measured.**

- **What's measured:** `./check.sh perf` (#2142) follows Ardia's method on each OS against Chrome on the same runner. It doesn't run on every release yet, or against Helium and Ardia (#456). On GitHub's runners (42522744185 Linux, 42522746429 Mac, 42522748582 Windows):
  - **Cold launch to window:** 367–810 ms against Chrome's 424 on Ardia's Mac.
  - **Tab switch:** medians of 15–20 ms, against the gate's 16.
  - **Memory at 10 and 20 tabs:** 12–62 % over Chrome's (#2138).
  - **New tab:** not yet compared with Chrome.
- **Launch:** a launch's page waits about 2 s for uBlock Origin (#2139). The native ad-block engine (#2111) removed it in a Linux proof run (42527042326).

| Metric | Target |
|---|---|
| Cold launch → window on screen | ≤ Chrome |
| Tab switch → shown | ≤ 16 ms (Chrome: 8 ms) |
| New tab → first paint | ≤ Chrome |
| Memory at 1, 10 and 20 tabs | ≤ Chrome |
| Idle CPU and wakeups, 20 tabs | ≤ Chrome |
