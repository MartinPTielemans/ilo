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

use std::path::{Path, PathBuf};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use gpui::{App, Context, EventEmitter, SharedString, Task, div, prelude::*, px, rgb, svg};

use crate::palette::NO_THEME_ACCENT;
pub use crate::settings_rows::Look;
use crate::settings_rows::{SLIDE, button, checked_row, drawn, group, row, switch};
use crate::theme::Theme;
use crate::tokens::settings::{
    BOX_GAP, BUTTONS_TOP, CHECK_TICK, DISCLOSURE_GAP, DISCLOSURE_GLYPH, PADDING,
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
        let addresses = self.plan.addresses.iter().all(|(profile, _)| loaded(profile));
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
                && spaces.clone().all(|&(space, _)| host.space_profile(space) == profile)
        };
        names.filter(|&(_, &id)| !open(id)).map(|(name, _)| name.as_str()).collect()
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
    /// What can come from one, to pick from.
    Pick { source: usize, picked: Vec<Kind> },
    /// Reading the browser's files, then waiting for new profiles to load and for the history to
    /// go in.
    /// From the browser `source`, or from a file, with what it's from, by `name`.
    Running {
        source: Option<usize>,
        name: String,
        run: Run,
    },
    /// The system keeps the browser's files from this app until the user gives it Full Disk
    /// Access. Each time the app comes back to the front, it tries again.
    NoAccess { source: usize, picked: Vec<Kind> },
    /// Chrome's importer reading a file of passwords the user chose.
    Passwords,
    /// What came in, or why nothing did.
    Done(SharedString),
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
        }
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
            let (suggested, runs) = (self.suggested.take(), self.runs);
            *self = ImportView::new(self.host.clone(), self.skip).look(self.look);
            (self.suggested, self.runs) = (suggested, runs);
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
        let picked = read::kinds(&self.sources[source]).to_vec();
        self.step = Step::Pick { source, picked };
        self.folders = Folders::default();
        self.unticked.clear();
        cx.notify();
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
        let space = existing.spaces.iter().find(|space| Some(space.profile) == start);
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

    /// Reads what was picked from the browser, away from the UI, then brings it in.
    pub(crate) fn import(&mut self, cx: &mut Context<Self>) {
        let (Step::Pick { source, picked } | Step::NoAccess { source, picked }) = &self.step else {
            return;
        };
        let (source, picked) = (*source, picked.clone());
        let from = self.sources[source].clone();
        let left_out = self.folders.left_out.clone();
        let id = self.new_run();
        self.step = Step::Running {
            source: Some(source),
            name: from.name(),
            run: Run { id, ..Run::default() },
        };
        self.suggested = None;
        cx.notify();
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
            Err(ReadError::NoAccess) => {
                log!("browser: import from {name} needs full disk access");
                self.step = Step::NoAccess { source, picked };
                let view = cx.entity().downgrade();
                self.host.when_active(Box::new(move |cx| {
                    view.update(cx, |view, cx| {
                        // Unless the user went back from it meanwhile.
                        if matches!(view.step, Step::NoAccess { .. }) {
                            view.import(cx);
                        }
                    })
                    .ok();
                }));
                cx.emit(ImportEvent::Finished(
                    format!("{name} needs Full Disk Access.").into(),
                ));
                return cx.notify();
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
        let listed = found.extensions.iter().filter(|each| !each.blocks_ads()).count();
        if !found.extensions.is_empty() {
            let ids: Vec<&str> = found.extensions.iter().map(|each| each.id.as_str()).collect();
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
        log!("browser: import from {name}: {} link rules", counts.link_rules);
        let Step::Running { run, .. } = &mut self.step else {
            return;
        };
        run.listed = listed;
        if plan.is_empty() {
            return self.finish(summary(&name, Counts::default(), listed), cx);
        }
        run.counts = counts;
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
        // What the core says it added, into the run's count of it and of what it has left.
        type Count = fn(&mut Run) -> (&mut usize, &mut usize);
        let then = |count: Count, cx: &mut Context<Self>| {
            let view = cx.entity().downgrade();
            Box::new(move |added, cx: &mut App| {
                view.update(cx, |view, cx| view.added(count, added, cx)).ok();
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
        match run.history_left + run.addresses_left {
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
        match run.history_left + run.addresses_left {
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
        self.finish(summary(name, counts, run.listed), cx);
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
                view.update(cx, |view, cx| view.import_passwords(file, cx)).ok();
            }
        });
        self.host.clone().choose_file(Exported::Passwords, then, cx);
    }

    /// Asks for a bookmarks file, and imports it.
    fn choose_bookmarks(&mut self, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let then = Box::new(move |file: Option<PathBuf>, cx: &mut App| {
            if let Some(file) = file {
                view.update(cx, |view, cx| view.import_bookmarks(file, cx)).ok();
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
        let id = self.new_run();
        self.step = Step::Running {
            source: None,
            name: name.clone(),
            run: Run { id, ..Run::default() },
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
            view.update(cx, |view, cx| view.passwords_imported(&chosen, passwords, cx))
                .ok();
        });
        if !self.host.import_passwords(&file, then) {
            return self.finish("The passwords couldn't be imported.", cx);
        }
        // check.sh waits for this line.
        log!("browser: import passwords");
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
        cx.emit(ImportEvent::Done);
        cx.notify();
    }

    fn back(&mut self, cx: &mut Context<Self>) {
        self.step = Step::List;
        cx.notify();
    }

    /// Says why the browser's files can't be read, with the way to System Settings, where the
    /// user can let this app read them.
    fn no_access(&self, source: usize, cx: &mut Context<Self>) -> gpui::AnyElement {
        let look = &self.look;
        let name = self.sources[source].name();
        let open = button(look, "import-full-disk-access", "Open System Settings")
            .on_click(cx.listener(|view, _, _, _| view.host.open_full_disk_access()));
        let back = button(look, "import-back", "Back")
            .on_click(cx.listener(|view, _, _, cx| view.back(cx)));
        div()
            .flex()
            .flex_col()
            .gap(px(BOX_GAP))
            .child(line(format!(
                "macOS lets an app read {name}'s bookmarks and history only with Full Disk Access. \
                 Turn on {app} in System Settings › Privacy & Security › Full Disk Access, and the \
                 import goes on once you're back. If it doesn't, quit {app} and open it again.",
                app = env!("APP_NAME"),
            )))
            .child(
                div()
                    .mt(px(BUTTONS_TOP - BOX_GAP))
                    .flex()
                    .justify_end()
                    .gap(px(PADDING))
                    .child(back)
                    .child(open),
            )
            .into_any_element()
    }

    fn list(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let list = div().flex().flex_col().gap(px(BOX_GAP));
        let list = match self.sources.is_empty() {
            true => list.child(line("No browser to import from was found on this computer.")),
            false => list.child(self.browsers(cx)),
        };
        let extensions = self.extensions_box(cx);
        list.children(extensions).child(self.files(cx)).into_any_element()
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
                    detailed(look, extension.name.clone().into(), pinned.into(), add)
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
            let anyway = button(look, "import-add-ad-blocker", "Add Anyway").on_click(
                cx.listener(move |view, _, _, cx| {
                    for id in &ids {
                        view.add_extension(id, cx);
                    }
                }),
            );
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
            .px(px(PADDING))
            .flex()
            .items_center()
            .gap(px(DISCLOSURE_GAP))
            .text_color(look.detail)
            .on_click(cx.listener(|view, _, _, cx| view.choose_folders(cx)))
            .child("Choose folders")
            .child(svg().path(glyph).size(px(DISCLOSURE_GLYPH)).text_color(look.detail));
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
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.toggle_folder(&toggled, cx)
                        }))
                        .child(checked_row(
                            look,
                            on,
                            name.clone(),
                            rgb(NO_THEME_ACCENT),
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
                    rgb(NO_THEME_ACCENT),
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
            let choose = button(look, ("import-from", index), "Import…")
                .on_click(cx.listener(move |view, _, _, cx| view.pick(index, cx)));
            detailed(
                look,
                source.name().into(),
                profiles.join(", ").into(),
                choose.into_any_element(),
            )
        });
        group(look, rows)
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
            .on_click(cx.listener(|view, _, _, cx| view.choose_passwords(cx)));
        let row = detailed(
            look,
            "Passwords".into(),
            format!("Export them from {from} as a file, then choose it here.").into(),
            choose.into_any_element(),
        );
        let choose = button(look, "import-bookmarks", "Choose File…")
            .on_click(cx.listener(|view, _, _, cx| view.choose_bookmarks(cx)));
        let bookmarks = detailed(
            look,
            "Bookmarks".into(),
            "A bookmarks file, in HTML, which every browser exports.".into(),
            choose.into_any_element(),
        );
        let places = export_places(&self.sources, self.os).into_iter().map(|place| {
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

    fn picking(&self, source: usize, picked: &[Kind], cx: &mut Context<Self>) -> gpui::AnyElement {
        let look = &self.look;
        let name = self.sources[source].name();
        let rows = read::kinds(&self.sources[source]).iter().map(|&kind| {
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
                .child(switch(look, on, slid, rgb(NO_THEME_ACCENT)));
            row(kind.label(), toggle.into_any_element())
        });
        // Drawn before Choose folders, which takes the context too.
        let rows: Vec<gpui::Div> = rows.collect();
        let any = !picked.is_empty();
        let import = button(look, "import", "Import")
            .when(!any, |button| button.opacity(0.5))
            .when(any, |button| {
                button.on_click(cx.listener(|view, _, _, cx| view.import(cx)))
            });
        let back = button(look, "import-back", "Back")
            .on_click(cx.listener(|view, _, _, cx| view.back(cx)));
        let chooser = picked
            .contains(&Kind::Bookmarks)
            .then(|| self.folder_chooser(cx));
        div()
            .flex()
            .flex_col()
            .gap(px(BOX_GAP))
            .child(line(format!("From {name}")))
            .child(group(look, rows))
            .children(chooser)
            .child(
                div()
                    .mt(px(BUTTONS_TOP - BOX_GAP))
                    .flex()
                    .justify_end()
                    .gap(px(PADDING))
                    .child(back)
                    .child(div().relative().child(import).child(drawn(
                        "import-go".into(),
                        self.drawn_at.clone(),
                    ))),
            )
            .into_any_element()
    }
}

/// A row with a grey line under its name.
fn detailed(
    look: &Look,
    name: SharedString,
    detail: SharedString,
    control: gpui::AnyElement,
) -> gpui::Div {
    div()
        .px(px(PADDING))
        .py(px(PADDING))
        .flex()
        .items_center()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(div().truncate().child(name))
                .child(
                    div()
                        .truncate()
                        .text_size(px(text::SMALL))
                        .text_color(look.detail)
                        .child(detail),
                ),
        )
        .child(div().flex_none().ml(px(PADDING)).child(control))
}

/// A line of text between the boxes with a button at its end.
fn detailed_line(look: &Look, said: SharedString, control: gpui::AnyElement) -> gpui::Div {
    div()
        .px(px(PADDING))
        .flex()
        .items_center()
        .text_color(look.detail)
        .child(div().flex_1().min_w_0().child(said))
        .child(div().flex_none().ml(px(PADDING)).child(control))
}

/// A line of text over or between the boxes.
fn line(text: impl Into<SharedString>) -> gpui::Div {
    div().px(px(PADDING)).child(text.into())
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
        let content = match &self.step {
            Step::List => self.list(cx),
            Step::Pick { source, picked } => self.picking(*source, &picked.clone(), cx),
            Step::Running { name, run, .. } => {
                // Cancel where Done goes, while there's still something it stops.
                let cancel = (!run.placed).then(|| {
                    button(&look, "import-cancel", "Cancel")
                        .on_click(cx.listener(|view, _, _, cx| view.cancel(cx)))
                });
                let again = run.stalled.is_some().then(|| {
                    button(&look, "import-try-again", "Try Again")
                        .on_click(cx.listener(|view, _, _, cx| view.try_again(cx)))
                });
                div()
                    .flex()
                    .flex_col()
                    .gap(px(BOX_GAP))
                    .child(line(progress(name, run)))
                    .when(cancel.is_some(), |column| {
                        column.child(
                            div()
                                .flex()
                                .justify_end()
                                .gap(px(PADDING))
                                .children(cancel)
                                .children(again),
                        )
                    })
                    .into_any_element()
            }
            Step::Passwords => line(self.running().unwrap_or_default()).into_any_element(),
            Step::NoAccess { source, .. } => self.no_access(*source, cx),
            Step::Done(said) => {
                let done = button(&look, "import-done", "Done")
                    .on_click(cx.listener(|view, _, _, cx| view.done(cx)));
                // The file holds the passwords in plain text, so the end offers to delete it.
                let file = self.passwords_file.as_ref().map(|file| {
                    let delete = button(&look, "import-delete-passwords", "Delete File")
                        .on_click(cx.listener(|view, _, _, cx| view.delete_passwords_file(cx)));
                    let why = format!(
                        "{} holds your passwords in plain text. Delete it once they're in.",
                        file_name(file)
                    );
                    (line(why).text_color(look.detail), delete)
                });
                let (why, delete) = file.unzip();
                let offered = self.suggested.as_deref().filter(|tabs| !tabs.is_empty());
                let favorites = offered.map(|tabs| self.favorites_offer(tabs, cx));
                let add = offered.map(|_| {
                    let any = !self.ticked().is_empty();
                    button(&look, "import-add-favorites", "Add Favorites")
                        .when(!any, |button| button.opacity(0.5))
                        .when(any, |button| {
                            button.on_click(cx.listener(|view, _, _, cx| view.add_ticked(cx)))
                        })
                });
                let extensions = self.extensions_box(cx);
                div()
                    .flex()
                    .flex_col()
                    .gap(px(BOX_GAP))
                    .child(line(said.clone()))
                    .children(why)
                    .children(favorites)
                    .children(extensions)
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(PADDING))
                            .children(delete)
                            .children(add)
                            .child(div().relative().child(done).child(drawn(
                                "import-done".into(),
                                self.drawn_at.clone(),
                            ))),
                    )
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
        div()
            .flex()
            .flex_col()
            .text_color(look.ink)
            .child(content)
            .children(skip.map(|skip| div().mt(px(BUTTONS_TOP)).flex().justify_end().child(skip)))
    }
}
