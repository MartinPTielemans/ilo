//! The UI alone, with made-up tabs, for working on how it looks: it needs no engine and rebuilds in
//! seconds. `cargo run --release --bin preview`. The browser's own rules for tabs live in the core,
//! so the ones here are only stand-ins. Built for the web, it's the site's live demo instead (`web`),
//! which draws the Spaces the site gives it and leaves the made-up ones and the env switches unused.
#![cfg_attr(target_family = "wasm", allow(dead_code, unused_imports))]

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    time::Duration,
};
use web_time::{SystemTime, UNIX_EPOCH};

use browser::account::{self, Account};
use browser::agents::task::{
    Card as TaskCard, Conversation, Dot, Line, Opens, Trace, TraceItem, TraceStep,
};
use browser::boost_editor;
use browser::boost_style::{self, Case, Face, Style};
use browser::command_bar::{self, Suggestion};
use browser::dialog;
use browser::foot_bar;
use browser::icon_picker::{self, Recents};
use browser::import::sources::{Os, Roots};
use browser::link_preview;
use browser::log;
use browser::menu;
use browser::palette::{self, Palette, Tone};
use browser::settings_card::Card;
use browser::shell::{
    self, Archived, BarTab, Browser, Command, Download, DownloadAction, DownloadState, Folders,
    Match, MediaAction, MenuBar, MenuEntry, Page, Shell, SpaceEntry, Split, Trouble, Volume,
    WindowButton,
};
use browser::site_controls::{self, Connection, Permission, Site, Value};
use browser::theme::{Appearance, Theme};
use browser::theme_picker;
use gpui::{
    App, AppContext, Bounds, Focusable, Pixels, Point, SharedString, Size, TitlebarOptions,
    TouchPhase, WeakEntity, WindowBounds, WindowOptions, point, px, size,
};

thread_local! {
    /// The preview window's size, where its menus may go.
    static PREVIEW_AREA: Cell<Size<Pixels>> = Cell::new(size(px(1280.), px(800.)));
    /// Its top left on screen, where an overlay opened later goes from.
    static PREVIEW_ORIGIN: Cell<Point<Pixels>> = Cell::new(point(px(0.), px(0.)));
    /// Whether the dialog's window has opened, the first time the preview asks something.
    static DIALOG_OPEN: Cell<bool> = const { Cell::new(false) };
}

#[cfg(not(target_family = "wasm"))]
unsafe extern "C" {
    // platform/mac/host.mm: where a window goes, centered on the main display or, with
    // BROWSER_DISPLAY=builtin, on the Mac's own.
    fn host_centered(width: f64, height: f64, x: *mut f64, y: *mut f64);
}

/// The web opens no window of its own, so the env switches' windows that call this never open.
#[cfg(target_family = "wasm")]
unsafe fn host_centered(_: f64, _: f64, _: *mut f64, _: *mut f64) {}

// Two Spaces, so the switcher shows, the second in Dia's pink. The tabs are in the first, the top
// two pinned, the second in a folder.
const SPACES: [(&str, Option<u32>); 2] = [("Space 1", None), ("Reading", Some(0xbf556a))];
// The colours of the Spaces past those two, in turn, with PREVIEW_SPACES.
const MORE_COLORS: [u32; 6] = [0x4f7cff, 0x2f9e6e, 0xd98a2b, 0x8a5cd6, 0x2aa3b8, 0xc44f3a];
const TABS: [(&str, &str); 5] = [
    (
        "Arc (web browser) - Wikipedia",
        "https://en.wikipedia.org/wiki/Arc_(web_browser)",
    ),
    ("Hacker News", "https://news.ycombinator.com/"),
    ("Zed — The editor for what's next", "https://zed.dev/"),
    ("Rust Programming Language", "https://www.rust-lang.org/"),
    ("", "https://example.com/"),
];

// The second Space's tabs, the first pinned, for its card in the Library.
const READING: [(&str, &str); 4] = [
    ("The Rust Book", "https://doc.rust-lang.org/book/"),
    ("Lobsters", "https://lobste.rs/"),
    ("Daring Fireball", "https://daringfireball.net/"),
    ("Wikipedia", "https://www.wikipedia.org/"),
];

// The favorites, which every Space shows above its tabs, with made-up icons that theme a selected
// tile each way (docs/ux-spec.md › Favorites): colourful twice, one colour, white, and none.
// PREVIEW_FAVORITE=<index> shows one selected.
const FAVORITES: [(&str, &str, Icon); 6] = [
    ("Gmail", "https://mail.google.com/", Icon::Envelope),
    ("Calendar", "https://calendar.google.com/", Icon::Blocks),
    (
        "GitHub",
        "https://github.com/",
        Icon::Disc([0.12, 0.13, 0.16]),
    ),
    ("YouTube", "https://www.youtube.com/", Icon::Play),
    ("Notes", "https://notes.example/", Icon::Disc([1., 1., 1.])),
    ("Sketch", "https://sketch.example/", Icon::None),
];

#[derive(Clone, Copy)]
enum Icon {
    /// A red tile with a white band.
    Envelope,
    /// Four blocks of different colours, with room around them.
    Blocks,
    /// A circle of one colour on nothing.
    Disc([f32; 3]),
    /// A red rounded tile with a white triangle.
    Play,
    /// No favicon at all, which draws a globe.
    None,
}

impl Icon {
    /// The icon as a 32 px PNG, as Chromium hands the UI a favicon.
    fn png(self) -> Option<Vec<u8>> {
        const SIDE: u32 = 32;
        let rounded = |x: f32, y: f32, edge: f32, radius: f32| {
            let inner = 15.5 - edge - radius;
            let (dx, dy) = ((x - 15.5).abs() - inner, (y - 15.5).abs() - inner);
            dx.max(0.).hypot(dy.max(0.)) <= radius && x.min(y) >= edge && x.max(y) <= 31. - edge
        };
        let pixel = |x: u32, y: u32| -> [f32; 4] {
            let (fx, fy) = (x as f32, y as f32);
            match self {
                Icon::Envelope if rounded(fx, fy, 2., 5.) => {
                    if (12.0..=19.0).contains(&fy) && (7.0..=24.0).contains(&fx) {
                        [1., 1., 1., 1.]
                    } else {
                        [0.86, 0.2, 0.18, 1.]
                    }
                }
                Icon::Blocks if (4.0..=27.0).contains(&fx) && (4.0..=27.0).contains(&fy) => {
                    match (fx > 15., fy > 15.) {
                        (false, false) => [0.26, 0.52, 0.96, 1.],
                        (true, false) => [0.98, 0.74, 0.02, 1.],
                        (false, true) => [0.2, 0.66, 0.33, 1.],
                        (true, true) => [0.92, 0.26, 0.21, 1.],
                    }
                }
                Icon::Disc(colour) if (fx - 15.5).hypot(fy - 15.5) <= 13. => {
                    [colour[0], colour[1], colour[2], 1.]
                }
                Icon::Play if rounded(fx, fy, 1., 8.) => {
                    // Right of x = 12, between the lines from (12, 9) and (12, 22) to (23, 15.5).
                    let inside = fx >= 12. && (fy - 15.5).abs() <= 6.5 * (23. - fx) / 11.;
                    if inside {
                        [1., 1., 1., 1.]
                    } else {
                        [0.9, 0.1, 0.1, 1.]
                    }
                }
                _ => [0.; 4],
            }
        };
        if matches!(self, Icon::None) {
            return None;
        }
        let image = image::RgbaImage::from_fn(SIDE, SIDE, |x, y| {
            image::Rgba(pixel(x, y).map(|c| (c * 255.).round() as u8))
        });
        let mut png = std::io::Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png).ok()?;
        Some(png.into_inner())
    }
}

// The archive: tabs closed hours and days ago, newest first.
const ARCHIVE: [(&str, &str, f64); 6] = [
    ("Wikipedia", "https://www.wikipedia.org/", 0.05),
    (
        "Internet Assigned Numbers Authority",
        "https://www.iana.org/",
        0.2,
    ),
    ("W3C", "https://www.w3.org/", 1.1),
    (
        "The GNU Operating System and the Free Software Movement",
        "https://www.gnu.org/",
        2.5,
    ),
    ("Chromium", "https://www.chromium.org/", 6.),
    ("", "https://example.org/", 12.),
];
const DAY: f64 = 86_400_000.;

// The downloads: one running, one paused, one held, and finished ones, newest first, with their
// bytes so far and in all, and how many days ago they started.
const DOWNLOADS: [(&str, &str, i64, i64, DownloadState, bool, f64); 6] = [
    (
        "zed-0.170.dmg",
        "https://zed.dev/",
        48_200_000,
        121_500_000,
        RUNNING,
        false,
        0.,
    ),
    (
        "rust-book.pdf",
        "https://doc.rust-lang.org/",
        3_100_000,
        7_800_000,
        PAUSED,
        false,
        0.01,
    ),
    (
        "installer.command",
        "http://example.com/",
        2_000,
        4_000,
        RUNNING,
        true,
        0.02,
    ),
    (
        "report.csv",
        "https://data.example.org/",
        18_400,
        18_400,
        DONE,
        false,
        0.3,
    ),
    (
        "photo.jpg",
        "https://commons.wikimedia.org/",
        2_480_000,
        2_480_000,
        DONE,
        false,
        1.2,
    ),
    (
        "old-notes.txt",
        "https://example.net/",
        0,
        0,
        DownloadState::Cancelled,
        false,
        9.,
    ),
];
const RUNNING: DownloadState = DownloadState::Running;
const PAUSED: DownloadState = DownloadState::Paused;
const DONE: DownloadState = DownloadState::Done;

/// Reports the Space shown with `today` as its Today tabs.
fn set_today(shell: &mut Shell, today: Vec<u64>, cx: &mut gpui::Context<Shell>) {
    set_lists(shell, shell.pinned().to_vec(), today, cx);
}

fn set_lists(shell: &mut Shell, pinned: Vec<u64>, today: Vec<u64>, cx: &mut gpui::Context<Shell>) {
    let (splits, folders) = (shell.splits(), shell.folders());
    shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
}

/// The folders holding `row`, innermost first.
fn holders(folders: &Folders, row: u64) -> impl Iterator<Item = u64> + '_ {
    std::iter::successors(folders.parents.get(&row).copied(), |folder| {
        folders.parents.get(folder).copied()
    })
}

/// Where `row`'s rows end in `pinned`: past what's in it, if it's a folder.
fn end_of(pinned: &[u64], folders: &Folders, row: u64) -> usize {
    let at = pinned
        .iter()
        .position(|&id| id == row)
        .map_or(0, |at| at + 1);
    let within = pinned[at..]
        .iter()
        .take_while(|&&id| holders(folders, id).any(|f| f == row));
    at + within.count()
}

/// Where `index` in `folder`, else in the pinned tabs, is in `pinned`, as the core counts.
fn pinned_at(pinned: &[u64], folders: &Folders, folder: u64, index: usize) -> usize {
    let start = pinned
        .iter()
        .position(|&id| id == folder)
        .map_or(0, |at| at + 1);
    let end = if folder == 0 {
        pinned.len()
    } else {
        end_of(pinned, folders, folder)
    };
    let parent = |at: &usize| folders.parents.get(&pinned[*at]).copied().unwrap_or(0) == folder;
    (start..end).filter(parent).nth(index).unwrap_or(end)
}

/// Moves `row`, with what's in it if it's a folder, to `index` in `folder`, else in the pinned
/// tabs, as the core does.
fn move_pinned(
    shell: &mut Shell,
    row: u64,
    folder: u64,
    index: usize,
    cx: &mut gpui::Context<Shell>,
) {
    let (mut pinned, mut folders) = (shell.pinned().to_vec(), shell.folders());
    let block = match pinned.iter().position(|&id| id == row) {
        Some(at) => pinned.drain(at..end_of(&pinned, &folders, row)).collect(),
        None => vec![row],
    };
    match folder {
        0 => folders.parents.remove(&row),
        folder => folders.parents.insert(row, folder),
    };
    let to = pinned_at(&pinned, &folders, folder, index);
    pinned.splice(to..to, block);
    let mut today = shell.today().to_vec();
    today.retain(|&id| id != row);
    let splits = shell.splits();
    shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0., |now| now.as_millis() as f64)
}

struct Fake {
    shell: WeakEntity<Shell>,
    /// Shared with the changes it makes later, which make ids too.
    next_id: Rc<Cell<u64>>,
    spaces: RefCell<Vec<SpaceEntry>>,
    archive: RefCell<Vec<(u64, &'static str, &'static str, f64)>>,
    downloads: RefCell<Vec<Download>>,
    /// The onboarding's two switches, both offered and off to begin with.
    opt_ins: Cell<(bool, bool)>,
    /// The tab each Space opens on where the site names one.
    opens_on: RefCell<HashMap<u64, u64>>,
    /// The tab each Space showed as it was left, which it shows again, and its page beside shows
    /// while it pages, as the core keeps them.
    shows: Rc<RefCell<HashMap<u64, u64>>>,
}

/// The tab the shown Space opens on, where the site names none: its first Today tab, the page in
/// use rather than a pinned app, else its first pinned one.
fn opening_tab(shell: &Shell) -> u64 {
    shell
        .today()
        .first()
        .or(shell.pinned().first())
        .copied()
        .unwrap_or(0)
}

impl Fake {
    fn report_archive(&self, cx: &mut App) {
        let archive = self
            .archive
            .borrow()
            .iter()
            .map(|&(id, title, url, archived)| Archived {
                id,
                url: url.into(),
                title: title.into(),
                archived,
                // Every fourth, so the archive's filter shows (Little ilo).
                little: id % 4 == 0,
            })
            .collect();
        self.later(cx, move |shell, cx| shell.set_archive(archive, cx));
    }

    fn report_downloads(&self, cx: &mut App) {
        let downloads = self.downloads.borrow().clone();
        self.later(cx, move |shell, cx| shell.set_downloads(downloads, cx));
    }

    /// Changes a folder's name or whether it's open.
    fn change_folder(
        &self,
        folder: u64,
        cx: &mut App,
        change: impl FnOnce(&mut (SharedString, bool)) + 'static,
    ) {
        self.later(cx, move |shell, cx| {
            let mut folders = shell.folders();
            if let Some(entry) = folders.folders.get_mut(&folder) {
                change(entry);
            }
            let (pinned, today) = (shell.pinned().to_vec(), shell.today().to_vec());
            let splits = shell.splits();
            shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
        });
    }

    fn report_spaces(&self, cx: &mut App) {
        let spaces = self.spaces.borrow().clone();
        self.later(cx, move |shell, cx| shell.set_spaces(spaces, cx));
    }

    /// Changes the shell once the current update is over, as the engine reports back.
    fn later(
        &self,
        cx: &mut App,
        change: impl FnOnce(&mut Shell, &mut gpui::Context<Shell>) + 'static,
    ) {
        let shell = self.shell.clone();
        cx.defer(move |cx| {
            shell.update(cx, change).ok();
        });
    }

    /// Changes `tab`'s split view, as the core reports it changed.
    fn change_split(&self, tab: u64, cx: &mut App, change: impl Fn(&mut Split) + 'static) {
        self.later(cx, move |shell, cx| {
            let mut splits = shell.splits();
            splits
                .iter_mut()
                .filter(|split| split.tabs.contains(&tab))
                .for_each(&change);
            let (pinned, today) = (shell.pinned().to_vec(), shell.today().to_vec());
            let folders = shell.folders();
            shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
        });
    }
}

impl Browser for Fake {
    fn select(&self, tab: u64, cx: &mut App) {
        self.later(cx, move |shell, cx| shell.set_selected(tab, cx));
    }

    fn close(&self, tab: u64, _split: bool, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            let mut today = shell.today().to_vec();
            let Some(index) = today.iter().position(|&id| id == tab) else {
                return;
            };
            today.remove(index);
            let next = today.get(index).or(today.last()).copied().unwrap_or(0);
            set_today(shell, today, cx);
            if shell.selected() == tab {
                shell.set_selected(next, cx);
            }
        });
    }

    fn close_below(&self, tab: u64, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            let mut today = shell.today().to_vec();
            let Some(index) = today.iter().position(|&id| id == tab) else {
                return;
            };
            today.truncate(index + 1);
            if !today.contains(&shell.selected()) && !shell.pinned().contains(&shell.selected()) {
                shell.set_selected(tab, cx);
            }
            set_today(shell, today, cx);
        });
    }

    // Under the tab, with its title and page, as the core does.
    fn duplicate(&self, tab: u64, cx: &mut App) -> u64 {
        let copy = self.next_id.get();
        self.next_id.set(copy + 1);
        self.later(cx, move |shell, cx| {
            let mut today = shell.today().to_vec();
            let at = today
                .iter()
                .position(|&id| id == tab)
                .map_or(0, |index| index + 1);
            today.insert(at, copy);
            shell.set_title(copy, shell.title(tab), cx);
            shell.set_page(copy, shell.page(tab), cx);
            set_today(shell, today, cx);
            shell.set_selected(copy, cx);
        });
        copy
    }

    // The made-up tabs stay in their Space.
    fn move_to_space(&self, _: u64, _: u64, _: &mut App) {}

    fn move_folder_to_space(&self, _: u64, _: u64, _: &mut App) {}

    fn warn_moving_profiles(&self, _: &mut App) -> bool {
        true
    }

    fn set_warn_moving_profiles(&self, _: bool, _: &mut App) {}

    // As the core does: pinned at the bottom of the pinned tabs, unpinned at the top of Today.
    fn set_pinned(&self, tab: u64, pinned: bool, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            let mut tabs = [shell.pinned().to_vec(), shell.today().to_vec()];
            let (from, to) = if pinned { (1, 0) } else { (0, 1) };
            let Some(index) = tabs[from].iter().position(|&id| id == tab) else {
                return;
            };
            tabs[from].remove(index);
            let at = if pinned { tabs[to].len() } else { 0 };
            tabs[to].insert(at, tab);
            let [pinned, today] = tabs;
            set_lists(shell, pinned, today, cx);
        });
    }

    fn move_to_favorites(&self, tab: u64, index: usize, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            let mut favorites = shell.favorites().to_vec();
            let favorite = favorites.contains(&tab);
            favorites.retain(|&id| id != tab);
            favorites.insert(index.min(favorites.len()), tab);
            // The core reports a tab in its new place before it leaves the old.
            shell.set_favorites(favorites.into_iter().map(|tab| (tab, 0)).collect(), cx);
            if !favorite {
                let (mut pinned, mut today) = (shell.pinned().to_vec(), shell.today().to_vec());
                let mut splits = shell.splits();
                pinned.retain(|&id| id != tab);
                today.retain(|&id| id != tab);
                splits
                    .iter_mut()
                    .for_each(|split| split.tabs.retain(|&id| id != tab));
                splits.retain(|split| split.tabs.len() > 1);
                let folders = shell.folders();
                shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
            }
        });
    }

    /// The preview's favorites are each one tab: a split's dragged tab goes alone.
    fn move_split_to_favorites(&self, tab: u64, index: usize, cx: &mut App) {
        self.move_to_favorites(tab, index, cx)
    }

    fn move_tab(&self, tab: u64, pinned: bool, folder: u64, index: usize, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            if pinned {
                move_pinned(shell, tab, folder, index, cx);
            } else {
                let mut tabs = [shell.pinned().to_vec(), shell.today().to_vec()];
                for list in &mut tabs {
                    list.retain(|&id| id != tab);
                }
                tabs[1].insert(index.min(tabs[1].len()), tab);
                let [pinned, today] = tabs;
                set_lists(shell, pinned, today, cx);
            }
            if shell.favorites().contains(&tab) {
                let favorites = shell.favorites().iter().copied().filter(|&id| id != tab);
                shell.set_favorites(favorites.map(|tab| (tab, 0)).collect(), cx);
            }
        });
    }

    // The preview's window has no host to take a drop.
    fn open_at(&self, _: &[String], _: bool, _: u64, _: usize, _: &mut App) -> u64 {
        0
    }

    fn open_behind(&self, _: &[String], _: bool, _: u64, _: usize, _: &mut App) {}

    fn create_folder(&self, _: u64, name: &str, folder: u64, index: usize, cx: &mut App) -> u64 {
        let id = self.next_id.get();
        self.next_id.set(id + 1);
        let name: SharedString = name.to_string().into();
        self.later(cx, move |shell, cx| {
            let (mut pinned, mut folders) = (shell.pinned().to_vec(), shell.folders());
            let at = pinned_at(&pinned, &folders, folder, index);
            pinned.insert(at, id);
            folders.folders.insert(id, (name, true));
            if folder != 0 {
                folders.parents.insert(id, folder);
            }
            let (today, splits) = (shell.today().to_vec(), shell.splits());
            shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
        });
        id
    }

    fn rename_folder(&self, folder: u64, name: &str, cx: &mut App) {
        let name: SharedString = name.to_string().into();
        self.change_folder(folder, cx, move |entry| entry.0 = name);
    }

    fn set_folder_icon(&self, folder: u64, icon: &str, cx: &mut App) {
        let icon: SharedString = icon.to_string().into();
        self.later(cx, move |shell, cx| {
            let mut folders = shell.folders();
            match icon.is_empty() {
                true => folders.icons.remove(&folder),
                false => folders.icons.insert(folder, icon),
            };
            let (pinned, today) = (shell.pinned().to_vec(), shell.today().to_vec());
            let splits = shell.splits();
            shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
        });
    }

    // As the core does: the copy under the folder, with a copy of each row in it.
    fn duplicate_folder(&self, folder: u64, cx: &mut App) -> u64 {
        let copy = self.next_id.get();
        let next = self.next_id.clone();
        next.set(copy + 1);
        self.later(cx, move |shell, cx| {
            let (mut pinned, mut folders) = (shell.pinned().to_vec(), shell.folders());
            let Some(at) = pinned.iter().position(|&id| id == folder) else {
                return;
            };
            let end = end_of(&pinned, &folders, folder);
            let mut copies = HashMap::from([(folder, copy)]);
            let mut made = vec![copy];
            for &row in &pinned[at + 1..end] {
                let id = next.get();
                next.set(id + 1);
                copies.insert(row, id);
                made.push(id);
            }
            for (&from, &to) in &copies {
                if let Some(parent) = folders.parents.get(&from).copied() {
                    let parent = copies.get(&parent).copied().unwrap_or(parent);
                    folders.parents.insert(to, parent);
                }
                if let Some(entry) = folders.folders.get(&from).cloned() {
                    folders.folders.insert(to, entry);
                }
                if let Some(icon) = folders.icons.get(&from).cloned() {
                    folders.icons.insert(to, icon);
                }
                if !folders.folders.contains_key(&from) {
                    shell.set_page(to, shell.page(from), cx);
                }
            }
            pinned.splice(end..end, made);
            let (today, splits) = (shell.today().to_vec(), shell.splits());
            shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
        });
        copy
    }

    fn set_folder_open(&self, folder: u64, open: bool, cx: &mut App) {
        self.change_folder(folder, cx, move |entry| entry.1 = open);
    }

    // The preview keeps the rows of the Space shown alone.
    fn set_pinned_open(&self, space: u64, open: bool, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            if space != shell.space() {
                return;
            }
            let folders = Folders {
                pinned_shut: !open,
                ..shell.folders()
            };
            let (pinned, today) = (shell.pinned().to_vec(), shell.today().to_vec());
            let splits = shell.splits();
            shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
        });
    }

    fn move_folder(&self, folder: u64, to: u64, index: usize, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            move_pinned(shell, folder, to, index, cx)
        });
    }

    fn delete_folder(&self, folder: u64, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            let (mut pinned, mut folders) = (shell.pinned().to_vec(), shell.folders());
            pinned.retain(|&id| id != folder);
            folders.folders.remove(&folder);
            let holder = folders.parents.remove(&folder).unwrap_or(0);
            for parent in folders
                .parents
                .values_mut()
                .filter(|parent| **parent == folder)
            {
                *parent = holder;
            }
            folders.parents.retain(|_, parent| *parent != 0);
            let (today, splits) = (shell.today().to_vec(), shell.splits());
            shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
        });
    }

    // As the core does, without the undo or split views.
    fn clear(&self, cx: &mut App) -> u64 {
        self.later(cx, |shell, cx| {
            let selected = shell.selected();
            let all = shell.today().to_vec();
            let today = if all == [selected] { Vec::new() } else { all };
            let today = today.into_iter().filter(|&tab| tab == selected).collect();
            set_today(shell, today, cx);
        });
        0
    }

    fn undo_clear(&self, _: u64, _: &mut App) -> usize {
        0
    }

    fn undo_delete_space(&self, _: u64, _: &mut App) -> bool {
        false
    }

    fn reopen_closed_tab(&self, _: &mut App) {}

    fn open_preview(
        &self,
        _: u64,
        _: SharedString,
        _: Point<Pixels>,
        _: (Bounds<Pixels>, f32),
        _: &mut App,
    ) {
    }

    fn open_archived_preview(
        &self,
        over: u64,
        archived: u64,
        url: SharedString,
        link: Point<Pixels>,
        page: (Bounds<Pixels>, f32),
        cx: &mut App,
    ) {
        // As PREVIEW_PEEK's, once the shell's update is done, with the shell's page showing
        // through the card.
        cx.defer(move |cx| {
            let bounds = Bounds::new(PREVIEW_ORIGIN.get(), PREVIEW_AREA.get());
            link_preview::open_at(Rc::new(PreviewPeek), bounds, true, cx);
            link_preview::show(1, 99, over, url, page, link, Some(archived), cx);
        });
    }

    fn link_previews(&self) -> bool {
        true
    }

    fn close_preview(&self, _: u64, _: &mut App) -> bool {
        false
    }

    fn reopen_tab(&self, _: u64, _: &mut App) -> u64 {
        0
    }

    fn pinned_url(&self, _: u64) -> String {
        String::new()
    }

    fn set_pinned_url(&self, _: u64, _: &str, _: &mut App) {}

    fn typed_url(&self, _: &str) -> Option<String> {
        None
    }

    fn tab_name(&self, _: u64) -> String {
        String::new()
    }

    fn undo_archive_remove(&self, _: u64, _: &mut App) -> usize {
        0
    }

    fn undo_remove_suggestion(&self, _: u64, _: &mut App) -> bool {
        false
    }

    fn redo_remove_suggestion(&self, _: u64, _: &mut App) -> bool {
        false
    }

    // Takes it out of the archive, without opening it.
    fn restore_archived(&self, archived: u64, cx: &mut App) {
        self.archive.borrow_mut().retain(|&(id, ..)| id != archived);
        self.report_archive(cx);
    }

    // The preview has nothing to undo, so no removal has an id.
    fn remove_archived(&self, archived: u64, cx: &mut App) -> u64 {
        self.archive.borrow_mut().retain(|&(id, ..)| id != archived);
        self.report_archive(cx);
        0
    }

    fn clear_archive(&self, cx: &mut App) -> u64 {
        self.archive.borrow_mut().clear();
        self.report_archive(cx);
        0
    }

    // Opening and showing a file do nothing here.
    fn act_on_download(&self, download: u64, action: DownloadAction, cx: &mut App) {
        let mut downloads = self.downloads.borrow_mut();
        let Some(index) = downloads.iter().position(|each| each.id == download) else {
            return;
        };
        let download = &mut downloads[index];
        match action {
            DownloadAction::Open if download.state != DownloadState::Done => {
                download.open_when_done = !download.open_when_done
            }
            DownloadAction::Pause => download.state = DownloadState::Paused,
            DownloadAction::Resume => download.state = DownloadState::Running,
            DownloadAction::Cancel => download.state = DownloadState::Cancelled,
            DownloadAction::Keep => download.held = false,
            DownloadAction::Remove => _ = downloads.remove(index),
            DownloadAction::Open | DownloadAction::Show => {}
        }
        drop(downloads);
        self.report_downloads(cx);
    }

    fn clear_downloads(&self, private: Option<u64>, cx: &mut App) {
        self.downloads.borrow_mut().retain(|each| {
            matches!(each.state, DownloadState::Running | DownloadState::Paused)
                || each.private != private.is_some()
                || private.is_some_and(|profile| profile != each.profile)
        });
        self.report_downloads(cx);
    }

    // The made-up files have the file glyph.
    fn download_icon(&self, _: u64, _: &mut App) {}

    // And no previews.
    fn file_preview(&self, _: u64, _: &str, _: &mut App) {}

    // Nor videos.
    fn open_video(&self, _: u64, _: &str, _: u32, _: &mut App) -> bool {
        false
    }

    fn video_frame(&self, _: u64, _: &mut App) -> Option<Arc<gpui::RenderImage>> {
        None
    }

    fn close_video(&self, _: u64, _: &mut App) {}

    // Nor captures.
    fn list_captures(&self, _: &mut App) {}

    fn hover_kinds(&self, _: &mut App) -> u32 {
        browser::shell::HOVER_ALL
    }

    fn set_hover_kinds(&self, _: u32, _: &mut App) {}

    // The made-up downloads have no files, so each is gone already and none drags.
    fn trash(&self, _: &str, _: &mut App) -> bool {
        true
    }

    fn drag_file(&self, _: &str, _: &mut App) -> bool {
        false
    }

    // The made-up pages have the globe.
    fn page_favicon(&self, _: &str, _: &mut App) {}

    // In UTC, with weeks from Monday.
    // Library › Agents' made-up past tasks: two today, one yesterday, one last week.
    fn agent_history(&self, _: u64) -> String {
        let at = (now() / 1000.) as u64;
        let past =
            |id: &str, title: &str, agent: &str, ago: u64, pages: usize, orders, messages| {
                browser::agent_history::Past {
                    id: id.into(),
                    title: title.into(),
                    agent: agent.into(),
                    started: at - ago - 300,
                    ended: at - ago,
                    pages: (0..pages)
                        .map(|n| (format!("Page {n}"), format!("https://example.com/{n}")))
                        .collect(),
                    orders,
                    messages,
                    // A step a minute, on its pages, and one refused.
                    steps: (0..pages)
                        .flat_map(|n| {
                            let at = at - ago - 300 + 60 * n as u64;
                            let step =
                                |at, action: &str, refused: &str| browser::agent_history::Step {
                                    at,
                                    action: action.into(),
                                    site: format!("example{n}.com"),
                                    refused: refused.into(),
                                    ..Default::default()
                                };
                            [
                                step(at, "open_tab", ""),
                                step(at + 20, "read_page", ""),
                                step(at + 40, "click", ""),
                            ]
                        })
                        .chain([browser::agent_history::Step {
                            at: at - ago - 10,
                            action: "passwords".into(),
                            site: String::new(),
                            refused: "Agents never read passwords, cookies or autofill data."
                                .into(),
                            ..Default::default()
                        }])
                        .collect(),
                    answers: "claude-code".into(),
                }
            };
        let day = 24 * 60 * 60;
        browser::agent_history::write(&[
            past("task-4", "Book a table for four", "Claude", 1800, 3, 1, 0),
            past("session-3", "Shipping form", "Claude Code", 7200, 2, 0, 0),
            past(
                "task-2",
                "Reply to the landlord",
                "Codex",
                day + 3600,
                1,
                0,
                1,
            ),
            past(
                "session-1",
                "Compare the museum passes",
                "Claude Code",
                6 * day,
                5,
                0,
                0,
            ),
        ])
    }

    // The made-up agent may always act on the first page's site and never on the second's.
    fn agent_answers(&self, key: &str, _: &App) -> Vec<(String, bool)> {
        match key {
            "claude-code" => vec![
                ("example0.com".into(), true),
                ("example1.com".into(), false),
            ],
            _ => Vec::new(),
        }
    }

    fn day_starts(&self) -> [f64; 4] {
        let today = now() - now() % DAY;
        // 1 January 1970 was a Thursday.
        let week = today - ((today / DAY + 3.) % 7.) * DAY;
        [today, today - DAY, week, week - 7. * DAY]
    }

    // The made-up pages have no history to go through.
    fn back_to_pinned_url(&self, _: u64, _: &mut App) {}

    fn set_muted(&self, tab: u64, muted: bool, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            let page = Page {
                muted,
                ..shell.page(tab)
            };
            shell.set_page(tab, page, cx);
        });
    }

    // The made-up pages share nothing.
    fn share_instead(&self, _: u64, _: &mut App) {}

    fn set_volume(&self, tab: u64, volume: f32, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            let page = Page {
                volume: Volume(volume),
                ..shell.page(tab)
            };
            shell.set_page(tab, page, cx);
        });
    }

    fn media(&self, tab: u64, action: MediaAction, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            let media_playing = match action {
                MediaAction::Play => true,
                MediaAction::Pause => false,
                MediaAction::Previous | MediaAction::Next => shell.page(tab).media_playing,
                MediaAction::PictureInPicture => shell.page(tab).media_playing,
            };
            // A made-up page floats nothing, but its player's button goes as a real one's would.
            let media_picture_in_picture = (shell.page(tab).media_picture_in_picture)
                && action != MediaAction::PictureInPicture;
            let page = Page {
                media_playing,
                media_picture_in_picture,
                ..shell.page(tab)
            };
            shell.set_page(tab, page, cx);
        });
    }

    fn cast(&self, tab: u64, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            let device = shell::CastDevice {
                id: "cast:preview".into(),
                name: "Living Room TV".into(),
                state: shell::CastState::Available,
                status: "".into(),
            };
            let cast = shell::Cast {
                header: "Cast tab".into(),
                devices: vec![device],
            };
            shell.set_cast(tab, Some(cast), cx);
        });
    }

    fn cast_close(&self, _: &mut App) {}

    fn choose_device(&self, tab: u64, _: SharedString, cx: &mut App) {
        self.later(cx, move |shell, cx| shell.set_chooser(tab, None, cx));
    }

    fn cancel_chooser(&self, tab: u64, cx: &mut App) {
        self.later(cx, move |shell, cx| shell.set_chooser(tab, None, cx));
    }

    fn scan_devices(&self, tab: u64, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            shell.set_chooser(tab, preview_chooser("bluetooth"), cx)
        });
    }

    fn device_settings(&self, _: u64, _: &mut App) {}

    fn pair(&self, tab: u64, _: Option<SharedString>, cx: &mut App) {
        self.later(cx, move |shell, cx| shell.set_pair(tab, None, cx));
    }

    fn cancel_pair(&self, tab: u64, cx: &mut App) {
        self.later(cx, move |shell, cx| shell.set_pair(tab, None, cx));
    }

    // Casting starting closes the dialog, as the core's does.
    fn cast_to(&self, tab: u64, _: SharedString, cx: &mut App) {
        self.later(cx, move |shell, cx| shell.set_cast(tab, None, cx));
    }

    // Safari's engine is the Mac's alone, so elsewhere the note offers what each system's does.
    fn can_use_system_engine(&self) -> bool {
        cfg!(target_os = "macos")
    }

    // PREVIEW_DRM=default draws the note as where ilo is the system's default browser.
    fn elsewhere(&self) -> Option<browser::shell::Elsewhere> {
        let default = || std::env::var("PREVIEW_DRM").is_ok_and(|drm| drm == "default");
        browser::shell::Elsewhere::on(self.os(), default)
    }

    // PREVIEW_SAFE_START=1 shows a safe start's card at the sidebar's foot, with Settings' ad
    // blocking on, or =unblocked with it off.
    fn safe_start(&self) -> bool {
        std::env::var_os("PREVIEW_SAFE_START").is_some()
    }

    fn ad_blocking(&self) -> bool {
        std::env::var("PREVIEW_SAFE_START").map_or(true, |state| state != "unblocked")
    }

    fn use_system_engine(&self, tab: u64, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            shell.set_media_unsupported(tab, "", cx);
            let page = Page {
                system_engine: true,
                ..shell.page(tab)
            };
            shell.set_page(tab, page, cx);
        });
    }

    fn set_icon(&self, tab: u64, icon: &str, cx: &mut App) {
        let icon: SharedString = icon.to_string().into();
        self.later(cx, move |shell, cx| {
            let page = Page {
                icon,
                ..shell.page(tab)
            };
            shell.set_page(tab, page, cx);
        });
    }

    fn go_back(&self, _: u64, _: &mut App) {}

    fn go_forward(&self, _: u64, _: &mut App) {}

    fn tab_history(&self, _: u64, _: bool, _: &mut App) {}

    fn go_to_history(&self, _: u64, _: i32, _: &mut App) {}

    fn open_history(&self, _: u64, _: i32, _: &mut App) {}

    fn reload(&self, _: u64, _: bool, _: &mut App) {}

    fn clear_site(&self, _: u64, _: bool, _: &mut App) {}

    fn clear_site_data(&self, _: u64, _: &mut App) {}

    fn set_developer_mode(&self, _: u64, _: bool, _: &mut App) {}

    // The made-up pages ask nothing.
    fn answer_permission(&self, _: u64, _: shell::Answer, _: &mut App) {}

    // The preview has no agents.
    fn answer_agent(&self, _: u64, _: browser::agents::Answer, _: &mut App) {}

    // The made-up pages ask no dialogs.
    fn show_page_dialog(&self, _: Option<shell::ShownPageDialog>, _: &mut App) {}
    fn answer_page_dialog(&self, _: u64, _: u64, _: shell::PageReply, _: &mut App) {}

    fn stop(&self, _: u64, _: &mut App) {}

    fn zoom(&self, _: u64, _: i32, _: &mut App) {}

    // The made-up pages have no source and no DevTools.
    fn view_source(&self, _: u64, _: &mut App) {}
    fn save_page(&self, _: u64, _: &mut App) {}

    fn save_markdown(&self, _: &str, _: String, _: usize, _: &mut App) {}
    fn close_page_menu(&self, _: u64, _: Option<i32>, _: &mut App) {}
    fn print(&self, _: u64, _: &mut App) {}

    fn devtools(&self, _: u64, _: i32, _: &mut App) {}

    fn task_manager(&self, _: &mut App) {}

    // The preview has no engine to trace, nor the toast's window.
    fn start_trace(&self) -> bool {
        false
    }

    fn stop_trace(&self) {}

    fn trace_note(&self, _: Option<Rc<dyn Fn(&mut App)>>, _: &mut App) {}

    // The preview has no menu bar and no extensions.
    fn set_menus(&self, _: &MenuBar, _: &mut App) {}

    fn set_window_title(&self, _: &str, _: &mut App) {}

    fn run_extension(&self, _: &str, _: Bounds<Pixels>, _: &mut App) -> bool {
        false
    }

    fn run_extension_command(&self, _: &str, _: &str, _: &mut App) -> bool {
        false
    }

    fn set_extension_pinned(&self, _: u64, _: &str, _: bool, _: &mut App) {}

    fn remove_extension(&self, _: u64, _: &str, _: &mut App) -> bool {
        false
    }

    fn turn_on_extension(&self, _: u64, _: &str, _: &mut App) -> bool {
        false
    }

    fn extension_crash_told(&self, _: u64, _: &str, _: &mut App) {}

    fn open_extension_options(&self, _: &str, _: &mut App) {}

    fn manage_extensions(&self, _: Option<&str>, _: &mut App) {}

    fn add_extension(&self, _: &mut App) {}

    fn lay_out_side_panel(&self, _: Bounds<Pixels>, _: &mut App) {}

    fn close_side_panel(&self, _: &mut App) {}

    fn has_side_panel(&self, _: &str) -> bool {
        false
    }

    fn toggle_side_panel(&self, _: &str, _: &mut App) {}

    fn set_side_panel_width(&self, _: f32, _: &mut App) {}

    fn view_history(&self, _: &mut App) {}
    fn open_page(&self, _: &str, _: &mut App) {}
    fn new_note(&self, _: u64, _: Option<u64>, _: &mut App) {}
    fn site_search_settings(&self, _: &mut App) {}

    fn clear_browsing_data(&self, _: &mut App) {}

    fn close_other_window(&self, _: &mut App) -> bool {
        false
    }

    fn command_bar(
        &self,
        editing: Option<(u64, SharedString)>,
        shown: Vec<SharedString>,
        tabs: Vec<BarTab>,
        field: Option<Bounds<Pixels>>,
        cx: &mut App,
    ) {
        // On the web the bar draws in the one window, over the shell (web::Root), which is mid-update
        // here, so the bar shows once that's over.
        if cfg!(target_family = "wasm") {
            return cx.defer(move |cx| command_bar::show(editing, shown, tabs, field, cx));
        }
        // No command bar here: a new tab opens at once.
        if editing.is_some() {
            return;
        }
        let tab = self.next_id.get();
        self.next_id.set(tab + 1);
        self.later(cx, move |shell, cx| {
            let mut today = shell.today().to_vec();
            today.insert(0, tab);
            set_today(shell, today, cx);
            shell.set_selected(tab, cx);
        });
    }

    // No command bar here, as for a new tab.
    // No Little ilo window here.
    fn command_bar_in(
        &self,
        _: Option<(u64, SharedString)>,
        _: Vec<SharedString>,
        _: Vec<BarTab>,
        _: Bounds<Pixels>,
        _: &mut App,
    ) {
    }

    fn spaces_menu(&self, _: Vec<shell::BarSpace>, _: Bounds<Pixels>, _: &mut App) {}

    fn search_tabs(&self, _: Vec<BarTab>, _: &mut App) {}

    // The made-up pages have nothing to find.
    fn find(&self, _: u64, _: &mut App) {}

    // PREVIEW_SITE_CONTROLS=1 opens them over a made-up site, in a window where the panel goes, as
    // the preview's host can't move one.
    fn site_controls(
        &self,
        tab: u64,
        place: site_controls::Place,
        window: Size<Pixels>,
        extensions: Vec<site_controls::Extension>,
        profiles: Option<site_controls::Profiles>,
        cx: &mut App,
    ) {
        if std::env::var_os("PREVIEW_SITE_CONTROLS").is_none() {
            return;
        }
        let (at, _) = site_controls::placed(place, window);
        let room = size(px(site_controls::WIDTH), window.height - at.y);
        let bounds = Bounds::new(PREVIEW_ORIGIN.get() + at, room);
        if site_controls::open_at(Rc::new(PreviewSiteControls), bounds, true, cx).is_some() {
            site_controls::show(tab, place, window, extensions, profiles, cx);
            site_controls::changed(tab, preview_site(), cx);
        }
    }

    fn site_controls_extensions(
        &self,
        _: u64,
        _: Vec<browser::site_controls::Extension>,
        _: &mut App,
    ) {
    }

    fn capture(
        &self,
        _: u64,
        _: Bounds<Pixels>,
        _: f32,
        _: bool,
        _: Option<Point<Pixels>>,
        _: bool,
        _: menu::Look,
        _: &mut App,
    ) {
    }

    fn capture_portrait(
        &self,
        _: u64,
        _: Bounds<Pixels>,
        _: f32,
        _: browser::capture::portrait::Background,
        _: &mut App,
    ) {
    }

    fn page_picture(&self, _: u64, _: u64, _: &mut App) {}

    fn find_next(&self, _: u64, _: bool, _: &mut App) {}

    fn jump_to_selection(&self, _: u64, _: &mut App) {}

    // The preview has no Settings window.
    fn settings(&self, _: &mut App) {}

    fn passwords(&self, _: u64, _: &mut App) {}

    fn show_autofill(&self, _: u64, _: bool) -> bool {
        false
    }

    fn take_over(&self, _: SharedString, _: u64, _: &mut App) {}

    fn hand_back(&self, _: u64, _: &mut App) {}

    fn stop_agent(&self, _: SharedString, _: &mut App) {}

    fn task(&self, _: SharedString, _: browser::agents::task::TaskAction, _: &mut App) {}

    // Each card's click shows the working task's conversation, as the Agents send it.
    fn show_task(&self, agent: SharedString, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            shell.set_conversation(Some(task_conversation(agent)), cx)
        });
    }

    // A task's Space is known by its agent's id, as the core's are.
    fn space_agent(&self, space: u64) -> SharedString {
        match (FIRST_TASK..FIRST_TASK + TASKS.len() as u64).contains(&space) {
            true => format!("task-{space}").into(),
            false => SharedString::default(),
        }
    }

    // English, for PREVIEW_TRANSLATE's pages to offer translating into.
    fn translation_target(&self) -> Option<SharedString> {
        std::env::var_os("PREVIEW_TRANSLATE").map(|_| "en".into())
    }

    fn task_picture(&self, _: u64, _: u64, _: &mut App) {}

    fn shortcuts(&self, _: &mut App) {}

    fn about(&self, _: &mut App) {}

    fn import(&self, _: &mut App) {}

    fn show_space(&self, space: u64, cx: &mut App) {
        let named = self.opens_on.borrow().get(&space).copied();
        let shows = self.shows.clone();
        self.later(cx, move |shell, cx| {
            shows.borrow_mut().insert(shell.space(), shell.selected());
            shell.set_space(space, cx);
            let held = |tab: &u64| shell.today().contains(tab) || shell.pinned().contains(tab);
            let last = shows.borrow().get(&space).copied().filter(held);
            let tab = last.or(named).unwrap_or_else(|| opening_tab(shell));
            shows.borrow_mut().insert(space, tab);
            shell.set_selected(tab, cx);
        });
    }

    fn space_tab(&self, space: u64) -> u64 {
        let last = self.shows.borrow().get(&space).copied();
        last.or_else(|| self.opens_on.borrow().get(&space).copied())
            .unwrap_or(0)
    }

    fn create_space(&self, name: &str, profile: u64, after: u64, cx: &mut App) -> u64 {
        let space = self.next_id.get();
        self.next_id.set(space + 1);
        let entry = (
            space,
            name.to_string().into(),
            None,
            profile,
            "".into(),
            "".into(),
            "".into(),
        );
        let mut spaces = self.spaces.borrow_mut();
        let at = spaces
            .iter()
            .position(|(id, ..)| *id == after)
            .map_or(spaces.len(), |at| at + 1);
        spaces.insert(at, entry);
        drop(spaces);
        self.report_spaces(cx);
        space
    }

    fn rename_space(&self, space: u64, name: &str, cx: &mut App) {
        if let Some(entry) = self
            .spaces
            .borrow_mut()
            .iter_mut()
            .find(|(id, ..)| *id == space)
        {
            entry.1 = name.to_string().into();
        }
        self.report_spaces(cx);
    }

    // The preview's tabs have no page title to go back to, so an empty name leaves the title.
    fn rename_tab(&self, tab: u64, name: &str, cx: &mut App) {
        let name: SharedString = name.to_string().into();
        if !name.is_empty() {
            self.later(cx, move |shell, cx| shell.set_title(tab, name, cx));
        }
    }

    fn restore_sidebar(&self, _: u64, _: &mut App) {}

    fn delete_space(&self, space: u64, cx: &mut App) -> bool {
        self.spaces.borrow_mut().retain(|(id, ..)| *id != space);
        self.report_spaces(cx);
        true
    }

    fn move_space(&self, space: u64, after: u64, cx: &mut App) {
        let mut spaces = self.spaces.borrow_mut();
        let Some(from) = spaces.iter().position(|(id, ..)| *id == space) else {
            return;
        };
        let entry = spaces.remove(from);
        let to = match after {
            0 => 0,
            after => match spaces.iter().position(|(id, ..)| *id == after) {
                Some(at) => at + 1,
                None => from,
            },
        };
        spaces.insert(to, entry);
        drop(spaces);
        self.report_spaces(cx);
    }

    fn set_space_color(&self, space: u64, color: Option<u32>, cx: &mut App) {
        if let Some(entry) = self
            .spaces
            .borrow_mut()
            .iter_mut()
            .find(|(id, ..)| *id == space)
        {
            entry.2 = color;
        }
        self.report_spaces(cx);
    }

    fn set_space_icon(&self, space: u64, icon: &str, cx: &mut App) {
        if let Some(entry) = self
            .spaces
            .borrow_mut()
            .iter_mut()
            .find(|(id, ..)| *id == space)
        {
            entry.4 = icon.to_string().into();
        }
        self.report_spaces(cx);
    }

    fn set_space_theme(&self, space: u64, theme: &str, cx: &mut App) {
        if let Some(entry) = self
            .spaces
            .borrow_mut()
            .iter_mut()
            .find(|(id, ..)| *id == space)
        {
            entry.6 = theme.to_string().into();
        }
        self.report_spaces(cx);
    }

    fn set_space_profile(&self, space: u64, profile: u64, cx: &mut App) {
        if let Some(entry) = self
            .spaces
            .borrow_mut()
            .iter_mut()
            .find(|(id, ..)| *id == space)
        {
            entry.3 = profile;
        }
        self.report_spaces(cx);
    }

    // The preview has the one profile.
    fn create_profile(&self, _: &str, _: &mut App) -> u64 {
        0
    }

    fn rename_profile(&self, _: u64, _: &str, _: &mut App) {}

    // The tests' made-up Arc, so the import's step shows a browser to pick.
    fn import_host(&self) -> Rc<dyn browser::import::Host> {
        let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/src/import/fixture");
        let roots = Roots::under(std::path::Path::new(fixture), Os::Mac);
        Rc::new(browser::import::Detached(roots))
    }

    fn delete_profile(&self, _: u64, _: &mut App) {}

    fn favorites_scope(&self, _: &mut App) -> shell::FavoritesScope {
        shell::FavoritesScope::default()
    }

    // The preview's window is GPUI's own, which has the keyboard already.
    fn focus_ui(&self, _: &mut App) {}

    fn focus_page(&self, _: &mut App) {}

    // There's no page, and the window's buttons stay where GPUI put them.
    // The site lays its frame out under the canvas there.
    fn lay_out_page(&self, _frame: Bounds<Pixels>, _radius: f32, _: &mut App) {
        #[cfg(target_family = "wasm")]
        web::lay_out(_frame, _radius);
    }

    fn set_focus_color(&self, _: u32, _: &mut App) {}

    fn set_page_color(&self, _: u32, _: &mut App) {}

    // The preview has no material: its window is the tint over GPUI's clear.
    fn set_window_light(&self, _: bool, _: &mut App) {}

    // The preview keeps nothing for a next launch.
    fn set_appearance(&self, _: Appearance, _: &mut App) {}
    fn keep_lone_header(&self, _: bool, _: &mut App) {}

    fn page_moving(&self, _: bool, _: &mut App) {}

    // Its pages are the shell's own drawing, which the cards scale.
    fn set_page_scale(&self, _: f32, _: &mut App) {}

    fn set_pane_corner(&self, _: f32, _: f32, _: f32, _: &mut App) {}

    // No command bar here but the web's, as for a new tab, and no split views but the made-up one.
    fn command_bar_open(&self, cx: &App) -> bool {
        cfg!(target_family = "wasm") && command_bar::is_open(cx)
    }

    fn command_bar_follows(&self, _: Vec<BarTab>, _: Vec<BarTab>, _: &mut App) -> bool {
        false
    }

    fn split_bar(&self, _: u64, _: Bounds<Pixels>, _: Bounds<Pixels>, _: &mut App) {}

    fn separate(&self, _: u64, _: &mut App) {}

    fn move_in_split(&self, _: u64, _: usize, _: &mut App) {}

    fn set_split_stacked(&self, tab: u64, stacked: bool, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            let mut splits = shell.splits();
            for split in splits.iter_mut().filter(|split| split.tabs.contains(&tab)) {
                split.stacked = stacked;
            }
            let (pinned, today) = (shell.pinned().to_vec(), shell.today().to_vec());
            let folders = shell.folders();
            shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
        });
    }

    fn set_split_name(&self, tab: u64, name: &str, cx: &mut App) {
        let name: SharedString = name.to_string().into();
        self.change_split(tab, cx, move |split| split.name = name.clone());
    }

    fn set_split_icon(&self, tab: u64, icon: &str, cx: &mut App) {
        let icon: SharedString = icon.to_string().into();
        self.change_split(tab, cx, move |split| split.icon = icon.clone());
    }

    fn set_split_shares(&self, tab: u64, shares: Option<&[f64]>, cx: &mut App) {
        let shares = shares.map_or_else(Vec::new, <[f64]>::to_vec);
        self.later(cx, move |shell, cx| {
            let mut splits = shell.splits();
            for split in splits.iter_mut().filter(|split| split.tabs.contains(&tab)) {
                split.shares = shares.clone();
                split.expanded = 0;
            }
            let (pinned, today) = (shell.pinned().to_vec(), shell.today().to_vec());
            let folders = shell.folders();
            shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
        });
    }

    fn set_split_expanded(&self, tab: u64, expanded: bool, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            let mut splits = shell.splits();
            for split in splits.iter_mut().filter(|split| split.tabs.contains(&tab)) {
                split.expanded = if expanded { tab } else { 0 };
            }
            let (pinned, today) = (shell.pinned().to_vec(), shell.today().to_vec());
            let folders = shell.folders();
            shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
        });
    }

    fn split_with(&self, _: u64, _: u64, _: bool, _: &mut App) {}
    fn open_split_at(&self, _: &str, _: u64, _: bool, _: bool, _: &mut App) {}

    // The targets' window is the app's, over the page, which the preview has neither of.
    fn show_drop_targets(&self, _: bool, _: &mut App) {}

    // The preview has no page for the pointer to be over.
    fn pointer(&self, _: &mut App) -> Option<Point<Pixels>> {
        None
    }

    fn over_page_blends(&self) -> bool {
        true
    }

    fn show_window_buttons(&self, _: bool, _: &mut App) {}

    fn little_drawn(&self, _: &mut App) {}

    fn press_window_button(&self, _: WindowButton, _: &mut App) {}

    fn set_on_top(&self, _: bool, _: &mut App) {}

    // The preview's window is GPUI's own, which GPUI moves.
    fn start_window_move(&self, _: &mut App) {}

    fn window_maximized(&self) -> bool {
        false
    }

    fn window_full_screen(&self) -> bool {
        false
    }

    // The reveal's window is the app's, over the page, which the preview has neither of. On the web
    // the panel draws in the one canvas, over the site's frame, which the site hears.
    fn show_reveal(&self, _: f32, _: &mut App) {
        #[cfg(target_family = "wasm")]
        web::cover(web::Over::Reveal, true);
    }

    fn hide_reveal(&self, _: &mut App) {
        #[cfg(target_family = "wasm")]
        web::cover(web::Over::Reveal, false);
    }

    fn option_reveals_sidebar(&self) -> bool {
        true
    }

    // PREVIEW_SIDEBAR=<width> starts the sidebar at that width, such as its narrowest, 164.
    fn saved_sidebar_width(&self) -> Option<f32> {
        std::env::var("PREVIEW_SIDEBAR").ok()?.parse().ok()
    }

    fn set_sidebar_width(&self, _: f32, _: &mut App) {}

    fn detent(&self, _: &mut App) {}

    // The preview doesn't hear ⌃ go up, so each ⌃Tab there ends its walk.
    fn watch_control(&self, cx: &mut App) {
        self.later(cx, |shell, cx| shell.control_released(cx));
    }

    // The preview leaves the clipboard alone, and shares nothing.
    fn copy(&self, _: &str, _: &mut App) {}

    fn pasted(&self, _: &App) -> Option<String> {
        None
    }

    fn share_services(&self, _: &str) -> Vec<SharedString> {
        Vec::new()
    }

    fn share(&self, _: &str, _: &str, _: usize, _: &mut App) {}

    // The toast's window is one the app's host lays over the browser window.
    fn toast(&self, _: &str, _: Option<(&str, &str)>, _: Duration, _: &mut App) {}

    fn toast_undo(&self, _: &str, _: Rc<dyn Fn(&mut App)>, _: Duration, _: &mut App) {}

    fn hide_undo_toast(&self, _: &mut App) {}

    fn in_front(&self) -> bool {
        true
    }

    fn toast_button(
        &self,
        _: &str,
        _: (&'static str, &'static str),
        _: Rc<dyn Fn(&mut App)>,
        _: Duration,
        _: &mut App,
    ) {
    }

    fn set_edit_titles(&self, _: Option<&str>, _: Option<&str>, _: &mut App) {}

    fn full_screen_note(&self, _: bool, _: &mut App) {}

    fn enter_full_screen(&self, _: &mut App) -> bool {
        false
    }

    // The toast is a window over the page, which the preview has neither of.
    fn hang_note(&self, _: u64, _: bool, _: &mut App) {}

    // The tooltip's window is the app's, over the page, which the preview has neither of.
    fn tooltip(&self, _: Option<(browser::tooltip::Tip, browser::tooltip::Anchor)>, _: &mut App) {}

    // So is the hover card's, and there's no engine to picture a page.
    fn hover_card(&self, _: browser::hover_card::Hover, _: &mut App) {}

    // And the folder preview's.
    fn folder_preview(&self, _: browser::folder_preview::Ask, _: &mut App) {}

    fn card_picture(&self, _: u64, _: f32, _: &mut App) {}

    // The same for the link target's, which the preview's made-up pages have no links for.
    fn link_target(
        &self,
        _: Option<(
            browser::link_target::Plate,
            gpui::Bounds<gpui::Pixels>,
            browser::link_target::Side,
        )>,
        _: &mut App,
    ) {
    }

    fn context_menu(
        &self,
        items: Vec<MenuEntry>,
        at: Point<Pixels>,
        _: bool,
        look: menu::Look,
        chosen: Box<dyn FnOnce(Option<usize>, &mut App)>,
        cx: &mut App,
    ) {
        // PREVIEW_MENU_LOOK=light shows it as a light theme's Space does.
        let light = std::env::var("PREVIEW_MENU_LOOK").is_ok_and(|look| look == "light");
        let look = menu::Look {
            light: light || look.light,
            ..look
        };
        let area = Bounds::new(point(px(0.), px(0.)), PREVIEW_AREA.get());
        let chosen = menu::picked(chosen);
        menu::show(
            menu::Request {
                items,
                at,
                area,
                look,
                chosen,
            },
            cx,
        );
    }

    fn tell(
        &self,
        _: dialog::Question,
        _: Box<dyn FnOnce(Vec<String>, &mut App)>,
        _: &mut App,
    ) -> bool {
        false
    }

    // The dialog's window is one the app's host lays over the browser window. With PREVIEW_DIALOG=1
    // it opens over the preview's the first time it asks, on top of it, such as when the Space
    // menu's Delete Space, through PREVIEW_MENU, asks.
    fn confirm(
        &self,
        message: String,
        detail: String,
        action: &'static str,
        confirmed: Box<dyn FnOnce(&mut App)>,
        cx: &mut App,
    ) {
        if std::env::var_os("PREVIEW_DIALOG").is_none() {
            return;
        }
        if !DIALOG_OPEN.replace(true) {
            let bounds = Bounds::new(PREVIEW_ORIGIN.get(), PREVIEW_AREA.get());
            dialog::open_at(Rc::new(PreviewDialog), bounds, true, cx);
        }
        dialog::ask(None, message, detail, action, confirmed, cx);
    }

    fn quit(&self, cx: &mut App) {
        cx.quit();
    }

    // The preview has one window.
    fn new_window(&self, _: &mut App) {}

    // So the onboarding shows its step for it.
    fn is_default_browser(&self) -> bool {
        false
    }

    fn crash_reports(&self, _: &App) -> Option<bool> {
        Some(self.opt_ins.get().0)
    }

    fn set_crash_reports(&self, on: bool, _: &mut App) {
        self.opt_ins.set((on, self.opt_ins.get().1));
    }

    fn agents(&self, _: &App) -> Option<bool> {
        Some(self.opt_ins.get().1)
    }

    fn set_agents(&self, on: bool, _: &mut App) {
        self.opt_ins.set((self.opt_ins.get().0, on));
    }

    fn new_private_window(&self, _: &mut App) {}

    fn new_blank_window(&self, _: &mut App) {}

    fn move_out_of_blank(&self, _: u64, _: u64, _: &mut App) {}

    fn new_little_window(&self, _: u64, _: &mut App) {}

    fn open_tab_in_little(&self, _: u64, _: &mut App) {}

    fn open_in_main_window(&self, _: u64, _: u64, _: &mut App) {}

    fn little_profile(&self, _: u64, _: &str, _: &mut App) {}

    fn close_window(&self, cx: &mut App) {
        cx.quit();
    }
}

/// The command bar's requests, for the preview: it goes nowhere, but a search goes to a search.
struct PreviewBar;

impl command_bar::Host for PreviewBar {
    fn suggest(&self, _: Option<&str>, _: &str, _: bool) {}
    fn remove_suggestion(&self, _: &str, _: &mut App) {}

    fn step_removal(&self, _: bool, _: &mut App) -> bool {
        false
    }
    fn typed_url(&self, text: &str) -> Option<String> {
        self.search_url(None, text)
    }
    fn search_url(&self, _: Option<&str>, text: &str) -> Option<String> {
        Some(format!("https://duckduckgo.com/?q={text}"))
    }
    fn page_favicon(&self, _: &str) {}
    fn open_tab(&self, _: &str, _: bool) {}
    fn open_window(&self, _: &str, _: &mut App) {}
    fn navigate(&self, _: u64, _: &str) {}
    fn reload(&self, _: u64) {}
    fn select(&self, _: u64) {}
    fn tab_window(&self, _: u64) -> Option<u64> {
        None
    }
    fn activate(&self, _: u64, _: u64, _: &mut App) {}
    fn split(&self, _: u64, _: u64, _: &mut App) {}
    fn open_split(&self, _: &str, _: u64, _: &mut App) {}
    fn copy(&self, _: &str, _: &mut App) {}
    fn show(&self, _: bool, _: f32, _: f32, _: f32, _: f32, _: browser::elevation::Level) {}
    fn set_height(&self, _: f32) {}
    fn hide(&self) {}
    fn dismiss(&self) {}
    fn take_keyboard(&self) {}
    fn commands(&self, text: &str, cx: &App) -> Vec<shell::BarCommand> {
        shell::bar_commands(text, cx)
    }

    /// Made up: Claude ready, so ⇥ shows the ask without an agent's CLI.
    fn task_agents(&self, _: &App) -> Option<command_bar::TaskAgents> {
        let claude = command_bar::TaskAgent {
            id: "claude".into(),
            name: "Claude".into(),
            glyph: "icons/terminal.svg",
            model: Some("Haiku".into()),
            resets: None,
        };
        Some(command_bar::TaskAgents {
            agents: vec![claude],
            known: true,
        })
    }

    fn over_private(&self) -> bool {
        false
    }

    fn asking(&self, _: bool, _: &mut App) {}

    fn task_page(&self, _: &App) -> Option<(SharedString, SharedString)> {
        None
    }

    fn run_task(&self, _: &str, _: &str, _: Option<(SharedString, SharedString)>, _: &mut App) {}

    fn set_up_agents(&self, _: &mut App) {}

    fn tooltip(&self, _: Option<(browser::tooltip::Tip, browser::tooltip::Anchor)>, _: &mut App) {}
}

/// A made-up page's request for a device, as PREVIEW_CHOOSER names it.
fn preview_chooser(name: &str) -> Option<shell::Chooser> {
    use shell::{Chooser, ChooserDevice, DeviceKind, Scan};
    let device = |id: &str, name: &str, detail: &str| ChooserDevice {
        id: id.to_owned().into(),
        name: name.to_owned().into(),
        detail: detail.to_owned().into(),
    };
    let ports = vec![
        device("serial:1", "Arduino Uno", "cu.usbmodem14101"),
        device(
            "serial:2",
            "CP2102 USB to UART Bridge Controller",
            "cu.SLAB_USBtoUART",
        ),
    ];
    let paired = vec![
        device("bt:1", "Polar H10 7E37D222", ""),
        device("bt:2", "TICKR 4F1A", "Paired"),
        device("bt:3", "Forerunner 255", ""),
    ];
    let (kind, devices, scan) = match name {
        "hid" => (
            DeviceKind::Hid,
            vec![device("hid:1", "Keychron K2", "05ac:024f")],
            Scan::Live,
        ),
        "usb" => (
            DeviceKind::Usb,
            vec![device("usb:1", "STM32 BOOTLOADER", "0483:df11")],
            Scan::Live,
        ),
        "none" => (DeviceKind::Usb, Vec::new(), Scan::Live),
        "bluetooth" => (DeviceKind::Bluetooth, paired, Scan::Scanning),
        "stopped" => (DeviceKind::Bluetooth, Vec::new(), Scan::Stopped),
        "off" => (DeviceKind::Bluetooth, Vec::new(), Scan::Off),
        "denied" => (DeviceKind::Bluetooth, Vec::new(), Scan::Denied),
        _ => (DeviceKind::Serial, ports, Scan::Live),
    };
    let site = match kind {
        DeviceKind::Bluetooth => "heartrate.example.com",
        _ => "flasher.example.com",
    };
    Some(Chooser {
        site: site.into(),
        kind,
        devices,
        scan,
    })
}

/// PREVIEW_THEME=#RRGGBB gives the first Space a theme of that colour alone, and
/// PREVIEW_THEME="x=… y=… colors=3 …" the theme as the core keeps it.
fn preview_theme() -> Option<Theme> {
    let text = std::env::var("PREVIEW_THEME").ok()?;
    match text.strip_prefix('#') {
        Some(hex) => u32::from_str_radix(hex, 16).ok().map(Theme::of_color),
        None => Theme::parse(&text),
    }
}

/// PREVIEW_PROFILES=Personal,Work names the profiles, the Spaces' the first, as a second one
/// shows the profile in the name row and Site Controls (#2339).
fn preview_profiles() -> Vec<(u64, SharedString)> {
    let names = std::env::var("PREVIEW_PROFILES").unwrap_or_else(|_| "Default".into());
    (0..)
        .zip(names.split(',').map(|name| name.to_owned().into()))
        .collect()
}

/// PREVIEW_APPEARANCE=light or =dark draws the window as if the system's appearance were that,
/// as a headless session has none to switch.
fn preview_tone() -> Option<Tone> {
    match std::env::var("PREVIEW_APPEARANCE").ok()?.as_str() {
        "light" => Some(Tone::Light),
        "dark" => Some(Tone::Dark),
        _ => None,
    }
}

/// The first task's Space, apart from the ids the rest of the preview takes.
const FIRST_TASK: u64 = 9001;
/// The tasks PREVIEW_TASKS shows: each one's title, status and dot.
const TASKS: [(&str, &str, Dot); 3] = [
    (
        "Find two tickets for Saturday",
        "Step 3 · Clicking “Tickets”",
        Dot::Working,
    ),
    (
        "Compare the museum passes",
        "Stopped after 6 steps",
        Dot::Ended,
    ),
    (
        "Book a table for four",
        "A table for four at 20:00 is held",
        Dot::Done,
    ),
];
/// The pages the done task left the user, each with its site.
const TASK_PAGES: [(&str, &str); 3] = [
    ("Booking · Taberna", "https://taberna.example/booking"),
    ("Menu · Taberna", "https://taberna.example/menu"),
    ("Directions", "https://maps.example/taberna"),
];

/// A task's Space, as the core lists one: its agent's name in the agent column, its agent's id
/// a task's (`Fake::space_agent`).
fn task_space(id: u64) -> SpaceEntry {
    (
        id,
        "Task".into(),
        None,
        0,
        "".into(),
        "Claude".into(),
        "".into(),
    )
}

/// The tasks' cards, each asked in `home`.
fn task_cards(home: u64) -> Vec<TaskCard> {
    let card = |(at, (title, status, dot)): (u64, (&str, &str, Dot))| TaskCard {
        agent: format!("task-{at}").into(),
        home,
        space: at,
        name: "Claude".into(),
        replies: true,
        title: title.into(),
        status: status.into(),
        asks: false,
        dot,
        answers: Vec::new(),
        question: None,
        open: match dot {
            Dot::Done => {
                let site = |url: &str| url.split('/').nth(2).unwrap_or("").into();
                let pages = (FIRST_TASK + 100..).zip(TASK_PAGES);
                pages.map(|(tab, (_, url))| (tab, site(url))).collect()
            }
            _ => Vec::new(),
        },
        retry: false,
        command: None,
    };
    let mut cards: Vec<TaskCard> = (FIRST_TASK..).zip(TASKS).map(card).collect();
    // Newest first, as the Agents send them.
    cards.reverse();
    cards
}

/// The working task's conversation: a step done, one that failed, what the agent said of it, and
/// the step under way, on four pages, so it opens on Pages.
fn task_conversation(agent: SharedString) -> Conversation {
    let (title, _, _) = TASKS[0];
    let step = |key, glyph, title: &str, description: &str, running, failed| TraceStep {
        key,
        glyph,
        title: title.into(),
        description: description.into(),
        running,
        failed,
        opens: match failed {
            true => Opens::Text("Another element would get the click: div.cookie-banner".into()),
            false => Opens::Nothing,
        },
    };
    let (click, read) = ("icons/mouse-pointer-2.svg", "icons/text-align-start.svg");
    let items = vec![
        TraceItem::Step(step(
            0,
            "icons/globe.svg",
            "Opened tickets.example",
            "2 s",
            false,
            false,
        )),
        TraceItem::Step(step(1, read, "Read tickets.example", "", false, false)),
        TraceItem::Step(step(
            2,
            click,
            "Couldn't click “Tickets”",
            "A banner covers it",
            false,
            true,
        )),
        TraceItem::Said("The banner is in the way, so I'll close it first.".into()),
        TraceItem::Step(step(3, click, "Clicked “Accept”", "", false, false)),
        TraceItem::Step(step(4, click, "Clicking “Tickets”", "", true, false)),
    ];
    Conversation {
        agent,
        name: "Claude".into(),
        title: title.into(),
        about: "Claude · Working".into(),
        progress: "step 3 · 4 pages".into(),
        asked: "Find two tickets for Saturday's concert, in the stalls".into(),
        page: None,
        lines: vec![Line::Trace(Trace {
            key: 0,
            running: true,
            took: None,
            items,
        })],
        tabs: vec![
            (FIRST_TASK + 200, "tickets.example".into()),
            (FIRST_TASK + 201, "seatmap.example".into()),
            (FIRST_TASK + 202, "tickets.example".into()),
            (FIRST_TASK + 203, "maps.example".into()),
        ],
        on: Some(FIRST_TASK + 200),
        stoppable: true,
        open: None,
        answers: Vec::new(),
        writing: false,
    }
}

/// Opens the command bar at `at`, with the suggestions for what `mode` has typed.
fn preview_bar(mode: &str, at: Point<Pixels>, cx: &mut App) {
    let page = |text: &str, detail: &str, url: &str, tab: u64| Suggestion {
        url: url.into(),
        text: text.to_string().into(),
        detail: detail.to_string().into(),
        tab,
        ..Suggestion::default()
    };
    let search = |text: &str| Suggestion {
        url: format!("https://duckduckgo.com/?q={text}"),
        text: text.to_string().into(),
        ..Suggestion::default()
    };
    // The Space's tabs, which the bar lists before anything is typed.
    let tabs: Vec<shell::BarTab> = [
        ("The Browser Company", "https://thebrowser.company/", 1),
        (
            "Arc (web browser) - Wikipedia",
            "https://en.wikipedia.org/wiki/Arc",
            2,
        ),
        ("Zed — The editor for what's next", "https://zed.dev/", 3),
        ("Example Domain", "https://example.com/", 5),
    ]
    .into_iter()
    .map(|(title, url, tab)| shell::BarTab {
        tab,
        title: title.into(),
        url: url.into(),
        favicon: None,
        renamed: false,
        switched: 0.0,
    })
    .collect();
    let (typed, rows) = match mode {
        "empty" => ("", tabs.iter().cloned().map(Suggestion::from).collect()),
        "commands" => {
            let command = |name: &str, command, shortcut: Option<&str>, matched| {
                Suggestion::from(shell::BarCommand {
                    name: name.to_string().into(),
                    command,
                    shortcut: shortcut.map(|key| key.to_string().into()),
                    matched,
                })
            };
            // Arc's "clear": the best command above the search row, others whose names start
            // with it under it, and a weaker one after the suggestions.
            let (cache, cookies) = (
                Command::ClearCacheAndRefresh,
                Command::ClearCookiesAndRefresh,
            );
            let clear_keys = shell::keystroke_label("shift-cmd-k");
            let rows = vec![
                command(
                    "Clear Today",
                    Command::Clear,
                    Some(&clear_keys),
                    Match::Name,
                ),
                search("clear"),
                command("Clear Cache and Refresh", cache, None, Match::Name),
                command("Clear Cookies and Refresh", cookies, None, Match::Name),
                page("clearhaus", "clearhaus.com", "https://clearhaus.com/", 0),
                command(
                    "Collapse All Folders",
                    Command::CollapseFolders,
                    None,
                    Match::Letters,
                ),
            ];
            ("clear", rows)
        }
        // The bar adds Open Library… itself, whose glyph the sidebar foot's button shares.
        "library" => ("library", vec![search("library")]),
        "sites" => (
            "wik",
            vec![
                Suggestion {
                    keyword: "wikipedia.org".into(),
                    site: "Wikipedia".into(),
                    ..page("Wikipedia", "wikipedia.org", "https://wikipedia.org/", 0)
                },
                search("wik"),
                page("Wikimedia", "wikimedia.org", "https://wikimedia.org/", 0),
                page("Fandom", "wikia.com", "https://wikia.com/", 0),
            ],
        ),
        _ => (
            "e",
            vec![
                page("Example Domain", "", "https://example.com/", 5),
                search("e"),
                // Only in the history, which Arc marks with a clock.
                Suggestion {
                    removable: true,
                    ..page(
                        "Hacker News",
                        "news.ycombinator.com",
                        "https://news.ycombinator.com/",
                        0,
                    )
                },
                page(
                    "Arc (web browser) - Wikipedia",
                    "",
                    "https://en.wikipedia.org/wiki/Arc",
                    1,
                ),
                page(
                    "Zed — The editor for what's next",
                    "",
                    "https://zed.dev/",
                    3,
                ),
                page(
                    "Rust Programming Language",
                    "",
                    "https://www.rust-lang.org/",
                    4,
                ),
                page("The Browser Company", "", "https://thebrowser.company/", 0),
            ],
        ),
    };
    // The command the bar adds for what's typed takes a row too.
    let rows_shown = rows.len() + usize::from(mode == "library");
    let bounds = Bounds::new(
        at,
        size(px(command_bar::WIDTH), px(command_bar::height(rows_shown))),
    );
    if command_bar::open_at(Rc::new(PreviewBar), bounds, true, cx).is_none() {
        return;
    }
    let theme = preview_theme();
    let forced = theme.and_then(|theme| match theme.appearance {
        Appearance::Light => Some(Tone::Light),
        Appearance::Dark => Some(Tone::Dark),
        Appearance::Automatic => None,
    });
    let tone = forced.or(preview_tone()).unwrap_or_default();
    palette::set_current(Palette::of(theme.as_ref(), tone), cx);
    match typed {
        "" => command_bar::show(None, Vec::new(), tabs, None, cx),
        typed => {
            let editing = Some((0, typed.to_string().into()));
            command_bar::show(editing, Vec::new(), Vec::new(), None, cx)
        }
    }
    let icons = [
        Icon::Envelope,
        Icon::Blocks,
        Icon::Play,
        Icon::Disc([0.9, 0.5, 0.1]),
    ];
    let favicons: Vec<_> = rows
        .iter()
        .zip(icons.iter().cycle())
        .map(|(row, icon)| {
            let png = icon.png();
            let image = png.map(|png| gpui::Image::from_bytes(gpui::ImageFormat::Png, png));
            (row.url.clone(), image.map(Arc::new))
        })
        .collect();
    command_bar::suggested(typed.to_string(), rows, cx);
    for (url, favicon) in favicons {
        command_bar::page_favicon(&url, favicon, cx);
    }
}

/// wasm-bindgen runs `main` as the page loads the module, and the page starts the demo with
/// `web::start`.
#[cfg(target_family = "wasm")]
fn main() {}

#[cfg(not(target_family = "wasm"))]
fn main() {
    gpui_platform::application()
        .with_assets(shell::Assets)
        .run(|cx| {
            browser::face::register(cx);
            shell::bind_keys(cx);
            browser::text_input::bind_keys(cx);
            // PREVIEW_SIDEBAR_RIGHT=1 puts the sidebar on the window's right, as Settings' Sidebar
            // position does.
            if std::env::var_os("PREVIEW_SIDEBAR_RIGHT").is_some() {
                cx.set_global(shell::SidebarRight(true));
            }
            // PREVIEW_UPDATE=ready or =available shows the update banner on a made-up 0.1.5, opened
            // as under the pointer with =ready-open or =available-open, and PREVIEW_UPDATE_CHECKS=off
            // as with the updater's automatic checks off.
            let update = std::env::var("PREVIEW_UPDATE").ok();
            if let Some(update) = &update {
                preview_update(update, cx);
            }
            // PREVIEW_SIDEBAR_POSITION=left or right shows Settings › Tabs & Sidebar's sidebar group
            // alone, Sidebar position's drawings with that side chosen (#2407).
            if let Ok(side) = std::env::var("PREVIEW_SIDEBAR_POSITION") {
                preview_pictures(
                    Pictures::Sidebar {
                        right: side == "right",
                    },
                    cx,
                );
                return;
            }
            // PREVIEW_APPEARANCE_PICKER=automatic, light or dark shows Settings › General's Look
            // group alone, Appearance's drawings with that one chosen and Settings drawn in it,
            // Automatic's as PREVIEW_APPEARANCE has the system (#2429).
            if let Ok(chosen) = std::env::var("PREVIEW_APPEARANCE_PICKER") {
                let chosen = match chosen.as_str() {
                    "light" => browser::theme::Appearance::Light,
                    "dark" => browser::theme::Appearance::Dark,
                    _ => browser::theme::Appearance::Automatic,
                };
                preview_pictures(Pictures::Appearance(chosen), cx);
                return;
            }
            // PREVIEW_SHARE_FAVORITES=every, profile or space shows Settings › Tabs & Sidebar's
            // Share favorites group alone, its drawings with that scope chosen (#2430).
            if let Ok(chosen) = std::env::var("PREVIEW_SHARE_FAVORITES") {
                let chosen = match chosen.as_str() {
                    "every" => browser::shell::FavoritesScope::Every,
                    "space" => browser::shell::FavoritesScope::Space,
                    _ => browser::shell::FavoritesScope::Profile,
                };
                preview_pictures(Pictures::Favorites(chosen), cx);
                return;
            }
            // PREVIEW_ACCOUNT=signed-out, no-server, words, waiting or signed-in shows Settings ›
            // Account in that state, alone, no-server's for a build that names no sync server; =approve
            // shows another device's request over the browser window.
            let account = std::env::var("PREVIEW_ACCOUNT").ok();
            if let Some(state) = account.as_deref().filter(|state| *state != "approve") {
                preview_account(state, cx);
                return;
            }
            // PREVIEW_SIZE=<width>x<height> opens the window at that size.
            let (width, height) = std::env::var("PREVIEW_SIZE")
                .ok()
                .and_then(|size| {
                    let (width, height) = size.split_once('x')?;
                    Some((width.parse().ok()?, height.parse().ok()?))
                })
                .unwrap_or((1280., 800.));
            let (mut x, mut y) = (0., 0.);
            unsafe { host_centered(width, height, &mut x, &mut y) };
            let bounds = Bounds::new(
                point(px(x as f32), px(y as f32)),
                size(px(width as f32), px(height as f32)),
            );
            PREVIEW_ORIGIN.set(bounds.origin);
            PREVIEW_AREA.set(bounds.size);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: None,
                    appears_transparent: true,
                    // Arc's: 16 pt buttons centred at (20, 24), 23 apart.
                    traffic_light_position: Some(point(px(12.), px(16.))),
                }),
                ..Default::default()
            };
            let window = cx.open_window(options, |window, cx| {
                let shell = cx.new(|cx| {
                    let first_space = TABS.len() as u64 + 1;
                    // PREVIEW_AGENT=1 makes Reading Claude Code's Space and shows it, with the agent at
                    // work in its third tab and its last loading; =loading has the third load too.
                    let agent = std::env::var("PREVIEW_AGENT").ok();
                    // The preview's Spaces and favorites are in its one profile, 0.
                    // PREVIEW_SPACE_ICONS=1 gives them glyphs, for the footer's row of icons.
                    let glyphs = std::env::var_os("PREVIEW_SPACE_ICONS").is_some();
                    // PREVIEW_SPACES=<count> makes that many, those past the two named by number, for
                    // a switcher and a Library's Spaces that run out of room.
                    let count = std::env::var("PREVIEW_SPACES").ok();
                    let count = count.and_then(|count| count.parse().ok());
                    let count: usize = count.unwrap_or(SPACES.len()).max(SPACES.len());
                    let named = (0..count).map(|index| match SPACES.get(index) {
                        Some(&(name, color)) => (name.to_string(), color),
                        None => (
                            format!("Space {}", index + 1),
                            Some(MORE_COLORS[(index - SPACES.len()) % MORE_COLORS.len()]),
                        ),
                    });
                    let mut spaces: Vec<SpaceEntry> = (first_space..)
                        .zip(named)
                        .zip(["glyph:star", "glyph:leaf"].into_iter().cycle())
                        .map(|((id, (name, color)), icon)| {
                            let icon = if glyphs { icon } else { "" };
                            let agent = match agent.is_some() && id == first_space + 1 {
                                true => "Claude Code".into(),
                                false => "".into(),
                            };
                            (id, name.into(), color, 0, icon.into(), agent, "".into())
                        })
                        .collect();
                    if let Some(theme) = preview_theme() {
                        spaces[0].2 = Some(theme.main_color());
                        spaces[0].6 = theme.to_text().into();
                    }
                    // PREVIEW_TASKS=1 shows three tasks' cards at the foot, one at work, one
                    // stopped and one done; =open shows the first's conversation, as its card's
                    // click does.
                    let tasks = std::env::var("PREVIEW_TASKS").ok();
                    if tasks.is_some() {
                        spaces.extend((FIRST_TASK..).take(TASKS.len()).map(task_space));
                    }
                    let first_archived = first_space + count as u64;
                    let archive = (first_archived..)
                        .zip(ARCHIVE)
                        .map(|(id, (title, url, days))| (id, title, url, now() - days * DAY))
                        .collect();
                    let first_download = first_archived + ARCHIVE.len() as u64;
                    let downloads = (first_download..)
                        .zip(DOWNLOADS)
                        .map(
                            |(id, (name, url, received, total, state, held, days))| Download {
                                id,
                                name: name.into(),
                                url: url.into(),
                                path: SharedString::default(),
                                received,
                                total,
                                state,
                                open_when_done: false,
                                held,
                                started: now() - days * DAY,
                                profile: 0,
                                private: false,
                            },
                        )
                        .collect();
                    // In a private window, the running one and the first finished one are its
                    // session's, which its Library lists alone.
                    let mut downloads: Vec<Download> = downloads;
                    if std::env::var("PREVIEW_EMPTY").as_deref() == Ok("private") {
                        for (index, download) in downloads.iter_mut().enumerate() {
                            download.private = matches!(index, 0 | 3);
                        }
                    }
                    let first_favorite = first_download + DOWNLOADS.len() as u64;
                    let favorites: Vec<u64> = (first_favorite..).take(FAVORITES.len()).collect();
                    let folder = first_favorite + FAVORITES.len() as u64;
                    let browser = Rc::new(Fake {
                        shell: cx.weak_entity(),
                        next_id: Rc::new(Cell::new(folder + 1 + READING.len() as u64)),
                        spaces: RefCell::new(spaces.clone()),
                        archive: RefCell::new(archive),
                        downloads: RefCell::new(downloads),
                        opt_ins: Cell::new((false, false)),
                        opens_on: RefCell::default(),
                        shows: Rc::default(),
                    });
                    browser.report_archive(cx);
                    browser.report_downloads(cx);
                    // PREVIEW_EMPTY=1 shows no page, as a fresh ⌘N window does; =reading does so in the
                    // second Space, for its colour, and =private in a private window with no tabs.
                    // PREVIEW_WELCOME=1 shows no page with the first run's welcome, and
                    // PREVIEW_ONBOARDING=1, =light or =dark the onboarding over it first.
                    let onboarding = std::env::var("PREVIEW_ONBOARDING").ok();
                    let welcome =
                        std::env::var_os("PREVIEW_WELCOME").is_some() || onboarding.is_some();
                    let empty = std::env::var("PREVIEW_EMPTY")
                        .ok()
                        .or(welcome.then(|| "1".into()));
                    let mut shell = match empty.as_deref() {
                        Some("private") => {
                            Shell::private_window(browser, first_space, "Private".into(), 0, cx)
                        }
                        _ => Shell::new(browser, cx),
                    };
                    shell.force_tone(preview_tone());
                    shell.set_profiles(preview_profiles(), cx);
                    shell.set_spaces(spaces, cx);
                    shell.set_space(first_space, cx);
                    // The shell forgets what it knows of a tab in no list, so the lists come first.
                    shell.set_favorites(favorites.iter().map(|&tab| (tab, 0)).collect(), cx);
                    for (&tab, (title, url, icon)) in favorites.iter().zip(FAVORITES) {
                        shell.set_title(tab, title.into(), cx);
                        shell.set_page(
                            tab,
                            Page {
                                url: url.into(),
                                ..Default::default()
                            },
                            cx,
                        );
                        let favicon = icon.png().map(|png| {
                            Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, png))
                        });
                        shell.set_favicon(tab, favicon, cx);
                    }
                    for (index, (title, url)) in TABS.iter().enumerate() {
                        let tab = index as u64 + 1;
                        shell.set_title(tab, (*title).into(), cx);
                        let page = Page {
                            url: (*url).into(),
                            // The first tab has been somewhere before, so back shows bright, and it's
                            // pinned somewhere else. The second is a pinned tab closed, and the third
                            // is still loading and plays sound.
                            can_go_back: index == 0,
                            away_from_pinned_url: index == 0,
                            unloaded: index == 1,
                            loading: index == 2,
                            audible: index == 2,
                            ..Default::default()
                        };
                        shell.set_page(tab, page, cx);
                    }
                    // PREVIEW_DRM=1 has the first tab's page ask for video ilo can't play, so its
                    // note shows, offering what this system's does (#2468).
                    if std::env::var_os("PREVIEW_DRM").is_some() {
                        shell.set_media_unsupported(1, "com.widevine.alpha", cx);
                    }
                    // PREVIEW_CHOOSER=serial, hid, usb, bluetooth, stopped, none, off or denied has the
                    // first tab's page ask for a device, so its chooser shows.
                    if let Ok(chooser) = std::env::var("PREVIEW_CHOOSER") {
                        shell.set_chooser(1, preview_chooser(&chooser), cx);
                    }
                    // PREVIEW_PAIR=pin, confirm or passkey has a Bluetooth device the first tab's
                    // page pairs with ask, so its card shows.
                    if let Ok(pair) = std::env::var("PREVIEW_PAIR") {
                        let kind = match pair.as_str() {
                            "pin" => shell::PairKind::Pin,
                            "passkey" => shell::PairKind::Passkey("123456".into()),
                            _ => shell::PairKind::Confirm,
                        };
                        let device = "Polar H10 7E37D222".into();
                        shell.set_pair(1, Some(shell::Pair { device, kind }), cx);
                    }
                    // The second in a folder, and the last two in a split view.
                    let pinned = vec![1, folder, 2];
                    let folders = Folders {
                        folders: [(folder, ("Docs".into(), true))].into(),
                        parents: [(2, folder)].into(),
                        ..Default::default()
                    };
                    let today = (3..=TABS.len() as u64).collect();
                    let split = Split {
                        tabs: vec![TABS.len() as u64 - 1, TABS.len() as u64],
                        ..Split::default()
                    };
                    shell.set_tabs(first_space, pinned, today, vec![split], folders, cx);
                    let reading: Vec<u64> = (folder + 1..).take(READING.len()).collect();
                    let (pinned, today) = (reading[..1].to_vec(), reading[1..].to_vec());
                    shell.set_tabs(
                        first_space + 1,
                        pinned,
                        today,
                        Vec::new(),
                        Folders::default(),
                        cx,
                    );
                    for (index, (&tab, (title, url))) in reading.iter().zip(READING).enumerate() {
                        shell.set_title(tab, title.into(), cx);
                        let loading = agent.is_some()
                            && (index == 3 || (index == 2 && agent.as_deref() == Some("loading")));
                        shell.set_page(
                            tab,
                            Page {
                                url: url.into(),
                                loading,
                                ..Default::default()
                            },
                            cx,
                        );
                    }
                    if agent.is_some() {
                        shell.set_space(first_space + 1, cx);
                        shell.agent_acted("claude-code".into(), reading[2], cx);
                        // =handoff has it hand its fourth tab to the user, shown, for a sign-in;
                        // =back has the user take that tab over, and the agent ask for it back.
                        let back = agent.as_deref() == Some("back");
                        let handoff = back || agent.as_deref() == Some("handoff");
                        let why: SharedString = match back {
                            true => "To finish the order".into(),
                            false => "Sign in to finish the order".into(),
                        };
                        let held = match (handoff, back) {
                            (true, true) => vec![(reading[3], SharedString::default())],
                            (true, false) => vec![(reading[3], why.clone())],
                            _ => Vec::new(),
                        };
                        let status = browser::agents::Status {
                            agent: "claude-code".into(),
                            name: "Claude Code".into(),
                            space: first_space + 1,
                            doing: "Clicked · en.wikipedia.org".into(),
                            held,
                        };
                        shell.set_agent_status(vec![status], cx);
                        if handoff {
                            shell.set_selected(reading[3], cx);
                            let question = browser::agents::Question {
                                id: 1,
                                agent: "Claude Code".into(),
                                from: "claude-code".into(),
                                what: "hand you a tab".into(),
                                why,
                                handoff: Some(reading[3]),
                                back,
                                ..Default::default()
                            };
                            shell.set_agent_question(Some(question), cx);
                        }
                    }
                    let favorite = std::env::var("PREVIEW_FAVORITE")
                        .ok()
                        .and_then(|n| n.parse().ok());
                    match (
                        empty.as_deref(),
                        favorite.and_then(|n: usize| favorites.get(n)),
                    ) {
                        (Some("private"), _) => {
                            let none = Folders::default();
                            shell.set_tabs(
                                first_space,
                                Vec::new(),
                                Vec::new(),
                                Vec::new(),
                                none,
                                cx,
                            );
                            shell.set_selected(0, cx);
                        }
                        (Some("reading"), _) => {
                            shell.set_space(first_space + 1, cx);
                            shell.set_selected(0, cx);
                        }
                        (Some(_), _) => shell.set_selected(0, cx),
                        (None, _) if agent.is_some() => shell.set_selected(reading[0], cx),
                        (None, Some(&tab)) => shell.set_selected(tab, cx),
                        (None, None) => shell.set_selected(1, cx),
                    }
                    // PREVIEW_TRANSLATE=<language> says the made-up pages are in that language,
                    // so the URL field offers to translate them.
                    if let Ok(language) = std::env::var("PREVIEW_TRANSLATE") {
                        for tab in 0..=TABS.len() as u64 {
                            shell.set_page_language(tab, &language, cx);
                        }
                    }
                    if let Some(tasks) = &tasks {
                        let pages: Vec<u64> = (FIRST_TASK + 100..).take(TASK_PAGES.len()).collect();
                        let none = Folders::default();
                        shell.set_tabs(
                            FIRST_TASK + 2,
                            Vec::new(),
                            pages.clone(),
                            Vec::new(),
                            none,
                            cx,
                        );
                        for (&tab, (title, url)) in pages.iter().zip(TASK_PAGES) {
                            shell.set_title(tab, title.into(), cx);
                            shell.set_page(
                                tab,
                                Page {
                                    url: url.into(),
                                    ..Default::default()
                                },
                                cx,
                            );
                        }
                        shell.set_task_cards(task_cards(first_space), cx);
                        // After the first frame, which sets the palette the reply field takes, as
                        // a click on the card comes after it.
                        if tasks == "open" {
                            let (shown, agent) = (cx.weak_entity(), format!("task-{FIRST_TASK}"));
                            window.on_next_frame(move |_, cx| {
                                shown
                                    .update(cx, |shell, cx| shell.show_task(agent.into(), cx))
                                    .ok();
                            });
                        }
                    }
                    // PREVIEW_TROUBLE=crashed shows the tab shown's page died, =hung that it stopped
                    // answering, which shows in its row (the note is the toast's, a window the preview
                    // hasn't), and =split that the second page of the split view died beside the first.
                    let trouble = match std::env::var("PREVIEW_TROUBLE").as_deref() {
                        Ok("crashed") => Some((1, Trouble::Crashed)),
                        Ok("hung") => Some((1, Trouble::Hung)),
                        Ok("split") => {
                            shell.set_selected(TABS.len() as u64 - 1, cx);
                            Some((TABS.len() as u64, Trouble::Crashed))
                        }
                        _ => None,
                    };
                    if let Some((tab, trouble)) = trouble {
                        shell.set_page(
                            tab,
                            Page {
                                trouble,
                                ..shell.page(tab)
                            },
                            cx,
                        );
                    }
                    // PREVIEW_DEVELOPER_MODE=1 has the tab shown, the third tab and the GitHub
                    // favorite in Developer Mode, so its outline and contrast show.
                    if std::env::var_os("PREVIEW_DEVELOPER_MODE").is_some() {
                        for tab in [shell.selected(), 3, favorites[2]] {
                            let page = Page {
                                developer_mode: true,
                                ..shell.page(tab)
                            };
                            shell.set_page(tab, page, cx);
                        }
                    }
                    if welcome {
                        shell.set_welcome(cx);
                    }
                    if let Some(appearance) = &onboarding {
                        shell.start_onboarding(Some(appearance), cx);
                    }
                    shell
                });
                window.focus(&shell.focus_handle(cx), cx);
                shell
            });
            let shell_window = window.as_ref().ok().copied();
            match window {
                // PREVIEW_LIBRARY=1 opens the Library, and PREVIEW_LIBRARY=downloads, =spaces or
                // =agents opens it there; =rename opens its Spaces with the first card's name in its field.
                Ok(window) if std::env::var_os("PREVIEW_LIBRARY").is_some() => {
                    window
                        .update(cx, |shell, window, cx| {
                            match std::env::var("PREVIEW_LIBRARY").as_deref() {
                                Ok("downloads") => shell.run(Command::ViewDownloads, "preview", cx),
                                Ok("spaces") => shell.show_spaces(cx),
                                Ok("agents") => shell.show_agents(cx),
                                Ok("rename") => {
                                    shell.show_spaces(cx);
                                    shell.preview_rename_card(window, cx);
                                }
                                _ => shell.run(Command::ViewLibrary, "preview", cx),
                            }
                        })
                        .ok();
                }
                Ok(window)
                    if update
                        .as_deref()
                        .is_some_and(|update| update.ends_with("-open")) =>
                {
                    window
                        .update(cx, |shell, _, cx| shell.preview_update_banner_open(cx))
                        .ok();
                }
                Ok(_) => {}
                Err(error) => {
                    log!("couldn't open the window: {error}");
                    cx.quit();
                }
            }
            // PREVIEW_BAR=tabs, sites, commands or library shows the command bar over the window, with
            // made-up suggestions.
            if let Ok(mode) = std::env::var("PREVIEW_BAR") {
                let left = x + (width - command_bar::WIDTH as f64) / 2.;
                let top = y as f32 + command_bar::centered_top(height as f32);
                let at = point(px(left as f32), px(top));
                preview_bar(&mode, at, cx);
            }
            if account.as_deref() == Some("approve") {
                if !DIALOG_OPEN.replace(true) {
                    dialog::open_at(Rc::new(PreviewDialog), bounds, true, cx);
                }
                let pairing = account::Pairing {
                    id: "2".into(),
                    name: "MacBook Air".into(),
                    os: "macOS".into(),
                    code: "312568".into(),
                    asked: "15:04".into(),
                    signed_in: "2 minutes ago".into(),
                };
                dialog::ask_account(account::pairing_question(&pairing), Box::new(|_, _| {}), cx);
            }
            // PREVIEW_ICON_PICKER=<emoji, or empty> shows the icon picker for a Space with that icon.
            if let Ok(icon) = std::env::var("PREVIEW_ICON_PICKER") {
                let left = x as f32 + 40.;
                let top = y as f32 + 120.;
                let bounds = Bounds::new(
                    point(px(left), px(top)),
                    size(px(icon_picker::WIDTH), px(icon_picker::HEIGHT)),
                );
                if icon_picker::open_at(
                    Rc::new(PreviewPicker),
                    Recents::default(),
                    bounds,
                    true,
                    cx,
                )
                .is_some()
                {
                    let picked: icon_picker::Picked = Rc::new(|_, _| {});
                    let subject = icon_picker::Subject::Space(1);
                    icon_picker::show(0, subject, icon.into(), point(0., 0.), picked, cx);
                }
            }
            // PREVIEW_THEME_PICKER=1 shows the theme picker for the first Space, with PREVIEW_THEME's
            // theme.
            if std::env::var_os("PREVIEW_THEME_PICKER").is_some() {
                let bounds = Bounds::new(
                    point(px(x as f32 + 230.), px(y as f32 + 100.)),
                    size(px(theme_picker::WIDTH), px(theme_picker::HEIGHT)),
                );
                if theme_picker::open_at(Rc::new(PreviewPicker), bounds, true, cx).is_some() {
                    let changed: theme_picker::Changed = Rc::new(|_, _, _| {});
                    let anchor = point(0., theme_picker::HEIGHT / 2. + 8.);
                    let at = theme_picker::At::Row(anchor);
                    theme_picker::show(0, 1, preview_theme(), at, false, f32::MAX, changed, cx);
                }
            }
            // PREVIEW_BOOST_EDITOR=new shows the Boost editor for a site's new Boost, its hint up
            // and its dots unplaced, =edited for one with its edits made, and =advanced for that
            // one with Advanced open and its sliders moved.
            if let Some(kind) = std::env::var_os("PREVIEW_BOOST_EDITOR") {
                let edited = Style {
                    ground: Some(boost_style::Dot { x: -0.3, y: 0.86 }),
                    links: Some(boost_style::Dot { x: 0.5, y: -0.2 }),
                    face: Some(Face::Serif),
                    size: 110,
                    case: Some(Case::Lower),
                    dragged: true,
                    name: Some("Quiet news".into()),
                    ..Style::default()
                };
                let style = match kind.to_str() {
                    Some("edited") => edited,
                    Some("advanced") => Style {
                        adjust: [110, 100, 80],
                        advanced: true,
                        ..edited
                    },
                    _ => Style::default(),
                };
                let bounds = Bounds::new(
                    point(px(x as f32 + 230.), px(y as f32 + 34.)),
                    size(px(boost_editor::WIDTH), px(boost_editor::height_of(&style))),
                );
                if boost_editor::open_at(Rc::new(PreviewPicker), bounds, true, cx).is_some() {
                    let requested: boost_editor::Requested = Rc::new(|_, _| {});
                    let at = theme_picker::At::Row(point(0., 30.));
                    let palette = browser::palette::Palette::of(
                        preview_theme().as_ref(),
                        browser::palette::Tone::Dark,
                    );
                    let site = "news.example".to_string();
                    boost_editor::show(
                        0,
                        site,
                        None,
                        style,
                        true,
                        at,
                        false,
                        f32::MAX,
                        palette,
                        requested,
                        cx,
                    );
                }
            }
            // PREVIEW_FOOT_BAR=arrange shows the foot bar over the page's bottom centre with one
            // line, as Arrange's, =point with two, as pointing's, and =long with a question past
            // its two lines.
            if let Ok(kind) = std::env::var("PREVIEW_FOOT_BAR")
                && let Some(shell) = shell_window
            {
                preview_foot_bar(&kind, shell, bounds, cx);
            }
            // PREVIEW_SITE_CONTROLS=1 opens the Site Controls of the tab shown, once the shell has
            // drawn the URL field they hang from.
            let site_controls = std::env::var_os("PREVIEW_SITE_CONTROLS").is_some();
            if let Some(shell) = shell_window.filter(|_| site_controls) {
                cx.spawn(async move |cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(500))
                        .await;
                    shell
                        .update(cx, |shell, _, cx| {
                            shell.run(Command::SiteControls, "preview", cx)
                        })
                        .ok();
                })
                .detach();
            }
            // PREVIEW_SWIPE=1 brings the Library out with fingers going right on the first Space, and
            // takes it back with them going left, through the gesture's own code, then does the
            // same with clicks, for a recording of its motion (#2375).
            if let Some(shell) =
                shell_window.filter(|_| std::env::var_os("PREVIEW_SWIPE").is_some())
            {
                cx.spawn(async move |cx| {
                    let executor = cx.background_executor().clone();
                    let wait = |ms| executor.timer(Duration::from_millis(ms));
                    wait(1000).await;
                    // Fingers 1.2 Library widths each way in half a second, a frame at a time.
                    for way in [1., -1.] {
                        log!(
                            "browser: preview swipe {}",
                            if way > 0. { "in" } else { "out" }
                        );
                        let step = |phase, x| {
                            move |shell: &mut Shell,
                                  window: &mut gpui::Window,
                                  cx: &mut gpui::Context<Shell>| {
                                shell.preview_fingers(x, phase, window, cx)
                            }
                        };
                        shell.update(cx, step(TouchPhase::Started, 0.)).ok();
                        for _ in 0..30 {
                            wait(16).await;
                            let x = way * 1.2 * 417. / 30. / 0.5;
                            shell.update(cx, step(TouchPhase::Moved, x)).ok();
                        }
                        shell.update(cx, step(TouchPhase::Ended, 0.)).ok();
                        wait(2000).await;
                    }
                    for stage in ["in", "out"] {
                        log!("browser: preview click {stage}");
                        shell
                            .update(cx, |shell, _, cx| {
                                shell.run(Command::ViewLibrary, "preview", cx)
                            })
                            .ok();
                        wait(2000).await;
                    }
                    log!("browser: preview swipe done");
                })
                .detach();
            }
            // PREVIEW_PEEK=1 opens a link of the page shown in Peek, once the shell has laid the page
            // out, with the shell's page showing through the card, as it does on Linux and Windows.
            if let Some(shell) = shell_window.filter(|_| std::env::var_os("PREVIEW_PEEK").is_some())
            {
                let peek_bounds = Bounds::new(PREVIEW_ORIGIN.get(), PREVIEW_AREA.get());
                cx.spawn(async move |cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(500))
                        .await;
                    let Ok(Some(laid)) = shell.update(cx, |shell, _, _| shell.page_laid()) else {
                        return;
                    };
                    cx.update(|cx| {
                        link_preview::open_at(Rc::new(PreviewPeek), peek_bounds, true, cx);
                        let url = "https://en.wikipedia.org/wiki/Hyperlink".into();
                        let link = laid.0.origin + point(px(160.), px(120.));
                        link_preview::show(1, 99, 1, url, laid, link, None, cx);
                    });
                })
                .detach();
            }
            // PREVIEW_MENU=<x>,<y>[,<key>…] right-clicks there in the window, as on a row or a Space's
            // name, and then presses the keys in ilo's menu, such as `m,right` for a submenu.
            if let Ok(what) = std::env::var("PREVIEW_MENU") {
                if let Some(shell) = shell_window {
                    preview_menu(shell.into(), bounds, &what, gpui::MouseButton::Right, cx);
                }
            }
            // PREVIEW_CLICK=<x>,<y> clicks there in the window, as on an archived tab's row.
            if let Ok(what) = std::env::var("PREVIEW_CLICK") {
                if let Some(shell) = shell_window {
                    preview_menu(shell.into(), bounds, &what, gpui::MouseButton::Left, cx);
                }
            }
            // PREVIEW_MODEL_MENU=<claude|codex>[,<model>] shows Settings › Agents' Model menu for that
            // CLI as it opens, from the CLI's recorded list, on Default or the model chosen, light
            // with PREVIEW_APPEARANCE=light (#2455).
            if let Ok(cli) = std::env::var("PREVIEW_MODEL_MENU") {
                preview_model_menu(bounds, &cli, cx);
            }
            // PREVIEW_PAGE_MENU=<link|image|text|field>,<x>,<y>[,<key>…] shows a page's menu there, made
            // up as Chrome builds it for a link, an image, selected text or a misspelt word in a text
            // field, and then presses the keys in ilo's menu, such as `s,right` for a submenu.
            if let Ok(what) = std::env::var("PREVIEW_PAGE_MENU") {
                PREVIEW_AREA.set(bounds.size);
                if let Some(shell) = shell_window {
                    preview_page_menu(shell, bounds, &what, cx);
                }
            }
            cx.activate(true);
        });
}

fn preview_menu(
    shell: gpui::AnyWindowHandle,
    bounds: Bounds<Pixels>,
    what: &str,
    button: gpui::MouseButton,
    cx: &mut App,
) {
    let mut parts = what.split(',');
    let mut number = || {
        parts
            .next()
            .and_then(|part| part.trim().parse::<f32>().ok())
    };
    let (Some(x), Some(y)) = (number(), number()) else {
        return;
    };
    let keys: Vec<String> = what
        .split(',')
        .skip(2)
        .map(|key| key.trim().to_string())
        .collect();
    let Some(window) = menu::open_at(Rc::new(PreviewMenu), bounds, cx) else {
        return;
    };
    // Not the menu's own handle, whose update would hold the menu the keys go to.
    let window: gpui::AnyWindowHandle = window.into();
    cx.spawn(async move |cx| {
        // Once the shell has drawn its rows, whose boxes the click finds.
        cx.background_executor()
            .timer(Duration::from_millis(500))
            .await;
        let position = point(px(x), px(y));
        let modifiers = gpui::Modifiers::default();
        let down = gpui::MouseDownEvent {
            button,
            position,
            modifiers,
            click_count: 1,
            first_mouse: false,
        };
        let up = gpui::MouseUpEvent {
            button,
            position,
            modifiers,
            click_count: 1,
        };
        shell
            .update(cx, |_, window, cx| {
                window.dispatch_event(gpui::PlatformInput::MouseDown(down), cx);
                window.dispatch_event(gpui::PlatformInput::MouseUp(up), cx);
            })
            .ok();
        press_keys(window, keys, cx).await;
    })
    .detach();
}

/// Presses `keys` in ilo's menu's `window`, as `PREVIEW_MENU` and `PREVIEW_PAGE_MENU` name them.
async fn press_keys(window: gpui::AnyWindowHandle, keys: Vec<String>, cx: &mut gpui::AsyncApp) {
    for key in keys {
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
        let os = gpui::OperatingSystem::Mac;
        let Some(keystroke) = gpui::Keystroke::parse_with_os(&key, os).ok() else {
            continue;
        };
        window
            .update(cx, |_, window, cx| window.dispatch_keystroke(keystroke, cx))
            .ok();
    }
}

fn preview_page_menu(
    shell: gpui::WindowHandle<Shell>,
    bounds: Bounds<Pixels>,
    what: &str,
    cx: &mut App,
) {
    let mut parts = what.split(',').map(str::trim);
    let kind = parts.next().unwrap_or("link").to_string();
    let mut number = || parts.next().and_then(|part| part.parse::<f32>().ok());
    let (Some(x), Some(y)) = (number(), number()) else {
        return;
    };
    let keys: Vec<String> = what
        .split(',')
        .skip(3)
        .map(|key| key.trim().to_string())
        .collect();
    let Some(window) = menu::open_at(Rc::new(PreviewMenu), bounds, cx) else {
        return;
    };
    let window: gpui::AnyWindowHandle = window.into();
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(500))
            .await;
        let at = point(px(x), px(y));
        // In this system's case, as its Chromium writes the menu.
        let items = shell::made_up_page_menu(&kind, cfg!(target_os = "macos"));
        shell
            .update(cx, |shell, _, cx| shell.show_page_menu(1, items, at, cx))
            .ok();
        press_keys(window, keys, cx).await;
    })
    .detach();
}

/// A CLI's Model menu as Settings › Agents opens it with Default chosen, its models as the CLI's
/// recorded answer lists them, in the light or dark Settings.
fn preview_model_menu(bounds: Bounds<Pixels>, cli: &str, cx: &mut App) {
    use browser::agents::ready::{claude_listed, codex_listed};
    let (cli, chosen) = cli.split_once(',').unwrap_or((cli, ""));
    let answer = |text: &str| serde_json::from_str(text).unwrap_or_default();
    let models = match cli {
        "codex" => Some(codex_listed(&answer(include_str!(
            "agents/runner/answers/codex-0.167.1-model-list.json"
        )))),
        _ => claude_listed(&answer(include_str!(
            "agents/runner/answers/claude-2.1.322-initialize.json"
        ))),
    }
    .unwrap_or_default();
    let (_, items) =
        browser::agents::task::model_menu(chosen, models.default.as_deref(), models.listed);
    let light = preview_tone() == Some(Tone::Light);
    if menu::open_at(Rc::new(PreviewMenu), bounds, cx).is_none() {
        return;
    }
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(500))
            .await;
        cx.update(|cx| {
            let request = menu::Request {
                items,
                at: point(px(40.), px(40.)),
                area: Bounds::new(point(px(0.), px(0.)), bounds.size),
                look: menu::Look::plain(light),
                chosen: menu::picked(|_, _| {}),
            };
            menu::show(request, cx);
        });
    })
    .detach();
}

/// ilo's menu over the preview's window, whose host can't move it: its window covers the preview's.
struct PreviewMenu;

impl menu::Host for PreviewMenu {
    fn show(&self, _: Bounds<Pixels>, _: &[Bounds<Pixels>]) {}

    fn hide(&self) {}
}

/// The icon picker over the preview's window, whose host can't move it: it opened where it's to be.
thread_local! {
    /// Where the foot bar asked to go.
    static FOOT_BAR_PLACE: std::cell::Cell<Option<foot_bar::Place>> =
        const { std::cell::Cell::new(None) };
}

struct PreviewFootBar;

impl foot_bar::Host for PreviewFootBar {
    fn show(&self, _: u64, place: foot_bar::Place) {
        FOOT_BAR_PLACE.set(Some(place));
    }

    fn hide(&self) {}
}

/// Shows the foot bar with made-up words over the page of the preview's `window` at `bounds`.
/// The host sizes the bar's window to what it shows, so a hidden one lays it out first, as the
/// host would place it.
fn preview_foot_bar(
    kind: &str,
    window: gpui::WindowHandle<Shell>,
    bounds: Bounds<Pixels>,
    cx: &mut App,
) {
    let Ok((page, palette)) = window.read_with(cx, |shell, _| {
        (shell.page_frame(bounds.size).0, shell.palette())
    }) else {
        return;
    };
    let Some(bar) = foot_bar::sample(kind) else {
        return;
    };
    let look = move |_, _: &App| palette;
    let hidden = Bounds::new(bounds.origin, size(px(1.), px(1.)));
    let Some(measuring) = foot_bar::open_at(Rc::new(PreviewFootBar), look, hidden, false, cx)
    else {
        return;
    };
    foot_bar::show(0, page, bounds.size, bar.clone(), cx);
    let _ = measuring.update(cx, |_, window, _| window.remove_window());
    let Some(place) = FOOT_BAR_PLACE.get() else {
        return;
    };
    let across = f32::from(bounds.size.width) - place.left - place.right;
    let left = place.left + (across - f32::from(place.size.width)) / 2.;
    let top = f32::from(bounds.size.height - place.size.height) - place.bottom;
    let at = Bounds::new(bounds.origin + point(px(left), px(top)), place.size);
    if foot_bar::open_at(Rc::new(PreviewFootBar), look, at, true, cx).is_some() {
        foot_bar::show(0, page, bounds.size, bar, cx);
        log!("preview: foot bar shown");
    }
}

struct PreviewPicker;

impl icon_picker::Host for PreviewPicker {
    fn show(&self, _: u64, _: f32, _: f32) {}

    fn hide(&self) {}

    fn keep_recents(&self, _: &str) {}
}

impl theme_picker::Host for PreviewPicker {
    fn show(&self, _: u64, _: f32, _: f32, _: &[(f32, f32, f32)]) {}

    fn hide(&self) {}

    fn detent(&self) {}
}

impl boost_editor::Host for PreviewPicker {
    fn show(&self, _: u64, _: f32, _: f32, _: &[(f32, f32, f32)]) {}

    // The preview's window is opened at the height it shows at.
    fn resize(&self, _: f32, _: &[(f32, f32, f32)]) {}

    fn hide(&self) {}
}

/// Peek over the preview's window, which it covers. There's no page of its own, so the shell's
/// shows through its card.
struct PreviewPeek;

impl link_preview::Host for PreviewPeek {
    fn place(&self, _: u64, _: Bounds<Pixels>, _: f32) {}

    fn show(&self, _: u64) {}

    fn drew(&self, _: Bounds<Pixels>, _: f32, _: f32) {}

    fn page_under(&self) -> bool {
        true
    }

    fn open_waits(&self) -> bool {
        // No core says when a page is on screen here.
        false
    }

    fn close_waits(&self) -> bool {
        false
    }

    fn release(&self) {}

    fn hide(&self) {}

    fn close(&self, _: u64) {}

    fn keep(&self, _: u64, _: u64, _: Option<u64>) {}

    fn closed(&self, _: u64, _: link_preview::Closed, _: bool, _: &mut App) {}

    // The tooltip's window is the app's, over the page, which the preview has neither of.
    fn tooltip(&self, _: Option<(browser::tooltip::Tip, browser::tooltip::Anchor)>, _: &mut App) {}
}

/// The dialog over the preview's window, which it covers, for its host can't move it.
struct PreviewDialog;

impl dialog::Host for PreviewDialog {
    fn show(&self, _: Option<u64>, _: Option<Bounds<Pixels>>) {}

    fn hide(&self) {}
}

/// The Site Controls over the preview's window, opened where they're to be: their host can't move
/// them, and the made-up site can't change.
struct PreviewSiteControls;

impl site_controls::Host for PreviewSiteControls {
    fn show(&self, _: Point<Pixels>, _: Size<Pixels>, _: &[(f32, f32, f32)]) {}

    fn set_height(&self, _: f32, _: &[(f32, f32, f32)]) {}

    fn drew(&self, _: f32, _: f32, _: Point<Pixels>) {}

    fn release(&self) {}

    fn hide(&self) {}

    fn read(&self, _: u64) {}

    fn stop_reading(&self) {}

    fn set_permission(&self, _: u64, _: i32, _: Value) {}

    fn reset_permissions(&self, _: u64) {}

    fn remove_device(&self, _: u64, _: usize) {}

    fn set_system_engine(&self, _: u64, _: bool) {}

    fn allow_saving(&self, _: u64, _: &browser::passwords::Login) {}

    fn set_ad_blocking(&self, _: u64, _: bool) {}
    fn set_developer_mode(&self, _: u64, _: bool) {}

    fn set_boost(&self, _: u64, _: bool) {}

    fn run(&self, _: site_controls::Run, _: u64, _: &mut App) {}

    fn menu(
        &self,
        _: Vec<MenuEntry>,
        _: Point<Pixels>,
        _: Box<dyn FnOnce(Option<usize>, &mut App)>,
        _: &mut App,
    ) {
    }

    fn tooltip(&self, _: Option<(SharedString, browser::tooltip::Anchor)>, _: &mut App) {}

    fn has_share_sheet(&self) -> bool {
        cfg!(any(target_os = "macos", target_os = "windows"))
    }
}

/// A made-up site, with permissions of each value and one a policy sets, in Chrome's words, the
/// longest of them Pop-ups and redirects.
fn preview_site() -> Site {
    let permission =
        |kind, key: &str, name: &str, value, state: &str, choices: &[Value]| Permission {
            kind,
            key: key.to_string().into(),
            name: name.to_string().into(),
            value,
            state: state.to_string().into(),
            choices: choices.to_vec(),
        };
    // PREVIEW_CONNECTION=not-secure shows a page over plain http, and =internal the browser's own.
    let (site, connection, summary) = match std::env::var("PREVIEW_CONNECTION").as_deref() {
        Ok("not-secure") => ("en.wikipedia.org", Connection::NotSecure, "Not secure"),
        Ok("internal") => ("ilo", Connection::Internal, "ilo"),
        _ => (
            "en.wikipedia.org",
            Connection::Secure,
            "Connection is secure",
        ),
    };
    Site {
        site: site.into(),
        connection,
        connection_summary: summary.into(),
        certificate: connection == Connection::Secure,
        permissions: vec![
            permission(
                5,
                "geolocation",
                "Location",
                Value::Ask,
                "Ask (default)",
                &Value::ALL,
            ),
            permission(
                6,
                "notifications",
                "Notifications",
                Value::Allow,
                "Allowed",
                &Value::ALL,
            ),
            permission(
                4,
                "popups",
                "Pop-ups and redirects",
                Value::Block,
                "Blocked",
                &Value::ALL,
            ),
            permission(2, "javascript", "JavaScript", Value::Allow, "Allowed", &[]),
        ],
        reset: Some("Reset permissions".into()),
        system_engine: None,
        logins: Vec::new(),
        devices: Vec::new(),
        ad_blocking: Some(true),
        ad_blocked: "12".into(),
        developer_mode: Some(false),
        // PREVIEW_BOOST=on or off gives the site a Boost, which shows its row.
        boostable: true,
        boost: match std::env::var("PREVIEW_BOOST").as_deref() {
            Ok("on") => Some(true),
            Ok("off") => Some(false),
            _ => None,
        },
    }
}

/// Settings › Sync alone, on the card as Settings shows a section, in a window of Settings' size.
fn preview_account(state: &str, cx: &mut App) {
    use account::{Device, State, Status};
    use browser::tokens::settings::{HEIGHT, WIDTH};
    account::bind_keys(cx);
    // PREVIEW_SIZE=<width>x<height> as for the browser's window, Settings' own size by default.
    let (width, height) = std::env::var("PREVIEW_SIZE")
        .ok()
        .and_then(|size| {
            let (width, height) = size.split_once('x')?;
            Some((width.parse().ok()?, height.parse().ok()?))
        })
        .unwrap_or((WIDTH, HEIGHT));
    // PREVIEW_SCROLL=<points> scrolls the card that far down once it's drawn.
    let scroll: f32 = std::env::var("PREVIEW_SCROLL")
        .ok()
        .and_then(|points| points.parse().ok())
        .unwrap_or(0.);
    let (mut x, mut y) = (0., 0.);
    unsafe { host_centered(width as f64, height as f64, &mut x, &mut y) };
    let bounds = Bounds::new(
        point(px(x as f32), px(y as f32)),
        size(px(width), px(height)),
    );
    let status = match state {
        "words" => Status::FirstDevice,
        "waiting" => Status::Pairing,
        "signed-in" => Status::On,
        _ => Status::SignedOut,
    };
    let no_server = state == "no-server";
    let state = State {
        status,
        problem: None,
        server: if no_server {
            "".into()
        } else {
            "https://sync.ilo.app".into()
        },
        default_server: !no_server,
        email: "sam@example.com".into(),
        code: if status == Status::Pairing {
            "312568".into()
        } else {
            "".into()
        },
        settings: true,
        extensions: false,
    };
    let device = |id: &str, name: &str, os: &str, current| Device {
        id: id.to_string().into(),
        name: name.to_string().into(),
        os: os.to_string().into(),
        current,
    };
    let devices = vec![
        device("1", "Sam's MacBook Pro", "macOS", true),
        device("2", "MacBook Air", "macOS", false),
        device("3", "ThinkPad X1", "Linux", false),
    ];
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };
    let opened = cx.open_window(options, |window, cx| {
        let page = cx.new(|cx| {
            let dialog = dialog::in_window(Rc::new(PreviewDialog), cx);
            let host = Rc::new(PreviewAccount {
                dialog: dialog.clone(),
            });
            let account = cx.new(|cx| {
                let mut account = Account::new(host, cx);
                account.set_state(state, cx);
                if status == Status::On {
                    account.set_devices(devices, cx);
                }
                account
            });
            let look = browser::settings_rows::Look::settings(true);
            let search = cx.new(|cx| {
                browser::text_input::TextInput::new("Search", cx)
                    .placeholder_color(look.detail)
                    .caret_color(look.ink)
            });
            AccountPage {
                account,
                dialog,
                card: Card::default(),
                search,
            }
        });
        window.focus(&page.read(cx).account.focus_handle(cx), cx);
        if scroll > 0. {
            let scrolled = page.clone();
            window.on_next_frame(move |_, cx| {
                scrolled.update(cx, |page, cx| {
                    page.card.scroll.set_offset(point(px(0.), px(-scroll)));
                    cx.notify();
                });
            });
        }
        page
    });
    if let Err(error) = opened {
        log!("preview: couldn't open the account's window: {error}");
    }
}

/// A Settings group whose row is chosen from drawings, on its card as Settings draws it.
#[derive(Clone, Copy)]
enum Pictures {
    /// Tabs & Sidebar's sidebar group: Sidebar position, `right` chosen, and the two switches
    /// under it.
    Sidebar { right: bool },
    /// General's Look group: Appearance, the one chosen, and Page zoom under it.
    Appearance(browser::theme::Appearance),
    /// Tabs & Sidebar's Share favorites, the scope chosen, in its group of one.
    Favorites(browser::shell::FavoritesScope),
}

/// `shown`'s group in a window of Settings' size. PREVIEW_BUTTONS=mac, windows or linux draws that
/// OS's window buttons in the drawings.
fn preview_pictures(shown: Pictures, cx: &mut App) {
    use browser::settings_pictures::Buttons;
    use browser::tokens::settings::{HEIGHT, WIDTH};
    let buttons = match std::env::var("PREVIEW_BUTTONS").as_deref() {
        Ok("mac") => Buttons::Mac,
        Ok("windows") => Buttons::Windows,
        Ok("linux") => Buttons::Linux,
        _ => Buttons::HERE,
    };
    let (mut x, mut y) = (0., 0.);
    unsafe { host_centered(WIDTH as f64, HEIGHT as f64, &mut x, &mut y) };
    let bounds = Bounds::new(
        point(px(x as f32), px(y as f32)),
        size(px(WIDTH), px(HEIGHT)),
    );
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    };
    let opened = cx.open_window(options, |_, cx| {
        cx.new(|cx| {
            let look = browser::settings_rows::Look::settings(true);
            let search = cx.new(|cx| {
                browser::text_input::TextInput::new("Search", cx)
                    .placeholder_color(look.detail)
                    .caret_color(look.ink)
            });
            PicturesPage {
                shown,
                buttons,
                card: Card::default(),
                search,
                logged: Rc::default(),
            }
        })
    });
    if let Err(error) = opened {
        log!("preview: couldn't open the pictures' window: {error}");
    }
}

struct PicturesPage {
    shown: Pictures,
    buttons: browser::settings_pictures::Buttons,
    card: Card,
    search: gpui::Entity<browser::text_input::TextInput>,
    logged: Rc<std::cell::RefCell<Vec<String>>>,
}

impl gpui::Render for PicturesPage {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use browser::palette::{INK, PAPER};
        use browser::settings_card::Colors;
        use browser::settings_column::{self, Section};
        use browser::settings_pictures::{appearance_picker, favorites_picker, picker};
        use browser::settings_rows::{
            Look, end, frame, group, popup, stop_switch, text as row_text,
        };
        use browser::theme::Appearance;
        use browser::tokens::palette::{LIGHT_STAND_IN, STAND_IN};
        use browser::tokens::settings::{
            HEADING_BOTTOM, NAME, POPUP_WIDTH, SIDE_ROOM, SIDE_TOP, TEXT_X,
        };
        use browser::tokens::text;
        use gpui::{FontWeight, ParentElement, Styled, div, rgb};
        // Settings takes Light's or Dark's look when it's chosen, and the system's for Automatic.
        let dark = match self.shown {
            Pictures::Appearance(Appearance::Light) => false,
            Pictures::Appearance(Appearance::Dark) => true,
            _ => preview_tone() != Some(Tone::Light),
        };
        let look = Look::settings(dark);
        let base = if dark { rgb(PAPER) } else { rgb(INK) };
        let a11y = window.is_a11y_active();
        let field = browser::settings_search::field(
            &self.search,
            base,
            gpui::canvas(|_, _, _| {}, |_, _, _, _| {}),
            |_, _| {},
            cx,
        );
        let section = match self.shown {
            Pictures::Sidebar { .. } | Pictures::Favorites(_) => Section::TabsAndSidebar,
            Pictures::Appearance(_) => Section::General,
        };
        let column = settings_column::column(
            &Default::default(),
            section,
            dark,
            Rc::default(),
            gpui::IntoElement::into_any_element(field),
            None,
            Rc::new(|_, _, _| {}),
            Rc::new(|_, _| {}),
            a11y,
        );
        let page = cx.entity().downgrade();
        let (heading, rows) = match self.shown {
            Pictures::Sidebar { right } => {
                let pair = picker(
                    &look,
                    base,
                    dark,
                    right,
                    self.buttons,
                    a11y,
                    self.logged.clone(),
                    move |side, _, cx| {
                        log!(
                            "preview: sidebar position {}",
                            browser::settings_pictures::title(side)
                        );
                        page.update(cx, |page, cx| {
                            page.shown = Pictures::Sidebar { right: side };
                            cx.notify();
                        })
                        .ok();
                    },
                );
                let position = frame()
                    .flex_col()
                    .items_stretch()
                    .child(row_text(
                        &look,
                        "Sidebar position",
                        Some(
                            "The side of the window the sidebar and the window's buttons are on."
                                .into(),
                        ),
                    ))
                    .child(div().flex().mt(px(SIDE_TOP)).ml(px(-SIDE_ROOM)).child(pair));
                let switch = |name: &'static str, on: bool| {
                    end([gpui::IntoElement::into_any_element(stop_switch(
                        &look,
                        name,
                        name,
                        on,
                        1.,
                        look.accent,
                        a11y,
                    ))])
                };
                let side = if right { "right" } else { "left" };
                let edge = frame()
                    .child(row_text(
                        &look,
                        "Show the hidden sidebar at the window's edge",
                        Some(
                            format!(
                                "Move the pointer along the {side} edge. ⌘S shows it either way."
                            )
                            .into(),
                        ),
                    ))
                    .child(switch("edge", true));
                let option = frame()
                    .child(row_text(
                        &look,
                        "Show the hidden sidebar when ⌥ is pressed alone",
                        Some("Press and let go of ⌥ with no other key.".into()),
                    ))
                    .child(switch("option", false));
                (Some("The sidebar"), vec![position, edge, option])
            }
            Pictures::Appearance(chosen) => {
                let three = appearance_picker(
                    &look,
                    base,
                    chosen,
                    self.buttons,
                    a11y,
                    self.logged.clone(),
                    move |appearance, _, cx| {
                        log!("preview: appearance {appearance:?}");
                        page.update(cx, |page, cx| {
                            page.shown = Pictures::Appearance(appearance);
                            cx.notify();
                        })
                        .ok();
                    },
                );
                let line = match self.buttons {
                    browser::settings_pictures::Buttons::Mac => {
                        "Light, dark, or as macOS is. View › Appearance has it too."
                    }
                    _ => "Light, dark, or as the system is.",
                };
                let appearance = frame()
                    .flex_col()
                    .items_stretch()
                    .child(row_text(&look, "Appearance", Some(line.into())))
                    .child(
                        div()
                            .flex()
                            .mt(px(SIDE_TOP))
                            .ml(px(-SIDE_ROOM))
                            .child(three),
                    );
                let zoom = frame()
                    .child(row_text(
                        &look,
                        "Page zoom",
                        Some(
                            "Every page's size, until you zoom a site, which keeps its own.".into(),
                        ),
                    ))
                    .child(end([gpui::IntoElement::into_any_element(popup(
                        &look,
                        "100%",
                        POPUP_WIDTH,
                        false,
                    ))]));
                (Some("Look"), vec![appearance, zoom])
            }
            Pictures::Favorites(chosen) => {
                let three = favorites_picker(
                    &look,
                    base,
                    dark,
                    chosen,
                    a11y,
                    self.logged.clone(),
                    move |scope, _, cx| {
                        log!("preview: share favorites {}", scope.title());
                        page.update(cx, |page, cx| {
                            page.shown = Pictures::Favorites(scope);
                            cx.notify();
                        })
                        .ok();
                    },
                );
                let shared = frame()
                    .flex_col()
                    .items_stretch()
                    .child(row_text(
                        &look,
                        "Share favorites",
                        Some("Which Spaces show the same favorites.".into()),
                    ))
                    .child(
                        div()
                            .flex()
                            .mt(px(SIDE_TOP))
                            .ml(px(-SIDE_ROOM))
                            .child(three),
                    );
                (None, vec![shared])
            }
        };
        let heading = heading.map(|heading| {
            div()
                .px(px(TEXT_X))
                .mb(px(HEADING_BOTTOM))
                .font_weight(FontWeight::MEDIUM)
                .text_color(look.heading)
                .child(heading)
        });
        let colors = Colors {
            line: look.line,
            name: base.opacity(NAME),
            heading: look.heading,
        };
        let card = self.card.draw(
            section.name(),
            div()
                .flex()
                .flex_col()
                .children(heading)
                .child(group(&look, rows)),
            dark,
            colors,
        );
        div()
            .relative()
            .size_full()
            .flex()
            .bg(rgb(if dark { STAND_IN } else { LIGHT_STAND_IN }))
            .font_family(browser::face::UI)
            .text_size(px(text::BODY))
            .text_color(look.ink)
            .child(column)
            .child(card)
    }
}

/// The account's view on Settings' card, beside Settings' column with Sync lit, with the dialog
/// it asks in over all of it, as Settings has on Linux.
struct AccountPage {
    account: gpui::Entity<Account>,
    dialog: gpui::Entity<dialog::Dialog>,
    card: Card,
    /// Settings' search field, at the column's top.
    search: gpui::Entity<browser::text_input::TextInput>,
}

impl gpui::Render for AccountPage {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use browser::palette::{INK, PAPER};
        use browser::settings_card::Colors;
        use browser::settings_column::{self, Section};
        use browser::settings_rows::Look;
        use browser::tokens::palette::{LIGHT_STAND_IN, STAND_IN};
        use browser::tokens::settings::NAME;
        use browser::tokens::text;
        use gpui::{ParentElement, Styled, div, rgb};
        // Dark, as Settings is in a dark system, unless PREVIEW_APPEARANCE=light.
        let dark = preview_tone() != Some(Tone::Light);
        let look = Look::settings(dark);
        self.account
            .update(cx, |account, cx| account.set_look(look, cx));
        let field = browser::settings_search::field(
            &self.search,
            if dark { rgb(PAPER) } else { rgb(INK) },
            gpui::canvas(|_, _, _| {}, |_, _, _, _| {}),
            |_, _| {},
            cx,
        );
        let column = settings_column::column(
            &Default::default(),
            Section::Sync,
            dark,
            Rc::default(),
            gpui::IntoElement::into_any_element(field),
            None,
            Rc::new(|_, _, _| {}),
            Rc::new(|_, _| {}),
            window.is_a11y_active(),
        );
        let base = if dark { rgb(PAPER) } else { rgb(INK) };
        let colors = Colors {
            line: look.line,
            name: base.opacity(NAME),
            heading: look.heading,
        };
        let card = self.card.draw(
            Section::Sync.name(),
            div().child(self.account.clone()),
            dark,
            colors,
        );
        div()
            .relative()
            .size_full()
            .flex()
            .bg(rgb(if dark { STAND_IN } else { LIGHT_STAND_IN }))
            .font_family(browser::face::UI)
            .text_size(px(text::BODY))
            .text_color(look.ink)
            .child(column)
            .child(card)
            .children(dialog::cover(&self.dialog, cx))
    }
}

/// A made-up account: its recovery words are made-up words, and it asks nothing of a server. It
/// asks the user in its window's dialog.
struct PreviewAccount {
    dialog: gpui::Entity<dialog::Dialog>,
}

impl account::Host for PreviewAccount {
    fn set_server(&self, _: &str) -> bool {
        true
    }
    fn sign_in(&self) {}
    fn cancel(&self) {}
    fn sign_out(&self) {}
    fn recovery_words(&self) -> String {
        "harbor velvet nominee orbit quarter skill tumble canyon effort glide lunar prize \
         answer bridge cotton dwarf frozen hollow island jacket kitten ladder mango noble"
            .into()
    }
    fn enter_recovery_words(&self, _: &str) -> bool {
        true
    }
    fn start_over(&self) {}
    fn request_pairing(&self) {}
    fn read_devices(&self) {}
    fn remove_device(&self, _: &str) {}
    fn choose(&self, _: account::Choice, _: bool) {}
    // PREVIEW_SYNC=never, unreachable, throttled, stopped or update shows that line, else synced at
    // 15:04.
    fn activity(&self) -> account::Activity {
        let shown = std::env::var("PREVIEW_SYNC").unwrap_or_default();
        let trouble = match shown.as_str() {
            "unreachable" => Some(account::Trouble::Unreachable),
            "throttled" => Some(account::Trouble::Throttled),
            "stopped" => Some(account::Trouble::Stopped),
            "update" => Some(account::Trouble::Update),
            _ => None,
        };
        let synced = if shown == "never" { "" } else { "at 15:04" };
        account::Activity {
            synced: synced.into(),
            trouble,
        }
    }
    fn delete_account(&self) {}
    fn copy_secret(&self, _: &str, _: Duration, _: &mut App) {}
    fn confirm(
        &self,
        message: String,
        detail: String,
        action: &'static str,
        on_return: dialog::Return,
        confirmed: Box<dyn FnOnce(&mut App)>,
        window: &mut gpui::Window,
        cx: &mut App,
    ) {
        dialog::ask_in(
            &self.dialog,
            message,
            detail,
            action,
            on_return,
            confirmed,
            window,
            cx,
        );
    }
}

/// An updater that does nothing, at the stage PREVIEW_UPDATE names.
fn preview_update(update: &str, cx: &mut App) {
    use browser::updates::{self, Stage, Track, Updater};
    struct Made;
    impl updates::Host for Made {
        fn check(&self, _: &mut App) {}
        fn show(&self, _: &mut App) {}
        fn restart(&self, _: &mut App) {}
        fn follow(&self, _: Track, _: &mut App) {}
    }
    updates::start(Updater::Own, std::rc::Rc::new(Made), cx);
    let off = std::env::var("PREVIEW_UPDATE_CHECKS").is_ok_and(|checks| checks == "off");
    updates::checks(Some(!off), cx);
    let version = Some("0.1.5".into());
    let stage = match update.trim_end_matches("-open") {
        "available" => Stage::Available(version),
        _ => Stage::Ready(version),
    };
    updates::changed(stage, cx);
}

/// The site's live demo: the browser's UI in the canvas the site gives it, with the site's Spaces,
/// over the site's frame, which the site lays out where the page goes and shows the selected tab
/// in. Its JS:
///
/// ```js
/// import init, { start } from "./ilo_demo.js";
/// await init();
/// start(canvas, spaces, host);
/// ```
///
/// `spaces` is `{ favorites: [tab], spaces: [space] }`, a Space `{ name, color, pinned: [tab],
/// today: [tab], selected }` and a tab `{ title, url, icon }`: `color` is 0xRRGGBB or none, `icon`
/// a PNG's URL or none for the globe, and `selected` the URL of the tab the Space opens on, or none
/// for its first Today tab, else its first pinned one. `host` hears `layOut(x, y, width, height,
/// radius)` for the page's frame, `show(url)` for the tab shown ("" for none), `space(index)` for
/// the Space shown, `cover(covered)` while something draws over the frame, and `ready()` once the
/// first frame is up. `host.scheme`, "dark" or "light", keeps the demo in that appearance; without
/// it the demo follows the visitor's, as the app follows the system's. The demo takes the keyboard
/// only once it's clicked or its command bar opens, and gives it back when the bar closes;
/// `openBar()` opens the bar.
#[cfg(target_family = "wasm")]
mod web {
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
        sync::Arc,
    };

    use browser::command_bar::{self, CommandBar, Suggestion};
    use browser::log;
    use browser::palette::Tone;
    use browser::shell::{self, Command, Folders, Page, Shell, SpaceEntry};
    use gpui::{
        AnyWindowHandle, App, AppContext, AsyncApp, Bounds, Context, Entity, Focusable, Image,
        ImageFormat, KeyBinding, MouseButton, NoAction, OperatingSystem, Pixels, Render,
        SharedString, Subscription, WeakEntity, Window, WindowBackgroundAppearance, WindowOptions,
        actions, div, prelude::*, px,
    };
    use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};

    use super::{Fake, set_today};

    actions!(web, [OpenCommandBar]);

    thread_local! {
        /// The site's `host`.
        static HOST: RefCell<JsValue> = const { RefCell::new(JsValue::UNDEFINED) };
        /// The running demo's app and shell, for the calls the site makes into it.
        static DEMO: RefCell<Option<(AsyncApp, WeakEntity<Shell>)>> = const { RefCell::new(None) };
    }

    /// Calls `host.<name>(…args)`, if the site has it.
    fn call(name: &str, args: &[JsValue]) {
        HOST.with_borrow(|host| {
            let function = js_sys::Reflect::get(host, &name.into()).ok();
            let Some(function) = function
                .as_ref()
                .and_then(|f| f.dyn_ref::<js_sys::Function>())
            else {
                return;
            };
            let args: js_sys::Array = args.iter().collect();
            if let Err(error) = function.apply(host, &args) {
                log!("host.{name} threw {error:?}");
            }
        });
    }

    /// What draws over the site's frame.
    #[derive(Clone, Copy)]
    pub enum Over {
        Bar,
        Reveal,
    }

    thread_local! {
        static OVER: Cell<[bool; 2]> = const { Cell::new([false; 2]) };
    }

    /// `over` draws over the site's frame, or has gone: the site hears `cover` as the first comes
    /// and the last goes.
    pub fn cover(over: Over, covered: bool) {
        let mut all = OVER.get();
        let was = all.contains(&true);
        all[over as usize] = covered;
        OVER.set(all);
        if all.contains(&true) != was {
            call("cover", &[JsValue::from_bool(!was)]);
        }
    }

    pub fn lay_out(frame: Bounds<Pixels>, radius: f32) {
        let (origin, size) = (frame.origin, frame.size);
        // The shell lays out once before the canvas gives the window its size, which leaves the
        // frame inside out; the site gets an empty frame there instead.
        let size = size.max(&gpui::Size::default());
        let numbers = [origin.x, origin.y, size.width, size.height].map(f32::from);
        let args = numbers
            .into_iter()
            .chain([radius])
            .map(|n| JsValue::from(n as f64));
        call("layOut", &args.collect::<Vec<_>>());
    }

    /// One tab the site gave.
    struct Tab {
        title: String,
        url: String,
        icon: Option<String>,
    }

    struct SiteSpace {
        name: String,
        color: Option<u32>,
        pinned: Vec<Tab>,
        today: Vec<Tab>,
        /// The URL of the tab the Space opens on, if the site names one.
        selected: Option<String>,
    }

    fn get(object: &JsValue, key: &str) -> JsValue {
        js_sys::Reflect::get(object, &key.into()).unwrap_or(JsValue::UNDEFINED)
    }

    fn list(object: &JsValue, key: &str) -> Vec<JsValue> {
        let list = get(object, key);
        match js_sys::Array::is_array(&list) {
            true => js_sys::Array::from(&list).iter().collect(),
            false => Vec::new(),
        }
    }

    fn tabs(object: &JsValue, key: &str) -> Vec<Tab> {
        let tab = |tab: JsValue| Tab {
            title: get(&tab, "title").as_string().unwrap_or_default(),
            url: get(&tab, "url").as_string().unwrap_or_default(),
            icon: get(&tab, "icon")
                .as_string()
                .filter(|icon| !icon.is_empty()),
        };
        list(object, key).into_iter().map(tab).collect()
    }

    /// Starts the demo in `canvas`, with the Spaces in `spaces`, telling `host` what it does.
    #[wasm_bindgen]
    pub fn start(canvas: web_sys::HtmlCanvasElement, spaces: JsValue, host: JsValue) {
        let tone = match get(&host, "scheme").as_string().as_deref() {
            Some("dark") => Some(Tone::Dark),
            Some("light") => Some(Tone::Light),
            _ => None,
        };
        HOST.with_borrow_mut(|site| *site = host);
        let favorites = tabs(&spaces, "favorites");
        let spaces: Vec<SiteSpace> = list(&spaces, "spaces")
            .into_iter()
            .map(|space| SiteSpace {
                name: get(&space, "name").as_string().unwrap_or_default(),
                color: get(&space, "color").as_f64().map(|color| color as u32),
                pinned: tabs(&space, "pinned"),
                today: tabs(&space, "today"),
                selected: get(&space, "selected").as_string(),
            })
            .collect();
        let notes = trace(&canvas);
        gpui_web::draw_in(canvas);
        gpui_platform::web_init();
        gpui_platform::single_threaded_web()
            .with_assets(shell::Assets)
            .run(move |cx| {
                browser::face::register(cx);
                bind_keys(cx);
                shell::bind_keys(cx);
                browser::text_input::bind_keys(cx);
                // The frame under the page's rect shows through.
                let options = WindowOptions {
                    window_background: WindowBackgroundAppearance::Transparent,
                    ..Default::default()
                };
                let opened = cx.open_window(options, |window, cx| {
                    let mut icons = Vec::new();
                    let mut space_ids = Vec::new();
                    let mut pages = Vec::new();
                    let shell = cx.new(|cx| {
                        let mut shell =
                            shell_of(&favorites, &spaces, &mut icons, &mut space_ids, cx);
                        shell.force_tone(tone);
                        if let Some(notes) = &notes {
                            shell.note_wheel(notes.clone());
                        }
                        pages = favorites
                            .iter()
                            .chain(
                                spaces
                                    .iter()
                                    .flat_map(|space| space.pinned.iter().chain(&space.today)),
                            )
                            .map(|tab| (SharedString::from(tab.title.clone()), tab.url.clone()))
                            .collect();
                        shell
                    });
                    window.focus(&shell.focus_handle(cx), cx);
                    DEMO.with_borrow_mut(|demo| *demo = Some((cx.to_async(), shell.downgrade())));
                    fetch_icons(&shell, icons, cx);
                    Root::new(shell, space_ids, pages, window, cx)
                });
                if let Err(error) = opened {
                    log!("couldn't open the demo: {error:#}");
                }
            });
    }

    /// With `?trace` on the page, records the wheel events over the demo, when the trackpad gave each
    /// and when the page heard it, the page's frames while they come, and what the demo decided of
    /// each swipe from them, and puts a button on the page that saves them as JSON: a real swipe,
    /// to check what `shell::swipe`'s model assumes of a trackpad and a browser against, and to
    /// replay through the model (`replay_wheel_trace`). Gives the shell what to tell its decisions.
    fn trace(canvas: &web_sys::HtmlCanvasElement) -> Option<shell::WheelNotes> {
        // Version 2 added `frames`, `decisions` and each event's `demo`; version 1 had no version.
        const RECORD: &str = r#"
            if (!new URLSearchParams(location.search).has("trace")) return;
            const events = [], frames = [], decisions = [];
            let heard = -Infinity, framing = false;
            const button = document.createElement("button");
            const label = () => { button.textContent = `Save wheel trace (${events.length} events)`; };
            // Frames only for a second after the wheel's last event, so an idle page stays idle.
            const frame = (now) => {
                frames.push(now);
                framing = performance.now() - heard < 1000;
                if (framing) requestAnimationFrame(frame);
            };
            canvas.addEventListener("wheel", (event) => {
                const { timeStamp: stamp, deltaX: x, deltaY: y, deltaMode: mode } = event;
                heard = performance.now();
                events.push({ stamp, heard, x, y, mode });
                if (!framing) {
                    framing = true;
                    requestAnimationFrame(frame);
                }
                label();
            }, { capture: true, passive: true });
            Object.assign(button.style, { position: "fixed", right: "12px", bottom: "12px",
                zIndex: "2147483647", font: "13px system-ui, sans-serif", padding: "6px 10px" });
            button.onclick = () => {
                const trace = { version: 2, userAgent: navigator.userAgent, devicePixelRatio,
                    events, frames, decisions };
                const link = document.createElement("a");
                link.href = URL.createObjectURL(new Blob([JSON.stringify(trace)], { type: "application/json" }));
                link.download = "ilo-wheel-trace.json";
                link.click();
                events.length = frames.length = decisions.length = 0;
                label();
            };
            label();
            document.body.append(button);
            // The demo hears each event after this listener, so its decisions are the last event's,
            // or with no event, its quiet timer's. "wheel" marks an event it took as a swipe's.
            return (kind, what) => {
                const event = events.length - 1;
                if (kind !== "wheel") decisions.push({ event, heard: performance.now(), kind, what });
                else if (event >= 0) events[event].demo = true;
            };
        "#;
        let record = js_sys::Function::new_with_args("canvas", RECORD);
        let note = match record.call1(&JsValue::NULL, canvas) {
            Ok(note) => note.dyn_into::<js_sys::Function>().ok()?,
            Err(error) => {
                log!("couldn't record wheel events: {error:?}");
                return None;
            }
        };
        Some(Rc::new(move |kind: &'static str, what: &str| {
            let (kind, what) = (JsValue::from_str(kind), JsValue::from_str(what));
            note.call2(&JsValue::NULL, &kind, &what).ok();
        }))
    }

    /// Opens the command bar and gives it the keyboard, as ⌘K (⌃K) in the demo does: the site calls
    /// it for ⌘K anywhere on its page, since the page's keys are the site's until the demo's clicked.
    #[wasm_bindgen(js_name = openBar)]
    pub fn open_bar() {
        let Some((app, shell)) = DEMO.with_borrow(Clone::clone) else {
            return;
        };
        app.update(|cx| shell.update(cx, |shell, cx| shell.run(Command::NewTab, "site", cx)))
            .ok();
    }

    /// The shell with the site's tabs, in one profile, the first Space shown on the tab it opens on.
    /// Fills `icons` with each tab's icon to fetch, and `space_ids` with the Spaces' ids in order.
    fn shell_of(
        favorites: &[Tab],
        spaces: &[SiteSpace],
        icons: &mut Vec<(u64, String)>,
        space_ids: &mut Vec<u64>,
        cx: &mut Context<Shell>,
    ) -> Shell {
        let mut next = 1;
        let mut ids = |count: usize| {
            let ids: Vec<u64> = (next..).take(count).collect();
            next += count as u64;
            ids
        };
        let favorite_ids = ids(favorites.len());
        let space_tabs: Vec<(u64, Vec<u64>, Vec<u64>)> = spaces
            .iter()
            .map(|space| (ids(1)[0], ids(space.pinned.len()), ids(space.today.len())))
            .collect();
        let entries: Vec<SpaceEntry> = spaces
            .iter()
            .zip(&space_tabs)
            .map(|(space, (id, ..))| {
                (
                    *id,
                    space.name.clone().into(),
                    space.color,
                    0,
                    "".into(),
                    "".into(),
                    "".into(),
                )
            })
            .collect();
        let browser = Rc::new(Fake {
            shell: cx.weak_entity(),
            next_id: Rc::new(Cell::new(next)),
            spaces: RefCell::new(entries.clone()),
            archive: RefCell::new(Vec::new()),
            downloads: RefCell::new(Vec::new()),
            opt_ins: Cell::new((false, false)),
            opens_on: RefCell::default(),
            shows: Rc::default(),
        });
        let mut shell = Shell::new(browser.clone(), cx);
        shell.set_profiles(vec![(0, "Default".into())], cx);
        shell.set_spaces(entries, cx);
        let Some(&(first, ..)) = space_tabs.first() else {
            return shell;
        };
        shell.set_space(first, cx);
        // The shell forgets what it knows of a tab in no list, so the lists come first.
        shell.set_favorites(favorite_ids.iter().map(|&tab| (tab, 0)).collect(), cx);
        for ((id, pinned, today), space) in space_tabs.iter().zip(spaces) {
            let none = (Vec::new(), Folders::default());
            shell.set_tabs(*id, pinned.clone(), today.clone(), none.0, none.1, cx);
            space_ids.push(*id);
            let tabs = pinned
                .iter()
                .chain(today)
                .zip(space.pinned.iter().chain(&space.today));
            for (&tab, site) in tabs {
                set_tab(&mut shell, tab, site, icons, cx);
                if space.selected.as_ref() == Some(&site.url) {
                    browser.opens_on.borrow_mut().insert(*id, tab);
                }
            }
        }
        for (&tab, site) in favorite_ids.iter().zip(favorites) {
            set_tab(&mut shell, tab, site, icons, cx);
        }
        shell.set_space(first, cx);
        let named = browser.opens_on.borrow().get(&first).copied();
        let tab = named.unwrap_or_else(|| super::opening_tab(&shell));
        shell.set_selected(tab, cx);
        shell
    }

    fn set_tab(
        shell: &mut Shell,
        tab: u64,
        site: &Tab,
        icons: &mut Vec<(u64, String)>,
        cx: &mut Context<Shell>,
    ) {
        shell.set_title(tab, site.title.clone().into(), cx);
        shell.set_page(
            tab,
            Page {
                url: site.url.clone().into(),
                ..Default::default()
            },
            cx,
        );
        if let Some(icon) = &site.icon {
            icons.push((tab, icon.clone()));
        }
    }

    /// Fetches the tabs' icons from the site, each its tab's favicon once it comes.
    fn fetch_icons(shell: &Entity<Shell>, icons: Vec<(u64, String)>, cx: &mut App) {
        let shell = shell.downgrade();
        for (tab, url) in icons {
            let shell = shell.clone();
            cx.spawn(async move |cx| {
                let Some(png) = fetch(&url).await else {
                    log!("couldn't fetch the icon {url}");
                    return;
                };
                let favicon = Arc::new(Image::from_bytes(ImageFormat::Png, png));
                shell
                    .update(cx, |shell, cx| shell.set_favicon(tab, Some(favicon), cx))
                    .ok();
            })
            .detach();
        }
    }

    async fn fetch(url: &str) -> Option<Vec<u8>> {
        use wasm_bindgen_futures::JsFuture;
        let response = JsFuture::from(web_sys::window()?.fetch_with_str(url))
            .await
            .ok()?;
        let response: web_sys::Response = response.dyn_into().ok()?;
        if !response.ok() {
            return None;
        }
        let bytes = JsFuture::from(response.array_buffer().ok()?).await.ok()?;
        Some(js_sys::Uint8Array::new(&bytes).to_vec())
    }

    /// The hosting browser keeps ⌘T, ⌘W, ⌘N and ⌘Q (⌃ off the Mac), so they do nothing here, and
    /// ⌘K (⌃K) opens the command bar, as the web's command palettes do.
    fn bind_keys(cx: &mut App) {
        let (open, reserved) = match cx.platform_info().os {
            OperatingSystem::Mac => ("cmd-k", ["cmd-t", "cmd-w", "cmd-n", "cmd-q"]),
            _ => ("ctrl-k", ["ctrl-t", "ctrl-w", "ctrl-n", "ctrl-q"]),
        };
        let mut bindings = vec![KeyBinding::new(
            open,
            OpenCommandBar,
            Some(shell::KEY_CONTEXT),
        )];
        for key in reserved {
            for context in [shell::KEY_CONTEXT, "CommandBar"] {
                bindings.push(KeyBinding::new(key, NoAction, Some(context)));
            }
        }
        cx.bind_keys(bindings);
    }

    /// Where the bar shows, as `command_bar::Host::show` asks.
    #[derive(Clone, Copy)]
    struct Place {
        centered: bool,
        left: f32,
        top: f32,
        width: f32,
        radius: f32,
    }

    /// The web's one window: the shell, and over it the command bar, which is a window of its own
    /// in the app.
    pub struct Root {
        shell: Entity<Shell>,
        bar: Entity<CommandBar>,
        place: Option<Place>,
        height: f32,
        /// The Spaces' ids, in the site's order.
        spaces: Vec<u64>,
        /// What the site last heard: the URL shown and the Space's index.
        shown: Option<(SharedString, Option<usize>)>,
        drawn: bool,
        _shell: Subscription,
    }

    impl Root {
        fn new(
            shell: Entity<Shell>,
            spaces: Vec<u64>,
            pages: Vec<(SharedString, String)>,
            window: &mut Window,
            cx: &mut App,
        ) -> Entity<Root> {
            let handle = window.window_handle();
            cx.new(|cx| {
                let host = Rc::new(Bar {
                    root: cx.weak_entity(),
                    window: handle,
                    app: cx.to_async(),
                    shell: shell.downgrade(),
                    pages,
                    next_tab: Cell::new(1_000_000),
                });
                let bar = command_bar::open_in(host, window, cx);
                let watch = cx.observe(&shell, |root: &mut Root, shell, cx| {
                    root.tell_site(&shell, cx)
                });
                let mut root = Root {
                    shell: shell.clone(),
                    bar,
                    place: None,
                    height: command_bar::height(0),
                    spaces,
                    shown: None,
                    drawn: false,
                    _shell: watch,
                };
                root.tell_site(&shell, cx);
                root
            })
        }

        /// Tells the site the page and the Space shown, when either changes.
        fn tell_site(&mut self, shell: &Entity<Shell>, cx: &App) {
            let shell = shell.read(cx);
            let selected = shell.selected();
            let url = match selected {
                0 => SharedString::default(),
                tab => shell.page(tab).url,
            };
            let space = self.spaces.iter().position(|&id| id == shell.space());
            let shown = Some((url.clone(), space));
            if shown == self.shown {
                return;
            }
            let (last_url, last_space) = self.shown.take().unzip();
            if last_url.as_ref() != Some(&url) {
                call("show", &[JsValue::from_str(&url)]);
            }
            if let (Some(index), true) = (space, last_space != Some(space)) {
                call("space", &[JsValue::from(index as f64)]);
            }
            self.shown = shown;
        }

        fn show(&mut self, place: Place, window: &mut Window, cx: &mut Context<Self>) {
            cover(Over::Bar, true);
            self.place = Some(place);
            if let Some(input) = command_bar::input_focus(cx) {
                window.focus(&input, cx);
            }
            cx.notify();
        }

        /// The bar's gone, `dismissed` if the user left it without choosing.
        fn close(&mut self, dismissed: bool, window: &mut Window, cx: &mut Context<Self>) {
            self.place = None;
            cover(Over::Bar, false);
            if dismissed {
                // The bar draws in this window, which is mid-update.
                cx.defer(command_bar::dismissed);
                self.shell
                    .update(cx, |shell, cx| shell.command_bar_dismissed(cx));
            }
            window.focus(&self.shell.focus_handle(cx), cx);
            // The site's keys were its own until the bar took them, so they go back with it; a
            // click that closed it in the demo takes them again.
            gpui_web::release_keyboard();
            cx.notify();
        }
    }

    impl Render for Root {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            if !std::mem::replace(&mut self.drawn, true) {
                window.on_next_frame(|_, _| call("ready", &[]));
            }
            let root = div()
                .size_full()
                .relative()
                .on_action(cx.listener(|root, _: &OpenCommandBar, _, cx| {
                    root.shell
                        .update(cx, |shell, cx| shell.run(Command::NewTab, "web", cx));
                }))
                .child(self.shell.clone());
            let Some(place) = self.place else {
                return root;
            };
            let viewport = window.viewport_size();
            let (left, top) = match place.centered {
                true => (
                    (viewport.width - px(place.width)) / 2.,
                    viewport.height / 2. - px(place.top),
                ),
                false => (px(place.left), px(place.top)),
            };
            // A click anywhere but the bar leaves it, as one outside its window does in the app.
            let outside = div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|root, _, window, cx| root.close(true, window, cx)),
                );
            let bar = div()
                .absolute()
                .left(left)
                .top(top)
                .w(px(place.width))
                .h(px(self.height))
                .rounded(px(place.radius))
                .shadow_lg()
                .occlude()
                .child(self.bar.clone());
            root.child(outside).child(bar)
        }
    }

    /// The command bar's requests: the site's pages, and a search for anything typed.
    struct Bar {
        root: WeakEntity<Root>,
        window: AnyWindowHandle,
        app: AsyncApp,
        shell: WeakEntity<Shell>,
        /// The site's tabs, by title and URL, which the bar suggests.
        pages: Vec<(SharedString, String)>,
        next_tab: Cell<u64>,
    }

    impl Bar {
        /// Runs `change` once the bar's update is over, as a host's reply comes after.
        fn later(&self, change: impl FnOnce(&mut App) + 'static) {
            let app = self.app.clone();
            self.app
                .foreground_executor()
                .spawn(async move { app.update(change) })
                .detach();
        }

        fn later_root(
            &self,
            change: impl FnOnce(&mut Root, &mut Window, &mut Context<Root>) + 'static,
        ) {
            let (root, window) = (self.root.clone(), self.window);
            self.later(move |cx| {
                window
                    .update(cx, |_, window, cx| {
                        root.update(cx, |root, cx| change(root, window, cx))
                    })
                    .ok();
            });
        }

        fn later_shell(&self, change: impl FnOnce(&mut Shell, &mut Context<Shell>) + 'static) {
            let shell = self.shell.clone();
            self.later(move |cx| _ = shell.update(cx, change));
        }
    }

    /// A page's title until it has its own: its host.
    fn title(url: &str) -> SharedString {
        let host = url.split_once("://").map_or(url, |(_, rest)| rest);
        host.split('/').next().unwrap_or(host).to_string().into()
    }

    impl command_bar::Host for Bar {
        fn suggest(&self, _: Option<&str>, text: &str, _: bool) {
            let text = text.to_string();
            let search = self.search_url(None, &text).filter(|_| !text.is_empty());
            let typed = text.to_lowercase();
            let matching = self.pages.iter().filter(|(title, url)| {
                !typed.is_empty() && (title.to_lowercase().contains(&typed) || url.contains(&typed))
            });
            let mut rows: Vec<Suggestion> = search
                .map(|url| Suggestion {
                    url,
                    text: text.clone().into(),
                    ..Default::default()
                })
                .into_iter()
                .collect();
            rows.extend(matching.map(|(text, url)| Suggestion {
                url: url.clone(),
                text: text.clone(),
                detail: title(url),
                ..Default::default()
            }));
            self.later(move |cx| command_bar::suggested(text, rows, cx));
        }
        fn remove_suggestion(&self, _: &str, _: &mut App) {}
        fn step_removal(&self, _: bool, _: &mut App) -> bool {
            false
        }
        fn typed_url(&self, text: &str) -> Option<String> {
            match text.contains('.') && !text.contains(' ') {
                true if text.contains("://") => Some(text.to_string()),
                true => Some(format!("https://{text}")),
                false => self.search_url(None, text),
            }
        }
        fn search_url(&self, _: Option<&str>, text: &str) -> Option<String> {
            let query = String::from(js_sys::encode_uri_component(text));
            Some(format!("https://duckduckgo.com/?q={query}"))
        }
        fn page_favicon(&self, _: &str) {}
        // A new tab at the top of Today, shown.
        fn open_tab(&self, url: &str, behind: bool) {
            let (tab, url) = (self.next_tab.get(), url.to_string());
            self.next_tab.set(tab + 1);
            self.later_shell(move |shell, cx| {
                shell.set_title(tab, title(&url), cx);
                shell.set_page(
                    tab,
                    Page {
                        url: url.into(),
                        ..Default::default()
                    },
                    cx,
                );
                let mut today = shell.today().to_vec();
                today.insert(0, tab);
                set_today(shell, today, cx);
                if !behind {
                    shell.set_selected(tab, cx);
                }
            });
        }
        // The demo has the one window.
        fn open_window(&self, url: &str, _: &mut App) {
            self.open_tab(url, false);
        }
        fn navigate(&self, tab: u64, url: &str) {
            let url = url.to_string();
            self.later_shell(move |shell, cx| {
                shell.set_title(tab, title(&url), cx);
                shell.set_page(
                    tab,
                    Page {
                        url: url.into(),
                        ..shell.page(tab)
                    },
                    cx,
                );
            });
        }
        fn reload(&self, _: u64) {}
        fn select(&self, tab: u64) {
            self.later_shell(move |shell, cx| shell.set_selected(tab, cx));
        }
        fn tab_window(&self, _: u64) -> Option<u64> {
            None
        }
        fn activate(&self, _: u64, _: u64, _: &mut App) {}
        fn split(&self, _: u64, _: u64, _: &mut App) {}
        fn open_split(&self, url: &str, _: u64, _: &mut App) {
            self.open_tab(url, false);
        }
        fn copy(&self, _: &str, _: &mut App) {}
        fn show(
            &self,
            centered: bool,
            left: f32,
            top: f32,
            width: f32,
            radius: f32,
            _: browser::elevation::Level,
        ) {
            let place = Place {
                centered,
                left,
                top,
                width,
                radius,
            };
            self.later_root(move |root, window, cx| root.show(place, window, cx));
        }
        fn set_height(&self, height: f32) {
            self.later_root(move |root, _, cx| {
                root.height = height;
                cx.notify();
            });
        }
        fn hide(&self) {
            self.later_root(|root, window, cx| root.close(false, window, cx));
        }
        fn dismiss(&self) {
            self.later_root(|root, window, cx| root.close(true, window, cx));
        }
        // The bar draws in the one canvas, which has the page's keys once clicked (web::Root).
        fn take_keyboard(&self) {}
        fn commands(&self, text: &str, cx: &App) -> Vec<shell::BarCommand> {
            shell::bar_commands(text, cx)
        }
        // The demo runs no agents: ⇥ asks nothing.
        fn task_agents(&self, _: &App) -> Option<command_bar::TaskAgents> {
            None
        }
        fn over_private(&self) -> bool {
            false
        }
        fn asking(&self, _: bool, _: &mut App) {}
        fn task_page(&self, _: &App) -> Option<(SharedString, SharedString)> {
            None
        }
        fn run_task(&self, _: &str, _: &str, _: Option<(SharedString, SharedString)>, _: &mut App) {
        }
        fn set_up_agents(&self, _: &mut App) {}
        fn tooltip(
            &self,
            _: Option<(browser::tooltip::Tip, browser::tooltip::Anchor)>,
            _: &mut App,
        ) {
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use browser::shell::{Browser, Folders, Shell};
    use gpui::TestAppContext;

    use super::Fake;

    /// A Space left keeps the tab it showed: its page beside, sliding out once the next Space is
    /// shown, draws that tab's URL field, not an empty one's, and the Space shows it again on its
    /// return, as the core's do.
    #[gpui::test]
    fn a_space_left_keeps_the_tab_it_showed(cx: &mut TestAppContext) {
        let fake: Rc<RefCell<Option<Rc<Fake>>>> = Rc::default();
        let made = fake.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| {
            let browser = Rc::new(Fake {
                shell: cx.weak_entity(),
                next_id: Default::default(),
                spaces: Default::default(),
                archive: Default::default(),
                downloads: Default::default(),
                opt_ins: Default::default(),
                opens_on: Default::default(),
                shows: Default::default(),
            });
            *made.borrow_mut() = Some(browser.clone());
            let mut shell = Shell::new(browser, cx);
            let space = |id: u64| (id, "".into(), None, 0, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space(1), space(2)], cx);
            for (space, today) in [(1, vec![11, 12]), (2, vec![21, 22])] {
                shell.set_tabs(space, Vec::new(), today, Vec::new(), Folders::default(), cx);
            }
            shell.set_space(1, cx);
            shell.set_selected(12, cx);
            shell
        });
        let fake = fake.borrow().clone().unwrap();
        cx.update(|_, cx| fake.show_space(2, cx));
        cx.run_until_parked();
        assert_eq!(fake.space_tab(1), 12, "the Space left forgot its tab");
        assert_eq!(shell.read_with(cx, |shell, _| shell.selected()), 21);

        cx.update(|_, cx| fake.select(22, cx));
        cx.run_until_parked();
        cx.update(|_, cx| fake.show_space(1, cx));
        cx.run_until_parked();
        assert_eq!(fake.space_tab(2), 22, "the Space left forgot its tab");
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.selected()),
            12,
            "back on another tab"
        );
    }
}
