# Site notifications

Part of the [UX parity checklist](README.md), whose top says what the columns mean and when to tick a row.

| Done | Item | Phase | Target | Source |
|---|---|---|---|---|
| [x] | A click shows the tab that sent it | MVP | its Space and the tab in its window, which comes in front, whether or not the page's handler focuses it, and once if it does. Built (`chromium-core-notification-click.patch`); `./check.sh notifications` clicks one through Linux's notification server, and on the Mac presses macOS's own banner, after Allow on its question: Check on macOS 42514667390 | Arc help centre |
| [ ] | Favorite badges | MVP | a favorite's tile counts the notifications its page posted while the tab wasn't shown, drawn as a pill in the tile's top left, at most 99; showing the tab, or turning Settings › Tabs & Sidebar › Show unseen notifications on favorites off, clears the count; a worker's notification counts for nothing and an agent's tab never counts (ux-spec.md › Site notifications › Badges). Built: the core's per-profile observer counting by id (`core/notifications.cc`), the badge in `app/src/shell/favorites.rs`. To tick: a capture of a tile with its badge, after a page's notification, in `./check.sh favorites` | ours |
