# A made-up Arc profile, from cra

A home folder holding an Arc profile in Arc 1.170's own file format, laid out as the Mac keeps it
(`Library/Application Support/Arc`), for the tests of ilo's Arc reader (`../arc_tests.rs`). It
holds no real account or browsing data: every address is on example.com, the profile names and
extensions are made up, and nothing in it was ever any person's Arc.

**Source.** cra, by Victor Kildahl, under the MIT License, which `LICENSE` here carries.
- The files are `Tests/Fixtures/Arc` of https://github.com/VictorKildahl/cra at commit
  `ac48c03f823eebc77e357fae8b75e1d97651da2a`, the last to change that folder, as of
  `51edf6df08ebbe6b35beca2b44823a60dead400c`.
- The Space colours follow the "Sidebar data model" section of its
  `docs/arc-reverse-engineering.md` at `51edf6df08ebbe6b35beca2b44823a60dead400c`.

**What it holds.**
- Two Spaces: Home (🏠, `emoji_v2`) on the default profile, and Work (💼, as `emoji` 128188) on
  `Profile 2`, which `User Data/Local State` names Client.
- Home's pinned tabs in a folder, Reading: News, and Blog under the name the user gave it, My blog.
- Each Space's Today tabs: three in Home, and a split view of two in Work.
- Two favorites, Mail and Calendar, in the default profile's top apps.
- The extensions in each profile's `Preferences` and `Extensions`, one of them turned off.
- Two archived tabs (`StorableArchiveItems.json`) and the sidebar's width (`StorableWindows.json`).

**What ilo changed.** Each Space gets a `customInfo.windowTheme`, which cra's fixture has none of:
- Home a dark two-colour `blendedGradient`;
- Work a `blendedSingleColor` with a light and a dark look, following the system.

The file was rewritten with one space of indent, as cra's is, and its README became this one.
