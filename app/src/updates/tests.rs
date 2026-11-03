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

    fn restart(&self, _: &mut App) {
        self.asked.borrow_mut().push("restart");
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
        let drawn = self.row.read_with(self.cx, |row, _| row.button_bounds().is_some());
        let (said, action) = self.cx.update(|_, cx| words(updater(cx), &stage(cx)));
        assert_eq!(drawn, action.is_some(), "the button drawn isn't the one offered");
        (said.map(|said| said.to_string()), action.map(|(_, title)| title))
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
    assert_eq!(about.reads(), (Some(up_to_date.into()), Some("Check for Updates")));
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
    assert_eq!(about.reads(), (Some("Downloading the update…".into()), None));
    assert!(about.asked().is_empty());
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
    assert_eq!(about.reads(), (Some(ready.into()), Some("Restart to Update")));
    assert_eq!(about.cx.update(|_, cx| command_title(cx)), "Restart to Update");
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

/// A dev build has no updater, so the row is the version alone, as it was.
#[gpui::test]
fn a_dev_build_has_no_button(cx: &mut TestAppContext) {
    let mut about = shown(Updater::None, cx);
    assert_eq!(about.reads(), (None, None));
    about.cx.update(|_, cx| act(Action::Check, cx));
    assert!(about.asked().is_empty());
    assert_eq!(about.cx.update(|_, cx| command_title(cx)), "Check for Updates…");
}

/// A launch that's older than the update the last quit started installing is the copy it was to
/// replace, so the install didn't go in: the row says so at once, with Try Again, which checks
/// again (#1459). A launch that's the update, or newer, says nothing of it.
#[gpui::test]
fn a_launch_older_than_the_update_installed_says_it_didn_t_install(cx: &mut TestAppContext) {
    assert!(went_in(1572, 1572), "the update's own build");
    assert!(went_in(1572, 1580), "a newer build, as one installed by hand");
    assert!(!went_in(1572, 1571), "the build it was to replace");
    let mut about = shown(Updater::Own, cx);
    about.cx.update(|_, cx| didnt_install(cx));
    about.cx.run_until_parked();
    assert_eq!(about.reads(), (Some("The update didn’t install".into()), Some("Try Again")));
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
    assert_eq!(launch("0.1.10"), notes("0.1.10"), "an update, numbered past 0.1.9");
    assert_eq!(launch("0.1.10"), None, "the update's next launch");
    assert_eq!(launch("0.1.9"), None, "a downgrade");
    assert_eq!(launch("0.2.0"), notes("0.2.0"), "an update after the downgrade");
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
    assert_eq!(about.reads(), (Some("Couldn’t check for updates".into()), Some("Try Again")));
    about.click();
    assert_eq!(about.asked(), ["check"]);
    about.says(Stage::Downloading(Some("0.1.1".into())));
    about.says(Stage::Failed { found: false });
    assert_eq!(about.reads(), (Some("The update didn’t install".into()), Some("Try Again")));
    about.says(Stage::Found(None));
    assert_eq!(about.reads(), (Some("An update is available".into()), None));
    about.says(Stage::Idle);
    assert_eq!(about.reads(), (None, Some("Check for Updates")));
}
