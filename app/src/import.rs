//! Import from the other browsers on the machine (#453): one view, shown in Settings › Import and
//! as a step of the first launch, that lists the browsers found, the one used last first, and
//! brings in what the user picks from one: Arc's Spaces with their names, icons, themes and
//! profiles, their pinned tabs and folders, and its favorites; the other browsers' bookmarks, from
//! Chromium's family, Firefox's and Safari, as a Space named after the browser, with Safari's
//! Reading List; and every one's history. Saved passwords and cookies never come from another
//! browser's files: passwords come from a file the user exports, behind that browser's own
//! sign-in, which Chrome's password importer reads (bc_profile_import_passwords), and the user
//! signs in again to the rest.
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
pub mod sources;
pub mod suggested;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Instant, SystemTime};

use gpui::{App, Context, EventEmitter, SharedString, div, prelude::*, px, rgb, svg};

use crate::palette::NO_THEME_ACCENT;
pub use crate::settings_rows::Look;
use crate::settings_rows::{SLIDE, button, checked_row, group, row, switch};
use crate::theme::Theme;
use crate::tokens::settings::{
    BOX_GAP, BUTTONS_TOP, CHECK_TICK, DISCLOSURE_GAP, DISCLOSURE_GLYPH, PADDING,
};
use crate::tokens::text;
use found::{Found, FromProfile, Page, Tab};
use plan::{Add, Counts, Existing, Plan, ProfileRef, Target};
use read::{Kind, ReadError};
use sources::{Family, Os, Roots, Source};

/// What the import asks of the browser: the core's calls, which the tests stand in for.
pub trait Host {
    /// Where the other browsers' folders are: the user's, or a made-up home a check run gives.
    fn roots(&self) -> Roots;
    fn existing(&self) -> Existing;
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
    fn existing(&self) -> Existing {
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
    fn profile_loaded(&self, _: u64) -> bool {
        true
    }
    fn add_history(&self, _: u64, _: &[Page], _: Box<dyn FnOnce(usize, &mut App)>) -> bool {
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
    /// goes to has loaded.
    pub fn ready(&self, host: &dyn Host) -> bool {
        let spaces =
            (self.spaces.iter()).all(|&(space, profile)| host.space_profile(space) == profile);
        let history = (self.plan.history.iter())
            .all(|(profile, _)| host.profile_loaded(self.profile(*profile)));
        spaces && history
    }

    /// Adds the pinned tabs, folders and favorites, and returns the history for each profile, which
    /// the core adds away from the UI.
    pub fn finish(mut self, host: &dyn Host) -> Vec<(u64, Vec<Page>)> {
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
        }
        let existing = host.existing();
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
        (history.into_iter())
            .map(|(profile, pages)| (self.profile(profile), pages))
            .collect()
    }
}

/// What the view tells whoever shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum ImportEvent {
    /// An import began, which goes on while the view isn't shown; running() says so.
    Started,
    /// The import that began is over, and this says what came in, or why nothing did.
    Finished(SharedString),
    /// The user read what came in, and pressed Done.
    Done,
    /// The user went on without importing.
    Skipped,
    /// The import read the history, and suggested() has the favorites it offers.
    Suggested,
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

/// An import under way.
#[derive(Default)]
struct Run {
    counts: Counts,
    /// Made once the browser's files are read.
    job: Option<Job>,
    /// The profiles whose history the core is still adding.
    waiting: usize,
    /// Pages the core added, which leaves out those that were here.
    pages: usize,
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
    /// The system it runs on, for where the browsers it names export their passwords.
    os: Os,
    /// The bookmark folders of the browser picked, to choose from.
    folders: Folders,
    /// The favorites the last import offers from its history, once it's read.
    suggested: Option<Vec<Tab>>,
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
            look: Look::settings(),
            sources,
            step: Step::List,
            switched: None,
            passwords_file: None,
            drawn: None,
            os: roots.os,
            folders: Folders::default(),
            suggested: None,
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
            Step::Running { name, .. } => Some(format!("Importing from {name}…").into()),
            Step::Passwords => Some("Importing passwords…".into()),
            _ => None,
        }
    }

    /// Lists the browsers found again, as Settings does each time it shows the tab.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if matches!(self.step, Step::List | Step::Done(_)) {
            let suggested = self.suggested.take();
            *self = ImportView::new(self.host.clone(), self.skip).look(self.look);
            self.suggested = suggested;
            cx.notify();
        }
    }

    /// The favorites the last import offers from the history it read, the sites visited most;
    /// none until it's read them. An import that brought favorites, as Arc's does, offers none.
    pub fn suggested(&self) -> Option<&[Tab]> {
        self.suggested.as_deref()
    }

    /// Whether an import is reading what it'll offer, so suggested() comes later.
    pub fn suggesting(&self) -> bool {
        matches!(self.step, Step::Running { .. }) && self.suggested.is_none()
    }

    /// Offers `tabs` as an import's history would, for the first launch's tests.
    #[cfg(test)]
    pub(crate) fn offer(&mut self, tabs: Vec<Tab>, cx: &mut Context<Self>) {
        self.suggested = Some(tabs);
        cx.emit(ImportEvent::Suggested);
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

    fn pick(&mut self, source: usize, cx: &mut Context<Self>) {
        // check.sh waits for this line.
        log!("browser: import pick {}", self.sources[source].name());
        let picked = read::kinds(&self.sources[source]).to_vec();
        self.step = Step::Pick { source, picked };
        self.folders = Folders::default();
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
            self.switched = Some((kind, Instant::now()));
            cx.notify();
        }
    }

    /// Reads what was picked from the browser, away from the UI, then brings it in.
    fn import(&mut self, cx: &mut Context<Self>) {
        let (Step::Pick { source, picked } | Step::NoAccess { source, picked }) = &self.step else {
            return;
        };
        let (source, picked) = (*source, picked.clone());
        let from = self.sources[source].clone();
        let left_out = self.folders.left_out.clone();
        self.step = Step::Running {
            source: Some(source),
            name: from.name(),
            run: Run::default(),
        };
        self.suggested = None;
        cx.emit(ImportEvent::Started);
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
            view.update(cx, |view, cx| view.read(from, picked, found, cx))
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
        let existing = self.host.existing();
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
        cx.emit(ImportEvent::Suggested);
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
            "browser: import from {name}: {} Spaces, {} pinned tabs, {} favorites, {} pages",
            counts.spaces,
            counts.pinned,
            counts.favorites,
            counts.pages
        );
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

    /// Adds what goes in once the Spaces are in their profiles, else waits for the next change.
    fn advance(&mut self, cx: &mut Context<Self>) {
        let Step::Running { run, .. } = &mut self.step else {
            return;
        };
        let Some(job) = run.job.take_if(|job| job.ready(&*self.host)) else {
            let view = cx.entity().downgrade();
            self.host.when_spaces_change(Box::new(move |cx| {
                view.update(cx, |view, cx| view.advance(cx)).ok();
            }));
            return;
        };
        for (profile, pages) in job.finish(&*self.host) {
            let view = cx.entity().downgrade();
            let then = Box::new(move |added, cx: &mut App| {
                view.update(cx, |view, cx| view.history_added(added, cx))
                    .ok();
            });
            if self.host.add_history(profile, &pages, then) {
                run.waiting += 1;
            }
        }
        if run.waiting == 0 {
            self.complete(cx);
        }
    }

    fn history_added(&mut self, added: usize, cx: &mut Context<Self>) {
        let Step::Running { run, .. } = &mut self.step else {
            return;
        };
        run.pages += added;
        run.waiting = run.waiting.saturating_sub(1);
        if run.waiting == 0 {
            self.complete(cx);
        }
    }

    fn complete(&mut self, cx: &mut Context<Self>) {
        let Step::Running { name, run, .. } = &self.step else {
            return;
        };
        let counts = Counts {
            pages: run.pages,
            ..run.counts
        };
        self.finish(summary(name, counts, run.listed), cx);
    }

    fn finish(&mut self, said: impl Into<SharedString>, cx: &mut Context<Self>) {
        let said = said.into();
        // check.sh waits for this line.
        log!("browser: import done: {said}");
        // An import from a browser that ended before reading its history offers nothing.
        let reading = matches!(self.step, Step::Running { .. });
        self.step = Step::Done(said.clone());
        if reading && self.suggested.is_none() {
            self.suggested = Some(Vec::new());
            cx.emit(ImportEvent::Suggested);
        }
        cx.emit(ImportEvent::Finished(said));
        cx.notify();
    }

    /// Asks for the file of passwords, and imports it.
    fn choose_passwords(&mut self, cx: &mut Context<Self>) {
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
        self.step = Step::Running {
            source: None,
            name: name.clone(),
            run: Run::default(),
        };
        self.suggested = None;
        cx.emit(ImportEvent::Started);
        cx.notify();
        let read = cx.background_spawn(async move {
            let bytes = std::fs::read(&file).map_err(|error| error.to_string())?;
            html::bookmarks(&String::from_utf8_lossy(&bytes))
        });
        cx.spawn(async move |view, cx| {
            let pinned = read.await;
            view.update(cx, |view, cx| match pinned {
                Ok(pinned) => {
                    let space = found::Space {
                        name: "Bookmarks".into(),
                        profile: FromProfile::Default,
                        icon: String::new(),
                        theme: None,
                        pinned,
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
        self.step = Step::Passwords;
        cx.emit(ImportEvent::Started);
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
    fn delete_passwords_file(&mut self, cx: &mut Context<Self>) {
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

    fn done(&mut self, cx: &mut Context<Self>) {
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
            true => list.child(line(
                "No browser to import from was found on this computer.",
            )),
            false => list.child(self.browsers(cx)),
        };
        let extensions = self.extensions_box(cx);
        list.children(extensions)
            .child(self.files(cx))
            .into_any_element()
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
                false => row(None, extension.name.clone(), add),
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
            .px(px(PADDING))
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
            row(None, kind.label(), toggle.into_any_element())
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
                    .child(import),
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

/// What an import brought in, as the view says it, with the `listed` extensions it lists to add.
fn summary(name: &str, counts: Counts, listed: usize) -> SharedString {
    let count = |count: usize, one: &str, many: &str| match count {
        0 => None,
        1 => Some(format!("1 {one}")),
        count => Some(format!("{count} {many}")),
    };
    let parts: Vec<String> = [
        count(counts.spaces, "Space", "Spaces"),
        count(counts.pinned, "pinned tab", "pinned tabs"),
        count(counts.favorites, "favorite", "favorites"),
        count(counts.pages, "page of history", "pages of history"),
    ]
    .into_iter()
    .flatten()
    .collect();
    let extensions = count(listed, "extension", "extensions");
    let what = match parts.as_slice() {
        [] => {
            let said = match extensions {
                None => format!("Everything from {name} is here already."),
                Some(them) => format!("{them} from {name} to add from the Chrome Web Store."),
            };
            return said.into();
        }
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
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
            log!("browser: import drawn {step}");
        }
        let look = self.look;
        let content = match &self.step {
            Step::List => self.list(cx),
            Step::Pick { source, picked } => self.picking(*source, &picked.clone(), cx),
            Step::Running { .. } | Step::Passwords => {
                line(self.running().unwrap_or_default()).into_any_element()
            }
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
                let extensions = self.extensions_box(cx);
                div()
                    .flex()
                    .flex_col()
                    .gap(px(BOX_GAP))
                    .child(line(said.clone()))
                    .children(why)
                    .children(extensions)
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(PADDING))
                            .children(delete)
                            .child(done),
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
