//! The UI alone, with made-up tabs, for working on how it looks: it needs no engine and rebuilds in
//! seconds. `cargo run --release --bin preview`. The browser's own rules for tabs live in the core,
//! so the ones here are only stand-ins.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use browser::account::{self, Account};
use browser::import::sources::{Os, Roots};
use browser::log;
use browser::shell::{
    self, Archived, BarTab, Browser, Command, Download, DownloadAction, DownloadState,
    Folders, Match, MediaAction, MenuBar, MenuEntry, Page, Shell, SpaceEntry, Split, Trouble,
    WindowButton,
};
use browser::command_bar::{self, Suggestion};
use browser::dialog;
use browser::icon_picker::{self, Recents};
use browser::menu;
use browser::palette::{self, Palette, Tone};
use browser::site_controls::{self, Connection, Permission, Site, Value};
use browser::theme::{Appearance, Theme};
use browser::theme_picker;
use gpui::{
    App, AppContext, Bounds, Focusable, Pixels, Point, SharedString, Size, TitlebarOptions,
    WeakEntity, WindowBounds, WindowOptions, point, px, size,
};

thread_local! {
    /// The preview window's size, where its menus may go.
    static PREVIEW_AREA: Cell<Size<Pixels>> = Cell::new(size(px(1280.), px(800.)));
    /// Its top left on screen, where an overlay opened later goes from.
    static PREVIEW_ORIGIN: Cell<Point<Pixels>> = Cell::new(point(px(0.), px(0.)));
    /// Whether the dialog's window has opened, the first time the preview asks something.
    static DIALOG_OPEN: Cell<bool> = const { Cell::new(false) };
}

unsafe extern "C" {
    // platform/mac/host.mm: where a window goes, centered on the main display or, with
    // BROWSER_DISPLAY=builtin, on the Mac's own.
    fn host_centered(width: f64, height: f64, x: *mut f64, y: *mut f64);
}

// Two Spaces, so the switcher shows, the second in Dia's pink. The tabs are in the first, the top
// two pinned, the second in a folder.
const SPACES: [(&str, Option<u32>); 2] = [("Space 1", None), ("Reading", Some(0xbf556a))];
const TABS: [(&str, &str); 5] = [
    (
        "Arc (web browser) - Wikipedia",
        "https://en.wikipedia.org/wiki/Arc_(web_browser)",
    ),
    ("Hacker News", "https://news.ycombinator.com/"),
    ("Zed — The editor for what's next", "https://zed.dev/"),
    (
        "Rust Programming Language",
        "https://www.rust-lang.org/",
    ),
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
    ("GitHub", "https://github.com/", Icon::Disc([0.12, 0.13, 0.16])),
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
                    if inside { [1., 1., 1., 1.] } else { [0.9, 0.1, 0.1, 1.] }
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
    ("Internet Assigned Numbers Authority", "https://www.iana.org/", 0.2),
    ("W3C", "https://www.w3.org/", 1.1),
    ("The GNU Operating System and the Free Software Movement", "https://www.gnu.org/", 2.5),
    ("Chromium", "https://www.chromium.org/", 6.),
    ("", "https://example.org/", 12.),
];
const DAY: f64 = 86_400_000.;

// The downloads: one running, one paused, one held, and finished ones, newest first, with their
// bytes so far and in all, and how many days ago they started.
const DOWNLOADS: [(&str, &str, i64, i64, DownloadState, bool, f64); 6] = [
    ("zed-0.170.dmg", "https://zed.dev/", 48_200_000, 121_500_000, RUNNING, false, 0.),
    ("rust-book.pdf", "https://doc.rust-lang.org/", 3_100_000, 7_800_000, PAUSED, false, 0.01),
    ("installer.command", "http://example.com/", 2_000, 4_000, RUNNING, true, 0.02),
    ("report.csv", "https://data.example.org/", 18_400, 18_400, DONE, false, 0.3),
    ("photo.jpg", "https://commons.wikimedia.org/", 2_480_000, 2_480_000, DONE, false, 1.2),
    ("old-notes.txt", "https://example.net/", 0, 0, DownloadState::Cancelled, false, 9.),
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
    let at = pinned.iter().position(|&id| id == row).map_or(0, |at| at + 1);
    let within = pinned[at..].iter().take_while(|&&id| holders(folders, id).any(|f| f == row));
    at + within.count()
}

/// Where `index` in `folder`, else in the pinned tabs, is in `pinned`, as the core counts.
fn pinned_at(pinned: &[u64], folders: &Folders, folder: u64, index: usize) -> usize {
    let start = pinned.iter().position(|&id| id == folder).map_or(0, |at| at + 1);
    let end = if folder == 0 { pinned.len() } else { end_of(pinned, folders, folder) };
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
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0., |now| now.as_millis() as f64)
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
    fn later(&self, cx: &mut App, change: impl FnOnce(&mut Shell, &mut gpui::Context<Shell>) + 'static) {
        let shell = self.shell.clone();
        cx.defer(move |cx| {
            shell.update(cx, change).ok();
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
            let at = today.iter().position(|&id| id == tab).map_or(0, |index| index + 1);
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
                splits.iter_mut().for_each(|split| split.tabs.retain(|&id| id != tab));
                splits.retain(|split| split.tabs.len() > 1);
                let folders = shell.folders();
                shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
            }
        });
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
            let folders = Folders { pinned_shut: !open, ..shell.folders() };
            let (pinned, today) = (shell.pinned().to_vec(), shell.today().to_vec());
            let splits = shell.splits();
            shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
        });
    }

    fn move_folder(&self, folder: u64, to: u64, index: usize, cx: &mut App) {
        self.later(cx, move |shell, cx| move_pinned(shell, folder, to, index, cx));
    }

    fn delete_folder(&self, folder: u64, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            let (mut pinned, mut folders) = (shell.pinned().to_vec(), shell.folders());
            pinned.retain(|&id| id != folder);
            folders.folders.remove(&folder);
            let holder = folders.parents.remove(&folder).unwrap_or(0);
            for parent in folders.parents.values_mut().filter(|parent| **parent == folder) {
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

    fn reopen_preview(
        &self,
        _: &browser::link_preview::Closed,
        _: (Bounds<Pixels>, f32),
        _: &mut App,
    ) {
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

    fn clear_downloads(&self, cx: &mut App) {
        self.downloads.borrow_mut().retain(|each| {
            matches!(each.state, DownloadState::Running | DownloadState::Paused)
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
            let page = Page { muted, ..shell.page(tab) };
            shell.set_page(tab, page, cx);
        });
    }

    // The made-up pages share nothing.
    fn share_instead(&self, _: u64, _: &mut App) {}

    fn media(&self, tab: u64, action: MediaAction, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            let media_playing = match action {
                MediaAction::Play => true,
                MediaAction::Pause => false,
                MediaAction::Next => shell.page(tab).media_playing,
                MediaAction::PictureInPicture => shell.page(tab).media_playing,
            };
            // A made-up page floats nothing, but its player's button goes as a real one's would.
            let media_picture_in_picture = (shell.page(tab).media_picture_in_picture)
                && action != MediaAction::PictureInPicture;
            let page = Page { media_playing, media_picture_in_picture, ..shell.page(tab) };
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
            let cast = shell::Cast { header: "Cast tab".into(), devices: vec![device] };
            shell.set_cast(tab, Some(cast), cx);
        });
    }

    fn cast_close(&self, _: &mut App) {}

    // Casting starting closes the dialog, as the core's does.
    fn cast_to(&self, tab: u64, _: SharedString, cx: &mut App) {
        self.later(cx, move |shell, cx| shell.set_cast(tab, None, cx));
    }

    fn can_use_system_engine(&self) -> bool {
        true
    }

    fn use_system_engine(&self, tab: u64, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            shell.set_media_unsupported(tab, "", cx);
            let page = Page { system_engine: true, ..shell.page(tab) };
            shell.set_page(tab, page, cx);
        });
    }

    fn set_icon(&self, tab: u64, icon: &str, cx: &mut App) {
        let icon: SharedString = icon.to_string().into();
        self.later(cx, move |shell, cx| {
            let page = Page { icon, ..shell.page(tab) };
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
        _: Vec<BarTab>,
        _: Option<Bounds<Pixels>>,
        cx: &mut App,
    ) {
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
        cx: &mut App,
    ) {
        if std::env::var_os("PREVIEW_SITE_CONTROLS").is_none() {
            return;
        }
        let (at, _) = site_controls::placed(place, window);
        let room = size(px(site_controls::WIDTH), window.height - at.y);
        let bounds = Bounds::new(PREVIEW_ORIGIN.get() + at, room);
        if site_controls::open_at(Rc::new(PreviewSiteControls), bounds, true, cx).is_some() {
            site_controls::show(tab, place, window, extensions, cx);
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
        _: bool,
        _: menu::Look,
        _: &mut App,
    ) {
    }

    fn page_picture(&self, _: u64, _: u64, _: &mut App) {}

    fn find_next(&self, _: u64, _: bool, _: &mut App) {}

    fn jump_to_selection(&self, _: u64, _: &mut App) {}

    // The preview has no Settings window.
    fn settings(&self, _: &mut App) {}

    fn passwords(&self, _: u64, _: &mut App) {}
    fn agent_activity(&self, _: &mut App) {}

    fn agent_permissions(&self, _: u64, _: &App) -> browser::shell::AgentGrants {
        Default::default()
    }

    fn revoke_agent(&self, _: u64, _: Option<SharedString>, _: &mut App) {}

    fn disconnect_agent(&self, _: u64, _: &mut App) {}

    fn take_back_agent_tabs(&self, _: u64, _: &mut App) {}

    fn take_over(&self, _: SharedString, _: u64, _: &mut App) {}

    fn hand_back(&self, _: u64, _: &mut App) {}

    fn give_to_agent(&self, _: u64, _: u64, _: &mut App) {}

    fn stop_agent(&self, _: SharedString, _: &mut App) {}

    fn shortcuts(&self, _: &mut App) {}

    fn about(&self, _: &mut App) {}

    fn import(&self, _: &mut App) {}

    // Each made-up Space shows its top tab.
    fn show_space(&self, space: u64, cx: &mut App) {
        self.later(cx, move |shell, cx| {
            shell.set_space(space, cx);
            let top = shell.today().first().or(shell.pinned().first()).copied().unwrap_or(0);
            shell.set_selected(top, cx);
        });
    }

    // The made-up Spaces' pages slide in without a selected row.
    fn space_tab(&self, _: u64) -> u64 {
        0
    }

    fn create_space(&self, name: &str, profile: u64, after: u64, cx: &mut App) -> u64 {
        let space = self.next_id.get();
        self.next_id.set(space + 1);
        let entry =
            (space, name.to_string().into(), None, profile, "".into(), "".into(), "".into());
        let mut spaces = self.spaces.borrow_mut();
        let at = spaces.iter().position(|(id, ..)| *id == after).map_or(spaces.len(), |at| at + 1);
        spaces.insert(at, entry);
        drop(spaces);
        self.report_spaces(cx);
        space
    }

    fn rename_space(&self, space: u64, name: &str, cx: &mut App) {
        if let Some(entry) = self.spaces.borrow_mut().iter_mut().find(|(id, ..)| *id == space) {
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

    fn delete_space(&self, space: u64, cx: &mut App) -> bool {
        self.spaces.borrow_mut().retain(|(id, ..)| *id != space);
        self.report_spaces(cx);
        true
    }

    fn set_space_color(&self, space: u64, color: Option<u32>, cx: &mut App) {
        if let Some(entry) = self.spaces.borrow_mut().iter_mut().find(|(id, ..)| *id == space) {
            entry.2 = color;
        }
        self.report_spaces(cx);
    }

    fn set_space_icon(&self, space: u64, icon: &str, cx: &mut App) {
        if let Some(entry) = self.spaces.borrow_mut().iter_mut().find(|(id, ..)| *id == space) {
            entry.4 = icon.to_string().into();
        }
        self.report_spaces(cx);
    }

    fn set_space_theme(&self, space: u64, theme: &str, cx: &mut App) {
        if let Some(entry) = self.spaces.borrow_mut().iter_mut().find(|(id, ..)| *id == space) {
            entry.6 = theme.to_string().into();
        }
        self.report_spaces(cx);
    }

    fn set_space_profile(&self, space: u64, profile: u64, cx: &mut App) {
        if let Some(entry) = self.spaces.borrow_mut().iter_mut().find(|(id, ..)| *id == space) {
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

    // The preview's window is GPUI's own, which has the keyboard already.
    fn focus_ui(&self, _: &mut App) {}

    fn focus_page(&self, _: &mut App) {}

    // There's no page, and the window's buttons stay where GPUI put them.
    fn lay_out_page(&self, _: Bounds<Pixels>, _: f32, _: &mut App) {}

    fn set_focus_color(&self, _: u32, _: &mut App) {}

    fn set_page_color(&self, _: u32, _: &mut App) {}

    // The preview has no material: its window is the tint over GPUI's clear.
    fn set_window_light(&self, _: bool, _: &mut App) {}

    // The preview keeps nothing for a next launch.
    fn set_appearance(&self, _: Appearance, _: &mut App) {}

    fn page_moving(&self, _: bool, _: &mut App) {}

    fn set_pane_corner(&self, _: f32, _: f32, _: &mut App) {}

    // No command bar here, as for a new tab, and no split views but the made-up one.
    fn command_bar_open(&self, _: &App) -> bool {
        false
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

    fn show_window_buttons(&self, _: bool, _: &mut App) {}

    fn press_window_button(&self, _: WindowButton, _: &mut App) {}

    // The preview's window is GPUI's own, which GPUI moves.
    fn start_window_move(&self, _: &mut App) {}

    fn window_maximized(&self) -> bool {
        false
    }

    fn window_full_screen(&self) -> bool {
        false
    }

    // The reveal's window is the app's, over the page, which the preview has neither of.
    fn show_reveal(&self, _: f32, _: &mut App) {}

    fn hide_reveal(&self, _: &mut App) {}

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
        _: (Option<&'static str>, &'static str),
        _: Rc<dyn Fn(&mut App)>,
        _: Duration,
        _: &mut App,
    ) {
    }

    fn set_edit_titles(&self, _: Option<&str>, _: Option<&str>, _: &mut App) {}

    fn full_screen_note(&self, _: bool, _: &mut App) {}

    // The toast is a window over the page, which the preview has neither of.
    fn hang_note(&self, _: u64, _: bool, _: &mut App) {}

    // The tooltip's window is the app's, over the page, which the preview has neither of.
    fn tooltip(&self, _: Option<(browser::tooltip::Tip, browser::tooltip::Anchor)>, _: &mut App) {}

    // So is the hover card's, and there's no engine to picture a page.
    fn hover_card(&self, _: browser::hover_card::Hover, _: &mut App) {}

    // And the folder preview's.
    fn folder_preview(&self, _: browser::folder_preview::Ask, _: &mut App) {}

    fn card_picture(&self, _: u64, _: &mut App) {}

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
        let look = menu::Look { light: light || look.light, ..look };
        let area = Bounds::new(point(px(0.), px(0.)), PREVIEW_AREA.get());
        menu::show(menu::Request { items, at, area, look, chosen }, cx);
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
        dialog::ask(message, detail, action, confirmed, cx);
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

    fn new_little_window(&self, _: u64, _: &mut App) {}

    fn open_tab_in_little(&self, _: u64, _: &mut App) {}

    fn open_in_main_window(&self, _: u64, _: u64, _: &mut App) {}

    fn close_window(&self, cx: &mut App) {
        cx.quit();
    }
}

/// The command bar's requests, for the preview: it goes nowhere, but a search goes to a search.
struct PreviewBar;

impl command_bar::Host for PreviewBar {
    fn suggest(&self, _: Option<&str>, _: &str, _: bool) {}
    fn remove_suggestion(&self, _: &str) {}
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
}

/// Opens the command bar at `at`, with the suggestions for what `mode` has typed.
/// PREVIEW_THEME=#RRGGBB gives the first Space a theme of that colour alone, and
/// PREVIEW_THEME="x=… y=… colors=3 …" the theme as the core keeps it.
fn preview_theme() -> Option<Theme> {
    let text = std::env::var("PREVIEW_THEME").ok()?;
    match text.strip_prefix('#') {
        Some(hex) => u32::from_str_radix(hex, 16).ok().map(Theme::of_color),
        None => Theme::parse(&text),
    }
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
        ("Arc (web browser) - Wikipedia", "https://en.wikipedia.org/wiki/Arc", 2),
        ("Zed — The editor for what's next", "https://zed.dev/", 3),
        ("Example Domain", "https://example.com/", 5),
    ]
    .into_iter()
    .map(|(title, url, tab)| shell::BarTab {
        tab,
        title: title.into(),
        url: url.into(),
        favicon: None,
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
            let (cache, cookies) = (Command::ClearCacheAndRefresh, Command::ClearCookiesAndRefresh);
            let clear_keys = shell::keystroke_label("shift-cmd-k");
            let rows = vec![
                command("Clear Today", Command::Clear, Some(&clear_keys), Match::Name),
                search("clear"),
                command("Clear Cache and Refresh", cache, None, Match::Name),
                command("Clear Cookies and Refresh", cookies, None, Match::Name),
                page("clearhaus", "clearhaus.com", "https://clearhaus.com/", 0),
                command("Collapse All Folders", Command::CollapseFolders, None, Match::Letters),
            ];
            ("clear", rows)
        }
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
                page("Arc (web browser) - Wikipedia", "", "https://en.wikipedia.org/wiki/Arc", 1),
                page("Zed — The editor for what's next", "", "https://zed.dev/", 3),
                page("Rust Programming Language", "", "https://www.rust-lang.org/", 4),
                page("The Browser Company", "", "https://thebrowser.company/", 0),
            ],
        ),
    };
    let rows_shown = rows.len();
    let bounds = Bounds::new(at, size(px(command_bar::WIDTH), px(command_bar::height(rows_shown))));
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
        "" => command_bar::show(None, tabs, None, cx),
        typed => command_bar::show(Some((0, typed.to_string().into())), Vec::new(), None, cx),
    }
    let icons = [Icon::Envelope, Icon::Blocks, Icon::Play, Icon::Disc([0.9, 0.5, 0.1])];
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

fn main() {
    gpui_platform::application().with_assets(shell::Assets).run(|cx| {
        browser::face::register(cx);
        shell::bind_keys(cx);
        browser::text_input::bind_keys(cx);
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
                let mut spaces: Vec<SpaceEntry> =
                    (first_space..)
                        .zip(SPACES)
                        .zip(["glyph:star", "glyph:leaf"])
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
                let first_archived = first_space + SPACES.len() as u64;
                let archive = (first_archived..)
                    .zip(ARCHIVE)
                    .map(|(id, (title, url, days))| (id, title, url, now() - days * DAY))
                    .collect();
                let first_download = first_archived + ARCHIVE.len() as u64;
                let downloads = (first_download..)
                    .zip(DOWNLOADS)
                    .map(|(id, (name, url, received, total, state, held, days))| Download {
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
                    })
                    .collect();
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
                let empty = std::env::var("PREVIEW_EMPTY").ok().or(welcome.then(|| "1".into()));
                let mut shell = match empty.as_deref() {
                    Some("private") => {
                        Shell::private_window(browser, first_space, "Private".into(), 0, cx)
                    }
                    _ => Shell::new(browser, cx),
                };
                shell.force_tone(preview_tone());
                shell.set_profiles(vec![(0, "Default".into())], cx);
                shell.set_spaces(spaces, cx);
                shell.set_space(first_space, cx);
                // The shell forgets what it knows of a tab in no list, so the lists come first.
                shell.set_favorites(favorites.iter().map(|&tab| (tab, 0)).collect(), cx);
                for (&tab, (title, url, icon)) in favorites.iter().zip(FAVORITES) {
                    shell.set_title(tab, title.into(), cx);
                    shell.set_page(tab, Page { url: url.into(), ..Default::default() }, cx);
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
                // PREVIEW_DRM=1 has the first tab's page ask for video ilo plays only in Apple's
                // engine, so its note shows.
                if std::env::var_os("PREVIEW_DRM").is_some() {
                    shell.set_media_unsupported(1, "com.widevine.alpha", cx);
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
                shell.set_tabs(first_space + 1, pinned, today, Vec::new(), Folders::default(), cx);
                for (index, (&tab, (title, url))) in reading.iter().zip(READING).enumerate() {
                    shell.set_title(tab, title.into(), cx);
                    let loading = agent.is_some()
                        && (index == 3 || (index == 2 && agent.as_deref() == Some("loading")));
                    shell.set_page(tab, Page { url: url.into(), loading, ..Default::default() }, cx);
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
                            what: "hand you a tab".into(),
                            why,
                            handoff: Some(reading[3]),
                            back,
                        };
                        shell.set_agent_question(Some(question), cx);
                    }
                }
                let favorite = std::env::var("PREVIEW_FAVORITE").ok().and_then(|n| n.parse().ok());
                match (empty.as_deref(), favorite.and_then(|n: usize| favorites.get(n))) {
                    (Some("private"), _) => {
                        let none = Folders::default();
                        shell.set_tabs(first_space, Vec::new(), Vec::new(), Vec::new(), none, cx);
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
                    shell.set_page(tab, Page { trouble, ..shell.page(tab) }, cx);
                }
                // PREVIEW_DEVELOPER_MODE=1 has the tab shown, the third tab and the GitHub
                // favorite in Developer Mode, so its outline and contrast show.
                if std::env::var_os("PREVIEW_DEVELOPER_MODE").is_some() {
                    for tab in [shell.selected(), 3, favorites[2]] {
                        let page = Page { developer_mode: true, ..shell.page(tab) };
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
            // PREVIEW_LIBRARY=1 opens the Library, and PREVIEW_LIBRARY=downloads or =spaces opens
            // it there; =rename opens its Spaces with the first card's name in its field.
            Ok(window) if std::env::var_os("PREVIEW_LIBRARY").is_some() => {
                window
                    .update(cx, |shell, window, cx| {
                        match std::env::var("PREVIEW_LIBRARY").as_deref() {
                            Ok("downloads") => shell.run(Command::ViewDownloads, "preview", cx),
                            Ok("spaces") => shell.show_spaces(cx),
                            Ok("rename") => {
                                shell.show_spaces(cx);
                                shell.preview_rename_card(window, cx);
                            }
                            _ => shell.run(Command::ViewLibrary, "preview", cx),
                        }
                    })
                    .ok();
            }
            Ok(_) => {}
            Err(error) => {
                log!("couldn't open the window: {error}");
                cx.quit();
            }
        }
        // PREVIEW_BAR=tabs, sites or commands shows the command bar over the window, with made-up
        // suggestions.
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
            if icon_picker::open_at(Rc::new(PreviewPicker), Recents::default(), bounds, true, cx)
                .is_some()
            {
                let picked: icon_picker::Picked = Rc::new(|_, _| {});
                let subject = icon_picker::Subject::Space(1);
                icon_picker::show(subject, icon.into(), point(0., 0.), picked, cx);
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
                theme_picker::show(1, preview_theme(), anchor, f32::MAX, changed, cx);
            }
        }
        // PREVIEW_SITE_CONTROLS=1 opens the Site Controls of the tab shown, once the shell has
        // drawn the URL field they hang from.
        let site_controls = std::env::var_os("PREVIEW_SITE_CONTROLS").is_some();
        if let Some(shell) = shell_window.filter(|_| site_controls) {
            cx.spawn(async move |cx| {
                cx.background_executor().timer(Duration::from_millis(500)).await;
                shell
                    .update(cx, |shell, _, cx| shell.run(Command::SiteControls, "preview", cx))
                    .ok();
            })
            .detach();
        }
        // PREVIEW_MENU=<x>,<y>[,<key>…] right-clicks there in the window, as on a row or a Space's
        // name, and then presses the keys in ilo's menu, such as `m,right` for a submenu.
        if let Ok(what) = std::env::var("PREVIEW_MENU") {
            if let Some(shell) = shell_window {
                preview_menu(shell.into(), bounds, &what, cx);
            }
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

fn preview_menu(shell: gpui::AnyWindowHandle, bounds: Bounds<Pixels>, what: &str, cx: &mut App) {
    let mut parts = what.split(',');
    let mut number = || parts.next().and_then(|part| part.trim().parse::<f32>().ok());
    let (Some(x), Some(y)) = (number(), number()) else { return };
    let keys: Vec<String> = what.split(',').skip(2).map(|key| key.trim().to_string()).collect();
    let Some(window) = menu::open_at(Rc::new(PreviewMenu), bounds, cx) else { return };
    // Not the menu's own handle, whose update would hold the menu the keys go to.
    let window: gpui::AnyWindowHandle = window.into();
    cx.spawn(async move |cx| {
        // Once the shell has drawn its rows, whose boxes the click finds.
        cx.background_executor().timer(Duration::from_millis(500)).await;
        let position = point(px(x), px(y));
        let modifiers = gpui::Modifiers::default();
        let down = gpui::MouseDownEvent {
            button: gpui::MouseButton::Right,
            position,
            modifiers,
            click_count: 1,
            first_mouse: false,
        };
        let up = gpui::MouseUpEvent { button: gpui::MouseButton::Right, position, modifiers, click_count: 1 };
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
        cx.background_executor().timer(Duration::from_millis(50)).await;
        let os = gpui::OperatingSystem::Mac;
        let Some(keystroke) = gpui::Keystroke::parse_with_os(&key, os).ok() else { continue };
        window.update(cx, |_, window, cx| window.dispatch_keystroke(keystroke, cx)).ok();
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
    let (Some(x), Some(y)) = (number(), number()) else { return };
    let keys: Vec<String> = what.split(',').skip(3).map(|key| key.trim().to_string()).collect();
    let Some(window) = menu::open_at(Rc::new(PreviewMenu), bounds, cx) else { return };
    let window: gpui::AnyWindowHandle = window.into();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_millis(500)).await;
        let at = point(px(x), px(y));
        // In this system's case, as its Chromium writes the menu.
        let items = shell::made_up_page_menu(&kind, cfg!(target_os = "macos"));
        shell.update(cx, |shell, _, cx| shell.show_page_menu(1, items, at, cx)).ok();
        press_keys(window, keys, cx).await;
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
struct PreviewPicker;

impl icon_picker::Host for PreviewPicker {
    fn show(&self, _: f32, _: f32) {}

    fn hide(&self) {}

    fn keep_recents(&self, _: &str) {}
}

impl theme_picker::Host for PreviewPicker {
    fn show(&self, _: f32, _: f32, _: &[(f32, f32, f32)]) {}

    fn hide(&self) {}

    fn detent(&self) {}
}

/// The dialog over the preview's window, which it covers, for its host can't move it.
struct PreviewDialog;

impl dialog::Host for PreviewDialog {
    fn show(&self, _: Option<Bounds<Pixels>>) {}

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

    fn set_system_engine(&self, _: u64, _: bool) {}

    fn allow_saving(&self, _: u64, _: &browser::passwords::Login) {}

    fn set_ad_blocking(&self, _: u64, _: bool) {}
    fn set_developer_mode(&self, _: u64, _: bool) {}

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
    Site {
        site: "en.wikipedia.org".into(),
        connection: Connection::Secure,
        connection_summary: "Connection is secure".into(),
        certificate: true,
        permissions: vec![
            permission(5, "geolocation", "Location", Value::Ask, "Ask (default)", &Value::ALL),
            permission(6, "notifications", "Notifications", Value::Allow, "Allowed", &Value::ALL),
            permission(4, "popups", "Pop-ups and redirects", Value::Block, "Blocked", &Value::ALL),
            permission(2, "javascript", "JavaScript", Value::Allow, "Allowed", &[]),
        ],
        reset: Some("Reset permissions".into()),
        system_engine: None,
        logins: Vec::new(),
        ad_blocking: Some(true),
        developer_mode: Some(false),
    }
}

/// Settings › Sync alone, framed as Settings frames a tab, in a window of Settings' size.
fn preview_account(state: &str, cx: &mut App) {
    use account::{Device, State, Status};
    use browser::tokens::settings::{HEIGHT, WIDTH};
    account::bind_keys(cx);
    let (mut x, mut y) = (0., 0.);
    unsafe { host_centered(WIDTH as f64, HEIGHT as f64, &mut x, &mut y) };
    let bounds = Bounds::new(point(px(x as f32), px(y as f32)), size(px(WIDTH), px(HEIGHT)));
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
        server: if no_server { "".into() } else { "https://sync.ilo.app".into() },
        default_server: !no_server,
        email: "sam@example.com".into(),
        code: if status == Status::Pairing { "312568".into() } else { "".into() },
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
    let options =
        WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
    let opened = cx.open_window(options, |window, cx| {
        let page = cx.new(|cx| {
            let dialog = dialog::in_window(Rc::new(PreviewDialog), cx);
            let host = Rc::new(PreviewAccount { dialog: dialog.clone() });
            let account = cx.new(|cx| {
                let mut account = Account::new(host, cx);
                account.set_state(state, cx);
                if status == Status::On {
                    account.set_devices(devices, cx);
                }
                account
            });
            AccountPage { account, dialog }
        });
        window.focus(&page.read(cx).account.focus_handle(cx), cx);
        page
    });
    if let Err(error) = opened {
        log!("preview: couldn't open the account's window: {error}");
    }
}

/// The account's view on Settings' background, in from its sides as Settings' tabs are, with the
/// dialog it asks in over all of it, as Settings has on Linux.
struct AccountPage {
    account: gpui::Entity<Account>,
    dialog: gpui::Entity<dialog::Dialog>,
}

impl gpui::Render for AccountPage {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use browser::tokens::settings::{BACKGROUND, FIRST, SIDE, TEXT};
        use browser::tokens::text;
        use gpui::{ParentElement, Styled, div, rgb};
        div()
            .relative()
            .size_full()
            .bg(rgb(BACKGROUND))
            .font_family(browser::face::UI)
            .text_size(px(text::BODY))
            .text_color(rgb(TEXT))
            .child(div().px(px(SIDE)).py(px(FIRST)).child(self.account.clone()))
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
        dialog::ask_in(&self.dialog, message, detail, action, on_return, confirmed, window, cx);
    }
}
