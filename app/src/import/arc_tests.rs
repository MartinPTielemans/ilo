//! The Arc reader on a made-up Arc profile in Arc's own format, from cra (`fixture-cra/`, which
//! says where it comes from): its Spaces with their icons, colours and profiles, their pinned tabs
//! and folders, the favorites, and what stays in Arc. Never a real Arc's data.

use std::fs;
use std::path::{Path, PathBuf};
use web_time::SystemTime;

use super::super::found::{Found, FromProfile, Node, Tab};
use super::super::plan::{self, Existing};
use super::super::read::{self, Kind};
use super::super::sources::{Os, Roots, Source};
use crate::theme::Appearance;

fn home() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/import/fixture-cra")
}

fn arc_dir(home: &Path) -> PathBuf {
    home.join("Library/Application Support/Arc")
}

fn sidebar() -> Found {
    let json = fs::read_to_string(arc_dir(&home()).join("StorableSidebar.json")).unwrap();
    super::parse(&json, None).unwrap()
}

fn tab(url: &str, name: &str) -> Node {
    Node::Tab(Tab {
        url: url.into(),
        name: name.into(),
    })
}

/// The Arc that `roots` finds, the only browser there.
fn arc(roots: &Roots) -> Source {
    let mut found = super::super::importable(roots);
    assert_eq!(found.len(), 1, "{found:?}");
    let arc = found.remove(0);
    assert_eq!(arc.browser.name, "Arc");
    arc
}

/// Each Space comes with its name, its icon, whether an `emoji_v2` or an `emoji` by its code, and
/// its profile: Home the default one, Work the one in `Profile 2`. Home's pinned folder comes
/// with its tabs in Arc's order, each under the name the user gave it, else none; Work pins
/// nothing. The default profile's favorites come in their order.
#[test]
fn the_sidebar_reads_as_its_spaces_folders_and_favorites() {
    let found = sidebar();
    let [home, work] = &found.spaces[..] else {
        panic!("{:?}", found.spaces)
    };
    assert_eq!(
        (home.name.as_str(), home.icon.as_str(), &home.profile),
        ("Home", "🏠", &FromProfile::Default)
    );
    assert_eq!(
        (work.name.as_str(), work.icon.as_str(), &work.profile),
        ("Work", "💼", &FromProfile::Custom("Profile 2".into()))
    );
    let reading = Node::Folder {
        name: "Reading".into(),
        children: vec![
            tab("https://example.com/news", ""),
            tab("https://example.com/blog", "My blog"),
        ],
    };
    assert_eq!(home.pinned, [reading]);
    assert!(work.pinned.is_empty(), "{:?}", work.pinned);
    let favorites = [
        Tab {
            url: "https://example.com/mail".into(),
            name: String::new(),
        },
        Tab {
            url: "https://example.com/calendar".into(),
            name: String::new(),
        },
    ];
    assert_eq!(
        found.favorites,
        [(FromProfile::Default, favorites.to_vec())]
    );
    assert!(found.link_rules.is_empty());
}

/// Each Space's colours come with it: Home's dark gradient of two colours, its first the main one,
/// with its intensity and grain, and Work's one colour with a light and a dark look, which
/// follows the system.
#[test]
fn a_spaces_colours_come_with_it() {
    // A colour as the fixture writes it, red, green and blue from 0 to 1.
    let srgb = |red: f64, green: f64, blue: f64| {
        let channel = |value: f64| (value * 255.).round() as u32;
        channel(red) << 16 | channel(green) << 8 | channel(blue)
    };
    let found = sidebar();
    let home = found.spaces[0].theme.as_ref().expect("Home's theme");
    assert_eq!(home.main_color(), srgb(0.2, 0.4, 0.8));
    assert_eq!((home.count, home.appearance), (2, Appearance::Dark));
    assert_eq!((home.intensity, home.grain), (0.5, 0.2));
    assert!(home.dark.is_none());
    let work = found.spaces[1].theme.as_ref().expect("Work's theme");
    assert_eq!(work.main_color(), srgb(0.3, 0.7, 0.4));
    assert_eq!((work.count, work.appearance), (1, Appearance::Automatic));
    assert_eq!(work.grain, 0.);
    assert!(work.dark.is_some(), "Work's dark look");
}

/// Each Space's Today tabs come with Open tabs, in Arc's order, Work's split view as its two tabs;
/// the archived tabs and the window's sidebar stay in Arc.
#[test]
fn today_tabs_come_and_the_archive_stays_in_arc() {
    let arc = arc(&Roots::under(&home(), Os::Mac));
    let found = read::read(&arc, read::kinds(&arc), SystemTime::now()).unwrap();
    let today = |at: usize| -> Vec<&str> {
        (found.spaces[at].today.iter())
            .map(|tab| tab.url.as_str())
            .collect()
    };
    let home = ["1", "2", "3"].map(|n| format!("https://example.com/today/{n}"));
    assert_eq!(
        today(0),
        home.iter().map(String::as_str).collect::<Vec<_>>()
    );
    assert_eq!(
        today(1),
        ["https://example.com/docs", "https://example.com/tracker"]
    );
    assert_eq!(read::tally(&found, &arc).open_tabs, 5);
    let all = format!("{found:?}");
    assert!(!all.contains("example.com/old/"), "an archived tab came in");
}

/// Open tabs switched off leaves the Today tabs in Arc; Spaces switched off with Open tabs on
/// brings each Space that has Today tabs for them, without its pinned tabs or link rules.
#[test]
fn open_tabs_and_spaces_are_switched_apart() {
    let arc = arc(&Roots::under(&home(), Os::Mac));
    let now = SystemTime::now();
    let all = read::read(&arc, read::kinds(&arc), now).unwrap();
    for picked in [
        &[Kind::Spaces, Kind::Favorites][..],
        &[Kind::OpenTabs][..],
        &[Kind::Spaces, Kind::OpenTabs][..],
    ] {
        let read = read::read(&arc, picked, now).unwrap();
        assert_eq!(
            read::keep(&all, &arc, picked),
            Some(read.clone()),
            "{picked:?}"
        );
        let today = picked.contains(&Kind::OpenTabs);
        let spaces = picked.contains(&Kind::Spaces);
        assert_eq!(read.spaces.len(), 2, "{picked:?}");
        assert!(
            read.spaces
                .iter()
                .all(|space| space.today.is_empty() != today)
        );
        assert_eq!(read.spaces[0].pinned.is_empty(), !spaces, "{picked:?}");
    }
}

/// Arc is found where the Mac keeps it, with each profile by the name Arc's Local State gives
/// it, and where Windows' Store keeps it, which reads the same; the Space in `Profile 2` goes to
/// a new profile named Client, as Arc names it.
#[test]
fn arc_is_found_on_the_mac_and_in_windows_store_folder() {
    let mac = arc(&Roots::under(&home(), Os::Mac));
    let mut names: Vec<(String, String)> = (mac.profiles.iter())
        .map(|profile| (profile.dir.clone(), profile.name.clone()))
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            ("Default".to_string(), "Personal".to_string()),
            ("Profile 2".to_string(), "Client".to_string())
        ]
    );
    let now = SystemTime::now();
    let on_mac = read::read(&mac, read::kinds(&mac), now).unwrap();

    let windows = std::env::temp_dir().join(format!("ilo-arc-cra-{}", std::process::id()));
    _ = fs::remove_dir_all(&windows);
    let store = windows.join("AppData/Local/Packages/TheBrowserCompany.Arc_ttt1ap7aakyb4");
    copy(&arc_dir(&home()), &store.join("LocalCache/Local/Arc"));
    let on_windows = arc(&Roots::under(&windows, Os::Windows));
    let read_there = read::read(&on_windows, read::kinds(&on_windows), now).unwrap();
    assert_eq!(read_there, on_mac);
    fs::remove_dir_all(windows).unwrap();

    let existing = Existing {
        profiles: vec![(1, "Default".into())],
        ..Existing::default()
    };
    let plan = plan::plan(&on_mac, &names, &existing);
    assert_eq!(plan.new_profiles, ["Client"]);
    assert_eq!(plan.counts().spaces, 2);
    assert_eq!(plan.counts().pinned, 2);
    assert_eq!(plan.counts().favorites, 2);
}

/// Extensions from outside the Web Store stay out, as cra's are, whose manifests name no Web
/// Store to update from, though each profile has them, pinned or turned off: there's no page to
/// add them from.
#[test]
fn extensions_from_outside_the_web_store_stay_out() {
    let arc = arc(&Roots::under(&home(), Os::Mac));
    let profiles = arc_dir(&home()).join("User Data");
    for (profile, ids) in [
        (
            "Default",
            [
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "dddddddddddddddddddddddddddddddd",
            ],
        ),
        (
            "Profile 2",
            [
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "cccccccccccccccccccccccccccccccc",
            ],
        ),
    ] {
        for id in ids {
            let manifest = profiles
                .join(profile)
                .join("Extensions")
                .join(id)
                .join("1.0_0/manifest.json");
            assert!(manifest.is_file(), "{manifest:?}");
        }
    }
    let found = read::read(&arc, &[Kind::Extensions], SystemTime::now()).unwrap();
    assert!(found.extensions.is_empty(), "{:?}", found.extensions);
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let path = entry.path();
        let into = to.join(entry.file_name());
        match path.is_dir() {
            true => copy(&path, &into),
            false => {
                fs::copy(&path, &into).unwrap();
            }
        }
    }
}
