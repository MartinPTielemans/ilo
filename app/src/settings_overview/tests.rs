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

/// Every fact known, so each tile says both its lines.
fn known() -> Facts {
    Facts {
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
        signed_out: false,
        devices: Some(2),
    }
}

/// Each line comes from its fact, and each tile opens its own page.
#[test]
fn each_tile_says_what_its_settings_hold() {
    let facts = known();
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

    // ⌘Z brings the last one back, and Show Hidden all of them, in the file too (#4409).
    let other = LooseEnd::SignIn(Cli::Claude);
    away.put_away(&other.key());
    assert_eq!(away.bring_back_last(), Some(other.key()));
    assert!(!away.has(&other.key()) && away.has(&end.key()));
    assert!(away.any());
    away.bring_back_all();
    assert!(!away.any());
    assert!(
        !PutAway::load(&folder).has(&end.key()),
        "the file still kept it"
    );
    assert_eq!(away.bring_back_last(), None);
    std::fs::remove_dir_all(&folder).ok();
}

/// Overview's tiles as Settings lays them out, in two rows of three.
struct Grid(Facts);

impl Render for Grid {
    fn render(&mut self, window: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        let look = Look::settings(true);
        let a11y = window.is_a11y_active();
        let cards = (tiles(&self.0).iter())
            .map(|tile| tile_card(&look, tile, a11y, Rc::new(|_, _| {})))
            .collect();
        div().size_full().child(tiles_grid(cards))
    }
}

/// The width `words` take in a tile's line.
fn line_width(cx: &mut gpui::VisualTestContext, words: &str) -> f32 {
    cx.update(|window, _| {
        let run = window.text_style().to_run(words.len());
        let size = px(form::TILE_BODY);
        let line = (window.text_system()).shape_line(words.to_string().into(), size, &[run], None);
        f32::from(line.width)
    })
}

/// Each tile as an assistive app is told of it: its name, what it says, and where it was drawn,
/// its width and height. A release build keeps these, where it keeps no debug selector.
fn told(cx: &mut gpui::VisualTestContext) -> Vec<(String, Option<String>, f32, f32)> {
    use gpui::accesskit::Role;
    let scale = cx.update(|window, _| f64::from(window.scale_factor()));
    let tree = cx.a11y_tree().expect("GPUI built a tree");
    (tree.nodes.iter())
        .filter(|(_, node)| node.role() == Role::Button)
        .map(|(_, node)| {
            let at = node.bounds().expect("a tile with no bounds");
            (
                node.label().unwrap_or_default().to_string(),
                node.description().map(str::to_string),
                ((at.x1 - at.x0) / scale) as f32,
                ((at.y1 - at.y0) / scale) as f32,
            )
        })
        .collect()
}

/// On tiles narrower than drawn, no tile's line is cut: one wider than its tile's text takes a
/// second line, and its row grows for them only past the drawn height. An assistive app reads
/// each tile's lines whole, after its page's name.
#[gpui::test]
fn a_tiles_line_is_never_cut(cx: &mut TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| Grid(known()));
    // Three tiles across 480, each about 150 wide: narrower than the drawing's.
    cx.simulate_resize(gpui::size(px(480.), px(600.)));
    cx.simulate_a11y_activation();
    cx.run_until_parked();
    let told = told(cx);
    let tiles = tiles(&known());
    let mut grew = false;
    for row in tiles.chunks(3) {
        // A row's tiles are as tall as its tallest needs, and the drawn height at least.
        let mut needed = form::TILE;
        for tile in row {
            let (_, said, width, _) = (told.iter())
                .find(|(name, ..)| name == tile.title)
                .unwrap_or_else(|| panic!("no {} tile", tile.title));
            assert_eq!(said.as_deref(), Some(tile.lines.join(", ").as_str()));
            let room = width - 2. * form::TILE_X;
            let lines: f32 = (tile.lines.iter())
                .map(|words| if line_width(cx, words) > room { 2. } else { 1. })
                .sum();
            let head = 2. * form::TILE_Y + form::TILE_TITLE_LINE + form::TILE_GAP;
            needed = needed.max(head + lines * form::TILE_BODY_LINE);
        }
        for tile in row {
            let (.., height) = told.iter().find(|(name, ..)| name == tile.title).unwrap();
            assert!(
                *height + 0.5 >= needed,
                "{} is {height} tall, its lines need {needed}: one was cut",
                tile.title
            );
            if needed == form::TILE {
                assert!((height - form::TILE).abs() < 0.5, "{} grew", tile.title);
            }
        }
        grew |= needed > form::TILE;
    }
    assert!(
        grew,
        "no row's lines needed more than the drawn height at this width"
    );
}

/// Signed out, the Sync tile says so in its one line, and signed in it says that instead, with
/// its devices; an assistive app reads the tile's line either way.
#[gpui::test]
fn the_sync_tile_says_whether_an_account_is_signed_in(cx: &mut TestAppContext) {
    let out = Facts {
        signed_out: true,
        ..Facts::default()
    };
    let sync = |facts: &Facts| {
        let tiles = tiles(facts);
        let tile = tiles
            .iter()
            .find(|tile| tile.section == Section::SyncImport);
        tile.unwrap().lines.clone()
    };
    assert_eq!(sync(&out), [NOT_SIGNED_IN]);
    assert_eq!(sync(&known())[0], "Signed in");
    for facts in [out, known()] {
        let lines = sync(&facts);
        let (_, cx) = cx.add_window_view(|_, _| Grid(facts));
        cx.simulate_a11y_activation();
        cx.run_until_parked();
        let said = (told(cx).into_iter()).find(|(name, ..)| name == "Sync");
        assert_eq!(said.and_then(|(_, said, ..)| said), Some(lines.join(", ")));
    }
}
