# Downloads

Part of the [UX parity checklist](README.md), whose top says what the columns mean and when to tick a row.

| Done | Item | Phase | Target | Source |
|---|---|---|---|---|
| [x] | Library section, ⇧⌘J, View Downloads | MVP | above Archived Tabs; rows as the archive's, with the system's icon, status and bar | Arc 1.170.0 menus, cap |
| [x] | Row actions | MVP | progress, pause/resume, cancel, open, Show in Finder, Open When Done, keep or delete a held file, Remove from List, Clear List | Dia P§14 |
| [x] | Move to Trash, drag out | MVP | to Finder, other apps, the page, the tab list | Dia P§14 |
| [x] | Footer button while downloading | MVP | ours: glyph over a 16 × 2 bar *interim*; Arc's look: measure | — |
| [x] | Recent downloads over the Library button | MVP | up to 4 finished, newest at the bottom, 56 apart: 40 pt preview or 26 pt icon at 8, name 15 pt at 61 over its age; ours: after a 0.5 s rest (its own, not the tooltip's 1.6 s), 0.15 s fade, a row acts as the Library's *interim*; tooltip with key chips and glyph motion: #189 | owner's Arc cap |
| [ ] | What the hover lists | MVP | a right-click on the Library button: Images, Videos, Other Files, a line, Captures, each ticked while listed, kept across a quit; captures list among the downloads by when they were saved, and their rows open and right-click as their Media tiles; ours: the Library's captures for Arc's Screenshots. Built (`recent_downloads.rs`, `hover_kinds_menu`). To tick: `./check.sh restore`'s `library-button-menu.png`, with its `hover kinds` lines and `recent downloads none` with Other Files off | Arc help centre, ours |
| [x] | Fly-in | MVP | ours: 32 → 16 pt icon from under the URL field to the button on a slight arc, 0.45 s on cubic-bezier(0.55, 0, 0.85, 0.4), fading over the last 30 %; Arc's: measure (Dia constants 0.1 s / 1.3 undecoded) | Dia bin |
| [ ] | Quarantine | MVP | `com.apple.quarantine` on a finished file, naming the browser; to tick, the Mac's check | Chrome |
| [x] | Per profile, persisted | MVP | Chrome's history; private never written once private windows land | Dia P§14 |
| [x] | Quit guard | MVP | asks while downloads run, in Chrome's alert on the Mac and the app's dialog in its words on Linux and Windows; Continue Downloading shows the Library's Downloads; Arc's wording: measure | Dia P§1, Chrome |
