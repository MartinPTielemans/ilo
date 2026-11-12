//! The import on made-up profiles: Arc's sidebar in `fixture/`, laid out as a Mac's home folder,
//! and each system's browsers, with their bookmarks and history, in folders the tests make. Never a
//! real browser's data.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use gpui::{App, AppContext, Entity, TestAppContext};

use super::arc;
use super::chromium;
use super::extensions::{self, Extension};
use super::found::{
    Address, Destination, Field, Found, FromProfile, LinkRule, Node, Page, Split, Tab,
};
use super::plan::{self, Counts, Existing, ExistingSpace, Row};
use super::read::{self, Kind, ReadError};
use super::session;
use super::sources::{self, Os, Roots};
use super::suggested;
use super::{Exported, Host, ImportEvent, ImportView, Look, Passwords, STALL};
use super::{firefox, safari};
use crate::link_rules::{Condition, FRONT, LITTLE, Rule};
use crate::theme::{Appearance, Texture, Theme};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/import/fixture")
}

fn sidebar() -> Found {
    let arc = fixture().join("Library/Application Support/Arc");
    let routing = fs::read_to_string(arc.join("StorableLinkRouting.json")).unwrap();
    arc::parse(
        &fs::read_to_string(arc.join("StorableSidebar.json")).unwrap(),
        Some(&routing),
    )
    .unwrap()
}

/// The fixture's sidebar, with its Spaces or its favorites left out.
fn picked(spaces: bool, favorites: bool) -> Found {
    let mut found = sidebar();
    if !spaces {
        found.spaces.clear();
        found.link_rules.clear();
        found.link_otherwise = None;
    }
    if !favorites {
        found.favorites.clear();
    }
    found
}

fn tab(url: &str) -> Node {
    Node::Tab(tab_of(url))
}

fn tab_of(url: &str) -> Tab {
    Tab {
        url: url.into(),
        name: String::new(),
    }
}

/// Spaces, their pinned tabs and folders, their profiles and the favorites come in; Arc's own
/// pages, the welcome card, Today tabs and ids that name nothing don't. A split view comes in as
/// one, and a folder named inside itself comes in once.
#[test]
fn arcs_sidebar_reads_as_its_spaces_and_favorites() {
    let sidebar = sidebar();
    let [personal, work] = &sidebar.spaces[..] else {
        panic!("{:?}", sidebar.spaces)
    };
    assert_eq!(
        (personal.name.as_str(), &personal.profile),
        ("Personal", &FromProfile::Default)
    );
    assert_eq!(personal.icon, "🏠");
    let named = Node::Tab(Tab {
        url: "https://news.ycombinator.com/".into(),
        name: "HN".into(),
    });
    let deep = Node::Folder {
        name: "Deep".into(),
        children: vec![tab("https://example.com/d")],
    };
    let reading = Node::Folder {
        name: "Reading".into(),
        children: vec![
            tab("https://example.com/a"),
            Node::Split(Split {
                tabs: vec![
                    tab_of("https://example.com/b"),
                    tab_of("https://example.com/c"),
                ],
                name: String::new(),
                icon: String::new(),
                stacked: false,
            }),
            deep,
        ],
    };
    assert_eq!(personal.pinned, vec![named, reading]);
    assert_eq!(
        (work.name.as_str(), &work.profile),
        ("Work", &FromProfile::Custom("Profile 1".into()))
    );
    assert_eq!(work.icon, "glyph:bolt");
    let code = Node::Tab(Tab {
        url: "https://github.com/".into(),
        name: "Code".into(),
    });
    assert_eq!(work.pinned, vec![code]);
    let urls = |tabs: &[Tab]| tabs.iter().map(|tab| tab.url.clone()).collect::<Vec<_>>();
    let [(default, mine), (custom, theirs)] = &sidebar.favorites[..] else {
        panic!()
    };
    assert_eq!(default, &FromProfile::Default);
    assert_eq!(
        urls(mine),
        ["https://mail.example.com/", "https://calendar.example.com/"]
    );
    assert_eq!(custom, &FromProfile::Custom("Profile 1".into()));
    assert_eq!(urls(theirs), ["https://linear.app/"]);
}

/// A gradient's first colour is the main one, its colours as many as ours go, each other one at
/// its own place in the field and its third overlaid one exactly, with Arc's intensity, grain,
/// texture and appearance; a colour outside sRGB is brought in. A Space with a light and a dark
/// look that follows the system keeps both.
#[test]
fn arcs_theme_becomes_the_spaces_theme() {
    let sidebar = sidebar();
    let personal = sidebar.spaces[0].theme.unwrap();
    assert_eq!(personal.count, 3);
    assert_eq!(
        (personal.colors()[2], personal.texture),
        (0x33cc99, Texture::Dust)
    );
    assert_eq!(
        personal.main_color(),
        Theme::of_color(0x3366cc).main_color()
    );
    // Arc's second, (1.1, 0.5, -0.05), is orange's hue and lightness, not the main one's opposite.
    let ((x, y), orange) = (personal.handle(1), Theme::of_color(0xff8000));
    assert!(
        (x - orange.x).abs() < 1e-3 && (y - orange.y).abs() < 1e-3,
        "at {x}, {y}"
    );
    // The third's handle is where its teal is, the window taking the teal itself.
    let ((x, y), teal) = (personal.handle(2), Theme::of_color(0x33cc99));
    assert!(
        (x - teal.x).abs() < 1e-3 && (y - teal.y).abs() < 1e-3,
        "at {x}, {y}"
    );
    assert_eq!((personal.intensity, personal.grain), (0.6, 0.3));
    assert_eq!(
        (personal.appearance, personal.dark),
        (Appearance::Dark, None)
    );
    let work = sidebar.spaces[1].theme.unwrap();
    assert_eq!(work.count, 1);
    assert_eq!(work.main_color(), Theme::of_color(0xe699b3).main_color());
    assert_eq!(work.appearance, Appearance::Automatic);
    let dark = work.shown(true);
    assert_eq!(dark.main_color(), Theme::of_color(0x1a1a33).main_color());
    assert_eq!(
        (dark.count, dark.grain),
        (1, 0.),
        "a basic colour has no grain"
    );
    assert_eq!(
        (work.texture, work.third),
        (Texture::Noise, None),
        "Arc's base as our noise"
    );
    assert_eq!(work.shown(false).main_color(), work.main_color());
}

/// Arc's appearance: `windowAppearance` forces light or dark, a dark theme taking its dark look;
/// without it one look follows its `translucencyStyle`, else the system. A gradient's second
/// overlay colour is its third.
#[test]
fn arcs_appearance_comes_from_its_window_appearance_else_its_look() {
    let pink = r#"{"blendedSingleColor": {"_0": {"color": {"red": 0.9, "green": 0.6, "blue": 0.7}, "translucencyStyle": "light"}}}"#;
    let navy = r#"{"basicColor": {"_0": {"color": {"red": 0.1, "green": 0.1, "blue": 0.2}}}}"#;
    let three = r#"{"blendedGradient": {"_0": {"baseColors": [{"red": 1, "green": 0, "blue": 0}, {"red": 0, "green": 0, "blue": 1}], "overlayColors": [{"red": 0, "green": 0, "blue": 0, "alpha": 0}, {"red": 0, "green": 1, "blue": 0}], "translucencyStyle": "auto"}}}"#;
    let look = |color: &str| format!(r#"{{"style": {{"color": {{"_0": {color}}}}}}}"#);
    let theme_of = |window_theme: String| {
        let json = format!(
            r#"{{"sidebar": {{"containers": [{{"global": {{}}}}, {{"spaces": ["S", {{"id": "S", "title": "S", "customInfo": {{"windowTheme": {window_theme}}}}}], "items": []}}]}}}}"#
        );
        arc::parse(&json, None).unwrap().spaces[0].theme.unwrap()
    };
    let one = |color: &str, forced: &str| {
        theme_of(format!(
            r#"{{{forced} "background": {{"single": {{"_0": {}}}}}}}"#,
            look(color)
        ))
    };
    assert_eq!(
        one(pink, "").appearance,
        Appearance::Light,
        "its translucency style"
    );
    assert_eq!(one(three, "").appearance, Appearance::Automatic);
    assert_eq!(one(three, "").count, 3);
    let forced = r#""windowAppearance": {"single": {"_0": "dark"}},"#;
    assert_eq!(one(pink, forced).appearance, Appearance::Dark);
    let both = format!(
        r#""background": {{"appearanceBased": {{"light": {}, "dark": {}}}}}"#,
        look(pink),
        look(navy)
    );
    let dark = theme_of(format!(r#"{{{forced} {both}}}"#));
    assert_eq!((dark.appearance, dark.dark), (Appearance::Dark, None));
    assert_eq!(
        dark.main_color(),
        Theme::of_color(0x1a1a33).main_color(),
        "a dark theme takes its dark look"
    );
    let auto = theme_of(format!("{{{both}}}"));
    assert_eq!(auto.appearance, Appearance::Automatic);
    assert!(auto.dark.is_some());
}

/// The fixture's Reading folder, with its split view's item edited by `edit` and `items` added.
fn reading_with(edit: &str, items: &str) -> Vec<Node> {
    let arc = fixture().join("Library/Application Support/Arc");
    let json = fs::read_to_string(arc.join("StorableSidebar.json")).unwrap();
    let split = r#"{ "id": "V-SPLIT", "parentID": "F-READING", "title": null, "childrenIds": ["T-B", "T-C"], "data": { "splitView": { "layoutOrientation": "horizontal", "itemWidthFactors": [0.5, 0.5] } } },"#;
    assert!(json.contains(split));
    let json = json.replace(split, &format!("{edit},\n{items}"));
    let found = arc::parse(&json, None).unwrap();
    let Node::Folder { children, .. } = &found.spaces[0].pinned[1] else {
        panic!("{:?}", found.spaces[0].pinned)
    };
    children.clone()
}

/// A split view keeps the name and orientation the user gave it in Arc, and its icon if it's an
/// emoji, as a split's icon is here: Arc's glyphs have no place on a split.
#[test]
fn an_arc_split_view_keeps_its_name_icon_and_orientation() {
    let split = |name: &str, icon: &str, stacked| {
        Node::Split(Split {
            tabs: vec![
                tab_of("https://example.com/b"),
                tab_of("https://example.com/c"),
            ],
            name: name.into(),
            icon: icon.into(),
            stacked,
        })
    };
    let named = r#"{ "id": "V-SPLIT", "parentID": "F-READING", "title": " Docs and notes ", "customInfo": { "iconType": { "emoji_v2": "📚" } }, "childrenIds": ["T-B", "T-C"], "data": { "splitView": { "layoutOrientation": "vertical" } } }"#;
    assert_eq!(
        reading_with(named, "")[1],
        split("Docs and notes", "📚", true)
    );
    let glyph = r#"{ "id": "V-SPLIT", "parentID": "F-READING", "title": "Docs", "customInfo": { "iconType": { "icon": "bolt" } }, "childrenIds": ["T-B", "T-C"], "data": { "splitView": { "layoutOrientation": "horizontal" } } }"#;
    assert_eq!(reading_with(glyph, "")[1], split("Docs", "", false));
}

/// A split here holds three pages, so an Arc split's fourth tab comes in after it, and a split
/// with one page that opens here comes in as that tab.
#[test]
fn an_arc_split_view_holds_what_a_split_here_does() {
    let tab_item = |id: &str, url: &str| {
        format!(
            r#""{id}", {{ "id": "{id}", "parentID": "V-SPLIT", "title": null, "childrenIds": [], "data": {{ "tab": {{ "savedURL": "{url}" }} }} }},"#
        )
    };
    let items = [
        tab_item("T-E", "https://example.com/e"),
        tab_item("T-F", "https://example.com/f"),
        tab_item("T-G", "arc://settings"),
    ]
    .concat();
    let four = r#"{ "id": "V-SPLIT", "parentID": "F-READING", "title": null, "childrenIds": ["T-B", "T-C", "T-E", "T-F"], "data": { "splitView": {} } }"#;
    let reading = reading_with(four, &items);
    assert_eq!(
        reading[1..4],
        [
            Node::Split(Split {
                tabs: ["b", "c", "e"]
                    .map(|page| tab_of(&format!("https://example.com/{page}")))
                    .into(),
                name: String::new(),
                icon: String::new(),
                stacked: false,
            }),
            tab("https://example.com/f"),
            Node::Folder {
                name: "Deep".into(),
                children: vec![tab("https://example.com/d")],
            },
        ]
    );
    let one = r#"{ "id": "V-SPLIT", "parentID": "F-READING", "title": "Settings", "childrenIds": ["T-B", "T-G"], "data": { "splitView": {} } }"#;
    assert_eq!(
        reading_with(one, &items)[..2],
        [tab("https://example.com/a"), tab("https://example.com/b")]
    );
}

#[test]
fn a_file_that_isnt_arcs_sidebar_says_so() {
    assert!(arc::parse("not json", None).is_err());
    assert!(arc::parse(r#"{"sidebar": {"containers": [{"global": {}}]}}"#, None).is_err());
}

/// A folder the test makes, empty, for one test's browsers.
pub(crate) fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ilo-import-{}-{name}", std::process::id()));
    _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub(crate) fn write(path: PathBuf, text: &str) -> PathBuf {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    path
}

/// Marks a file used `ago` seconds ago.
fn used(path: &Path, ago: u64) {
    let file = fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(ago))
        .unwrap();
}

fn found(roots: &Roots) -> Vec<(&'static str, Vec<String>)> {
    let sources = sources::find(roots);
    let names = |source: &sources::Source| source.profiles.iter().map(|p| p.name.clone()).collect();
    sources
        .iter()
        .map(|source| (source.browser.name, names(source)))
        .collect()
}

/// On the Mac, Arc's profiles by the names its `Local State` gives them, with its sidebar.
#[test]
fn the_mac_finds_arc_with_its_profiles() {
    let roots = Roots::under(&fixture(), Os::Mac);
    assert_eq!(
        found(&roots),
        [("Arc", vec!["Personal".to_string(), "Work".to_string()])]
    );
    let arc = &sources::find(&roots)[0];
    assert!(arc.arc_sidebar().is_some());
}

/// On Windows, Arc in the folder the Store gives it, Chrome in Local AppData and Firefox's
/// profiles in Roaming, the one used last first; a folder with no profile in it is no browser.
#[test]
fn windows_finds_each_browser_where_it_keeps_its_profiles() {
    let home = home("windows");
    let arc = home
        .join("AppData/Local/Packages/TheBrowserCompany.Arc_ttt1ap7aakyb4/LocalCache/Local/Arc");
    write(arc.join("StorableSidebar.json"), "{}");
    let arc_used = write(arc.join("User Data/Default/History"), "");
    write(arc.join("User Data/Default/Preferences"), "{}");
    let chrome = home.join("AppData/Local/Google/Chrome/User Data");
    write(
        chrome.join("Local State"),
        r#"{"profile":{"info_cache":{"Profile 2":{"name":"Two"},"Profile 10":{"name":"Ten"},"Default":{"name":"Me"}}}}"#,
    );
    for profile in ["Default", "Profile 2", "Profile 10"] {
        used(&write(chrome.join(profile).join("Preferences"), "{}"), 100);
    }
    let firefox = home.join("AppData/Roaming/Mozilla/Firefox");
    write(
        firefox.join("profiles.ini"),
        "[Install1]\nDefault=Profiles/a.default\n\n[Profile0]\nName=default-release\nIsRelative=1\nPath=Profiles/a.default\n\n[Profile1]\nName=gone\nIsRelative=1\nPath=Profiles/b.gone\n",
    );
    used(
        &write(firefox.join("Profiles/a.default/places.sqlite"), ""),
        50,
    );
    fs::create_dir_all(home.join("AppData/Local/Microsoft/Edge/User Data")).unwrap();
    used(&arc_used, 10);
    let roots = Roots::under(&home, Os::Windows);
    let strings = |names: &[&str]| {
        names
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        found(&roots),
        [
            ("Arc", strings(&["Default"])),
            ("Firefox", strings(&["default-release"])),
            ("Chrome", strings(&["Me", "Two", "Ten"])),
        ]
    );
    fs::remove_dir_all(home).unwrap();
}

/// On Linux, Chrome in `~/.config` and Firefox in `~/.mozilla`; Opera keeps its one profile in
/// its folder itself. Arc and Safari aren't looked for.
#[test]
fn linux_finds_each_browser_where_it_keeps_its_profiles() {
    let home = home("linux");
    used(
        &write(home.join(".config/google-chrome/Default/Preferences"), "{}"),
        30,
    );
    used(&write(home.join(".config/opera/Preferences"), "{}"), 20);
    write(
        home.join(".mozilla/firefox/profiles.ini"),
        "[Profile0]\nName=default\nPath=x.default\n",
    );
    used(
        &write(home.join(".mozilla/firefox/x.default/places.sqlite"), ""),
        10,
    );
    write(
        home.join("Library/Application Support/Arc/User Data/Default/Preferences"),
        "{}",
    );
    let roots = Roots::under(&home, Os::Linux);
    let names: Vec<_> = found(&roots).into_iter().map(|(name, _)| name).collect();
    assert_eq!(names, ["Firefox", "Opera", "Chrome"]);
    fs::remove_dir_all(home).unwrap();
}

/// The names the browsers found under `roots` are listed by, the one used last first.
fn listed(roots: &Roots) -> Vec<String> {
    sources::find(roots)
        .iter()
        .map(sources::Source::name)
        .collect()
}

/// A Chromium user data folder with its default profile, used `ago` seconds ago.
fn chromium_dir(dir: PathBuf, ago: u64) {
    used(&write(dir.join("Default/Preferences"), "{}"), ago);
}

/// A Firefox folder with one profile, used `ago` seconds ago.
pub(crate) fn firefox_dir(dir: PathBuf, ago: u64) {
    write(
        dir.join("profiles.ini"),
        "[Profile0]\nName=default\nPath=p.default\n",
    );
    used(&write(dir.join("p.default/places.sqlite"), ""), ago);
}

/// On Linux, each browser where its Flatpak, its Snap or Firefox's newer `~/.config/mozilla`
/// keeps it, named by how it was installed only beside another install of the same browser, and a
/// folder linked from another place the table names found once.
#[test]
fn linux_finds_flatpak_snap_and_xdg_installs() {
    let home = home("linux-installs");
    let chrome = |dir: &str, ago| chromium_dir(home.join(dir), ago);
    firefox_dir(home.join(".config/mozilla/firefox"), 10);
    chrome("snap/chromium/common/chromium", 20);
    chrome(".var/app/com.google.Chrome/config/google-chrome", 30);
    chrome(".config/google-chrome", 40);
    firefox_dir(home.join("snap/firefox/common/.mozilla/firefox"), 50);
    firefox_dir(home.join(".config/zen"), 60);
    chrome(
        ".var/app/com.brave.Browser/config/BraveSoftware/Brave-Browser",
        70,
    );
    chrome("snap/opera/current/.config/opera", 80);
    chrome(".var/app/com.vivaldi.Vivaldi/config/vivaldi", 90);
    chrome(".var/app/com.microsoft.Edge/config/microsoft-edge", 100);
    write(
        home.join(".var/app/io.gitlab.librewolf-community/.librewolf/profiles.ini"),
        "[Profile0]\nName=default\nPath=p.default\n",
    );
    fs::create_dir_all(home.join(".var/app/io.gitlab.librewolf-community/.librewolf/p.default"))
        .unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(home.join(".config/mozilla"), home.join(".mozilla")).unwrap();
    assert_eq!(
        listed(&Roots::under(&home, Os::Linux)),
        [
            "Firefox",
            "Chromium",
            "Chrome (Flatpak)",
            "Chrome",
            "Firefox (Snap)",
            "Zen",
            "Brave",
            "Opera",
            "Vivaldi",
            "Edge",
            "LibreWolf",
        ]
    );
    fs::remove_dir_all(home).unwrap();
}

/// On Windows, the Chromium family in Local AppData, Opera's one profile in Roaming, and Firefox
/// from its Microsoft Store package beside the one from its installer; none of Linux's folders.
#[test]
fn windows_finds_the_chromium_family_operas_profile_and_store_installs() {
    let home = home("windows-installs");
    let local = home.join("AppData/Local");
    let chrome = |dir: &str, ago| chromium_dir(local.join(dir), ago);
    firefox_dir(home.join("AppData/Roaming/Mozilla/Firefox"), 10);
    firefox_dir(
        local.join("Packages/Mozilla.Firefox_n80bbvh6b1yt2/LocalCache/Roaming/Mozilla/Firefox"),
        20,
    );
    used(
        &write(
            home.join("AppData/Roaming/Opera Software/Opera Stable/Preferences"),
            "{}",
        ),
        30,
    );
    chrome("Microsoft/Edge/User Data", 40);
    chrome("BraveSoftware/Brave-Browser/User Data", 50);
    chrome("Vivaldi/User Data", 60);
    chrome("Chromium/User Data", 70);
    chrome("imput/Helium/User Data", 80);
    firefox_dir(home.join("AppData/Roaming/zen"), 90);
    firefox_dir(home.join("AppData/Roaming/librewolf"), 100);
    write(home.join(".config/google-chrome/Default/Preferences"), "{}");
    firefox_dir(home.join(".mozilla/firefox"), 5);
    assert_eq!(
        listed(&Roots::under(&home, Os::Windows)),
        [
            "Firefox",
            "Firefox (Microsoft Store)",
            "Opera",
            "Edge",
            "Brave",
            "Vivaldi",
            "Chromium",
            "Helium",
            "Zen",
            "LibreWolf",
        ]
    );
    fs::remove_dir_all(home).unwrap();
}

/// Inside a Flatpak, `XDG_CONFIG_HOME` is the app's own, so the browsers are looked for in the
/// user's `~/.config`, or where the host moved it.
#[test]
fn a_flatpak_looks_for_browsers_in_the_users_config() {
    let home = Path::new("/home/u");
    let config = |vars: &[(&str, &str)]| {
        let var = |name: &str| {
            let found = vars.iter().find(|(each, _)| *each == name);
            found.map(|(_, value)| PathBuf::from(value))
        };
        sources::config_home(var, home)
    };
    assert_eq!(config(&[]), home.join(".config"));
    assert_eq!(config(&[("XDG_CONFIG_HOME", "/x")]), Path::new("/x"));
    let flatpak = [
        ("FLATPAK_ID", "app.ilo.Browser"),
        ("XDG_CONFIG_HOME", "/home/u/.var/app/app.ilo.Browser/config"),
    ];
    assert_eq!(config(&flatpak), home.join(".config"));
    let moved = [flatpak[0], flatpak[1], ("HOST_XDG_CONFIG_HOME", "/x")];
    assert_eq!(config(&moved), Path::new("/x"));
}

/// The Passwords box names where each browser found exports, once a browser however many
/// installs, else the browsers most people on that system come from, which on Linux and Windows
/// aren't Safari.
#[test]
fn the_passwords_box_names_exports_for_the_system() {
    let names = |places: Vec<String>| -> Vec<String> {
        let name = |place: &String| place.split(':').next().unwrap().to_string();
        places.iter().map(name).collect()
    };
    assert_eq!(
        names(super::export_places(&[], Os::Mac)),
        ["Arc", "Chrome", "Firefox", "Safari"]
    );
    assert_eq!(
        names(super::export_places(&[], Os::Windows)),
        ["Chrome", "Edge", "Firefox", "Arc"]
    );
    assert_eq!(
        names(super::export_places(&[], Os::Linux)),
        ["Firefox", "Chrome", "Chromium", "Brave"]
    );
    let home = home("passwords-places");
    let chrome = |dir: &str| write(home.join(dir).join("Default/Preferences"), "{}");
    chrome(".config/google-chrome");
    chrome(".var/app/com.google.Chrome/config/google-chrome");
    let found = sources::find(&Roots::under(&home, Os::Linux));
    assert_eq!(
        super::export_places(&found, Os::Linux),
        ["Chrome: chrome://password-manager/settings › Export passwords"]
    );
    fs::remove_dir_all(home).unwrap();
}

/// The browser as the import sees it: profiles, Spaces with their pinned rows, and favorites.
/// A Space's profile changes only once `load` stands in for the profile having loaded.
#[derive(Default)]
pub(crate) struct Browser {
    roots: Option<Roots>,
    state: RefCell<State>,
    waiting: RefCell<Vec<Box<dyn FnOnce(&mut App)>>>,
    /// History and addresses the core is adding: how many were new, and what waits to hear it.
    adding: RefCell<Vec<(usize, Box<dyn FnOnce(usize, &mut App)>)>>,
    /// What waits for the app to come back to the front.
    active: RefCell<Vec<Box<dyn FnOnce(&mut App)>>>,
    /// The file of passwords the user picks in the open panel, or none to cancel it.
    pub(crate) passwords_file: RefCell<Option<PathBuf>>,
    /// The bookmarks file the user picks.
    bookmarks_file: RefCell<Option<PathBuf>>,
    /// The passwords file the core is importing, and what waits to hear what came of it.
    importing: RefCell<Option<(PathBuf, Box<dyn FnOnce(Passwords, &mut App)>)>>,
    /// Reads the window's Shell, as the core's existing() does for each Today tab's URL, once
    /// `reads_shell` gives it one.
    shell: RefCell<Option<Box<dyn Fn(&App)>>>,
}

#[derive(Default)]
struct State {
    /// The browsers, by their rows' names, whose app the system has an icon for, and the sizes
    /// the view asked for, in pixels.
    icons: Vec<String>,
    icon_pixels: Vec<u32>,
    /// Each browser's last import, as the record keeps it.
    imports: Vec<(String, SystemTime)>,
    next: u64,
    profiles: Vec<(u64, String)>,
    spaces: Vec<Space>,
    favorites: Vec<(u64, Tab)>,
    link_rules: Vec<Rule>,
    /// Where everything else opens, once an import sets it; Little ilo before.
    link_otherwise: Option<u64>,
    /// Spaces put in a profile that hasn't loaded yet.
    loading: Vec<(u64, u64)>,
    /// Profiles made that haven't loaded yet.
    unloaded: Vec<u64>,
    /// Each page in a profile's history: its URL and last visit.
    history: Vec<(u64, String, i64)>,
    /// Each profile's saved addresses.
    addresses: Vec<(u64, Address)>,
    /// How many times the import showed System Settings' Full Disk Access.
    asked_access: usize,
    /// The Web Store pages the import opened, for the user to add an extension.
    store_pages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct Space {
    id: u64,
    name: String,
    profile: u64,
    icon: String,
    theme: Option<Theme>,
    rows: Vec<FakeRow>,
    /// Its Today tabs, top first.
    today: Vec<Tab>,
}

#[derive(Clone, Debug, PartialEq)]
enum FakeRow {
    Tab(Tab),
    Split(Split),
    Folder(u64, String, Vec<FakeRow>),
}

impl State {
    fn id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    fn space(&mut self, id: u64) -> &mut Space {
        self.spaces.iter_mut().find(|space| space.id == id).unwrap()
    }
}

fn folder_rows(rows: &mut Vec<FakeRow>, folder: u64) -> Option<&mut Vec<FakeRow>> {
    if folder == 0 {
        return Some(rows);
    }
    for row in rows {
        if let FakeRow::Folder(id, _, inner) = row {
            if *id == folder {
                return Some(inner);
            }
            if let Some(found) = folder_rows(inner, folder) {
                return Some(found);
            }
        }
    }
    None
}

impl Browser {
    /// A browser as a first launch leaves it: one profile and one empty Space.
    pub(crate) fn new(roots: Option<Roots>) -> Browser {
        let browser = Browser {
            roots,
            ..Browser::default()
        };
        {
            let mut state = browser.state.borrow_mut();
            state.profiles.push((1, "Default".into()));
            state.spaces.push(Space {
                id: 2,
                name: "Space".into(),
                profile: 1,
                icon: String::new(),
                theme: None,
                rows: Vec::new(),
                today: Vec::new(),
            });
            state.next = 2;
        }
        browser
    }

    /// The new profiles have loaded: the Spaces waiting for them move in, and the Spaces changed.
    pub(crate) fn load(&self, cx: &mut App) {
        let mut state = self.state.borrow_mut();
        state.unloaded.clear();
        for (space, profile) in std::mem::take(&mut state.loading) {
            let space = state.space(space);
            if space.rows.is_empty() && space.today.is_empty() {
                space.profile = profile;
            }
        }
        drop(state);
        for then in self.waiting.take() {
            then(cx);
        }
    }

    fn spaces(&self) -> Vec<Space> {
        self.state.borrow().spaces.clone()
    }

    /// From now on, existing() reads the Shell `read` reads for each Today tab, as the core's
    /// does for their URLs (#2428).
    pub(crate) fn reads_shell(&self, read: impl Fn(&App) + 'static) {
        *self.shell.borrow_mut() = Some(Box::new(read));
    }

    /// The URLs of the Today tabs the browser has, in each Space.
    pub(crate) fn today(&self) -> Vec<String> {
        let state = self.state.borrow();
        let tabs = state.spaces.iter().flat_map(|space| &space.today);
        tabs.map(|tab| tab.url.clone()).collect()
    }

    /// The user came back to the app.
    pub(crate) fn activate(&self, cx: &mut App) {
        for then in self.active.take() {
            then(cx);
        }
    }

    /// The core is done with the passwords file it was given.
    pub(crate) fn imported(&self, passwords: Passwords, cx: &mut App) {
        let (_, then) = self.importing.take().unwrap();
        then(passwords, cx);
    }

    /// The core has added the history and addresses it was given.
    fn added(&self, cx: &mut App) {
        for (added, then) in self.adding.take() {
            then(added, cx);
        }
    }

    /// What the browser has, as existing() says it.
    fn here(&self) -> Existing {
        fn rows(of: &[FakeRow]) -> Vec<Row> {
            // The core lists a split's tabs as it lists any pinned tab.
            of.iter()
                .flat_map(|row| match row {
                    FakeRow::Tab(tab) => vec![Row::Tab(tab.url.clone())],
                    FakeRow::Split(split) => (split.tabs.iter())
                        .map(|tab| Row::Tab(tab.url.clone()))
                        .collect(),
                    FakeRow::Folder(id, name, inner) => vec![Row::Folder {
                        id: *id,
                        name: name.clone(),
                        rows: rows(inner),
                    }],
                })
                .collect()
        }
        let state = self.state.borrow();
        Existing {
            profiles: state.profiles.clone(),
            spaces: (state.spaces.iter())
                .map(|space| ExistingSpace {
                    id: space.id,
                    name: space.name.clone(),
                    profile: space.profile,
                    pinned: rows(&space.rows),
                    today: space.today.iter().map(|tab| tab.url.clone()).collect(),
                })
                .collect(),
            favorites: state
                .favorites
                .iter()
                .map(|(profile, tab)| (*profile, tab.url.clone()))
                .collect(),
            link_rules: state.link_rules.clone(),
            link_otherwise: state.link_otherwise.unwrap_or(LITTLE),
        }
    }
}

impl Host for Browser {
    fn app_icon(
        &self,
        source: &sources::Source,
        pixels: u32,
    ) -> Option<std::sync::Arc<gpui::RenderImage>> {
        let mut state = self.state.borrow_mut();
        state.icon_pixels.push(pixels);
        state.icons.contains(&source.name()).then(|| {
            let icon = image::RgbaImage::new(pixels, pixels);
            std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(icon)]))
        })
    }

    fn roots(&self) -> Roots {
        self.roots
            .clone()
            .unwrap_or_else(|| Roots::under(&fixture(), Os::Mac))
    }

    fn existing(&self, cx: &App) -> Existing {
        if let Some(read) = self.shell.borrow().as_ref() {
            for _ in self.today() {
                read(cx);
            }
        }
        self.here()
    }

    fn create_profile(&self, name: &str) -> u64 {
        let mut state = self.state.borrow_mut();
        let id = state.id();
        state.profiles.push((id, name.into()));
        state.unloaded.push(id);
        id
    }

    fn create_space(&self, name: &str, icon: &str, theme: Option<&Theme>) -> u64 {
        let mut state = self.state.borrow_mut();
        let id = state.id();
        let space = Space {
            id,
            name: name.into(),
            profile: 1,
            icon: icon.into(),
            theme: theme.copied(),
            rows: Vec::new(),
            today: Vec::new(),
        };
        state.spaces.push(space);
        id
    }

    fn set_space_profile(&self, space: u64, profile: u64) {
        self.state.borrow_mut().loading.push((space, profile));
    }

    fn space_profile(&self, space: u64) -> u64 {
        self.state.borrow_mut().space(space).profile
    }

    fn create_folder(&self, space: u64, folder: u64, name: &str) -> u64 {
        let mut state = self.state.borrow_mut();
        let id = state.id();
        let rows = folder_rows(&mut state.space(space).rows, folder).unwrap();
        rows.push(FakeRow::Folder(id, name.into(), Vec::new()));
        id
    }

    fn add_pinned(&self, space: u64, folder: u64, tab: &Tab) {
        let mut state = self.state.borrow_mut();
        folder_rows(&mut state.space(space).rows, folder)
            .unwrap()
            .push(FakeRow::Tab(tab.clone()));
    }

    fn add_split(&self, space: u64, folder: u64, split: &Split) {
        let mut state = self.state.borrow_mut();
        folder_rows(&mut state.space(space).rows, folder)
            .unwrap()
            .push(FakeRow::Split(split.clone()));
    }

    fn add_favorite(&self, space: u64, tab: &Tab) {
        let mut state = self.state.borrow_mut();
        let profile = state.space(space).profile;
        state.favorites.push((profile, tab.clone()));
    }

    fn add_today(&self, space: u64, tab: &Tab) {
        self.state
            .borrow_mut()
            .space(space)
            .today
            .insert(0, tab.clone());
    }

    fn add_link_rules(&self, rules: &[Rule], otherwise: Option<u64>) {
        let mut state = self.state.borrow_mut();
        state.link_rules.extend_from_slice(rules);
        state.link_otherwise = otherwise.or(state.link_otherwise);
    }

    fn profile_loaded(&self, profile: u64) -> bool {
        !self.state.borrow().unloaded.contains(&profile)
    }

    /// As the core does: a page it has with a visit as late is skipped.
    fn add_history(
        &self,
        profile: u64,
        pages: &[Page],
        then: Box<dyn FnOnce(usize, &mut App)>,
    ) -> bool {
        let mut state = self.state.borrow_mut();
        let mut added = 0;
        for page in pages {
            let had = state
                .history
                .iter_mut()
                .find(|(each, url, _)| *each == profile && *url == page.url);
            match had {
                Some((_, _, last)) if *last >= page.last_visit => continue,
                Some((_, _, last)) => *last = page.last_visit,
                None => state
                    .history
                    .push((profile, page.url.clone(), page.last_visit)),
            }
            added += 1;
        }
        self.adding.borrow_mut().push((added, then));
        true
    }

    /// As the core does: an address the profile has, or one with nothing in it, is skipped.
    fn add_addresses(
        &self,
        profile: u64,
        addresses: &[Address],
        then: Box<dyn FnOnce(usize, &mut App)>,
    ) -> bool {
        let mut state = self.state.borrow_mut();
        let mut added = 0;
        for address in addresses {
            let had =
                (state.addresses.iter()).any(|(each, had)| *each == profile && had == address);
            if address.fields.is_empty() || had {
                continue;
            }
            state.addresses.push((profile, address.clone()));
            added += 1;
        }
        self.adding.borrow_mut().push((added, then));
        true
    }

    fn when_spaces_change(&self, then: Box<dyn FnOnce(&mut App)>) {
        self.waiting.borrow_mut().push(then);
    }

    fn open_full_disk_access(&self) {
        self.state.borrow_mut().asked_access += 1;
    }

    fn when_active(&self, then: Box<dyn FnOnce(&mut App)>) {
        self.active.borrow_mut().push(then);
    }

    /// As the system's panel answers, after the click that opened it.
    fn choose_file(
        &self,
        exported: Exported,
        then: Box<dyn FnOnce(Option<PathBuf>, &mut App)>,
        cx: &mut App,
    ) {
        let file = match exported {
            Exported::Passwords => self.passwords_file.borrow().clone(),
            Exported::Bookmarks => self.bookmarks_file.borrow().clone(),
        };
        cx.defer(move |cx| then(file, cx));
    }

    fn import_passwords(&self, file: &Path, then: Box<dyn FnOnce(Passwords, &mut App)>) -> bool {
        let mut importing = self.importing.borrow_mut();
        let idle = importing.is_none();
        if idle {
            *importing = Some((file.to_path_buf(), then));
        }
        idle
    }

    fn passwords_profile(&self) -> Option<String> {
        let profiles = &self.state.borrow().profiles;
        (profiles.len() > 1).then(|| profiles[0].1.clone())
    }

    fn open_store_page(&self, url: &str) {
        self.state.borrow_mut().store_pages.push(url.into());
    }

    fn last_import(&self, browser: &str) -> Option<SystemTime> {
        let state = self.state.borrow();
        state
            .imports
            .iter()
            .find(|(each, _)| each == browser)
            .map(|(_, at)| *at)
    }

    fn imported(&self, browser: &str, at: SystemTime) {
        let mut state = self.state.borrow_mut();
        state.imports.retain(|(each, _)| each != browser);
        state.imports.push((browser.into(), at));
    }
}

/// Rows as text: a tab's URL, after its name if it has one, a split's tabs, after its icon, name
/// and orientation if it has them, and a folder's name with its rows.
fn shape(rows: &[FakeRow]) -> String {
    let rows: Vec<String> = rows
        .iter()
        .map(|row| match row {
            FakeRow::Tab(tab) if tab.name.is_empty() => tab.url.clone(),
            FakeRow::Tab(tab) => format!("{}={}", tab.name, tab.url),
            FakeRow::Split(split) => {
                let tabs: Vec<FakeRow> = split.tabs.iter().cloned().map(FakeRow::Tab).collect();
                let stacked = if split.stacked { "stacked" } else { "" };
                let label: Vec<&str> = [split.icon.as_str(), split.name.as_str(), stacked]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect();
                format!(
                    "split{}({})",
                    label
                        .iter()
                        .map(|part| format!(" {part}"))
                        .collect::<String>(),
                    shape(&tabs)
                )
            }
            FakeRow::Folder(_, name, rows) => format!("{name}[{}]", shape(rows)),
        })
        .collect();
    rows.join(", ")
}

fn names(browser: &Browser) -> Vec<String> {
    browser
        .spaces()
        .into_iter()
        .map(|space| space.name)
        .collect()
}

fn profile_names() -> Vec<(String, String)> {
    vec![
        ("Default".into(), "Personal".into()),
        ("Profile 1".into(), "Work".into()),
    ]
}

/// Runs a whole import of `found` into `browser`, its profiles loading at once.
fn import(browser: &Browser, found: &Found, cx: &mut App) {
    let plan = plan::plan(found, &profile_names(), &browser.here());
    let job = super::Job::start(plan, browser);
    browser.load(cx);
    assert!(job.ready(browser));
    job.finish(browser, cx);
}

/// Arc's default profile is the one the browser started with, and its other profile a new one
/// named as Arc named it. Each Space comes in after the others, with its icon and theme, and its
/// pinned tabs and folders only once it's in its profile. Each profile's favorites go to it.
#[gpui::test]
fn an_import_brings_in_spaces_folders_and_favorites(cx: &mut TestAppContext) {
    let browser = Browser::new(None);
    cx.update(|cx| import(&browser, &sidebar(), cx));
    assert_eq!(names(&browser), ["Space", "Personal", "Work"]);
    let state = browser.state.borrow();
    assert_eq!(
        state.profiles,
        [(1, "Default".to_string()), (3, "Work".to_string())]
    );
    let personal = &state.spaces[1];
    assert_eq!((personal.profile, personal.icon.as_str()), (1, "🏠"));
    assert_eq!(personal.theme.unwrap().count, 3);
    assert_eq!(
        shape(&personal.rows),
        "HN=https://news.ycombinator.com/, Reading[https://example.com/a, \
         split(https://example.com/b, https://example.com/c), Deep[https://example.com/d]]"
    );
    let work = &state.spaces[2];
    assert_eq!((work.profile, work.icon.as_str()), (3, "glyph:bolt"));
    let favorites: Vec<_> = state
        .favorites
        .iter()
        .map(|(profile, tab)| (*profile, tab.url.as_str()))
        .collect();
    assert_eq!(
        favorites,
        [
            (1, "https://mail.example.com/"),
            (1, "https://calendar.example.com/"),
            (3, "https://linear.app/")
        ]
    );
}

/// Arc's link rules that match a link's URL come in, in Arc's order, to one of its Spaces whichever
/// way the file names it, to Little Arc or to the Space in front, and so does where it opens
/// everything else; one that matches the app a link came from or goes to a Space that's gone
/// doesn't, nor does a file that can't be read.
#[test]
fn arcs_link_rules_read_as_ours() {
    let rule = |condition, text: &str, to| LinkRule {
        condition,
        text: text.into(),
        to,
    };
    let rules = [
        rule(
            Condition::Contains,
            "work.example.com",
            Destination::Space(1),
        ),
        rule(
            Condition::Is,
            "https://news.ycombinator.com/",
            Destination::Space(0),
        ),
        rule(Condition::Contains, "meet.example.com", Destination::Little),
        rule(Condition::Contains, "front.example.com", Destination::Front),
        rule(
            Condition::Is,
            "https://docs.example.com/",
            Destination::Space(1),
        ),
    ];
    let found = sidebar();
    assert_eq!(found.link_rules, rules);
    assert_eq!(found.link_otherwise, Some(Destination::Space(0)));
    let arc = fixture().join("Library/Application Support/Arc");
    let json = fs::read_to_string(arc.join("StorableSidebar.json")).unwrap();
    let otherwise = |to: &str| {
        let routing = format!(r#"{{"defaultDestination": {to}, "rules": []}}"#);
        arc::parse(&json, Some(&routing)).unwrap().link_otherwise
    };
    // Little Arc is ours' default too, so it changes nothing.
    assert_eq!(
        otherwise(r#"{"littleArc": {"_0": {"mostRecent": {}}}}"#),
        None
    );
    let front = otherwise(r#"{"space": {"_0": {"mostRecent": {}}}}"#);
    assert_eq!(front, Some(Destination::Front));
    let unread = arc::parse(&json, Some("not json")).unwrap();
    assert!(unread.link_rules.is_empty() && unread.link_otherwise.is_none());
}

/// Arc's link rules go after the user's, each to the Space its Arc Space became, Little ilo or
/// the Space in front, and everything else to the Space Arc sends it to. One the user has
/// already, to the Space of that name here or the Space in front, doesn't come twice, and a Space
/// the user picked for everything else stays.
#[gpui::test]
fn arcs_link_rules_go_to_the_spaces_they_became(cx: &mut TestAppContext) {
    let rule = |condition, text: &str, space| Rule {
        condition,
        text: text.into(),
        space,
    };
    let browser = Browser::new(None);
    let mine = rule(Condition::Contains, "mine.example.com", 2);
    browser.state.borrow_mut().link_rules.push(mine.clone());
    cx.update(|cx| import(&browser, &sidebar(), cx));
    let state = browser.state.borrow();
    let (personal, work) = (state.spaces[1].id, state.spaces[2].id);
    let rules = [
        mine,
        rule(Condition::Contains, "work.example.com", work),
        rule(Condition::Is, "https://news.ycombinator.com/", personal),
        rule(Condition::Contains, "meet.example.com", LITTLE),
        rule(Condition::Contains, "front.example.com", FRONT),
        rule(Condition::Is, "https://docs.example.com/", work),
    ];
    assert_eq!(state.link_rules, rules);
    assert_eq!(state.link_otherwise, Some(personal));
    drop(state);

    let browser = Browser::new(None);
    {
        let mut state = browser.state.borrow_mut();
        state.space(2).name = "Personal".into();
        let hn = rule(Condition::Is, "https://news.ycombinator.com/", 2);
        let front = rule(Condition::Contains, "front.example.com", FRONT);
        state.link_rules.extend([hn, front]);
    }
    let plan = plan::plan(&sidebar(), &profile_names(), &browser.here());
    assert_eq!(plan.counts().link_rules, 4);
    assert_eq!(plan.link_otherwise, Some(plan::SpaceRef::Existing(2)));
    browser.state.borrow_mut().link_otherwise = Some(0);
    let plan = plan::plan(&sidebar(), &profile_names(), &browser.here());
    assert_eq!(plan.link_otherwise, None);
}

/// A Space here that an import gives only Today tabs is still the one its link rules go to; where
/// everything else opens, set the first time, is set already.
#[gpui::test]
fn link_rules_go_to_a_space_that_gains_only_today_tabs(cx: &mut TestAppContext) {
    let browser = Browser::new(None);
    cx.update(|cx| import(&browser, &sidebar(), cx));
    let rules = std::mem::take(&mut browser.state.borrow_mut().link_rules);
    let mut found = sidebar();
    found.spaces[0].today.push(Tab {
        url: "https://open.example.com/".into(),
        name: String::new(),
    });
    let plan = plan::plan(&found, &profile_names(), &browser.here());
    let counts = Counts {
        today: 1,
        link_rules: 5,
        ..Counts::default()
    };
    assert_eq!(plan.counts(), counts);
    cx.update(|cx| import(&browser, &found, cx));
    let state = browser.state.borrow();
    assert_eq!(state.spaces.len(), 3, "the import made a Space again");
    assert_eq!(state.link_rules, rules);
}

/// Running it again finds everything there, so it adds nothing: no Space, profile, folder, tab
/// or favorite twice.
#[gpui::test]
fn running_it_again_adds_nothing(cx: &mut TestAppContext) {
    let browser = Browser::new(None);
    cx.update(|cx| import(&browser, &sidebar(), cx));
    let plan = plan::plan(&sidebar(), &profile_names(), &browser.here());
    assert!(plan.is_empty(), "{plan:?}");
}

/// A Space of Arc's name in the same profile is the one it joins: what it has already stays as it
/// is, a folder of the same name gets what it lacks, and the Space keeps its own look.
#[gpui::test]
fn a_space_already_here_gets_only_what_it_lacks(cx: &mut TestAppContext) {
    let browser = Browser::new(None);
    {
        let mut state = browser.state.borrow_mut();
        let space = state.space(2);
        space.name = "Personal".into();
        space.rows = vec![
            FakeRow::Tab(Tab {
                url: "https://news.ycombinator.com/".into(),
                name: String::new(),
            }),
            FakeRow::Folder(
                9,
                "Reading".into(),
                vec![FakeRow::Tab(Tab {
                    url: "https://example.com/a".into(),
                    name: String::new(),
                })],
            ),
        ];
        state.next = 9;
    }
    let found = picked(true, false);
    let plan = plan::plan(&found, &profile_names(), &browser.here());
    let counts = Counts {
        spaces: 1,
        pinned: 4,
        today: 1,
        link_rules: 6,
        ..Counts::default()
    };
    assert_eq!(plan.counts(), counts);
    cx.update(|cx| import(&browser, &found, cx));
    assert_eq!(names(&browser), ["Personal", "Work"]);
    let personal = &browser.spaces()[0];
    assert_eq!(personal.icon, "");
    assert_eq!(
        shape(&personal.rows),
        "https://news.ycombinator.com/, Reading[https://example.com/a, \
         split(https://example.com/b, https://example.com/c), Deep[https://example.com/d]]"
    );
}

/// A split view comes in once: importing again adds nothing, and one with a page already in its
/// place brings only the pages that aren't, a single one as a tab.
#[gpui::test]
fn an_arc_split_view_comes_in_once(cx: &mut TestAppContext) {
    let found = picked(true, false);
    let browser = Browser::new(None);
    cx.update(|cx| import(&browser, &found, cx));
    let first = shape(&browser.spaces()[1].rows);
    assert!(first.contains("split(https://example.com/b, https://example.com/c)"));
    cx.update(|cx| import(&browser, &found, cx));
    assert_eq!(shape(&browser.spaces()[1].rows), first);

    let browser = Browser::new(None);
    {
        let mut state = browser.state.borrow_mut();
        let space = state.space(2);
        space.name = "Personal".into();
        space.rows = vec![FakeRow::Folder(
            9,
            "Reading".into(),
            vec![FakeRow::Tab(tab_of("https://example.com/b"))],
        )];
        state.next = 9;
    }
    cx.update(|cx| import(&browser, &found, cx));
    assert_eq!(
        shape(&browser.spaces()[0].rows),
        "Reading[https://example.com/b, https://example.com/a, https://example.com/c, \
         Deep[https://example.com/d]], HN=https://news.ycombinator.com/"
    );
}

/// A profile of Arc's name here already is the one its Spaces and favorites go to.
#[test]
fn a_profile_of_arcs_name_is_the_one_used() {
    let existing = Existing {
        profiles: vec![(1, "Me".into()), (7, "Work".into())],
        ..Existing::default()
    };
    let plan = plan::plan(&sidebar(), &profile_names(), &existing);
    assert!(plan.new_profiles.is_empty());
    let work = plan.spaces.iter().find_map(|space| match &space.into {
        plan::Target::New { name, profile, .. } if name == "Work" => Some(*profile),
        _ => None,
    });
    assert_eq!(work, Some(plan::ProfileRef::Existing(7)));
}

/// The view, as the first launch shows it: the browsers found, Arc's choices, the import, which
/// reads Arc's files away from the UI and waits for the new profile, then what came in, and Done.
/// The view says once it has read and goes on by itself, and when it finishes, for a host to show
/// while the view doesn't.
#[gpui::test]
fn the_view_imports_what_the_user_picks(cx: &mut TestAppContext) {
    let browser = Rc::new(Browser::new(None));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, true));
    let events = Rc::new(RefCell::new(Vec::new()));
    let _events = cx.update(|cx| {
        let events = events.clone();
        cx.subscribe(&view, move |_, event: &ImportEvent, _| {
            events.borrow_mut().push(event.clone())
        })
    });
    view.read_with(cx, |view, _| {
        let names: Vec<_> = view
            .sources
            .iter()
            .map(|source| source.browser.name)
            .collect();
        assert_eq!(names, ["Arc"]);
    });
    view.update(cx, |view, cx| {
        view.pick(0, cx);
        view.import(cx);
    });
    cx.run_until_parked();
    // The Work Space waits for its profile, empty.
    view.read_with(cx, |view, _| {
        assert_eq!(view.running().as_deref(), Some("Importing from Arc…"));
    });
    assert!(browser.spaces()[2].rows.is_empty());
    cx.update(|cx| browser.load(cx));
    view.read_with(cx, |view, _| {
        let super::Step::Done(said) = &view.step else {
            panic!()
        };
        assert_eq!(
            said.as_ref(),
            "2 Spaces, 6 pinned tabs, 1 open tab, 3 favorites and 6 link rules came in from Arc."
        );
    });
    assert_eq!(browser.spaces()[2].rows.len(), 1);
    view.update(cx, |view, cx| view.done(cx));
    cx.run_until_parked();
    let said =
        "2 Spaces, 6 pinned tabs, 1 open tab, 3 favorites and 6 link rules came in from Arc.";
    assert_eq!(
        *events.borrow(),
        [
            ImportEvent::Started,
            ImportEvent::Placed,
            ImportEvent::Finished(said.into()),
            ImportEvent::Done
        ]
    );
    // Again, there's nothing new.
    view.update(cx, |view, cx| {
        view.pick(0, cx);
        view.import(cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        let super::Step::Done(said) = &view.step else {
            panic!()
        };
        assert_eq!(said.as_ref(), "Everything from Arc is here already.");
    });
}

/// Each state's one button, the one that does what it's for, is in the host's accent: Settings'
/// green, or the first launch's Signal. The list has none, as each browser's Import… is one of
/// many, and an import going on by itself none until a profile hasn't opened, when it's Try Again.
/// At the end it's Add Favorites while there are favorites to add, else Done.
#[gpui::test]
fn each_states_one_button_is_in_the_hosts_accent(cx: &mut TestAppContext) {
    let signal = gpui::rgb(crate::tokens::shell::onboarding::SIGNAL);
    let ground = Look::on(gpui::rgb(crate::palette::INK), gpui::white().into(), signal);
    assert_eq!(ground.accent, signal);
    assert_eq!(
        Look::settings(false).accent,
        gpui::rgb(crate::palette::NO_THEME_ACCENT)
    );
    let primary = |view: &Entity<ImportView>, cx: &mut TestAppContext| {
        view.read_with(cx, |view, _| view.primary())
    };

    // Arc's import, which waits for its Work profile.
    let browser = Rc::new(Browser::new(None));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false).look(ground));
    view.read_with(cx, |view, _| assert_eq!(view.look.accent, signal));
    assert_eq!(primary(&view, cx), None);
    view.update(cx, |view, cx| view.pick(0, cx));
    assert_eq!(primary(&view, cx), Some("Import"));
    view.update(cx, |view, cx| view.import(cx));
    cx.run_until_parked();
    assert_eq!(
        primary(&view, cx),
        None,
        "an import under way goes on by itself"
    );
    cx.executor().advance_clock(STALL);
    cx.run_until_parked();
    assert_eq!(primary(&view, cx), Some("Try Again"));
    cx.update(|cx| browser.load(cx));
    assert_eq!(
        says(&view, cx),
        "2 Spaces, 6 pinned tabs, 1 open tab, 3 favorites and 6 link rules came in from Arc."
    );
    assert_eq!(
        primary(&view, cx),
        Some("Done"),
        "Arc's favorites come with it, so none are offered"
    );

    // Chrome's, whose end offers the favorites its history visits most.
    let made = chrome("chrome-primary", &[("Default", "Me")], SystemTime::now());
    let browser = Rc::new(Browser::new(Some(Roots::under(&made, Os::Linux))));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    import_first(&view, cx);
    cx.update(|cx| browser.added(cx));
    assert_eq!(primary(&view, cx), Some("Add Favorites"));
    view.update(cx, |view, cx| view.add_ticked(cx));
    assert_eq!(
        primary(&view, cx),
        Some("Done"),
        "the offer ends once they're added"
    );
    fs::remove_dir_all(made).unwrap();
}

/// Esc in a browser's choices goes back to the list, as Back does, and keeps the browser picked
/// with what was switched off, so its Import… shows them as they were left. On the list, Esc is
/// left to the host.
#[gpui::test]
fn escape_from_the_choices_goes_back_to_the_list_keeping_the_pick(cx: &mut TestAppContext) {
    let made = chrome("chrome-escape", &[("Default", "Me")], SystemTime::now());
    let host: Rc<dyn Host> = Rc::new(Browser::new(Some(Roots::under(&made, Os::Linux))));
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut view = ImportView::new(host, false).headed();
        view.take_keys(cx);
        view
    });
    let focus = view.read_with(cx, |view, _| view.keys().unwrap());
    cx.update(|window, cx| window.focus(&focus, cx));
    view.update(cx, |view, cx| {
        view.pick(0, cx);
        view.toggle(Kind::History, cx);
    });
    let picked = |view: &ImportView| match &view.step {
        super::Step::Pick { picked, .. } => Some(picked.clone()),
        _ => None,
    };
    view.read_with(cx, |view, _| {
        assert_eq!(view.crumb().as_deref(), Some("Chrome"));
        assert!(view.under_way());
    });
    cx.simulate_keystrokes("escape");
    view.read_with(cx, |view, _| {
        assert!(
            matches!(view.step, super::Step::List),
            "Esc stayed in the choices"
        );
        assert_eq!((view.kept(), view.crumb()), (Some(0), None));
        assert!(!view.under_way());
    });
    view.update(cx, |view, cx| view.pick(0, cx));
    view.read_with(cx, |view, _| {
        let picked = picked(view).unwrap();
        assert!(
            !picked.contains(&Kind::History),
            "History came back on: {picked:?}"
        );
        assert!(picked.contains(&Kind::Bookmarks));
    });
    // Back keeps it too; on the list Esc does nothing of the view's.
    view.update(cx, |view, cx| view.back(cx));
    cx.simulate_keystrokes("escape");
    view.read_with(cx, |view, _| {
        assert!(matches!(view.step, super::Step::List));
        assert_eq!(view.kept(), Some(0));
    });
    fs::remove_dir_all(made).unwrap();
}

/// What the view says, as the user reads it: how the import under way goes, or what came in.
fn says(view: &Entity<ImportView>, cx: &mut TestAppContext) -> String {
    view.read_with(cx, |view, _| match &view.step {
        super::Step::Running { name, run, .. } => super::progress(name, run).to_string(),
        super::Step::Done(said) => said.to_string(),
        _ => panic!("neither importing nor done"),
    })
}

/// The cards come in the order the core makes the Spaces, each under the profile it goes into:
/// Arc's Personal in the profile the browser started with, Work in the one the import makes,
/// waiting with none of their tabs in while Work's profile loads. Chrome's card ticks once its
/// tabs are in, while the core still adds its history.
#[gpui::test]
fn cards_come_as_the_core_makes_the_spaces_and_tick_once_their_tabs_are_in(
    cx: &mut TestAppContext,
) {
    let browser = Rc::new(Browser::new(None));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    import_first(&view, cx);
    let made: Vec<String> = (browser.spaces().iter().skip(1))
        .map(|space| space.name.clone())
        .collect();
    let cards = view.read_with(cx, |view, _| view.cards());
    let names: Vec<String> = cards.iter().map(|card| card.0.clone()).collect();
    assert_eq!(
        names, made,
        "the cards aren't in the order the core made the Spaces"
    );
    assert_eq!(
        cards,
        [
            ("Personal".into(), ("Default".into(), false), 0, 5, false),
            ("Work".into(), ("Work".into(), true), 0, 1, false),
        ]
    );

    let made = chrome("chrome-cards", &[("Default", "Me")], SystemTime::now());
    let browser = Rc::new(Browser::new(Some(Roots::under(&made, Os::Linux))));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    import_first(&view, cx);
    assert_eq!(
        view.read_with(cx, |view, _| view.cards()),
        [("Chrome".into(), ("Default".into(), false), 3, 3, true)]
    );
    cx.update(|cx| browser.added(cx));
    view.read_with(cx, |view, _| {
        assert!(
            matches!(view.step, super::Step::Done(_)),
            "it didn't end once the history was in"
        )
    });
    fs::remove_dir_all(made).unwrap();
}

/// Picks the browser the view found first and imports all it offers.
fn import_first(view: &Entity<ImportView>, cx: &mut TestAppContext) {
    view.update(cx, |view, cx| {
        view.pick(0, cx);
        view.import(cx);
    });
    cx.run_until_parked();
}

/// Back while the browser picked is still read, with Import pressed meanwhile, leaves it: what
/// the read finds never comes in.
#[gpui::test]
fn back_while_the_pick_reads_brings_nothing_in(cx: &mut TestAppContext) {
    let browser = Rc::new(Browser::new(None));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    let spaces = browser.spaces().len();
    view.update(cx, |view, cx| {
        view.pick(0, cx);
        view.import(cx);
        assert_eq!(view.primary(), Some("Import"), "it went on before its read");
        view.back(cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(matches!(view.step, super::Step::List))
    });
    assert_eq!(browser.spaces().len(), spaces);
}

/// Cancel while the import waits for the profile it made stops it, "Stopped.", with the Spaces it
/// made left empty, even once the profile opens, and Import Again the one thing to do: it reads the
/// browser again with what was switched on and fills them. The record of the import comes only
/// with its end, and the list then says when it ran, with Import Again….
#[gpui::test]
fn cancel_leaves_the_spaces_empty_and_import_again_fills_them(cx: &mut TestAppContext) {
    let browser = Rc::new(Browser::new(None));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    import_first(&view, cx);
    assert_eq!(says(&view, cx), "Importing from Arc…");
    let cancelled = |cx: &mut TestAppContext| {
        view.read_with(cx, |view, _| {
            matches!(view.step, super::Step::Cancelled { made: 2, .. })
        })
    };
    view.update(cx, |view, cx| view.cancel(cx));
    assert!(cancelled(cx), "Cancel didn't stop it with its two Spaces");
    view.read_with(cx, |view, _| {
        assert_eq!(view.primary(), Some("Import Again"));
        assert_eq!(view.crumb().as_deref(), Some("Arc"));
        let guide = view.guide();
        assert_eq!(guide.title, "Stopped.");
        assert_eq!(
            guide.line,
            "Nothing came in. The Spaces it made are empty, and importing again fills them."
        );
    });
    assert_eq!(
        super::stopped(2).as_ref(),
        "Stopped. Nothing came in. The two Spaces it made are empty, and importing again fills \
         them."
    );
    cx.update(|cx| browser.load(cx));
    cx.run_until_parked();
    assert!(cancelled(cx), "the profile opening went on with it");
    assert!(
        browser.spaces()[1..]
            .iter()
            .all(|space| space.rows.is_empty())
    );
    assert_eq!(
        browser.last_import("Arc"),
        None,
        "a stopped import was kept as one"
    );
    // Import Again is its button.
    view.update(cx, |view, cx| view.press(cx));
    cx.run_until_parked();
    assert_eq!(
        says(&view, cx),
        "6 pinned tabs, 1 open tab, 3 favorites and 6 link rules came in from Arc."
    );
    assert_eq!(browser.spaces()[2].rows.len(), 1);
    assert!(browser.last_import("Arc").is_some());
    view.update(cx, |view, cx| view.done(cx));
    view.read_with(cx, |view, _| {
        let arc = &view.sources[0];
        let line = view.row_line(arc);
        assert!(
            line.starts_with("Personal, Work · imported today at ")
                && line.ends_with("; again brings only what's new"),
            "{line}"
        );
        assert_eq!(view.row_button(arc), "Import Again…");
    });
    // A view made again, as Settings' is each time it shows, reads the record.
    let host: Rc<dyn Host> = browser.clone();
    let again = cx.new(|_| ImportView::new(host, false));
    again.read_with(cx, |view, _| {
        assert_eq!(view.row_button(&view.sources[0]), "Import Again…")
    });
}

/// The record of an import lasts the run alone, kept by the host in memory: a new session's host,
/// as after a relaunch, has none, so Arc's row is Import… again with its profiles alone, and
/// importing Arc again still adds nothing twice, as the import skips what's here.
#[gpui::test]
fn a_new_session_lists_a_browser_as_never_imported(cx: &mut TestAppContext) {
    let browser = Rc::new(Browser::new(None));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    import_first(&view, cx);
    cx.update(|cx| browser.load(cx));
    assert!(browser.last_import("Arc").is_some());

    // The next session: the same Spaces here, a host with no record.
    let next = Rc::new(Browser::new(None));
    next.state.replace(browser.state.take());
    next.state.borrow_mut().imports.clear();
    let host: Rc<dyn Host> = next.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    view.read_with(cx, |view, _| {
        let arc = &view.sources[0];
        assert_eq!(view.row_line(arc), "Personal, Work");
        assert_eq!(view.row_button(arc), "Import…");
    });
    import_first(&view, cx);
    assert_eq!(says(&view, cx), "Everything from Arc is here already.");
}

/// A stopped import's Esc goes back to Settings' page, as its Done does; on the first launch it
/// skips the step, which its corner says.
#[gpui::test]
fn esc_from_a_stopped_import_goes_back_to_the_list(cx: &mut TestAppContext) {
    let browser = Rc::new(Browser::new(None));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    import_first(&view, cx);
    view.update(cx, |view, cx| {
        view.cancel(cx);
        assert!(view.escape(cx), "Esc did nothing of the view's");
        assert!(matches!(view.step, super::Step::List));
    });
    let host: Rc<dyn Host> = browser.clone();
    let guided = cx.new(|_| ImportView::new(host, false).guided());
    import_first(&guided, cx);
    guided.update(cx, |view, cx| {
        view.cancel(cx);
        assert_eq!(view.guide().corner, super::Corner::Skip);
        assert!(!view.escape(cx), "the first launch's Esc didn't skip");
    });
}

/// A profile the import made that hasn't opened after STALL ends the wait, naming the profile and
/// the Spaces that wait for it, with Try Again the one thing to do, which asks for it again and
/// waits as long again; once it opens, the import goes on, and the wait's end no longer comes.
#[gpui::test]
fn a_profile_that_doesnt_open_offers_try_again(cx: &mut TestAppContext) {
    let browser = Rc::new(Browser::new(None));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    import_first(&view, cx);
    cx.executor().advance_clock(STALL - Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(says(&view, cx), "Importing from Arc…");
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    let stalled = "Work hasn't opened in 20 seconds, so its Space waits.";
    assert_eq!(says(&view, cx), stalled);
    view.read_with(cx, |view, _| {
        assert_eq!(view.running().as_deref(), Some(stalled));
        assert_eq!(view.primary(), Some("Try Again"));
        let guide = view.guide();
        assert_eq!(guide.title, "Work hasn't opened.");
        assert_eq!(
            guide.line,
            format!(
                "{} made the profile Work for one of Arc's Spaces, and it hasn't opened in 20 \
                 seconds.",
                env!("APP_NAME")
            )
        );
        assert_eq!(guide.button.as_deref(), Some("Try Again"));
    });
    let asked = browser.state.borrow().loading.len();
    // Try Again is the stalled import's button.
    view.update(cx, |view, cx| view.press(cx));
    assert_eq!(browser.state.borrow().loading.len(), asked + 1);
    assert_eq!(says(&view, cx), "Importing from Arc…");
    cx.update(|cx| browser.load(cx));
    let said =
        "2 Spaces, 6 pinned tabs, 1 open tab, 3 favorites and 6 link rules came in from Arc.";
    assert_eq!(says(&view, cx), said);
    cx.executor().advance_clock(STALL);
    cx.run_until_parked();
    assert_eq!(says(&view, cx), said);
}

/// Once the wait for Work's profile has ended, Work's card is held, dimmed with ⚠ and still, while
/// Personal's, whose profile is open, waits on as it was; Cancel then leaves both Spaces it made as
/// cards, outlined and empty. Before the wait ends, and once Work opens, none is held.
#[gpui::test]
fn a_stalled_profiles_card_is_held_and_cancel_leaves_the_made_ones_empty(cx: &mut TestAppContext) {
    let browser = Rc::new(Browser::new(None));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    import_first(&view, cx);
    let none = (Vec::<String>::new(), Vec::<String>::new());
    assert_eq!(view.read_with(cx, |view, _| view.held_and_stopped()), none);
    cx.executor().advance_clock(STALL);
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |view, _| view.held_and_stopped()),
        (vec!["Work".to_string()], Vec::new()),
        "Work's card, and it alone, isn't held once Work hasn't opened"
    );
    view.update(cx, |view, cx| view.cancel(cx));
    assert_eq!(
        view.read_with(cx, |view, _| view.held_and_stopped()),
        (Vec::new(), vec!["Personal".to_string(), "Work".to_string()]),
        "Cancel didn't leave the Spaces it made as cards"
    );

    let browser = Rc::new(Browser::new(None));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    import_first(&view, cx);
    cx.executor().advance_clock(STALL);
    cx.run_until_parked();
    cx.update(|cx| browser.load(cx));
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |view, _| view.held_and_stopped()), none);
}

/// While the core adds the history and addresses, the view says what came in so far, and what
/// it's still adding, each as it comes; then there's nothing left to cancel.
#[gpui::test]
fn counts_show_as_they_come_in(cx: &mut TestAppContext) {
    let home = chrome("chrome-progress", &[("Default", "Me")], SystemTime::now());
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Linux))));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    import_first(&view, cx);
    assert_eq!(
        says(&view, cx),
        "1 Space, 3 pinned tabs and 1 open tab came in from Chrome. Adding its history and \
         addresses…"
    );
    let (added, then) = browser.adding.borrow_mut().remove(0);
    cx.update(|cx| then(added, cx));
    assert_eq!(
        says(&view, cx),
        "1 Space, 3 pinned tabs, 1 open tab and 2 pages of history came in from Chrome. Adding \
         its addresses…"
    );
    view.update(cx, |view, cx| view.cancel(cx));
    cx.update(|cx| browser.added(cx));
    assert_eq!(
        says(&view, cx),
        "1 Space, 3 pinned tabs, 1 open tab, 2 pages of history and 1 address came in from \
         Chrome."
    );
    fs::remove_dir_all(home).unwrap();
}

/// A Chromium `Bookmarks` file with `bar` on its bookmarks bar, a folder of other bookmarks and
/// the phone's.
fn bookmarks_file(bar: &str) -> String {
    format!(
        r#"{{"roots": {{
            "bookmark_bar": {{"type": "folder", "name": "Bookmarks bar", "children": [{bar}]}},
            "other": {{"type": "folder", "name": "Other bookmarks", "children": [
                {{"type": "url", "name": "Docs", "url": "https://docs.example.com/"}}
            ]}},
            "synced": {{"type": "folder", "name": "Mobile bookmarks", "children": []}}
        }}, "version": 1}}"#
    )
}

fn bookmark(name: &str, url: &str) -> String {
    format!(r#"{{"type": "url", "name": "{name}", "url": "{url}"}}"#)
}

/// A short bar's bookmarks are the Space's first pinned tabs and folders, then the other
/// bookmarks in a folder of their name. A bookmarklet, a browser's own page and a folder left
/// empty stay out, and the phone's empty folder too.
#[test]
fn bookmarks_become_pinned_tabs_and_folders() {
    let bar = [
        bookmark("Mail", "https://mail.example.com/"),
        bookmark("Bookmarklet", "javascript:alert(1)"),
        bookmark("Flags", "chrome://flags"),
        format!(
            r#"{{"type": "folder", "name": "", "children": [{}]}}"#,
            bookmark("", "https://example.com/a")
        ),
        r#"{"type": "folder", "name": "Empty", "children": []}"#.to_string(),
    ];
    let pinned = chromium::bookmarks(&bookmarks_file(&bar.join(","))).unwrap();
    let docs = Node::Tab(Tab {
        url: "https://docs.example.com/".into(),
        name: "Docs".into(),
    });
    assert_eq!(
        pinned,
        [
            Node::Tab(Tab {
                url: "https://mail.example.com/".into(),
                name: "Mail".into(),
            }),
            Node::Folder {
                name: "Folder".into(),
                children: vec![tab("https://example.com/a")],
            },
            Node::Folder {
                name: "Other bookmarks".into(),
                children: vec![docs],
            },
        ]
    );
    assert!(chromium::bookmarks("{}").is_err());
}

/// A bar longer than the sidebar can hold comes in as a folder of its name.
#[test]
fn a_long_bookmarks_bar_comes_as_a_folder() {
    let bar: Vec<String> = (0..=read::BAR_ROWS)
        .map(|n| bookmark("", &format!("https://example.com/{n}")))
        .collect();
    let pinned = chromium::bookmarks(&bookmarks_file(&bar.join(","))).unwrap();
    let Node::Folder { name, children } = &pinned[0] else {
        panic!("{pinned:?}")
    };
    assert_eq!(
        (name.as_str(), children.len()),
        ("Bookmarks bar", read::BAR_ROWS + 1)
    );
    assert_eq!(pinned.len(), 2);
}

/// Microseconds since 1601, as Chromium's History keeps times, for `ago` seconds before `now`.
fn chromium_time(now: SystemTime, ago: u64) -> i64 {
    let since_1970 = (now - Duration::from_secs(ago))
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    since_1970.as_micros() as i64 + 11_644_473_600_000_000
}

/// A Chromium `History` file at `path` with `pages`: URL, title, visits, typed, seconds ago and
/// whether it's hidden.
fn history_file(path: &Path, now: SystemTime, pages: &[(&str, Option<&str>, i64, i64, u64, bool)]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    _ = fs::remove_file(path);
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute_batch(
        "CREATE TABLE urls(id INTEGER PRIMARY KEY AUTOINCREMENT, url LONGVARCHAR, title LONGVARCHAR, \
         visit_count INTEGER DEFAULT 0 NOT NULL, typed_count INTEGER DEFAULT 0 NOT NULL, \
         last_visit_time INTEGER NOT NULL, hidden INTEGER DEFAULT 0 NOT NULL)",
    )
    .unwrap();
    for &(url, title, visits, typed, ago, hidden) in pages {
        db.execute(
            "INSERT INTO urls(url, title, visit_count, typed_count, last_visit_time, hidden) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![url, title, visits, typed, chromium_time(now, ago), hidden],
        )
        .unwrap();
    }
}

/// The pages of the last 90 days, the latest first, with their titles, visits and times; not the
/// pages Chromium hides, those older, nor its own. The file itself is only read.
#[test]
fn history_is_the_last_90_days_of_pages() {
    let home = home("history");
    let path = home.join("History");
    let now = SystemTime::now();
    let day = 24 * 60 * 60;
    history_file(
        &path,
        now,
        &[
            (
                "https://old.example.com/",
                Some("Old"),
                1,
                0,
                91 * day,
                false,
            ),
            ("https://example.com/", Some("Example"), 4, 2, 60, false),
            ("https://frame.example.com/", None, 1, 0, 30, true),
            ("chrome://settings/", Some("Settings"), 1, 0, 20, false),
            ("https://new.example.com/", None, 1, 0, 10, false),
        ],
    );
    let before = fs::read(&path).unwrap();
    let pages = chromium::history(&path, now - Duration::from_secs(90 * day)).unwrap();
    let urls: Vec<&str> = pages.iter().map(|page| page.url.as_str()).collect();
    assert_eq!(urls, ["https://new.example.com/", "https://example.com/"]);
    let example = &pages[1];
    assert_eq!(
        (example.title.as_str(), example.visits, example.typed),
        ("Example", 4, 2)
    );
    let ago = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64
        - example.last_visit;
    assert!((60_000_000..61_000_000).contains(&ago), "{ago}");
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(chromium::history(&home.join("missing"), now).is_err());
    fs::remove_dir_all(home).unwrap();
}

/// Chrome on Linux, as the tests make it: `profiles` by folder and name, each with a short bar, two
/// tabs open, one of them a bookmark's, two pages of history and an address.
pub(crate) fn chrome(name: &str, profiles: &[(&str, &str)], now: SystemTime) -> PathBuf {
    let home = home(name);
    let chrome = home.join(".config/google-chrome");
    let cache: Vec<String> = (profiles.iter())
        .map(|(dir, name)| format!(r#""{dir}": {{"name": "{name}"}}"#))
        .collect();
    write(
        chrome.join("Local State"),
        &format!(
            r#"{{"profile": {{"info_cache": {{{}}}}}}}"#,
            cache.join(",")
        ),
    );
    for (dir, _) in profiles {
        let profile = chrome.join(dir);
        write(profile.join("Preferences"), "{}");
        let bar = [
            bookmark("Mail", "https://mail.example.com/"),
            bookmark("", "https://example.com/"),
        ];
        write(profile.join("Bookmarks"), &bookmarks_file(&bar.join(",")));
        let tabs = [
            (SET_WINDOW_TYPE, ints(&[1, 0])),
            (SET_TAB_WINDOW, ints(&[1, 2])),
            (SET_TAB_INDEX_IN_WINDOW, ints(&[2, 0])),
            (NAVIGATION, navigation(2, 0, "https://open.example.com/")),
            (SET_TAB_WINDOW, ints(&[1, 3])),
            (SET_TAB_INDEX_IN_WINDOW, ints(&[3, 1])),
            (NAVIGATION, navigation(3, 0, "https://mail.example.com/")),
            (MARKER, Vec::new()),
        ];
        let session = profile.join("Sessions/Session_13400000000000000");
        fs::create_dir_all(session.parent().unwrap()).unwrap();
        fs::write(session, session_file(3, &tabs)).unwrap();
        history_file(
            &profile.join("History"),
            now,
            &[
                ("https://example.com/", Some("Example"), 3, 1, 60, false),
                ("https://news.example.com/", Some("News"), 1, 0, 120, false),
            ],
        );
        web_data(
            &profile.join("Web Data"),
            "address_type_tokens",
            &[
                ("a", NAME_FULL, "Ada Lovelace", 4),
                ("a", CITY, "London", 4),
            ],
        );
    }
    home
}

const NAME_FULL: i32 = 7;
const CITY: i32 = 33;
const COUNTRY: i32 = 36;

/// A Web Data file with each of `fields` of an address in `table`: its address's guid, then its
/// FieldType, value and VerificationStatus, as Chromium keeps them. Cards are in a table of their
/// own, encrypted, which the import never reads.
fn web_data(path: &Path, table: &str, fields: &[(&str, i32, &str, i32)]) {
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {table} (guid VARCHAR, type INTEGER, value VARCHAR, \
         verification_status INTEGER DEFAULT 0, observations BLOB);
         CREATE TABLE IF NOT EXISTS credit_cards (guid VARCHAR, card_number_encrypted BLOB);
         INSERT INTO credit_cards VALUES ('card', x'763130');"
    ))
    .unwrap();
    for (guid, kind, value, status) in fields {
        db.execute(
            &format!(
                "INSERT INTO {table} (guid, type, value, verification_status) \
                 VALUES (?1, ?2, ?3, ?4)"
            ),
            rusqlite::params![guid, kind, value, status],
        )
        .unwrap();
    }
}

fn address(fields: &[(i32, &str, i32)]) -> Address {
    Address {
        fields: (fields.iter())
            .map(|&(kind, value, status)| Field {
                kind,
                value: value.into(),
                status,
            })
            .collect(),
    }
}

/// The addresses a profile saved come field by field, its own and its account's, from the table
/// the Web Data of its version has, with what's empty left out.
#[test]
fn a_profiles_addresses_come_from_its_web_data() {
    let home = home("web-data");
    let now = home.join("Web Data");
    web_data(
        &now,
        "address_type_tokens",
        &[
            ("b", CITY, "Paris", 4),
            ("a", COUNTRY, "GB", 0),
            ("a", CITY, "London", 4),
            ("a", NAME_FULL, "", 4),
            ("c", NAME_FULL, "", 4),
        ],
    );
    assert_eq!(
        chromium::addresses(&now).unwrap(),
        [
            address(&[(CITY, "London", 4), (COUNTRY, "GB", 0)]),
            address(&[(CITY, "Paris", 4)]),
        ]
    );
    // Before version 134, the user's own and their account's were apart.
    let old = home.join("Old Web Data");
    web_data(
        &old,
        "local_addresses_type_tokens",
        &[("a", CITY, "Oslo", 2)],
    );
    web_data(&old, "contact_info_type_tokens", &[("b", CITY, "Rome", 3)]);
    assert_eq!(
        chromium::addresses(&old).unwrap(),
        [address(&[(CITY, "Oslo", 2)]), address(&[(CITY, "Rome", 3)])]
    );
    fs::remove_dir_all(home).unwrap();
}

const SET_TAB_WINDOW: u8 = 0;
const SET_TAB_INDEX_IN_WINDOW: u8 = 2;
const NAVIGATION: u8 = 6;
const SET_SELECTED_NAVIGATION_INDEX: u8 = 7;
const SET_WINDOW_TYPE: u8 = 9;
const TAB_CLOSED: u8 = 16;
const PRUNED: u8 = 24;
const MARKER: u8 = 255;

/// A session file as Chromium writes one: "SNSS" and its version, then each command's size, id
/// and what it says.
fn session_file(version: i32, commands: &[(u8, Vec<u8>)]) -> Vec<u8> {
    let mut file = b"SNSS".to_vec();
    file.extend(version.to_le_bytes());
    for (id, body) in commands {
        file.extend((body.len() as u16 + 1).to_le_bytes());
        file.push(*id);
        file.extend(body);
    }
    file
}

fn ints(values: &[i32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// An UpdateTabNavigation's pickle: the tab, the page's index, its URL and title, and the start of
/// what follows them, padded as base::Pickle pads.
fn navigation(tab: i32, index: i32, url: &str) -> Vec<u8> {
    let pad = |bytes: &mut Vec<u8>| bytes.resize(bytes.len().next_multiple_of(4), 0);
    let mut payload = ints(&[tab, index, url.len() as i32]);
    payload.extend(url.as_bytes());
    pad(&mut payload);
    let title: Vec<u8> = "Title".encode_utf16().flat_map(u16::to_le_bytes).collect();
    payload.extend(ints(&[5]));
    payload.extend(title);
    pad(&mut payload);
    payload.extend(ints(&[0, 1, 0]));
    let mut pickle = ints(&[payload.len() as i32]);
    pickle.extend(payload);
    pickle
}

/// A session's open tabs come window by window in their order, each at the page it showed, as
/// Chromium would restore them: not a popup's, a closed tab's or one Chromium's own page shows. A
/// page updated again is the later one, and a tab whose page was pruned shows the one before.
#[test]
fn a_session_reads_as_the_tabs_it_had_open() {
    let tab = |window, tab, index| {
        [
            (SET_TAB_WINDOW, ints(&[window, tab])),
            (SET_TAB_INDEX_IN_WINDOW, ints(&[tab, index])),
        ]
    };
    let mut commands = vec![
        (SET_WINDOW_TYPE, ints(&[3, 0])),
        (SET_WINDOW_TYPE, ints(&[1, 0])),
        (SET_WINDOW_TYPE, ints(&[2, 1])),
    ];
    commands.extend(tab(1, 10, 1));
    commands.extend([
        (NAVIGATION, navigation(10, 0, "https://a.example.com/")),
        (NAVIGATION, navigation(10, 1, "https://b.example.com/")),
        (SET_SELECTED_NAVIGATION_INDEX, ints(&[10, 0])),
    ]);
    commands.extend(tab(1, 11, 0));
    commands.push((NAVIGATION, navigation(11, 0, "https://first.example.com/")));
    commands.extend(tab(2, 12, 0));
    commands.push((NAVIGATION, navigation(12, 0, "https://popup.example.com/")));
    commands.extend(tab(1, 13, 2));
    commands.push((NAVIGATION, navigation(13, 0, "chrome://settings/")));
    commands.push((MARKER, Vec::new()));
    commands.extend(tab(1, 14, 3));
    commands.push((NAVIGATION, navigation(14, 0, "https://closed.example.com/")));
    commands.push((TAB_CLOSED, ints(&[14, 0, 0, 0])));
    commands.extend(tab(3, 15, 0));
    commands.extend([
        (NAVIGATION, navigation(15, 0, "https://kept.example.com/")),
        (NAVIGATION, navigation(15, 1, "https://pruned.example.com/")),
        (
            NAVIGATION,
            navigation(15, 2, "https://pruned-too.example.com/"),
        ),
        (SET_SELECTED_NAVIGATION_INDEX, ints(&[15, 2])),
        (PRUNED, ints(&[15, 1, 2])),
    ]);
    commands.extend(tab(3, 16, 1));
    commands.extend([
        (NAVIGATION, navigation(16, 0, "https://before.example.com/")),
        (NAVIGATION, navigation(16, 0, "https://after.example.com/")),
    ]);
    let tabs = session::parse(&session_file(3, &commands)).unwrap();
    let urls: Vec<&str> = tabs.iter().map(|tab| tab.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://first.example.com/",
            "https://a.example.com/",
            "https://kept.example.com/",
            "https://after.example.com/"
        ]
    );
    assert!(tabs.iter().all(|tab| tab.name.is_empty()));
    // A last command a crash cut short is left out, and the rest still come.
    let mut cut = session_file(3, &commands);
    cut.extend([40, 0, NAVIGATION, 1, 2]);
    assert_eq!(session::parse(&cut).unwrap(), tabs);
}

/// The newest session Chromium would restore is the one read: an encrypted one, which import never
/// decrypts, and one left before its first state was written are passed over, as are the
/// recently closed tabs in Tabs_ and a file that isn't a session.
#[test]
fn a_session_chromium_wouldnt_restore_is_passed_over() {
    let profile = home("sessions");
    assert_eq!(session::open_tabs(&profile).unwrap(), []);
    let open = |url| {
        vec![
            (SET_WINDOW_TYPE, ints(&[1, 0])),
            (SET_TAB_WINDOW, ints(&[1, 2])),
            (NAVIGATION, navigation(2, 0, url)),
            (MARKER, Vec::new()),
        ]
    };
    let sessions = profile.join("Sessions");
    fs::create_dir_all(&sessions).unwrap();
    let mut unfinished = open("https://unfinished.example.com/");
    unfinished.pop();
    let files = [
        (
            "Tabs_500",
            session_file(3, &open("https://closed.example.com/")),
        ),
        ("Session_400", b"not a session".to_vec()),
        (
            "Session_300",
            session_file(5, &open("https://encrypted.example.com/")),
        ),
        ("Session_200", session_file(3, &unfinished)),
        (
            "Session_100",
            session_file(3, &open("https://older.example.com/")),
        ),
        (
            "Session_99",
            session_file(3, &open("https://oldest.example.com/")),
        ),
    ];
    for (name, bytes) in files {
        fs::write(sessions.join(name), bytes).unwrap();
    }
    let tabs = session::open_tabs(&profile).unwrap();
    let urls: Vec<&str> = tabs.iter().map(|tab| tab.url.as_str()).collect();
    assert_eq!(urls, ["https://older.example.com/"]);
    fs::remove_dir_all(profile).unwrap();
}

/// A browser with more than one profile gets a Space for each, named after the browser and the
/// profile; what was left off doesn't come.
#[test]
fn each_profile_of_a_browser_gets_a_space_named_after_it() {
    let now = SystemTime::now();
    let home = chrome(
        "chrome-two",
        &[("Default", "Me"), ("Profile 1", "Work")],
        now,
    );
    let source = &super::importable(&Roots::under(&home, Os::Linux))[0];
    assert_eq!(
        read::kinds(source),
        [
            Kind::Bookmarks,
            Kind::OpenTabs,
            Kind::History,
            Kind::Addresses,
            Kind::Extensions,
        ]
    );
    let found = read::read(source, &[Kind::Bookmarks], now).unwrap();
    let spaces: Vec<_> = (found.spaces.iter())
        .map(|space| (space.name.as_str(), &space.profile))
        .collect();
    assert_eq!(
        spaces,
        [
            ("Chrome – Me", &FromProfile::Default),
            ("Chrome – Work", &FromProfile::Custom("Profile 1".into()))
        ]
    );
    assert!(found.history.is_empty() && found.addresses.is_empty());
    let found = read::read(source, &[Kind::History], now).unwrap();
    assert!(found.spaces.is_empty());
    assert_eq!(found.history.len(), 2);
    let found = read::read(source, &[Kind::Addresses], now).unwrap();
    assert_eq!(found.addresses.len(), 2);
    fs::remove_dir_all(home).unwrap();
}

/// Chrome's bookmarks come in as a Space named Chrome in the profile the browser started with,
/// and its history and addresses once the core has added them, by as many as were new. Again, the
/// core finds every one there, and nothing new came.
#[gpui::test]
fn chromes_bookmarks_and_history_come_in(cx: &mut TestAppContext) {
    let home = chrome("chrome-view", &[("Default", "Me")], SystemTime::now());
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Linux))));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    let import = |cx: &mut TestAppContext| {
        view.update(cx, |view, cx| {
            view.pick(0, cx);
            view.import(cx);
        });
        cx.run_until_parked();
    };
    let said = |cx: &mut TestAppContext| {
        view.read_with(cx, |view, _| match &view.step {
            super::Step::Done(said) => said.to_string(),
            _ => panic!("still importing"),
        })
    };
    import(cx);
    // The history and addresses are still going in.
    view.read_with(cx, |view, _| {
        assert_eq!(view.running().as_deref(), Some("Importing from Chrome…"))
    });
    cx.update(|cx| browser.added(cx));
    assert_eq!(
        said(cx),
        "1 Space, 3 pinned tabs, 1 open tab, 2 pages of history and 1 address came in from \
         Chrome."
    );
    let chrome = &browser.spaces()[1];
    assert_eq!((chrome.name.as_str(), chrome.profile), ("Chrome", 1));
    assert_eq!(
        shape(&chrome.rows),
        "Mail=https://mail.example.com/, https://example.com/, \
         Other bookmarks[Docs=https://docs.example.com/]"
    );
    // The open tab that's a bookmark is pinned already.
    let today: Vec<&str> = chrome.today.iter().map(|tab| tab.url.as_str()).collect();
    assert_eq!(today, ["https://open.example.com/"]);
    import(cx);
    cx.update(|cx| browser.added(cx));
    assert_eq!(said(cx), "Everything from Chrome is here already.");
    assert_eq!(browser.state.borrow().history.len(), 2);
    assert_eq!(browser.state.borrow().addresses.len(), 1);
    fs::remove_dir_all(home).unwrap();
}

/// With no browser to import from, the view says it's empty, so a first launch leaves its step
/// out.
#[gpui::test]
fn a_machine_with_no_browser_is_empty(cx: &mut TestAppContext) {
    let home = home("empty");
    let host: Rc<dyn Host> = Rc::new(Browser::new(Some(Roots::under(&home, Os::Mac))));
    let view = cx.new(|_| ImportView::new(host, true));
    view.read_with(cx, |view, _| assert!(view.is_empty()));
    let arc: Rc<dyn Host> = Rc::new(Browser::new(None));
    let view = cx.new(|_| ImportView::new(arc, true));
    view.read_with(cx, |view, _| assert!(!view.is_empty()));
    fs::remove_dir_all(home).unwrap();
}

/// On a host's ground, the rows' lines, controls and grey text are its ink over it, edged so the
/// groups stand off a ground with no card, light or dark.
#[test]
fn rows_on_a_hosts_ground_mix_its_ink() {
    let light = Look::on(
        gpui::rgb(crate::tokens::palette::DIALOG_LIGHT),
        gpui::rgb(crate::palette::INK),
        gpui::rgb(crate::tokens::shell::onboarding::SIGNAL),
    );
    assert!(light.line.r < light.surface.r && light.control.r > light.detail.r);
    assert_eq!(light.edge, Some(light.line));
}

/// A control under the pointer is a quiet step toward pressed and short of it, on Settings' card
/// and on a host's ground alike, so it shows it can be clicked and a press still shows more.
#[test]
fn a_control_under_the_pointer_is_a_step_short_of_pressed() {
    let light = |color: gpui::Rgba| color.r + color.g + color.b;
    let dark = Look::settings(true);
    assert!(light(dark.control) < light(dark.hovered) && light(dark.hovered) < light(dark.pressed));
    let paper = Look::settings(false);
    assert!(
        light(paper.control) > light(paper.hovered) && light(paper.hovered) > light(paper.pressed)
    );
    let ground = Look::on(
        gpui::rgb(crate::tokens::palette::DIALOG_LIGHT),
        gpui::rgb(crate::palette::INK),
        gpui::rgb(crate::tokens::shell::onboarding::SIGNAL),
    );
    assert!(
        light(ground.control) > light(ground.hovered)
            && light(ground.hovered) > light(ground.pressed)
    );
}

/// Firefox's default profile is the one its install section names, as Firefox starts with it,
/// else the one marked default, else the first; its things go to the profile this browser
/// started with.
#[test]
fn firefoxs_default_profile_is_the_one_it_starts_with() {
    let home = home("firefox-default");
    let firefox = home.join(".mozilla/firefox");
    let profiles = |ini: &str| {
        write(firefox.join("profiles.ini"), ini);
        let source = sources::find(&Roots::under(&home, Os::Linux)).remove(0);
        (source.profiles.into_iter())
            .map(|profile| (profile.name, profile.default))
            .collect::<Vec<_>>()
    };
    write(firefox.join("a.default/places.sqlite"), "");
    write(firefox.join("b.default-release/places.sqlite"), "");
    let both = |a, b| {
        vec![
            ("default".to_string(), a),
            ("default-release".to_string(), b),
        ]
    };
    let sections = "[Profile1]\nName=default\nIsRelative=1\nPath=a.default\nDefault=1\n\n\
                    [Profile0]\nName=default-release\nIsRelative=1\nPath=b.default-release\n";
    assert_eq!(
        profiles(&format!(
            "{sections}\n[Install4F96D1932A9F858E]\nDefault=b.default-release\nLocked=1\n"
        )),
        both(false, true)
    );
    assert_eq!(profiles(sections), both(true, false));
    assert_eq!(
        profiles(&sections.replace("Default=1\n", "")),
        both(true, false)
    );
    fs::remove_dir_all(home).unwrap();
}

/// A `places.sqlite` at `path` as Firefox lays one out, with its own folders, in write-ahead mode
/// with the log kept, as while Firefox runs: what goes in stays in the log while the connection
/// it returns is open.
fn places_file(path: &Path) -> rusqlite::Connection {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    _ = fs::remove_file(path);
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute_batch(
        "PRAGMA journal_mode = WAL; PRAGMA wal_autocheckpoint = 0;
         CREATE TABLE moz_places(id INTEGER PRIMARY KEY, url LONGVARCHAR, title LONGVARCHAR,
           visit_count INTEGER DEFAULT 0, hidden INTEGER DEFAULT 0 NOT NULL,
           typed INTEGER DEFAULT 0 NOT NULL, last_visit_date INTEGER, guid TEXT);
         CREATE TABLE moz_bookmarks(id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER DEFAULT NULL,
           parent INTEGER, position INTEGER, title LONGVARCHAR, guid TEXT);
         INSERT INTO moz_bookmarks(id, type, parent, position, title, guid) VALUES
           (1, 2, NULL, 0, '', 'root________'), (2, 2, 1, 0, 'menu', 'menu________'),
           (3, 2, 1, 1, 'toolbar', 'toolbar_____'), (4, 2, 1, 2, 'tags', 'tags________'),
           (5, 2, 1, 3, 'unfiled', 'unfiled_____'), (6, 2, 1, 4, 'mobile', 'mobile______');",
    )
    .unwrap();
    db
}

/// A page in `db`'s `moz_places`, visited `visits` times, last `ago` seconds before `now`, and
/// its id.
fn place(db: &rusqlite::Connection, url: &str, title: &str, visits: i64, ago: Option<u64>) -> i64 {
    let now = SystemTime::now();
    let last = ago.map(|ago| read::micros(now - Duration::from_secs(ago)));
    db.execute(
        "INSERT INTO moz_places(url, title, visit_count, typed, last_visit_date, hidden) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            url,
            title,
            visits,
            (visits > 2) as i64,
            last,
            url.contains("frame")
        ],
    )
    .unwrap();
    db.last_insert_rowid()
}

/// A bookmark (type 1) of `place` or a folder (type 2) in `parent`, after what's there.
fn mark(db: &rusqlite::Connection, id: i64, parent: i64, title: &str, place: Option<i64>) {
    db.execute(
        "INSERT INTO moz_bookmarks(id, type, fk, parent, position, title) VALUES \
         (?1, ?2, ?3, ?4, (SELECT COUNT(*) FROM moz_bookmarks WHERE parent = ?4), ?5)",
        rusqlite::params![
            id,
            if place.is_some() { 1 } else { 2 },
            place,
            parent,
            title
        ],
    )
    .unwrap();
}

/// Firefox, as the tests make it on Linux: one profile whose toolbar has a bookmark and a folder,
/// whose menu and phone each have one, with a tag that isn't a bookmark, and two pages visited
/// lately. The connection is open, so all of it is in the log.
fn firefox(name: &str) -> (PathBuf, rusqlite::Connection) {
    let home = home(name);
    let firefox = home.join(".mozilla/firefox");
    write(
        firefox.join("profiles.ini"),
        "[Profile0]\nName=default-release\nIsRelative=1\nPath=x.default-release\n",
    );
    let db = places_file(&firefox.join("x.default-release/places.sqlite"));
    let day = 24 * 60 * 60;
    let mail = place(&db, "https://mail.example.com/", "Inbox", 1, Some(120));
    let docs = place(&db, "https://docs.example.com/", "Docs", 0, None);
    let prefs = place(&db, "about:preferences", "Settings", 1, Some(30));
    let mozilla = place(&db, "https://mozilla.example.org/", "Mozilla", 0, None);
    let news = place(&db, "https://news.example.com/", "News", 0, None);
    let phone = place(
        &db,
        "https://phone.example.com/",
        "Phone",
        1,
        Some(91 * day),
    );
    place(&db, "https://example.com/", "Example", 4, Some(60));
    place(&db, "https://frame.example.com/", "", 1, Some(40));
    mark(&db, 10, 3, "Mail", Some(mail));
    mark(&db, 11, 3, "Work", None);
    mark(&db, 12, 11, "", Some(docs));
    mark(&db, 13, 3, "Settings", Some(prefs));
    mark(&db, 14, 2, "Mozilla", Some(mozilla));
    mark(&db, 15, 4, "news", None);
    mark(&db, 16, 15, "", Some(news));
    mark(&db, 17, 6, "Phone", Some(phone));
    db.execute(
        "INSERT INTO moz_bookmarks(type, parent, position) VALUES (3, 3, 9)",
        [],
    )
    .unwrap();
    (home, db)
}

/// Firefox's toolbar comes as the bar, its menu, other bookmarks and phone's in folders, but for
/// its tags and its own pages; its history is the last 90 days' pages but for those it hides. It
/// reads what Firefox still holds in its log.
#[test]
fn firefoxs_bookmarks_and_history_come_from_places() {
    let (home, db) = firefox("firefox-places");
    let profile = home.join(".mozilla/firefox/x.default-release");
    let wal = profile.join("places.sqlite-wal");
    assert!(fs::metadata(&wal).unwrap().len() > 0);
    let now = SystemTime::now();
    let since = now - Duration::from_secs(read::HISTORY_DAYS * 24 * 60 * 60);
    let read = firefox::read(&profile, &[Kind::Bookmarks, Kind::History], since).unwrap();
    let shape = |nodes: &[Node]| {
        fn shape(nodes: &[Node]) -> String {
            let nodes: Vec<String> = (nodes.iter())
                .map(|node| match node {
                    Node::Tab(tab) if tab.name.is_empty() => tab.url.clone(),
                    Node::Tab(tab) => format!("{}={}", tab.name, tab.url),
                    Node::Folder { name, children } => format!("{name}[{}]", shape(children)),
                    Node::Split(_) => unreachable!("bookmarks have no split views"),
                })
                .collect();
            nodes.join(", ")
        }
        shape(nodes)
    };
    assert_eq!(
        shape(&read.pinned),
        "Mail=https://mail.example.com/, Work[https://docs.example.com/], \
         Bookmarks Menu[Mozilla=https://mozilla.example.org/], \
         Mobile Bookmarks[Phone=https://phone.example.com/]"
    );
    let pages: Vec<_> = (read.pages.iter())
        .map(|page| {
            (
                page.url.as_str(),
                page.title.as_str(),
                page.visits,
                page.typed,
            )
        })
        .collect();
    assert_eq!(
        pages,
        [
            ("https://example.com/", "Example", 4, 1),
            ("https://mail.example.com/", "Inbox", 1, 0)
        ]
    );
    let only = firefox::read(&profile, &[Kind::History], since).unwrap();
    assert!(only.pinned.is_empty() && only.pages.len() == 2);
    // Windows doesn't delete a file while it's open.
    drop(db);
    fs::remove_dir_all(home).unwrap();
}

/// Firefox's bookmarks come in as a Space named Firefox in the profile the browser started with,
/// as Firefox's default profile, and its history with them; again, nothing new comes.
#[gpui::test]
fn firefoxs_bookmarks_and_history_come_in(cx: &mut TestAppContext) {
    let (home, db) = firefox("firefox-view");
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Linux))));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    view.read_with(cx, |view, _| {
        let source = &view.sources[0];
        assert_eq!(source.browser.name, "Firefox");
        assert_eq!(read::kinds(source), [Kind::Bookmarks, Kind::History]);
    });
    let import = |cx: &mut TestAppContext| {
        view.update(cx, |view, cx| {
            view.pick(0, cx);
            view.import(cx);
        });
        cx.run_until_parked();
        cx.update(|cx| browser.added(cx));
        view.read_with(cx, |view, _| match &view.step {
            super::Step::Done(said) => said.to_string(),
            _ => panic!("still importing"),
        })
    };
    assert_eq!(
        import(cx),
        "1 Space, 4 pinned tabs and 2 pages of history came in from Firefox."
    );
    let space = &browser.spaces()[1];
    assert_eq!((space.name.as_str(), space.profile), ("Firefox", 1));
    assert_eq!(browser.state.borrow().profiles.len(), 1);
    assert_eq!(import(cx), "Everything from Firefox is here already.");
    // Windows doesn't delete a file while it's open.
    drop(db);
    fs::remove_dir_all(home).unwrap();
}

fn plist_dict(entries: &[(&str, plist::Value)]) -> plist::Value {
    let mut dict = plist::Dictionary::new();
    for (key, value) in entries {
        dict.insert(key.to_string(), value.clone());
    }
    plist::Value::Dictionary(dict)
}

fn safari_leaf(title: &str, url: &str) -> plist::Value {
    plist_dict(&[
        ("WebBookmarkType", "WebBookmarkTypeLeaf".into()),
        ("URLString", url.into()),
        ("URIDictionary", plist_dict(&[("title", title.into())])),
    ])
}

fn safari_list(title: &str, children: Vec<plist::Value>) -> plist::Value {
    plist_dict(&[
        ("WebBookmarkType", "WebBookmarkTypeList".into()),
        ("Title", title.into()),
        ("Children", plist::Value::Array(children)),
    ])
}

/// Safari's `Bookmarks.plist`, as it keeps it: the history's proxy, the bar with a bookmark and a
/// folder, the menu, the Reading List, a folder of the user's and a bookmark outside any.
fn safari_bookmarks() -> plist::Value {
    let proxy = plist_dict(&[
        ("WebBookmarkType", "WebBookmarkTypeProxy".into()),
        ("Title", "History".into()),
    ]);
    safari_list(
        "",
        vec![
            proxy,
            safari_list(
                "BookmarksBar",
                vec![
                    safari_leaf("Mail", "https://mail.example.com/"),
                    safari_list(
                        "Work",
                        vec![safari_leaf("Docs", "https://docs.example.com/")],
                    ),
                    safari_leaf("Bad", "javascript:alert(1)"),
                ],
            ),
            safari_list(
                "BookmarksMenu",
                vec![safari_leaf("Apple", "https://apple.example.com/")],
            ),
            safari_list(
                "com.apple.ReadingList",
                vec![safari_leaf("Later", "https://later.example.com/")],
            ),
            safari_list(
                "Trips",
                vec![safari_leaf("Map", "https://map.example.com/")],
            ),
            safari_leaf("Loose", "https://loose.example.com/"),
        ],
    )
}

fn node_shape(nodes: &[Node]) -> String {
    let nodes: Vec<String> = (nodes.iter())
        .map(|node| match node {
            Node::Tab(tab) => format!("{}={}", tab.name, tab.url),
            Node::Folder { name, children } => format!("{name}[{}]", node_shape(children)),
            Node::Split(_) => unreachable!("bookmarks have no split views"),
        })
        .collect();
    nodes.join(", ")
}

/// Safari's bar comes as the bar, its menu, with what's outside any folder, and the user's
/// folders in folders, and its Reading List in one of its own when it's picked; the binary file
/// and the XML one read the same.
#[test]
fn safaris_bookmarks_and_reading_list_become_pinned_tabs() {
    let mut binary = Vec::new();
    safari_bookmarks().to_writer_binary(&mut binary).unwrap();
    let mut xml = Vec::new();
    safari_bookmarks().to_writer_xml(&mut xml).unwrap();
    let both = safari::bookmarks(&binary, true, true).unwrap();
    assert_eq!(
        node_shape(&both),
        "Mail=https://mail.example.com/, Work[Docs=https://docs.example.com/], \
         Bookmarks Menu[Apple=https://apple.example.com/, Loose=https://loose.example.com/], \
         Trips[Map=https://map.example.com/], Reading List[Later=https://later.example.com/]"
    );
    assert_eq!(safari::bookmarks(&xml, true, true).unwrap(), both);
    let list = safari::bookmarks(&binary, false, true).unwrap();
    assert_eq!(
        node_shape(&list),
        "Reading List[Later=https://later.example.com/]"
    );
    let marks = safari::bookmarks(&binary, true, false).unwrap();
    assert_eq!(marks.len(), 4);
    assert!(safari::bookmarks(b"not a plist", true, true).is_err());
}

/// Safari's `History.db` at `path`, its pages as (URL, visits, and each visit's title and seconds
/// before `now`).
fn safari_history(path: &Path, now: SystemTime, pages: &[(&str, i64, &[(&str, u64)])]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    _ = fs::remove_file(path);
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute_batch(
        "CREATE TABLE history_items(id INTEGER PRIMARY KEY, url TEXT NOT NULL UNIQUE,
           visit_count INTEGER NOT NULL);
         CREATE TABLE history_visits(id INTEGER PRIMARY KEY, history_item INTEGER NOT NULL,
           visit_time REAL NOT NULL, title TEXT);",
    )
    .unwrap();
    for (url, visits, each) in pages {
        db.execute(
            "INSERT INTO history_items(url, visit_count) VALUES (?1, ?2)",
            rusqlite::params![url, visits],
        )
        .unwrap();
        let item = db.last_insert_rowid();
        for (title, ago) in *each {
            let at = read::micros(now - Duration::from_secs(*ago)) as f64 / 1e6 - 978_307_200.;
            db.execute(
                "INSERT INTO history_visits(history_item, visit_time, title) VALUES (?1, ?2, ?3)",
                rusqlite::params![item, at, title],
            )
            .unwrap();
        }
    }
}

/// Safari's history is the last 90 days' pages, the latest first, each by its last visit's title.
#[test]
fn safaris_history_is_the_last_90_days_of_pages() {
    let home = home("safari-history");
    let path = home.join("History.db");
    let now = SystemTime::now();
    let day = 24 * 60 * 60;
    safari_history(
        &path,
        now,
        &[
            (
                "https://example.com/",
                2,
                &[("Before", 120), ("Example", 60)],
            ),
            ("https://old.example.com/", 1, &[("Old", 91 * day)]),
            ("https://new.example.com/", 1, &[("New", 10)]),
            ("file:///Users/me/a.html", 1, &[("A", 30)]),
            ("feed://example.com/", 1, &[("Feed", 20)]),
        ],
    );
    let pages = safari::history(&path, now - Duration::from_secs(90 * day)).unwrap();
    let pages: Vec<_> = (pages.iter())
        .map(|page| (page.url.as_str(), page.title.as_str(), page.visits))
        .collect();
    assert_eq!(
        pages,
        [
            ("https://new.example.com/", "New", 1),
            ("file:///Users/me/a.html", "A", 1),
            ("https://example.com/", "Example", 2)
        ]
    );
    fs::remove_dir_all(home).unwrap();
}

/// Safari's folder as the tests make it on the Mac, its bookmarks file kept from the app as macOS
/// keeps it without Full Disk Access, unless the tests run where nothing can be kept from them.
#[cfg(unix)]
pub(crate) fn safari(name: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let home = home(name);
    let safari = home.join("Library/Safari");
    let mut file = Vec::new();
    safari_bookmarks().to_writer_binary(&mut file).unwrap();
    fs::create_dir_all(&safari).unwrap();
    let bookmarks = safari.join("Bookmarks.plist");
    fs::write(&bookmarks, file).unwrap();
    safari_history(
        &safari.join("History.db"),
        SystemTime::now(),
        &[("https://example.com/", 1, &[("Example", 60)])],
    );
    fs::set_permissions(&bookmarks, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&bookmarks).is_ok() {
        fs::remove_dir_all(home).unwrap();
        return None;
    }
    Some(home)
}

#[cfg(unix)]
pub(crate) fn let_read(home: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let bookmarks = home.join("Library/Safari/Bookmarks.plist");
    fs::set_permissions(bookmarks, fs::Permissions::from_mode(0o644)).unwrap();
}

/// Picking a browser reads it, away from the UI, and its choices then say how much of each kind
/// comes, with the profiles it goes into: Arc's Spaces by name, a new profile for its Work, and
/// Chrome's bookmarks, open tabs, history and address. A kind with nothing to bring isn't a choice:
/// they fold into one line that names them. Until the read is in, each says it's counting, and
/// Import pressed then goes on once it is.
#[gpui::test]
fn picking_a_browser_shows_its_kinds_with_their_counts(cx: &mut TestAppContext) {
    let browser = Rc::new(Browser::new(None));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    view.update(cx, |view, cx| view.pick(0, cx));
    view.read_with(cx, |view, _| {
        let counting: Vec<_> = view.choices().into_iter().map(|(_, said)| said).collect();
        assert_eq!(counting, ["Counting…"; 6]);
        assert_eq!(view.note(), None);
        assert_eq!(view.nothing(), None);
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.choices(),
            [
                (Kind::Spaces, "6 pinned tabs · 6 link rules".into()),
                (Kind::OpenTabs, "1".into()),
                (Kind::Favorites, "3".into()),
            ]
        );
        assert_eq!(
            view.nothing().as_deref(),
            Some("Nothing to bring from History, Addresses or Extensions.")
        );
        assert_eq!(
            view.note().as_deref(),
            Some(
                "Into **Default** and a new profile, **Work**. Cards and sign-ins don't come: \
                 you sign in to each site again."
            )
        );
    });

    let made = chrome("chrome-counts", &[("Default", "Me")], SystemTime::now());
    let browser = Rc::new(Browser::new(Some(Roots::under(&made, Os::Linux))));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    view.update(cx, |view, cx| {
        view.pick(0, cx);
        view.toggle(Kind::OpenTabs, cx);
        view.import(cx);
    });
    cx.run_until_parked();
    cx.update(|cx| browser.added(cx));
    // Import pressed while it counted went on once it had, without what was switched off.
    assert_eq!(
        says(&view, cx),
        "1 Space, 3 pinned tabs, 2 pages of history and 1 address came in from Chrome."
    );
    view.update(cx, |view, cx| {
        view.done(cx);
        view.pick(0, cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        let said: Vec<_> = view.choices().into_iter().map(|(_, said)| said).collect();
        assert_eq!(said, ["3 in 1 folder", "2", "2 pages", "1"]);
        assert_eq!(
            view.nothing().as_deref(),
            Some("Nothing to bring from Extensions.")
        );
        assert_eq!(
            view.note().as_deref(),
            Some(
                "Into **Default**. Cards and sign-ins don't come: you sign in to each site again."
            )
        );
    });
    fs::remove_dir_all(made).unwrap();
}

/// Reading everything a browser offers as it's picked, then leaving out what's switched off,
/// gives what reading what's switched on alone would, for each set of Arc's and Chrome's kinds.
#[test]
fn what_is_left_of_a_whole_read_is_what_a_read_of_the_picked_gives() {
    let made = chrome("chrome-keep", &[("Default", "Me")], SystemTime::now());
    let now = SystemTime::now();
    let arc = Roots::under(&fixture(), Os::Mac);
    for roots in [arc, Roots::under(&made, Os::Linux)] {
        let source = super::importable(&roots).remove(0);
        let kinds = read::kinds(&source);
        let whole = read::read(&source, kinds, now).unwrap();
        for mask in 0..1u32 << kinds.len() {
            let picked: Vec<Kind> = (kinds.iter().enumerate())
                .filter(|(at, _)| mask & 1 << at != 0)
                .map(|(_, kind)| *kind)
                .collect();
            assert_eq!(
                read::keep(&whole, &source, &picked),
                Some(read::read(&source, &picked, now).unwrap()),
                "{} {picked:?}",
                source.name()
            );
        }
    }
    fs::remove_dir_all(made).unwrap();
}

/// The counts read as people read them: thousands apart, and one of a thing in the singular.
#[test]
fn counts_read_with_their_thousands_apart() {
    let tally = read::Tally {
        spaces: 1,
        pinned: 1,
        pages: 12408,
        ..read::Tally::default()
    };
    assert_eq!(
        super::kind_count(Kind::Spaces, &tally).as_deref(),
        Some("1 pinned tab")
    );
    assert_eq!(
        super::kind_count(Kind::History, &tally).as_deref(),
        Some("12,408 pages")
    );
    assert_eq!(super::kind_count(Kind::Addresses, &tally), None);
    let two = [("Personal".to_string(), true), ("Work".to_string(), true)];
    assert_eq!(
        super::into_note(&two),
        "Into two new profiles, **Personal** and **Work**. Cards and sign-ins don't come: you \
         sign in to each site again."
    );
}

/// Picking Safari, whose files the system keeps from the app, asks for Full Disk Access at once,
/// before its choices, with Open System Settings the one thing to do and a line saying it waits.
/// Nothing has started, so the view says nothing has ended either.
#[cfg(unix)]
#[gpui::test]
fn a_read_that_needs_access_shows_full_disk_access(cx: &mut TestAppContext) {
    let Some(home) = safari("safari-access") else {
        return;
    };
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Mac))));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    let events = Rc::new(RefCell::new(Vec::new()));
    let _events = cx.update(|cx| {
        let events = events.clone();
        cx.subscribe(&view, move |_, event: &ImportEvent, _| {
            events.borrow_mut().push(event.clone())
        })
    });
    view.read_with(cx, |view, _| {
        let source = &view.sources[0];
        assert_eq!(source.browser.name, "Safari");
        assert_eq!(
            read::kinds(source),
            [Kind::Bookmarks, Kind::ReadingList, Kind::History]
        );
    });
    view.update(cx, |view, cx| view.pick(0, cx));
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(matches!(view.step, super::Step::NoAccess { .. }));
        assert_eq!(view.primary(), Some("Open System Settings"));
        assert_eq!(view.access().as_deref(), Some("Waiting for access…"));
        assert_eq!(view.crumb().as_deref(), Some("Safari"));
    });
    assert!(events.borrow().is_empty(), "{:?}", events.borrow());
    // Back leaves it, and coming back to the app then reads nothing.
    view.update(cx, |view, cx| view.back(cx));
    cx.update(|cx| browser.activate(cx));
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(matches!(view.step, super::Step::List))
    });
    fs::remove_dir_all(home).unwrap();
}

/// Safari's row says it imports from Safari, as every browser's does, though the system keeps its
/// files from the app until it has Full Disk Access: a screen reader hears "Import from Safari…",
/// and once it's picked, the access it waits for has Open System Settings by its title (#1911).
#[cfg(unix)]
#[gpui::test]
fn safaris_row_says_it_imports_from_safari(cx: &mut TestAppContext) {
    use crate::settings_rows::spoken::{pressable, unnamed};
    let Some(home) = safari("safari-named") else {
        return;
    };
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Mac))));
    let host: Rc<dyn Host> = browser.clone();
    let (view, cx) = cx.add_window_view(|_, _| ImportView::new(host, false).headed());
    cx.run_until_parked();
    let names = |cx: &mut gpui::VisualTestContext| -> Vec<String> {
        let said = pressable(cx);
        assert_eq!(unnamed(&said), [], "a control without a name: {said:?}");
        said.into_iter().map(|(_, name, _)| name).collect()
    };
    let list = names(cx);
    assert!(
        list.contains(&"Import from Safari…".to_string()),
        "Safari's row in {list:?}"
    );
    view.update(cx, |view, cx| view.pick(0, cx));
    cx.run_until_parked();
    let waiting = names(cx);
    assert!(
        waiting.contains(&"Open System Settings".to_string()),
        "the access Safari waits for in {waiting:?}"
    );
    fs::remove_dir_all(home).unwrap();
}

/// Back in the app with access still off, the wait says so and what to do; once access is on,
/// coming back shows Safari's choices with their counts, and Import brings them in.
#[cfg(unix)]
#[gpui::test]
fn returning_with_access_still_off_says_still_no_access(cx: &mut TestAppContext) {
    let Some(home) = safari("safari-still") else {
        return;
    };
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Mac))));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    view.update(cx, |view, cx| view.pick(0, cx));
    cx.run_until_parked();
    cx.update(|cx| browser.activate(cx));
    cx.run_until_parked();
    let still = format!(
        "Still no access. If you turned it on, quit {} and open it again.",
        env!("APP_NAME")
    );
    view.read_with(cx, |view, _| {
        assert_eq!(view.access(), Some(still.clone()));
        assert_eq!(view.primary(), Some("Open System Settings"));
    });
    // Still off, it says so again, and keeps waiting.
    cx.update(|cx| browser.activate(cx));
    cx.run_until_parked();
    view.read_with(cx, |view, _| assert_eq!(view.access(), Some(still.clone())));
    let_read(&home);
    cx.update(|cx| browser.activate(cx));
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(view.access(), None);
        let said: Vec<_> = view.choices().into_iter().map(|(_, said)| said).collect();
        assert_eq!(said, ["5 in 3 folders", "1", "1 page"]);
    });
    view.update(cx, |view, cx| view.import(cx));
    cx.run_until_parked();
    cx.update(|cx| browser.added(cx));
    assert_eq!(
        says(&view, cx),
        "1 Space, 6 pinned tabs and 1 page of history came in from Safari."
    );
    assert_eq!(browser.spaces()[1].name, "Safari");
    fs::remove_dir_all(home).unwrap();
}

/// A file the system keeps from the app is no access, which asks for Full Disk Access; one that
/// isn't there is nothing to bring.
#[cfg(unix)]
#[test]
fn a_file_kept_from_the_app_is_no_access() {
    let Some(home) = safari("safari-kept") else {
        return;
    };
    let safari = home.join("Library/Safari");
    let since = SystemTime::now() - Duration::from_secs(60 * 60);
    assert_eq!(
        safari::read(&safari, &[Kind::Bookmarks], since).err(),
        Some(ReadError::NoAccess)
    );
    let history = safari::read(&safari, &[Kind::History], since).unwrap();
    assert_eq!(history.pages.len(), 1);
    let_read(&home);
    fs::remove_file(safari.join("Bookmarks.plist")).unwrap();
    assert!(
        (safari::read(&safari, &[Kind::Bookmarks], since)
            .unwrap()
            .pinned)
            .is_empty()
    );
    fs::remove_dir_all(home).unwrap();
}

/// A passwords export as Chrome writes one, of made-up logins.
pub(crate) const PASSWORDS: &str = "name,url,username,password,note\n\
    example.com,https://example.com/,check@example.test,not-a-real-password,\n";

/// Passwords come from a file the user chooses: the view gives it to the core, says how many came
/// in and how many conflicted, and offers to delete the file, which holds them in plain text. The
/// view says when it finishes, never that it goes on without the view, as its end is the offer.
#[gpui::test]
fn a_passwords_file_comes_in_and_the_view_offers_to_delete_it(cx: &mut TestAppContext) {
    let browser = Rc::new(Browser::new(None));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, true));
    let events = Rc::new(RefCell::new(Vec::new()));
    let _events = cx.update(|cx| {
        let events = events.clone();
        cx.subscribe(&view, move |_, event: &ImportEvent, _| {
            events.borrow_mut().push(event.clone())
        })
    });
    // Cancelling the panel imports nothing.
    view.update(cx, |view, cx| view.choose_passwords(cx));
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(matches!(view.step, super::Step::List))
    });
    assert!(browser.importing.borrow().is_none());

    let file = write(home("passwords").join("Chrome Passwords.csv"), PASSWORDS);
    *browser.passwords_file.borrow_mut() = Some(file.clone());
    view.update(cx, |view, cx| view.choose_passwords(cx));
    cx.run_until_parked();
    let given = browser
        .importing
        .borrow()
        .as_ref()
        .map(|(file, _)| file.clone());
    assert_eq!(given, Some(file.clone()));
    view.read_with(cx, |view, _| {
        assert_eq!(view.running().as_deref(), Some("Importing passwords…"));
    });
    let passwords = Passwords::Imported {
        imported: 3,
        conflicted: 1,
        skipped: 0,
    };
    cx.update(|cx| browser.imported(passwords, cx));
    let said = "3 passwords came in from Chrome Passwords.csv. 1 conflicted with a saved password, \
                which was kept.";
    view.read_with(cx, |view, _| {
        let super::Step::Done(shown) = &view.step else {
            panic!()
        };
        assert_eq!(shown.as_ref(), said);
        assert_eq!(view.passwords_file.as_ref(), Some(&file));
    });
    // Its end offers to delete the file, so it never goes on without the view.
    assert_eq!(*events.borrow(), [ImportEvent::Finished(said.into())]);
    assert!(file.exists());
    view.update(cx, |view, cx| view.delete_passwords_file(cx));
    assert!(!file.exists());
    view.read_with(cx, |view, _| {
        let super::Step::Done(shown) = &view.step else {
            panic!()
        };
        assert_eq!(
            shown.as_ref(),
            format!("{said} Chrome Passwords.csv is deleted.")
        );
        assert_eq!(view.passwords_file, None);
    });
    view.update(cx, |view, cx| view.done(cx));
    view.read_with(cx, |view, _| {
        assert!(matches!(view.step, super::Step::List))
    });
    fs::remove_dir_all(file.parent().unwrap()).unwrap();
}

/// A file Chrome's importer can't take says why, and isn't offered for deleting, as the user may
/// want it again.
#[gpui::test]
fn a_file_that_isnt_a_passwords_export_says_so(cx: &mut TestAppContext) {
    let browser = Rc::new(Browser::new(None));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    let file = write(home("not-passwords").join("notes.csv"), "a,b\n1,2\n");
    *browser.passwords_file.borrow_mut() = Some(file.clone());
    view.update(cx, |view, cx| view.choose_passwords(cx));
    cx.run_until_parked();
    cx.update(|cx| browser.imported(Passwords::NotExport, cx));
    view.read_with(cx, |view, _| {
        let super::Step::Done(shown) = &view.step else {
            panic!()
        };
        assert_eq!(
            shown.as_ref(),
            "notes.csv isn't a passwords export: its first row needs url, username and password."
        );
        assert_eq!(view.passwords_file, None);
    });
    fs::remove_dir_all(file.parent().unwrap()).unwrap();
}

/// With more than one profile, the summary names the one they went to, and rows that weren't
/// logins are counted.
#[test]
fn the_passwords_summary_names_the_profile_and_what_it_skipped() {
    let passwords = Passwords::Imported {
        imported: 1,
        conflicted: 2,
        skipped: 1,
    };
    let said = super::passwords_summary(Path::new("/tmp/export.csv"), passwords, Some("Work"));
    assert_eq!(
        said.as_ref(),
        "1 password came in to Work from export.csv. 2 conflicted with saved passwords, which \
         were kept. 1 row wasn't a login it could read."
    );
    let none = Passwords::Imported {
        imported: 0,
        conflicted: 0,
        skipped: 0,
    };
    let said = super::passwords_summary(Path::new("export.csv"), none, None);
    assert_eq!(said.as_ref(), "No passwords came in from export.csv.");
}

/// An extension in a made-up profile's `Extensions` folder: its id, the manifest of its version,
/// and its messages in English, if any.
fn extension(profile: &Path, id: &str, manifest: &str, messages: Option<&str>) {
    let version = profile.join("Extensions").join(id).join("1.0_0");
    write(version.join("manifest.json"), manifest);
    if let Some(messages) = messages {
        write(version.join("_locales/en/messages.json"), messages);
    }
}

const STORE: &str = r#""update_url": "https://clients2.google.com/service/update2/crx""#;
const UBLOCK: &str = "cjpalhdlnbpafiamejdnhcphjbkeiagm";

/// Gives Chrome's default profile under `home` a pinned extension named by its messages, one not
/// pinned, uBlock Origin, a theme and one from outside the Web Store.
fn chrome_extensions(home: &Path) -> PathBuf {
    let profile = home.join(".config/google-chrome/Default");
    let pinned = r#"{"extensions": {"pinned_extensions": ["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]}}"#;
    write(profile.join("Preferences"), pinned);
    let named = format!(r#"{{"name": "__MSG_appName__", "default_locale": "en", {STORE}}}"#);
    let messages = r#"{"APPNAME": {"message": "Dark Reader"}}"#;
    extension(
        &profile,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        &named,
        Some(messages),
    );
    let plain = format!(r#"{{"name": "Bitwarden", {STORE}}}"#);
    extension(&profile, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", &plain, None);
    let blocker = format!(r#"{{"name": "uBlock Origin", {STORE}}}"#);
    extension(&profile, UBLOCK, &blocker, None);
    let theme = format!(r#"{{"name": "Ocean", "theme": {{}}, {STORE}}}"#);
    extension(&profile, "cccccccccccccccccccccccccccccccc", &theme, None);
    let own = r#"{"name": "Mine"}"#;
    extension(&profile, "dddddddddddddddddddddddddddddddd", own, None);
    profile
}

/// A profile's Web Store extensions come by their names, from their messages when the manifest
/// leaves it to them, with the pins of its `Preferences`; themes and extensions from elsewhere
/// don't, and an ad blocker is known as one.
#[test]
fn a_profiles_extensions_come_with_their_pins() {
    let home = home("extensions");
    let profile = chrome_extensions(&home);
    let read = extensions::read(&profile);
    let listed: Vec<_> = read
        .iter()
        .map(|e| (e.name.as_str(), e.pinned, e.blocks_ads()))
        .collect();
    assert_eq!(
        listed,
        [
            ("Bitwarden", false, false),
            ("Dark Reader", true, false),
            ("uBlock Origin", false, true)
        ]
    );
    assert_eq!(
        read[1].store_page(),
        "https://chromewebstore.google.com/detail/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    let named = |name: &str| Extension {
        id: "x".into(),
        name: name.into(),
        pinned: false,
    };
    assert!(named("AdGuard AdBlocker").blocks_ads() && named("Total Adblock").blocks_ads());
    assert!(!named("Grammarly").blocks_ads());
    fs::remove_dir_all(home).unwrap();
}

/// Chrome's extensions are listed after the import, uBlock Origin left off the list, as the
/// browser blocks ads itself; Add… takes one off and opens its Web Store page, and Add Anyway does
/// the same for the ad blocker. One that was pinned in Chrome is pinned once it comes in, once.
#[gpui::test]
fn chromes_extensions_are_listed_to_add_and_keep_their_pins(cx: &mut TestAppContext) {
    let home = chrome("chrome-extensions", &[("Default", "Me")], SystemTime::now());
    chrome_extensions(&home);
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Linux))));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    view.update(cx, |view, cx| {
        view.pick(0, cx);
        view.import(cx);
    });
    cx.run_until_parked();
    cx.update(|cx| browser.added(cx));
    view.read_with(cx, |view, _| {
        let super::Step::Done(said) = &view.step else {
            panic!("still importing")
        };
        assert_eq!(
            said.as_ref(),
            "1 Space, 3 pinned tabs, 1 open tab, 2 pages of history and 1 address came in \
             from Chrome, with 2 extensions to add from the Chrome Web Store."
        );
    });
    let ids = || -> Vec<String> {
        extensions::listed()
            .extensions
            .iter()
            .map(|each| each.id.clone())
            .collect()
    };
    assert_eq!(extensions::listed().from, "Chrome");
    assert_eq!(ids().len(), 3, "the ad blocker is kept to add anyway");
    let dark_reader = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    view.update(cx, |view, cx| view.add_extension(dark_reader, cx));
    view.update(cx, |view, cx| view.add_extension(UBLOCK, cx));
    assert_eq!(ids(), ["bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"]);
    let pages = browser.state.borrow().store_pages.clone();
    assert_eq!(
        pages,
        [
            format!("https://chromewebstore.google.com/detail/{dark_reader}"),
            format!("https://chromewebstore.google.com/detail/{UBLOCK}"),
        ]
    );
    // Dark Reader comes in, unpinned here: it's pinned, and only the once.
    let now = [(dark_reader, false), (UBLOCK, false)];
    assert_eq!(extensions::to_pin(&[], now), [dark_reader]);
    assert!(extensions::to_pin(&[], now).is_empty());
    fs::remove_dir_all(home).unwrap();
}

/// Choose folders lists the bar's own bookmarks by the bar's name and each top folder, and a
/// folder unticked stays out of the import.
#[gpui::test]
fn a_bookmark_folder_unticked_stays_out(cx: &mut TestAppContext) {
    let now = SystemTime::now();
    let home = chrome("chrome-folders", &[("Default", "Me")], now);
    let source = &super::importable(&Roots::under(&home, Os::Linux))[0];
    assert_eq!(
        read::folders(source, now).unwrap(),
        ["Bookmarks Bar", "Other bookmarks"]
    );
    let mut found = read::read(source, &[Kind::Bookmarks], now).unwrap();
    read::leave_out(&mut found, &["Bookmarks Bar".into()], source.browser.family);
    let nodes = node_shape(&found.spaces[0].pinned);
    assert_eq!(nodes, "Other bookmarks[Docs=https://docs.example.com/]");
    let mut found = read::read(source, &[Kind::Bookmarks], now).unwrap();
    let both = ["Bookmarks Bar".into(), "Other bookmarks".into()];
    read::leave_out(&mut found, &both, source.browser.family);
    assert!(
        found.spaces.is_empty(),
        "a Space with nothing left doesn't come"
    );

    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Linux))));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    view.update(cx, |view, cx| {
        view.pick(0, cx);
        view.choose_folders(cx);
    });
    cx.run_until_parked();
    view.update(cx, |view, cx| {
        let names = view.folders.names.clone();
        assert_eq!(
            names.as_deref(),
            Some(&["Bookmarks Bar".into(), "Other bookmarks".into()][..])
        );
        view.toggle_folder("Other bookmarks", cx);
        view.import(cx);
    });
    cx.run_until_parked();
    cx.update(|cx| browser.added(cx));
    assert_eq!(
        shape(&browser.spaces()[1].rows),
        "Mail=https://mail.example.com/, https://example.com/"
    );
    fs::remove_dir_all(home).unwrap();
}

/// The favorites offered are the sites visited most, once a site however its pages and `www.`
/// differ, at its root: not one visited once or twice, a favorite already, or the user's own
/// machine or network.
#[test]
fn the_sites_visited_most_are_offered_as_favorites() {
    let page = |url: &str, visits| Page {
        url: url.into(),
        title: String::new(),
        last_visit: 0,
        visits,
        typed: 0,
    };
    let history = [
        page("https://www.example.com/a", 2),
        page("https://news.example.org/today", 3),
        page("https://example.com/b?x=1", 2),
        page("http://localhost:3000/", 40),
        page("http://192.168.1.1/admin", 40),
        page("https://printer.local/", 40),
        page("https://mail.example.net/inbox", 9),
        page("https://rare.example.com/", 2),
        page("file:///Users/me/notes.html", 40),
    ];
    let skip = ["https://mail.example.net/".to_string()];
    let urls: Vec<String> = suggested::favorites(&history, &skip)
        .into_iter()
        .map(|tab| tab.url)
        .collect();
    assert_eq!(
        urls,
        ["https://www.example.com/", "https://news.example.org/"]
    );
    assert_eq!(
        suggested::site_name("https://www.example.com/"),
        "example.com"
    );
    let many: Vec<Page> = (0..12)
        .map(|n| page(&format!("https://site{n}.example/"), 3))
        .collect();
    assert_eq!(suggested::favorites(&many, &[]).len(), suggested::MOST);
}

/// An import offers the sites its history visited most once it's read them, and none when it
/// brought favorites of its own, as Arc's does.
#[gpui::test]
fn an_import_offers_favorites_from_its_history(cx: &mut TestAppContext) {
    let home = chrome("chrome-suggested", &[("Default", "Me")], SystemTime::now());
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Linux))));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    view.update(cx, |view, cx| {
        assert_eq!(view.suggested(), None);
        view.pick(0, cx);
        view.import(cx);
        assert_eq!(view.suggested(), None, "not until it's read the history");
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        let urls: Vec<&str> = view
            .suggested()
            .unwrap()
            .iter()
            .map(|t| t.url.as_str())
            .collect();
        assert_eq!(urls, ["https://example.com/"]);
    });
    fs::remove_dir_all(home).unwrap();

    let arc = Rc::new(Browser::new(None));
    let host: Rc<dyn Host> = arc.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    view.update(cx, |view, cx| {
        view.pick(0, cx);
        view.import(cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| assert_eq!(view.suggested(), Some(&[][..])));
}

/// Chrome's export: its bar marked as the toolbar folder, then Other bookmarks. A page of the
/// browser's own stays out, and the names and addresses read their character references.
const CHROME_BOOKMARKS_HTML: &str = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<!-- This is an automatically generated file.
     It will be read and overwritten.
     DO NOT EDIT! -->
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks</H1>
<DL><p>
    <DT><H3 ADD_DATE="1700000000" LAST_MODIFIED="1700000001" PERSONAL_TOOLBAR_FOLDER="true">Bookmarks bar</H3>
    <DL><p>
        <DT><A HREF="https://mail.example.com/" ADD_DATE="1700000000" ICON="data:image/png;base64,iVBORw0KGgo=">Mail</A>
        <DT><A HREF="chrome://settings/">Settings</A>
        <DT><H3 ADD_DATE="1700000000">Work</H3>
        <DL><p>
            <DT><A HREF="https://docs.example.com/?a=1&amp;b=2">Docs &amp; Sheets</A>
        </DL><p>
        <DT><H3>Empty</H3>
        <DL><p>
        </DL><p>
    </DL><p>
    <DT><H3>Other bookmarks</H3>
    <DL><p>
        <DT><A HREF="https://example.com/">Caf&#233; &#x2014; menu</A>
    </DL><p>
</DL><p>
"#;

/// Firefox's export: its menu's bookmarks outside any folder, under the file's heading, with a
/// separator and descriptions, then its toolbar and Other Bookmarks.
const FIREFOX_BOOKMARKS_HTML: &str = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'self'; script-src 'none'; img-src data: *; object-src 'none'"></meta>
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks Menu</H1>

<DL><p>
    <DT><A HREF="https://news.example.org/" ADD_DATE="1700000000">News</A>
    <DD>What's new
    <HR>
    <DT><A HREF="place:sort=8&maxResults=10">Most Visited</A>
    <DT><H3 ADD_DATE="1700000000" PERSONAL_TOOLBAR_FOLDER="true">Bookmarks Toolbar</H3>
    <DD>Add bookmarks to this folder to see them displayed on the Bookmarks Toolbar
    <DL><p>
        <DT><A HREF="https://wiki.example.org/" TAGS="ref">Wiki</A>
    </DL><p>
    <DT><H3 ADD_DATE="1700000000" UNFILED_BOOKMARKS_FOLDER="true">Other Bookmarks</H3>
    <DL><p>
        <DT><A HREF="https://shop.example.org/">Shop</A>
    </DL><p>
</DL>
"#;

#[test]
fn a_bookmarks_file_comes_in_as_its_browser_kept_it() {
    let chrome = super::html::bookmarks(CHROME_BOOKMARKS_HTML).unwrap();
    assert_eq!(
        node_shape(&chrome),
        "Mail=https://mail.example.com/, \
         Work[Docs & Sheets=https://docs.example.com/?a=1&b=2], \
         Other bookmarks[Café — menu=https://example.com/]"
    );
    let firefox = super::html::bookmarks(FIREFOX_BOOKMARKS_HTML).unwrap();
    assert_eq!(
        node_shape(&firefox),
        "Wiki=https://wiki.example.org/, Bookmarks Menu[News=https://news.example.org/], \
         Other Bookmarks[Shop=https://shop.example.org/]"
    );
}

/// Safari's export marks no bar: what's outside any folder is the bar, and every folder comes as
/// one, its Reading List too. Tags in any case, unquoted values and a file cut short read alike.
#[test]
fn a_bookmarks_file_without_a_bar_keeps_its_folders() {
    let safari = "\u{feff}<!doctype netscape-bookmark-file-1>\n<html><head><title>Bookmarks</title>\
        </head><body><h1>Bookmarks</h1><dl><dt><h3 FOLDED>Favourites</h3><dl>\
        <dt><a href=https://a.example/>A</a></dl><dt><a href='https://b.example/'>B</a>\
        <dt><h3 FOLDED id=\"com.apple.ReadingList\">Reading List</h3><dl><dt>\
        <a href=\"https://c.example/\">C\n   later</a>";
    assert_eq!(
        node_shape(&super::html::bookmarks(safari).unwrap()),
        "B=https://b.example/, Favourites[A=https://a.example/], \
         Reading List[C later=https://c.example/]"
    );
}

/// A bar of more than read::BAR_ROWS comes in as a folder of its name.
#[test]
fn a_long_bar_in_a_bookmarks_file_comes_as_a_folder() {
    let rows: String = (0..=read::BAR_ROWS)
        .map(|n| format!("<DT><A HREF=\"https://{n}.example/\">{n}</A>\n"))
        .collect();
    let html = format!(
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n<DL><p><DT><H3 PERSONAL_TOOLBAR_FOLDER=\"true\">\
         Bar</H3><DL><p>{rows}</DL><p></DL><p>"
    );
    let pinned = super::html::bookmarks(&html).unwrap();
    assert!(matches!(&pinned[..], [Node::Folder { name, children }]
        if name == "Bar" && children.len() == read::BAR_ROWS + 1));
}

#[test]
fn a_file_that_isnt_bookmarks_says_so() {
    let error = super::html::bookmarks("url,username,password\n").unwrap_err();
    assert_eq!(error, "it isn't a bookmarks file a browser exported");
    assert_eq!(
        super::html::bookmarks("<!DOCTYPE NETSCAPE-Bookmark-file-1>").unwrap(),
        []
    );
}

/// Bookmarks from a file come in as a Space named Bookmarks, in the profile this browser started
/// with, and a second time add nothing.
#[gpui::test]
fn a_bookmarks_file_comes_in_once(cx: &mut TestAppContext) {
    let browser = Rc::new(Browser::new(None));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    let said = |cx: &mut TestAppContext| {
        view.read_with(cx, |view, _| match &view.step {
            super::Step::Done(said) => said.to_string(),
            _ => panic!("still importing"),
        })
    };
    // Cancelling the panel imports nothing.
    view.update(cx, |view, cx| view.choose_bookmarks(cx));
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(matches!(view.step, super::Step::List))
    });

    let file = write(
        home("bookmarks-file").join("bookmarks.html"),
        CHROME_BOOKMARKS_HTML,
    );
    *browser.bookmarks_file.borrow_mut() = Some(file.clone());
    view.update(cx, |view, cx| view.choose_bookmarks(cx));
    cx.run_until_parked();
    assert_eq!(
        said(cx),
        "1 Space and 3 pinned tabs came in from bookmarks.html."
    );
    let space = browser
        .spaces()
        .into_iter()
        .find(|space| space.name == "Bookmarks")
        .unwrap();
    assert_eq!(space.profile, 1);
    assert_eq!(
        shape(&space.rows),
        "Mail=https://mail.example.com/, Work[Docs & Sheets=https://docs.example.com/?a=1&b=2], \
         Other bookmarks[Café — menu=https://example.com/]"
    );
    view.update(cx, |view, cx| view.refresh(cx));
    view.update(cx, |view, cx| view.choose_bookmarks(cx));
    cx.run_until_parked();
    assert_eq!(said(cx), "Everything from bookmarks.html is here already.");

    let other = write(file.with_file_name("notes.html"), "<html>notes</html>");
    *browser.bookmarks_file.borrow_mut() = Some(other);
    view.update(cx, |view, cx| view.refresh(cx));
    view.update(cx, |view, cx| view.choose_bookmarks(cx));
    cx.run_until_parked();
    assert_eq!(
        said(cx),
        "Couldn't import notes.html: it isn't a bookmarks file a browser exported."
    );
    fs::remove_dir_all(file.parent().unwrap()).unwrap();
}

/// On Windows a Chromium installed for the user has its exe beside its profiles, then its name for
/// the registry's App Paths; Chromium's own, whose name is Chrome's, is looked for beside its
/// profiles alone, and a browser with no exe the host can find, as the Store's Arc, has none.
#[test]
fn windows_knows_a_browsers_app_by_its_exe() {
    let source = |name: &str, dir: &str| sources::Source {
        browser: (sources::BROWSERS.iter())
            .find(|browser| browser.name == name)
            .unwrap(),
        dir: PathBuf::from(dir),
        install: None,
        profiles: Vec::new(),
        used: None,
    };
    let local = "C:/Users/me/AppData/Local";
    let ids = |name: &str, dir: &str| source(name, &format!("{local}/{dir}")).app_ids(Os::Windows);
    let beside = |dir: &str, exe: &str| {
        Path::new(&format!("{local}/{dir}"))
            .join("Application")
            .join(exe)
            .to_string_lossy()
            .into_owned()
    };
    assert_eq!(
        ids("Chrome", "Google/Chrome/User Data"),
        [beside("Google/Chrome", "chrome.exe"), "chrome.exe".into()]
    );
    assert_eq!(
        ids("Chromium", "Chromium/User Data"),
        [beside("Chromium", "chrome.exe")]
    );
    assert_eq!(
        ids("Firefox", "../Roaming/Mozilla/Firefox"),
        ["firefox.exe"]
    );
    assert!(
        ids(
            "Arc",
            "Packages/TheBrowserCompany.Arc_x/LocalCache/Local/Arc/User Data"
        )
        .is_empty()
    );
}

/// Each browser's row has its app's icon where the system has one, and the globe plate where it
/// hasn't, the names lining up either way; on Linux the app is known by its desktop entry, a
/// Flatpak's by its app id from where its profiles are.
#[gpui::test]
fn a_browser_with_no_app_icon_has_the_globe_plate_and_the_names_line_up(cx: &mut TestAppContext) {
    let home = chrome("chrome-icons", &[("Default", "Me")], SystemTime::now());
    let flatpak = chrome(
        "chrome-icons-flatpak",
        &[("Default", "Me")],
        SystemTime::now(),
    );
    let app = home.join(".var/app/com.google.Chrome/config");
    fs::create_dir_all(&app).unwrap();
    fs::rename(
        flatpak.join(".config/google-chrome"),
        app.join("google-chrome"),
    )
    .unwrap();
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Linux))));
    browser.state.borrow_mut().icons = vec!["Chrome".into()];
    let host: Rc<dyn Host> = browser.clone();
    let (view, cx) = cx.add_window_view(|_, _| ImportView::new(host, false).headed());
    cx.run_until_parked();
    let (names, icons, ids) = view.read_with(cx, |view, _| {
        let names: Vec<String> = view.sources.iter().map(|source| source.name()).collect();
        let icons: Vec<bool> = view.icons.iter().map(Option::is_some).collect();
        let ids: Vec<Vec<String>> = (view.sources.iter())
            .map(|source| source.app_ids(Os::Linux))
            .collect();
        (names, icons, ids)
    });
    let mut rows: Vec<(String, bool, Vec<String>)> = (names.into_iter().zip(icons).zip(ids))
        .map(|((name, icon), ids)| (name, icon, ids))
        .collect();
    rows.sort();
    let row = |name: &str, icon: bool, id: &str| (name.to_string(), icon, vec![id.to_string()]);
    assert_eq!(
        rows,
        [
            row("Chrome", true, "google-chrome"),
            row("Chrome (Flatpak)", false, "com.google.Chrome")
        ]
    );
    let lines = view.read_with(cx, |view, _| view.drawn_at.borrow().clone());
    let x = |name: &str| {
        let at = format!("browser: settings import-name {name} at ");
        (lines.iter())
            .find_map(|line| line.strip_prefix(&at))
            .and_then(|rest| rest.split(' ').next()?.parse::<f32>().ok())
            .unwrap_or_else(|| panic!("{name}'s name wasn't drawn: {lines:?}"))
    };
    assert_eq!(
        x("Chrome"),
        x("Chrome (Flatpak)"),
        "the names don't line up"
    );
    // Asked once each, at a plate's 28 points in the window's pixels.
    let scale = cx.update(|window, _| window.scale_factor());
    let pixels = (28. * scale).round() as u32;
    assert_eq!(browser.state.borrow().icon_pixels, [pixels, pixels]);
    fs::remove_dir_all(home).unwrap();
    fs::remove_dir_all(flatpak).unwrap();
}

/// Favorite tiles that fit on one row are centred under the view's centred heading and button, two
/// as much as one; with more rows the grid keeps its left edge, the last row's tiles under the
/// first's.
#[gpui::test]
fn one_row_of_tiles_is_centred_and_more_keep_their_left_edge(cx: &mut TestAppContext) {
    let made = chrome("chrome-tiles-row", &[("Default", "Me")], SystemTime::now());
    let browser = Rc::new(Browser::new(Some(Roots::under(&made, Os::Linux))));
    let host: Rc<dyn Host> = browser.clone();
    let (view, cx) = cx.add_window_view(|_, _| ImportView::new(host, false).headed());
    view.update(cx, |view, cx| {
        view.pick(0, cx);
        view.import(cx);
    });
    cx.run_until_parked();
    cx.update(|_, cx| browser.added(cx));
    // Offers `sites`, and says where the grid's middle and each tile's were drawn, across.
    let offer = |sites: &[&str], cx: &mut gpui::VisualTestContext| {
        let tabs = (sites.iter())
            .map(|site| Tab {
                url: format!("https://{site}/"),
                name: String::new(),
            })
            .collect();
        view.update(cx, |view, cx| {
            view.offer(tabs);
            cx.notify();
        });
        cx.run_until_parked();
        let lines = view.read_with(cx, |view, _| view.drawn_at.borrow().clone());
        let x = |what: &str| {
            let at = format!("browser: settings {what} at ");
            (lines.iter())
                .find_map(|line| line.strip_prefix(&at))
                .and_then(|rest| rest.split(' ').next()?.parse::<f32>().ok())
                .unwrap_or_else(|| panic!("{what} wasn't drawn: {lines:?}"))
        };
        let tiles: Vec<f32> = (sites.iter())
            .map(|site| x(&format!("import-favorite {site}")))
            .collect();
        (x("import-favorites"), tiles)
    };
    let (middle, two) = offer(&["one.example.org", "two.example.org"], cx);
    assert!(
        ((two[0] + two[1]) / 2. - middle).abs() <= 1.,
        "two tiles aren't centred on {middle}: {two:?}"
    );
    let (middle, one) = offer(&["lone.example.org"], cx);
    assert!(
        (one[0] - middle).abs() <= 1.,
        "one tile isn't centred on {middle}: {one:?}"
    );
    let five = [
        "a.example.org",
        "b.example.org",
        "c.example.org",
        "d.example.org",
        "e.example.org",
    ];
    let (middle, five) = offer(&five, cx);
    assert!(
        (five[4] - five[0]).abs() <= 1.,
        "the second row's tile isn't under the first's: {five:?}"
    );
    assert!(
        five[0] < middle && five[3] > middle,
        "a full row isn't across the grid: {five:?}"
    );
    fs::remove_dir_all(made).unwrap();
}

/// The favorites offered are tiles, all ticked at first: a click on one unticks it and the button
/// counts those left, "Add 2 Favorites", then "Add 1 Favorite", and with none, "Add Favorites",
/// dimmed; Add adds the ticked ones alone.
#[gpui::test]
fn favorite_tiles_tick_and_untick_at_a_click_and_the_button_counts_them(cx: &mut TestAppContext) {
    let made = chrome("chrome-tiles", &[("Default", "Me")], SystemTime::now());
    let browser = Rc::new(Browser::new(Some(Roots::under(&made, Os::Linux))));
    let host: Rc<dyn Host> = browser.clone();
    let (view, cx) = cx.add_window_view(|_, _| ImportView::new(host, false).headed());
    view.update(cx, |view, cx| {
        view.pick(0, cx);
        view.import(cx);
    });
    cx.run_until_parked();
    cx.update(|_, cx| browser.added(cx));
    let tab = |url: &str| Tab {
        url: url.into(),
        name: String::new(),
    };
    let offered = [
        "https://news.example.org/",
        "https://mail.example.org/",
        "https://docs.example.com/",
    ];
    view.update(cx, |view, cx| {
        view.offer(offered.iter().map(|url| tab(url)).collect());
        cx.notify();
    });
    cx.run_until_parked();
    let title = |cx: &mut gpui::VisualTestContext| {
        view.read_with(cx, |view, _| view.add_favorites_title().to_string())
    };
    assert_eq!(title(cx), "Add 3 Favorites");
    // Where each tile was drawn, as check.sh reads it from the log.
    let at = |name: &str, cx: &mut gpui::VisualTestContext| {
        let lines = view.read_with(cx, |view, _| view.drawn_at.borrow().clone());
        let prefix = format!("browser: settings import-favorite {name} at ");
        let line = (lines.iter())
            .find_map(|line| line.strip_prefix(&prefix).map(str::to_string))
            .unwrap_or_else(|| panic!("no tile for {name} drawn: {lines:?}"));
        let (x, y) = line.split_once(' ').unwrap();
        gpui::point(gpui::px(x.parse().unwrap()), gpui::px(y.parse().unwrap()))
    };
    let click = |name: &str, cx: &mut gpui::VisualTestContext| {
        let point = at(name, cx);
        cx.simulate_click(point, gpui::Modifiers::none());
        cx.run_until_parked();
    };
    click("mail.example.org", cx);
    assert_eq!(title(cx), "Add 2 Favorites");
    click("news.example.org", cx);
    assert_eq!(title(cx), "Add 1 Favorite");
    click("docs.example.com", cx);
    assert_eq!(title(cx), "Add Favorites");
    click("mail.example.org", cx);
    let ticked = view.read_with(cx, |view, _| view.ticked());
    let urls: Vec<&str> = ticked.iter().map(|tab| tab.url.as_str()).collect();
    assert_eq!(urls, ["https://mail.example.org/"]);
    view.update(cx, |view, cx| view.add_ticked(cx));
    let favorites: Vec<String> = (browser.state.borrow().favorites.iter())
        .map(|(_, tab)| tab.url.clone())
        .collect();
    assert_eq!(favorites, ["https://mail.example.org/"]);
    fs::remove_dir_all(made).unwrap();
}

/// The end of an import from Settings offers the favorites it worked out from the history, each
/// ticked; a click unticks one, Add Favorites adds those ticked to the first Space and says so, and
/// the offer ends. A passwords file's end, after, offers none.
#[gpui::test]
fn the_end_of_an_import_offers_its_favorites(cx: &mut TestAppContext) {
    let made = chrome("chrome-offer", &[("Default", "Me")], SystemTime::now());
    let browser = Rc::new(Browser::new(Some(Roots::under(&made, Os::Linux))));
    let host: Rc<dyn Host> = browser.clone();
    let view = cx.new(|_| ImportView::new(host, false));
    let import = |cx: &mut TestAppContext| {
        view.update(cx, |view, cx| {
            view.pick(0, cx);
            view.import(cx);
        });
        cx.run_until_parked();
        cx.update(|cx| browser.added(cx));
    };
    let ticked = |cx: &mut TestAppContext| {
        view.read_with(cx, |view, _| {
            view.ticked()
                .into_iter()
                .map(|tab| tab.url)
                .collect::<Vec<_>>()
        })
    };
    let said = |cx: &mut TestAppContext| {
        view.read_with(cx, |view, _| match &view.step {
            super::Step::Done(said) => said.to_string(),
            _ => panic!("still importing"),
        })
    };
    import(cx);
    assert_eq!(ticked(cx), ["https://example.com/"]);
    view.update(cx, |view, cx| {
        view.toggle_favorite("https://example.com/", cx)
    });
    assert!(ticked(cx).is_empty());
    view.update(cx, |view, cx| view.add_ticked(cx));
    assert!(
        browser.state.borrow().favorites.is_empty(),
        "none ticked, none added"
    );
    view.update(cx, |view, cx| {
        view.toggle_favorite("https://example.com/", cx)
    });
    view.update(cx, |view, cx| view.add_ticked(cx));
    let favorites: Vec<(u64, String)> = (browser.state.borrow().favorites.iter())
        .map(|(profile, tab)| (*profile, tab.url.clone()))
        .collect();
    assert_eq!(favorites, [(1, "https://example.com/".to_string())]);
    assert_eq!(
        said(cx),
        "1 Space, 3 pinned tabs, 1 open tab, 2 pages of history and 1 address came in from \
         Chrome. 1 favorite added."
    );
    view.read_with(cx, |view, _| assert_eq!(view.suggested(), Some(&[][..])));

    // An offer left with Done isn't made again at a passwords file's end.
    view.update(cx, |view, cx| view.done(cx));
    browser.state.borrow_mut().favorites.clear();
    import(cx);
    assert_eq!(ticked(cx), ["https://example.com/"]);
    view.update(cx, |view, cx| view.done(cx));
    let file = write(
        home("offer-passwords").join("Chrome Passwords.csv"),
        PASSWORDS,
    );
    *browser.passwords_file.borrow_mut() = Some(file.clone());
    view.update(cx, |view, cx| view.choose_passwords(cx));
    cx.run_until_parked();
    let passwords = Passwords::Imported {
        imported: 3,
        conflicted: 0,
        skipped: 0,
    };
    cx.update(|cx| browser.imported(passwords, cx));
    view.read_with(cx, |view, _| assert_eq!(view.suggested(), None));
    assert!(ticked(cx).is_empty());
    fs::remove_dir_all(made).unwrap();
    fs::remove_dir_all(file.parent().unwrap()).unwrap();
}

/// A browser's row names its profiles only when it has several: Chrome's one, "Me", says nothing
/// its name doesn't, and two are each named.
#[gpui::test]
fn a_browser_names_its_profiles_only_when_it_has_several(cx: &mut TestAppContext) {
    let now = SystemTime::now();
    for (name, profiles, said) in [
        ("chrome-one-profile", &[("Default", "Me")][..], ""),
        (
            "chrome-two-profiles",
            &[("Default", "Me"), ("Profile 1", "Work")][..],
            "Me, Work",
        ),
    ] {
        let home = chrome(name, profiles, now);
        let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Linux))));
        let host: Rc<dyn Host> = browser.clone();
        let view = cx.new(|_| ImportView::new(host, false));
        view.read_with(cx, |view, _| {
            let mut line: Vec<String> = (view.row_line(&view.sources[0]).split(", "))
                .filter(|part| !part.is_empty())
                .map(str::to_string)
                .collect();
            line.sort();
            assert_eq!(line.join(", "), said, "{name}");
        });
        fs::remove_dir_all(home).unwrap();
    }
}

/// Done says what came in as one row, the browser over its counts, and comes at the foot, under
/// the extensions left to add, as what's left to do comes before it.
#[gpui::test]
fn done_comes_at_the_foot_after_the_extensions_to_add(cx: &mut TestAppContext) {
    let home = chrome("chrome-done-foot", &[("Default", "Me")], SystemTime::now());
    chrome_extensions(&home);
    let browser = Rc::new(Browser::new(Some(Roots::under(&home, Os::Linux))));
    let host: Rc<dyn Host> = browser.clone();
    let (view, cx) = cx.add_window_view(|_, _| ImportView::new(host, false).headed());
    view.update(cx, |view, cx| {
        view.pick(0, cx);
        view.import(cx);
    });
    cx.run_until_parked();
    cx.update(|_, cx| browser.added(cx));
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let came = view.read_with(cx, |view, _| view.came().cloned());
    assert_eq!(
        came,
        Some(super::Came {
            title: "Chrome came in".into(),
            counts: "1 Space · 3 pinned tabs · 1 open tab · 2 pages of history · 1 address".into(),
            more: None,
        })
    );
    let lines = view.read_with(cx, |view, _| view.drawn_at.borrow().clone());
    let y = |what: &str| {
        let at = format!("browser: settings {what} at ");
        (lines.iter())
            .find_map(|line| line.strip_prefix(&at))
            .and_then(|rest| rest.split(' ').nth(1)?.parse::<f32>().ok())
            .unwrap_or_else(|| panic!("{what} wasn't drawn: {lines:?}"))
    };
    let extension = y("import-add-extension aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    assert!(
        y("import-done") > extension,
        "Done was drawn above the extensions to add"
    );
    fs::remove_dir_all(home).unwrap();
}

/// Every control of the import has a role and a name, on its list, the choices for the browser
/// picked and the end with its favorites, and ⇥ stops only at those (#1911): a browser's button
/// says which browser, the kinds are switches by their names, the favorites boxes to tick by
/// their sites, and Add Favorites says it's off with none ticked.
#[gpui::test]
fn every_control_in_the_import_has_a_name(cx: &mut TestAppContext) {
    use crate::settings_rows::spoken::{Said, pressable, stops, unnamed};
    use gpui::accesskit::{Role, Toggled};
    let made = chrome("chrome-named", &[("Default", "Me")], SystemTime::now());
    let browser = Rc::new(Browser::new(Some(Roots::under(&made, Os::Linux))));
    let host: Rc<dyn Host> = browser.clone();
    let (view, cx) = cx.add_window_view(|_, _| ImportView::new(host, false).headed());
    cx.run_until_parked();
    let named = |cx: &mut gpui::VisualTestContext| -> Vec<Said> {
        let pressable = pressable(cx);
        assert_eq!(
            unnamed(&pressable),
            [],
            "a control without a name: {pressable:?}"
        );
        let stops = stops(cx);
        assert_eq!(unnamed(&stops), [], "a stop without a name: {stops:?}");
        pressable
    };
    let names =
        |said: &[Said]| -> Vec<String> { said.iter().map(|(_, name, _)| name.clone()).collect() };
    let source = view.read_with(cx, |view, _| view.sources[0].name());
    let list = names(&named(cx));
    for name in [
        format!("Import from {source}…"),
        "Import a Bookmarks File…".into(),
        "Import a Passwords File…".into(),
    ] {
        assert!(list.contains(&name), "{name} in {list:?}");
    }

    view.update(cx, |view, cx| view.pick(0, cx));
    cx.run_until_parked();
    let picking = named(cx);
    for name in ["Import", "Back", "Choose Passwords File…"] {
        assert!(
            names(&picking).contains(&name.into()),
            "{name} in {picking:?}"
        );
    }
    assert!(
        (picking.iter()).any(|(role, _, _)| *role == Role::Switch),
        "no kind's switch: {picking:?}"
    );

    view.update(cx, |view, cx| view.import(cx));
    cx.run_until_parked();
    cx.update(|_, cx| browser.added(cx));
    let tab = |url: &str| Tab {
        url: url.into(),
        name: String::new(),
    };
    view.update(cx, |view, cx| {
        view.offer(vec![
            tab("https://news.example.org/"),
            tab("https://mail.example.org/"),
        ]);
        cx.notify();
    });
    cx.run_until_parked();
    let end = named(cx);
    for name in ["Add 2 Favorites", "Done"] {
        assert!(names(&end).contains(&name.into()), "{name} in {end:?}");
    }
    let ticks = |cx: &mut gpui::VisualTestContext| {
        let tree = cx.a11y_tree().expect("a tree");
        let mut ticks: Vec<(String, Option<Toggled>)> = (tree.nodes.iter())
            .filter(|(_, node)| node.role() == Role::CheckBox)
            .map(|(_, node)| (node.label().unwrap_or_default().to_string(), node.toggled()))
            .collect();
        ticks.sort_by(|a, b| a.0.cmp(&b.0));
        ticks
    };
    assert_eq!(
        ticks(cx),
        [
            ("mail.example.org".into(), Some(Toggled::True)),
            ("news.example.org".into(), Some(Toggled::True)),
        ]
    );
    view.update(cx, |view, cx| {
        view.toggle_favorite("https://news.example.org/", cx);
        view.toggle_favorite("https://mail.example.org/", cx);
    });
    cx.run_until_parked();
    pressable(cx);
    assert!(ticks(cx).iter().all(|(_, on)| *on == Some(Toggled::False)));
    let tree = cx.a11y_tree().expect("a tree");
    let add = (tree.nodes.iter()).find(|(_, node)| node.label() == Some("Add Favorites"));
    assert_eq!(
        add.map(|(_, node)| node.is_disabled()),
        Some(true),
        "Add Favorites with none ticked doesn't say it's off"
    );
    fs::remove_dir_all(made).unwrap();
}
