# UX parity checklist

One row per item in [ux-spec.md](../ux-spec.md). The spec holds the full rule and wins when the two disagree; change both in the same commit. When an Arc capture replaces a value, change the row's target and source.

- Only the integrator's daily `docs/parity-ticks-<date>` PR ticks a row, or changes the roadmap's counts, and only once the running app matches its target, checked against a capture of our window (AGENTS.md › Verifying), with the run that proved it. A feature's PR names the rows it completes on its `Parity:` line and keeps its new rows and measured values (#2204).
- **A tick is the Mac's.** Windows and Linux are tracked by run, not by row: `tools/runs-per-os.py` prints, for X11, Wayland and Windows, each of `check.sh`'s default runs as passing, skipped with its reason, or not listed, and under each passing run the steps it skipped there and why (#2179). Only a skip for the system's nature is done: one that says "yet" or cites an issue is open, and the script lists those with no issue. A row proven by the UI's tests alone holds on every system, as CI runs `cargo test` on each. A Mac-worded row follows the spec's rule for the other systems, such as Ctrl for ⌘ and the command bar for the menu bar.
- **Phase.** MVP is Phase 3, 4 is Phase 4, and later is anything not yet on the roadmap.
- **Target.** "measure" means there's no number yet; the item is in the spec's Arc capture plan.
- **Source.** The spec's source tags: `D›` is Ardia's `dia-spec.md`, `P§` its `dia-feature-parity.md`, `O` its `omnibox-ux.md`.
- **Attribution.** Items marked Dia, Arc (Ardia) and Ardia are adapted from Ardia (https://github.com/mantrakp04/ardia) at `v0.2.40-nightly.5`, commit `fce3a02ffb917d2902af4ce47b398ce8b03a440d`, Copyright 2026 Mantra Patel, Apache-2.0. The spec's Attribution section has the full notice.

## Areas

One file per area, so lanes working on different areas edit different files. A new area gets its own file and a line here.

- [Window](window.md)
- [Sidebar](sidebar.md)
- [Spaces](spaces.md)
- [Command bar](command-bar.md)
- [New tab](new-tab.md)
- [Split view](split-view.md)
- [Settings](settings.md)
- [Archive](archive.md)
- [Profiles](profiles.md)
- [Extensions](extensions.md)
- [Ad blocking](ad-blocking.md)
- [Passwords and autofill](passwords-and-autofill.md)
- [Downloads](downloads.md)
- [Site notifications](site-notifications.md)
- [Find in page](find-in-page.md)
- [Zoom](zoom.md)
- [Link target](link-target.md)
- [DevTools](devtools.md)
- [Developer Mode](developer-mode.md)
- [Capture](capture.md)
- [Boosts](boosts.md)
- [Import](import.md)
- [Updates](updates.md)
- [Keyboard](keyboard.md)
- [Phase 4](phase-4.md)
- [Later](later.md)
