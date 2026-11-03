# Import

Part of the [UX parity checklist](README.md), whose top says what the columns mean and when to tick a row.

| Done | Item | Phase | Target | Source |
|---|---|---|---|---|
| [x] | Arc: Spaces → Spaces, profiles → profiles | MVP | yes; Settings › Import | decision 5 |
| [x] | Arc: favorites, pinned tabs, custom names | MVP | favorites → favorites; pinned → pinned rows | Dia 1.52 P§20 |
| [ ] | Arc: themes | MVP | `windowTheme` colours, intensity, noise, texture, appearance, light and dark. Built: colour, count, intensity, noise, the texture as ours, the third colour exactly, Arc's placing of the others, each at its own hue and lightness in the field at the first's saturation, the appearance (`windowAppearance`, else the looks' `translucencyStyle`) and the separate light and dark looks, which `cargo test` reads from the fixture; `./check.sh import`'s `import-personal.png` shows the Personal Space's theme, which the run doesn't assert. Not built: translucency, which needs ours | Arc 1.167 P§20 |
| [x] | Arc: folders | MVP | ours: folders, nested; split views as their tabs *interim*; measure | ours |
| [ ] | Chrome: bookmarks, history, open tabs, passwords, addresses, cards | MVP | yes, passwords from the user's exported CSV; cards never, as they're encrypted (#453); built: bookmarks, open tabs, history and addresses, for Chrome's whole family, and passwords from any browser's exported CSV; `./check.sh import`'s `import-chrome.png`, `import-passwords.png` and `import-passwords-conflict.png` | P§20 |
| [x] | Chrome: cookies | MVP | not imported: no other browser's cookies or passwords are decrypted, and the user signs in again | #453 |
| [x] | Firefox, Zen, LibreWolf: bookmarks, history | MVP | ours: as Chrome's family's, from `places.sqlite` | ours |
| [x] | Safari: bookmarks, Reading List, history | MVP | ours: as Chrome's family's, the Reading List as a folder | ours |
| [x] | A bookmarks file, from any browser | MVP | ours: the Netscape HTML every browser exports, as a browser's own bookmarks come; `cargo test` and `./check.sh import`'s `import-bookmarks.png` | #453 |
| [ ] | Chrome: extensions | MVP | listed with store links, not installed. Built for Chrome's family and Arc, from each profile's `Extensions` folder, and checked by `cargo test`. To tick: an extension in the import run's made-up Chrome, and a capture of the list | P§20 |
| [ ] | Extensions keep their pins | MVP | one pinned in the other browser is pinned once it's added from the list (owner, 2026-10-29). Built, and checked by `cargo test`; to tick: a run that adds one from the store | Arc help |
| [ ] | Another ad blocker | MVP | left off the extension list, as uBlock Origin is built in, said once with Add Anyway (owner, 2026-10-29). Built, and checked by `cargo test`; to tick: a capture of the line | Arc help |
| [ ] | Choose bookmark folders | MVP | Choose folders, a checkbox a folder, all ticked (owner, 2026-10-29). Built, and checked by `cargo test`; to tick: a capture of the box and an import run that unticks one | Arc help |
| [ ] | Suggested favorites | MVP | the first launch's Your favorites, after the import: the sites its history visited most, ticked, worked out on the machine and sent nowhere (owner, 2026-10-29). Built, and checked by `cargo test`; to tick: the onboarding run with a made-up Chrome's history, and a capture of the step | Arc help |
| [x] | Full Disk Access | MVP | explain, link to System Settings, re-check on return; Safari's | P§20 |
| [x] | Rerun | MVP | adds nothing | P§20 |
| [x] | Where Chrome bookmarks land | MVP | ours: a Space named after the browser, the bar's bookmarks pinned, or in a folder past 12; measure Arc's | ours |
