//! The Library, which slides in from the left over the sidebar and pushes the card right without
//! resizing it (Arc): a column of its sections, Media, Downloads, Spaces and Archived Tabs so far,
//! and beside it the section's list, newest first under day headings, with a search field over it,
//! or for Spaces a card for each (docs/ux-spec.md › Archive and Downloads).

use std::{
    cell::RefCell,
    hash::{DefaultHasher, Hash, Hasher},
    rc::Rc,
    sync::Arc,
};
use web_time::Instant;

use gpui::{
    AnyElement, ClickEvent, Context, DispatchPhase, Div, DragMoveEvent, Entity, Focusable,
    FontWeight, HitboxBehavior, Hsla, Image, ListAlignment, ListState, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, Pixels, Point, RenderImage,
    SharedString, Stateful, StyledText, Subscription, WeakEntity, Window, canvas, div, img,
    linear_color_stop, linear_gradient, list, prelude::*, px, relative, svg,
};

use super::{
    Command, DRAG_START, Edit, FAVICON_SIZE, FOLDER_INDENT, FOOTER_HEIGHT, FORM_TEXT, MenuEntry,
    NAV, NAV_RADIUS, NAV_SIZE, Naming, PLATE_HEIGHT, PLATE_INSET, PLATE_RADIUS, SECONDARY,
    SPACE_DRAGGED, SPACE_SWATCH_SHOWN, Shell, Space, UNSELECTED_TITLE, URL_BUTTON_GLYPH,
    URL_BUTTON_SIZE, What,
    access::Accessible,
    dashed_square,
    display_face::{cap_top, display_text},
    divider_box, favicon, file_url, folder_icon,
    header::TipSide,
    host, keystroke_label, plate, row_box, selected_plate, space_icon_drawn,
    space_order::{Along, DraggedSpace},
    split_plate, spring_to, still_clear,
    task_cards::number as task_number,
    title,
};
use crate::agents::task::{ArchivedTask, TaskAction};
use crate::tokens::shell::EMOJI_SIZE;
use crate::tokens::shell::task::GLYPH as TASK_GLYPH;
use crate::{
    icon_picker::{self, Subject},
    menu::Look,
    palette::Palette,
    text_input::TextInput,
    theme_picker,
    tooltip::{Anchor, Tip},
};

use crate::tokens::shell::library::LIBRARY_WIDTH;
/// The push, on Arc's measured spring (1 run, rms 1.0 pt). The Library comes with it from its own
/// width to the left, so they meet as they stop.
pub(super) const LIBRARY_RESPONSE: f32 = 0.245;
pub(super) const LIBRARY_DAMPING: f32 = 0.92;
/// Opened or closed by a click, the sidebar under the Library fades out in 2 frames at 60 fps as
/// it comes, and back in over 130 ms from 80 ms after it starts to go (Arc on video, 2 opens and 2
/// closes).
const SIDEBAR_FADE_OUT: f32 = 0.033;
const SIDEBAR_FADE_IN_DELAY: f32 = 0.08;
const SIDEBAR_FADE_IN: f32 = 0.13;
/// Moved by fingers, and on after they let go, the sidebar recedes as the card goes: it fades out
/// by 0.48 of the way, and shrinks by 0.076 of its size over the way, about a point at the window's
/// middle 128 right of its edge, so it goes down and right at the top and up and right at the
/// bottom. The Library is one sheet, coming from its width to the left at the card's pace (Arc
/// 1.170 rec at 2×, frame by frame: Spaces with one Space and with three, both ways, and Media
/// closing).
const RECEDE_SPAN: f32 = 0.48;
const RECEDE_SCALE: f32 = 0.076;
pub(super) const RECEDE_X: f32 = 128.;
/// Fingers going on past where the Library opens take it on with the card, a quarter of their way
/// at first and never 80 (ours, as Arc's isn't recorded): about as far as the Space pages' rubber
/// band on a short pull, so it gives, and half as far on a long one, so it pulls back.
const LIBRARY_BAND: f32 = 80.;
const LIBRARY_BAND_STIFFNESS: f32 = 0.25;
/// How near, in points, the Library's panel counts as at rest where it opens, for the window's
/// buttons: half a pixel at 2×, so their swap from the panel's copy can't be seen.
const LIBRARY_REST_SLACK: f32 = 0.25;

use crate::palette::{STATE_HOVERED, STATE_PRESSED};
use crate::tokens::shell::library::COLUMN;
use crate::tokens::shell::library::COLUMN_EDGE;
use crate::tokens::shell::library::COLUMN_WIDTH;
pub(super) use crate::tokens::shell::library::LIBRARY_BACK_INSET;
use crate::tokens::shell::library::SECTION_GAP;
use crate::tokens::shell::library::SECTION_GLYPH;
use crate::tokens::shell::library::SECTION_GLYPH_UNSELECTED;
use crate::tokens::shell::library::SECTION_GLYPH_Y;
use crate::tokens::shell::library::SECTION_HEIGHT;
use crate::tokens::shell::library::SECTION_LABEL;
use crate::tokens::shell::library::SECTION_LABEL_HEIGHT;
use crate::tokens::shell::library::SECTION_LABEL_Y;
use crate::tokens::shell::library::SECTION_RADIUS;
use crate::tokens::shell::library::SECTION_WIDTH;
use crate::tokens::shell::library::SECTION_X;
use crate::tokens::shell::library::TITLE_CAP;
use crate::tokens::shell::library::TITLE_LINE;
use crate::tokens::shell::library::TITLE_SIZE;
use crate::tokens::shell::library::TITLE_X;
use crate::tokens::shell::library::{LIBRARY_PLATE, SECTION_HOVERED};

use crate::elevation::Level;
use crate::tokens::shell::library::CARD_BRUSH;
use crate::tokens::shell::library::CARD_BRUSH_COLOR;
use crate::tokens::shell::library::CARD_BRUSH_GLYPH;
use crate::tokens::shell::library::CARD_BRUSH_INSET;
use crate::tokens::shell::library::CARD_DROP_RING;
use crate::tokens::shell::library::CARD_FIELD_HEIGHT;
use crate::tokens::shell::library::CARD_FIELD_INSET;
use crate::tokens::shell::library::CARD_FIELD_PLATE;
use crate::tokens::shell::library::CARD_FIELD_RADIUS;
use crate::tokens::shell::library::CARD_GAP;
use crate::tokens::shell::library::CARD_NAME_COLOR;
use crate::tokens::shell::library::CARD_NAME_GAP;
use crate::tokens::shell::library::CARD_NAME_HEIGHT;
use crate::tokens::shell::library::CARD_NAME_TOP;
use crate::tokens::shell::library::CARD_RADIUS;
use crate::tokens::shell::library::CARD_SLOT;
use crate::tokens::shell::library::CARD_SLOT_DASH;
use crate::tokens::shell::library::CARD_SLOT_X;
use crate::tokens::shell::library::CARD_WIDTH;
use crate::tokens::shell::library::PLUS_AFTER;
use crate::tokens::shell::library::PLUS_GAP;
use crate::tokens::shell::library::PLUS_GLYPH;
use crate::tokens::shell::library::PLUS_SIZE;
use crate::tokens::shell::library::SPACES_INSET;
use crate::tokens::shell::library::SPACES_LEFT;
use crate::tokens::shell::library::SPACES_PAGE;
use crate::tokens::shell::library::{CARD_MORE_X, CARD_MORE_Y};

use crate::tokens::shell::library::SEARCH_GLYPH;
use crate::tokens::shell::library::SEARCH_GLYPH_X;
use crate::tokens::shell::library::SEARCH_HEIGHT;
use crate::tokens::shell::library::SEARCH_PLACEHOLDER;
use crate::tokens::shell::library::SEARCH_RADIUS;
use crate::tokens::shell::library::SEARCH_TEXT_X;
use crate::tokens::shell::library::SEARCH_TOP;
use crate::tokens::shell::library::SEARCH_WIDTH;
use crate::tokens::shell::library::SEARCH_X;
use crate::tokens::shell::library::{FILTER_GAP, FILTER_WIDTH, FILTER_X};

use crate::tokens::shell::library::ARCHIVE_TOP;
use crate::tokens::shell::library::ARCHIVE_WIDTH;
use crate::tokens::shell::library::ARCHIVED_FAVICON_X;
use crate::tokens::shell::library::ARCHIVED_LEFT;
use crate::tokens::shell::library::ARCHIVED_PITCH;
use crate::tokens::shell::library::ARCHIVED_PLATE;
use crate::tokens::shell::library::ARCHIVED_RADIUS;
use crate::tokens::shell::library::ARCHIVED_RIGHT;
use crate::tokens::shell::library::ARCHIVED_TEXT_X;
use crate::tokens::shell::library::HEADING_BOTTOM;
use crate::tokens::shell::library::HEADING_GAP;
use crate::tokens::shell::library::HEADING_HEIGHT;
use crate::tokens::shell::library::HEADING_TOP;
use crate::tokens::shell::library::HEADING_X;
use crate::tokens::shell::library::MEDIA_GAP;
use crate::tokens::shell::library::MEDIA_RADIUS;
/// Media's tiles a row, and each one's side, as the rows' width holds them.
const MEDIA_COLUMNS: usize = 3;
const MEDIA_TILE: f32 =
    (ARCHIVE_WIDTH - ARCHIVED_LEFT - ARCHIVED_RIGHT - (MEDIA_COLUMNS as f32 - 1.) * MEDIA_GAP)
        / MEDIA_COLUMNS as f32;
/// The file's own icon in a tile without a picture of it.
const MEDIA_ICON: f32 = 32.;
/// Set in the preview ids of captures, which downloads' ids never reach.
const CAPTURE_BIT: u64 = 1 << 63;
/// Rows laid out past the list's ends, so a scroll doesn't show them late.
const ARCHIVE_OVERDRAW: f32 = 300.;
use crate::tokens::shell::library::BUTTON_PROGRESS_BOTTOM;
use crate::tokens::shell::library::BUTTON_PROGRESS_HEIGHT;
use crate::tokens::shell::library::BUTTON_PROGRESS_WIDTH;
use crate::tokens::shell::library::PROGRESS_BOTTOM;
use crate::tokens::shell::library::PROGRESS_FILL;
use crate::tokens::shell::library::PROGRESS_HEIGHT;
use crate::tokens::shell::library::PROGRESS_TRACK;
use crate::tokens::text;

/// The headings, by how long ago a tab was archived: the starts of `Browser::day_starts`, then
/// before them.
const HEADINGS: [&str; 5] = [
    "Today",
    "Yesterday",
    "Earlier This Week",
    "Last Week",
    "Older",
];

/// A tab in the archive, as the browser reports it.
#[derive(Clone)]
pub struct Archived {
    pub id: u64,
    pub url: SharedString,
    pub title: SharedString,
    /// When it was archived, in milliseconds since the Unix epoch.
    pub archived: f64,
    /// Whether it closed in a Little ilo window, which the archive's filter picks out (Arc).
    pub little: bool,
}

/// A video a Media tile plays: its file's name, for the log, and the frame it shows, none before
/// its first.
pub(super) struct MediaVideo {
    name: SharedString,
    frame: Option<Arc<RenderImage>>,
}

/// A capture saved to the Library, as the browser lists them.
#[derive(Clone)]
pub struct Captured {
    pub path: SharedString,
    pub name: SharedString,
    /// When it was saved, in milliseconds since the Unix epoch.
    pub saved: f64,
}

/// The id a capture's preview is asked and kept by: from its path, so each window's is the same.
pub(super) fn capture_id(path: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    CAPTURE_BIT | hasher.finish()
}

/// Whether a file is an image, a video, or neither, by its name, as the Library's Media lists the
/// first two.
pub(super) fn is_video(name: &str) -> Option<bool> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    match extension.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "heic" | "heif" | "bmp" | "tif"
        | "tiff" | "svg" => Some(false),
        "mp4" | "m4v" | "mov" | "webm" | "mkv" | "ogv" => Some(true),
        _ => None,
    }
}

/// Where a download is.
#[derive(Clone, Copy, PartialEq)]
pub enum DownloadState {
    Running,
    Paused,
    Done,
    Cancelled,
    Failed,
}

impl DownloadState {
    /// For check.sh's log lines.
    fn name(self) -> &'static str {
        match self {
            DownloadState::Running => "running",
            DownloadState::Paused => "paused",
            DownloadState::Done => "done",
            DownloadState::Cancelled => "cancelled",
            DownloadState::Failed => "failed",
        }
    }
}

/// One of the profile's downloads, as the browser reports it.
#[derive(Clone)]
pub struct Download {
    pub id: u64,
    pub name: SharedString,
    pub url: SharedString,
    /// Where its file is once it's done, or empty until that's known.
    pub path: SharedString,
    /// The bytes it has, and the bytes it will have in all, or 0 while that isn't known.
    pub received: i64,
    pub total: i64,
    pub state: DownloadState,
    pub open_when_done: bool,
    /// The browser holds it as possibly harmful until it's kept.
    pub held: bool,
    /// When it started, in milliseconds since the Unix epoch.
    pub started: f64,
    /// The profile it's of, and whether it's of that profile's private session, which only that
    /// profile's private windows list, as only the regular windows list the rest.
    pub profile: u64,
    pub private: bool,
}

/// What `Browser::act_on_download` does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DownloadAction {
    /// Opens the file, or for one still running, opens it once it's done, or no longer.
    Open,
    /// Shows the file in Finder, or the file manager off the Mac.
    Show,
    Pause,
    Resume,
    Cancel,
    /// Takes it off the list, keeping a finished file and deleting any other.
    Remove,
    /// Keeps one the browser held.
    Keep,
}

/// What an item of a download's menu does.
#[derive(Clone, Copy, Debug, PartialEq)]
enum DownloadChoice {
    Act(DownloadAction),
    /// Moves the file to the Trash and takes it off the list.
    Trash,
    /// Takes the downloads that aren't running or paused off the list.
    Clear,
}

/// Where a download's Trash puts the file, by Windows' own name for it there (#2468).
const TRASH: &str = match cfg!(all(target_os = "windows", not(test))) {
    true => "Move to Recycle Bin",
    false => "Move to Trash",
};

/// The menu a right-click on `download`'s row shows, what can be done with it where it is, with
/// Clear List enabled if any download is `finished`.
fn download_menu_items(
    download: &Download,
    finished: bool,
) -> Vec<(MenuEntry, Option<DownloadChoice>)> {
    let item = |title: &'static str, choice| (MenuEntry::new(title, true), Some(choice));
    let act = |title: &'static str, action| item(title, DownloadChoice::Act(action));
    let mut items = match download.state {
        _ if download.held => {
            let delete = MenuEntry::new("Delete", true).destructive();
            vec![
                act("Keep", DownloadAction::Keep),
                (delete, Some(DownloadChoice::Act(DownloadAction::Remove))),
            ]
        }
        DownloadState::Running | DownloadState::Paused => vec![
            match download.state {
                DownloadState::Paused => act("Resume", DownloadAction::Resume),
                _ => act("Pause", DownloadAction::Pause),
            },
            act("Cancel", DownloadAction::Cancel),
            (
                MenuEntry::new("Open When Done", true).checked(download.open_when_done),
                Some(DownloadChoice::Act(DownloadAction::Open)),
            ),
        ],
        DownloadState::Done => vec![
            act("Open", DownloadAction::Open),
            // Off the Mac, Chrome's word for the file manager there.
            act(
                match super::OFF_THE_MAC {
                    true => "Show in Folder",
                    false => "Show in Finder",
                },
                DownloadAction::Show,
            ),
            (
                MenuEntry::new(TRASH, true).destructive(),
                Some(DownloadChoice::Trash),
            ),
        ],
        DownloadState::Cancelled | DownloadState::Failed => Vec::new(),
    };
    if !download.held {
        if !items.is_empty() {
            items.push((MenuEntry::line(), None));
        }
        items.push(act("Remove from List", DownloadAction::Remove));
    }
    items.push((
        MenuEntry::new("Clear List", finished),
        Some(DownloadChoice::Clear),
    ));
    items
}

/// A section of the Library.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Section {
    Media,
    Downloads,
    Spaces,
    Archive,
    /// The past tasks and sessions of the window's profile's agents (#1080).
    Agents,
    /// The window's profile's Boosts (#695).
    Boosts,
}

impl Section {
    fn name(self) -> &'static str {
        match self {
            Section::Media => "media",
            Section::Downloads => "downloads",
            Section::Spaces => "spaces",
            Section::Archive => "archive",
            Section::Agents => "agents",
            Section::Boosts => "boosts",
        }
    }
}

/// A line of a Space's card.
#[derive(Debug, PartialEq)]
enum Line {
    /// A folder, in this many folders.
    Folder(u64, usize),
    /// A tab, or a split view by its first tab, in this many folders.
    Tab(u64, usize),
    /// The line over the Today tabs, with Clear at its end.
    Divider,
}

impl Space {
    /// What its card lists: the sidebar's rows, pinned tabs and folders, then the Today tabs
    /// under their line, without the New Tab row between (Arc).
    fn card_lines(&self) -> Vec<Line> {
        let (pinned, today) = self.rows();
        let line = |row: u64| match self.is_folder(row) {
            true => Line::Folder(row, self.listed_depth(row)),
            false => Line::Tab(row, self.listed_depth(row)),
        };
        let divider = (!today.is_empty()).then_some(Line::Divider);
        let lines = pinned.into_iter().map(line).chain(divider);
        lines.chain(today.into_iter().map(line)).collect()
    }
}

/// The open Library, until it has closed.
pub(super) struct Library {
    search: Entity<TextInput>,
    /// The search the list shows, to skip notes from the field that leave its text as it was.
    query: String,
    items: Vec<Item>,
    list: ListState,
    /// What Archived Tabs lists, the Filter button's choice.
    only: Only,
    /// The downloads the log last said Downloads lists, for check.sh.
    logged: Option<String>,
    /// The past task whose page Agents shows in its list's place, by its id (#1080).
    opened: Option<String>,
    /// That page's sites, each with the agent's answer there for good: true for Always Allow,
    /// false for Never, None while it asks first.
    sites: Vec<(String, Option<bool>)>,
    /// Where the page's controls were drawn, as the log said, for check.sh to click them.
    drawn: Rc<RefCell<Vec<String>>>,
    /// The page's requests opened to show where each went and what it sent (#2326), by their
    /// past task's id and step, and those of them showing every line.
    steps_opened: Vec<(String, usize)>,
    steps_all: Vec<(String, usize)>,
    _search: Subscription,
}

/// What Archived Tabs lists, as its Filter button's menu offers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) enum Only {
    #[default]
    All,
    /// The tabs Little ilo windows closed (Arc).
    Little,
    /// The tasks dismissed from the sidebar's foot.
    Tasks,
}

impl Only {
    const MENU: [Only; 3] = [Only::All, Only::Little, Only::Tasks];

    fn label(self) -> &'static str {
        match self {
            Only::All => "All",
            Only::Little => "Little ilo",
            Only::Tasks => "Tasks",
        }
    }
}

/// A line of the section's list.
#[derive(Clone, Copy, PartialEq)]
enum Item {
    Heading(&'static str),
    /// The archived tab at this index of `Shell::archive`.
    Tab(usize),
    /// The task at this index of `Shell::archived_tasks`.
    Task(usize),
    /// The past task at this index of `Shell::agent_history`.
    Past(usize),
    /// Agents' Clear, over its list.
    Clear,
    /// A past task's page, opened from its row: the way back, its title, and what it did.
    PastHead(usize),
    /// The page's site at this index of `Library::sites`.
    PastSite(usize),
    /// The step of the past task at the first index, at the second of its steps.
    PastStep(usize, usize),
    /// The Boost at this index of `Shell::boost_list`.
    Boost(usize),
    /// The download at this index of `Shell::downloads`.
    Download(usize),
    /// A row of Media's tiles, from its left.
    Media([Option<Medium>; MEDIA_COLUMNS]),
}

/// A tile of the Library's Media.
#[derive(Clone, Copy, PartialEq)]
enum Medium {
    /// The capture at this index of `Shell::captures`.
    Capture(usize),
    /// The download at this index of `Shell::downloads`, an image or video.
    Download(usize),
}

/// The group a Space card's field is, for its parts to show under the pointer.
const FIELD: &str = "space-field";

impl Shell {
    /// The archive changed, all of it, newest first.
    pub fn set_archive(&mut self, archive: Vec<Archived>, cx: &mut Context<Self>) {
        // A tab closed since a task was dismissed is what ⇧⌘T reopens: the core's to reopen, as
        // the archive is its list of what closed, a task's pages too.
        let newest = archive.first().map_or(0., |tab| tab.archived);
        if self
            .closed_task
            .as_ref()
            .is_some_and(|(_, at)| newest >= *at)
        {
            self.closed_task = None;
        }
        self.archive = archive;
        let archive = &self.archive;
        self.page_favicons
            .retain(|url, _| archive.iter().any(|tab| tab.url == *url));
        if self.section == Section::Archive {
            self.filter(true, cx);
        }
    }

    /// The tasks in the Archive changed, all of them, the last to leave the foot first. A private
    /// window, which offers no tasks, lists none.
    pub fn set_archived_tasks(&mut self, mut tasks: Vec<ArchivedTask>, cx: &mut Context<Self>) {
        if self.one_space() {
            tasks.clear();
        }
        if tasks == self.archived_tasks {
            return;
        }
        self.archived_tasks = tasks;
        if self.section == Section::Archive {
            self.filter(true, cx);
        }
    }

    /// Whether the window lists `download`: a private window its own session's, as nothing else
    /// may see them, and a regular window those of every profile's own (#669).
    pub(super) fn lists_download(&self, download: &Download) -> bool {
        match self.private {
            true => download.private && Some(download.profile) == self.private_profile(),
            false => !download.private,
        }
    }

    /// A private window's profile, whose session it's of; none for a regular window.
    pub(super) fn private_profile(&self) -> Option<u64> {
        self.private
            .then(|| self.spaces.first().map(|space| space.profile))
            .flatten()
    }

    /// The Library's sections the window has: a private window's own downloads and their media,
    /// but none of the Spaces or archived tabs the regular windows keep (#669).
    pub(super) fn has_section(&self, section: Section) -> bool {
        // Settings' switch off takes away every way into Boosts, the list too.
        if section == Section::Boosts && crate::boosts::with(|boosts| boosts.all_off()) {
            return false;
        }
        !self.private || matches!(section, Section::Media | Section::Downloads)
    }

    /// The downloads changed, all of them, newest first: the window keeps those it lists.
    pub fn set_downloads(&mut self, downloads: Vec<Download>, cx: &mut Context<Self>) {
        let downloads: Vec<Download> = downloads
            .into_iter()
            .filter(|download| self.lists_download(download))
            .collect();
        // check.sh waits for these lines: a download's state, and how many there are.
        for download in &downloads {
            let before = self.downloads.iter().find(|each| each.id == download.id);
            if before.is_none_or(|before| before.state != download.state) {
                let state = download.state.name();
                log!(
                    "browser: download {} {state} {}",
                    download.id,
                    download.name
                );
            }
        }
        if downloads.len() != self.downloads.len() {
            log!("browser: downloads {}", downloads.len());
        }
        self.download_icons
            .retain(|id, _| downloads.iter().any(|each| each.id == *id));
        self.download_previews
            .retain(|id, _| id & CAPTURE_BIT != 0 || downloads.iter().any(|each| each.id == *id));
        let bar = bar_drawn(&self.downloads);
        let flying = self.flight.as_ref().map(|flight| flight.id);
        self.fly_new_download(&downloads, cx);
        self.downloads = downloads;
        self.recent_downloads_changed(cx);
        if matches!(self.section, Section::Downloads | Section::Media) {
            self.filter(true, cx);
        }
        // A download changes about twice a second, and shows only in its list, the footer's bar
        // and the icon flying in.
        let listed = self.library.is_some() && self.section == Section::Downloads;
        let flying = self.flight.as_ref().map(|flight| flight.id) != flying;
        if listed || flying || bar_drawn(&self.downloads) != bar {
            cx.notify();
        }
    }

    /// The captures saved to the Library changed, all of them, newest first. A private window lists
    /// none, as it saves none and the regular windows' aren't its session's.
    pub fn set_captures(&mut self, captures: Vec<Captured>, cx: &mut Context<Self>) {
        let captures = if self.private { Vec::new() } else { captures };
        self.download_previews.retain(|id, _| {
            id & CAPTURE_BIT == 0 || captures.iter().any(|each| capture_id(&each.path) == *id)
        });
        self.captures = captures;
        if self.section == Section::Media {
            self.filter(true, cx);
        }
        self.recent_downloads_changed(cx);
    }

    /// The icon `Browser::download_icon` asked for.
    pub fn set_download_icon(&mut self, id: u64, icon: Option<Arc<Image>>, cx: &mut Context<Self>) {
        if let Some(entry) = self.download_icons.get_mut(&id) {
            *entry = icon;
            cx.notify();
        }
    }

    /// The favicon `Browser::page_favicon` asked for.
    pub fn set_page_favicon(
        &mut self,
        url: SharedString,
        favicon: Option<Arc<Image>>,
        cx: &mut Context<Self>,
    ) {
        if let Some(entry) = self.page_favicons.get_mut(&url) {
            *entry = favicon;
            cx.notify();
        }
    }

    /// How far the Library pushes the card right of `card_left`, at the same width, or left of it
    /// on the right, where the Library stops at the window's edge: fingers past where it opens
    /// take nothing further there, with nothing past the edge to take it to.
    pub fn card_push(&self) -> f32 {
        if self.page_fullscreen {
            return 0.;
        }
        let out = match self.right {
            true => self.library_edge(),
            false => self.library_out(),
        };
        (self.library_width() - self.card_left()) * out
    }

    /// How wide the Library is with `section` in it: Spaces as wide as its cards, in this window.
    pub(super) fn section_width(&self, section: Section) -> f32 {
        match section {
            Section::Spaces => spaces_width(self.spaces.len(), f32::from(self.viewport.width)),
            _ => LIBRARY_WIDTH,
        }
    }

    /// How wide the Library is: its section's, and on its way to another's after a click on it, or
    /// as Spaces come and go.
    pub(super) fn library_width(&self) -> f32 {
        let to = self.section_width(self.section);
        let Some((from, start)) = self.library_resize else {
            return to;
        };
        let t = self.since(start);
        spring_to(t, from, to, 0., LIBRARY_RESPONSE, LIBRARY_DAMPING).map_or(to, |(width, _)| width)
    }

    /// How far the card has gone to the Library's edge, 0 not at all and 1 there, and how fast,
    /// while it moves on its own. None once it's at rest.
    fn library_step(&self) -> Option<(f32, f32)> {
        let (from, speed, start) = self.library_motion?;
        let to = if self.library_open { 1. } else { 0. };
        let t = self.since(start);
        spring_to(t, from, to, speed, LIBRARY_RESPONSE, LIBRARY_DAMPING)
    }

    /// How far the card has gone to the Library's edge: 0 not at all and 1 there. Fingers that
    /// move the Library hold it where they have it.
    pub(super) fn library_out(&self) -> f32 {
        if let Some(out) = self
            .swipe
            .as_ref()
            .and_then(|swipe| self.library_swiped(swipe))
        {
            return out;
        }
        match self.library_step() {
            Some((out, _)) => out,
            None => f32::from(u8::from(self.library_open)),
        }
    }

    /// Whether fingers have the Library, out of the sidebar's edge.
    fn library_swiping(&self) -> bool {
        self.swipe
            .as_ref()
            .and_then(|swipe| self.library_swiped(swipe))
            .is_some()
    }

    /// Whether the Library is on its way open or closed, on its own or with the fingers.
    pub(super) fn library_moving(&self) -> bool {
        self.library_motion.is_some() || self.library_swiping()
    }

    /// The click's fade the sidebar is in, if it is: what it faded from, and when.
    fn clicked_fade(&self) -> Option<(f32, Instant)> {
        self.sidebar_fade.filter(|_| !self.library_swiping())
    }

    /// The sidebar's opacity under the Library: receding with the card, or, after a click, gone
    /// fast as the Library comes and back late as it goes.
    pub(super) fn sidebar_opacity(&self) -> f32 {
        self.sidebar_opacity_at(self.clock.now())
    }

    /// The sidebar's opacity at `now`.
    pub(super) fn sidebar_opacity_at(&self, now: Instant) -> f32 {
        let Some((from, start)) = self.clicked_fade() else {
            return receded_opacity(self.library_out());
        };
        let shown = if self.library_open { 0. } else { 1. };
        let t = now.saturating_duration_since(start).as_secs_f32();
        let done = match self.library_open {
            true => t / SIDEBAR_FADE_OUT,
            false => (t - SIDEBAR_FADE_IN_DELAY) / SIDEBAR_FADE_IN,
        };
        if done >= 1. {
            shown
        } else {
            from + (shown - from) * done.max(0.)
        }
    }

    /// Whether a click's fade of the sidebar is still under way, which outlasts the card's motion
    /// under Reduce Motion.
    fn sidebar_fading(&self) -> bool {
        let shown = if self.library_open { 0. } else { 1. };
        self.clicked_fade().is_some() && self.sidebar_opacity() != shown
    }

    /// The sidebar's size under the Library, about `RECEDE_X` right of its edge: receding with the
    /// card, or full while a click fades it.
    pub(super) fn sidebar_scale(&self) -> f32 {
        match self.clicked_fade() {
            Some(_) => 1.,
            None => 1. - RECEDE_SCALE * self.library_out(),
        }
    }

    /// How far the Library's right edge has come to the card's: 0 at the window's edge and 1 there,
    /// where it stops, to go on past only with the card (`library_past`). It comes with the card.
    pub(super) fn library_edge(&self) -> f32 {
        self.library_out().min(1.)
    }

    /// Where the Library's column is across the window: its width to the left as it comes, and at
    /// the window's edge from where it opens on, the copy of the window's buttons on it over the
    /// window's own.
    pub(super) fn library_column_left(&self) -> f32 {
        (self.library_edge() - 1.) * self.library_width()
    }

    /// How far, in points, fingers past where the Library opens have taken it on with the card:
    /// nowhere on the right, where it opens at the window's edge (`card_push`).
    fn library_past(&self) -> f32 {
        match self.clicked_fade() {
            Some(_) => 0.,
            None if self.right => 0.,
            None => past_end(self.library_out(), self.library_width()),
        }
    }

    /// Where the Library's panel starts across the window. On the left its column comes in from
    /// the window's edge with the card's (`library_column_left`), and fingers past where it opens
    /// take it on with the card. On the right it follows the sidebar's Space pages in from the
    /// page side, as fingers going right take them: its right edge on the sidebar's left, both
    /// going right from the card's edge until the Library's reaches the window's (#2375).
    pub(super) fn library_x(&self) -> f32 {
        match self.right {
            true => f32::from(self.viewport.width) - self.library_width() - self.library_behind(),
            false => self.library_column_left() + self.library_past(),
        }
    }

    /// On the right, how far the sidebar's Space pages, and the Library after them, have yet to go
    /// right to where the Library opens: the card's edge's way in from the window's edge while
    /// it's shut, and none once it's open.
    fn library_behind(&self) -> f32 {
        (1. - self.library_edge()) * self.card_left()
    }

    /// How far right the sidebar has gone with the Library coming in after it on the right, from
    /// none to its whole way out of the window. The Library takes its place from the left, where
    /// the sidebar recedes in place on the left.
    pub(super) fn sidebar_shift(&self) -> f32 {
        match self.right {
            true => self.card_left() - self.library_behind(),
            false => 0.,
        }
    }

    /// The Library's visible span across the window on the right: from the card's edge, which it
    /// comes out from behind, to the window's.
    pub(super) fn library_shown_from(&self) -> f32 {
        f32::from(self.viewport.width) - self.card_left() - self.card_push()
    }

    /// The Library's opacity: what the sidebar under it has lost, on every way in and out. Arc's
    /// is a sheet that covers the sidebar up to its edge, but ours has no fill, and cut there the
    /// half-faded sidebar looks sliced along a line with nothing on it, so the sidebar fades out
    /// whole and the Library fades in over it. It takes the sidebar's opacity of the same frame, as
    /// a second read of the clock would leave the two not quite adding up.
    pub(super) fn library_opacity(sidebar_opacity: f32) -> f32 {
        1. - sidebar_opacity
    }

    /// Whether the Library's panel is at rest where it opens, or past it with the card, to within
    /// a pixel's half. Its column, and the window's buttons on it, then sit where the window's
    /// own do.
    pub(super) fn library_at_rest(&self) -> bool {
        panel_at_rest(self.library_edge(), self.library_width())
    }

    /// Opens the Library, or closes it, from wherever the card is, the sidebar fading.
    pub(super) fn set_library(&mut self, open: bool, cx: &mut Context<Self>) {
        // A Blank Window has no Library: what it shows is the regular windows'. A private window's is
        // its own session's (#669).
        if open == self.library_open || (open && (self.page_fullscreen || self.blank)) {
            return;
        }
        // A private window's Library opens on its downloads where it last showed a section it
        // hasn't.
        if open && !self.has_section(self.section) {
            self.section = Section::Downloads;
        }
        if open {
            self.hide_link(cx);
        }
        let speed = self.library_step().map_or(0., |(_, speed)| speed);
        let opacity = Some(self.sidebar_opacity());
        self.move_library(open, self.library_out(), speed, opacity, cx);
    }

    /// Sends the Library open or closed from `out` (`library_out`), going at `speed` a second, and
    /// the sidebar under it fading from `opacity`, or receding with the card without one.
    pub(super) fn move_library(
        &mut self,
        open: bool,
        out: f32,
        speed: f32,
        opacity: Option<f32>,
        cx: &mut Context<Self>,
    ) {
        // Under Reduce Motion the card is where it goes at once, and the sidebar still fades.
        self.library_motion = Some((out, speed, crate::motion::start_at(self.clock.now(), cx)));
        self.sidebar_fade = opacity.map(|opacity| (opacity, self.clock.now()));
        self.library_open = open;
        if open {
            self.conceal_now(cx);
            self.recent_downloads_gone(cx);
            if self.library.is_none() {
                self.show_section(self.section, cx);
            }
        }
        cx.notify();
    }

    /// Opens the Library on `section`, or shows it there, or closes it if it's there already.
    /// Nothing for a section the window hasn't.
    pub(super) fn view_section(&mut self, section: Section, cx: &mut Context<Self>) {
        if !self.has_section(section) {
            return;
        }
        if !self.library_open {
            self.section = section;
            return self.set_library(true, cx);
        }
        if section == self.section {
            return self.set_library(false, cx);
        }
        self.show_section(section, cx);
    }

    /// Manage Spaces… in a Space's menu: the Library on its Spaces, left there if it shows them.
    pub(super) fn manage_spaces(&mut self, cx: &mut Context<Self>) {
        if !(self.library_open && self.section == Section::Spaces) {
            self.view_section(Section::Spaces, cx);
        }
    }

    /// Opens the Library on its Spaces, for the preview, which has no pointer to pick it with.
    pub fn show_spaces(&mut self, cx: &mut Context<Self>) {
        self.view_section(Section::Spaces, cx);
    }

    /// Opens the Library on its Agents, for the preview.
    pub fn show_agents(&mut self, cx: &mut Context<Self>) {
        self.view_section(Section::Agents, cx);
    }

    /// Shows the Library's Downloads for Chrome, which asks after the user stopped a quit to let
    /// downloads finish. Unlike ⇧⌘J, it leaves the Library open when Downloads are shown already.
    pub fn show_downloads(&mut self, cx: &mut Context<Self>) {
        if !self.library_open || self.section != Section::Downloads {
            self.run(Command::ViewDownloads, "chrome", cx);
        }
    }

    /// Lists `section` in the Library, with an empty search field of its own.
    pub(super) fn show_section(&mut self, section: Section, cx: &mut Context<Self>) {
        // Ours: the Library goes to the new section's width on the push's spring, as Arc's
        // isn't recorded.
        if self.library.is_some() && self.section_width(section) != self.library_width() {
            self.library_resize = Some((
                self.library_width(),
                crate::motion::start_at(self.clock.now(), cx),
            ));
        }
        self.section = section;
        let placeholder = match section {
            Section::Media => "Search Media…",
            Section::Downloads => "Search Downloads…",
            Section::Spaces => "",
            Section::Archive => "Search Archive…",
            Section::Agents => "Search Agents…",
            Section::Boosts => "Search Boosts…",
        };
        let search = cx.new(|cx| {
            TextInput::new(placeholder, cx)
                .placeholder_color(self.palette.text(SEARCH_PLACEHOLDER))
                .caret_color(self.palette.accent)
                .selection_color(self.palette.selection())
        });
        let observe = cx.observe(&search, |shell, _, cx| shell.filter(false, cx));
        self.library = Some(Library {
            search,
            query: String::new(),
            items: Vec::new(),
            list: ListState::new(0, ListAlignment::Top, px(ARCHIVE_OVERDRAW)),
            only: Only::All,
            logged: None,
            opened: None,
            sites: Vec::new(),
            steps_opened: Vec::new(),
            steps_all: Vec::new(),
            drawn: Rc::default(),
            _search: observe,
        });
        // check.sh waits for these lines.
        log!("browser: library section {}", section.name());
        if section == Section::Media && !self.private {
            self.browser.list_captures(cx);
        }
        if section == Section::Spaces {
            self.reset_cards();
            let (count, width) = (self.spaces.len(), self.section_width(section));
            log!("browser: library spaces {count} wide {width}");
        }
        self.filter(true, cx);
    }

    /// Closes the Library at once, as a page goes full screen.
    pub(super) fn close_library_now(&mut self) {
        self.library = None;
        self.library_open = false;
        self.library_resize = None;
        self.library_motion = None;
        self.sidebar_fade = None;
    }

    /// Steps the Library's motion for a frame, and once it's done, says so and lets the closed
    /// Library go.
    pub(super) fn step_library(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The search field goes with the Library, so the keyboard goes back to the shell.
        if !self.library_open
            && let Some(library) = &self.library
            && library.search.focus_handle(cx).is_focused(window)
        {
            window.focus(&self.focus_handle, cx);
        }
        if self.library.is_none() {
            self.library_resize = None;
        }
        if self.step_cards() {
            window.request_animation_frame();
        }
        if let Some((from, start)) = self.library_resize {
            let (t, to) = (self.since(start), self.section_width(self.section));
            match spring_to(t, from, to, 0., LIBRARY_RESPONSE, LIBRARY_DAMPING) {
                Some(_) => window.request_animation_frame(),
                None => {
                    self.library_resize = None;
                    // check.sh waits for this before it clicks a section's contents.
                    log!("browser: library resized {to}");
                }
            }
        }
        if self.library_motion.is_none() {
            // Fingers that took the Library out a little, then paged instead, let it go.
            if !self.library_open && !self.library_swiping() {
                self.library = None;
            }
            return;
        }
        if self.library_step().is_some() || self.sidebar_fading() {
            window.request_animation_frame();
            return;
        }
        self.library_motion = None;
        self.sidebar_fade = None;
        // check.sh waits for these lines.
        if self.library_open {
            // check/sidebar-right.sh reads which side it opened on.
            let left = self.library_x();
            log!(
                "browser: library across {left:.0} to {:.0} of {:.0}",
                left + self.library_width(),
                f32::from(self.viewport.width)
            );
            log!("browser: library shown");
        } else {
            log!("browser: library hidden");
            self.library = None;
        }
    }

    /// Lists the archived tabs, downloads or media the search finds, under their headings. With
    /// `changed` false, only if the search did.
    fn filter(&mut self, changed: bool, cx: &mut Context<Self>) {
        if self.section == Section::Agents && self.library.is_some() {
            self.read_agent_history();
        }
        if self.section == Section::Boosts && self.library.is_some() {
            self.read_boosts(cx);
        }
        let Some(library) = &mut self.library else {
            return;
        };
        let query = library.search.read(cx).text().trim().to_lowercase();
        if !changed && query == library.query {
            return;
        }
        // A search goes back to the list, to find in it.
        if query != library.query {
            library.opened = None;
        }
        if self.section == Section::Agents
            && let Some(id) = library.opened.clone()
        {
            match self.agent_history.iter().position(|past| past.id == id) {
                Some(index) => {
                    let past = &self.agent_history[index];
                    let answers = match past.answers.is_empty() {
                        true => Vec::new(),
                        false => self.browser.agent_answers(&past.answers, cx),
                    };
                    library.sites = (past.sites().into_iter())
                        .map(|site| {
                            let answer = answers.iter().find(|(each, _)| *each == site);
                            (site, answer.map(|(_, may)| *may))
                        })
                        .collect();
                    let mut items = vec![Item::PastHead(index)];
                    if !library.sites.is_empty() {
                        items.push(Item::Heading("Sites"));
                        items.extend((0..library.sites.len()).map(Item::PastSite));
                    }
                    if !past.steps.is_empty() {
                        items.push(Item::Heading("Steps"));
                        items.extend(
                            (0..past.steps.len())
                                .rev()
                                .map(|step| Item::PastStep(index, step)),
                        );
                    }
                    let allowed: Vec<String> = (library.sites.iter())
                        .filter_map(|(site, answer)| match answer {
                            Some(true) => Some(format!("{site} allowed")),
                            Some(false) => Some(format!("{site} never")),
                            None => None,
                        })
                        .collect();
                    // check.sh reads this line.
                    log!(
                        "browser: agents history shows {id}: {} steps, {} sites, answers {}",
                        past.steps.len(),
                        library.sites.len(),
                        if allowed.is_empty() {
                            "none".to_string()
                        } else {
                            allowed.join(", ")
                        }
                    );
                    if items != library.items {
                        library.list.reset(items.len());
                        library.items = items;
                    }
                    library.query = query;
                    cx.notify();
                    return;
                }
                // Deleted, or gone with history: back to the list.
                None => library.opened = None,
            }
        }
        let found = |text: &SharedString| text.to_lowercase().contains(&query);
        let filtered = self.archive.iter().any(|tab| tab.little) || !self.archived_tasks.is_empty();
        let only = if filtered { library.only } else { Only::All };
        // Each with the time it's listed by, newest first.
        let found: Vec<(Item, f64)> = match self.section {
            Section::Spaces => Vec::new(),
            Section::Archive => {
                let tabs = (self.archive.iter().enumerate())
                    .filter(|(_, tab)| match only {
                        Only::All => true,
                        Only::Little => tab.little,
                        Only::Tasks => false,
                    })
                    .filter(|(_, tab)| query.is_empty() || found(&tab.title) || found(&tab.url))
                    .map(|(index, tab)| (Item::Tab(index), tab.archived));
                let tasks = (self.archived_tasks.iter().enumerate())
                    .filter(|_| matches!(only, Only::All | Only::Tasks))
                    .filter(|(_, task)| {
                        query.is_empty() || found(&task.title) || found(&task.asked)
                    })
                    .map(|(index, task)| (Item::Task(index), task.left));
                let mut listed: Vec<_> = tabs.chain(tasks).collect();
                listed.sort_by(|a, b| b.1.total_cmp(&a.1));
                listed
            }
            Section::Agents => (self.agent_history.iter().enumerate())
                .filter(|(_, past)| {
                    query.is_empty()
                        || found(&past.title.clone().into())
                        || found(&past.agent.clone().into())
                })
                .map(|(index, past)| (Item::Past(index), past.ended as f64 * 1000.)) // The Library's times are milliseconds.
                .collect(),
            // By site, without headings, as none is newer than another.
            Section::Boosts => (self.boost_list.iter().enumerate())
                .filter(|(_, boost)| query.is_empty() || found(&boost.site))
                .map(|(index, _)| (Item::Boost(index), 0.))
                .collect(),
            Section::Downloads => (self.downloads.iter().enumerate())
                .filter(|(_, each)| query.is_empty() || found(&each.name) || found(&each.url))
                .map(|(index, download)| (Item::Download(index), download.started))
                .collect(),
            Section::Media => {
                let tile = |medium| Item::Media([Some(medium), None, None]);
                let captures = (self.captures.iter().enumerate())
                    .filter(|(_, each)| query.is_empty() || found(&each.name))
                    .map(|(index, each)| (tile(Medium::Capture(index)), each.saved));
                let downloads = (self.downloads.iter().enumerate())
                    .filter(|(_, each)| {
                        each.state == DownloadState::Done && !each.held && !each.path.is_empty()
                    })
                    .filter(|(_, each)| is_video(&each.name).is_some())
                    .filter(|(_, each)| query.is_empty() || found(&each.name) || found(&each.url))
                    .map(|(index, each)| (tile(Medium::Download(index)), each.started));
                let mut media: Vec<_> = captures.chain(downloads).collect();
                media.sort_by(|a, b| b.1.total_cmp(&a.1));
                media
            }
        };
        let starts = self.browser.day_starts();
        let mut items = Vec::new();
        if self.section == Section::Agents && !found.is_empty() {
            items.push(Item::Clear);
        }
        let mut heading = None;
        for (item, time) in found {
            let day = starts.iter().position(|&start| time >= start).unwrap_or(4);
            if self.section != Section::Boosts && heading != Some(day) {
                heading = Some(day);
                items.push(Item::Heading(HEADINGS[day]));
            }
            // A tile goes into the row before it, under the same heading, while it has room.
            if let (Item::Media([Some(medium), ..]), Some(Item::Media(row))) =
                (item, items.last_mut())
                && let Some(free) = row.iter_mut().find(|slot| slot.is_none())
            {
                *free = Some(medium);
                continue;
            }
            items.push(item);
        }
        // Reset only for other lines, as it scrolls back to the top, and a running download
        // reports twice a second.
        if self.section == Section::Downloads {
            let names: Vec<&str> = (items.iter())
                .filter_map(|item| match item {
                    Item::Download(index) => Some(self.downloads[*index].name.as_ref()),
                    _ => None,
                })
                .collect();
            let names = names.join(", ");
            if library.logged.as_deref() != Some(names.as_str()) {
                // check.sh waits for this line: what the window's Downloads lists, once each time
                // that changes.
                log!("browser: library lists downloads: {names}");
                library.logged = Some(names);
            }
        }
        if items != library.items {
            library.list.reset(items.len());
            library.items = items;
        }
        library.query = query;
        cx.notify();
    }

    /// A click on the archived tab `id`'s row, at `at` in the window. It opens in a link preview
    /// beside the Library, which stays, as Arc's archive previews its tabs (the owner, 2026-10-29,
    /// #696): ⌘W, Esc or its Close leave it in the archive, and ⌘↩ or its Open as Tab restore it
    /// (link_preview). With ⌘, Ctrl off the Mac, it's restored at once, as before, and so it is
    /// where no preview can show it: a page that isn't http(s), previews off, or no tab whose page
    /// the window lays out for the preview to go over.
    fn open_archived(
        &mut self,
        id: u64,
        at: Point<Pixels>,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let Some(url) = (self.archive.iter())
            .find(|tab| tab.id == id)
            .map(|tab| tab.url.clone())
        else {
            return;
        };
        let web = url.starts_with("https://") || url.starts_with("http://");
        let page = self.preview_page().filter(|_| {
            web && self.selected != 0 && !modifiers.secondary(cx) && self.browser.link_previews()
        });
        let Some(page) = page else {
            return self.restore_archived(id, cx);
        };
        // check.sh waits for this line.
        log!("browser: archived {id} in preview");
        self.browser
            .open_archived_preview(self.selected, id, url, at, page, cx);
    }

    /// The archived tab a preview showed is the window's tab now, kept from the preview, and the
    /// Library closes as it does for one restored from its row.
    pub fn restored_from_preview(&mut self, cx: &mut Context<Self>) {
        self.set_library(false, cx);
    }

    /// The newest tab in the archive at `url`, its id and when it closed, which the command bar's
    /// row for the page reopens (#2337).
    pub fn archived_at(&self, url: &str) -> Option<(u64, f64)> {
        (self.archive.iter())
            .find(|tab| *tab.url == *url)
            .map(|tab| (tab.id, tab.archived))
    }

    /// The command bar's ↩ on a closed tab's row: it comes back as the Library's Reopen brings it,
    /// if it's still in the archive, and whether it was (#2337).
    pub fn reopen_archived(&mut self, id: u64, cx: &mut Context<Self>) -> bool {
        let kept = self.archive.iter().any(|tab| tab.id == id);
        if kept {
            self.restore_archived(id, cx);
        }
        kept
    }

    /// Opens the archived tab `id` at the top of the Space's Today tabs and closes the Library.
    fn restore_archived(&mut self, id: u64, cx: &mut Context<Self>) {
        // check.sh waits for this line.
        log!("browser: restore archived {id}");
        self.browser.restore_archived(id, cx);
        self.set_library(false, cx);
    }

    /// The task `agent` comes back from the Archive, as ⇧⌘T brings it: its card to the foot, with
    /// its conversation open, and the Library closes.
    fn restore_archived_task(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        // check.sh waits for this line.
        log!("browser: restore archived task {agent}");
        self.show_restored = Some(agent.clone());
        self.bring_back_task(agent, cx);
        self.set_library(false, cx);
    }

    /// Delete from Archive on a task: it leaves the Archive and ⇧⌘T's reach, a step of the
    /// window's ⌘Z history, as a tab's Delete from Archive is.
    fn delete_archived_task(&mut self, agent: SharedString, cx: &mut Context<Self>) {
        log!("browser: remove archived task {agent}");
        if self
            .closed_task
            .as_ref()
            .is_some_and(|(closed, _)| *closed == agent)
        {
            self.closed_task = None;
        }
        self.browser.task(agent.clone(), TaskAction::Delete, cx);
        self.keep(What::DELETE_ARCHIVED, Some(Edit::UndeleteTask(agent)), cx);
    }

    /// Whether Archived Tabs has the Filter button: while the archive holds a Little ilo tab or a
    /// task, which are what it picks out.
    fn archive_filtered(&self) -> bool {
        self.section == Section::Archive
            && (self.archive.iter().any(|tab| tab.little) || !self.archived_tasks.is_empty())
    }

    /// What the list shows, the Filter button's choice while it has the button.
    fn archive_only(&self) -> Only {
        match self.archive_filtered() {
            true => self
                .library
                .as_ref()
                .map_or(Only::All, |library| library.only),
            false => Only::All,
        }
    }

    /// The Filter button's menu: All, Little ilo's tabs or the tasks alone, the one shown checked.
    fn archive_filter_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let only = self.archive_only();
        let entries = (Only::MENU.iter())
            .map(|each| MenuEntry::new(each.label(), true).checked(*each == only))
            .collect();
        let shell = cx.weak_entity();
        self.browser.context_menu(
            entries,
            position,
            false,
            self.menu_look(),
            Box::new(move |chosen, cx| {
                if let Some(&only) = chosen.and_then(|chosen| Only::MENU.get(chosen)) {
                    shell
                        .update(cx, |shell, cx| shell.filter_archive(only, cx))
                        .ok();
                }
            }),
            cx,
        );
    }

    /// Lists only Little ilo's archived tabs, only the tasks, or all of them again.
    fn filter_archive(&mut self, only: Only, cx: &mut Context<Self>) {
        let Some(library) = &mut self.library else {
            return;
        };
        library.only = only;
        // check.sh waits for this line.
        let name = match only {
            Only::All => "all",
            Only::Little => "little",
            Only::Tasks => "tasks",
        };
        log!("browser: archive filter {name}");
        self.filter(true, cx);
    }

    /// The menu a right-click on a task's row shows: its conversation, as a click opens it, or
    /// out of the Archive.
    fn archived_task_menu(
        &mut self,
        agent: SharedString,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let entries = vec![
            MenuEntry::new("Open Conversation", true),
            MenuEntry::line(),
            MenuEntry::new("Delete from Archive", true).destructive(),
        ];
        let shell = cx.weak_entity();
        self.browser.context_menu(
            entries,
            position,
            false,
            self.menu_look(),
            Box::new(move |chosen, cx| {
                shell
                    .update(cx, |shell, cx| match chosen {
                        Some(0) => shell.restore_archived_task(agent.clone(), cx),
                        Some(2) => shell.delete_archived_task(agent.clone(), cx),
                        _ => {}
                    })
                    .ok();
            }),
            cx,
        );
    }

    /// The menu a right-click on an archived tab's row shows: reopen it, copy its URL, or take it
    /// out of the archive.
    pub(super) fn archived_menu(
        &mut self,
        id: u64,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(url) = self
            .archive
            .iter()
            .find(|tab| tab.id == id)
            .map(|tab| tab.url.clone())
        else {
            return;
        };
        let entries = vec![
            MenuEntry::new("Restore Tab", true),
            MenuEntry::new("Copy URL", true),
            MenuEntry::line(),
            MenuEntry::new("Delete from Archive", true).destructive(),
        ];
        let shell = cx.weak_entity();
        self.browser.context_menu(
            entries,
            position,
            false,
            self.menu_look(),
            Box::new(move |chosen, cx| {
                shell
                    .update(cx, |shell, cx| match chosen {
                        Some(0) => shell.restore_archived(id, cx),
                        Some(1) => {
                            let link = shell.copied_link(&url);
                            shell.copy(&link, "Copied URL", cx)
                        }
                        Some(3) => {
                            // check.sh waits for this line.
                            log!("browser: remove archived {id}");
                            shell.delete_archived(id, cx);
                        }
                        _ => {}
                    })
                    .ok();
            }),
            cx,
        );
    }

    /// The menu a right-click on the Archived Tabs section shows, Clear Archive as Arc's has it,
    /// which asks first as the Archive menu's does.
    fn archive_section_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let full = !self.archive.is_empty() || !self.archived_tasks.is_empty();
        let entries = vec![MenuEntry::new("Clear Archive", full)];
        let shell = cx.weak_entity();
        self.browser.context_menu(
            entries,
            position,
            false,
            Look {
                light: false,
                ..self.menu_look()
            },
            Box::new(move |chosen, cx| {
                if chosen == Some(0) {
                    shell
                        .update(cx, |shell, cx| {
                            shell.run(Command::ClearArchive, "library", cx)
                        })
                        .ok();
                }
            }),
            cx,
        );
    }

    /// Empties the archive, its tabs and the tasks it lists, once the user says so.
    pub(super) fn clear_archive(&mut self, cx: &mut Context<Self>) {
        let (tabs, tasks) = (self.archive.len(), self.archived_tasks.len());
        if tabs + tasks == 0 {
            return;
        }
        let shell = cx.weak_entity();
        self.browser.confirm(
            "Clear the archive?".into(),
            {
                let undo = keystroke_label("cmd-z");
                let some = |count: usize, what: &str| match count {
                    1 => format!("1 {what}"),
                    _ => format!("{count} {what}s"),
                };
                match (tabs, tasks) {
                    (1, 0) => format!("Its tab leaves the archive. {undo} brings it back."),
                    (0, 1) => format!("Its task leaves the archive. {undo} brings it back."),
                    (_, 0) => {
                        format!("Its {tabs} tabs leave the archive. {undo} brings them back.")
                    }
                    (0, _) => {
                        format!("Its {tasks} tasks leave the archive. {undo} brings them back.")
                    }
                    _ => format!(
                        "Its {} and {} leave the archive. {undo} brings them back.",
                        some(tabs, "tab"),
                        some(tasks, "task")
                    ),
                }
            },
            "Clear Archive",
            Box::new(move |cx| {
                shell.update(cx, |shell, cx| shell.empty_archive(cx)).ok();
            }),
            cx,
        );
    }

    /// The line a section's list holds while it has nothing to list: what fills it, or with a
    /// search, that nothing matches.
    fn empty_line(&self, section: Section, unsearched: bool) -> &'static str {
        let tasks = self.archive_only() == Only::Tasks;
        match (section, unsearched) {
            (Section::Spaces, _) => "",
            (Section::Archive, true) if tasks => "Finished tasks stay here until ilo quits.",
            (Section::Archive, false) if tasks => "No tasks match.",
            (Section::Archive, true) => "Closed Today tabs stay here for 30 days.",
            (Section::Archive, false) => "No archived tabs match.",
            (Section::Agents, true) => "Your agents' finished tasks stay here for 30 days.",
            (Section::Agents, false) => "No tasks match.",
            (Section::Boosts, true) => "New Boost makes one for the page shown.",
            (Section::Boosts, false) => "No Boosts match.",
            (Section::Downloads, true) => "Files you download show here.",
            (Section::Downloads, false) => "No downloads match.",
            (Section::Media, true) => "Saved captures and downloaded media show here.",
            (Section::Media, false) => "No media match.",
        }
    }

    /// The Library, where the sidebar was.
    pub(super) fn library_panel(
        &self,
        buttons: bool,
        sidebar_opacity: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let Some(library) = &self.library else {
            return div();
        };
        // The selected section's glyph has the Space's accent, as in Arc, where it reads there.
        let color = Hsla::from(self.palette.section_glyph);
        let sections = div()
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP))
            // Arc's buttons sit 5 above the column's middle.
            .mb(px(10.))
            .child(self.section_button(Section::Media, color, cx))
            .child(self.section_button(Section::Downloads, color, cx))
            .children(
                self.has_section(Section::Spaces)
                    .then(|| self.section_button(Section::Spaces, color, cx)),
            )
            .children(
                self.has_section(Section::Archive)
                    .then(|| self.section_button(Section::Archive, color, cx)),
            )
            .children(
                self.has_section(Section::Agents)
                    .then(|| self.section_button(Section::Agents, color, cx)),
            )
            .children(
                self.has_section(Section::Boosts)
                    .then(|| self.section_button(Section::Boosts, color, cx)),
            );
        // Taken on past where it opens, the column's fill reaches back to the window's edge, so
        // the window's own background never shows left of it.
        let past = self.library_past();
        let width = self.library_width();
        // On the right the columns swap, list then sections, so the sections and the window's
        // buttons on them stay at the window's edge, and what's across the list moves the column's
        // width left.
        let right = self.right;
        let list_x = |x: f32| match right {
            true => x - COLUMN_WIDTH,
            false => x,
        };
        let back = div()
            .id("library-back")
            .absolute()
            .map(|back| match right {
                true => back.right(px(LIBRARY_BACK_INSET)),
                false => back.left(px(LIBRARY_BACK_INSET + past)),
            })
            .bottom(px(LIBRARY_BACK_INSET))
            .size(px(NAV_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(NAV_RADIUS))
            .block_mouse_except_scroll()
            .hover(|style| style.bg(self.palette.plate(STATE_HOVERED)))
            .active(|style| style.bg(self.palette.plate(STATE_PRESSED)))
            .on_click(cx.listener(|shell, _, _, cx| shell.run(Command::ViewLibrary, "library", cx)))
            .child(
                svg()
                    .path("icons/arrow-left.svg")
                    .size(px(16.))
                    .text_color(self.palette.glyph(NAV)),
            );
        // Placed by its caps and its L's stem, not its line's box.
        let heading = display_text("Library", TITLE_SIZE, TITLE_LINE, cx)
            .absolute()
            .left(px(TITLE_X + past))
            .top(px(TITLE_CAP - cap_top(TITLE_SIZE, TITLE_LINE)))
            .text_color(self.palette.text(SECTION_LABEL));
        let column = div()
            .absolute()
            .left(px(if right { width - COLUMN_WIDTH } else { -past }))
            .top_0()
            .bottom_0()
            .w(px(COLUMN_WIDTH + past))
            .bg(self.palette.plate(COLUMN))
            .flex()
            .items_center()
            .map(|column| match right {
                true => column.justify_end().pr(px(SECTION_X)),
                false => column.pl(px(SECTION_X + past)),
            })
            .child(heading)
            .child(sections)
            .child(back)
            // Where the window's own sit, so they slide in with the panel, and stay put on the
            // column when fingers take it past its end; mirrored across it on the right.
            .children(buttons.then(|| self.window_buttons(px(COLUMN_WIDTH), cx)));
        let edge = div()
            .absolute()
            .left(px(match right {
                true => width - COLUMN_WIDTH - 1.,
                false => COLUMN_WIDTH,
            }))
            .top_0()
            .bottom_0()
            .w(px(1.))
            .bg(self.palette.faint(COLUMN_EDGE));
        let search = library.search.clone();
        // The Filter button takes the field's end, while there's something to filter.
        let filtered = self.archive_filtered();
        let field_width = if filtered {
            FILTER_X - FILTER_GAP - SEARCH_X
        } else {
            SEARCH_WIDTH
        };
        let field = div()
            .id("library-search")
            .absolute()
            .left(px(list_x(SEARCH_X)))
            .top(px(SEARCH_TOP))
            .w(px(field_width))
            .h(px(SEARCH_HEIGHT))
            .flex()
            .items_center()
            .pl(px(SEARCH_TEXT_X))
            .pr(px(12.))
            .rounded(px(SEARCH_RADIUS))
            .bg(self.palette.plate(LIBRARY_PLATE))
            .block_mouse_except_scroll()
            // The page may have the keyboard.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, _, window, cx| {
                    shell.browser.focus_ui(cx);
                    window.focus(&search.focus_handle(cx), cx);
                }),
            )
            .text_size(px(text::BODY))
            .text_color(self.palette.text(FORM_TEXT))
            .child(
                svg()
                    .path("icons/search.svg")
                    .absolute()
                    .left(px(SEARCH_GLYPH_X))
                    .top(px((SEARCH_HEIGHT - SEARCH_GLYPH) / 2.))
                    .size(px(SEARCH_GLYPH))
                    .text_color(self.palette.text(SEARCH_PLACEHOLDER)),
            )
            .child(library.search.clone());
        let filter = filtered.then(|| {
            let label = match self.archive_only() {
                Only::All => "Filter",
                only => only.label(),
            };
            div()
                .id("library-filter")
                .absolute()
                .left(px(list_x(FILTER_X)))
                .top(px(SEARCH_TOP))
                .w(px(FILTER_WIDTH))
                .h(px(SEARCH_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(SEARCH_RADIUS))
                .bg(self.palette.plate(LIBRARY_PLATE))
                .hover(|style| style.bg(self.palette.plate(STATE_HOVERED)))
                .active(|style| style.bg(self.palette.plate(STATE_PRESSED)))
                .block_mouse_except_scroll()
                .text_size(px(text::BODY))
                .text_color(self.palette.text(FORM_TEXT))
                .on_click(cx.listener(|shell, event: &ClickEvent, _, cx| {
                    shell.archive_filter_menu(event.position(), cx)
                }))
                .child(label)
        });
        let spaces = self.section == Section::Spaces;
        let archive = if library.items.is_empty() {
            let empty = self.empty_line(self.section, library.query.is_empty());
            // A block, not a flex row, whose text would keep its one line's width past the list's
            // edges: it wraps in the list's width, centred.
            let line = StyledText::new(empty);
            let laid = line.layout().clone();
            let drawn = self.empty_drawn.clone();
            let section = self.section.name();
            let log_drawn = canvas(
                |_, _, _| {},
                move |_, _, _, _| {
                    let bounds = laid.bounds();
                    let lines: usize = laid
                        .line_layouts()
                        .iter()
                        .map(|line| line.wrap_boundaries.len() + 1)
                        .sum();
                    let at = format!(
                        "{section} at {:.0} {:.0} lines {lines}",
                        f32::from(bounds.origin.x),
                        f32::from(bounds.size.width)
                    );
                    if drawn.replace(at.clone()) != at {
                        log!("browser: library empty {at}");
                    }
                },
            )
            .absolute()
            .size_full();
            div()
                .size_full()
                .pt(px(HEADING_TOP + 40.))
                .px(px(HEADING_X))
                .text_center()
                .text_size(px(text::SMALL))
                .text_color(self.palette.text(SECONDARY))
                .child(div().relative().child(line).child(log_drawn))
                .into_any_element()
        } else {
            let item =
                cx.processor(|shell, index, window, cx| shell.archive_item(index, window, cx));
            list(library.list.clone(), item)
                .size_full()
                .into_any_element()
        };
        // On the right it comes out from behind the card, which shows none of it past its edge,
        // in its insets or round its corners.
        let (clip, x) = match right {
            true => {
                let from = self.library_shown_from();
                (Some(from), self.library_x() - from)
            }
            false => (None, self.library_x()),
        };
        let panel = div()
            .absolute()
            .left(px(x))
            .top_0()
            .bottom_0()
            .w(px(width))
            .opacity(Shell::library_opacity(sidebar_opacity))
            .child(column)
            .child(edge);
        let panel = match spaces {
            true => panel.child(self.space_cards(cx)),
            false => panel
                .child(field)
                .children(filter)
                .child(
                    div()
                        .absolute()
                        .left(px(list_x(COLUMN_WIDTH)))
                        .top(px(ARCHIVE_TOP))
                        .bottom_0()
                        .w(px(ARCHIVE_WIDTH))
                        .child(archive),
                )
                .children(self.download_press_listeners(cx)),
        };
        match clip {
            Some(from) => div()
                .absolute()
                .left(px(from))
                .top_0()
                .bottom_0()
                .right_0()
                .overflow_hidden()
                .child(panel),
            None => panel,
        }
    }

    /// The Spaces section: a card for each Space of the window's profile set, in the switcher's
    /// order, and the + after them, which scroll across together if they don't fit (`card_strip`),
    /// clipped at the Library's edge.
    fn space_cards(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        // A card dragged by its handle dims in its place, and the cards glide to their places once
        // the Spaces' order changes, as the switcher's dots do (#1580).
        let cards: Vec<_> = self
            .spaces
            .iter()
            .map(|space| {
                let id = space.id;
                self.space_card(space, cx)
                    .left(px(self.space_slide(Along::Cards, id)))
                    .when(self.space_dragged(id), |card| card.opacity(SPACE_DRAGGED))
            })
            .collect();
        let plus = div()
            .id("library-new-space")
            .flex_none()
            .ml(px(PLUS_GAP - CARD_GAP))
            .mr(px(PLUS_AFTER))
            .size(px(PLUS_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(self.palette.accent)
            .hover({
                let hovered = self.palette.accent_hovered();
                move |style| style.bg(hovered)
            })
            .active({
                let pressed = self.palette.accent_pressed();
                move |style| style.bg(pressed)
            })
            .block_mouse_except_scroll()
            .on_click(cx.listener(|shell, _, _, cx| {
                shell.run(Command::NewSpace, "library", cx);
                shell.set_library(false, cx);
            }))
            .child(
                svg()
                    .path("icons/nav-plus.svg")
                    .size(px(PLUS_GLYPH))
                    .text_color(self.palette.accent_label),
            );
        // On the right the cards are left of the column, still in the switcher's order, left to
        // right, as Spaces read.
        let (left, right) = match self.right {
            true => (0., COLUMN_WIDTH),
            false => (COLUMN_WIDTH, 0.),
        };
        div()
            .id("library-spaces")
            .absolute()
            .left(px(left))
            .top_0()
            .bottom_0()
            .right(px(right))
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(-self.cards_at()))
                    .flex()
                    .items_center()
                    .gap(px(CARD_GAP))
                    .pl(px(SPACES_LEFT))
                    .py(px(SPACES_INSET))
                    .children(cards)
                    .child(plus),
            )
            .on_drag_move(
                cx.listener(|shell, event: &DragMoveEvent<DraggedSpace>, _, cx| {
                    shell.drag_space_over(Along::Cards, event, cx)
                }),
            )
            .children(self.space_drop_line(Along::Cards))
            .child(self.space_drop_listener(Along::Cards, cx))
    }

    /// A Space's card: its name's field over the rows the sidebar would show, in its colour. A
    /// click shows the Space, and a right-click shows its menu.
    fn space_card(&self, space: &Space, cx: &mut Context<Self>) -> Stateful<Div> {
        let id = space.id;
        let shell = cx.weak_entity();
        // The card is the Space's window, opaque, so it's drawn in the Space's own palette.
        let palette = self.palette_of(id);
        let underlay = Hsla::from(palette.underlay);
        let window_at = |y: f32| underlay.blend(Hsla::from(self.space_color_at(id, 0.5, y)));
        let fill = linear_gradient(
            180.,
            linear_color_stop(window_at(0.), 0.),
            linear_color_stop(window_at(1.), 1.),
        );
        let rows = space.card_lines().into_iter().map(|line| match line {
            Line::Divider => {
                let clear = still_clear(&palette, space.clears_last(space.shown));
                divider_box(&palette, self.hairline(), Some(clear)).into_any_element()
            }
            Line::Folder(folder, depth) => {
                let name = space
                    .folders
                    .folders
                    .get(&folder)
                    .map(|(name, _)| name.clone());
                let name = title(&palette, name.unwrap_or_default(), false).pr(px(PLATE_INSET));
                let elided = space.elides(folder);
                let icon = folder_icon(&palette, space.is_open(folder), elided);
                card_row(depth, plate(icon, name))
            }
            Line::Tab(tab, depth) => {
                let panes = space.panes(tab);
                if panes.len() == 1 {
                    let label = title(&palette, self.title(tab), false).pr(px(PLATE_INSET));
                    let icon = self.with_return_mark(
                        &palette,
                        tab,
                        FAVICON_SIZE,
                        self.tab_bitmap_in(&palette, tab),
                        || self.tab_icon_in(&palette, tab),
                        cx,
                    );
                    let plate = plate(icon, label);
                    let dragged = CardTab {
                        tab,
                        space: id,
                        depth,
                    };
                    let shell = shell.clone();
                    let plate =
                        plate
                            .id(("card-tab", tab))
                            .on_drag(dragged, move |dragged, _, _, cx| {
                                cx.new(|_| CardGhost {
                                    shell: shell.clone(),
                                    dragged: *dragged,
                                })
                            });
                    return card_row(depth, plate);
                }
                let weights = self.rest_weights(&panes, self.browser.space_tab(id));
                let segments =
                    panes
                        .iter()
                        .zip(weights)
                        .enumerate()
                        .map(|(index, (&pane, weight))| {
                            let speaker = self.still_speaker(&palette, pane);
                            self.segment(
                                &palette,
                                pane,
                                (weight, 0.),
                                index == 0,
                                false,
                                speaker,
                                cx,
                            )
                        });
                card_row(depth, split_plate(segments))
            }
        });
        let rows: Vec<_> = rows.collect();
        let field = self.card_field(space, &palette, window_at(0.), cx);
        // The ••• at the card's foot (Arc) opens the Space's menu, as the sidebar's does, with the
        // pointer on the card. Its press stops there, so the card sees no click to pick the Space.
        let half = URL_BUTTON_SIZE / 2.;
        let more = div()
            .id(("space-card-more", id))
            .absolute()
            .right(px(CARD_MORE_X - half))
            .bottom(px(CARD_MORE_Y - half))
            .size(px(URL_BUTTON_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(NAV_RADIUS))
            .opacity(0.)
            .group_hover("space-card", |style| style.opacity(1.))
            .hover(|style| style.bg(palette.plate(STATE_HOVERED)))
            .when(self.a11y, |more| {
                more.button_named("Edit Space")
                    .pressed_by_mouse_down(true, false)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    shell.space_menu(id, event.position, cx)
                }),
            )
            .child(
                svg()
                    .path("icons/ellipsis.svg")
                    .size(px(URL_BUTTON_GLYPH))
                    .text_color(palette.glyph(NAV)),
            );
        div()
            .id(("space-card", id))
            .flex_none()
            .w(px(CARD_WIDTH))
            .h_full()
            .overflow_hidden()
            .rounded(px(CARD_RADIUS))
            .bg(fill)
            .shadow(Level::Resting.shadow())
            .block_mouse_except_scroll()
            .on_click(cx.listener(move |shell, _, _, cx| shell.pick_space(id, cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                    shell.space_menu(id, event.position, cx)
                }),
            )
            .on_drop(cx.listener(move |shell, dragged: &CardTab, _, cx| {
                shell.drop_on_card(*dragged, id, cx)
            }))
            .group("space-card")
            // Over the gradient and under the rows, ringed while another card's tab is over it.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(CARD_RADIUS))
                    .group_hover("space-card", |style| style.bg(palette.plate(STATE_HOVERED)))
                    .can_drop(move |dragged, _, _| {
                        dragged
                            .downcast_ref::<CardTab>()
                            .is_some_and(|d| d.space != id)
                    })
                    .group_drag_over::<CardTab>("space-card", |style| {
                        style
                            .border(px(CARD_DROP_RING))
                            .border_color(palette.accent)
                    }),
            )
            .child(field)
            .child(div().children(rows))
            .child(more)
            .child(self.card_handle(id, &palette, cx))
    }

    /// The handle at the card's foot, on the •••'s line and as far in from the card's left (Arc),
    /// that drags the card along the others to move the Space (#1580), shown with the pointer on
    /// the card as the ••• is. Its press stops there, so the card sees no click to pick the Space,
    /// and a tab's row still drags the tab.
    fn card_handle(
        &self,
        id: u64,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let half = URL_BUTTON_SIZE / 2.;
        let shell = cx.weak_entity();
        div()
            .id(("space-card-handle", id))
            .absolute()
            .left(px(CARD_MORE_X - half))
            .bottom(px(CARD_MORE_Y - half))
            .size(px(URL_BUTTON_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(NAV_RADIUS))
            .opacity(0.)
            .group_hover("space-card", |style| style.opacity(1.))
            .hover(|style| style.bg(palette.plate(STATE_HOVERED)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            // GPUI holds the handle where it was pressed; the card is held that far into it.
            .on_drag(DraggedSpace, move |_, grab, _, cx| {
                let grab = CARD_MORE_X - half + f32::from(grab.x);
                Shell::start_space_drag(&shell, id, Along::Cards, CARD_WIDTH, grab, cx)
            })
            .child(
                svg()
                    .path("icons/grip-vertical.svg")
                    .size(px(URL_BUTTON_GLYPH))
                    .text_color(palette.glyph(NAV)),
            )
    }

    /// The card dragged by its handle, whole, as it follows the pointer, lifted with a popover's
    /// shadow as a card's dragged tab is. GPUI draws the drag from the handle's corner, so the
    /// card hangs from there.
    pub(super) fn space_card_ghost(
        &self,
        space: u64,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let space = self.spaces.iter().find(|each| each.id == space)?;
        let height = f32::from(self.viewport.height) - 2. * SPACES_INSET;
        let half = URL_BUTTON_SIZE / 2.;
        let card = self
            .space_card(space, cx)
            .absolute()
            .left(px(half - CARD_MORE_X))
            .top(px(CARD_MORE_Y + half - height))
            .h(px(height))
            .shadow(Level::Popover.shadow());
        Some(div().child(card).into_any_element())
    }

    /// The field across the top of a Space's card (Arc): its icon in a dashed slot, which the icon
    /// picker hangs from, its name, which a click puts in a field there to type over, and the
    /// brush, which the theme picker opens beside. `behind` is the card's colour at its top, for
    /// the swatch of a Space with no icon. At rest it's the icon and the name alone; its plate, the
    /// slot's dashes and the brush show with the pointer anywhere on the field, the slot and the
    /// brush too, at once as other hover plates do, and while the name is being typed over (#811)
    /// or a picker shows for the Space. They keep their places at rest, so nothing moves as they
    /// come. The pickers open over the Library, which stays open, so several Spaces can be
    /// changed in a row, as in Arc (#1074).
    fn card_field(
        &self,
        space: &Space,
        palette: &Palette,
        behind: Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let id = space.id;
        let renaming = self
            .renaming
            .as_ref()
            .filter(|renaming| renaming.naming == Naming::Space(id) && renaming.in_library);
        let held = renaming.is_some() || self.card_picking(id);
        // A Space without an icon shows its swatch in the icon's place.
        let icon = space_icon_drawn(palette, &space.icon).unwrap_or_else(|| {
            palette
                .swatch
                .draw(SPACE_SWATCH_SHOWN, &[behind.to_rgb()])
                .into_any_element()
        });
        let frames = self.slot_frames.clone();
        let slot = div()
            .id(("space-icon-slot", id))
            .flex_none()
            .relative()
            .size(px(CARD_SLOT))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(EMOJI_SIZE))
            .on_hover(cx.listener(move |shell, hovered: &bool, _, cx| {
                let frame = shell.slot_frames.borrow().get(&id).copied();
                let tip = frame.filter(|_| *hovered).map(|frame| {
                    (
                        Tip::named("Edit Space icon".into()),
                        Anchor::under(frame, shell.viewport),
                    )
                });
                shell.hover_tip(tip, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, _, _, cx| {
                    shell.browser.tooltip(None, cx);
                    shell.card_press_closes = icon_picker::open_for(cx) == Some(Subject::Space(id));
                }),
            )
            // The card's click is the Space's, so the slot's stops there, as the brush's does.
            .on_click(cx.listener(move |shell, _, _, cx| {
                cx.stop_propagation();
                if !std::mem::take(&mut shell.card_press_closes) {
                    shell.change_space_icon_in_card(id, cx);
                }
            }))
            .child(
                canvas(
                    move |bounds, _, _| {
                        frames.borrow_mut().insert(id, bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .size_full()
                    .when(!held, |dashes| {
                        dashes
                            .opacity(0.)
                            .group_hover(FIELD, |style| style.opacity(1.))
                    })
                    .child(dashed_square(palette.glyph(CARD_SLOT_DASH)).size_full()),
            )
            .child(icon);
        let name = match renaming {
            // A click in the field places the caret, and mustn't reach the card.
            Some(renaming) => self
                .naming_field(renaming, cx)
                .id(("space-name", id))
                .pr_0()
                .on_click(|_, _, cx| cx.stop_propagation()),
            None => div()
                .id(("space-name", id))
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .items_center()
                .cursor_text()
                .text_color(palette.text(CARD_NAME_COLOR))
                .on_click(cx.listener(move |shell, _, window, cx| {
                    cx.stop_propagation();
                    shell.rename_space_in_card(id, window, cx);
                }))
                .child(div().truncate().child(space.name.clone())),
        };
        let brushes = self.brush_frames.clone();
        let brush = div()
            .id(("space-brush", id))
            .flex_none()
            .size(px(CARD_BRUSH))
            .flex()
            .items_center()
            .justify_center()
            .relative()
            .rounded(px(NAV_RADIUS))
            .when(!held, |brush| {
                brush
                    .opacity(0.)
                    .group_hover(FIELD, |style| style.opacity(1.))
            })
            .hover(|style| style.bg(palette.plate(STATE_HOVERED)))
            .active(|style| style.bg(palette.plate(STATE_PRESSED)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, _, _, cx| {
                    shell.card_press_closes = theme_picker::open_for(cx) == Some(id);
                }),
            )
            .on_click(cx.listener(move |shell, _, _, cx| {
                cx.stop_propagation();
                if !std::mem::take(&mut shell.card_press_closes) {
                    shell.edit_space_theme_in_card(id, cx);
                }
            }))
            .child(
                canvas(
                    move |bounds, _, _| {
                        brushes.borrow_mut().insert(id, bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                svg()
                    .path("icons/paintbrush.svg")
                    .size(px(CARD_BRUSH_GLYPH))
                    .text_color(palette.glyph(CARD_BRUSH_COLOR)),
            );
        let plate = palette.plate(CARD_FIELD_PLATE);
        let drawn = self.fields_drawn.clone();
        // check.sh waits for these lines, logged as the field is drawn with its plate or without.
        // Hovered as the field's group is: not under a control that blocks the pointer (#1074).
        let log_drawn = canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                let shown = held || hitbox.is_hovered(window);
                if drawn.borrow_mut().insert(id, shown) != Some(shown) {
                    let state = if shown { "shown" } else { "at rest" };
                    log!("browser: library field {id} {state} drawn");
                }
            },
        )
        .absolute()
        .size_full();
        div()
            .flex_none()
            .mt(px(CARD_NAME_TOP))
            .h(px(CARD_NAME_HEIGHT))
            .px(px(CARD_FIELD_INSET))
            .flex()
            .items_center()
            .child(
                div()
                    .id(("space-field", id))
                    .group(FIELD)
                    .relative()
                    .w_full()
                    .h(px(CARD_FIELD_HEIGHT))
                    .flex()
                    .items_center()
                    .gap(px(CARD_NAME_GAP))
                    .pl(px(CARD_SLOT_X))
                    .pr(px(CARD_BRUSH_INSET))
                    .rounded(px(CARD_FIELD_RADIUS))
                    .when(held, |field| field.bg(plate))
                    .group_hover(FIELD, |style| style.bg(plate))
                    .text_size(px(text::BODY))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(log_drawn)
                    .child(slot)
                    .child(name)
                    .child(brush),
            )
    }

    /// Shows `space`, from its card, and closes the Library.
    /// A tab's row dragged from one Space's card let go over `space`'s: the tab moves there, as
    /// Move to moves it, pinned if it was, and the Library stays open over both cards.
    fn drop_on_card(&mut self, dragged: CardTab, space: u64, cx: &mut Context<Self>) {
        if dragged.space == space {
            return;
        }
        // check.sh waits for this line.
        log!("browser: library dragged {} to space {space}", dragged.tab);
        self.move_to_space(dragged.tab, space, false, cx);
    }

    /// What follows the pointer while a card's tab is dragged: its row, lifted off the card.
    fn card_ghost(&self, dragged: CardTab) -> AnyElement {
        let CardTab { tab, space, depth } = dragged;
        let palette = self.palette_of(space);
        let behind = Hsla::from(self.space_color_at(space, 0.5, 0.));
        let behind = Hsla::from(palette.underlay).blend(behind);
        let label = title(&palette, self.title(tab), true).pr(px(PLATE_INSET));
        // A row lifted off its card, not the shown tab, so never Developer Mode's firmer fill.
        let row = plate(self.tab_icon_in(&palette, tab), label);
        let row = selected_plate(&palette, row, 1., 0., false);
        div()
            .w(px(CARD_WIDTH
                - 2. * PLATE_INSET
                - depth as f32 * FOLDER_INDENT))
            .h(px(PLATE_HEIGHT))
            .rounded(px(PLATE_RADIUS))
            .bg(behind)
            .shadow(Level::Popover.shadow())
            .child(row)
            .into_any_element()
    }

    fn pick_space(&mut self, space: u64, cx: &mut Context<Self>) {
        // check.sh waits for this line.
        log!("browser: library space {space}");
        self.show_space(space, cx);
        self.set_library(false, cx);
    }

    /// A line of the archive's list: a heading, or an archived tab, which a click opens in a
    /// preview and a right-click shows the menu of.
    fn archive_item(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(item) = self
            .library
            .as_ref()
            .and_then(|library| library.items.get(index))
        else {
            return div().into_any_element();
        };
        let tab = match *item {
            Item::Heading(heading) => {
                return div()
                    .pt(px(if index == 0 {
                        HEADING_TOP
                    } else {
                        HEADING_TOP + HEADING_GAP
                    }))
                    .pb(px(HEADING_BOTTOM))
                    .pl(px(HEADING_X))
                    .child(
                        div()
                            .h(px(HEADING_HEIGHT))
                            .flex()
                            .items_center()
                            .text_size(px(text::SMALL))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(self.palette.text(SECONDARY))
                            .child(heading),
                    )
                    .into_any_element();
            }
            Item::Tab(tab) => tab,
            Item::Task(task) => return self.archived_task_item(task, cx),
            Item::Past(past) => return self.past_task_item(past, cx),
            Item::Clear => return self.agents_clear_item(cx),
            Item::PastHead(past) => return self.past_head_item(past, cx),
            Item::PastSite(site) => return self.past_site_item(site, cx),
            Item::PastStep(past, step) => return self.past_step_item(past, step, cx),
            Item::Boost(boost) => return self.boost_item(boost, cx),
            Item::Download(download) => return self.download_item(download, cx),
            Item::Media(row) => return self.media_row(row, window, cx),
        };
        let Some(tab) = self.archive.get(tab) else {
            return div().into_any_element();
        };
        let (id, url) = (tab.id, tab.url.clone());
        let title = if tab.title.is_empty() {
            url.clone()
        } else {
            tab.title.clone()
        };
        // Asked for once a row first shows, so a long archive costs nothing until it's scrolled.
        if !self.page_favicons.contains_key(&url) {
            self.page_favicons.insert(url.clone(), None);
            self.browser.page_favicon(&url, cx);
        }
        let icon = favicon(
            &self.palette,
            self.page_favicons.get(&url).cloned().flatten(),
        );
        let host = SharedString::from(host(&url).to_string());
        div()
            .h(px(ARCHIVED_PITCH))
            .pl(px(ARCHIVED_LEFT))
            .pr(px(ARCHIVED_RIGHT))
            .py(px((ARCHIVED_PITCH - ARCHIVED_PLATE) / 2.))
            .child(
                div()
                    .id(("archived", id))
                    .size_full()
                    .relative()
                    .rounded(px(ARCHIVED_RADIUS))
                    .block_mouse_except_scroll()
                    .hover(|style| style.bg(self.palette.plate(STATE_HOVERED)))
                    .active(|style| style.bg(self.palette.plate(STATE_PRESSED)))
                    .on_click(cx.listener(move |shell, event: &ClickEvent, _, cx| {
                        shell.open_archived(id, event.position(), event.modifiers(), cx)
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                            shell.archived_menu(id, event.position, cx)
                        }),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(ARCHIVED_FAVICON_X))
                            .top_0()
                            .bottom_0()
                            .flex()
                            .items_center()
                            .child(icon),
                    )
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .flex_col()
                            .justify_center()
                            .pl(px(ARCHIVED_TEXT_X))
                            .pr(px(12.))
                            .child(text_line(
                                title,
                                text::BODY,
                                17.,
                                self.palette.text(UNSELECTED_TITLE),
                            ))
                            .child(text_line(
                                host,
                                text::SMALL,
                                15.,
                                self.palette.text(SECONDARY),
                            )),
                    ),
            )
            .into_any_element()
    }

    /// A task's line in the archive, as a tab's: its card's mark in the favicon's place, its title
    /// over its CLI, how it ended and when. A click brings its card back with its conversation
    /// open, and a right-click shows its menu.
    fn archived_task_item(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(task) = self.archived_tasks.get(index) else {
            return div().into_any_element();
        };
        let agent = task.agent.clone();
        let title = if task.title.is_empty() {
            task.asked.clone()
        } else {
            task.title.clone()
        };
        let mark = self.status_mark(task.dot);
        let menu_agent = agent.clone();
        div()
            .h(px(ARCHIVED_PITCH))
            .pl(px(ARCHIVED_LEFT))
            .pr(px(ARCHIVED_RIGHT))
            .py(px((ARCHIVED_PITCH - ARCHIVED_PLATE) / 2.))
            .child(
                div()
                    .id(("archived-task", task_number(&agent)))
                    .size_full()
                    .relative()
                    .rounded(px(ARCHIVED_RADIUS))
                    .block_mouse_except_scroll()
                    .hover(|style| style.bg(self.palette.plate(STATE_HOVERED)))
                    .active(|style| style.bg(self.palette.plate(STATE_PRESSED)))
                    .on_click(cx.listener(move |shell, _, _, cx| {
                        shell.restore_archived_task(agent.clone(), cx)
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                            shell.archived_task_menu(menu_agent.clone(), event.position, cx)
                        }),
                    )
                    // Centred where a favicon is.
                    .child(
                        div()
                            .absolute()
                            .left(px(ARCHIVED_FAVICON_X + (FAVICON_SIZE - TASK_GLYPH) / 2.))
                            .top_0()
                            .bottom_0()
                            .flex()
                            .items_center()
                            .child(mark),
                    )
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .flex_col()
                            .justify_center()
                            .pl(px(ARCHIVED_TEXT_X))
                            .pr(px(12.))
                            .child(text_line(
                                title,
                                text::BODY,
                                17.,
                                self.palette.text(UNSELECTED_TITLE),
                            ))
                            .child(text_line(
                                task.about.clone(),
                                text::SMALL,
                                15.,
                                self.palette.text(SECONDARY),
                            )),
                    ),
            )
            .into_any_element()
    }

    /// Reads the window's profile's past tasks, as Library › Agents shows them, leaving those that
    /// ended 30 days ago or more, which go from the file too. None in a private window.
    fn read_agent_history(&mut self) {
        let Some(profile) = self.learning_profile() else {
            self.agent_history = Vec::new();
            return;
        };
        let read = crate::agent_history::read(&self.browser.agent_history(profile));
        let count = read.len();
        let living = crate::agent_history::living(read, crate::usage::now());
        if living.len() != count {
            self.browser
                .set_agent_history(profile, &crate::agent_history::write(&living));
        }
        self.agent_history = living;
    }

    /// Delete on a past task: it goes from Library › Agents and its file.
    pub(super) fn delete_past_task(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(profile) = self.learning_profile() else {
            return;
        };
        log!("browser: agents history delete {id}");
        let entries = crate::agent_history::read(&self.browser.agent_history(profile));
        let entries = crate::agent_history::deleted(entries, id);
        self.browser
            .set_agent_history(profile, &crate::agent_history::write(&entries));
        self.filter(true, cx);
    }

    /// Agents' Clear: the profile's past tasks all go.
    pub(super) fn clear_agent_history(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.learning_profile() else {
            return;
        };
        log!("browser: agents history cleared");
        self.browser.set_agent_history(profile, "");
        self.filter(true, cx);
    }

    /// A past task's row: its title over its agent, when it ended and what it did, as its receipt
    /// said, with ✕ at its end under the pointer, which deletes it.
    fn past_task_item(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(past) = self.agent_history.get(index) else {
            return div().into_any_element();
        };
        let id = past.id.clone();
        let opened = id.clone();
        let drawn = (self.library.as_ref()).map(|library| {
            crate::settings_rows::drawn(format!("library past {id}"), library.drawn.clone())
        });
        let group: SharedString = format!("past-{index}").into();
        let mark = self.status_mark(crate::agents::task::Dot::Done);
        let close = div()
            .id(("past-delete", index))
            .absolute()
            .right(px(8.))
            .top_0()
            .bottom_0()
            .flex()
            .items_center()
            .invisible()
            .group_hover(group.clone(), |style| style.visible())
            .child(
                svg()
                    .path("icons/x.svg")
                    .size(px(14.))
                    .text_color(self.palette.text(SECONDARY)),
            )
            .on_click(cx.listener(move |shell, _, _, cx| {
                cx.stop_propagation();
                shell.delete_past_task(&id, cx)
            }));
        div()
            .h(px(ARCHIVED_PITCH))
            .pl(px(ARCHIVED_LEFT))
            .pr(px(ARCHIVED_RIGHT))
            .py(px((ARCHIVED_PITCH - ARCHIVED_PLATE) / 2.))
            .child(
                div()
                    .id(("past-task", index))
                    .group(group)
                    .size_full()
                    .relative()
                    .rounded(px(ARCHIVED_RADIUS))
                    .hover(|style| style.bg(self.palette.plate(STATE_HOVERED)))
                    .children(drawn)
                    .on_click(cx.listener(move |shell, _, _, cx| shell.open_past_task(&opened, cx)))
                    .child(
                        div()
                            .absolute()
                            .left(px(ARCHIVED_FAVICON_X + (FAVICON_SIZE - TASK_GLYPH) / 2.))
                            .top_0()
                            .bottom_0()
                            .flex()
                            .items_center()
                            .child(mark),
                    )
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .flex_col()
                            .justify_center()
                            .pl(px(ARCHIVED_TEXT_X))
                            .pr(px(30.))
                            .child(text_line(
                                past.title.clone().into(),
                                text::BODY,
                                17.,
                                self.palette.text(UNSELECTED_TITLE),
                            ))
                            .child(text_line(
                                past.about().into(),
                                text::SMALL,
                                15.,
                                self.palette.text(SECONDARY),
                            )),
                    )
                    .child(close),
            )
            .into_any_element()
    }

    /// A click on a past task's row: its page, in the list's place, with its steps and the sites
    /// it went to, each with its agent's answer there for good and Revoke (#1080).
    pub(super) fn open_past_task(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(library) = &mut self.library else {
            return;
        };
        library.opened = Some(id.to_string());
        self.filter(true, cx);
    }

    /// The page's way back: Agents' list again.
    pub(super) fn close_past_task(&mut self, cx: &mut Context<Self>) {
        let Some(library) = &mut self.library else {
            return;
        };
        if library.opened.take().is_some() {
            // check.sh reads this line.
            log!("browser: agents history lists");
        }
        self.filter(true, cx);
    }

    /// Revoke on the page's site at `index`: the agent's answer there for good goes, as Settings ›
    /// Agents' Revoke has it go, and the activity log says so, so what asks first there asks
    /// again.
    pub(super) fn revoke_past_site(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(library) = &self.library else {
            return;
        };
        let opened = library.opened.as_deref();
        let past = self
            .agent_history
            .iter()
            .find(|past| Some(past.id.as_str()) == opened);
        let (Some(past), Some((site, Some(_)))) = (past, library.sites.get(index)) else {
            return;
        };
        let (answers, site) = (past.answers.clone(), site.clone());
        self.browser.revoke_agent_answer(&answers, &site, cx);
        self.filter(true, cx);
    }

    /// A small button of the Library's lists, as Agents' Clear.
    fn list_button(&self, id: impl Into<gpui::ElementId>, label: &'static str) -> Stateful<Div> {
        div()
            .id(id)
            .px(px(8.))
            .py(px(3.))
            .rounded(px(ARCHIVED_RADIUS))
            .text_size(px(text::SMALL))
            .text_color(self.palette.text(SECONDARY))
            .hover(|style| style.bg(self.palette.plate(STATE_HOVERED)))
            .child(label)
    }

    /// The past task's page's head: Back in its mark's place, which goes back to the list, and its
    /// title over its agent, how long it ran, when it ended and what it did.
    fn past_head_item(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(past) = self.agent_history.get(index) else {
            return div().into_any_element();
        };
        let drawn = (self.library.as_ref()).map(|library| {
            crate::settings_rows::drawn("library back".into(), library.drawn.clone())
        });
        let back = div()
            .id("past-back")
            .absolute()
            .left(px(ARCHIVED_FAVICON_X + FAVICON_SIZE / 2. - 12.))
            .top(px((ARCHIVED_PLATE - 24.) / 2.))
            .size(px(24.))
            .rounded(px(ARCHIVED_RADIUS))
            .flex()
            .items_center()
            .justify_center()
            .hover(|style| style.bg(self.palette.plate(STATE_HOVERED)))
            .children(drawn)
            .child(
                svg()
                    .path("icons/arrow-left.svg")
                    .size(px(FAVICON_SIZE))
                    .text_color(self.palette.text(SECONDARY)),
            )
            .on_click(cx.listener(|shell, _, _, cx| shell.close_past_task(cx)));
        div()
            .h(px(ARCHIVED_PITCH))
            .pl(px(ARCHIVED_LEFT))
            .pr(px(ARCHIVED_RIGHT))
            .py(px((ARCHIVED_PITCH - ARCHIVED_PLATE) / 2.))
            .child(
                div()
                    .size_full()
                    .relative()
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .flex_col()
                            .justify_center()
                            .pl(px(ARCHIVED_TEXT_X))
                            .pr(px(8.))
                            .child(text_line(
                                past.title.clone().into(),
                                text::BODY,
                                17.,
                                self.palette.text(UNSELECTED_TITLE),
                            ))
                            .child(text_line(
                                past.page_about().into(),
                                text::SMALL,
                                15.,
                                self.palette.text(SECONDARY),
                            )),
                    )
                    .child(back),
            )
            .into_any_element()
    }

    /// A site the past task went to, over its agent's answer there for good, with Revoke at its
    /// end while it has one.
    fn past_site_item(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some((site, answer)) =
            (self.library.as_ref()).and_then(|library| library.sites.get(index))
        else {
            return div().into_any_element();
        };
        let answer_words = match answer {
            Some(true) => "Always Allow",
            Some(false) => "Never",
            None => "Asks first",
        };
        let revoke = answer.is_some().then(|| {
            let drawn = (self.library.as_ref()).map(|library| {
                crate::settings_rows::drawn(format!("library revoke {site}"), library.drawn.clone())
            });
            self.list_button(("past-revoke", index), "Revoke")
                .relative()
                .children(drawn)
                .on_click(cx.listener(move |shell, _, _, cx| shell.revoke_past_site(index, cx)))
        });
        div()
            .h(px(ARCHIVED_PITCH))
            .pl(px(ARCHIVED_LEFT))
            .pr(px(ARCHIVED_RIGHT + 8.))
            .py(px((ARCHIVED_PITCH - ARCHIVED_PLATE) / 2.))
            .flex()
            .items_center()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .pl(px(ARCHIVED_TEXT_X))
                    .child(text_line(
                        site.clone().into(),
                        text::BODY,
                        17.,
                        self.palette.text(UNSELECTED_TITLE),
                    ))
                    .child(text_line(
                        answer_words.into(),
                        text::SMALL,
                        15.,
                        self.palette.text(SECONDARY),
                    )),
            )
            .children(revoke)
            .into_any_element()
    }

    /// A step of the past task, in the words Settings › Agents' activity log has, over when it
    /// came and why it was refused, if it was. A request opens, as it does there, to show where it
    /// went and what it sent (#2326).
    fn past_step_item(&mut self, index: usize, step: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(past) = self.agent_history.get(index) else {
            return div().into_any_element();
        };
        let Some(each) = past.steps.get(step) else {
            return div().into_any_element();
        };
        let did = crate::agents::tools::Did {
            tool: each.action.clone(),
            site: each.site.clone(),
            target: each.target.clone(),
            ..Default::default()
        };
        let tense = match each.refused.is_empty() {
            true => crate::agents::tools::Tense::Done,
            false => crate::agents::tools::Tense::Failed,
        };
        let when = crate::agents::local_time(each.at);
        let under = match each.refused.is_empty() {
            true => when,
            false => format!("{when} · Refused: {}", each.refused),
        };
        let title = crate::agents::tools::step_title(&did, tense);
        let key = (past.id.clone(), step);
        let opens = !each.url.is_empty();
        let (opened, all) = match &self.library {
            Some(library) => (
                library.steps_opened.contains(&key),
                library.steps_all.contains(&key),
            ),
            None => (false, false),
        };
        let secondary = self.palette.text(SECONDARY);
        let head = div()
            .h(px(ARCHIVED_PITCH))
            .pl(px(ARCHIVED_LEFT))
            .pr(px(ARCHIVED_RIGHT + 8.))
            .py(px((ARCHIVED_PITCH - ARCHIVED_PLATE) / 2.))
            .flex()
            .items_center()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .pl(px(ARCHIVED_TEXT_X))
                    .pr(px(8.))
                    .child(text_line(
                        title.clone().into(),
                        text::BODY,
                        17.,
                        self.palette.text(UNSELECTED_TITLE),
                    ))
                    .child(text_line(under.into(), text::SMALL, 15., secondary)),
            )
            .child(crate::settings_rows::chevron_slot(
                opens.then_some(opened),
                secondary,
            ));
        if !opens {
            return head.into_any_element();
        }
        let drawn = (self.library.as_ref()).map(|library| {
            crate::settings_rows::drawn(format!("library step {step}"), library.drawn.clone())
        });
        let head = head
            .id(("past-step", step))
            .relative()
            .children(drawn)
            .role(gpui::accesskit::Role::Button)
            .aria_label(title)
            .aria_expanded(opened)
            .on_click(cx.listener(move |shell, _, _, cx| shell.open_past_step(step, false, cx)));
        let part = opened.then(|| {
            let more = cx.listener(move |shell, _: &gpui::ClickEvent, _, cx| {
                shell.open_past_step(step, true, cx)
            });
            crate::settings_rows::sent_part(
                ("past-step-all", step),
                &each.url,
                &each.sent,
                !each.refused.is_empty(),
                all,
                (self.palette.text(UNSELECTED_TITLE), secondary),
                true,
                move |window, cx| more(&gpui::ClickEvent::default(), window, cx),
            )
            .pl(px(ARCHIVED_LEFT + ARCHIVED_TEXT_X))
            .pr(px(ARCHIVED_RIGHT + 8.))
            .pb(px((ARCHIVED_PITCH - ARCHIVED_PLATE) / 2.))
        });
        div().child(head).children(part).into_any_element()
    }

    /// Opens the opened past task's request at `step`, or closes it, or with `all` shows every
    /// line of what it sent, and measures its line of the list again.
    pub(super) fn open_past_step(&mut self, step: usize, all: bool, cx: &mut Context<Self>) {
        let Some(library) = &mut self.library else {
            return;
        };
        let Some(id) = library.opened.clone() else {
            return;
        };
        let key = (id, step);
        let place = |keys: &[(String, usize)]| keys.iter().position(|each| *each == key);
        let said = match (all, place(&library.steps_opened)) {
            (true, _) => {
                library.steps_all.push(key.clone());
                "shows all"
            }
            (false, Some(at)) => {
                library.steps_opened.remove(at);
                "closed"
            }
            (false, None) => {
                library.steps_opened.push(key.clone());
                "opened"
            }
        };
        let past = (library.items.iter()).position(|item| match item {
            Item::PastStep(_, each) => *each == step,
            _ => false,
        });
        if let Some(at) = past {
            library.list.splice(at..at + 1, 1);
        }
        // check.sh waits for this line.
        log!("browser: library step {step} {said}");
        cx.notify();
    }

    /// Reads the window's profile's Boosts, as Library › Boosts lists them.
    fn read_boosts(&mut self, cx: &mut Context<Self>) {
        let Some(profile) = self.learning_profile() else {
            self.boost_list = Vec::new();
            return;
        };
        if !crate::boosts::with(|boosts| boosts.loaded(profile)) {
            self.browser.load_boosts(profile, cx);
        }
        self.boost_list = crate::boosts::with(|boosts| boosts.list(profile));
    }

    /// Lists the Boosts again, as one changed.
    pub(super) fn list_boosts(&mut self, cx: &mut Context<Self>) {
        self.filter(true, cx);
    }

    /// A Boost's row: its site over what it does, dimmed while it's off. A click or right-click
    /// opens its menu: Edit CSS…, Turn Off or On, Show in Finder and Delete….
    fn boost_item(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(boost) = self.boost_list.get(index) else {
            return div().into_any_element();
        };
        let site = boost.site.clone();
        let about = match boost.on {
            true => boost.summary.describe(),
            false => format!("Off. {}", boost.summary.describe()),
        };
        let menu = |site: SharedString| {
            move |shell: &mut Shell,
                  event: &MouseDownEvent,
                  _: &mut Window,
                  cx: &mut Context<Shell>| {
                cx.stop_propagation();
                shell.boost_menu(site.clone(), event.position, cx)
            }
        };
        div()
            .h(px(ARCHIVED_PITCH))
            .pl(px(ARCHIVED_LEFT))
            .pr(px(ARCHIVED_RIGHT))
            .py(px((ARCHIVED_PITCH - ARCHIVED_PLATE) / 2.))
            .child(
                div()
                    .id(("boost", index))
                    .size_full()
                    .relative()
                    .rounded(px(ARCHIVED_RADIUS))
                    .hover(|style| style.bg(self.palette.plate(STATE_HOVERED)))
                    .when(!boost.on, |row| row.opacity(0.5))
                    .on_mouse_down(MouseButton::Left, cx.listener(menu(site.clone())))
                    .on_mouse_down(MouseButton::Right, cx.listener(menu(site.clone())))
                    .child(
                        div()
                            .absolute()
                            .left(px(ARCHIVED_FAVICON_X))
                            .top_0()
                            .bottom_0()
                            .flex()
                            .items_center()
                            .child(
                                svg()
                                    .path("icons/globe.svg")
                                    .size(px(FAVICON_SIZE))
                                    .text_color(self.palette.text(SECONDARY)),
                            ),
                    )
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .flex_col()
                            .justify_center()
                            .pl(px(ARCHIVED_TEXT_X))
                            .pr(px(12.))
                            .child(text_line(
                                site,
                                text::BODY,
                                17.,
                                self.palette.text(UNSELECTED_TITLE),
                            ))
                            .child(text_line(
                                about.into(),
                                text::SMALL,
                                15.,
                                self.palette.text(SECONDARY),
                            )),
                    ),
            )
            .into_any_element()
    }

    /// A Boost's menu in Library › Boosts.
    pub(super) fn boost_menu(
        &mut self,
        site: SharedString,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(profile) = self.learning_profile() else {
            return;
        };
        let Some(on) =
            crate::boosts::with(|boosts| boosts.get(profile, &site)).map(|boost| boost.on)
        else {
            return;
        };
        let show = match super::OFF_THE_MAC {
            true => "Show in Folder",
            false => "Show in Finder",
        };
        let entries = vec![
            MenuEntry::new("Edit CSS…", true),
            MenuEntry::new("Edit Script…", true),
            MenuEntry::new(if on { "Turn Off" } else { "Turn On" }, true),
            MenuEntry::new(show, true),
            MenuEntry::line(),
            MenuEntry::new("Delete…", true).destructive(),
        ];
        let shell = cx.weak_entity();
        self.browser.context_menu(
            entries,
            position,
            false,
            Look {
                light: false,
                ..self.menu_look()
            },
            Box::new(move |chosen, cx| {
                shell
                    .update(cx, |shell, cx| match chosen {
                        Some(0) => shell.edit_boost_file(profile, &site, cx),
                        Some(1) => shell.edit_boost_script_file(profile, &site, cx),
                        Some(2) => shell.set_boost_on(profile, &site, !on, cx),
                        Some(3) => shell.show_boost_file(profile, &site, cx),
                        Some(5) => shell.delete_boost(profile, &site, cx),
                        _ => {}
                    })
                    .ok();
            }),
            cx,
        );
    }

    /// Turns `site`'s Boost on or off there, from its row or Site Controls' switch.
    pub fn set_boost_on(&mut self, profile: u64, site: &str, on: bool, cx: &mut Context<Self>) {
        // check.sh reads this line.
        log!("browser: boost {site} {}", if on { "on" } else { "off" });
        let changes = crate::boosts::with(|boosts| boosts.set_on(profile, site, on));
        self.browser.show_boosts(changes, cx);
        self.list_boosts(cx);
    }

    fn show_boost_file(&mut self, profile: u64, site: &str, cx: &mut Context<Self>) {
        let folder = crate::boosts::with(|boosts| boosts.folder(profile));
        if let Some(folder) = folder {
            self.browser
                .show_file(&folder.join(format!("{site}.css")), cx);
        }
    }

    /// Delete…: the Boost's file goes, once the user says so.
    pub(super) fn delete_boost(&mut self, profile: u64, site: &str, cx: &mut Context<Self>) {
        let shell = cx.weak_entity();
        let site = site.to_owned();
        self.browser.confirm(
            format!("Delete the Boost for {site}?"),
            "Its file goes from the Boosts folder, and the site shows as it made itself.".into(),
            "Delete",
            Box::new(move |cx| {
                shell
                    .update(cx, |shell, cx| {
                        // check.sh reads this line.
                        log!("browser: boost {site} deleted");
                        let changes = crate::boosts::with(|boosts| boosts.delete(profile, &site));
                        shell.browser.show_boosts(changes, cx);
                        shell.list_boosts(cx);
                    })
                    .ok();
            }),
            cx,
        );
    }

    /// Agents' Clear, at the end of a line over its list.
    fn agents_clear_item(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .w_full()
            .h(px(ARCHIVED_PITCH))
            .pr(px(ARCHIVED_RIGHT + 8.))
            .flex()
            .items_center()
            .justify_end()
            .child(
                self.list_button("agents-clear", "Clear")
                    .on_click(cx.listener(|shell, _, _, cx| shell.clear_agent_history(cx))),
            )
            .into_any_element()
    }

    /// A row of Media's tiles: each a picture of its file, filling its square, or the file's icon,
    /// with a video's glyph in its corner. A click opens the file in a tab, and a right-click shows
    /// a capture's menu, or a download's as its row in Downloads does.
    fn media_row(
        &mut self,
        row: [Option<Medium>; MEDIA_COLUMNS],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tiles: Vec<_> =
            row.iter()
                .flatten()
                .filter_map(|&medium| {
                    let (id, path, name) = match medium {
                        Medium::Capture(index) => self.captures.get(index).map(|each| {
                            (capture_id(&each.path), each.path.clone(), each.name.clone())
                        }),
                        Medium::Download(index) => self
                            .downloads
                            .get(index)
                            .map(|each| (each.id, each.path.clone(), each.name.clone())),
                    }?;
                    Some(self.media_tile(id, path, name, window, cx))
                })
                .collect();
        div()
            .pl(px(ARCHIVED_LEFT))
            .pr(px(ARCHIVED_RIGHT))
            .pb(px(MEDIA_GAP))
            .flex()
            .gap(px(MEDIA_GAP))
            .children(tiles)
            .into_any_element()
    }

    fn media_tile(
        &mut self,
        id: u64,
        path: SharedString,
        name: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Asked for once a tile first shows, as the archive's favicons are.
        if !self.download_previews.contains_key(&id) {
            self.download_previews.insert(id, None);
            self.browser.file_preview(id, &path, cx);
        }
        let video = is_video(&name) == Some(true);
        let playing = video
            .then(|| self.video_frame(id, &path, &name, window, cx))
            .flatten();
        let picture = match self.download_previews.get(&id).cloned().flatten() {
            // Square, as the tile.
            _ if let Some(frame) = playing => img(frame)
                .size_full()
                .rounded(px(MEDIA_RADIUS))
                .into_any_element(),
            Some(preview) if preview.photo => img(preview.image)
                .size_full()
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            Some(preview) => img(preview.image).size(px(MEDIA_ICON)).into_any_element(),
            None => svg()
                .path(if video {
                    "icons/video.svg"
                } else {
                    "icons/image.svg"
                })
                .size(px(MEDIA_ICON * 0.75))
                .text_color(self.palette.text(SECONDARY))
                .into_any_element(),
        };
        let shown = self.palette.plate(STATE_HOVERED);
        div()
            .id(("media", id))
            .relative()
            .flex_none()
            .size(px(MEDIA_TILE))
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .rounded(px(MEDIA_RADIUS))
            .bg(self.palette.plate(LIBRARY_PLATE))
            .block_mouse_except_scroll()
            .on_click({
                let path = path.clone();
                cx.listener(move |shell, _, _, cx| shell.open_media(&path, cx))
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(
                    move |shell, event: &MouseDownEvent, _, cx| match id & CAPTURE_BIT {
                        0 => shell.download_menu(id, event.position, cx),
                        _ => shell.capture_menu(path.clone(), event.position, cx),
                    },
                ),
            )
            .child(picture)
            // Over the picture, which would hide a plate under it.
            .child(
                div()
                    .id("plate")
                    .absolute()
                    .size_full()
                    .hover(|style| style.bg(shown))
                    .active(|style| style.bg(self.palette.plate(STATE_PRESSED))),
            )
            .children(video.then(|| {
                svg()
                    .path("icons/video.svg")
                    .absolute()
                    .left(px(6.))
                    .bottom(px(6.))
                    .size(px(14.))
                    .text_color(Hsla::white())
            }))
            .into_any_element()
    }

    /// The frame tile `id`'s video shows, playing it, without sound and again from its start, from
    /// the tile's first draw until a frame without it. None before its first frame, and where the
    /// platform plays none, while the tile shows its still.
    fn video_frame(
        &mut self,
        id: u64,
        path: &str,
        name: &SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        if !self.media_videos.contains_key(&id) {
            let side = (MEDIA_TILE * window.scale_factor()).ceil() as u32;
            if !self.browser.open_video(id, path, side, cx) {
                return None;
            }
            self.media_videos.insert(
                id,
                MediaVideo {
                    name: name.clone(),
                    frame: None,
                },
            );
        }
        self.videos_drawn.insert(id);
        // Its next frame may be due by the window's.
        window.request_animation_frame();
        let video = self.media_videos.get_mut(&id)?;
        if let Some(frame) = self.browser.video_frame(id, cx) {
            match video.frame.replace(frame) {
                // Each frame is an image of its own in GPUI's atlas, which keeps it until dropped.
                Some(shown) => window.drop_image(shown).unwrap_or_default(),
                // check.sh waits for this line.
                None => log!("browser: media video playing {}", video.name),
            }
        }
        video.frame.clone()
    }

    /// Stops the videos whose tiles the last frame didn't draw, as the Library closed, left Media
    /// or scrolled them away.
    pub(super) fn stop_videos(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let drawn = std::mem::take(&mut self.videos_drawn);
        let gone: Vec<u64> = self
            .media_videos
            .keys()
            .filter(|id| !drawn.contains(id))
            .copied()
            .collect();
        for id in gone {
            self.browser.close_video(id, cx);
            let Some(video) = self.media_videos.remove(&id) else {
                continue;
            };
            if let Some(frame) = video.frame {
                window.drop_image(frame).unwrap_or_default();
            }
            // check.sh waits for this line.
            log!("browser: media video stopped {}", video.name);
        }
        // Those left are drawn again in this frame, or stop in the next.
        if !self.media_videos.is_empty() {
            window.request_animation_frame();
        }
    }

    /// Opens a file of the Library's Media in a tab at the top of the Today tabs, where the page
    /// shows a picture or plays a video, and closes the Library.
    pub(super) fn open_media(&mut self, path: &str, cx: &mut Context<Self>) {
        // check.sh waits for this line.
        log!("browser: open media {path}");
        self.browser.open_at(&[file_url(path)], false, 0, 0, cx);
        self.set_library(false, cx);
    }

    /// The menu a right-click on a capture's tile shows: open it, or delete it from the Library, as
    /// Arc deletes a capture, to the system's Trash, from where it can come back.
    pub(super) fn capture_menu(
        &mut self,
        path: SharedString,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let entries = vec![
            MenuEntry::new("Open", true),
            MenuEntry::line(),
            MenuEntry::new("Delete", true).destructive(),
        ];
        let chosen = move |chosen, shell: &mut Shell, cx: &mut Context<Shell>| match chosen {
            Some(0) => shell.open_media(&path, cx),
            Some(2) => shell.delete_capture(&path, cx),
            _ => {}
        };
        if self.library.is_none() {
            // From the recent downloads' rows, in the reveal's panel while the sidebar is hidden.
            let in_reveal = self.sidebar_hidden;
            return self.context_menu_in(entries, position, in_reveal, Box::new(chosen), cx);
        }
        let shell = cx.weak_entity();
        self.browser.context_menu(
            entries,
            position,
            false,
            Look {
                light: false,
                ..self.menu_look()
            },
            Box::new(move |at, cx| {
                shell.update(cx, |shell, cx| chosen(at, shell, cx)).ok();
            }),
            cx,
        );
    }

    /// Moves a capture to the system's Trash and takes it out of every window's Media, or leaves
    /// it if it can't go.
    fn delete_capture(&mut self, path: &str, cx: &mut Context<Self>) {
        // check.sh waits for this line.
        log!("browser: delete capture {path}");
        if !self.browser.trash(path, cx) {
            return;
        }
        self.captures.retain(|each| each.path.as_ref() != path);
        self.filter(true, cx);
        self.browser.list_captures(cx);
    }

    /// A section's button in the column, which shows the section.
    fn section_button(
        &self,
        section: Section,
        color: Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let selected = section == self.section;
        let (glyph, label) = match section {
            Section::Media => ("icons/image.svg", "Media"),
            Section::Downloads => ("icons/arrow-down-to-line.svg", "Downloads"),
            Section::Spaces => ("icons/spaces.svg", "Spaces"),
            Section::Archive => ("icons/archive.svg", "Archived Tabs"),
            Section::Agents => ("icons/square-mouse-pointer.svg", "Agents"),
            Section::Boosts => ("icons/paintbrush.svg", "Boosts"),
        };
        div()
            .id(label)
            .relative()
            .w(px(SECTION_WIDTH))
            .h(px(SECTION_HEIGHT))
            .rounded(px(SECTION_RADIUS))
            .block_mouse_except_scroll()
            .when(selected, |button| {
                button.bg(self.palette.plate(LIBRARY_PLATE))
            })
            .when(!selected, |button| {
                button.hover(|style| style.bg(self.palette.plate(SECTION_HOVERED)))
            })
            .on_click(cx.listener(move |shell, _, _, cx| {
                if section != shell.section {
                    shell.show_section(section, cx);
                }
            }))
            .when(section == Section::Archive, |button| {
                button.on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|shell, event: &MouseDownEvent, _, cx| {
                        shell.archive_section_menu(event.position, cx)
                    }),
                )
            })
            .child(
                svg()
                    .path(glyph)
                    .absolute()
                    .left(px((SECTION_WIDTH - SECTION_GLYPH) / 2.))
                    .top(px(SECTION_GLYPH_Y - SECTION_GLYPH / 2.))
                    .size(px(SECTION_GLYPH))
                    .text_color(if selected {
                        color
                    } else {
                        self.palette.glyph(SECTION_GLYPH_UNSELECTED)
                    }),
            )
            .child(
                div()
                    .absolute()
                    .top(px(SECTION_LABEL_Y - SECTION_LABEL_HEIGHT / 2.))
                    .w_full()
                    .h(px(SECTION_LABEL_HEIGHT))
                    .flex()
                    .justify_center()
                    .text_size(px(text::SMALL))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(self.palette.text(SECTION_LABEL))
                    .child(label),
            )
    }

    /// A download's row: its file's icon, its name over where it is, and while it runs, a bar. A
    /// click opens a finished file or shows a held one's menu, and a right-click shows the menu.
    fn download_item(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(download) = self.downloads.get(index) else {
            return div().into_any_element();
        };
        let id = download.id;
        let (state, held) = (download.state, download.held);
        let site = host(&download.url);
        let status: SharedString = match state {
            _ if held => "May harm your computer".into(),
            DownloadState::Running => of(download.received, download.total).into(),
            DownloadState::Paused => {
                format!("Paused · {}", of(download.received, download.total)).into()
            }
            DownloadState::Done => format!("{} · {site}", bytes(download.received)).into(),
            DownloadState::Cancelled => format!("Cancelled · {site}").into(),
            DownloadState::Failed => format!("Failed · {site}").into(),
        };
        let progress = match state {
            DownloadState::Running | DownloadState::Paused if download.total > 0 && !held => {
                Some((download.received as f32 / download.total as f32).clamp(0., 1.))
            }
            _ => None,
        };
        let name = download.name.clone();
        // Asked for once a row first shows, as the archive's favicons are.
        if !self.download_icons.contains_key(&id) {
            self.download_icons.insert(id, None);
            self.browser.download_icon(id, cx);
        }
        let icon = match self.download_icons.get(&id).cloned().flatten() {
            Some(icon) => favicon(&self.palette, Some(icon)).into_any_element(),
            None => (svg()
                .path("icons/file.svg")
                .size(px(16.))
                .text_color(self.palette.text(SECONDARY)))
            .into_any_element(),
        };
        div()
            .h(px(ARCHIVED_PITCH))
            .pl(px(ARCHIVED_LEFT))
            .pr(px(ARCHIVED_RIGHT))
            .py(px((ARCHIVED_PITCH - ARCHIVED_PLATE) / 2.))
            .child(
                self.download_row(div().id(("download", id)), id, "row", cx)
                    .block_mouse_except_scroll()
                    .size_full()
                    .relative()
                    .rounded(px(ARCHIVED_RADIUS))
                    .child(
                        div()
                            .absolute()
                            .left(px(ARCHIVED_FAVICON_X))
                            .top_0()
                            .bottom_0()
                            .flex()
                            .items_center()
                            .child(icon),
                    )
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .flex_col()
                            .justify_center()
                            .pl(px(ARCHIVED_TEXT_X))
                            .pr(px(12.))
                            .child(text_line(
                                name,
                                text::BODY,
                                17.,
                                self.palette.text(UNSELECTED_TITLE),
                            ))
                            .child(text_line(
                                status,
                                text::SMALL,
                                15.,
                                self.palette.text(SECONDARY),
                            )),
                    )
                    .children(progress.map(|done| {
                        div()
                            .absolute()
                            .left(px(ARCHIVED_TEXT_X))
                            .right(px(12.))
                            .bottom(px(PROGRESS_BOTTOM))
                            .h(px(PROGRESS_HEIGHT))
                            .rounded(px(PROGRESS_HEIGHT / 2.))
                            .bg(self.palette.faint(PROGRESS_TRACK))
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(done))
                                    .rounded(px(PROGRESS_HEIGHT / 2.))
                                    .bg(self.palette.glyph(PROGRESS_FILL)),
                            )
                    })),
            )
            .into_any_element()
    }

    /// What a download's row does, in the Library or the Library button's recent downloads, with
    /// its plate lit under the pointer: a press that can drag the file out, a click that opens a
    /// finished file or shows a held one's menu, and a right-click that shows the menu.
    pub(super) fn download_row(
        &self,
        row: Stateful<Div>,
        id: u64,
        source: &'static str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let Some(download) = self.downloads.iter().find(|each| each.id == id) else {
            return row;
        };
        let (state, held) = (download.state, download.held);
        // GPUI never hears the release that ends the system's drag, and would keep the row
        // pressed.
        let dragged = matches!(self.download_press, Some((pressed, _, true)) if pressed == id);
        row.hover(|style| style.bg(self.palette.plate(STATE_HOVERED)))
            .when(!dragged, |row| {
                row.active(|style| style.bg(self.palette.plate(STATE_PRESSED)))
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                    shell.download_press = Some((id, event.position, false));
                    cx.notify();
                }),
            )
            .on_click(cx.listener(move |shell, event: &ClickEvent, _, cx| {
                // A press that became a drag isn't a click.
                if shell
                    .download_press
                    .take()
                    .is_some_and(|(_, _, dragged)| dragged)
                {
                    return;
                }
                // A held file's only choices are in its menu, so a click shows them.
                if held {
                    shell.download_menu(id, event.position(), cx);
                } else if state == DownloadState::Done {
                    shell.act_on_download(id, DownloadAction::Open, source, cx);
                    shell.hide_recent_downloads(cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                    shell.download_menu(id, event.position, cx)
                }),
            )
    }

    /// The menu a right-click on a download's row shows: what can be done with it where it is.
    fn download_menu(&mut self, id: u64, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(download) = self.downloads.iter().find(|each| each.id == id) else {
            return;
        };
        let finished = self
            .downloads
            .iter()
            .any(|each| !matches!(each.state, DownloadState::Running | DownloadState::Paused));
        let items = download_menu_items(download, finished);
        let entries = items.iter().map(|(entry, _)| entry.clone()).collect();
        // The recent downloads' rows are in the reveal's panel while the sidebar is hidden.
        let in_reveal = self.library.is_none() && self.sidebar_hidden;
        self.context_menu_in(
            entries,
            position,
            in_reveal,
            Box::new(move |chosen, shell, cx| {
                let Some((_, Some(choice))) = chosen.and_then(|at| items.get(at)) else {
                    return;
                };
                match *choice {
                    DownloadChoice::Act(action) => {
                        shell.act_on_download(id, action, "download menu", cx)
                    }
                    DownloadChoice::Trash => shell.trash_download(id, cx),
                    DownloadChoice::Clear => {
                        log!("browser: command clear-downloads via download menu");
                        shell.browser.clear_downloads(shell.private_profile(), cx);
                    }
                }
            }),
            cx,
        );
    }

    fn act_on_download(
        &mut self,
        id: u64,
        action: DownloadAction,
        source: &str,
        cx: &mut Context<Self>,
    ) {
        // check.sh waits for this line.
        log!("browser: download {id} {action:?} via {source}");
        self.browser.act_on_download(id, action, cx);
    }

    /// Moves a finished download's file to the Trash and takes it off the list, or leaves both
    /// if the file is there and can't go.
    fn trash_download(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(path) = self
            .downloads
            .iter()
            .find(|each| each.id == id)
            .map(|d| d.path.clone())
        else {
            return;
        };
        // For check.sh.
        log!("browser: download {id} Trash via download menu");
        if path.is_empty() || self.browser.trash(&path, cx) {
            self.act_on_download(id, DownloadAction::Remove, "download menu", cx);
        }
    }

    /// The pointer pressing a download's row moved to `position`, the left button held. Past
    /// DRAG_START a finished file goes into the system's drag, for Finder, other apps, the page
    /// or the tab list, and the Library closes so the tab list under it shows.
    fn drag_download(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some((id, press, false)) = self.download_press else {
            return;
        };
        let moved = position - press;
        if moved.x.abs().max(moved.y.abs()) < px(DRAG_START) {
            return;
        }
        let Some(download) = self.downloads.iter().find(|each| each.id == id) else {
            return;
        };
        // One with no file yet keeps its press a click, as a sidebar row's does.
        if download.state != DownloadState::Done || download.held || download.path.is_empty() {
            return;
        }
        let path = download.path.clone();
        self.download_press = Some((id, press, true));
        if self.browser.drag_file(&path, cx) {
            // For check.sh.
            log!("browser: download {id} drag");
            // A row of the recent downloads leaves the Library closed, and the rows go.
            self.library_dragged = self.library.is_some();
            match self.library_dragged {
                true => self.set_library(false, cx),
                false => self.hide_recent_downloads(cx),
            }
        }
        cx.notify();
    }

    /// The system's drag of a download's file was let go. If nothing took it, the Library comes
    /// back with the row it came from, as a sidebar row dropped nowhere goes back.
    pub fn file_drag_ended(&mut self, taken: bool, cx: &mut Context<Self>) {
        // For check.sh.
        log!(
            "browser: file drag ended {}",
            if taken { "taken" } else { "nowhere" }
        );
        if std::mem::take(&mut self.library_dragged) && !taken {
            self.set_library(true, cx);
        }
    }

    /// The pointer's presses, moves and release anywhere in the window while a download's row is
    /// pressed, which only hears the pointer while it's over it.
    pub(super) fn download_press_listeners(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        self.download_press.filter(|(_, _, dragged)| !dragged)?;
        let shell = cx.weak_entity();
        Some(
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let pressed = shell.clone();
                    window.on_mouse_event(move |_: &MouseDownEvent, phase, _, cx| {
                        // Before the row pressed, if it's one, hears it.
                        if phase == DispatchPhase::Capture {
                            pressed
                                .update(cx, |shell, _| shell.download_press = None)
                                .ok();
                        }
                    });
                    let moved = shell.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                        if phase == DispatchPhase::Capture && event.dragging() {
                            let position = event.position;
                            moved
                                .update(cx, |shell, cx| shell.drag_download(position, cx))
                                .ok();
                        }
                    });
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        // Before the row's click, which hears no press then and opens.
                        if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                            shell
                                .update(cx, |shell, _| shell.download_press = None)
                                .ok();
                        }
                    });
                },
            )
            .absolute()
            .size_0(),
        )
    }

    /// The footer's button that opens the Library, at its left (Arc). While downloads run, it keeps
    /// its glyph and name over a bar of how far they've got. The pointer resting on it shows the
    /// recent downloads over it.
    pub(super) fn library_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let progress = downloads_progress(&self.downloads);
        let command = Command::ViewLibrary;
        div()
            .id("library")
            .when(self.a11y, |button| {
                button.button_named(self.button_name(command).unwrap_or(""))
            })
            .absolute()
            .left(px(LIBRARY_BACK_INSET))
            .bottom(px((FOOTER_HEIGHT - NAV_SIZE) / 2.))
            .size(px(NAV_SIZE))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(NAV_RADIUS))
            .block_mouse_except_scroll()
            .map(|button| self.with_tooltip(button, command, TipSide::Beside, cx))
            .hover(|style| style.bg(self.palette.plate(STATE_HOVERED)))
            .active(|style| style.bg(self.palette.plate(STATE_PRESSED)))
            // A button has one hover handler, so this one also carries the tooltip's.
            .on_hover(cx.listener(move |shell, hovered: &bool, window, cx| {
                shell.hover_button(command, TipSide::Beside, *hovered, cx);
                shell.hover_recent_downloads(false, *hovered, window, cx)
            }))
            .on_click(cx.listener(move |shell, _, _, cx| shell.run(command, "sidebar", cx)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|shell, event: &MouseDownEvent, _, cx| {
                    shell.hover_kinds_menu(event.position, cx)
                }),
            )
            // A task's card folding into it bumps its glyph as it lands.
            .child({
                let (scale, alpha) = self.archive_glyph(web_time::Instant::now());
                svg()
                    .path("icons/library.svg")
                    .size(px(16.))
                    .text_color(self.palette.glyph(alpha))
                    .when(scale != 1., |glyph| {
                        glyph.with_transformation(gpui::Transformation::scale(gpui::size(
                            scale, scale,
                        )))
                    })
            })
            .when_some(progress, |button, done| {
                button.child(
                    div()
                        .absolute()
                        .left(px((NAV_SIZE - BUTTON_PROGRESS_WIDTH) / 2.))
                        .bottom(px(BUTTON_PROGRESS_BOTTOM))
                        .w(px(BUTTON_PROGRESS_WIDTH))
                        .h(px(BUTTON_PROGRESS_HEIGHT))
                        .rounded(px(BUTTON_PROGRESS_HEIGHT / 2.))
                        .bg(self.palette.faint(PROGRESS_TRACK))
                        .child(
                            div()
                                .h_full()
                                .w(relative(done))
                                .rounded(px(BUTTON_PROGRESS_HEIGHT / 2.))
                                .bg(self.palette.glyph(PROGRESS_FILL)),
                        ),
                )
            })
    }
}

/// How far the running downloads have got in all, 0 to 1, or none while none runs. A held one,
/// which Chrome keeps running until it's kept or deleted, and one whose size isn't known don't
/// count, as they'd leave a bar that never moves.
fn downloads_progress(downloads: &[Download]) -> Option<f32> {
    let mut running = downloads
        .iter()
        .filter(|each| each.state == DownloadState::Running && !each.held && each.total > 0);
    let first = running.next()?;
    let running = std::iter::once(first).chain(running);
    let (received, total) = running.fold((0, 0), |(received, total), each| {
        (received + each.received.min(each.total), total + each.total)
    });
    Some(received as f32 / total as f32)
}

/// What the footer's button draws of `downloads`: its bar's length in quarter points, if it has
/// one. Smaller steps don't show.
fn bar_drawn(downloads: &[Download]) -> Option<u32> {
    downloads_progress(downloads).map(|done| (done * BUTTON_PROGRESS_WIDTH * 4.).round() as u32)
}

/// A row of a Space's card, `depth` folders in.
/// A tab's row dragged from a Space's card in the Library: the tab, its Space and how deep in its
/// folders it is.
#[derive(Clone, Copy)]
pub(super) struct CardTab {
    tab: u64,
    space: u64,
    depth: usize,
}

/// The drag's view, which draws the row from the Shell as it is each frame.
struct CardGhost {
    shell: WeakEntity<Shell>,
    dragged: CardTab,
}

impl Render for CardGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shell = self.shell.upgrade();
        div().children(shell.map(|shell| shell.read(cx).card_ghost(self.dragged)))
    }
}

fn card_row(depth: usize, plate: impl IntoElement) -> AnyElement {
    row_box()
        .pl(px(PLATE_INSET + depth as f32 * FOLDER_INDENT))
        .child(plate)
        .into_any_element()
}

/// Where a card's left edge is in the Spaces section, the `index`th from the Library's left.
fn card_x(index: usize) -> f32 {
    COLUMN_WIDTH + SPACES_LEFT + index as f32 * (CARD_WIDTH + CARD_GAP)
}

/// Where the +'s left edge is after `cards` cards.
fn plus_x(cards: usize) -> f32 {
    card_x(cards.max(1) - 1) + CARD_WIDTH + PLUS_GAP
}

/// How wide the Spaces section's contents are with `cards` of them: to the room after the +.
pub(super) fn spaces_content(cards: usize) -> f32 {
    plus_x(cards) + PLUS_SIZE + PLUS_AFTER
}

/// How wide the Spaces section is with `cards` of them in a window `window` wide: its contents',
/// but leaving the page `SPACES_PAGE` however narrow the window, its cards scrolling past that
/// (Arc 1.171), and always with its column.
fn spaces_width(cards: usize, window: f32) -> f32 {
    spaces_content(cards)
        .min(window - SPACES_PAGE)
        .max(COLUMN_WIDTH)
}

/// How far, in points, fingers past where the Library opens take it on, its card `out` of its way
/// there (`library_out`) and it `width` wide: its width over the card's way as far, so it keeps
/// the card's pace as one sheet, 1.4 times the card's way with three Spaces (Arc 1.170 rec).
fn past_end(out: f32, width: f32) -> f32 {
    width * (out - 1.).max(0.)
}

/// The sidebar's opacity as it recedes with the Library `out` (`library_out`).
fn receded_opacity(out: f32) -> f32 {
    (1. - out / RECEDE_SPAN).clamp(0., 1.)
}

/// Whether the Library's panel, `width` wide with its edge `edge` of the way (`library_edge`), is
/// at rest, to within `LIBRARY_REST_SLACK`.
fn panel_at_rest(edge: f32, width: f32) -> bool {
    (1. - edge) * width < LIBRARY_REST_SLACK
}

/// How far the card goes, in points, for fingers' `way` past where the Library opens.
pub(super) fn library_band(way: f32) -> f32 {
    super::band(way, LIBRARY_BAND, LIBRARY_BAND_STIFFNESS)
}

/// A line of a row's text, cut short with an ellipsis.
fn text_line(text: SharedString, size: f32, height: f32, color: Hsla) -> gpui::Div {
    div()
        .h(px(height))
        .flex()
        .items_center()
        .text_size(px(size))
        .text_color(color)
        .child(div().truncate().child(text))
}

/// A size as Finder writes it, in powers of 1000.
pub(crate) fn bytes(bytes: i64) -> String {
    let bytes = bytes.max(0) as f64;
    match bytes {
        b if b < 1e3 => format!("{b} bytes"),
        b if b < 1e6 => format!("{:.0} KB", b / 1e3),
        b if b < 1e9 => format!("{:.1} MB", b / 1e6),
        b => format!("{:.2} GB", b / 1e9),
    }
}

/// How much of a download has come, as Safari writes it.
pub(super) fn of(received: i64, total: i64) -> String {
    match total > 0 {
        true => format!("{} of {}", bytes(received), bytes(total)),
        false => bytes(received),
    }
}

#[cfg(test)]
mod agents_tests;
#[cfg(test)]
mod drag_tests;
#[cfg(test)]
mod peek_tests;
#[cfg(test)]
mod private_tests;
#[cfg(test)]
mod rename_tests;
#[cfg(test)]
mod task_tests;

#[cfg(test)]
mod tests {
    use super::{
        ARCHIVE_TOP, ARCHIVE_WIDTH, ARCHIVED_LEFT, Archived, CARD_WIDTH, COLUMN_WIDTH, Captured,
        Download, DownloadAction, DownloadChoice, DownloadState, FILTER_WIDTH, FILTER_X,
        HEADING_BOTTOM, HEADING_HEIGHT, HEADING_TOP, HEADING_X, Item, LIBRARY_BAND, Line,
        MEDIA_GAP, MEDIA_TILE, Medium, SEARCH_HEIGHT, SEARCH_TOP, SECTION_WIDTH, SECTION_X, Space,
        bar_drawn, card_x, download_menu_items, downloads_progress, library_band, panel_at_rest,
        past_end, plus_x, receded_opacity, spaces_width,
    };
    use std::rc::Rc;

    use gpui::{Modifiers, MouseButton, TestAppContext, point, px};

    use crate::shell::recorder::Recorder;
    use crate::shell::sidebar::window_buttons_at;
    use crate::shell::{Folders, Section, Shell, Split, rubber_band};

    fn space(pinned: &[u64], today: &[u64], folders: Folders, splits: Vec<Split>) -> Space {
        Space {
            id: 1,
            name: "Space 1".into(),
            color: None,
            theme: None,
            icon: Default::default(),
            profile: 0,
            agent: Default::default(),
            pinned: pinned.to_vec(),
            today: today.to_vec(),
            splits,
            folders,
            shown: 0,
            peeked: Vec::new(),
        }
    }

    /// A card lists what the sidebar does: folders with what's in them an open folder's depth in,
    /// a split view by its first tab, and a line over the Today tabs only if there are any.
    #[test]
    fn a_card_lists_the_rows_of_the_sidebar() {
        let folders = Folders {
            folders: [(10, ("Docs".into(), true))].into(),
            parents: [(2, 10)].into(),
            ..Default::default()
        };
        let split = Split {
            tabs: vec![5, 6],
            ..Split::default()
        };
        let card = space(&[1, 10, 2], &[4, 5, 6], folders, vec![split]);
        assert_eq!(
            card.card_lines(),
            [
                Line::Tab(1, 0),
                Line::Folder(10, 0),
                Line::Tab(2, 1),
                Line::Divider,
                Line::Tab(4, 0),
                Line::Tab(5, 0),
            ]
        );
        let one = space(&[1], &[], Folders::default(), Vec::new());
        assert_eq!(one.card_lines(), [Line::Tab(1, 0)]);
    }

    /// What's in a closed folder is left out of its card, as of the sidebar, except the tab the
    /// Space shows.
    #[test]
    fn a_closed_folder_hides_its_tabs_but_the_one_shown() {
        let folders = Folders {
            folders: [(10, ("Docs".into(), false))].into(),
            parents: [(2, 10), (3, 10)].into(),
            ..Default::default()
        };
        let mut card = space(&[10, 2, 3], &[], folders, Vec::new());
        assert_eq!(card.card_lines(), [Line::Folder(10, 0)]);
        card.shown = 3;
        assert_eq!(card.card_lines(), [Line::Folder(10, 0), Line::Tab(3, 1)]);
    }

    /// The tab shown two closed folders down is listed one under the outermost, the folder
    /// between staying hidden, and the outermost marks it with its dots, as Arc's does (#1405).
    /// One folder down, it's listed where it is, with no dots.
    #[test]
    fn the_tab_shown_in_nested_closed_folders_is_listed_under_the_outermost() {
        let folders = |inner_open| Folders {
            folders: [
                (10, ("Work".into(), false)),
                (11, ("Plans".into(), inner_open)),
            ]
            .into(),
            parents: [(11, 10), (3, 11)].into(),
            ..Default::default()
        };
        for inner_open in [false, true] {
            let mut card = space(&[10, 11, 3, 4], &[], folders(inner_open), Vec::new());
            card.shown = 3;
            assert_eq!(
                card.card_lines(),
                [Line::Folder(10, 0), Line::Tab(3, 1), Line::Tab(4, 0)],
                "Plans open: {inner_open}"
            );
            assert_eq!(card.dotted(), [(10, 3)], "Plans open: {inner_open}");
            assert_eq!(card.depth(3), 2, "the drop's depth is where it is");
        }

        // With Work open and Plans closed, Plans is listed and the tab one under it, as ever.
        let mut card = space(&[10, 11, 3], &[], folders(false), Vec::new());
        card.folders.folders.insert(10, ("Work".into(), true));
        card.shown = 3;
        assert_eq!(
            card.card_lines(),
            [Line::Folder(10, 0), Line::Folder(11, 1), Line::Tab(3, 2)]
        );
        assert_eq!(card.dotted(), []);
        // Nothing shown there, nothing is listed in them.
        card.shown = 0;
        assert_eq!(card.dotted(), []);
    }

    /// A right-click on a card shows its Space's menu at the pointer, as a right-click on the
    /// sidebar's name does, and leaves the Space shown and the Library open.
    #[gpui::test]
    fn a_right_click_on_a_card_shows_its_space_s_menu(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        // The second Space has a colour, so its menu, unlike the first's, offers to remove it.
        shell.update(cx, |shell, cx| {
            let space = |id: u64, color| {
                (
                    id,
                    format!("Space {id}").into(),
                    color,
                    1,
                    "".into(),
                    "".into(),
                    "".into(),
                )
            };
            shell.set_spaces(vec![space(1, None), space(2, Some(0x3d81f9))], cx);
            shell.set_space(1, cx);
            shell.section = Section::Spaces;
            shell.set_library(true, cx);
            // At rest where it opens.
            shell.library_motion = None;
            shell.sidebar_fade = None;
        });
        cx.run_until_parked();
        host.take();
        let height = cx.update(|window, _| f32::from(window.viewport_size().height));
        let card = point(px(card_x(1) + CARD_WIDTH / 2.), px(height / 2.));
        cx.simulate_mouse_down(card, MouseButton::Right, Modifiers::none());
        cx.simulate_mouse_up(card, MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
        let titles = host.menus.borrow().last().cloned().unwrap_or_default();
        assert!(
            titles.iter().any(|(title, _)| title == "Rename Space…"),
            "no Space menu"
        );
        assert!(
            host.shown.borrow().is_empty(),
            "the right-click showed the Space"
        );
        assert!(
            shell.read_with(cx, |shell, _| shell.library_open),
            "the Library closed"
        );
        cx.update(|_, cx| host.choose("Remove Theme Color", cx));
        assert_eq!(host.take(), ["space theme 2 none", "space color 2 none"]);
    }

    /// A right-click on the Archived Tabs section offers Clear Archive, which has nothing to do
    /// while the archive is empty.
    /// With nothing in it, each list's line wraps in the list's width, inside its margins, rather
    /// than keeping its one line's width under the column and the card (#828).
    #[gpui::test]
    fn an_empty_section_s_line_wraps_inside_its_list(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        for section in [Section::Media, Section::Downloads, Section::Archive] {
            shell.update(cx, |shell, cx| {
                if shell.library_open {
                    shell.show_section(section, cx);
                } else {
                    shell.section = section;
                    shell.set_library(true, cx);
                }
                shell.library_motion = None;
                shell.sidebar_fade = None;
                cx.notify();
            });
            cx.run_until_parked();
            let drawn = shell.read_with(cx, |shell, _| shell.empty_drawn.borrow().clone());
            let words: Vec<&str> = drawn.split(' ').collect();
            let (x, width): (f32, f32) = (words[2].parse().unwrap(), words[3].parse().unwrap());
            assert_eq!(words[0], section.name(), "{drawn}");
            assert!(x >= COLUMN_WIDTH + HEADING_X, "{drawn}");
            assert!(
                x + width <= COLUMN_WIDTH + ARCHIVE_WIDTH - HEADING_X,
                "{drawn}"
            );
        }
    }

    #[gpui::test]
    fn a_right_click_on_archived_tabs_offers_clear_archive(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            shell.set_library(true, cx);
            shell.library_motion = None;
            shell.sidebar_fade = None;
        });
        cx.run_until_parked();
        let height = cx.update(|window, _| f32::from(window.viewport_size().height));
        // The fourth button, Archived Tabs, the column's six centred 5 above its middle, 100 apart.
        let section = point(px(SECTION_X + SECTION_WIDTH / 2.), px(height / 2. + 45.));
        let menu = |cx: &mut gpui::VisualTestContext| {
            cx.simulate_mouse_down(section, MouseButton::Right, Modifiers::none());
            cx.simulate_mouse_up(section, MouseButton::Right, Modifiers::none());
            cx.run_until_parked();
            host.menus.borrow().last().cloned().unwrap_or_default()
        };
        assert_eq!(menu(cx), [("Clear Archive".into(), false)]);
        let tab = Archived {
            id: 7,
            url: "https://example.org/".into(),
            title: "".into(),
            archived: 0.,
            little: false,
        };
        shell.update(cx, |shell, cx| shell.set_archive(vec![tab], cx));
        assert_eq!(menu(cx), [("Clear Archive".into(), true)]);
        assert!(
            shell.read_with(cx, |shell, _| shell.library_open),
            "the Library closed"
        );
    }

    /// Archived Tabs grows a Filter button once a Little ilo window's tab is archived, and its
    /// menu lists Little ilo's alone, then all again.
    #[gpui::test]
    fn the_archive_filters_to_little_ilos_tabs(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        let tab = |id, little| Archived {
            id,
            url: format!("https://{id}.example/").into(),
            title: "".into(),
            archived: 0.,
            little,
        };
        shell.update(cx, |shell, cx| {
            shell.section = Section::Archive;
            shell.set_library(true, cx);
            shell.library_motion = None;
            shell.sidebar_fade = None;
            shell.set_archive(vec![tab(1, false), tab(2, false)], cx);
        });
        cx.run_until_parked();
        let filter = point(
            px(FILTER_X + FILTER_WIDTH / 2.),
            px(SEARCH_TOP + SEARCH_HEIGHT / 2.),
        );
        let click = |cx: &mut gpui::VisualTestContext| {
            cx.simulate_click(filter, Modifiers::none());
            cx.run_until_parked();
        };
        let tabs = |cx: &mut gpui::VisualTestContext| {
            shell.read_with(cx, |shell, _| {
                let items = &shell.library.as_ref().unwrap().items;
                let ids = items.iter().filter_map(|item| match item {
                    Item::Tab(index) => Some(shell.archive[*index].id),
                    _ => None,
                });
                ids.collect::<Vec<_>>()
            })
        };
        // No Little ilo tab, no button: the click lands on the field.
        click(cx);
        assert!(
            host.menus.borrow().is_empty(),
            "a menu with nothing to filter"
        );

        shell.update(cx, |shell, cx| {
            shell.set_archive(vec![tab(1, false), tab(2, true), tab(3, false)], cx)
        });
        cx.run_until_parked();
        click(cx);
        let menu = host.menus.borrow().last().cloned().unwrap_or_default();
        assert_eq!(
            menu,
            [
                ("All".into(), true),
                ("Little ilo".into(), true),
                ("Tasks".into(), true)
            ]
        );
        assert_eq!(
            host.entries
                .borrow()
                .iter()
                .map(|e| e.checked)
                .collect::<Vec<_>>(),
            [true, false, false]
        );
        cx.update(|_, cx| host.choose("Little ilo", cx));
        cx.run_until_parked();
        assert_eq!(tabs(cx), [2]);

        click(cx);
        assert_eq!(
            host.entries
                .borrow()
                .iter()
                .map(|e| e.checked)
                .collect::<Vec<_>>(),
            [false, true, false]
        );
        cx.update(|_, cx| host.choose("All", cx));
        cx.run_until_parked();
        assert_eq!(tabs(cx), [1, 2, 3]);
    }

    /// Media tiles the captures and the downloaded images and videos together, newest first, three
    /// a row under each day's heading. A click on one opens its file in a tab and closes the
    /// Library.
    #[gpui::test]
    fn media_tiles_captures_and_downloaded_pictures(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        let file = |id, name: &str, state, started| Download {
            id,
            name: name.to_string().into(),
            path: format!("/tmp/{name}").into(),
            state,
            started,
            ..download(DownloadState::Done, false)
        };
        let downloads = vec![
            file(1, "report.pdf", DownloadState::Done, 9.),
            file(2, "my photo.jpg", DownloadState::Done, 8.),
            file(3, "clip.mp4", DownloadState::Done, 6.),
            file(4, "coming.png", DownloadState::Running, 5.),
        ];
        let capture = |name: &str, saved| Captured {
            path: format!("/media/{name}").into(),
            name: name.to_string().into(),
            saved,
        };
        shell.update(cx, |shell, cx| {
            shell.set_downloads(downloads, cx);
            shell.section = Section::Media;
            shell.set_library(true, cx);
            shell.library_motion = None;
            shell.sidebar_fade = None;
        });
        assert!(host.take().contains(&"list captures".to_string()));
        let captures = vec![
            capture("Capture 1.png", 7.),
            capture("Capture 2.png", 3.),
            capture("Old.png", -1.),
        ];
        shell.update(cx, |shell, cx| shell.set_captures(captures, cx));
        let items = shell.read_with(cx, |shell, _| shell.library.as_ref().unwrap().items.clone());
        let (download, capture) = (Medium::Download, Medium::Capture);
        assert!(
            items
                == [
                    Item::Heading("Today"),
                    Item::Media([Some(download(1)), Some(capture(0)), Some(download(2))]),
                    Item::Media([Some(capture(1)), None, None]),
                    Item::Heading("Older"),
                    Item::Media([Some(capture(2)), None, None]),
                ]
        );
        cx.run_until_parked();
        host.take();
        // The first tile, under the first heading.
        let top = ARCHIVE_TOP + HEADING_TOP + HEADING_HEIGHT + HEADING_BOTTOM;
        let left = COLUMN_WIDTH + ARCHIVED_LEFT;
        let first = point(px(left + MEDIA_TILE / 2.), px(top + MEDIA_TILE / 2.));
        cx.simulate_click(first, Modifiers::none());
        cx.run_until_parked();
        let opened = host.take();
        assert!(
            opened
                .iter()
                .any(|call| call.starts_with("open file:///tmp/my%20photo.jpg "))
        );
        assert!(
            !shell.read_with(cx, |shell, _| shell.library_open),
            "the Library stayed open"
        );
    }

    /// A video's tile plays it from its first draw, and stops it once the Library closes.
    #[gpui::test]
    fn a_video_plays_in_its_tile_while_media_shows_it(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        let clip = Download {
            id: 3,
            name: "clip.mp4".into(),
            path: "/tmp/clip.mp4".into(),
            started: 1.,
            ..download(DownloadState::Done, false)
        };
        shell.update(cx, |shell, cx| {
            shell.set_downloads(vec![clip], cx);
            shell.section = Section::Media;
            shell.set_library(true, cx);
            shell.library_motion = None;
            shell.sidebar_fade = None;
        });
        cx.run_until_parked();
        let opened = host.take();
        assert!(
            opened.iter().any(|call| call.starts_with("open video 3 ")),
            "{opened:?}"
        );
        let shown = |cx: &mut gpui::VisualTestContext| {
            let video = |shell: &Shell| shell.media_videos.get(&3).map(|each| each.frame.is_some());
            shell.read_with(cx, |shell, _| video(shell))
        };
        assert_eq!(
            shown(cx),
            Some(true),
            "the tile didn't show the video's frame"
        );
        // Closed at once, as the slide out's frames don't come in a test, nor the frame after the
        // first without the tile, which a window asks for.
        shell.update(cx, |shell, cx| {
            shell.close_library_now();
            cx.notify();
        });
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!(
            host.take().contains(&"close video 3".to_string()),
            "the video played on"
        );
        assert_eq!(shown(cx), None);
    }

    /// A right-click on a capture's tile offers Open and Delete, which moves it to the Trash and
    /// out of Media; one on a downloaded picture's shows the download's menu.
    #[gpui::test]
    fn a_right_click_deletes_a_capture_from_media(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        let photo = Download {
            id: 2,
            name: "photo.jpg".into(),
            path: "/tmp/photo.jpg".into(),
            started: 1.,
            ..download(DownloadState::Done, false)
        };
        let (path, name) = ("/media/Capture.png".into(), "Capture.png".into());
        let capture = Captured {
            path,
            name,
            saved: 2.,
        };
        shell.update(cx, |shell, cx| {
            shell.set_downloads(vec![photo], cx);
            shell.set_captures(vec![capture], cx);
            shell.section = Section::Media;
            shell.set_library(true, cx);
            shell.library_motion = None;
            shell.sidebar_fade = None;
        });
        cx.run_until_parked();
        let top = ARCHIVE_TOP + HEADING_TOP + HEADING_HEIGHT + HEADING_BOTTOM + MEDIA_TILE / 2.;
        let left = COLUMN_WIDTH + ARCHIVED_LEFT + MEDIA_TILE / 2.;
        let menu = |x: f32, cx: &mut gpui::VisualTestContext| {
            let at = point(px(x), px(top));
            cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
            cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
            cx.run_until_parked();
            host.menus.borrow().last().cloned().unwrap_or_default()
        };
        // The capture is newer, so first; the picture next to it.
        let titles = |menu: Vec<(gpui::SharedString, bool)>| {
            menu.into_iter()
                .map(|(title, _)| title.to_string())
                .collect::<Vec<_>>()
        };
        assert!(titles(menu(left + MEDIA_TILE + MEDIA_GAP, cx)).contains(&"Move to Trash".into()));
        cx.update(|_, cx| host.dismiss(cx));
        assert_eq!(titles(menu(left, cx)), ["Open", "", "Delete"]);
        host.take();
        cx.update(|_, cx| host.choose("Delete", cx));
        cx.run_until_parked();
        assert_eq!(host.take(), ["trash /media/Capture.png", "list captures"]);
        let items = shell.read_with(cx, |shell, _| shell.library.as_ref().unwrap().items.clone());
        let left = Item::Media([Some(Medium::Download(0)), None, None]);
        assert!(items == [Item::Heading("Today"), left]);
    }

    /// Past its end, the Library gives about as far as the Space pages do past theirs on a short
    /// pull, so it's felt, and pulls back harder on a long one, never as far as the band's limit.
    #[test]
    fn past_its_end_the_library_gives_and_pulls_back() {
        for way in [20., 100., 300., 600., 2000.] {
            let (library, page) = (library_band(way), rubber_band(way, 0., 0.));
            let gives = if way <= 300. {
                library > 0.9 * page
            } else {
                library < 0.8 * page
            };
            assert!(gives, "{library} past the Library, {page} past a page");
        }
        assert!(library_band(600.) < library_band(2000.) && library_band(2000.) < LIBRARY_BAND);
    }

    /// Where the left edge of the Library `width` wide is with the card `out` of its way there.
    fn sheet_left(out: f32, width: f32) -> f32 {
        (out.min(1.) - 1.) * width + past_end(out, width)
    }

    /// Every section Arc's recordings show moving comes in as one sheet at the card's pace,
    /// the sidebar gone by 0.48 of the way: Spaces with one Space, opening from a sidebar 228 wide
    /// to 480 (the + measured), and Media closing from 417 (its search field measured).
    #[test]
    fn every_section_comes_in_as_one_sheet_with_the_card() {
        // The page card's left and the +'s left, 413 in the Library at rest.
        for (card, plus) in [(309.5, 88.), (365., 195.), (442., 341.)] {
            let left = sheet_left((card - 228.) / 252., 480.) + 413.;
            assert!((left - plus).abs() < 1.5, "the + at {left}, not {plus}");
        }
        // The page card's left and how far the sheet is from its place.
        for (card, sheet) in [(413., -8.), (376., -90.), (340.5, -169.), (305.5, -246.)] {
            let left = sheet_left((card - 228.) / 189., 417.);
            assert!(
                (left - sheet).abs() < 1.5,
                "the sheet at {left}, not {sheet}"
            );
        }
        // How far the card has gone and the sidebar's contrast: one Space opening, Media closing.
        let sidebars = [
            (0.107, 0.78),
            (0.202, 0.6),
            (0.28, 0.45),
            (0.331, 0.34),
            (0.138, 0.71),
        ];
        for (out, sidebar) in sidebars {
            let faded = receded_opacity(out);
            assert!(
                (faded - sidebar).abs() < 0.05,
                "the sidebar at {faded} at {out}"
            );
        }
    }

    /// The Spaces section is as wide as Arc's: 480 with one Space and 1008 with three, the cards
    /// 264 apart from 140 and the + 33 after the last. Once they don't fit it leaves the page 56,
    /// however narrow the window, as Arc 1.171's with 8 Spaces: 1044 at 1100 and 444 at 500.
    #[test]
    fn the_spaces_section_grows_with_its_spaces() {
        assert_eq!(spaces_width(1, 1800.), 480.);
        assert_eq!(spaces_width(3, 1800.), 1008.);
        assert_eq!([card_x(0), card_x(1), card_x(2)], [140., 404., 668.]);
        assert_eq!(plus_x(3), 941.);
        assert_eq!(spaces_width(0, 1800.), 480.);
        assert_eq!(spaces_width(8, 1100.), 1044.);
        assert_eq!(spaces_width(8, 500.), 444.);
        assert_eq!(spaces_width(10, 1800.), 1744.);
        assert_eq!(spaces_width(3, 1200.), 1008.);
        assert_eq!(spaces_width(3, 700.), 644.);
        assert_eq!(spaces_width(1, 500.), 444.);
        // The column stays, in a window narrower than any.
        assert_eq!(spaces_width(3, 100.), COLUMN_WIDTH);
    }

    /// With three Spaces in Arc's 1800-wide window, the card going from 288 to 1008, the Library
    /// is one sheet whose edge keeps the card's pace, as measured frame by frame, the
    /// sidebar under it gone by 0.48 of the way, and past where it opens it goes on 1.4 times as
    /// far as the card.
    #[test]
    fn the_spaces_section_comes_in_with_the_card() {
        // The page card's left and the second card's, measured, both ways.
        let cards = [
            (565., -216.),
            (630., -125.5),
            (734.5, 21.),
            (905., 260.),
            (947.5, 320.),
        ];
        for (page, red) in cards {
            let left = sheet_left((page - 288.) / 720., 1008.) + card_x(1);
            assert!((left - red).abs() < 2., "the card at {left}, not {red}");
        }
        // How far the card has gone and the sidebar's opacity, measured, both ways.
        let sidebars = [
            (0.17, 0.66),
            (0.32, 0.36),
            (0.43, 0.05),
            (0.16, 0.68),
            (0.38, 0.22),
        ];
        for (out, sidebar) in sidebars {
            let faded = receded_opacity(out);
            assert!(
                (faded - sidebar).abs() < 0.06,
                "the sidebar at {faded} at {out}"
            );
        }
        assert_eq!(receded_opacity(0.48), 0.);
        let past = past_end(1. + 72. / 720., 1008.);
        assert!((past / 72. - 1.4).abs() < 0.001, "{past} past its end");
    }

    /// How the window's buttons are drawn with the Library `out` of the way (`library_out`),
    /// moving, as (its own, the panel's copy, the sidebar's copy).
    fn buttons(open: bool, out: f32, docked: bool) -> (bool, bool, bool) {
        let at_rest = panel_at_rest(out.min(1.), 417.);
        let drawn = window_buttons_at(open, true, at_rest, docked);
        (drawn.window, drawn.library, drawn.sidebar)
    }

    /// Opening, the panel's copy slides in with it and the window's own take over once the panel
    /// has stopped, within a pixel's half of where its copy was, and stay.
    #[test]
    fn the_windows_buttons_slide_in_with_the_library_and_take_over_at_rest() {
        let mut real = false;
        for step in 0..=1200 {
            let out = step as f32 / 1000.;
            let (window, panel, sidebar) = buttons(true, out, true);
            assert!(!(window && (panel || sidebar)), "drawn twice at {out}");
            assert!(window || panel, "missing at {out}");
            assert!(!real || window, "the window's own went again at {out}");
            if window && !real {
                let offset = (1. - out.min(1.)) * 417.;
                assert!(offset < 0.25, "swapped {offset} pt from the copy's place");
            }
            real |= window;
        }
        assert!(real);
    }

    /// Closing is the same the other way, whether the sidebar is docked or hidden, and the sidebar
    /// draws its copy only once the window's own have gone.
    #[test]
    fn the_windows_buttons_leave_with_the_library() {
        for docked in [true, false] {
            let mut gone = false;
            for step in (0..=1200).rev() {
                let out = step as f32 / 1000.;
                let (window, panel, sidebar) = buttons(false, out, docked);
                assert!(!(window && (panel || sidebar)), "drawn twice at {out}");
                assert!(window || panel, "missing at {out}");
                assert!(!(gone && window), "the window's own came back at {out}");
                gone |= !window;
            }
            assert!(gone);
        }
    }

    /// Fingers let go short of halfway take the Library back: the window's own never show, and the
    /// copies are all there is.
    #[test]
    fn a_swipe_let_go_short_keeps_the_windows_own_buttons_away() {
        for step in 0..=500 {
            let (window, panel, sidebar) = buttons(false, step as f32 / 1000., true);
            assert!(!window && panel && sidebar, "at {step}");
        }
    }

    /// At rest the window's own show over the open Library and the docked sidebar, and with
    /// neither, nobody draws any.
    #[test]
    fn the_windows_buttons_at_rest() {
        let rest = |open, docked| window_buttons_at(open, false, true, docked);
        assert!(rest(true, false).window && !rest(true, false).library);
        assert!(rest(false, true).window && !rest(false, true).library);
        let none = rest(false, false);
        assert!(!none.window && !none.library && !none.sidebar);
        // Taken past where it opens, the panel is still at rest, its column under them.
        assert!(panel_at_rest(1., 417.) && !panel_at_rest(0.99, 417.));
    }

    fn download(state: DownloadState, held: bool) -> Download {
        Download {
            id: 1,
            name: "report.pdf".into(),
            url: "https://example.com/report.pdf".into(),
            path: "/Users/someone/Downloads/report.pdf".into(),
            received: 0,
            total: 0,
            state,
            open_when_done: false,
            held,
            started: 0.,
            profile: 0,
            private: false,
        }
    }

    /// A finished file's menu has Move to Trash after Open and Show in Finder; one still coming,
    /// held, cancelled or failed has no file of its own to move.
    #[test]
    fn only_a_finished_file_goes_to_the_trash() {
        use DownloadState::{Cancelled, Done, Failed, Paused, Running};
        let done = download_menu_items(&download(Done, false), true);
        let titles: Vec<_> = done.iter().map(|(entry, _)| entry.title.as_ref()).collect();
        assert_eq!(
            titles,
            [
                "Open",
                "Show in Finder",
                "Move to Trash",
                "",
                "Remove from List",
                "Clear List"
            ]
        );
        assert_eq!(done[2].1, Some(DownloadChoice::Trash));
        assert_eq!(done[4].1, Some(DownloadChoice::Act(DownloadAction::Remove)));
        let trashes = |state, held| {
            download_menu_items(&download(state, held), true)
                .iter()
                .any(|(_, choice)| *choice == Some(DownloadChoice::Trash))
        };
        for state in [Running, Paused, Cancelled, Failed] {
            assert!(!trashes(state, false));
            assert!(!trashes(state, true));
        }
        assert!(!trashes(Done, true));
    }

    fn running(received: i64, total: i64) -> Download {
        Download {
            received,
            total,
            ..download(DownloadState::Running, false)
        }
    }

    /// The footer's bar is all the running downloads' progress together, and only they have one.
    /// One held, or of a size not yet known, never moves, so it has none and leaves the rest alone.
    #[test]
    fn the_footer_bar_follows_the_running_downloads() {
        let done = download(DownloadState::Done, false);
        assert_eq!(downloads_progress(&[done.clone()]), None);
        assert_eq!(downloads_progress(&[]), None);
        assert_eq!(
            downloads_progress(&[running(50, 100), done.clone()]),
            Some(0.5)
        );
        assert_eq!(
            downloads_progress(&[running(50, 100), running(0, 100)]),
            Some(0.25)
        );
        // A download is never past its end.
        assert_eq!(downloads_progress(&[running(300, 100)]), Some(1.));
        let held = Download {
            received: 0,
            total: 100,
            ..download(DownloadState::Running, true)
        };
        assert_eq!(downloads_progress(&[running(7, 0)]), None);
        assert_eq!(downloads_progress(&[held.clone()]), None);
        assert_eq!(
            downloads_progress(&[held, running(7, 0), running(25, 100)]),
            Some(0.25)
        );
    }

    /// A download's report that moves the bar less than it can draw is not worth a frame, while
    /// one that finishes it, or starts another, is. One of a size not yet known, or held, draws no
    /// bar, so its reports are never worth one.
    #[test]
    fn the_bar_draws_a_frame_only_when_it_moves() {
        let gigabyte = 1_000_000_000;
        let bar = |received| bar_drawn(&[running(received, gigabyte)]);
        assert_eq!(bar(1_000), bar(2_000));
        assert_ne!(bar(0), bar(gigabyte / 4));
        assert_eq!(bar(gigabyte), Some(64));
        assert_eq!(bar_drawn(&[download(DownloadState::Done, false)]), None);
        assert_ne!(bar_drawn(&[]), bar(0));
        let unknown = |received| bar_drawn(&[running(received, 0)]);
        assert_eq!(unknown(1_000), None);
        assert_eq!(unknown(1_000), unknown(gigabyte));
        let held = |received| {
            bar_drawn(&[Download {
                received,
                total: gigabyte,
                held: true,
                ..running(0, 0)
            }])
        };
        assert_eq!(held(0), held(gigabyte / 4));
    }
}
