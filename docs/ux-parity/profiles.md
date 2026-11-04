# Profiles

Part of the [UX parity checklist](README.md), whose top says what the columns mean and when to tick a row.

| Done | Item | Phase | Target | Source |
|---|---|---|---|---|
| [x] | Profile holds cookies, history, logins, autofill, extensions | MVP | yes | decision 5 |
| [x] | Create, rename, delete, pick for a Space | MVP | ours: Settings › Profiles, the New Space form's Profile row (Arc's) and the Space menu's Profile items *interim*; Arc's: measure | Arc 1.170.0 cap |
| [ ] | Where a Space shows its profile | MVP | built: the Space's menu checks it and the form's Profile row names it, nothing in the sidebar *interim*. To tick: `./check.sh profiles`' `profile-popup.png`, `profile-form.png` and `profile-menu.png`. Arc's: measure | — |
| [ ] | New Note / New Note Split in the profile's notes service | MVP | Settings › Features › New Note per profile: Notion, Google Docs, Word, Confluence, at notion.new, docs.new, word.new, confluence.new, Google Docs until one is picked *interim*, the one setting for new documents (owner, 2026-10-29, #698); New Note in the command bar, the footer's + and ⌃⇧N, New Note Split in the command bar beside the page. Built; to tick: the running app with the Profiles tab's New Documents rows and a note opened in each way, on an engine with the core's notes service | Arc help centre, Arc 1.170.0 Settings › Shortcuts |
| [x] | Incognito ⇧⌘N | MVP | per profile, dark, one session per profile, nothing written. Built (private check): a private window of the key window's profile with one unsaved Space of its own, Today tabs only, black 0.35 in the gradient's place *interim*; Arc's and Dia's private window: measure | Chrome, Dia |
