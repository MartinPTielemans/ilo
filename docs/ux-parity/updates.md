# Updates

Part of the [UX parity checklist](README.md), whose top says what the columns mean and when to tick a row.

| Done | Item | Phase | Target | Source |
|---|---|---|---|---|
| [ ] | Check for Updates… | MVP | app menu. Built (#503, #587). `check/updates.sh` checks it on `build.sh updates`, a copy with Sparkle whose feed is a local appcast signed with the run's own key, finding an update with its notes; needs its Mac run | Dia, Ardia P§24 |
| [ ] | Automatic checks | MVP | every 8 h (Ardia). Built: `SUScheduledCheckInterval` 28800. The updates run reads Sparkle's next check, 8 h from the last; needs its Mac run | Ardia |
| [ ] | No install during full screen | MVP | waits. Built: `platform/mac/updates.mm` holds a scheduled check's update while a window is full screen and shows it once none is. The updates run checks both; needs its Mac run | Ardia P§24 |
| [ ] | Release notes after an update | MVP | first launch. Built: `platform/mac/updates.mm` keeps the notes for `host_take_update_notes`. Not checked: only a prod build opens them, for https notes, and an update needs a signed app to install, so the updates run checks only the alert's notes | Ardia P§24 |
| [ ] | Minimum macOS | MVP | 15 (Sequoia), the owner's call; Arc: 13. Built: `build.sh` sets `LSMinimumSystemVersion`, which generate_appcast puts in the feed's entry; README.md and release.yml's notes name it. To tick: `plutil -extract LSMinimumSystemVersion raw build/Browser.app/Contents/Info.plist` reads 15.0, and the next appcast's entry has `sparkle:minimumSystemVersion` | Arc help, #678 |
| [ ] | Get nightly builds | MVP | ours (#871, from #678): Settings › About's switch under the version, on a nightly to start, and no updater goes back a build when it's turned off. Built (`src/updates.rs`), with `cargo test --lib`'s `the_switch_turns_nightly_builds_on_and_off`, `a_nightly_turned_off_keeps_its_build` and `only_a_copy_with_a_nightly_feed_offers_it`. `check/nightly.sh` checks it on a nightly check build with two local feeds, on the Mac and Windows: the feed each check reads, no offer of an older build, the choice after a relaunch, with `nightly-on.png` and `nightly-off.png`. To tick: the first nightly offered to a release turned on | ours |
| [ ] | Feed URL is a default service URL | MVP | forks can change it. Built: `branding/update-feed`, which `build.sh` puts in the prod build; the updates run's build checks a feed of its own the same way. No run reads the product's | principle 2 |
