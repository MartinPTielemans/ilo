# Roadmap

Each phase ends with a check. Work that depends on a phase waits for its check.

## Phase 0: UX spec (2–3 days)

- Import Ardia's Dia spec and parity list as the starting point.
- Install Arc and capture it before 2026-12-22: 60 fps recordings of every interaction, 2× screenshots of every state, measured sizes and timings.
- Merge the two into `docs/ux-spec.md` and a parity checklist with a measured value per item.

**Done when** every Phase 3 feature has a spec entry with numbers.

## Phase 1: architecture spike (2–4 days)

GPUI and prebuilt CEF in windowed mode, in one Mac process. CEF stands in for Chrome's framework here. It has the same `NSApplication` requirements, native page view, and focus and IME behavior.

| Day | Work | Risk it answers |
|---|---|---|
| 0 | Read GPUI's macOS platform code: how it creates the `NSApplication`, whether it can attach to a run loop it doesn't own | Run loop, on paper |
| 1 | Chromium runs `[NSApp run]` and GPUI attaches. A page renders as a native view inside a GPUI window next to a GPUI sidebar | Run loop, native view embedding |
| 2 | Command bar as a borderless child window over the page. ⌘T, ⌘L and ⌘W while the page has focus, focus moving between GPUI and the page, Japanese IME in both, resize, fullscreen, frame timing while scrolling | Overlays, focus and keyboard |
| 3–4 | Buffer, or swap CEF for Ardia's prebuilt engine for a closer match | — |

**Go** if all three risks are solved. **No-go** on the run loop or on focus and keyboard means switching the UI to Chromium Views.

## Phase 2: engine (1–2 weeks, can overlap Phase 1)

- ungoogled-chromium built as Chrome's framework, from `engine/series`.
- First patches: embedding hooks, Manifest V2 kept, password manager on.
- Prebuilt engine downloads, so UI work never waits on a Chromium build.
- CI compiles the core on Linux.

**Done when** a prebuilt framework downloads and links into the spike (`spikes/gpui-core/` at abb8164, since removed).

## Phase 3: daily-driver MVP on Mac (about 2–3 months)

Sidebar, Spaces (with the Agent Space kind in the data model), command bar, split view, archive, profiles, extensions (including install prompts and popups), ad blocking on from the first page, passwords and autofill, downloads, find in page, DevTools, import from Arc and Chrome, Sparkle updates, signing and notarization.

**Done when** the parity checklist's MVP items pass and the performance gate holds.

## Phase 4: polish, parity and agents

Peek, Little Arc, link rules (Arc's Air Traffic Control), themes, end-to-end encrypted sync (not iCloud-only), Widevine, passkeys.

Agents (decision 8): the built-in local MCP server, Agent Spaces (their hidden tabs keep running unthrottled, as Chrome keeps a tab it captures), engine-native page access, per-agent and per-site permissions with an activity log, and handoff to the human. Then tasks asked from ⌘T, which the user's own `claude` or `codex` CLI runs through the same server. This can move earlier if it becomes the launch pitch. The core API and data model are built for it from Phase 3 either way.

## Phase 5: Windows, then Linux

Per platform: the platform layer, packaging, updater, signing. Linux first runs on X11, under XWayland on Wayland desktops, then on native Wayland (decision 3).

**Done when** each platform's build passes the parity checklist's MVP items and the performance gate, and on Linux that holds on native Wayland as well as X11.

## Performance gate

Measured on every release against Chrome, Helium and Ardia, following the method in Ardia's `docs/perf/vs-browsers.md`.

| Metric | Target |
|---|---|
| Cold launch → window on screen | ≤ Chrome |
| Tab switch → shown | ≤ 16 ms (Chrome: 8 ms) |
| New tab → first paint | ≤ Chrome |
| Memory at 1, 10 and 20 tabs | ≤ Chrome |
| Idle CPU and wakeups, 20 tabs | ≤ Chrome |
