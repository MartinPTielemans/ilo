# Link target

Part of the [UX parity checklist](README.md), whose top says what the columns mean and when to tick a row.

| Done | Item | Phase | Target | Source |
|---|---|---|---|---|
| [x] | The whole address after resting on a link | MVP | after 1.5 s on the same link, a cut address shows whole, up to the card's width inside the plate's margins and 1200; the plate snaps wider in its corner and stays whole for the next links while it shows. Built, and checked by `cargo test`'s `shell::link_target` tests. Check on macOS `clear` 41784676486 and Check on Linux 41784670759: resting on a 130-character link, the plate's cut address, then its `link target whole` line with the address whole, and `link-target-whole.png` | Arc release notes and help |
