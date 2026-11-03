# Link target

Part of the [UX parity checklist](README.md), whose top says what the columns mean and when to tick a row.

| Done | Item | Phase | Target | Source |
|---|---|---|---|---|
| [ ] | The whole address after resting on a link | MVP | after 1.5 s on the same link, a cut address shows whole, up to the card's width inside the plate's margins and 1200; the plate snaps wider in its corner and stays whole for the next links while it shows. Built, and checked by `cargo test`'s `shell::link_target` tests. To tick: a run resting on a link longer than half the card, which the served page of `check/site.sh` doesn't have yet, and its capture | Arc release notes and help |
