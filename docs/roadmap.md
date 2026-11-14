# Roadmap

Each phase ends with a check. Work that depends on a phase waits for its check. States are as of main, 2026-11-09.

## Phase 0: UX spec

Ardia's Dia spec and parity list, merged with captures of Arc (60 fps recordings, 2× screenshots, measured sizes and timings) into `docs/ux-spec.md` and the parity checklist (`docs/ux-parity/`), a measured value per item.

**Done when** every Phase 3 feature has a spec entry with numbers. **Partly met:** 50 MVP rows still measure an Arc value, and the last eleven takes are due before Arc shuts down on 2026-12-22 (#2177).

## Phase 1: architecture spike

GPUI and a prebuilt Chromium in one Mac process, to settle the run loop, the native page view, overlays over the page, focus, keyboard and IME. A no-go meant Chromium Views.

**Met, go:** `spikes/gpui-cef/` and `spikes/gpui-ardia/` at 91ddfe6 (decision 4).

## Phase 2: engine

ungoogled-chromium built as Chrome's framework from `engine/series`, with embedding hooks, Manifest V2 kept and the password manager on. Prebuilt engines download, so UI work never waits on a build. CI compiles the core on Linux.

**Done when** a prebuilt framework downloads and links into the app. **Met:** `engine/fetch.sh`, and Core on Linux (#2010).

## Phase 3: daily-driver MVP on Mac

Sidebar, Spaces (with the Agent Space kind in the data model), command bar, split view, archive, profiles, extensions (including install prompts and popups), ad blocking on from the first page, passwords and autofill, downloads, find in page, DevTools, import from Arc and Chrome, Sparkle updates, signing and notarization.

**Done when** the parity checklist's MVP items pass and the performance gate holds. **Partly met:** everything ships in 0.1.13, signed and notarized, and Sparkle updates it. 454 of 480 MVP rows pass, and #2178 lists the open rows no other issue tracks. The gate doesn't hold.

## Phase 4: polish, parity and agents

Peek, Little Arc, link rules (Arc's Air Traffic Control), themes, Boosts, end-to-end encrypted sync (not iCloud-only), Widevine, passkeys.

Agents (decision 8): the built-in local MCP server, agent sessions (#1080), their hidden tabs unthrottled as Chrome's captured tabs are, engine-native page access, per-agent and per-site permissions with an activity log, and handoff to the human. Then tasks asked from ⌘T, which the user's own `claude` or `codex` CLI runs through the same server.

**Partly met** (37 of 51 rows). Built:

- Peek, link rules, Little ilo, themes and passkeys;
- the agents' parts (#1481, #1750, #1858).

Open: sync (#589, #1235, #2098) and Widevine (#649).

## Phase 5: Windows, then Linux

Per platform: the platform layer, packaging, updater, signing. Linux runs on X11 and on native Wayland (decision 3). On wlroots' Xwayland its panels can't keep the keys, so use Wayland there (#1646).

**Done when** each platform's build passes the parity checklist's MVP items and the performance gate, and on Linux that holds on native Wayland as well as X11. **Partly met:**

- Both ship, with updates, from 0.1.0 (#454), but Windows is unsigned (#2180).
- Of 61 check runs, 55 pass on X11, 40 on Wayland and 38 on Windows (#2179).
- The gate doesn't hold.

## Performance gate

Measured on every release against Chrome, Helium and Ardia, following the method in Ardia's `docs/perf/vs-browsers.md`.

**Not held.** `./check.sh perf` (#2142) measures only against Chrome, on CI runners, and not on every release (#456).

- Memory at 10 and 20 tabs is 12–62 % over Chrome's (#2138).
- A launch's page waits 2 s for uBlock Origin (#2139).
- A tab switch takes 15–20 ms.

| Metric | Target |
|---|---|
| Cold launch → window on screen | ≤ Chrome |
| Tab switch → shown | ≤ 16 ms (Chrome: 8 ms) |
| New tab → first paint | ≤ Chrome |
| Memory at 1, 10 and 20 tabs | ≤ Chrome |
| Idle CPU and wakeups, 20 tabs | ≤ Chrome |
