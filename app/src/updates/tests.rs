//! Settings › About's version row, driven as the user does, on a host that records what it asks
//! of the updater, while the test says what the updater would.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{App, Entity, Modifiers, TestAppContext, VisualTestContext};

use super::*;

/// The updater, recording what the row asks of it.
#[derive(Default)]
struct Recorder {
    asked: RefCell<Vec<&'static str>>,
}

impl Host for Recorder {
    fn check(&self, _: &mut App) {
        self.asked.borrow_mut().push("check");
    }

    fn show(&self, _: &mut App) {
        self.asked.borrow_mut().push("show");
    }

    fn restart(&self, _: &mut App) {
        self.asked.borrow_mut().push("restart");
    }

    fn follow(&self, track: Track, _: &mut App) {
        self.asked.borrow_mut().push(match track {
            Track::Release => "follow releases",
            Track::Nightly => "follow nightlies",
        });
    }
}

struct Shown<'a> {
    row: Entity<VersionRow>,
    cx: &'a mut VisualTestContext,
    host: Rc<Recorder>,
}

/// About, drawn for a copy `updater` updates.
fn shown(updater: Updater, cx: &mut TestAppContext) -> Shown<'_> {
    let host = Rc::new(Recorder::default());
    let asked = host.clone();
    cx.update(|cx| start(updater, asked, cx));
    let (row, cx) = cx.add_window_view(|_, cx| VersionRow::new("Version 0.1.0", cx));
    cx.run_until_parked();
    Shown { row, cx, host }
}

impl Shown<'_> {
    /// The updater says the update is at `stage`.
    fn says(&mut self, stage: Stage) {
        self.cx.update(|_, cx| changed(stage, cx));
        self.cx.run_until_parked();
    }

    /// What the row says, and its button's title, as drawn.
    fn reads(&mut self) -> (Option<String>, Option<&'static str>) {
        let drawn = self
            .row
            .read_with(self.cx, |row, _| row.button_bounds().is_some());
        let (said, action) = self.cx.update(|_, cx| words(updater(cx), &stage(cx)));
        assert_eq!(
            drawn,
            action.is_some(),
            "the button drawn isn't the one offered"
        );
        (
            said.map(|said| said.to_string()),
            action.map(|(_, title)| title),
        )
    }

    /// Clicks the row's button, which must be drawn.
    fn click(&mut self) {
        let bounds = self.row.read_with(self.cx, |row, _| row.button_bounds());
        let bounds = bounds.expect("no button drawn");
        self.cx.simulate_click(bounds.center(), Modifiers::none());
        self.cx.run_until_parked();
    }

    fn asked(&self) -> Vec<&'static str> {
        self.host.asked.take()
    }
}

/// Before the updater has said anything, the row offers the check, which asks the updater, and
/// says nothing until the updater does.
#[gpui::test]
fn the_button_asks_the_updater_to_check(cx: &mut TestAppContext) {
    let mut about = shown(Updater::Own, cx);
    assert_eq!(about.reads(), (None, Some("Check for Updates")));
    about.click();
    assert_eq!(about.asked(), ["check"]);
}

/// While the updater looks, the row says so, with nothing to press.
#[gpui::test]
fn checking_says_so(cx: &mut TestAppContext) {
    let mut about = shown(Updater::Own, cx);
    about.click();
    about.says(Stage::Checking);
    assert_eq!(about.reads(), (Some("Checking…".into()), None));
    assert_eq!(about.asked(), ["check"]);
}

/// A check that finds nothing says so, and the button can look again.
#[gpui::test]
fn up_to_date_says_so_and_checks_again(cx: &mut TestAppContext) {
    let mut about = shown(Updater::Own, cx);
    about.says(Stage::Checking);
    about.says(Stage::Current);
    let up_to_date = concat!(env!("APP_NAME"), " is up to date");
    assert_eq!(
        about.reads(),
        (Some(up_to_date.into()), Some("Check for Updates"))
    );
    about.click();
    assert_eq!(about.asked(), ["check"]);
}

/// A download names its version, where the updater names it, with nothing to press.
#[gpui::test]
fn downloading_names_the_version(cx: &mut TestAppContext) {
    let mut about = shown(Updater::Own, cx);
    about.says(Stage::Checking);
    about.says(Stage::Downloading(Some("0.1.1".into())));
    assert_eq!(about.reads(), (Some("Downloading 0.1.1…".into()), None));
    // Flatpak's portal doesn't name it.
    about.says(Stage::Downloading(None));
    assert_eq!(
        about.reads(),
        (Some("Downloading the update…".into()), None)
    );
    assert!(about.asked().is_empty());
}

/// An update the updater won't download by itself, as Sparkle leaves one for the user to say,
/// offers Show Update, which asks the updater to show it in its own window. A failure after it is
/// the update's.
#[gpui::test]
fn an_update_the_updater_waits_on_offers_its_window(cx: &mut TestAppContext) {
    let mut about = shown(Updater::Own, cx);
    about.says(Stage::Found(Some("0.1.5".into())));
    about.says(Stage::Available(Some("0.1.5".into())));
    let available = concat!(env!("APP_NAME"), " 0.1.5 is available");
    assert_eq!(about.reads(), (Some(available.into()), Some("Show Update")));
    about.click();
    assert_eq!(about.asked(), ["show"]);
    about.says(Stage::Failed { found: false });
    assert_eq!(
        about.reads(),
        (Some("The update didn’t install".into()), Some("Try Again"))
    );
}

/// A ready update offers Restart to Update, which asks the updater to install it now, as do the
/// app menu's and the command bar's Check for Updates…, which say so too. It stays ready whatever
/// a check says after.
#[gpui::test]
fn ready_restarts_into_the_update(cx: &mut TestAppContext) {
    let mut about = shown(Updater::Own, cx);
    about.says(Stage::Downloading(Some("0.1.1".into())));
    about.says(Stage::Ready(Some("0.1.1".into())));
    let ready = concat!(env!("APP_NAME"), " 0.1.1 is ready");
    assert_eq!(
        about.reads(),
        (Some(ready.into()), Some("Restart to Update"))
    );
    assert_eq!(
        about.cx.update(|_, cx| command_title(cx)),
        "Restart to Update"
    );
    about.click();
    assert_eq!(about.asked(), ["restart"]);
    about.says(Stage::Checking);
    about.says(Stage::Current);
    assert_eq!(about.reads().1, Some("Restart to Update"));
    // An install that was cancelled, such as by a page that kept the quit, can be restarted again.
    about.click();
    assert_eq!(about.asked(), ["restart"]);
}

/// A copy from the Microsoft Store says the Store updates it, with no button.
#[gpui::test]
fn a_store_copy_says_the_store_updates_it(cx: &mut TestAppContext) {
    let mut about = shown(Updater::Store, cx);
    let store = concat!("The Microsoft Store updates ", env!("APP_NAME"));
    assert_eq!(about.reads(), (Some(store.into()), None));
    about.cx.update(|_, cx| act(Action::Check, cx));
    assert!(about.asked().is_empty());
}

/// The native Linux install says its line updates it, with no button, as nothing in ilo can.
#[gpui::test]
fn an_install_sh_copy_says_to_run_it_again(cx: &mut TestAppContext) {
    let mut about = shown(Updater::Script, cx);
    assert_eq!(
        about.reads(),
        (Some("Run install.sh again to update".into()), None)
    );
    about.cx.update(|_, cx| act(Action::Check, cx));
    assert!(about.asked().is_empty());
}

/// A dev build has no updater, so the row is the version alone, as it was.
#[gpui::test]
fn a_dev_build_has_no_button(cx: &mut TestAppContext) {
    let mut about = shown(Updater::None, cx);
    assert_eq!(about.reads(), (None, None));
    about.cx.update(|_, cx| act(Action::Check, cx));
    assert!(about.asked().is_empty());
    assert_eq!(
        about.cx.update(|_, cx| command_title(cx)),
        "Check for Updates…"
    );
}

/// A launch that's older than the update the last quit started installing is the copy it was to
/// replace, so the install didn't go in: the row says so at once, with Try Again, which checks
/// again (#1459). A launch that's the update, or newer, says nothing of it.
#[gpui::test]
fn a_launch_older_than_the_update_installed_says_it_didn_t_install(cx: &mut TestAppContext) {
    assert!(went_in(1572, 1572), "the update's own build");
    assert!(
        went_in(1572, 1580),
        "a newer build, as one installed by hand"
    );
    assert!(!went_in(1572, 1571), "the build it was to replace");
    let mut about = shown(Updater::Own, cx);
    about.cx.update(|_, cx| didnt_install(cx));
    about.cx.run_until_parked();
    assert_eq!(
        about.reads(),
        (Some("The update didn’t install".into()), Some("Try Again"))
    );
    about.click();
    assert_eq!(about.asked(), ["check"]);
    about.says(Stage::Checking);
    assert_eq!(about.reads(), (Some("Checking…".into()), None));
}

/// On Linux, a profile's launches, each of a version: the first launch of a newer one opens its
/// changelog entry, once, and a new profile's first launch and a downgrade open nothing (#1495).
#[test]
fn only_the_first_launch_of_a_newer_version_opens_its_notes() {
    let dir = std::env::temp_dir().join(format!("ilo-last-version-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("last-version");
    let launch = |version: &str| first_launch_notes(&file, version);
    let notes = |version: &str| Some(format!("{}#{version}", crate::build_info::WHATS_NEW));
    assert_eq!(launch("0.1.9"), None, "a new profile's first launch");
    assert_eq!(launch("0.1.9"), None, "the same version again");
    assert_eq!(
        launch("0.1.10"),
        notes("0.1.10"),
        "an update, numbered past 0.1.9"
    );
    assert_eq!(launch("0.1.10"), None, "the update's next launch");
    assert_eq!(launch("0.1.9"), None, "a downgrade");
    assert_eq!(
        launch("0.2.0"),
        notes("0.2.0"),
        "an update after the downgrade"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The ways back: a failed check says so and tries again; a failed download or install says the
/// update didn't install; and when the user closes the updater's own window, the row offers the
/// check again.
#[gpui::test]
fn a_failure_or_a_cancel_offers_the_check_again(cx: &mut TestAppContext) {
    let mut about = shown(Updater::Own, cx);
    about.says(Stage::Checking);
    about.says(Stage::Failed { found: true });
    assert_eq!(
        about.reads(),
        (Some("Couldn’t check for updates".into()), Some("Try Again"))
    );
    about.click();
    assert_eq!(about.asked(), ["check"]);
    about.says(Stage::Downloading(Some("0.1.1".into())));
    about.says(Stage::Failed { found: false });
    assert_eq!(
        about.reads(),
        (Some("The update didn’t install".into()), Some("Try Again"))
    );
    about.says(Stage::Found(None));
    assert_eq!(about.reads(), (Some("An update is available".into()), None));
    about.says(Stage::Idle);
    assert_eq!(about.reads(), (None, Some("Check for Updates")));
}

/// About's nightly row and the version row above it, drawn for a copy `updater` updates, which is
/// one of `built`'s builds and takes `followed`'s.
struct Nightly<'a> {
    row: Entity<NightlyRow>,
    version: Entity<VersionRow>,
    cx: &'a mut VisualTestContext,
    host: Rc<Recorder>,
}

fn nightly(
    updater: Updater,
    built: Track,
    followed: Option<Track>,
    cx: &mut TestAppContext,
) -> Nightly<'_> {
    let host = Rc::new(Recorder::default());
    let asked = host.clone();
    cx.update(|cx| {
        start(updater, asked, cx);
        tracks(built, followed, cx);
    });
    let (row, cx) = cx.add_window_view(|_, cx| NightlyRow::new(cx));
    let version = cx.new(|cx| VersionRow::new("Version 0.2.0 (1688)", cx));
    cx.run_until_parked();
    Nightly {
        row,
        version,
        cx,
        host,
    }
}

impl Nightly<'_> {
    /// What the row says under its name, and whether its switch is on, if the row is drawn.
    fn reads(&mut self) -> Option<(&'static str, bool)> {
        let drawn = self
            .row
            .read_with(self.cx, |row, _| row.switch_bounds().is_some());
        let shown = self
            .cx
            .update(|_, cx| followed(cx).map(|followed| (built(cx), followed)));
        assert_eq!(
            drawn,
            shown.is_some(),
            "the switch drawn isn't the row offered"
        );
        shown.map(|(built, followed)| (nightly_words(built, followed), followed == Track::Nightly))
    }

    /// Clicks the switch, which must be drawn.
    fn click(&mut self) {
        let bounds = self.row.read_with(self.cx, |row, _| row.switch_bounds());
        let bounds = bounds.expect("no switch drawn");
        self.cx.simulate_click(bounds.center(), Modifiers::none());
        self.cx.run_until_parked();
    }

    fn says(&mut self, stage: Stage) {
        self.cx.update(|_, cx| changed(stage, cx));
        self.cx.run_until_parked();
    }

    /// What the version row says, and its button's title.
    fn version_reads(&mut self) -> (Option<String>, Option<&'static str>) {
        _ = &self.version;
        let (said, action) = self.cx.update(|_, cx| words(updater(cx), &stage(cx)));
        (
            said.map(|said| said.to_string()),
            action.map(|(_, title)| title),
        )
    }

    fn asked(&self) -> Vec<&'static str> {
        self.host.asked.take()
    }
}

const NIGHTLY_LINE: &str = concat!(
    "Each night’s build of ",
    env!("APP_NAME"),
    ", ahead of the next release."
);
const KEEPS_LINE: &str = concat!(
    env!("APP_NAME"),
    " keeps this build until a release is newer."
);

/// On a release, the switch starts off and says what a nightly is. Turned on, the updater reads
/// the nightly feed and checks it at once; turned off again, it reads the releases' and checks
/// nothing, as the release running is the newest it would take.
#[gpui::test]
fn the_switch_turns_nightly_builds_on_and_off(cx: &mut TestAppContext) {
    let mut about = nightly(Updater::Own, Track::Release, Some(Track::Release), cx);
    assert_eq!(about.reads(), Some((NIGHTLY_LINE, false)));
    about.click();
    assert_eq!(about.asked(), ["follow nightlies", "check"]);
    assert_eq!(about.reads(), Some((NIGHTLY_LINE, true)));
    about.click();
    assert_eq!(about.asked(), ["follow releases"]);
    assert_eq!(about.reads(), Some((NIGHTLY_LINE, false)));
}

/// A nightly starts on. Turned off, it never goes back to an older release: the updater reads the
/// releases' feed, which it installs only a build numbered above this one from, so the row says
/// this build stays until a release is newer, and nothing is installed meanwhile. Turned on again,
/// it checks the nightly feed at once.
#[gpui::test]
fn a_nightly_turned_off_keeps_its_build(cx: &mut TestAppContext) {
    let mut about = nightly(Updater::Own, Track::Nightly, Some(Track::Nightly), cx);
    assert_eq!(about.reads(), Some((NIGHTLY_LINE, true)));
    about.click();
    assert_eq!(about.asked(), ["follow releases"]);
    assert_eq!(about.reads(), Some((KEEPS_LINE, false)));
    // The release feed's newest is older than this nightly, so the updater has nothing to offer.
    about.says(Stage::Checking);
    about.says(Stage::Current);
    let up_to_date = concat!(env!("APP_NAME"), " is up to date");
    assert_eq!(
        about.version_reads(),
        (Some(up_to_date.into()), Some("Check for Updates"))
    );
    assert!(about.asked().is_empty());
    about.click();
    assert_eq!(about.asked(), ["follow nightlies", "check"]);
    assert_eq!(about.reads(), Some((NIGHTLY_LINE, true)));
}

/// The choice is the user's, kept across launches by the host: a nightly whose user turned it off
/// starts off, saying it keeps its build.
#[gpui::test]
fn a_nightly_turned_off_before_starts_off(cx: &mut TestAppContext) {
    let mut about = nightly(Updater::Own, Track::Nightly, Some(Track::Release), cx);
    assert_eq!(about.reads(), Some((KEEPS_LINE, false)));
    assert!(about.asked().is_empty());
}

/// There's no row where nothing would follow the choice: a build with no nightly feed, a copy the
/// Microsoft Store updates, or a dev build. Asked to follow nightlies there, nothing is asked of
/// the updater.
#[gpui::test]
fn only_a_copy_with_a_nightly_feed_offers_it(_: &mut TestAppContext) {
    let cases = [
        (Updater::Own, None),
        (Updater::Store, Some(Track::Release)),
        (Updater::Script, Some(Track::Release)),
        (Updater::None, Some(Track::Release)),
    ];
    for (updater, followed) in cases {
        let mut cx = TestAppContext::single();
        let mut about = nightly(updater, Track::Release, followed, &mut cx);
        assert_eq!(
            about.reads(),
            None,
            "{updater:?} with {followed:?} offers nightly builds"
        );
        about.cx.update(|_, cx| follow(Track::Nightly, cx));
        assert!(about.asked().is_empty());
    }
}

/// With Settings › About's Show what's new off, an update's first launch opens no notes, and on
/// again it opens them.
#[test]
fn with_whats_new_off_an_update_opens_no_notes() {
    let dir = std::env::temp_dir().join(format!("ilo-whats-new-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let notes = || Some(format!("{}#0.1.10", crate::build_info::WHATS_NEW));
    assert_eq!(notes_to_open(notes(), &dir), notes(), "on, the default");
    std::fs::write(dir.join(NO_WHATS_NEW), "").unwrap();
    assert_eq!(notes_to_open(notes(), &dir), None, "off");
    assert_eq!(notes_to_open(None, &dir), None);
    std::fs::remove_file(dir.join(NO_WHATS_NEW)).unwrap();
    assert_eq!(notes_to_open(notes(), &dir), notes(), "on again");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The version row's button is a button an assistive app calls by what it does, at each stage
/// that offers one, and ⇥ stops at it (#1911): Try Again says it checks again.
#[gpui::test]
fn the_version_rows_button_says_what_it_does(cx: &mut TestAppContext) {
    use crate::settings_rows::spoken::{pressable, stops, unnamed};
    use gpui::accesskit::Role;
    let mut about = shown(Updater::Own, cx);
    for (stage, said) in [
        (Stage::Idle, "Check for Updates"),
        (Stage::Failed { found: true }, "Check for Updates Again"),
        (Stage::Ready(Some("0.1.1".into())), "Restart to Update"),
    ] {
        about.says(stage);
        let pressable = pressable(about.cx);
        assert_eq!(unnamed(&pressable), [], "{pressable:?}");
        assert_eq!(
            pressable,
            [(Role::Button, said.to_string(), None)],
            "{said}"
        );
        let stops = stops(about.cx);
        assert_eq!(stops, [(Role::Button, said.to_string(), None)], "{said}");
    }
}

/// Get nightly builds is a switch an assistive app reads by its row's name, on or off (#1911).
#[gpui::test]
fn the_nightly_switch_is_named(cx: &mut TestAppContext) {
    use crate::settings_rows::spoken::{pressable, unnamed};
    use gpui::accesskit::{Role, Toggled};
    let mut about = nightly(Updater::Own, Track::Release, Some(Track::Release), cx);
    let switch = |about: &mut Nightly| {
        let pressable = pressable(about.cx);
        assert_eq!(unnamed(&pressable), [], "{pressable:?}");
        let tree = about.cx.a11y_tree().expect("a tree");
        (tree.nodes.iter())
            .find(|(_, node)| node.role() == Role::Switch)
            .map(|(_, node)| (node.label().unwrap_or_default().to_string(), node.toggled()))
    };
    assert_eq!(
        switch(&mut about),
        Some(("Get nightly builds".into(), Some(Toggled::False)))
    );
    about.click();
    assert_eq!(about.asked(), ["follow nightlies", "check"]);
    assert_eq!(
        switch(&mut about),
        Some(("Get nightly builds".into(), Some(Toggled::True)))
    );
}
