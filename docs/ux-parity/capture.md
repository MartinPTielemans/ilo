# Capture

Part of the [UX parity checklist](README.md), whose top says what the columns mean and when to tick a row.

| Done | Item | Phase | Target | Source |
|---|---|---|---|---|
| [x] | Capture mode | MVP | ⇧⌘2 from the page and the UI, File and the command bar; grey page (luma × 0.749), 4 pt ring at radius 6, camera pointer; pill 356 × 67, 35.5 up | Arc 1.170 cap |
| [x] | Element under the pointer, click, drag | MVP | the core's hit test, no page JavaScript; click takes the box, drag the rectangle; Esc, ⇧⌘2, a click outside or a resize leaves. Built (`bc_tab_element_at`, `capture.rs`). Check on macOS `capture` 41784676486, Check on Linux 41784670759 and Check on Windows 41815358057: `capture-element.png` and `capture-chosen.png` with its pointer, click, Esc and ⇧⌘2; a drag's `capture chose 50 300 200 100` and `capture-dragged.png`; a click on the sidebar and a resize each log `capture left (dismissed)` | Arc 1.170 cap, ours |
| [x] | Menu | MVP | at the ring's top left + 10; field, glyphed rows; Copy, Save…, Send via iMessage…, Retake; Edit and New Easel with their feature | Arc 1.170 cap |
| [x] | Capture in ilo's look | MVP | the menu is ilo's menu, as the tab and Space menus are: its panel, 24 pt rows, lines and the Space's accent under the pointer; the pill on its panel; the ring the Space's accent; in light and dark (#829). Built. Check on macOS `capture` 41780140344, with Check on Linux 41781270122: `capture-pill-light.png`, `capture-menu-light.png` and their dark pair, each after its `capture pill drawn` or `capture menu drawn` line | ours |
| [x] | Save to Library | MVP | the menu's first row, as Arc's; saves the PNG into the profile's `Media` without asking, says "Saved to Library" (ours); none in a private window (ours). Built (`capture.rs`, `save_to_library`). Check on macOS `capture` 41780140344, with Check on Linux 41781270122: `capture-library.png`, with its `saved to library` line and the file | Arc 1.170 cap, ours |
| [x] | Toast after Copy | MVP | "Screenshot Copied to Clipboard" with Save and Share; time and hover *interim* | Arc 1.170 cap |
| [x] | Full page | MVP | the whole page past its view, copied; "Capture in Progress" *interim* | ours |
| [x] | Site Controls' capture tiles | MVP | Capture a Portion of This Page and Capture Full Page after Share, in Arc's order; each closes the panel, then runs its command; glyphs *interim* | Arc 1.170.0 cap |
