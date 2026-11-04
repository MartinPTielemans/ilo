//! Import from the other browsers on the machine (#453): one view, shown in Settings › Import and
//! as a step of the first launch, that lists the browsers found, the one used last first, and
//! brings in what the user picks from one: Arc's Spaces with their names, icons, themes and
//! profiles, their pinned tabs and folders, its link rules and its favorites; the other browsers'
//! bookmarks, from Chromium's family, Firefox's and Safari, as a Space named after the browser,
//! with Safari's Reading List and the tabs Chromium's family had open as its Today tabs; every
//! one's history; and the addresses Chromium's family saved, never its cards. Saved passwords and
//! cookies never come from another browser's files: passwords come from a file the user exports,
//! behind that browser's own sign-in, which Chrome's password importer reads
//! (bc_profile_import_passwords), and the user signs in again to the rest.
//!
//! It reads the user's data, so it's the user's alone: agents get no path to it, and nothing of a
//! passwords file, its rows or its counts. It only reads
//! the other browser's files, and it adds through the core's calls a user's actions make, so a
//! second run adds nothing the first did (plan.rs).

pub mod arc;
mod arrival;
pub mod chromium;
pub mod extensions;
pub mod firefox;
pub mod found;
pub mod html;
pub mod plan;
pub mod read;
pub mod safari;
pub mod session;
pub mod sources;
pub mod suggested;
#[cfg(test)]
pub(crate) mod tests;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use gpui::{
    App, Context, EventEmitter, FocusHandle, KeyDownEvent, SharedString, Task, div, prelude::*, px,
    rgb, svg,
};

pub use crate::settings_rows::Look;
use crate::settings_rows::{
    SLIDE, button, checked_row, detailed_row, drawn, end, frame, glyph_plate, group, primary, row,
    switch, text as words,
};
use crate::theme::Theme;
use crate::tokens::radius;
use crate::tokens::settings::{
    BOX_GAP, BUTTONS_TOP, CHECK_TICK, CHIP_GAP, CHIP_HEIGHT, CHIP_PADDING, CHIP_RADIUS,
    CONTROL_GAP, DETAIL_LINE, DETAIL_TOP, DISCLOSURE_GAP, DISCLOSURE_GLYPH, DOT, ROW_X, STEP,
};
use crate::tokens::text;
use found::{Address, Found, FromProfile, Page, Tab};
use plan::{Add, Counts, Existing, Plan, ProfileRef, SpaceRef, Target};
use read::{Kind, ReadError};
use sources::{Family, Os, Roots, Source};

/// What the import asks of the browser: the core's calls, which the tests stand in for.
pub trait Host {
    /// Where the other browsers' folders are: the user's, or a made-up home a check run gives.
    fn roots(&self) -> Roots;
    fn existing(&self, cx: &App) -> Existing;
    /// Makes a profile, which loads afterwards, and returns it.
    fn create_profile(&self, name: &str) -> u64;
    /// Makes a Space after the others, in the profile the browser started with, and gives it
    /// `icon` and `theme`, its colour the theme's main one.
    fn create_space(&self, name: &str, icon: &str, theme: Option<&Theme>) -> u64;
    /// Puts the empty `space` in `profile`, which may happen only once the profile has loaded.
    fn set_space_profile(&self, space: u64, profile: u64);
    fn space_profile(&self, space: u64) -> u64;
    /// Makes a folder named `name` at the end of `folder`, else of `space`'s pinned tabs.
    fn create_folder(&self, space: u64, folder: u64, name: &str) -> u64;
    /// Pins a tab at `tab`'s URL, with its name, at the end of `folder`, else of `space`'s pinned
    /// tabs, in `space`'s profile. Its page loads once it's first shown.
    fn add_pinned(&self, space: u64, folder: u64, tab: &Tab);
    /// Makes a tab at `tab`'s URL the last of the favorites `space` shows, in its profile.
    fn add_favorite(&self, space: u64, tab: &Tab);
    /// Opens a tab at `tab`'s URL at the top of `space`'s Today tabs, in its profile. Its page
    /// loads once it's first shown.
    fn add_today(&self, space: u64, tab: &Tab);
    /// Puts `rules` after the user's link rules.
    fn add_link_rules(&self, rules: &[crate::link_rules::Rule]);
    /// Whether `profile` has loaded, so its history can take pages.
    fn profile_loaded(&self, profile: u64) -> bool;
    /// Adds `pages` to `profile`'s history but for those it has, and returns whether it will. If
    /// it will, `then` gets how many it added, later.
    fn add_history(
        &self,
        profile: u64,
        pages: &[Page],
        then: Box<dyn FnOnce(usize, &mut App)>,
    ) -> bool;
    /// Adds `addresses` to `profile`'s but for those it has, and returns whether it will. If it
    /// will, `then` gets how many it added, later.
    fn add_addresses(
        &self,
        profile: u64,
        addresses: &[Address],
        then: Box<dyn FnOnce(usize, &mut App)>,
    ) -> bool;
    /// Calls `then` once, the next time the Spaces change, such as when a Space's profile has
    /// loaded.
    fn when_spaces_change(&self, then: Box<dyn FnOnce(&mut App)>);
    /// Shows the system's list of apps with Full Disk Access, where the user can add this one.
    fn open_full_disk_access(&self);
    /// Calls `then` once, the next time this app becomes the active one, such as back from System
    /// Settings.
    fn when_active(&self, then: Box<dyn FnOnce(&mut App)>);
    /// Asks the user for a file of `exported` with the system's open panel; `then` gets it, or
    /// none if they cancel.
    fn choose_file(
        &self,
        exported: Exported,
        then: Box<dyn FnOnce(Option<PathBuf>, &mut App)>,
        cx: &mut App,
    );
    /// Imports the passwords in `file` into the profile the user is in, and returns whether it
    /// will. If it will, `then` gets what came of it, later.
    fn import_passwords(&self, file: &Path, then: Box<dyn FnOnce(Passwords, &mut App)>) -> bool;
    /// The name of the profile passwords go into, when there's more than one to tell apart.
    fn passwords_profile(&self) -> Option<String>;
    /// Opens an extension's page in the Chrome Web Store in a new tab behind the one shown, where
    /// the user can add it.
    fn open_store_page(&self, url: &str);
}

/// What a file the user exported from another browser holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Exported {
    Passwords,
    /// Bookmarks, as an HTML file in the format every browser exports (html.rs).
    Bookmarks,
}

/// What came of importing a file of passwords, as bc_profile_import_passwords says it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Passwords {
    /// `imported` came in or were here already, `conflicted` were kept as the profile has them, as
    /// it holds another password for the site and username, and `skipped` rows weren't logins.
    Imported {
        imported: usize,
        conflicted: usize,
        skipped: usize,
    },
    Unreadable,
    /// The file's first row isn't a passwords export's.
    NotExport,
    TooLarge,
    Failed,
}

/// A host with no browser behind it, for the preview and the shell's tests: it finds the browsers
/// under its roots, and an import there adds nothing.
pub struct Detached(pub Roots);

impl Host for Detached {
    fn roots(&self) -> Roots {
        self.0.clone()
    }
    fn existing(&self, _: &App) -> Existing {
        Existing::default()
    }
    fn create_profile(&self, _: &str) -> u64 {
        0
    }
    fn create_space(&self, _: &str, _: &str, _: Option<&Theme>) -> u64 {
        0
    }
    fn set_space_profile(&self, _: u64, _: u64) {}
    fn space_profile(&self, _: u64) -> u64 {
        0
    }
    fn create_folder(&self, _: u64, _: u64, _: &str) -> u64 {
        0
    }
    fn add_pinned(&self, _: u64, _: u64, _: &Tab) {}
    fn add_favorite(&self, _: u64, _: &Tab) {}
    fn add_today(&self, _: u64, _: &Tab) {}
    fn add_link_rules(&self, _: &[crate::link_rules::Rule]) {}
    fn profile_loaded(&self, _: u64) -> bool {
        true
    }
    fn add_history(&self, _: u64, _: &[Page], _: Box<dyn FnOnce(usize, &mut App)>) -> bool {
        false
    }
    fn add_addresses(&self, _: u64, _: &[Address], _: Box<dyn FnOnce(usize, &mut App)>) -> bool {
        false
    }
    fn when_spaces_change(&self, _: Box<dyn FnOnce(&mut App)>) {}
    fn open_full_disk_access(&self) {}
    fn when_active(&self, _: Box<dyn FnOnce(&mut App)>) {}
    fn choose_file(&self, _: Exported, _: Box<dyn FnOnce(Option<PathBuf>, &mut App)>, _: &mut App) {
    }
    fn import_passwords(&self, _: &Path, _: Box<dyn FnOnce(Passwords, &mut App)>) -> bool {
        false
    }
    fn passwords_profile(&self) -> Option<String> {
        None
    }
    fn open_store_page(&self, _: &str) {}
}

/// An import under way: the profiles and Spaces it made, and what goes into them once each Space
/// is in its profile.
pub struct Job {
    plan: Plan,
    profiles: Vec<u64>,
    /// Each planned Space's id, with the profile it waits for.
    spaces: Vec<(u64, u64)>,
}

impl Job {
    /// Makes the profiles and Spaces `plan` needs.
    pub fn start(plan: Plan, host: &dyn Host) -> Job {
        let profiles: Vec<u64> = plan
            .new_profiles
            .iter()
            .map(|name| host.create_profile(name))
            .collect();
        let profile_id = |profile: ProfileRef| match profile {
            ProfileRef::Existing(id) => id,
            ProfileRef::New(at) => profiles[at],
        };
        let spaces = (plan.spaces.iter())
            .map(|space| match &space.into {
                Target::Existing(id) => (*id, host.space_profile(*id)),
                Target::New {
                    name,
                    profile,
                    icon,
                    theme,
                } => {
                    let id = host.create_space(name, icon, theme.as_ref());
                    let profile = profile_id(*profile);
                    if host.space_profile(id) != profile {
                        host.set_space_profile(id, profile);
                    }
                    (id, profile)
                }
            })
            .collect();
        Job {
            plan,
            profiles,
            spaces,
        }
    }

    fn profile(&self, profile: ProfileRef) -> u64 {
        match profile {
            ProfileRef::Existing(id) => id,
            ProfileRef::New(at) => self.profiles[at],
        }
    }

    /// Whether every Space is in its profile, so its tabs can go in, and every profile history
    /// and addresses go to has loaded.
    pub fn ready(&self, host: &dyn Host) -> bool {
        let spaces =
            (self.spaces.iter()).all(|&(space, profile)| host.space_profile(space) == profile);
        let loaded = |profile: &ProfileRef| host.profile_loaded(self.profile(*profile));
        let history = self.plan.history.iter().all(|(profile, _)| loaded(profile));
        let addresses = self
            .plan
            .addresses
            .iter()
            .all(|(profile, _)| loaded(profile));
        spaces && history && addresses
    }

    /// How many Spaces it made, which stay, empty, if it stops before they're filled.
    fn made(&self) -> usize {
        let new = |space: &&plan::SpacePlan| matches!(space.into, Target::New { .. });
        self.plan.spaces.iter().filter(new).count()
    }

    /// The profiles it made that haven't opened with their Spaces in them, by name.
    fn unopened(&self, host: &dyn Host) -> Vec<&str> {
        let names = self.plan.new_profiles.iter().zip(&self.profiles);
        let open = |profile: u64| {
            let spaces = self.spaces.iter().filter(|&&(_, each)| each == profile);
            host.profile_loaded(profile)
                && spaces
                    .clone()
                    .all(|&(space, _)| host.space_profile(space) == profile)
        };
        names
            .filter(|&(_, &id)| !open(id))
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Asks again to put each Space not yet in its profile there, which loads the profile.
    fn retry(&self, host: &dyn Host) {
        for &(space, profile) in &self.spaces {
            if host.space_profile(space) != profile {
                host.set_space_profile(space, profile);
            }
        }
    }

    /// Adds the pinned tabs, folders, Today tabs and favorites, and returns the history and
    /// addresses for each profile, which the core adds away from the UI.
    pub fn finish(mut self, host: &dyn Host, cx: &App) -> Later {
        fn add(host: &dyn Host, space: u64, folder: u64, adds: &[Add]) {
            for each in adds {
                match each {
                    Add::Tab(tab) => host.add_pinned(space, folder, tab),
                    Add::Folder {
                        existing,
                        name,
                        adds,
                    } => {
                        let inner =
                            existing.unwrap_or_else(|| host.create_folder(space, folder, name));
                        add(host, space, inner, adds);
                    }
                }
            }
        }
        for (space, &(id, _)) in self.plan.spaces.iter().zip(&self.spaces) {
            add(host, id, 0, &space.pinned);
            // Each opens at the top, so the last goes first to keep the browser's order.
            for tab in space.today.iter().rev() {
                host.add_today(id, tab);
            }
        }
        let rules: Vec<crate::link_rules::Rule> = (self.plan.link_rules.iter())
            .map(|rule| crate::link_rules::Rule {
                condition: rule.condition,
                text: rule.text.clone(),
                space: match rule.space {
                    SpaceRef::Existing(id) => id,
                    SpaceRef::Planned(at) => self.spaces[at].0,
                },
            })
            .collect();
        if !rules.is_empty() {
            host.add_link_rules(&rules);
        }
        let existing = host.existing(cx);
        for (profile, tab) in &self.plan.favorites {
            let profile = self.profile(*profile);
            // A favorite goes in through a Space of its profile; one with no Space there stays out.
            let imported = self
                .spaces
                .iter()
                .find(|&&(_, each)| each == profile)
                .map(|&(id, _)| id);
            let any = existing
                .spaces
                .iter()
                .find(|space| space.profile == profile)
                .map(|space| space.id);
            if let Some(space) = imported.or(any) {
                host.add_favorite(space, tab);
            }
        }
        let history = std::mem::take(&mut self.plan.history);
        let addresses = std::mem::take(&mut self.plan.addresses);
        Later {
            history: (history.into_iter())
                .map(|(profile, pages)| (self.profile(profile), pages))
                .collect(),
            addresses: (addresses.into_iter())
                .map(|(profile, addresses)| (self.profile(profile), addresses))
                .collect(),
        }
    }
}

/// What the core adds away from the UI, by profile.
pub struct Later {
    pub history: Vec<(u64, Vec<Page>)>,
    pub addresses: Vec<(u64, Vec<Address>)>,
}

/// What the view tells whoever shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum ImportEvent {
    /// An import read what it brings, and goes on by itself while the view isn't shown; running()
    /// says so. Until then the view may still need the user, as for Full Disk Access.
    Started,
    /// The import that began is over, and this says what came in, or why nothing did.
    Finished(SharedString),
    /// The user read what came in, and pressed Done.
    Done,
    /// The user went on without importing.
    Skipped,
}

enum Step {
    /// The browsers found.
    List,
    /// What can come from one, to pick from, once its files are read (`read`, none while they're
    /// read away from the UI), and the file of its passwords the user chose, with whether it comes.
    Pick {
        source: usize,
        picked: Vec<Kind>,
        read: Option<Box<Picked>>,
        passwords: Option<(PathBuf, bool)>,
    },
    /// Reading the browser's files, then waiting for new profiles to load and for the history to
    /// go in.
    /// From the browser `source`, or from a file, with what it's from, by `name`.
    Running {
        source: Option<usize>,
        name: String,
        run: Run,
    },
    /// The system keeps the browser's files from this app until the user gives it Full Disk
    /// Access, as macOS does Safari's. Each time the app comes back to the front, it reads them
    /// again, `again` once it has without access.
    NoAccess {
        source: usize,
        picked: Vec<Kind>,
        again: bool,
    },
    /// Chrome's importer reading a file of passwords the user chose.
    Passwords,
    /// What came in, or why nothing did.
    Done(SharedString),
}

/// What the first launch shows over the view, and around it, for the state it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Guide {
    pub title: String,
    pub line: String,
    /// Its one button's words, which press() does, if the state has one.
    pub button: Option<String>,
    pub corner: Corner,
}

/// What the first launch's corner says Esc does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Corner {
    /// Goes on without importing.
    Skip,
    /// Goes back to the list (escape).
    Back,
    /// Nothing to say: Esc is the view's Cancel there.
    None,
}

/// What a browser picked has, read as it's picked, so its choices say how much of each comes.
pub(crate) struct Picked {
    found: Found,
    tally: read::Tally,
    /// The profiles what it brings goes into, by name, each with whether the import makes it.
    into: Vec<(String, bool)>,
}

/// The bookmark folders to choose from, behind Choose folders, and those the user unticked.
#[derive(Default)]
struct Folders {
    open: bool,
    /// Read the first time it opens, for the browser picked.
    names: Option<Vec<String>>,
    reading: bool,
    left_out: Vec<String>,
}

/// How long an import waits for the profiles it made to open before it says one hasn't. Chrome
/// makes and loads one in a second or two, so this is far past a slow Mac's.
const STALL: Duration = Duration::from_secs(20);

/// An import under way.
#[derive(Default)]
struct Run {
    /// Which run it is, so what an earlier one waited for, such as its files, doesn't reach it.
    id: u64,
    counts: Counts,
    /// Made once the browser's files are read, until what it adds is in.
    job: Option<Job>,
    /// Whether the job waits for the next change of the Spaces, so it waits once at a time.
    watching: bool,
    /// Ends the wait for the profiles after STALL; dropped with the run, which cancels it.
    stall: Option<Task<()>>,
    /// What the view says once that wait has gone on too long.
    stalled: Option<SharedString>,
    /// Whether the job's Spaces, tabs, favorites and link rules are in, so the history and
    /// addresses the core adds, which it can't stop, are all that's left.
    placed: bool,
    /// The profiles whose history, and whose addresses, the core is still adding.
    history_left: usize,
    addresses_left: usize,
    /// Pages and addresses the core added, which leaves out those that were here.
    pages: usize,
    addresses: usize,
    /// The extensions it listed to add, but for ad blockers.
    listed: usize,
    /// The file of passwords chosen in the browser's choices, which Chrome's importer reads once
    /// the rest is placed, whether it's still reading it, and what came of it.
    passwords_file: Option<PathBuf>,
    passwords_left: bool,
    passwords_said: Option<SharedString>,
    /// A card for each Space it makes or fills, as the core makes them (arrival.rs).
    cards: Vec<arrival::Card>,
    /// When the Spaces were in, for what the core adds after them.
    placed_at: Option<Instant>,
}

impl Run {
    /// How many of what the core adds away from the UI it still waits for.
    fn left(&self) -> usize {
        self.history_left + self.addresses_left + usize::from(self.passwords_left)
    }
}

/// The import's view: Settings shows it as its Import tab, and the first launch as a step, with
/// Skip.
pub struct ImportView {
    host: Rc<dyn Host>,
    skip: bool,
    look: Look,
    sources: Vec<Source>,
    step: Step,
    /// Which switch last changed, and when, for its knob's slide.
    switched: Option<(Kind, Instant)>,
    /// The passwords file the last import read, while Done offers to delete it, as it holds them
    /// in plain text. The view keeps its path, never its contents.
    passwords_file: Option<PathBuf>,
    /// The step last drawn. A click before a step draws lands on the one before it, so check.sh
    /// waits for the line each one logs.
    drawn: Option<&'static str>,
    /// Where Import and Done were logged as drawn, emptied with each step so each logs them anew:
    /// check.sh clicks them there, as a summary that wraps, or a taller row, moves them.
    drawn_at: Rc<RefCell<Vec<String>>>,
    /// The system it runs on, for where the browsers it names export their passwords.
    os: Os,
    /// The bookmark folders of the browser picked, to choose from.
    folders: Folders,
    /// The favorites offered that the user unticked, by URL, which its end leaves out when it
    /// adds them.
    unticked: Vec<String>,
    /// The favorites the last import offers from its history, once it's read.
    suggested: Option<Vec<Tab>>,
    /// How many imports it ran, which numbers each run.
    runs: u64,
    /// Whether its host draws the heading over it, as Settings does, naming the browser or file
    /// it's inside of after it (crumb), so the view doesn't say it again.
    headed: bool,
    /// Whether its host draws its title, its one button and the corner (guided).
    guided: bool,
    /// The browser the guided list has picked, by its row.
    picked_row: usize,
    /// The browser or file the step shown is from, from Import… until Done.
    crumb: Option<String>,
    /// The browser last picked, and what was switched on for it, which going back keeps for the
    /// next Import… on it.
    kept: Option<(usize, Vec<Kind>)>,
    /// Its own focus, once its host gives it the keyboard, for Esc.
    focus: Option<FocusHandle>,
    /// Which read of a browser picked is the one wanted, so an earlier one doesn't land.
    reads: u64,
    /// Whether Import was pressed while the browser picked was still being read.
    import_when_read: bool,
}

impl EventEmitter<ImportEvent> for ImportView {}

/// The browsers under `roots` that have something this build can bring in, the one used last
/// first.
pub fn importable(roots: &Roots) -> Vec<Source> {
    sources::find(roots)
        .into_iter()
        .filter(read::has_any)
        .collect()
}

impl ImportView {
    /// Finds the browsers on the machine. With `skip`, it offers to go on without importing, as
    /// the first launch's step does. It's drawn in Settings' look until `look` gives another.
    pub fn new(host: Rc<dyn Host>, skip: bool) -> ImportView {
        let roots = host.roots();
        let sources = importable(&roots);
        let names: Vec<String> = sources.iter().map(Source::name).collect();
        // check.sh waits for this line.
        log!("browser: import found {}", names.join(", "));
        ImportView {
            host,
            skip,
            look: Look::settings(true),
            sources,
            step: Step::List,
            switched: None,
            passwords_file: None,
            drawn: None,
            drawn_at: Rc::default(),
            os: roots.os,
            unticked: Vec::new(),
            folders: Folders::default(),
            suggested: None,
            runs: 0,
            headed: false,
            guided: false,
            picked_row: 0,
            crumb: None,
            kept: None,
            focus: None,
            reads: 0,
            import_when_read: false,
        }
    }

    /// Leaves its heading to its host, which draws it over the view with crumb() after it.
    pub fn headed(mut self) -> ImportView {
        self.headed = true;
        self
    }

    /// Takes the keyboard while the user is in it, as a click there gives it, so Esc leaves the
    /// state shown (escape).
    pub fn take_keys(&mut self, cx: &mut Context<Self>) {
        self.focus.get_or_insert_with(|| cx.focus_handle());
    }

    /// Draws it in `look`, the ground and text of the view it's shown in.
    pub fn look(mut self, look: Look) -> ImportView {
        self.look = look;
        self
    }

    pub fn set_look(&mut self, look: Look, cx: &mut Context<Self>) {
        if self.look != look {
            self.look = look;
            cx.notify();
        }
    }

    /// Whether no browser to import from was found, so a first launch can leave its step out.
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// What an import under way says, such as "Importing from Chrome…", for a host to show while
    /// the view isn't.
    pub fn running(&self) -> Option<SharedString> {
        match &self.step {
            Step::Running { name, run, .. } => match &run.stalled {
                Some(stalled) => Some(stalled.clone()),
                None => Some(format!("Importing from {name}…").into()),
            },
            Step::Passwords => Some("Importing passwords…".into()),
            _ => None,
        }
    }

    /// Lists the browsers found again, as Settings does each time it shows the tab.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if matches!(self.step, Step::List | Step::Done(_)) {
            let (suggested, runs, focus) = (self.suggested.take(), self.runs, self.focus.take());
            let (headed, guided) = (self.headed, self.guided);
            *self = ImportView::new(self.host.clone(), self.skip).look(self.look);
            (self.suggested, self.runs, self.focus) = (suggested, runs, focus);
            (self.headed, self.guided) = (headed, guided);
            cx.notify();
        }
    }

    /// The browser or file the state shown is about, such as "Arc" from Import… on its row until
    /// Done, for the heading over the view: Import › Arc.
    pub fn crumb(&self) -> Option<SharedString> {
        self.crumb.clone().map(SharedString::from)
    }

    /// Whether an import is between Import… and its end, while its host shows nothing beside it,
    /// as Settings hides Export.
    pub fn under_way(&self) -> bool {
        !matches!(self.step, Step::List | Step::Done(_))
    }

    /// Its focus, once it takes the keyboard.
    #[cfg(test)]
    pub(crate) fn keys(&self) -> Option<gpui::FocusHandle> {
        self.focus.clone()
    }

    /// The browser last picked, while the list is shown again, as Esc or Back keep it.
    #[cfg(test)]
    pub(crate) fn kept(&self) -> Option<usize> {
        self.kept.as_ref().map(|(source, _)| *source)
    }

    /// The button that does what the state shown is for, the one in the host's accent: Import,
    /// Open System Settings, Try Again once a profile hasn't opened, then Add Favorites while
    /// there are favorites to add, else Done. The list has none, as each row's Import… is one of
    /// many, and a running import none, as it goes on by itself.
    pub fn primary(&self) -> Option<&'static str> {
        match &self.step {
            Step::List | Step::Passwords => None,
            Step::Pick { .. } => Some("Import"),
            Step::NoAccess { .. } => Some("Open System Settings"),
            Step::Running { run, .. } => run.stalled.is_some().then_some("Try Again"),
            Step::Done(_) if self.offered().is_some() => Some("Add Favorites"),
            Step::Done(_) => Some("Done"),
        }
    }

    /// The favorites offered at the end, while there are any.
    fn offered(&self) -> Option<&[Tab]> {
        self.suggested.as_deref().filter(|tabs| !tabs.is_empty())
    }

    /// A push button titled `title`, in the accent if it's the state's primary().
    fn control(
        &self,
        id: impl Into<gpui::ElementId>,
        title: &'static str,
    ) -> gpui::Stateful<gpui::Div> {
        match self.primary() == Some(title) {
            true => primary(&self.look, id, title),
            false => button(&self.look, id, title),
        }
    }

    /// What Esc does: from what comes from a browser, or the wait for Full Disk Access, back to
    /// the list, and, guided, from an import that can still stop, Cancel. Says whether it did
    /// anything, as elsewhere Esc is the host's, which on the first launch's list skips the step.
    pub fn escape(&mut self, cx: &mut Context<Self>) -> bool {
        match &self.step {
            Step::Pick { .. } | Step::NoAccess { .. } => {
                log!("browser: import escape");
                self.back(cx);
                true
            }
            Step::Running { run, .. } if self.guided && !run.placed => {
                log!("browser: import escape");
                self.cancel(cx);
                true
            }
            _ => false,
        }
    }

    /// Leaves the view's title, its one button and the corner to its host, as the first launch
    /// draws them for each state (guide): the list picks one browser, the used-last, rather than
    /// give each its Import…, and the browser's choices and the wait for access have no buttons
    /// of their own.
    pub fn guided(mut self) -> ImportView {
        self.guided = true;
        self
    }

    /// What the first launch says over the view for the state it shows, and what its button and
    /// its corner do.
    pub fn guide(&self) -> Guide {
        let here = match self.os {
            Os::Mac => "this Mac",
            _ => "this computer",
        };
        let name = |source: usize| {
            self.sources
                .get(source)
                .map(Source::name)
                .unwrap_or_default()
        };
        let app = env!("APP_NAME");
        let (title, line, button, corner) = match &self.step {
            Step::List => (
                "Bring your stuff.".to_string(),
                format!(
                    "Spaces, tabs and history from the browser you used. It all stays on {here}."
                ),
                (!self.sources.is_empty())
                    .then(|| format!("Import from {}", name(self.picked_row))),
                Corner::Skip,
            ),
            Step::Pick { source, .. } => (
                format!("What comes from {}.", name(*source)),
                "All of it, unless you turn something off. Tabs load when you first open them."
                    .to_string(),
                Some("Import".to_string()),
                Corner::Back,
            ),
            Step::NoAccess { source, .. } => (
                format!("Let {app} read {}.", name(*source)),
                format!(
                    "macOS keeps {}'s bookmarks and history from other apps until you turn on \
                     Full Disk Access.",
                    name(*source)
                ),
                Some("Open System Settings".to_string()),
                Corner::Back,
            ),
            // Esc there is Cancel, which the view offers itself, so the corner says nothing.
            Step::Running {
                source: Some(source),
                run,
                ..
            } if !run.placed => (
                "Here it comes.".to_string(),
                format!(
                    "From {}. It goes on by itself once your Spaces are in.",
                    name(*source)
                ),
                None,
                Corner::None,
            ),
            _ => (
                "Bring your stuff.".to_string(),
                format!(
                    "Spaces, tabs and history from the browser you used. It all stays on {here}."
                ),
                None,
                Corner::Skip,
            ),
        };
        Guide {
            title,
            line,
            button,
            corner,
        }
    }

    /// What the first launch's button does in the state shown: the list's brings the browser
    /// picked, the choices' imports, and the wait for access's opens System Settings.
    pub fn press(&mut self, cx: &mut Context<Self>) {
        match &self.step {
            Step::List if self.picked_row < self.sources.len() => self.pick(self.picked_row, cx),
            Step::Pick { .. } => self.import(cx),
            Step::NoAccess { .. } => self.host.open_full_disk_access(),
            _ => {}
        }
    }

    /// Moves the list's pick `by` rows, stopping at its ends, as ↑ and ↓ do.
    pub fn move_pick(&mut self, by: isize, cx: &mut Context<Self>) {
        if !matches!(self.step, Step::List) || self.sources.is_empty() {
            return;
        }
        let last = self.sources.len() - 1;
        let to = (self.picked_row as isize + by).clamp(0, last as isize) as usize;
        if to != self.picked_row {
            self.picked_row = to;
            cx.notify();
        }
    }

    /// The favorites the last import offers from the history it read, the sites visited most;
    /// none until it's read them. An import that brought favorites, as Arc's does, offers none.
    pub fn suggested(&self) -> Option<&[Tab]> {
        self.suggested.as_deref()
    }

    /// Offers `tabs` as an import's history would, for the first launch's tests.
    #[cfg(test)]
    pub(crate) fn offer(&mut self, tabs: Vec<Tab>) {
        self.suggested = Some(tabs);
    }

    /// The step last drawn, as check.sh reads it from the log.
    #[cfg(test)]
    pub(crate) fn drawn(&self) -> Option<&'static str> {
        self.drawn
    }

    /// Makes `tabs` favorites of `space`, as the user ticked them.
    pub fn add_favorites(&self, space: u64, tabs: &[Tab]) {
        let urls: Vec<&str> = tabs.iter().map(|tab| tab.url.as_str()).collect();
        // check.sh reads this line.
        log!("browser: import favorites {}", urls.join(" "));
        for tab in tabs {
            self.host.add_favorite(space, tab);
        }
    }

    pub(crate) fn pick(&mut self, source: usize, cx: &mut Context<Self>) {
        // check.sh waits for this line.
        log!("browser: import pick {}", self.sources[source].name());
        let picked = match self.kept.take() {
            Some((kept, picked)) if kept == source => picked,
            _ => read::kinds(&self.sources[source]).to_vec(),
        };
        self.crumb = Some(self.sources[source].name());
        self.step = Step::Pick {
            source,
            picked,
            read: None,
            passwords: None,
        };
        self.folders = Folders::default();
        self.unticked.clear();
        self.import_when_read = false;
        self.read_picked(source, cx);
        cx.notify();
    }

    /// Reads all `source` offers, away from the UI, so its choices say how much of each comes,
    /// and so the system asks for access now, not once Import is pressed.
    fn read_picked(&mut self, source: usize, cx: &mut Context<Self>) {
        self.reads += 1;
        let id = self.reads;
        let from = self.sources[source].clone();
        let read =
            cx.background_spawn(
                async move { read::read(&from, read::kinds(&from), SystemTime::now()) },
            );
        cx.spawn(async move |view, cx| {
            let found = read.await;
            view.update(cx, |view, cx| view.picked_read(source, id, found, cx))
                .ok();
        })
        .detach();
    }

    /// What reading the browser picked found: its choices with their counts, or the way to the
    /// access the system keeps from the app.
    fn picked_read(
        &mut self,
        source: usize,
        id: u64,
        found: Result<Found, ReadError>,
        cx: &mut Context<Self>,
    ) {
        // Unless the user went back, or picked again, meanwhile.
        let (Step::Pick {
            source: now,
            picked,
            ..
        }
        | Step::NoAccess {
            source: now,
            picked,
            ..
        }) = &self.step
        else {
            return;
        };
        if id != self.reads || *now != source {
            return;
        }
        let picked = picked.clone();
        let again = matches!(self.step, Step::NoAccess { .. });
        let name = self.sources[source].name();
        let found = match found {
            // Only macOS keeps a browser's files from an app until it's let read them.
            Err(ReadError::NoAccess) if self.os == Os::Mac => {
                return self.wait_for_access(source, picked, again, cx);
            }
            Ok(found) => found,
            Err(error) => {
                log!("browser: import from {name} failed: {error}");
                return self.finish(format!("Couldn't read {name}'s data: {error}."), cx);
            }
        };
        let tally = read::tally(&found, &self.sources[source]);
        // check.sh reads this line.
        log!("browser: import read {name}: {tally:?}");
        let into = self.profiles_into(&found, source, cx);
        let passwords = match &mut self.step {
            Step::Pick { passwords, .. } => passwords.take(),
            _ => None,
        };
        self.step = Step::Pick {
            source,
            picked,
            read: Some(Box::new(Picked { found, tally, into })),
            passwords,
        };
        if std::mem::take(&mut self.import_when_read) {
            return self.import(cx);
        }
        cx.notify();
    }

    /// Waits for the user to let the app read the browser's files, reading them again each time
    /// it comes back to the front.
    fn wait_for_access(
        &mut self,
        source: usize,
        picked: Vec<Kind>,
        again: bool,
        cx: &mut Context<Self>,
    ) {
        let name = self.sources[source].name();
        // check.sh can wait for this line.
        let still = if again { " still" } else { "" };
        log!("browser: import from {name}{still} needs full disk access");
        self.step = Step::NoAccess {
            source,
            picked,
            again,
        };
        self.import_when_read = false;
        let view = cx.entity().downgrade();
        self.host.when_active(Box::new(move |cx| {
            view.update(cx, |view, cx| {
                // Unless the user went back from it meanwhile.
                if matches!(view.step, Step::NoAccess { source: now, .. } if now == source) {
                    view.read_picked(source, cx);
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// The profiles here that what `found`, from the browser `source`, goes into, by name, each
    /// with whether the import makes it, as the plan for it now has them.
    fn profiles_into(
        &self,
        found: &Found,
        source: usize,
        cx: &mut Context<Self>,
    ) -> Vec<(String, bool)> {
        let names: Vec<(String, String)> = (self.sources[source].profiles.iter())
            .map(|profile| (profile.dir.clone(), profile.name.clone()))
            .collect();
        let existing = self.host.existing(cx);
        let plan = plan::plan(found, &names, &existing);
        let name = |profile: ProfileRef| match profile {
            ProfileRef::Existing(id) => (existing.profiles.iter())
                .find(|(each, _)| *each == id)
                .map(|(_, name)| (name.clone(), false)),
            ProfileRef::New(at) => plan.new_profiles.get(at).map(|name| (name.clone(), true)),
        };
        let spaces = plan.spaces.iter().filter_map(|space| match &space.into {
            Target::New { profile, .. } => Some(*profile),
            Target::Existing(id) => (existing.spaces.iter())
                .find(|each| each.id == *id)
                .map(|each| ProfileRef::Existing(each.profile)),
        });
        let rest = (plan.favorites.iter().map(|(profile, _)| *profile))
            .chain(plan.history.iter().map(|(profile, _)| *profile))
            .chain(plan.addresses.iter().map(|(profile, _)| *profile));
        let mut into: Vec<(String, bool)> = Vec::new();
        for each in spaces.chain(rest).filter_map(name) {
            if !into.contains(&each) {
                into.push(each);
            }
        }
        into
    }

    /// Asks for the file of passwords the browser picked exported, which comes with the rest.
    fn choose_picked_passwords(&mut self, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let then = Box::new(move |file: Option<PathBuf>, cx: &mut App| {
            view.update(cx, |view, cx| {
                if let (Some(file), Step::Pick { passwords, .. }) = (file, &mut view.step) {
                    // check.sh waits for this line.
                    log!("browser: import passwords file {}", file_name(&file));
                    *passwords = Some((file, true));
                    cx.notify();
                }
            })
            .ok();
        });
        self.host.clone().choose_file(Exported::Passwords, then, cx);
    }

    /// Turns the passwords file chosen on or off.
    fn toggle_passwords(&mut self, cx: &mut Context<Self>) {
        if let Step::Pick {
            passwords: Some((_, on)),
            ..
        } = &mut self.step
        {
            *on = !*on;
            cx.notify();
        }
    }

    /// Opens or closes the bookmark folders to choose from, reading them the first time, away from
    /// the UI.
    fn choose_folders(&mut self, cx: &mut Context<Self>) {
        let Step::Pick { source, .. } = self.step else {
            return;
        };
        self.folders.open = !self.folders.open;
        cx.notify();
        if self.folders.names.is_some() || self.folders.reading {
            return;
        }
        self.folders.reading = true;
        let from = self.sources[source].clone();
        let read = cx.background_spawn(async move { read::folders(&from, SystemTime::now()) });
        cx.spawn(async move |view, cx| {
            let names = read.await.unwrap_or_default();
            view.update(cx, |view, cx| {
                // Unless the user went back to another browser meanwhile.
                if matches!(view.step, Step::Pick { source: now, .. } if now == source) {
                    // check.sh waits for this line.
                    log!("browser: import folders {}", names.join(", "));
                    view.folders.names = Some(names);
                    view.folders.reading = false;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Ticks or unticks the bookmark folder `name`, which comes in only ticked.
    fn toggle_folder(&mut self, name: &str, cx: &mut Context<Self>) {
        let left_out = &mut self.folders.left_out;
        match left_out.iter().position(|each| each == name) {
            Some(at) => {
                left_out.remove(at);
            }
            None => left_out.push(name.into()),
        }
        log!("browser: import leaves out {}", left_out.join(", "));
        cx.notify();
    }

    /// Makes those of the favorites offered that the user ticked, `tabs`, favorites of `space`,
    /// which ends the offer, so the import's end doesn't make it again.
    pub fn add_offered(&mut self, space: u64, tabs: &[Tab]) {
        if !tabs.is_empty() {
            self.add_favorites(space, tabs);
        }
        self.suggested = Some(Vec::new());
        self.unticked.clear();
    }

    /// The favorites offered that are still ticked.
    fn ticked(&self) -> Vec<Tab> {
        let offered = self.suggested.iter().flatten();
        let ticked = offered.filter(|tab| !self.unticked.contains(&tab.url));
        ticked.cloned().collect()
    }

    /// Ticks or unticks the favorite offered at `url`.
    fn toggle_favorite(&mut self, url: &str, cx: &mut Context<Self>) {
        match self.unticked.iter().position(|each| each == url) {
            Some(at) => {
                self.unticked.remove(at);
            }
            None => self.unticked.push(url.into()),
        }
        cx.notify();
    }

    /// Adds the favorites still ticked to the first Space of the profile the browser started
    /// with, whose history they come from, as the first launch adds them to its first Space, and
    /// says how many after what came in.
    fn add_ticked(&mut self, cx: &mut Context<Self>) {
        let ticked = self.ticked();
        let existing = self.host.existing(cx);
        let start = existing.profiles.first().map(|(id, _)| *id);
        let space = existing
            .spaces
            .iter()
            .find(|space| Some(space.profile) == start);
        let (Some(space), Step::Done(said)) = (space, &self.step) else {
            return;
        };
        if ticked.is_empty() {
            return;
        }
        let added = match ticked.len() {
            1 => format!("{said} 1 favorite added."),
            count => format!("{said} {count} favorites added."),
        };
        self.add_offered(space.id, &ticked);
        self.step = Step::Done(added.into());
        cx.notify();
    }

    /// Takes extension `id` off the list and opens its Web Store page, where the user adds it.
    fn add_extension(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(extension) = extensions::take(id) {
            // check.sh waits for this line.
            log!("browser: import add extension {id}");
            self.host.open_store_page(&extension.store_page());
            cx.notify();
        }
    }

    fn toggle(&mut self, kind: Kind, cx: &mut Context<Self>) {
        if let Step::Pick { picked, .. } = &mut self.step {
            match picked.iter().position(|each| *each == kind) {
                Some(at) => {
                    picked.remove(at);
                }
                None => picked.push(kind),
            }
            self.switched = Some((kind, crate::motion::start(cx)));
            cx.notify();
        }
    }

    /// Brings in what was picked from what the browser's files gave, or once they have, if
    /// they're still being read.
    pub(crate) fn import(&mut self, cx: &mut Context<Self>) {
        let Step::Pick {
            source,
            picked,
            read,
            passwords,
        } = &self.step
        else {
            return;
        };
        let Some(read) = read else {
            self.import_when_read = true;
            return;
        };
        let (source, picked) = (*source, picked.clone());
        let from = self.sources[source].clone();
        let kept = read::keep(&read.found, &from, &picked);
        let passwords_file = (passwords.as_ref())
            .filter(|(_, on)| *on)
            .map(|(file, _)| file.clone());
        let left_out = self.folders.left_out.clone();
        let id = self.new_run();
        self.step = Step::Running {
            source: Some(source),
            name: from.name(),
            run: Run {
                id,
                passwords_file,
                ..Run::default()
            },
        };
        self.suggested = None;
        cx.notify();
        if let Some(mut found) = kept {
            read::leave_out(&mut found, &left_out, from.browser.family);
            return self.read(from, picked, Ok(found), cx);
        }
        // What can't be left out of what's read is read again, for what was picked alone.
        let read = cx.background_spawn(async move {
            let mut found = read::read(&from, &picked, SystemTime::now());
            if let Ok(found) = &mut found {
                read::leave_out(found, &left_out, from.browser.family);
            }
            (from, picked, found)
        });
        cx.spawn(async move |view, cx| {
            let (from, picked, found) = read.await;
            view.update(cx, |view, cx| {
                // Unless the user stopped it meanwhile.
                if view.is_run(id) {
                    view.read(from, picked, found, cx)
                }
            })
            .ok();
        })
        .detach();
    }

    /// Plans what's read against what's here, and starts it.
    fn read(
        &mut self,
        from: Source,
        picked: Vec<Kind>,
        found: Result<Found, ReadError>,
        cx: &mut Context<Self>,
    ) {
        let Step::Running {
            source: Some(source),
            name,
            ..
        } = &self.step
        else {
            return;
        };
        let (source, name) = (*source, name.clone());
        let found = match found {
            Ok(found) => found,
            // Taken away since it was picked.
            Err(ReadError::NoAccess) if self.os == Os::Mac => {
                return self.wait_for_access(source, picked, false, cx);
            }
            Err(error) => {
                log!("browser: import from {name} failed: {error}");
                return self.finish(format!("Couldn't read {name}'s data: {error}."), cx);
            }
        };
        let names: Vec<(String, String)> = (from.profiles.iter())
            .map(|profile| (profile.dir.clone(), profile.name.clone()))
            .collect();
        self.bring(&names, found, cx);
    }

    /// Plans what's read, from profiles by `names`, against what's here, and starts it.
    fn bring(&mut self, names: &[(String, String)], found: Found, cx: &mut Context<Self>) {
        let Step::Running { name, .. } = &self.step else {
            return;
        };
        let name = name.clone();
        let existing = self.host.existing(cx);
        let plan = plan::plan(&found, names, &existing);
        let counts = plan.counts();
        // The favorites to offer, from the history of the profile this browser started with, but
        // for its favorites; an import that brings its own offers none.
        let start = existing.profiles.first().map(|(id, _)| *id);
        let here: Vec<String> = (existing.favorites.iter())
            .filter(|(profile, _)| Some(*profile) == start)
            .map(|(_, url)| url.clone())
            .collect();
        let history = (found.history.iter())
            .find(|(profile, _)| *profile == FromProfile::Default)
            .map_or(&[][..], |(_, pages)| &pages[..]);
        let suggested = match plan.favorites.is_empty() {
            true => suggested::favorites(history, &here),
            false => Vec::new(),
        };
        let urls: Vec<&str> = suggested.iter().map(|tab| tab.url.as_str()).collect();
        // check.sh reads this line.
        log!("browser: import suggests {}", urls.join(" "));
        self.suggested = Some(suggested);
        // What's read is all the import needs of the user; the rest goes on by itself.
        cx.emit(ImportEvent::Started);
        // Another ad blocker is left off the list, which says so once (extensions_box).
        let listed = found
            .extensions
            .iter()
            .filter(|each| !each.blocks_ads())
            .count();
        if !found.extensions.is_empty() {
            let ids: Vec<&str> = found
                .extensions
                .iter()
                .map(|each| each.id.as_str())
                .collect();
            // check.sh reads this line.
            log!("browser: import lists extensions {}", ids.join(" "));
            extensions::list(&name, found.extensions);
        }
        // check.sh waits for this line.
        log!(
            "browser: import from {name}: {} Spaces, {} pinned tabs, {} open tabs, {} favorites, {} \
             pages, {} addresses",
            counts.spaces,
            counts.pinned,
            counts.today,
            counts.favorites,
            counts.pages,
            counts.addresses
        );
        // check.sh reads this line.
        log!(
            "browser: import from {name}: {} link rules",
            counts.link_rules
        );
        let Step::Running { run, .. } = &mut self.step else {
            return;
        };
        run.listed = listed;
        if plan.is_empty() {
            run.placed = true;
            return self.import_run_passwords(cx);
        }
        run.counts = counts;
        // A card a Space, from as the core makes each.
        run.cards = arrival::cards(&plan, &existing, Instant::now());
        let names: Vec<&str> = run.cards.iter().map(|card| card.name.as_str()).collect();
        // check.sh reads this line.
        log!("browser: import cards {}", names.join(", "));
        run.job = Some(Job::start(plan, &*self.host));
        self.advance(cx);
    }

    /// A number for an import starting now.
    fn new_run(&mut self) -> u64 {
        self.runs += 1;
        self.runs
    }

    /// Whether the import under way is the run `id`.
    fn is_run(&self, id: u64) -> bool {
        matches!(&self.step, Step::Running { run, .. } if run.id == id)
    }

    /// Adds what goes in once the Spaces are in their profiles, else waits for the next change,
    /// and for STALL at most before saying so.
    fn advance(&mut self, cx: &mut Context<Self>) {
        let Step::Running { run, .. } = &mut self.step else {
            return;
        };
        let Some(job) = run.job.take_if(|job| job.ready(&*self.host)) else {
            if run.job.is_none() {
                return;
            }
            if !run.watching {
                run.watching = true;
                let (view, id) = (cx.entity().downgrade(), run.id);
                self.host.when_spaces_change(Box::new(move |cx| {
                    view.update(cx, |view, cx| view.spaces_changed(id, cx)).ok();
                }));
            }
            if run.stall.is_none() && run.stalled.is_none() {
                run.stall = Some(cx.spawn(async move |view, cx| {
                    cx.background_executor().timer(STALL).await;
                    view.update(cx, |view, cx| view.stall(cx)).ok();
                }));
            }
            return;
        };
        (run.stall, run.stalled, run.placed) = (None, None, true);
        let later = job.finish(&*self.host, cx);
        // Every tab is in once the core has placed them, so each card ticks.
        let now = Instant::now();
        run.placed_at = Some(now);
        for card in &mut run.cards {
            (card.placed, card.done) = (card.total, Some(now));
        }
        // What the core says it added, into the run's count of it and of what it has left.
        type Count = fn(&mut Run) -> (&mut usize, &mut usize);
        let then = |count: Count, cx: &mut Context<Self>| {
            let view = cx.entity().downgrade();
            Box::new(move |added, cx: &mut App| {
                view.update(cx, |view, cx| view.added(count, added, cx))
                    .ok();
            })
        };
        for (profile, pages) in later.history {
            let then = then(|run| (&mut run.pages, &mut run.history_left), cx);
            if self.host.add_history(profile, &pages, then) {
                run.history_left += 1;
            }
        }
        for (profile, addresses) in later.addresses {
            let then = then(|run| (&mut run.addresses, &mut run.addresses_left), cx);
            if self.host.add_addresses(profile, &addresses, then) {
                run.addresses_left += 1;
            }
        }
        self.import_run_passwords(cx);
    }

    /// Imports the passwords file chosen in the browser's choices, once what can be stopped is
    /// in, and completes the run once what the core adds is.
    fn import_run_passwords(&mut self, cx: &mut Context<Self>) {
        let Step::Running { run, .. } = &mut self.step else {
            return;
        };
        if let Some(file) = run.passwords_file.take() {
            let (view, id, chosen) = (cx.entity().downgrade(), run.id, file.clone());
            let then = Box::new(move |passwords: Passwords, cx: &mut App| {
                view.update(cx, |view, cx| {
                    view.run_passwords(id, &chosen, passwords, cx)
                })
                .ok();
            });
            match self.host.import_passwords(&file, then) {
                true => {
                    // check.sh waits for this line.
                    log!("browser: import passwords");
                    run.passwords_left = true;
                }
                false => run.passwords_said = Some("The passwords couldn't be imported.".into()),
            }
        }
        match run.left() {
            0 => self.complete(cx),
            _ => cx.notify(),
        }
    }

    /// What came of the passwords file the run `id` imports, which Done offers to delete.
    fn run_passwords(
        &mut self,
        id: u64,
        file: &Path,
        passwords: Passwords,
        cx: &mut Context<Self>,
    ) {
        let into = self.host.passwords_profile();
        let Step::Running { run, .. } = &mut self.step else {
            return;
        };
        if run.id != id {
            return;
        }
        run.passwords_left = false;
        run.passwords_said = Some(passwords_summary(file, passwords, into.as_deref()));
        if matches!(passwords, Passwords::Imported { .. }) {
            self.passwords_file = Some(file.to_path_buf());
        }
        match run.left() {
            0 => self.complete(cx),
            _ => cx.notify(),
        }
    }

    /// The Spaces changed while the run `id` waited for its profiles.
    fn spaces_changed(&mut self, id: u64, cx: &mut Context<Self>) {
        let Step::Running { run, .. } = &mut self.step else {
            return;
        };
        if run.id == id {
            run.watching = false;
            self.advance(cx);
        }
    }

    /// The profiles haven't opened in STALL, so the view says which, with Try Again.
    fn stall(&mut self, cx: &mut Context<Self>) {
        let Step::Running { run, .. } = &mut self.step else {
            return;
        };
        let Some(job) = &run.job else {
            return;
        };
        let said = match job.unopened(&*self.host)[..] {
            [] => "A profile hasn't opened, so the import can't go on.".to_string(),
            [one] => format!("The {one} profile hasn't opened, so the import can't go on."),
            [ref rest @ .., last] => format!(
                "The {} and {last} profiles haven't opened, so the import can't go on.",
                rest.join(", ")
            ),
        };
        // check.sh can wait for this line.
        log!("browser: import stalled: {said}");
        run.stalled = Some(said.into());
        cx.notify();
    }

    /// Asks again for the profiles that haven't opened, and waits for them as long again.
    fn try_again(&mut self, cx: &mut Context<Self>) {
        let Step::Running { run, .. } = &mut self.step else {
            return;
        };
        let Some(job) = run.job.as_ref().filter(|_| run.stalled.is_some()) else {
            return;
        };
        log!("browser: import tries again");
        job.retry(&*self.host);
        (run.stall, run.stalled) = (None, None);
        cx.notify();
        self.advance(cx);
    }

    /// Stops the import while nothing it brings is in yet, and says so, with the Spaces it made,
    /// empty, which importing again fills. Once they're in, the history the core adds can't stop.
    fn cancel(&mut self, cx: &mut Context<Self>) {
        let Step::Running { name, run, .. } = &self.step else {
            return;
        };
        if run.placed {
            return;
        }
        let said = match run.job.as_ref().map_or(0, Job::made) {
            0 => format!("Stopped. Nothing came in from {name}."),
            1 => format!("Stopped. 1 Space from {name} is here, empty: import again to fill it."),
            made => format!(
                "Stopped. {made} Spaces from {name} are here, empty: import again to fill them."
            ),
        };
        log!("browser: import stopped");
        self.finish(said, cx);
    }

    fn added(
        &mut self,
        count: fn(&mut Run) -> (&mut usize, &mut usize),
        added: usize,
        cx: &mut Context<Self>,
    ) {
        let Step::Running { run, .. } = &mut self.step else {
            return;
        };
        let (count, left) = count(run);
        *count += added;
        *left = left.saturating_sub(1);
        match run.left() {
            0 => self.complete(cx),
            _ => cx.notify(),
        }
    }

    fn complete(&mut self, cx: &mut Context<Self>) {
        let Step::Running { name, run, .. } = &self.step else {
            return;
        };
        let counts = Counts {
            pages: run.pages,
            addresses: run.addresses,
            ..run.counts
        };
        let said = summary(name, counts, run.listed);
        let said = match &run.passwords_said {
            Some(passwords) => format!("{said} {passwords}").into(),
            None => said,
        };
        self.finish(said, cx);
    }

    fn finish(&mut self, said: impl Into<SharedString>, cx: &mut Context<Self>) {
        let said = said.into();
        // check.sh waits for this line.
        log!("browser: import done: {said}");
        self.step = Step::Done(said.clone());
        cx.emit(ImportEvent::Finished(said));
        cx.notify();
    }

    /// Asks for the file of passwords, and imports it.
    pub(crate) fn choose_passwords(&mut self, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let then = Box::new(move |file: Option<PathBuf>, cx: &mut App| {
            if let Some(file) = file {
                view.update(cx, |view, cx| view.import_passwords(file, cx))
                    .ok();
            }
        });
        self.host.clone().choose_file(Exported::Passwords, then, cx);
    }

    /// Asks for a bookmarks file, and imports it.
    fn choose_bookmarks(&mut self, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let then = Box::new(move |file: Option<PathBuf>, cx: &mut App| {
            if let Some(file) = file {
                view.update(cx, |view, cx| view.import_bookmarks(file, cx))
                    .ok();
            }
        });
        self.host.clone().choose_file(Exported::Bookmarks, then, cx);
    }

    /// Reads the bookmarks in `file`, away from the UI, and brings them in as the pinned tabs and
    /// folders of a Space named Bookmarks, in the profile this browser started with.
    fn import_bookmarks(&mut self, file: PathBuf, cx: &mut Context<Self>) {
        if !matches!(self.step, Step::List) {
            return;
        }
        let name = file_name(&file);
        // check.sh waits for this line.
        log!("browser: import bookmarks file {name}");
        self.crumb = Some(name.clone());
        let id = self.new_run();
        self.step = Step::Running {
            source: None,
            name: name.clone(),
            run: Run {
                id,
                ..Run::default()
            },
        };
        self.suggested = None;
        cx.notify();
        let read = cx.background_spawn(async move {
            let bytes = std::fs::read(&file).map_err(|error| error.to_string())?;
            html::bookmarks(&String::from_utf8_lossy(&bytes))
        });
        cx.spawn(async move |view, cx| {
            let pinned = read.await;
            view.update(cx, |view, cx| match pinned {
                // The user stopped it meanwhile.
                _ if !view.is_run(id) => {}
                Ok(pinned) => {
                    let space = found::Space {
                        name: "Bookmarks".into(),
                        profile: FromProfile::Default,
                        icon: String::new(),
                        theme: None,
                        pinned,
                        today: vec![],
                    };
                    let found = Found {
                        spaces: vec![space],
                        ..Found::default()
                    };
                    view.bring(&[], found, cx);
                }
                Err(error) => {
                    log!("browser: import from {name} failed: {error}");
                    view.finish(format!("Couldn't import {name}: {error}."), cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn import_passwords(&mut self, file: PathBuf, cx: &mut Context<Self>) {
        if !matches!(self.step, Step::List) {
            return;
        }
        // An earlier import's favorites aren't offered at this one's end.
        self.suggested = None;
        let view = cx.entity().downgrade();
        let chosen = file.clone();
        let then = Box::new(move |passwords: Passwords, cx: &mut App| {
            view.update(cx, |view, cx| {
                view.passwords_imported(&chosen, passwords, cx)
            })
            .ok();
        });
        if !self.host.import_passwords(&file, then) {
            return self.finish("The passwords couldn't be imported.", cx);
        }
        // check.sh waits for this line.
        log!("browser: import passwords");
        self.crumb = Some(file_name(&file));
        // No Started: its end offers to delete the file, which the user sees there.
        self.step = Step::Passwords;
        cx.notify();
    }

    fn passwords_imported(&mut self, file: &Path, passwords: Passwords, cx: &mut Context<Self>) {
        if !matches!(self.step, Step::Passwords) {
            return;
        }
        let into = self.host.passwords_profile();
        if matches!(passwords, Passwords::Imported { .. }) {
            self.passwords_file = Some(file.to_path_buf());
        }
        self.finish(passwords_summary(file, passwords, into.as_deref()), cx);
    }

    /// Deletes the passwords file the import read, as the user asked.
    pub(crate) fn delete_passwords_file(&mut self, cx: &mut Context<Self>) {
        let (Some(file), Step::Done(said)) = (self.passwords_file.take(), &self.step) else {
            return;
        };
        let name = file_name(&file);
        let said = match std::fs::remove_file(&file) {
            // check.sh waits for this line.
            Ok(()) => {
                log!("browser: import deleted the passwords file");
                format!("{said} {name} is deleted.")
            }
            Err(error) => format!("{said} {name} couldn't be deleted: {error}."),
        };
        self.step = Step::Done(said.into());
        cx.notify();
    }

    pub(crate) fn done(&mut self, cx: &mut Context<Self>) {
        self.passwords_file = None;
        self.step = Step::List;
        self.crumb = None;
        cx.emit(ImportEvent::Done);
        cx.notify();
    }

    /// Back to the list, keeping the browser picked and what was switched on for it.
    fn back(&mut self, cx: &mut Context<Self>) {
        let list = std::mem::replace(&mut self.step, Step::List);
        if let Step::Pick { source, picked, .. } | Step::NoAccess { source, picked, .. } = list {
            self.kept = Some((source, picked));
        }
        self.crumb = None;
        cx.notify();
    }

    /// The way to the access macOS keeps from the app: why, three steps to System Settings and
    /// back, and whether it waits or the app came back without it, with Back and Open System
    /// Settings.
    fn no_access(&self, source: usize, again: bool, cx: &mut Context<Self>) -> gpui::AnyElement {
        let look = &self.look;
        let name = self.sources[source].name();
        let app = env!("APP_NAME");
        let open = self
            .control("import-full-disk-access", "Open System Settings")
            .on_click(cx.listener(|view, _, _, _| view.host.open_full_disk_access()));
        let back = self
            .control("import-back", "Back")
            .on_click(cx.listener(|view, _, _, cx| view.back(cx)));
        let step = |number: usize, said: gpui::AnyElement| {
            let number = div()
                .flex_none()
                .size(px(STEP))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(look.control)
                .text_size(px(text::SMALL))
                .child(number.to_string());
            frame().gap(px(CONTROL_GAP)).child(number).child(said)
        };
        let turn_on = div()
            .flex()
            .items_center()
            .gap(px(CONTROL_GAP))
            .child(format!("Turn on {app}"))
            .child(switch(look, true, 1., look.accent));
        let steps = [
            step(
                1,
                "Open System Settings › Privacy & Security › Full Disk Access".into_any_element(),
            ),
            step(2, turn_on.into_any_element()),
            step(
                3,
                "Come back here. The import goes on by itself.".into_any_element(),
            ),
        ];
        let mark = match again {
            true => svg()
                .path("icons/circle-alert.svg")
                .size(px(DISCLOSURE_GLYPH))
                .text_color(look.detail)
                .into_any_element(),
            false => div()
                .size(px(DOT))
                .rounded_full()
                .bg(look.detail)
                .into_any_element(),
        };
        let status = div()
            .px(px(ROW_X))
            .flex()
            .items_center()
            .gap(px(CONTROL_GAP))
            .text_color(look.detail)
            .child(mark)
            .child(access_said(again));
        div()
            .flex()
            .flex_col()
            .gap(px(BOX_GAP))
            // Guided, the host's title and line say it, and its button is Open System Settings.
            .when(!self.guided, |view| {
                view.child(line(format!(
                    "macOS keeps {name}'s bookmarks and history from other apps until you turn on \
                     Full Disk Access."
                )))
            })
            .child(group(look, steps))
            .child(status)
            .when(!self.guided, |view| {
                view.child(buttons([back.into_any_element(), open.into_any_element()]))
            })
            .into_any_element()
    }

    /// The cards of the import under way: each Space's name, its profile and whether the import
    /// makes it, its tabs in and of how many, and whether it has ticked.
    #[cfg(test)]
    pub(crate) fn cards(&self) -> Vec<(String, (String, bool), usize, usize, bool)> {
        let Step::Running { run, .. } = &self.step else {
            return Vec::new();
        };
        (run.cards.iter())
            .map(|card| {
                let done = card.done.is_some();
                (
                    card.name.clone(),
                    card.profile.clone(),
                    card.placed,
                    card.total,
                    done,
                )
            })
            .collect()
    }

    /// The import under way's cards and the rows after them, asking for frames while they move.
    fn arrival(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let (look, settings, still) = (self.look, !self.guided, crate::motion::still(cx));
        let Step::Running { run, .. } = &mut self.step else {
            return None;
        };
        if run.cards.is_empty() {
            return None;
        }
        let now = Instant::now();
        let adding = run.history_left + run.addresses_left > 0;
        if run.cards.iter().any(|card| card.moving(now)) || (adding && !still) {
            window.request_animation_frame();
        }
        let cards = arrival::draw(&mut run.cards, &look, settings, still, now);
        let counts = run.counts;
        let since = run.placed_at.unwrap_or(now);
        let stage = |left: usize, adding: String, came: usize| match (run.placed, left) {
            (false, _) => arrival::Stage::Waiting,
            (true, 0) => arrival::Stage::In(grouped(came).into()),
            (true, _) => arrival::Stage::Adding(adding.into()),
        };
        let mut rows = Vec::new();
        if counts.favorites > 0 {
            let stage = stage(0, String::new(), counts.favorites);
            rows.push(arrival::stage_row(
                &look,
                glyph(Kind::Favorites),
                "Favorites",
                "At the top of each profile's Spaces",
                &stage,
                still,
                now,
                since,
            ));
        }
        if counts.pages > 0 {
            let adding = format!("Adding {} pages…", grouped(counts.pages));
            let stage = stage(run.history_left, adding, run.pages);
            rows.push(arrival::stage_row(
                &look,
                glyph(Kind::History),
                "History",
                "It goes on while you carry on, and can't be stopped once it starts",
                &stage,
                still,
                now,
                since,
            ));
        }
        if counts.addresses > 0 {
            let stage = stage(run.addresses_left, "Adding…".into(), run.addresses);
            rows.push(arrival::stage_row(
                &look,
                glyph(Kind::Addresses),
                "Addresses",
                "Into each profile",
                &stage,
                still,
                now,
                since,
            ));
        }
        let rows = arrival::stages(&look, rows);
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(BOX_GAP))
                .child(cards)
                .children(rows),
        )
    }

    /// What the wait for Full Disk Access says, while it shows.
    #[cfg(test)]
    pub(crate) fn access(&self) -> Option<String> {
        match self.step {
            Step::NoAccess { again, .. } => Some(access_said(again)),
            _ => None,
        }
    }

    fn list(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let list = div().flex().flex_col().gap(px(BOX_GAP));
        let list = match self.sources.is_empty() {
            true => list.child(line(
                "No browser to import from was found on this computer.",
            )),
            false => list.child(self.browsers(cx)),
        };
        let extensions = self.extensions_box(cx);
        let files = match self.guided {
            true => self.file_links(cx),
            false => self.files(cx),
        };
        list.children(extensions).child(files).into_any_element()
    }

    /// The extensions the last import listed, each with Add…, which opens its Web Store page, and
    /// a line for another ad blocker, which stays off the list, with Add Anyway; none once each is
    /// added.
    fn extensions_box(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let look = &self.look;
        let listed = extensions::listed();
        if listed.extensions.is_empty() {
            return None;
        }
        let from = &listed.from;
        let (blockers, extensions): (Vec<_>, Vec<_>) =
            listed.extensions.iter().partition(|each| each.blocks_ads());
        let rows = extensions.iter().enumerate().map(|(index, extension)| {
            let id = extension.id.clone();
            let add = button(look, ("import-add-extension", index), "Add…")
                .on_click(cx.listener(move |view, _, _, cx| view.add_extension(&id, cx)))
                .into_any_element();
            match extension.pinned {
                true => {
                    let pinned = format!("Pinned in {from}, and here once it's added");
                    detailed_row(look, extension.name.clone(), pinned, Some(add))
                }
                false => row(extension.name.clone(), add),
            }
        });
        let rows: Vec<gpui::Div> = rows.collect();
        let header = (!rows.is_empty()).then(|| {
            line(format!(
                "Extensions from {from}. Add… opens each one's page in the Chrome Web Store in a \
                 tab."
            ))
        });
        let names: Vec<&str> = blockers.iter().map(|each| each.name.as_str()).collect();
        let ids: Vec<String> = blockers.iter().map(|each| each.id.clone()).collect();
        let blockers = (!blockers.is_empty()).then(|| {
            let is = if names.len() == 1 { "isn't" } else { "aren't" };
            let names = match names.split_last() {
                Some((last, rest)) if !rest.is_empty() => {
                    format!("{} and {last}", rest.join(", "))
                }
                _ => names.concat(),
            };
            let said = format!(
                "{names} from {from} {is} listed, as {} blocks ads itself.",
                env!("APP_NAME")
            );
            let anyway = button(look, "import-add-ad-blocker", "Add Anyway").on_click(cx.listener(
                move |view, _, _, cx| {
                    for id in &ids {
                        view.add_extension(id, cx);
                    }
                },
            ));
            detailed_line(look, said.into(), anyway.into_any_element())
        });
        let group = (!rows.is_empty()).then(|| group(look, rows));
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(BOX_GAP))
                .children(header)
                .children(group)
                .children(blockers),
        )
    }

    /// Choose folders, under the switches while bookmarks come: a box of the bookmark folders,
    /// each ticked to come in, which a click unticks.
    fn folder_chooser(&self, cx: &mut Context<Self>) -> gpui::Div {
        let look = &self.look;
        let folders = &self.folders;
        let glyph = match folders.open {
            true => "icons/chevron-up.svg",
            false => "icons/chevron-down.svg",
        };
        let disclosure = div()
            .id("import-choose-folders")
            .px(px(ROW_X))
            .flex()
            .items_center()
            .gap(px(DISCLOSURE_GAP))
            .text_color(look.detail)
            .on_click(cx.listener(|view, _, _, cx| view.choose_folders(cx)))
            .child("Choose folders")
            .child(
                svg()
                    .path(glyph)
                    .size(px(DISCLOSURE_GLYPH))
                    .text_color(look.detail),
            );
        let names = folders.open.then_some(folders.names.as_deref());
        let chosen = match names {
            None => None,
            Some(None) => Some(line("Reading the folders…").text_color(look.detail)),
            Some(Some([])) => Some(line("No folders to choose from.").text_color(look.detail)),
            Some(Some(names)) => {
                let rows = names.iter().enumerate().map(|(index, name)| {
                    let on = !folders.left_out.contains(name);
                    let toggled = name.clone();
                    div()
                        .id(("import-folder", index))
                        .on_click(
                            cx.listener(move |view, _, _, cx| view.toggle_folder(&toggled, cx)),
                        )
                        .child(checked_row(
                            look,
                            on,
                            name.clone(),
                            look.accent,
                            rgb(CHECK_TICK),
                        ))
                });
                Some(group(look, rows.map(|row| div().child(row))))
            }
        };
        div()
            .flex()
            .flex_col()
            .gap(px(BOX_GAP))
            .child(disclosure)
            .children(chosen)
    }

    /// The favorites an import worked out from its history, with the line saying where they come
    /// from, the first launch's own, and a row each, ticked in the accent as Choose folders' are,
    /// which a click unticks.
    fn favorites_offer(&self, tabs: &[Tab], cx: &mut Context<Self>) -> gpui::Div {
        let look = &self.look;
        let here = match self.os {
            Os::Mac => "this Mac",
            _ => "this computer",
        };
        let rows = tabs.iter().enumerate().map(|(index, tab)| {
            let on = !self.unticked.contains(&tab.url);
            let url = tab.url.clone();
            div()
                .id(("import-favorite", index))
                .on_click(cx.listener(move |view, _, _, cx| view.toggle_favorite(&url, cx)))
                .child(checked_row(
                    look,
                    on,
                    suggested::site_name(&tab.url),
                    look.accent,
                    rgb(CHECK_TICK),
                ))
        });
        let said = format!(
            "The sites you visit most, from the history it brought, worked out on {here}. \
             Nothing is sent anywhere."
        );
        div()
            .flex()
            .flex_col()
            .gap(px(BOX_GAP))
            .child(line(said).text_color(look.detail))
            .child(group(look, rows.map(|row| div().child(row))))
    }

    /// A row for each browser found, with Import….
    fn browsers(&self, cx: &mut Context<Self>) -> gpui::Div {
        let look = &self.look;
        let rows = self.sources.iter().enumerate().map(|(index, source)| {
            let profiles: Vec<&str> = (source.profiles.iter())
                .map(|profile| profile.name.as_str())
                .collect();
            // check.sh clicks each where it's logged, as Settings and the first launch place it.
            let at = drawn(
                format!("import-from {}", source.name()),
                self.drawn_at.clone(),
            );
            if self.guided {
                return self.guided_row(index, source, &profiles, at, cx);
            }
            let choose = button(look, ("import-from", index), "Import…")
                .relative()
                .child(at)
                .on_click(cx.listener(move |view, _, _, cx| view.pick(index, cx)))
                .into_any_element();
            match profiles.is_empty() {
                true => row(source.name(), choose),
                false => detailed_row(look, source.name(), profiles.join(", "), Some(choose)),
            }
        });
        group(look, rows)
    }

    /// A browser's row in the guided list, which a click picks: the one picked on a plate, with
    /// the ↩ that brings it at its end.
    fn guided_row(
        &self,
        index: usize,
        source: &Source,
        profiles: &[&str],
        at: gpui::Canvas<()>,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let look = &self.look;
        let picked = index == self.picked_row;
        let enter = picked.then(|| {
            svg()
                .path("icons/corner-down-left.svg")
                .size(px(DISCLOSURE_GLYPH))
                .text_color(look.detail)
                .into_any_element()
        });
        let line = (!profiles.is_empty()).then(|| profiles.join(", ").into());
        let row = frame()
            .child(words(look, source.name(), line))
            .children(enter.map(|enter| end([enter])));
        let hovered = look.hovered;
        div().child(
            div()
                .id(("import-from", index))
                .relative()
                .rounded(px(radius::ROW))
                .when(picked, |row| row.bg(look.control))
                .when(!picked, |row| row.hover(move |row| row.bg(hovered)))
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.picked_row = index;
                    cx.notify();
                }))
                .child(row)
                .child(at),
        )
    }

    /// The guided list's files, as one quiet line: a bookmarks file or a passwords file, each a
    /// link, in the ink, to the system's open panel.
    fn file_links(&self, cx: &mut Context<Self>) -> gpui::Div {
        let look = &self.look;
        let said = "Another browser or computer? Bring a bookmarks file or a passwords file.";
        let range = |part: &str| {
            let start = said.find(part).unwrap_or_default();
            start..start + part.len()
        };
        let links = vec![range("bookmarks file"), range("passwords file")];
        let ink = gpui::HighlightStyle {
            color: Some(look.ink.into()),
            ..Default::default()
        };
        let styled = gpui::StyledText::new(said).with_highlights(
            links
                .iter()
                .map(|link| (link.clone(), ink))
                .collect::<Vec<_>>(),
        );
        let view = cx.entity().downgrade();
        let text = gpui::InteractiveText::new("import-files", styled).on_click(
            links,
            move |link, _, cx| {
                view.update(cx, |view, cx| match link {
                    0 => view.choose_bookmarks(cx),
                    _ => view.choose_passwords(cx),
                })
                .ok();
            },
        );
        div()
            .px(px(ROW_X))
            .flex()
            .justify_center()
            .text_size(px(text::SMALL))
            .text_color(look.detail)
            .child(text)
    }

    /// The rows for the files a browser exports, after the browsers: bookmarks, for one that isn't
    /// on this computer, and passwords, which no browser's files give, with where each browser
    /// found keeps their export.
    fn files(&self, cx: &mut Context<Self>) -> gpui::Div {
        let look = &self.look;
        let from = match self.sources.first() {
            Some(source) => source.browser.name,
            None => "another browser",
        };
        let choose = button(look, "import-passwords", "Choose File…")
            .relative()
            .child(drawn("import-passwords".into(), self.drawn_at.clone()))
            .on_click(cx.listener(|view, _, _, cx| view.choose_passwords(cx)));
        let row = detailed_row(
            look,
            "Passwords",
            format!("Export them from {from} as a file, then choose it here."),
            Some(choose.into_any_element()),
        );
        let choose = button(look, "import-bookmarks", "Choose File…")
            .relative()
            .child(drawn("import-bookmarks".into(), self.drawn_at.clone()))
            .on_click(cx.listener(|view, _, _, cx| view.choose_bookmarks(cx)));
        let bookmarks = detailed_row(
            look,
            "Bookmarks",
            "A bookmarks file, in HTML, which every browser exports.",
            Some(choose.into_any_element()),
        );
        let places = export_places(&self.sources, self.os)
            .into_iter()
            .map(|place| {
                line(place)
                    .text_size(px(text::SMALL))
                    .text_color(look.detail)
            });
        div()
            .flex()
            .flex_col()
            .child(group(look, [bookmarks, row]))
            .child(div().mt(px(BOX_GAP)).flex().flex_col().children(places))
    }

    /// The kinds the browser picked offers, each with what of it comes, once its files are read,
    /// as the choices say them, with the counts in **.
    pub(crate) fn choices(&self) -> Vec<(Kind, String)> {
        let Step::Pick { source, read, .. } = &self.step else {
            return Vec::new();
        };
        let here = match self.os {
            Os::Mac => "this Mac",
            _ => "this computer",
        };
        let kinds = read::kinds(&self.sources[*source]).iter();
        kinds
            .map(|&kind| match read {
                Some(read) => (kind, kind_line(kind, &read.tally, here)),
                None => (kind, "Counting…".to_string()),
            })
            .collect()
    }

    /// The line under the choices: the profiles what comes goes into, and that logins don't.
    pub(crate) fn note(&self) -> Option<String> {
        let Step::Pick {
            source,
            read: Some(read),
            ..
        } = &self.step
        else {
            return None;
        };
        Some(into_note(&read.into, self.sources[*source].browser.name))
    }

    /// What can come from the browser picked, a row a kind with its count and its switch, then
    /// its passwords, from a file it exports, and the profiles it goes into, with Back and
    /// Import.
    fn picking(&self, source: usize, cx: &mut Context<Self>) -> gpui::AnyElement {
        let look = &self.look;
        let Step::Pick {
            picked,
            read,
            passwords,
            ..
        } = &self.step
        else {
            return div().into_any_element();
        };
        let from = &self.sources[source];
        let rows = self.choices().into_iter().map(|(kind, said)| {
            let slid = match self.switched {
                Some((switched, at)) if switched == kind => {
                    (at.elapsed().as_secs_f32() / SLIDE).min(1.)
                }
                _ => 1.,
            };
            let on = picked.contains(&kind);
            let toggle = div()
                .id(("import-kind", kind as usize))
                .on_click(cx.listener(move |view, _, _, cx| view.toggle(kind, cx)))
                .child(switch(look, on, slid, look.accent));
            let chips = (read.as_ref())
                .filter(|_| kind == Kind::Spaces)
                .map(|read| chips(look, &read.found.spaces));
            frame()
                .child(glyph_plate(look, glyph(kind), on))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(div().truncate().child(kind.label()))
                        .child(marked(look, &said))
                        .children(chips),
                )
                .child(end([toggle.into_any_element()]))
        });
        // Drawn before Choose folders, which takes the context too.
        let mut rows: Vec<gpui::Div> = rows.collect();
        rows.push(self.passwords_row(from, passwords.as_ref(), cx));
        let any = !picked.is_empty() || passwords.as_ref().is_some_and(|(_, on)| *on);
        let import = self
            .control("import", "Import")
            .when(!any, |button| button.opacity(0.5))
            .when(any, |button| {
                button.on_click(cx.listener(|view, _, _, cx| view.import(cx)))
            });
        let import = div()
            .relative()
            .child(import)
            .child(drawn("import-go".into(), self.drawn_at.clone()));
        let back = self
            .control("import-back", "Back")
            .on_click(cx.listener(|view, _, _, cx| view.back(cx)));
        let chooser = picked
            .contains(&Kind::Bookmarks)
            .then(|| self.folder_chooser(cx));
        let note = self
            .note()
            .map(|note| marked(look, &note).mt_0().px(px(ROW_X)));
        // Its host's heading says where it's from, if it has one.
        let named = (!self.headed && !self.guided).then(|| line(format!("From {}", from.name())));
        div()
            .flex()
            .flex_col()
            .gap(px(BOX_GAP))
            .children(named)
            .child(group(look, rows))
            .children(note)
            .children(chooser)
            // Guided, the host's Import and Back are the step's.
            .when(!self.guided, |view| {
                view.child(buttons([
                    back.into_any_element(),
                    import.into_any_element(),
                ]))
            })
            .into_any_element()
    }

    /// The passwords' row among the kinds: where the browser exports them, with Choose File…, or
    /// the file chosen, with its switch. Chrome's importer reads the file once, as the import
    /// runs; the view never reads it.
    fn passwords_row(
        &self,
        from: &Source,
        passwords: Option<&(PathBuf, bool)>,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let look = &self.look;
        let (said, on, control) = match passwords {
            None => {
                let place = export_place(from.browser);
                let place = match place.split_once(" › ") {
                    Some((first, rest)) => format!("**{first}** › {rest}"),
                    None => format!("**{place}**"),
                };
                let choose = button(look, "import-pick-passwords", "Choose File…")
                    .relative()
                    .child(drawn("import-pick-passwords".into(), self.drawn_at.clone()))
                    .on_click(cx.listener(|view, _, _, cx| view.choose_picked_passwords(cx)));
                let said = format!("Export them from {} first: {place}", from.browser.name);
                (said, true, choose.into_any_element())
            }
            Some((file, on)) => {
                let toggle = div()
                    .id("import-pick-passwords-switch")
                    .on_click(cx.listener(|view, _, _, cx| view.toggle_passwords(cx)))
                    .child(switch(look, *on, 1., look.accent));
                let said = format!("**{}**, read by Chromium's importer", file_name(file));
                (said, *on, toggle.into_any_element())
            }
        };
        frame()
            .child(glyph_plate(look, "icons/key-round.svg", on))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(div().truncate().child("Passwords"))
                    .child(marked(look, &said)),
            )
            .child(end([control]))
    }
}

/// A line of text between the boxes with a button at its end.
fn detailed_line(look: &Look, said: SharedString, control: gpui::AnyElement) -> gpui::Div {
    div()
        .px(px(ROW_X))
        .flex()
        .items_center()
        .text_color(look.detail)
        .child(div().flex_1().min_w_0().child(said))
        .child(end([control]))
}

/// A line of text over or between the boxes, where the rows' text starts.
fn line(text: impl Into<SharedString>) -> gpui::Div {
    div().px(px(ROW_X)).child(text.into())
}

/// A row that says something, as long as it needs, which wraps.
fn said_row(said: impl Into<SharedString>) -> gpui::Div {
    frame().child(div().flex_1().min_w_0().child(said.into()))
}

/// The buttons under a group, at its end, 8 apart, the one that does what the state is for last.
fn buttons(controls: impl IntoIterator<Item = gpui::AnyElement>) -> gpui::Div {
    div()
        .mt(px(BUTTONS_TOP - BOX_GAP))
        .flex()
        .justify_end()
        .gap(px(CONTROL_GAP))
        .children(controls)
}

/// The glyph on a kind's plate, from the icons the app has.
fn glyph(kind: Kind) -> &'static str {
    match kind {
        Kind::Spaces => "icons/spaces.svg",
        Kind::Favorites => "icons/grid-2x2.svg",
        Kind::Bookmarks => "icons/bookmark.svg",
        Kind::OpenTabs => "icons/app-window.svg",
        Kind::ReadingList => "icons/book-open.svg",
        Kind::History => "icons/clock.svg",
        Kind::Addresses => "icons/map-pin.svg",
        Kind::Extensions => "icons/puzzle.svg",
    }
}

/// Where each browser found exports its passwords, once a browser, else the four most people on
/// `os` come from.
fn export_places(sources: &[Source], os: Os) -> Vec<String> {
    let mut browsers: Vec<&sources::Browser> = Vec::new();
    for source in sources {
        if !browsers.iter().any(|each| each.name == source.browser.name) {
            browsers.push(source.browser);
        }
    }
    if browsers.is_empty() {
        let four = match os {
            Os::Mac => ["Arc", "Chrome", "Firefox", "Safari"],
            Os::Windows => ["Chrome", "Edge", "Firefox", "Arc"],
            Os::Linux => ["Firefox", "Chrome", "Chromium", "Brave"],
        };
        let named = |name: &str| sources::BROWSERS.iter().find(|each| each.name == name);
        browsers = four.into_iter().filter_map(named).collect();
    }
    let place = |browser: &sources::Browser| format!("{}: {}", browser.name, export_place(browser));
    browsers.into_iter().map(place).collect()
}

/// Where `browser` exports its passwords to a file, the same on each system it runs on.
fn export_place(browser: &sources::Browser) -> &'static str {
    match (browser.name, browser.family) {
        ("Arc", _) => "arc://password-manager/settings › Export passwords",
        ("Brave", _) => "brave://password-manager/settings › Export passwords",
        ("Chrome" | "Chromium" | "Helium", _) => {
            "chrome://password-manager/settings › Export passwords"
        }
        (_, Family::Chromium) => "its password manager's settings › Export passwords",
        (_, Family::Firefox) => "about:logins › ••• › Export Passwords…",
        (_, Family::Safari) => "the Passwords app › File › Export All Passwords to File…",
    }
}

fn file_name(file: &Path) -> String {
    let name = file.file_name().unwrap_or(file.as_os_str());
    name.to_string_lossy().into_owned()
}

/// What the wait for Full Disk Access says: that it waits, or, once the app came back to the
/// front without it, what to do.
fn access_said(again: bool) -> String {
    match again {
        false => "Waiting for access…".into(),
        true => format!(
            "Still no access. If you turned it on, quit {app} and open it again.",
            app = env!("APP_NAME")
        ),
    }
}

/// `count` with its thousands apart, as "12,408".
fn grouped(count: usize) -> String {
    let digits = count.to_string();
    let mut said = String::new();
    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at) % 3 == 0 {
            said.push(',');
        }
        said.push(digit);
    }
    said
}

/// `count` of a thing, by its name for one and for more: "1 Space", "84 pinned tabs".
fn counted(count: usize, one: &str, many: &str) -> String {
    format!("{} {}", grouped(count), if count == 1 { one } else { many })
}

/// What of `kind` comes, by `tally`, as its choice says it, the counts in **: history kept on
/// `here`, this Mac or this computer.
pub(crate) fn kind_line(kind: Kind, tally: &read::Tally, here: &str) -> String {
    let days = read::HISTORY_DAYS;
    match kind {
        Kind::Spaces if tally.spaces == 0 => "No Spaces to bring".into(),
        Kind::Spaces => {
            let mut said = format!(
                "**{}** with **{}**",
                counted(tally.spaces, "Space", "Spaces"),
                counted(tally.pinned, "pinned tab", "pinned tabs")
            );
            if tally.folders > 0 {
                said += &format!(" in {}", counted(tally.folders, "folder", "folders"));
            }
            if tally.link_rules > 0 {
                let rules = counted(tally.link_rules, "link rule", "link rules");
                said += &format!(", and {rules}");
            }
            said
        }
        Kind::Favorites if tally.favorites == 0 => "None to bring".into(),
        Kind::Favorites => format!(
            "**{}**, at the top of each profile's Spaces",
            grouped(tally.favorites)
        ),
        Kind::History if tally.pages == 0 => format!("No pages from the last {days} days"),
        Kind::History => {
            let pages = if tally.pages == 1 { "page" } else { "pages" };
            let count = grouped(tally.pages);
            format!("**{count}** {pages} from the last {days} days, kept on {here}")
        }
        Kind::Addresses if tally.addresses == 0 => "No saved addresses. Cards never come.".into(),
        Kind::Addresses => {
            let addresses = if tally.addresses == 1 {
                "address"
            } else {
                "addresses"
            };
            let count = grouped(tally.addresses);
            format!("**{count}** saved {addresses}. Cards never come.")
        }
        Kind::Extensions if tally.extensions == 0 => "None to add from the Chrome Web Store".into(),
        Kind::Extensions => format!(
            "**{}** to add from the Chrome Web Store, one at a time",
            grouped(tally.extensions)
        ),
        Kind::Bookmarks if tally.bookmarks == 0 => "No bookmarks".into(),
        Kind::Bookmarks => {
            let mut said = format!("**{}**", counted(tally.bookmarks, "bookmark", "bookmarks"));
            if tally.bookmark_folders > 0 {
                let folders = counted(tally.bookmark_folders, "folder", "folders");
                said += &format!(" in {folders}");
            }
            said + ", as a Space's pinned tabs"
        }
        Kind::OpenTabs if tally.open_tabs == 0 => "No tabs open".into(),
        Kind::OpenTabs => format!(
            "**{}**, as that Space's Today tabs",
            counted(tally.open_tabs, "tab", "tabs")
        ),
        Kind::ReadingList if tally.reading_list == 0 => "Nothing in the Reading List".into(),
        Kind::ReadingList => format!(
            "**{}**, as a folder in that Space",
            counted(tally.reading_list, "page", "pages")
        ),
    }
}

/// `names` as a list, "A", "A and B", "A, B and C".
fn listed(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The line under a browser's choices: the profiles here that what comes goes into, by name,
/// those the import makes as `browser` has them, and that logins don't come.
pub(crate) fn into_note(into: &[(String, bool)], browser: &str) -> String {
    let logins = "Logins don't come: you sign in to each site again.";
    let bold = |made: bool| -> Vec<String> {
        (into.iter())
            .filter(|(_, new)| *new == made)
            .map(|(name, _)| format!("**{name}**"))
            .collect()
    };
    let (here, new) = (bold(false), bold(true));
    let words = [
        "no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    ];
    let made = match new.len() {
        0 => None,
        1 => Some(format!("a new profile, {}, as {browser} has it", new[0])),
        count => {
            let count = words
                .get(count)
                .map_or(count.to_string(), |word| word.to_string());
            Some(format!(
                "{count} new profiles, {}, as {browser} has them",
                listed(&new)
            ))
        }
    };
    match (here.is_empty(), made) {
        (true, None) => logins.into(),
        (false, None) => format!("Into {}. {logins}", listed(&here)),
        (true, Some(made)) => format!("Into {made}. {logins}"),
        (false, Some(made)) => format!("Into {} and {made}. {logins}", listed(&here)),
    }
}

/// A grey line under a row's name, its parts in ** in the row's ink at medium weight, as the
/// counts in a browser's choices.
fn marked(look: &Look, said: &str) -> gpui::Div {
    let mut plain = String::new();
    let mut runs = Vec::new();
    for (at, part) in said.split("**").enumerate() {
        let start = plain.len();
        plain.push_str(part);
        if at % 2 == 1 {
            let style = gpui::HighlightStyle {
                color: Some(look.ink.into()),
                font_weight: Some(gpui::FontWeight::MEDIUM),
                ..Default::default()
            };
            runs.push((start..plain.len(), style));
        }
    }
    div()
        .mt(px(DETAIL_TOP))
        .text_size(px(text::SMALL))
        .line_height(px(DETAIL_LINE))
        .text_color(look.detail)
        .child(gpui::StyledText::new(plain).with_highlights(runs))
}

/// Arc's Spaces by name, each on a chip with its colour's dot, as they'll come.
fn chips(look: &Look, spaces: &[found::Space]) -> gpui::Div {
    let chips = spaces.iter().map(|space| {
        let color = (space.theme.as_ref()).map_or(look.detail, |theme| rgb(theme.main_color()));
        div()
            .h(px(CHIP_HEIGHT))
            .px(px(CHIP_PADDING))
            .flex()
            .items_center()
            .gap(px(CHIP_GAP))
            .rounded(px(CHIP_RADIUS))
            .bg(look.control)
            .text_size(px(text::SMALL))
            .child(div().flex_none().size(px(DOT)).rounded_full().bg(color))
            .child(space.name.clone())
    });
    div()
        .mt(px(CHIP_GAP))
        .flex()
        .flex_wrap()
        .gap(px(CHIP_GAP))
        .children(chips)
}

/// What importing a passwords file brought in, into the profile named `into`, if there are more
/// than one.
fn passwords_summary(file: &Path, passwords: Passwords, into: Option<&str>) -> SharedString {
    let name = file_name(file);
    let count = |count: usize, one: &str, many: &str| match count {
        1 => format!("1 {one}"),
        count => format!("{count} {many}"),
    };
    let said = match passwords {
        Passwords::Imported {
            imported,
            conflicted,
            skipped,
        } => {
            let came = match imported {
                0 => "No passwords came in".to_string(),
                _ => format!("{} came in", count(imported, "password", "passwords")),
            };
            let into = into.map(|into| format!(" to {into}")).unwrap_or_default();
            let mut said = format!("{came}{into} from {name}.");
            if conflicted > 0 {
                let kept = match conflicted {
                    1 => "1 conflicted with a saved password, which was kept.".to_string(),
                    _ => format!("{conflicted} conflicted with saved passwords, which were kept."),
                };
                said = format!("{said} {kept}");
            }
            if skipped > 0 {
                let rows = count(skipped, "row wasn't a login", "rows weren't logins");
                said = format!("{said} {rows} it could read.");
            }
            said
        }
        Passwords::Unreadable => format!("Couldn't read {name}."),
        Passwords::NotExport => format!(
            "{name} isn't a passwords export: its first row needs url, username and password."
        ),
        Passwords::TooLarge => format!("{name} is larger than a passwords file can be."),
        Passwords::Failed => "The passwords couldn't be imported.".to_string(),
    };
    said.into()
}

/// What an import under way says: that it's reading, then what came in so far while the core adds
/// the history and addresses, or that a profile it waits for hasn't opened.
fn progress(name: &str, run: &Run) -> SharedString {
    if let Some(stalled) = &run.stalled {
        return stalled.clone();
    }
    if !run.placed {
        return format!("Importing from {name}…").into();
    }
    let adding = match (run.history_left > 0, run.addresses_left > 0) {
        (true, true) => "history and addresses",
        (true, false) => "history",
        _ => "addresses",
    };
    let counts = Counts {
        pages: run.pages,
        addresses: run.addresses,
        ..run.counts
    };
    match came(counts) {
        Some(what) => format!("{what} came in from {name}. Adding its {adding}…").into(),
        None => format!("Adding {name}'s {adding}…").into(),
    }
}

/// How much of each thing came in, such as "1 Space and 3 pinned tabs", or none for nothing.
fn came(counts: Counts) -> Option<String> {
    let count = |count: usize, one: &str, many: &str| match count {
        0 => None,
        1 => Some(format!("1 {one}")),
        count => Some(format!("{count} {many}")),
    };
    let parts: Vec<String> = [
        count(counts.spaces, "Space", "Spaces"),
        count(counts.pinned, "pinned tab", "pinned tabs"),
        count(counts.today, "open tab", "open tabs"),
        count(counts.favorites, "favorite", "favorites"),
        count(counts.link_rules, "link rule", "link rules"),
        count(counts.pages, "page of history", "pages of history"),
        count(counts.addresses, "address", "addresses"),
    ]
    .into_iter()
    .flatten()
    .collect();
    match parts.as_slice() {
        [] => None,
        [one] => Some(one.clone()),
        [rest @ .., last] => Some(format!("{} and {last}", rest.join(", "))),
    }
}

/// What an import brought in, as the view says it, with the `listed` extensions it lists to add.
fn summary(name: &str, counts: Counts, listed: usize) -> SharedString {
    let extensions = match listed {
        0 => None,
        1 => Some("1 extension".to_string()),
        listed => Some(format!("{listed} extensions")),
    };
    let Some(what) = came(counts) else {
        let said = match extensions {
            None => format!("Everything from {name} is here already."),
            Some(them) => format!("{them} from {name} to add from the Chrome Web Store."),
        };
        return said.into();
    };
    let with = extensions.map(|them| format!(", with {them} to add from the Chrome Web Store"));
    format!("{what} came in from {name}{}.", with.unwrap_or_default()).into()
}

impl Render for ImportView {
    fn render(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self
            .switched
            .is_some_and(|(_, at)| at.elapsed().as_secs_f32() < SLIDE)
        {
            window.request_animation_frame();
        }
        let step = match &self.step {
            Step::List => "list",
            Step::Pick { .. } => "pick",
            Step::Running { run, .. } if run.placed => "placed",
            Step::Running { .. } | Step::Passwords => "running",
            Step::NoAccess { .. } => "no access",
            // Delete File takes its row away and moves Done, so its end draws as a step of its own.
            Step::Done(_) if self.passwords_file.is_some() => "done with file",
            Step::Done(_) => "done",
        };
        if self.drawn != Some(step) {
            self.drawn = Some(step);
            self.drawn_at.borrow_mut().clear();
            log!("browser: import drawn {step}");
        }
        let look = self.look;
        let arrival = self.arrival(window, cx);
        let content = match &self.step {
            Step::List => self.list(cx),
            Step::Pick { source, .. } => self.picking(*source, cx),
            Step::Running { name, run, .. } => {
                // Cancel while there's still something it stops, and Try Again once a profile
                // hasn't opened, the one thing to do then.
                let cancel = (!run.placed).then(|| {
                    self.control("import-cancel", "Cancel")
                        .on_click(cx.listener(|view, _, _, cx| view.cancel(cx)))
                        .into_any_element()
                });
                let again = run.stalled.is_some().then(|| {
                    self.control("import-try-again", "Try Again")
                        .on_click(cx.listener(|view, _, _, cx| view.try_again(cx)))
                        .into_any_element()
                });
                // The cards and the rows after them, but for a stalled import's line, which
                // says why it waits, and a run with nothing to show yet.
                let said = (arrival.is_none() || run.stalled.is_some())
                    .then(|| group(&look, [said_row(progress(name, run))]));
                div()
                    .flex()
                    .flex_col()
                    .gap(px(BOX_GAP))
                    .children(said)
                    .children(arrival)
                    .when(cancel.is_some(), |column| {
                        column.child(buttons(cancel.into_iter().chain(again)))
                    })
                    .into_any_element()
            }
            Step::Passwords => {
                let said = said_row(self.running().unwrap_or_default());
                group(&look, [said]).into_any_element()
            }
            Step::NoAccess { source, again, .. } => self.no_access(*source, *again, cx),
            Step::Done(said) => {
                // The file holds the passwords in plain text, so the end offers to delete it, in
                // the row under what came in.
                let file = self.passwords_file.as_ref().map(|file| {
                    let delete = self
                        .control("import-delete-passwords", "Delete File")
                        .relative()
                        .child(drawn("import-delete".into(), self.drawn_at.clone()))
                        .on_click(cx.listener(|view, _, _, cx| view.delete_passwords_file(cx)));
                    let why = format!(
                        "{} holds your passwords in plain text. Delete it once they're in.",
                        file_name(file)
                    );
                    let why = div().flex_1().min_w_0().text_color(look.detail).child(why);
                    frame().child(why).child(end([delete.into_any_element()]))
                });
                let offered = self.offered();
                let favorites = offered.map(|tabs| self.favorites_offer(tabs, cx));
                let add = offered.map(|_| {
                    let any = !self.ticked().is_empty();
                    self.control("import-add-favorites", "Add Favorites")
                        .when(!any, |button| button.opacity(0.5))
                        .when(any, |button| {
                            button.on_click(cx.listener(|view, _, _, cx| view.add_ticked(cx)))
                        })
                        .into_any_element()
                });
                let done = self.control("import-done", "Done");
                let done = div()
                    .relative()
                    .child(done.on_click(cx.listener(|view, _, _, cx| view.done(cx))))
                    .child(drawn("import-done".into(), self.drawn_at.clone()))
                    .into_any_element();
                let extensions = self.extensions_box(cx);
                div()
                    .flex()
                    .flex_col()
                    .gap(px(BOX_GAP))
                    .child(group(
                        &look,
                        [said_row(said.clone())].into_iter().chain(file),
                    ))
                    .children(favorites)
                    .child(buttons([done].into_iter().chain(add)))
                    .children(extensions)
                    .into_any_element()
            }
        };
        let skip = (self.skip
            && matches!(
                self.step,
                Step::List | Step::Pick { .. } | Step::NoAccess { .. }
            ))
        .then(|| {
            button(&look, "import-skip", "Skip")
                .on_click(cx.listener(|_, _, _, cx| cx.emit(ImportEvent::Skipped)))
        });
        let escape = cx.listener(|view, event: &KeyDownEvent, _, cx| {
            let key = &event.keystroke;
            if key.key == "escape" && !key.modifiers.modified() && view.escape(cx) {
                cx.stop_propagation();
            }
        });
        div()
            .when_some(self.focus.as_ref(), |view, focus| {
                view.track_focus(focus).on_key_down(escape)
            })
            .flex()
            .flex_col()
            .text_color(look.ink)
            .child(content)
            .children(skip.map(|skip| div().mt(px(BUTTONS_TOP)).flex().justify_end().child(skip)))
    }
}
