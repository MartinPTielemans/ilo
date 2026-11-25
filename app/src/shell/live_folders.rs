//! Live Folders (#2979): a folder that its profile's GitHub pull requests fill, kept fresh by what
//! the user does and never by a timer, so an idle browser stays asleep.
//!
//! The core keeps which folders are live, with the folder, and fetches a list on the folder's
//! Space's profile's own session, at most once in two minutes for a profile however often it's
//! asked, but for Refresh. The app names the source, never a URL, and gets parsed items back, with
//! when the site gave them. What's here is what the sidebar decides: when to ask, what an answer
//! leaves, and which rows show.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use gpui::{
    AnyElement, Context, FontWeight, MouseButton, Role, SharedString, div, prelude::*, px, svg,
};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super::{
    FAVICON_DRAWN, FOLDER_INDENT, PLATE_INSET, ROW_PITCH, SECONDARY, Shell, Slot, Space,
    TITLE_END_INSET, plate, row_box, rows::with_host, title,
};
use crate::tokens::shell::live_folder::{DRAFT, FAILED_MARK, HEADING_HEIGHT, HEADING_X};
use crate::tokens::text;

/// What fills a Live Folder. A Space has at most one of each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LiveSource {
    /// The user's open pull requests on GitHub and those waiting on their review.
    PullRequests,
}

impl LiveSource {
    /// Every source, in the Space's menu's order.
    pub const ALL: [LiveSource; 1] = [LiveSource::PullRequests];

    /// Its item in the Space's menu's Live Folders ›.
    pub fn menu_title(self) -> &'static str {
        match self {
            LiveSource::PullRequests => "GitHub Pull Requests",
        }
    }

    /// The name a folder made for it starts with.
    pub fn folder_name(self) -> &'static str {
        match self {
            LiveSource::PullRequests => "Pull Requests",
        }
    }

    /// The glyph its folder shows in place of the folder's.
    pub fn glyph(self) -> &'static str {
        match self {
            LiveSource::PullRequests => "icons/git-pull-request.svg",
        }
    }

    /// Where its sign-in is, for the row a signed-out profile's folder shows.
    pub fn sign_in_url(self) -> &'static str {
        match self {
            LiveSource::PullRequests => "https://github.com/login",
        }
    }

    /// Whether a page committing at `url` in the folder's profile is likely to have changed its
    /// lists, as merging or opening a pull request on the site does.
    pub fn changed_by(self, url: &str) -> bool {
        match self {
            LiveSource::PullRequests => super::host(url) == "github.com",
        }
    }
}

/// Which of a folder's lists an item is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveList {
    /// Pull requests waiting on the user's review, first, as they're the ones waiting on them.
    ReviewRequests,
    /// Pull requests the user opened.
    Created,
}

impl LiveList {
    pub const ALL: [LiveList; 2] = [LiveList::ReviewRequests, LiveList::Created];

    pub fn heading(self) -> &'static str {
        match self {
            LiveList::ReviewRequests => "Review requests",
            LiveList::Created => "Created by me",
        }
    }
}

/// A pull request's state, which its row's glyph shows. The lists are of open ones, so one merged
/// or closed leaves them, and a tab opened from its row is then a tab like any other (#3009).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PullState {
    Open,
    Draft,
}

impl PullState {
    pub fn glyph(self) -> &'static str {
        match self {
            PullState::Open => "icons/git-pull-request.svg",
            PullState::Draft => "icons/git-pull-request-draft.svg",
        }
    }
}

/// An item of a folder's lists, as the core parsed it: its title is plain text.
#[derive(Clone, Debug, PartialEq)]
pub struct LiveItem {
    pub list: LiveList,
    /// `owner/name`.
    pub repo: SharedString,
    pub number: u32,
    pub title: SharedString,
    pub state: PullState,
}

impl LiveItem {
    /// Its page, which a press on its row opens as a tab in the folder.
    pub fn url(&self) -> String {
        format!("https://github.com/{}/pull/{}", self.repo, self.number)
    }

    /// Its row's second line, where a pinned row away from its URL shows its host.
    pub fn place(&self) -> SharedString {
        format!("{} #{}", self.repo, self.number).into()
    }

    fn key(&self) -> (SharedString, u32) {
        (self.repo.clone(), self.number)
    }
}

/// The core's answer to a fetch.
#[derive(Clone, Debug, PartialEq)]
pub enum LiveAnswer {
    /// Both lists, each in the site's order.
    Items(Vec<LiveItem>),
    /// The profile has no session on the site.
    SignedOut,
    /// Offline, a redirect, an answer that wasn't the site's or too big: the rows stay as they were.
    Failed,
}

/// What can ask for a folder's lists: each something the user did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveEvent {
    /// The window became active.
    WindowActive,
    /// The folder's Space was shown, asked after the switch's frames.
    SpaceShown,
    /// The folder was opened.
    Opened,
    /// The folder's Space was given another profile, whose account's lists it shows now.
    Profile,
    /// The folder was made, which each window hears from the core. Nothing kept is fetched, and a
    /// kept answer is under two minutes old, so it's no refresh.
    Made,
    /// A page from the source's site committed in a tab of the folder's profile.
    SiteCommitted,
    /// The folder's menu's Refresh: the user's own ask, which the core's two minutes don't hold
    /// back.
    Refresh,
}

/// A row of an open Live Folder under its other tabs.
#[derive(Clone, Debug, PartialEq)]
pub enum LiveRow {
    Heading(LiveList),
    Item {
        item: LiveItem,
        new: bool,
    },
    /// The tab opened from an item still listed, in the item's place: a row pressed stays put.
    Tab(u64),
    SignIn,
}

/// How an open Live Folder's rows are drawn under its other tabs: runs of its own rows, each so
/// tall, with the tabs opened from its listed items between them, each in its item's place. Run
/// `i` is above `tabs[i]`, so there's one run more than tabs. Made once for each answer and each
/// report of the Space's tabs, so a frame reads it and builds nothing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveLayout {
    pub runs: Vec<f32>,
    pub tabs: Vec<u64>,
    /// Its rows, each run's split from the next by its tab's.
    pub rows: Vec<LiveRow>,
    /// The item each tab directly in it at an item's URL stands for.
    pub items: HashMap<u64, LiveItem>,
}

/// A Live Folder's lists as last fetched.
#[derive(Clone, Debug, Default)]
pub struct LiveFolder {
    items: Vec<LiveItem>,
    /// When the site gave the rows, which is their age in the folder's tooltip.
    updated: Option<Instant>,
    /// The last fetch failed, so its glyph has the mark and its tooltip says so.
    failed: bool,
    signed_out: bool,
    /// Items that came after the first answer, until the user opens them.
    new: HashSet<(SharedString, u32)>,
    /// When the core says the site gave the rows, in milliseconds since the Unix epoch: the same
    /// answer again, kept by the core, changes nothing.
    answered: Option<i64>,
}

/// When the site gave an answer the core says it gave at `answered`, in milliseconds since the Unix
/// epoch: an answer the core kept from inside its two minutes is that old, not new.
pub fn answered_at(answered: i64) -> Instant {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let age = now.saturating_sub(Duration::from_millis(answered.max(0) as u64));
    let now = Instant::now();
    now.checked_sub(age).unwrap_or(now)
}

impl LiveFolder {
    /// The site's answer, given at `now`.
    pub fn answer(&mut self, answer: LiveAnswer, now: Instant) {
        match answer {
            LiveAnswer::Items(items) => {
                // The first answer has nothing new: what came before it isn't known.
                if self.updated.is_some() && !self.signed_out {
                    let before: HashSet<_> = self.items.iter().map(LiveItem::key).collect();
                    let came = items.iter().map(LiveItem::key);
                    self.new.extend(came.filter(|key| !before.contains(key)));
                }
                let now_listed: HashSet<_> = items.iter().map(LiveItem::key).collect();
                self.new.retain(|key| now_listed.contains(key));
                self.items = items;
                self.signed_out = false;
                self.failed = false;
                self.updated = Some(now);
            }
            LiveAnswer::SignedOut => {
                self.items.clear();
                self.new.clear();
                self.signed_out = true;
                self.failed = false;
                self.updated = Some(now);
            }
            LiveAnswer::Failed => self.failed = true,
        }
    }

    /// Whether `answer`, which the site gave at `answered`, is what the folder holds already.
    pub fn holds(&self, answer: &LiveAnswer, answered: i64) -> bool {
        match answer {
            LiveAnswer::Failed => self.failed,
            _ if self.failed || self.answered != Some(answered) => false,
            LiveAnswer::Items(items) => !self.signed_out && self.items == *items,
            LiveAnswer::SignedOut => self.signed_out,
        }
    }

    /// The item at `url` was opened, so it's no longer new.
    pub fn opened(&mut self, url: &str) {
        let item = self.items.iter().find(|item| item.url() == url);
        if let Some(key) = item.map(LiveItem::key) {
            self.new.remove(&key);
        }
    }

    pub fn failed(&self) -> bool {
        self.failed
    }

    /// How many review requests wait, which a closed folder's row ends with.
    pub fn waiting(&self) -> usize {
        (self.items.iter())
            .filter(|item| item.list == LiveList::ReviewRequests)
            .count()
    }

    /// Its rows under its other tabs, for which `opened` gives the tab open at an item's URL: each
    /// list under its heading, an opened item's tab in its place, or the sign-in row. A list with
    /// no rows has no heading.
    pub fn rows(&self, opened: impl Fn(&str) -> Option<u64>) -> Vec<LiveRow> {
        if self.signed_out {
            return vec![LiveRow::SignIn];
        }
        let mut rows = Vec::new();
        for list in LiveList::ALL {
            let at = rows.len();
            for item in (self.items.iter()).filter(|item| item.list == list) {
                rows.push(match opened(&item.url()) {
                    Some(tab) => LiveRow::Tab(tab),
                    None => LiveRow::Item {
                        item: item.clone(),
                        new: self.new.contains(&item.key()),
                    },
                });
            }
            if rows.len() > at {
                rows.insert(at, LiveRow::Heading(list));
            }
        }
        rows
    }

    /// The item a tab opened from the folder at `url` is, whose glyph and place its row shows.
    pub fn item_at(&self, url: &str) -> Option<&LiveItem> {
        self.items.iter().find(|item| item.url() == url)
    }

    /// What the folder's tooltip says after its name: the rows' age, and whether the last try
    /// failed. Nothing before the first answer, unless the first try failed.
    pub fn age(&self, now: Instant) -> Option<String> {
        let Some(updated) = self.updated else {
            return self.failed.then(|| "Couldn't refresh".to_string());
        };
        let minutes = now.saturating_duration_since(updated).as_secs() / 60;
        let age = match minutes {
            0 => "Updated just now".to_string(),
            1 => "Updated 1 min ago".to_string(),
            minutes if minutes < 60 => format!("Updated {minutes} min ago"),
            minutes if minutes < 120 => "Updated 1 hr ago".to_string(),
            minutes => format!("Updated {} hr ago", minutes / 60),
        };
        Some(match self.failed {
            true => format!("Couldn't refresh · {age}"),
            false => age,
        })
    }
}

/// How `space`'s Live Folders' rows are drawn, each, by `url`, the pinned URL of each tab in it.
pub(super) fn rooms(
    space: &Space,
    live: &HashMap<u64, LiveFolder>,
    url: impl Fn(u64) -> String,
) -> HashMap<u64, LiveLayout> {
    (space.folders.live.keys())
        .map(|&folder| {
            let mut layout = LiveLayout {
                runs: vec![0.],
                ..LiveLayout::default()
            };
            let Some(kept) = live.get(&folder) else {
                return (folder, layout);
            };
            // Each tab directly in it, by its pinned URL, read once.
            let tabs: Vec<(u64, String)> = (space.pinned.iter().copied())
                .filter(|&tab| space.folder_of(tab) == folder)
                .map(|tab| (tab, url(tab)))
                .collect();
            for (tab, at) in &tabs {
                if let Some(item) = kept.item_at(at) {
                    layout.items.insert(*tab, item.clone());
                }
            }
            // The tab open at an item, whose row stands for it, if there's one with a row of its
            // own: the first of its split, if it's in one.
            let opened = |item: &str| {
                (tabs.iter())
                    .find(|(tab, at)| at == item && !space.later(*tab))
                    .map(|&(tab, _)| tab)
            };
            layout.rows = kept.rows(opened);
            for row in &layout.rows {
                match row {
                    LiveRow::Tab(tab) => {
                        layout.tabs.push(*tab);
                        layout.runs.push(0.);
                    }
                    LiveRow::Heading(_) => *layout.runs.last_mut().unwrap() += HEADING_HEIGHT,
                    _ => *layout.runs.last_mut().unwrap() += ROW_PITCH,
                }
            }
            (folder, layout)
        })
        .collect()
}

impl Shell {
    /// The source filling `folder`, if it's a Live Folder of one of the window's Spaces.
    pub(super) fn live_source(&self, folder: u64) -> Option<LiveSource> {
        (self.spaces.iter()).find_map(|space| space.folders.live.get(&folder).copied())
    }

    /// The Live Folder of `space` that `source` fills, if it has one.
    pub(super) fn live_folder_of(&self, space: u64, source: LiveSource) -> Option<u64> {
        let space = self.spaces.iter().find(|each| each.id == space)?;
        (space.folders.live.iter()).find_map(|(&folder, &each)| (each == source).then_some(folder))
    }

    /// Whether `space` can have Live Folders: the user's own, in a window that isn't private. An
    /// agent's rows are its own tabs, and a Live Folder's are the user's account's.
    pub(super) fn offers_live_folders(&self, space: u64) -> bool {
        !self.private()
            && (self.spaces.iter()).any(|each| each.id == space && each.agent.is_empty())
    }

    /// Whether the Space shown has a Live Folder, which only then asks as it's shown.
    pub(super) fn shows_live(&self) -> bool {
        (self.shown_space()).is_some_and(|space| !space.folders.live.is_empty())
    }

    /// The Live Folders of the Space shown.
    fn shown_live(&self) -> Vec<u64> {
        let live = self
            .shown_space()
            .map(|space| space.folders.live.keys().copied());
        live.into_iter().flatten().collect()
    }

    /// Asks the core for the lists of each of `folders` on `event`. How often the site is asked is
    /// the core's: it answers what it has from inside its two minutes, but for Refresh.
    fn ask_live(&mut self, event: LiveEvent, folders: Vec<u64>, cx: &mut Context<Self>) {
        let refresh = event == LiveEvent::Refresh;
        for folder in folders {
            let Some(source) = self.live_source(folder) else {
                continue;
            };
            // check.sh reads this line.
            log!("browser: live folder {folder} fetch on {event:?}");
            self.browser.fetch_live_folder(folder, source, refresh, cx);
        }
    }

    /// The window became active: the Space shown's Live Folders may be out of date.
    pub(super) fn live_window_active(&mut self, cx: &mut Context<Self>) {
        let folders = self.shown_live();
        self.ask_live(LiveEvent::WindowActive, folders, cx);
    }

    /// The Space shown has finished coming in.
    pub(super) fn live_space_shown(&mut self, cx: &mut Context<Self>) {
        let folders = self.shown_live();
        self.ask_live(LiveEvent::SpaceShown, folders, cx);
    }

    /// A page committed at `url` in `tab`: one of the source's site in the Space shown, which is
    /// the tab's profile, may have changed its lists.
    pub(super) fn live_page_committed(&mut self, tab: u64, url: &str, cx: &mut Context<Self>) {
        let Some(space) = self.shown_space() else {
            return;
        };
        if !space.pinned.contains(&tab) && !space.today.contains(&tab) {
            return;
        }
        let folders = (space.folders.live.iter())
            .filter(|&(_, source)| source.changed_by(url))
            .map(|(&folder, _)| folder)
            .collect();
        self.ask_live(LiveEvent::SiteCommitted, folders, cx);
    }

    /// The core's answer for `folder`'s lists, which the site gave at `answered`, in milliseconds
    /// since the Unix epoch. Only its kind is logged: the rows are the user's account's. One the
    /// folder holds already, as each ask inside the core's two minutes gets, draws nothing.
    pub fn set_live_answer(
        &mut self,
        folder: u64,
        answer: LiveAnswer,
        answered: i64,
        cx: &mut Context<Self>,
    ) {
        let failed = answer == LiveAnswer::Failed;
        // A fetch waiting as the profile changed answers failed, never the old account's rows,
        // and the ask the change made was dropped while it waited: that one goes again.
        if self.live_reask.remove(&folder) && failed {
            self.ask_live(LiveEvent::Profile, vec![folder], cx);
            return;
        }
        let kept = self.live.get(&folder);
        if kept.is_some_and(|kept| kept.holds(&answer, answered)) {
            return;
        }
        // Rows older than those shown, as an ask that waited behind a Refresh gets, aren't news.
        let older = kept
            .and_then(|kept| kept.answered)
            .is_some_and(|shown| answered < shown);
        if !failed && older {
            return;
        }
        self.set_live_answer_at(folder, answer, answered_at(answered), cx);
        if let Some(kept) = self.live.get_mut(&folder).filter(|_| !failed) {
            kept.answered = Some(answered);
        }
    }

    /// The Spaces `spaces` were each given another profile: their Live Folders' rows were the old
    /// account's, so they go, a failed answer keeping none of them, and the new one's are asked.
    pub(super) fn live_profiles_changed(&mut self, spaces: &[u64], cx: &mut Context<Self>) {
        let folders: Vec<u64> = (self.spaces.iter())
            .filter(|space| spaces.contains(&space.id))
            .flat_map(|space| space.folders.live.keys().copied())
            .collect();
        if folders.is_empty() {
            return;
        }
        for folder in &folders {
            self.live.remove(folder);
            self.live_reask.insert(*folder);
        }
        self.update_live_rooms(cx);
        self.ask_live(LiveEvent::Profile, folders, cx);
    }

    /// The answer for `folder` as the site gave it at `at`.
    fn set_live_answer_at(
        &mut self,
        folder: u64,
        answer: LiveAnswer,
        at: Instant,
        cx: &mut Context<Self>,
    ) {
        if self.live_source(folder).is_none() {
            return;
        }
        let kind = match &answer {
            LiveAnswer::Items(items) => format!("items {}", items.len()),
            LiveAnswer::SignedOut => "signed out".into(),
            LiveAnswer::Failed => "failed".into(),
        };
        // check.sh reads this line.
        log!("browser: live folder {folder} answered {kind}");
        self.live.entry(folder).or_default().answer(answer, at);
        self.update_live_rooms(cx);
        cx.notify();
    }

    /// Sizes each Space's Live Folders' rows again, sliding the rows under the shown Space's to
    /// their new places, and forgets the lists of folders that are gone or no longer live.
    pub(super) fn update_live_rooms(&mut self, cx: &mut Context<Self>) {
        let before = self.layout();
        let browser = self.browser.clone();
        for space in &mut self.spaces {
            space.live_rooms = rooms(space, &self.live, |tab| browser.pinned_url(tab));
        }
        self.forget_live();
        if before != self.layout() {
            self.move_rows(before, super::Curve::Slide, cx);
        }
    }

    /// Lets go of the lists of folders that are gone or no longer live.
    pub(super) fn forget_live(&mut self) {
        if self.live.is_empty() && self.live_reask.is_empty() {
            return;
        }
        let spaces = &self.spaces;
        let live =
            |folder: &u64| (spaces.iter()).any(|space| space.folders.live.contains_key(folder));
        self.live.retain(|folder, _| live(folder));
        self.live_reask.retain(live);
    }

    /// Takes the core's report of `space`'s folders after `was`: a Live Folder made since asks
    /// for its lists at once, and one opened, as it's shown. Unless the Space was `filled` before,
    /// its Live Folders weren't made but brought back, as at launch.
    pub(super) fn live_folders_changed(
        &mut self,
        space: u64,
        was: &super::Folders,
        filled: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.spaces.iter().find(|each| each.id == space) else {
            return;
        };
        let made: Vec<u64> = (entry.folders.live.keys())
            .filter(|folder| filled && !was.live.contains_key(folder))
            .copied()
            .collect();
        let opened: Vec<u64> = (entry.folders.live.keys())
            .filter(|&&folder| {
                entry.is_open(folder) && was.folders.get(&folder).is_some_and(|&(_, open)| !open)
            })
            .copied()
            .collect();
        let shown = space == self.space;
        self.ask_live(LiveEvent::Made, made, cx);
        if shown {
            self.ask_live(LiveEvent::Opened, opened, cx);
        }
    }

    /// Makes `space` a Live Folder that `source` fills, or deletes the one it has, as its menu's
    /// item is ticked or not.
    pub(super) fn toggle_live_folder(
        &mut self,
        space: u64,
        source: LiveSource,
        cx: &mut Context<Self>,
    ) {
        if !self.offers_live_folders(space) {
            return;
        }
        match self.live_folder_of(space, source) {
            Some(folder) => self.delete_folder_now(folder, cx),
            None => {
                self.show_space(space, cx);
                self.browser
                    .create_live_folder(space, source, source.folder_name(), cx);
            }
        }
    }

    /// Refresh in the folder's menu, which the core's two minutes don't hold back.
    pub(super) fn refresh_live_folder(&mut self, folder: u64, cx: &mut Context<Self>) {
        self.ask_live(LiveEvent::Refresh, vec![folder], cx);
    }

    /// Convert to Folder: the tabs opened from it stay, in a plain folder in the order they're
    /// drawn, and its lists go. ⌘Z makes it a Live Folder again.
    pub(super) fn convert_live_folder(&mut self, folder: u64, cx: &mut Context<Self>) {
        let relive = self.convert_live_folder_now(folder, cx);
        self.keep(super::undo::What::CONVERT_TO_FOLDER, relive, cx);
    }

    /// Converts `folder`, and returns the step that makes it live again.
    pub(super) fn convert_live_folder_now(
        &mut self,
        folder: u64,
        cx: &mut Context<Self>,
    ) -> Option<super::undo::Edit> {
        let source = self.live_source(folder)?;
        // Its own Space's, which a redo with another Space shown isn't.
        let space = (self.spaces.iter()).find(|space| space.folders.live.contains_key(&folder));
        let listed = space.and_then(|space| space.live_rooms.get(&folder));
        let listed = listed.map(|layout| layout.tabs.clone()).unwrap_or_default();
        // The core keeps them in the order they were opened, so each goes to the folder's end.
        let end = space.map_or(0, |space| {
            (space.pinned.iter())
                .filter(|&&row| space.folder_of(row) == folder)
                .count()
        });
        for tab in listed {
            self.browser
                .move_tab(tab, true, folder, end.saturating_sub(1), cx);
        }
        self.browser.end_live_folder(folder, cx);
        Some(super::undo::Edit::Relive { folder, source })
    }

    /// Opens the item at `url` as a pinned tab at the end of `folder`, shown, as a pinned tab's
    /// first click does, which makes it an item seen.
    fn open_live_item(&mut self, folder: u64, url: String, cx: &mut Context<Self>) {
        let Some(space) = self.shown_space() else {
            return;
        };
        let index = (space.pinned.iter())
            .filter(|&&row| space.folder_of(row) == folder && !space.later(row))
            .count();
        log!("browser: command open-live-item via sidebar");
        if let Some(kept) = self.live.get_mut(&folder) {
            kept.opened(&url);
        }
        self.browser.open_at(&[url], true, folder, index, cx);
    }

    /// The item a tab opened from a Live Folder stands for: the tab directly in one, at an item's
    /// URL. Its row shows the item's glyph, title and place while the lists have it; once it has
    /// left them, merged or closed, the tab is a tab like any other, its page's title and icon.
    pub(super) fn live_item_of(&self, tab: u64) -> Option<&LiveItem> {
        (self.spaces.iter())
            .flat_map(|space| space.live_rooms.values())
            .find_map(|layout| layout.items.get(&tab))
    }

    /// The folder's glyph, the source's, marked after a refresh that failed.
    pub(super) fn live_glyph(&self, folder: u64, source: LiveSource) -> AnyElement {
        let failed = self.live.get(&folder).is_some_and(LiveFolder::failed);
        let glyph = svg()
            .path(source.glyph())
            .size(px(FAVICON_DRAWN))
            .text_color(self.palette.text(SECONDARY));
        let mark = failed.then(|| {
            div()
                .absolute()
                .right(px(-FAILED_MARK / 3.))
                .bottom(px(-FAILED_MARK / 3.))
                .size(px(FAILED_MARK))
                .rounded_full()
                .bg(self.window_fill())
                .child(
                    svg()
                        .path("icons/circle-alert.svg")
                        .size(px(FAILED_MARK))
                        .text_color(self.palette.text(SECONDARY)),
                )
        });
        div()
            .relative()
            .size(px(FAVICON_DRAWN))
            .child(glyph)
            .children(mark)
            .into_any_element()
    }

    /// How many review requests wait in `folder`, which its row ends with while it's closed.
    pub(super) fn live_waiting(&self, folder: u64) -> usize {
        self.live.get(&folder).map_or(0, LiveFolder::waiting)
    }

    /// What `folder`'s tooltip says after its name, if anything.
    pub(super) fn live_age(&self, folder: u64) -> Option<String> {
        self.live.get(&folder)?.age(Instant::now())
    }

    /// Run `run` of an open Live Folder's rows, `depth` folders in: its lists' headings and rows,
    /// or the sign-in row, between the tabs opened from its items, sliding with the rows round
    /// them.
    pub(super) fn live_block(&self, folder: u64, run: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(space) = self.shown_space() else {
            return div().into_any_element();
        };
        let source = self.live_source(folder).unwrap_or(LiveSource::PullRequests);
        let depth = space.listed_depth(folder) + 1;
        let layout = space.live_rooms.get(&folder);
        let room = (layout.and_then(|layout| layout.runs.get(run).copied())).unwrap_or_default();
        let inset = PLATE_INSET + depth as f32 * FOLDER_INDENT;
        let mut runs = (layout.map_or(&[][..], |layout| &layout.rows[..]))
            .split(|row| matches!(row, LiveRow::Tab(_)));
        let rows = runs.nth(run).unwrap_or_default();
        let rows = rows.iter().map(|row| match row {
            LiveRow::Heading(list) => div()
                .flex_none()
                .h(px(HEADING_HEIGHT))
                .pl(px(inset + HEADING_X))
                .flex()
                .items_center()
                .text_size(px(text::SMALL))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(self.palette.text(SECONDARY))
                .child(list.heading())
                .into_any_element(),
            LiveRow::Item { item, new } => self.live_row(folder, inset, item, *new, cx),
            LiveRow::SignIn => self.sign_in_row(source, inset, cx),
            LiveRow::Tab(_) => div().into_any_element(),
        });
        div()
            .id(SharedString::from(format!("live-{folder}-{run}")))
            .flex_none()
            .h(px(room))
            .overflow_hidden()
            .top(px(self.slide(Slot::Live(folder, run))))
            .flex()
            .flex_col()
            .when(self.a11y, |block| {
                block.role(Role::List).aria_label(source.folder_name())
            })
            .children(rows)
            .into_any_element()
    }

    /// An item's row: its state's glyph, its title, semibold while it's new, and its place under
    /// it. A press opens it in the folder.
    fn live_row(
        &self,
        folder: u64,
        inset: f32,
        item: &LiveItem,
        new: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let draft = item.state == PullState::Draft;
        let glyph = svg()
            .path(item.state.glyph())
            .size(px(FAVICON_DRAWN))
            .text_color(self.palette.text(SECONDARY));
        let name = title(&self.palette, item.title.clone(), false)
            .when(new, |name| name.font_weight(FontWeight::SEMIBOLD));
        let label = with_host(&self.palette, name, item.place()).pr(px(TITLE_END_INSET));
        let plate = plate(glyph, label)
            .when(draft, |plate| plate.opacity(DRAFT))
            .id("plate")
            .hover(|plate| plate.bg(self.palette.hovered_row()));
        row_box()
            .pl(px(inset))
            // Keyed by its item, not its place: an answer that moves the rows between press and
            // release doesn't open another.
            .id(SharedString::from(format!(
                "live-row-{:?}-{}",
                item.list,
                item.url()
            )))
            .when(self.a11y, |row| {
                row.role(Role::Link)
                    .aria_label(format!("{}, {}", item.title, item.place()))
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click({
                let item = item.clone();
                cx.listener(move |shell, _, _, cx| shell.open_live_item(folder, item.url(), cx))
            })
            .child(plate)
            .into_any_element()
    }

    /// The row a signed-out profile's folder has, which opens the site's sign-in in a Today tab;
    /// signing in there commits a page of the site, which fills the folder.
    fn sign_in_row(&self, source: LiveSource, inset: f32, cx: &mut Context<Self>) -> AnyElement {
        let glyph = svg()
            .path("icons/log-in.svg")
            .size(px(FAVICON_DRAWN))
            .text_color(self.palette.text(SECONDARY));
        let label = title(&self.palette, "Sign in to GitHub".into(), false).pr(px(TITLE_END_INSET));
        let plate = plate(glyph, label)
            .id("plate")
            .hover(|plate| plate.bg(self.palette.hovered_row()));
        row_box()
            .pl(px(inset))
            .id("live-sign-in")
            .when(self.a11y, |row| {
                row.role(Role::Link).aria_label("Sign in to GitHub")
            })
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |shell, _, _, cx| {
                log!("browser: command live-folder-sign-in via sidebar");
                shell
                    .browser
                    .open_at(&[source.sign_in_url().to_string()], false, 0, 0, cx);
            }))
            .child(plate)
            .into_any_element()
    }
}

#[cfg(test)]
#[path = "live_folders_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "live_folders_sidebar_tests.rs"]
mod sidebar_tests;
