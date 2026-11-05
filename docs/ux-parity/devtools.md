# DevTools

Part of the [UX parity checklist](README.md), whose top says what the columns mean and when to tick a row.

| Done | Item | Phase | Target | Source |
|---|---|---|---|---|
| [x] | Keys and menu | MVP | ⌥⌘I, ⌥⌘J, ⌥⌘C, ⌥⌘U, from the page, DevTools and the UI; View › Developer | Arc 1.170.0 menus |
| [x] | Inspect in the page menu | MVP | Chrome's: docks DevTools at the element | Chrome P§23 |
| [x] | F12 | MVP | opens or closes DevTools, as ⌥⌘I, from the page and the UI; not in the menu | Dia, Chrome P§23 |
| [x] | Docked in the card | MVP | right, full card height, page on the left; each tab its own; closing gives the page the keyboard | Arc 1.170.0 cap |
| [x] | Docked width | MVP | Arc a third of the card; ours Chrome's 555 pt, then the user's, kept per profile | Arc 1.170.0 cap |
| [x] | Undocked | MVP | Chrome's window, from DevTools' Dock side menu; ⌥⌘I from the page brings it to the front, ⌘C copies in it; Dia's 640 × 640 first, "Developer Tools - <url>" | Dia P§23, Chrome, owner's call 2026-10-29 |
| [x] | Restore Earlier Sidebar | MVP | each profile's sidebar backed up in its directory as the Spaces are saved, an hour apart at least, on Arc's schedule (10 today, a day's for 10 days, a week's to a month, a month's to a year); Help › Restore Earlier Sidebar, and the command bar's Restore Earlier Sidebar… where there's no menu bar, list them by date and time, ask first, and keep the sidebar it replaces as the newest backup *interim*. Built, and checked by `cargo test`'s `a_backup_chosen_from_help_is_restored_once_confirmed`, `the_command_bar_lists_the_backups_and_restores_one_once_confirmed` and `an_agents_space_offers_no_restore`, and the schedule by `browser_core_unittests`' `SidebarBackupsTest`. Check on macOS `restore` 41780140344: `sidebar-restored.png` | Arc help, Arc 1.170.0 menus |
| [x] | Task Manager | MVP | Chrome's, from Help › Troubleshooting › Open Task Manager as Arc's; ⌘W closes it | Arc 1.170.0 menus, Chrome |
| [x] | No agent path to a profile's settings | MVP | none to its search engine, archive timing, download folder, offers to save, saved addresses and cards, or Clear Browsing Data. Holds: no agent tool or call reaches them. Built (#683): the list of agents' actions refuses `profile_settings` with why, which `./check.sh agents` asserts in its activity-log line. Check on macOS `agents` 41780140344: its activity log's `profile_settings refused: Agents can't change a profile's settings. They're the user's, in Settings › Profiles.` | decision 8 |
| [x] | Record a Trace | MVP | Help › Troubleshooting and the command bar; a toast with Stop while it records; saved where the save panel picks, sent nowhere; no agent tool | Arc 1.170.0 menus, owner's call 2026-10-29 |
| [x] | No agent DevTools or debugging port | MVP | none. Holds: no agent call opens DevTools or a port; `--remote-debugging-port` and `--remote-debugging-pipe` open one for whoever launches the browser, as the trouble run does. Built: the list of agents' actions refuses `devtools` with why, which `./check.sh agents` asserts in its activity-log line, ticking from it; on the Mac, the run logs `devtools refused: Agents get no DevTools or debugging port.` Not built: a check that a plain launch leaves no DevToolsActivePort | decision 8 |
