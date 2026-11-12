# Site notifications

Part of the [UX parity checklist](README.md), whose top says what the columns mean and when to tick a row.

| Done | Item | Phase | Target | Source |
|---|---|---|---|---|
| [x] | A click shows the tab that sent it | MVP | its Space and the tab in its window, which comes in front, whether or not the page's handler focuses it, and once if it does. Built (`chromium-core-notification-click.patch`); `./check.sh notifications` clicks one through Linux's notification server, and on the Mac presses macOS's own banner, after Allow on its question: Check on macOS 42514667390 | Arc help centre |
