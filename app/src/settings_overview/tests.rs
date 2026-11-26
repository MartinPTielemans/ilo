//! Overview's tiles say only what the settings hold, and a loose end's ✕ keeps one key in the
//! profile's folder and nothing else.

use std::cell::Cell;

use gpui::TestAppContext;

use super::*;
use crate::default_browser::{DefaultBrowser, Os, System};

/// A stand-in for the OS's default-browser setting, which counts each time it's asked anything.
#[derive(Default)]
struct Setting {
    default: Cell<bool>,
    read: Cell<usize>,
    asked: Cell<usize>,
}

impl System for Setting {
    fn is_default(&self) -> bool {
        self.read.set(self.read.get() + 1);
        self.default.get()
    }

    fn make_default(&self) {
        self.asked.set(self.asked.get() + 1);
    }
}

/// The notice shows while the product isn't the default and goes once it is; deciding whether
/// it shows asks the OS nothing, and never asks it to change the default.
#[gpui::test]
fn the_notice_asks_the_os_nothing(cx: &mut TestAppContext) {
    let setting = std::rc::Rc::new(Setting::default());
    let system = setting.clone();
    let row = cx.new(|_| DefaultBrowser::new(system, Os::Mac));
    let read = setting.read.get();
    for _ in 0..3 {
        assert!(row.read_with(cx, |row, _| notice_shows(row)));
    }
    assert_eq!(setting.read.get(), read, "drawing the notice asked the OS");
    assert_eq!(
        setting.asked.get(),
        0,
        "only the user's click asks to change it"
    );
    setting.default.set(true);
    row.update(cx, |row, cx| row.check(cx));
    assert!(!row.read_with(cx, |row, _| notice_shows(row)));
    assert_eq!(setting.asked.get(), 0);
}

/// Nothing known, nothing said: a tile with no fact behind a line leaves the line out.
#[test]
fn a_tile_says_nothing_it_doesnt_know() {
    for tile in tiles(&Facts::default()) {
        assert!(
            tile.lines.is_empty(),
            "{} says {:?}",
            tile.title,
            tile.lines
        );
    }
}

/// Each line comes from its fact, and each tile opens its own page.
#[test]
fn each_tile_says_what_its_settings_hold() {
    let facts = Facts {
        edge: true,
        zoom: Some(1.1),
        archive: Some("12 hours".into()),
        profiles: 1,
        spaces: 3,
        level: Some("Work on pages".into()),
        ads_blocked: true,
        nothing_sent: true,
        saved_and_filled: true,
        logins: Some((1, "Personal".into())),
        signed_in: true,
        devices: Some(2),
    };
    let lines: Vec<(Section, Vec<String>)> = (tiles(&facts).into_iter())
        .map(|tile| (tile.section, tile.lines))
        .collect();
    let at = |section| &lines.iter().find(|(each, _)| *each == section).unwrap().1;
    assert_eq!(at(Section::Window).len(), 2);
    assert!(at(Section::Window)[1].contains("110%"));
    assert!(at(Section::TabsAndSpaces)[1].contains("1 profile,"));
    assert!(at(Section::TabsAndSpaces)[1].contains("3 Spaces"));
    assert!(at(Section::Threads)[0].contains("Work on pages"));
    assert!(at(Section::Passwords)[1].contains("1 login in Personal"));
    assert!(at(Section::SyncImport)[1].contains("2 devices"));
    let sections: Vec<Section> = lines.iter().map(|(section, _)| *section).collect();
    assert_eq!(
        sections,
        [
            Section::Window,
            Section::TabsAndSpaces,
            Section::Threads,
            Section::Privacy,
            Section::Passwords,
            Section::SyncImport
        ]
    );
}

/// The Agents tile never says how much of a plan is left: Usage left isn't built.
#[test]
fn the_agents_tile_has_no_usage_line() {
    let facts = Facts {
        level: Some("Look only".into()),
        ..Facts::default()
    };
    let agents = tiles(&facts)
        .into_iter()
        .find(|tile| tile.title == "Agents")
        .unwrap();
    assert_eq!(agents.lines.len(), 1);
    assert!(
        agents
            .lines
            .iter()
            .all(|line| !line.contains('%') && !line.contains("plan"))
    );
}

/// Devices are said only while signed in.
#[test]
fn devices_only_while_signed_in() {
    let facts = Facts {
        devices: Some(2),
        ..Facts::default()
    };
    let sync = tiles(&facts)
        .into_iter()
        .find(|tile| tile.section == Section::SyncImport)
        .unwrap();
    assert!(sync.lines.is_empty());
}

/// The ✕ keeps the loose end's key in the profile's folder, in its one file and nothing else,
/// and a later launch reads it back.
#[test]
fn a_put_away_loose_end_stays_away() {
    let folder = std::env::temp_dir().join(format!(
        "overview-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&folder).unwrap();
    let end = LooseEnd::SignIn(Cli::Codex);
    let mut away = PutAway::load(&folder);
    assert!(!away.has(&end.key()));
    away.put_away(&end.key());
    away.put_away(&end.key());
    let written: Vec<_> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(written, [PUT_AWAY_FILE]);
    let kept = std::fs::read_to_string(folder.join(PUT_AWAY_FILE)).unwrap();
    assert_eq!(kept, format!("{}\n", end.key()));
    assert!(PutAway::load(&folder).has(&end.key()));
    std::fs::remove_dir_all(&folder).ok();
}
