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

use browser::log;
use browser::shell::{
    self, Archived, BarTab, Browser, Command, Download, DownloadAction, DownloadState,
    Folders, Match, MenuBar, MenuEntry, Page, Shell, SpaceEntry, Split, Trouble, WindowButton,
};
use browser::command_bar::{self, Suggestion};
use browser::icon_picker::{self, Recents};
use browser::menu;
use browser::palette::{self, Palette};
use browser::theme::Theme;
use gpui::{
    App, AppContext, Bounds, Focusable, Pixels, Point, SharedString, Size, TitlebarOptions,
    WeakEntity, WindowBounds, WindowOptions, point, px, size,
};

thread_local! {
    /// The preview window's size, where its menus may go.
    static PREVIEW_AREA: Cell<Size<Pixels>> = Cell::new(size(px(1280.), px(800.)));
}

#[cfg(not(target_family = "wasm"))]
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
    next_id: Cell<u64>,
    spaces: RefCell<Vec<SpaceEntry>>,
    archive: RefCell<Vec<(u64, &'static str, &'static str, f64)>>,
    downloads: RefCell<Vec<Download>>,
    /// The tab each Space opens on where the site names one.
    opens_on: RefCell<HashMap<u64, u64>>,
    /// The tab each Space showed as it was left, which it shows again, and its page beside shows
    /// while it pages, as the core keeps them.
    shows: Rc<RefCell<HashMap<u64, u64>>>,
}

/// The tab the shown Space opens on, where the site names none: its first Today tab, the page in
/// use rather than a pinned app, else its first pinned one.
fn opening_tab(shell: &Shell) -> u64 {
    shell.today().first().or(shell.pinned().first()).copied().unwrap_or(0)
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
    fn duplicate(&self, tab: u64, cx: &mut App) {
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

    fn set_folder_open(&self, folder: u64, open: bool, cx: &mut App) {
        self.change_folder(folder, cx, move |entry| entry.1 = open);
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

    fn reopen_tab(&self, _: u64, _: &mut App) -> u64 {
        0
    }

    fn pinned_url(&self, _: u64) -> String {
        String::new()
    }

    fn set_pinned_url(&self, _: u64, _: &str, _: &mut App) {}

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

    fn reload(&self, _: u64, _: bool, _: &mut App) {}

    fn clear_site(&self, _: u64, _: bool, _: &mut App) {}

    // The made-up pages ask nothing.
    fn answer_permission(&self, _: u64, _: shell::Answer, _: &mut App) {}

    fn stop(&self, _: u64, _: &mut App) {}

    fn zoom(&self, _: u64, _: i32, _: &mut App) {}

    // The made-up pages have no source and no DevTools.
    fn view_source(&self, _: u64, _: &mut App) {}
    fn save_page(&self, _: u64, _: &mut App) {}
    fn close_page_menu(&self, _: u64, _: Option<i32>, _: &mut App) {}
    fn print(&self, _: u64, _: &mut App) {}

    fn devtools(&self, _: u64, _: i32, _: &mut App) {}

    fn task_manager(&self, _: &mut App) {}

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

    fn close_other_window(&self, _: &mut App) -> bool {
        false
    }

    fn command_bar(
        &self,
        editing: Option<(u64, SharedString)>,
        tabs: Vec<BarTab>,
        field: Option<Bounds<Pixels>>,
        cx: &mut App,
    ) {
        // On the web the bar draws in the one window, over the shell (web::Root), which is mid-update
        // here, so the bar shows once that's over.
        if cfg!(target_family = "wasm") {
            return cx.defer(move |cx| command_bar::show(editing, tabs, field, cx));
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

    // The made-up pages have nothing to find.
    fn find(&self, _: u64, _: &mut App) {}

    fn site_controls(
        &self,
        _: u64,
        _: browser::site_controls::Place,
        _: Size<Pixels>,
        _: Vec<browser::site_controls::Extension>,
        _: &mut App,
    ) {
    }

    fn site_controls_extensions(
        &self,
        _: u64,
        _: Vec<browser::site_controls::Extension>,
        _: &mut App,
    ) {
    }

    fn capture(&self, _: u64, _: Bounds<Pixels>, _: f32, _: bool, _: &mut App) {}

    fn page_picture(&self, _: u64, _: u64, _: &mut App) {}

    fn page_picture_end(&self, _: u64, _: &mut App) {}

    fn find_next(&self, _: u64, _: bool, _: &mut App) {}

    fn jump_to_selection(&self, _: u64, _: &mut App) {}

    // The preview has no Settings window.
    fn settings(&self, _: &mut App) {}

    fn shortcuts(&self, _: &mut App) {}

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
        last.or_else(|| self.opens_on.borrow().get(&space).copied()).unwrap_or(0)
    }

    fn create_space(&self, name: &str, profile: u64, cx: &mut App) -> u64 {
        let space = self.next_id.get();
        self.next_id.set(space + 1);
        let entry =
            (space, name.to_string().into(), None, profile, "".into(), "".into(), "".into());
        self.spaces.borrow_mut().push(entry);
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

    fn delete_profile(&self, _: u64, _: &mut App) {}

    // The preview's window is GPUI's own, which has the keyboard already.
    fn focus_ui(&self, _: &mut App) {}

    // There's no page, and the window's buttons stay where GPUI put them.
    // The site lays its frame out under the canvas there.
    fn lay_out_page(&self, _frame: Bounds<Pixels>, _radius: f32, _: &mut App) {
        #[cfg(target_family = "wasm")]
        web::lay_out(_frame, _radius);
    }

    fn set_focus_color(&self, _: u32, _: &mut App) {}

    fn set_page_color(&self, _: u32, _: &mut App) {}

    fn page_moving(&self, _: bool, _: &mut App) {}

    fn set_pane_corner(&self, _: f32, _: f32, _: &mut App) {}

    // No command bar here but the web's, as for a new tab, and no split views but the made-up one.
    fn command_bar_open(&self, cx: &App) -> bool {
        cfg!(target_family = "wasm") && command_bar::is_open(cx)
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
            }
            let (pinned, today) = (shell.pinned().to_vec(), shell.today().to_vec());
            let folders = shell.folders();
            shell.set_tabs(shell.space(), pinned, today, splits, folders, cx);
        });
    }

    fn split_with(&self, _: u64, _: u64, _: bool, _: &mut App) {}

    // The targets' window is the app's, over the page, which the preview has neither of.
    fn show_drop_targets(&self, _: bool, _: &mut App) {}

    fn show_window_buttons(&self, _: bool, _: &mut App) {}

    fn press_window_button(&self, _: WindowButton, _: &mut App) {}

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

    fn share_services(&self, _: &str) -> Vec<SharedString> {
        Vec::new()
    }

    fn share(&self, _: &str, _: &str, _: usize, _: &mut App) {}

    // The toast's window is one the app's host lays over the browser window.
    fn toast(&self, _: &str, _: Option<(&str, &str)>, _: Duration, _: &mut App) {}

    fn toast_undo(&self, _: &str, _: Rc<dyn Fn(&mut App)>, _: Duration, _: &mut App) {}

    fn hide_undo_toast(&self, _: &mut App) {}

    fn set_edit_titles(&self, _: Option<&str>, _: Option<&str>, _: &mut App) {}

    fn full_screen_note(&self, _: bool, _: &mut App) {}

    // The toast is a window over the page, which the preview has neither of.
    fn hang_note(&self, _: u64, _: bool, _: &mut App) {}

    // The tooltip's window is the app's, over the page, which the preview has neither of.
    fn tooltip(&self, _: Option<(browser::tooltip::Tip, browser::tooltip::Anchor)>, _: &mut App) {}

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
        let look = if light { menu::Look::Light } else { look };
        let area = Bounds::new(point(px(0.), px(0.)), PREVIEW_AREA.get());
        menu::show(menu::Request { items, at, area, look, chosen }, cx);
    }

    // The dialog's window is one the app's host lays over the browser window. The preview goes
    // without it.
    fn confirm(
        &self,
        _: String,
        _: String,
        _: &'static str,
        _: Box<dyn FnOnce(&mut App)>,
        _: &mut App,
    ) {
    }

    fn quit(&self, cx: &mut App) {
        cx.quit();
    }

    // The preview has one window.
    fn new_window(&self, _: &mut App) {}

    fn new_private_window(&self, _: &mut App) {}

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
    fn select(&self, _: u64) {}
    fn tab_window(&self, _: u64) -> Option<u64> {
        None
    }
    fn activate(&self, _: u64, _: u64, _: &mut App) {}
    fn split(&self, _: u64, _: u64, _: &mut App) {}
    fn open_split(&self, _: &str, _: u64, _: &mut App) {}
    fn copy(&self, _: &str, _: &mut App) {}
    fn show(&self, _: bool, _: f32, _: f32, _: f32, _: f32) {}
    fn set_height(&self, _: f32) {}
    fn hide(&self) {}
    fn dismiss(&self) {}
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
            let rows = vec![
                command("Clear Today", Command::Clear, Some("⇧⌘K"), Match::Name),
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
    palette::set_current(Palette::of(preview_theme().as_ref()), cx);
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

/// wasm-bindgen runs `main` as the page loads the module, and the page starts the demo with
/// `web::start`.
#[cfg(target_family = "wasm")]
fn main() {}

#[cfg(not(target_family = "wasm"))]
fn main() {
    gpui_platform::application().with_assets(shell::Assets).run(|cx| {
        browser::face::register(cx);
        shell::bind_keys(cx);
        browser::text_input::bind_keys(cx);
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
                    next_id: Cell::new(folder + 1 + READING.len() as u64),
                    spaces: RefCell::new(spaces.clone()),
                    archive: RefCell::new(archive),
                    downloads: RefCell::new(downloads),
                    opens_on: RefCell::default(),
                    shows: Rc::default(),
                });
                browser.report_archive(cx);
                browser.report_downloads(cx);
                // PREVIEW_EMPTY=1 shows no page, as a fresh ⌘N window does; =reading does so in the
                // second Space, for its colour, and =private in a private window with no tabs.
                // PREVIEW_WELCOME=1 shows no page with the first run's welcome.
                let welcome = std::env::var_os("PREVIEW_WELCOME").is_some();
                let empty = std::env::var("PREVIEW_EMPTY").ok().or(welcome.then(|| "1".into()));
                let mut shell = match empty.as_deref() {
                    Some("private") => {
                        Shell::private_window(browser, first_space, "Private".into(), 0, cx)
                    }
                    _ => Shell::new(browser, cx),
                };
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
                if welcome {
                    shell.set_welcome(cx);
                }
                shell
            });
            window.focus(&shell.focus_handle(cx), cx);
            shell
        });
        let shell_window = window.as_ref().ok().copied();
        match window {
            // PREVIEW_LIBRARY=1 opens the Library, and PREVIEW_LIBRARY=downloads or =spaces opens
            // it there.
            Ok(window) if std::env::var_os("PREVIEW_LIBRARY").is_some() => {
                window
                    .update(cx, |shell, _, cx| match std::env::var("PREVIEW_LIBRARY").as_deref() {
                        Ok("downloads") => shell.run(Command::ViewDownloads, "preview", cx),
                        Ok("spaces") => shell.show_spaces(cx),
                        _ => shell.run(Command::ViewLibrary, "preview", cx),
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
        // PREVIEW_MENU=<x>,<y>[,<key>…] right-clicks there in the window, as on a row or a Space's
        // name, and then presses the keys in ilo's menu, such as `m,right` for a submenu.
        if let Ok(what) = std::env::var("PREVIEW_MENU") {
            PREVIEW_AREA.set(bounds.size);
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

/// A made-up page menu of `kind`, as Chrome builds it on the Mac once the core has taken out what
/// Arc's leaves out.
fn page_menu(kind: &str) -> Vec<shell::PageMenuItem> {
    let row = |command: i32, title: &str| shell::PageMenuItem {
        command,
        title: title.to_string().into(),
        parent: None,
        enabled: true,
        checked: false,
    };
    let line = || row(0, "");
    let under =
        |parent: usize, item: shell::PageMenuItem| shell::PageMenuItem { parent: Some(parent), ..item };
    let inspect = [line(), row(26500, "Inspect")];
    let mut items = match kind {
        "image" => vec![
            row(50120, "Open Image in New Tab"),
            row(50121, "Save Image As…"),
            row(50122, "Copy Image"),
            row(50123, "Copy Image Address"),
        ],
        "text" => vec![
            row(50160, "Look Up “ilo”"),
            line(),
            row(50150, "Copy"),
            row(50151, "Search DuckDuckGo for “ilo”"),
            row(35003, "Print…"),
            line(),
            row(50170, "Speech"),
            under(6, row(50171, "Start Speaking")),
            under(6, shell::PageMenuItem { enabled: false, ..row(50172, "Stop Speaking") }),
        ],
        "field" => vec![
            row(41000, "receive"),
            row(41001, "relieve"),
            row(41100, "Add to Dictionary"),
            row(41101, "Language Settings"),
            line(),
            shell::PageMenuItem { enabled: false, ..row(50151, "Cut") },
            shell::PageMenuItem { enabled: false, ..row(50150, "Copy") },
            row(50152, "Paste"),
            row(50153, "Paste and Match Style"),
            row(50154, "Select All"),
            row(50180, "Writing Direction"),
            under(10, shell::PageMenuItem { checked: true, ..row(50181, "Default") }),
            under(10, row(50182, "Left to Right")),
            under(10, row(50183, "Right to Left")),
            line(),
            row(50170, "Speech"),
            under(15, row(50171, "Start Speaking")),
            under(15, shell::PageMenuItem { enabled: false, ..row(50172, "Stop Speaking") }),
        ],
        _ => vec![
            row(50100, "Open Link in New Tab"),
            row(50101, "Open Link in New Window"),
            row(50105, "Open Link in Split View"),
            row(50102, "Open Link in Incognito Window"),
            line(),
            row(50103, "Save Link As…"),
            row(50104, "Copy Link Address"),
        ],
    };
    items.extend(inspect);
    items
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
        shell.update(cx, |shell, _, cx| shell.show_page_menu(1, page_menu(&kind), at, cx)).ok();
        press_keys(window, keys, cx).await;
    })
    .detach();
}

/// ilo's menu over the preview's window, whose host can't move it: its window covers the preview's.
struct PreviewMenu;

impl menu::Host for PreviewMenu {
    fn show(&self, _: Bounds<Pixels>) {}

    fn hide(&self) {}
}

/// The icon picker over the preview's window, whose host can't move it: it opened where it's to be.
struct PreviewPicker;

impl icon_picker::Host for PreviewPicker {
    fn show(&self, _: f32, _: f32) {}

    fn hide(&self) {}

    fn keep_recents(&self, _: &str) {}
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
/// first frame is up. The demo takes the keyboard only once it's clicked or its command bar opens,
/// and gives it back when the bar closes; `openBar()` opens the bar.
#[cfg(target_family = "wasm")]
mod web {
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
        sync::Arc,
    };

    use browser::command_bar::{self, CommandBar, Suggestion};
    use browser::log;
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
            let Some(function) = function.as_ref().and_then(|f| f.dyn_ref::<js_sys::Function>())
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
        let args = numbers.into_iter().chain([radius]).map(|n| JsValue::from(n as f64));
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
            icon: get(&tab, "icon").as_string().filter(|icon| !icon.is_empty()),
        };
        list(object, key).into_iter().map(tab).collect()
    }

    /// Starts the demo in `canvas`, with the Spaces in `spaces`, telling `host` what it does.
    #[wasm_bindgen]
    pub fn start(canvas: web_sys::HtmlCanvasElement, spaces: JsValue, host: JsValue) {
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
        trace(&canvas);
        gpui_web::draw_in(canvas);
        gpui_platform::web_init();
        gpui_platform::single_threaded_web().with_assets(shell::Assets).run(move |cx| {
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
                    let shell = shell_of(&favorites, &spaces, &mut icons, &mut space_ids, cx);
                    pages = favorites
                        .iter()
                        .chain(spaces.iter().flat_map(|space| space.pinned.iter().chain(&space.today)))
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
    /// and when the page heard it, and puts a button on the page that saves them as JSON: a real
    /// swipe, to check what `shell::swipe`'s model assumes of a trackpad and a browser against.
    fn trace(canvas: &web_sys::HtmlCanvasElement) {
        const RECORD: &str = r#"
            if (!new URLSearchParams(location.search).has("trace")) return;
            const events = [];
            const button = document.createElement("button");
            const label = () => { button.textContent = `Save wheel trace (${events.length} events)`; };
            canvas.addEventListener("wheel", (event) => {
                const { timeStamp: stamp, deltaX: x, deltaY: y, deltaMode: mode } = event;
                events.push({ stamp, heard: performance.now(), x, y, mode });
                label();
            }, { capture: true, passive: true });
            Object.assign(button.style, { position: "fixed", right: "12px", bottom: "12px",
                zIndex: "2147483647", font: "13px system-ui, sans-serif", padding: "6px 10px" });
            button.onclick = () => {
                const trace = { userAgent: navigator.userAgent, devicePixelRatio, events };
                const link = document.createElement("a");
                link.href = URL.createObjectURL(new Blob([JSON.stringify(trace)], { type: "application/json" }));
                link.download = "ilo-wheel-trace.json";
                link.click();
                events.length = 0;
                label();
            };
            label();
            document.body.append(button);
        "#;
        let record = js_sys::Function::new_with_args("canvas", RECORD);
        if let Err(error) = record.call1(&JsValue::NULL, canvas) {
            log!("couldn't record wheel events: {error:?}");
        }
    }

    /// Opens the command bar and gives it the keyboard, as ⌘K (⌃K) in the demo does: the site calls
    /// it for ⌘K anywhere on its page, since the page's keys are the site's until the demo's clicked.
    #[wasm_bindgen(js_name = openBar)]
    pub fn open_bar() {
        let Some((app, shell)) = DEMO.with_borrow(Clone::clone) else {
            return;
        };
        app.update(|cx| shell.update(cx, |shell, cx| shell.run(Command::NewTab, "site", cx))).ok();
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
                (*id, space.name.clone().into(), space.color, 0, "".into(), "".into(), "".into())
            })
            .collect();
        let browser = Rc::new(Fake {
            shell: cx.weak_entity(),
            next_id: Cell::new(next),
            spaces: RefCell::new(entries.clone()),
            archive: RefCell::new(Vec::new()),
            downloads: RefCell::new(Vec::new()),
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
            let tabs = pinned.iter().chain(today).zip(space.pinned.iter().chain(&space.today));
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
        shell.set_page(tab, Page { url: site.url.clone().into(), ..Default::default() }, cx);
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
                shell.update(cx, |shell, cx| shell.set_favicon(tab, Some(favicon), cx)).ok();
            })
            .detach();
        }
    }

    async fn fetch(url: &str) -> Option<Vec<u8>> {
        use wasm_bindgen_futures::JsFuture;
        let response = JsFuture::from(web_sys::window()?.fetch_with_str(url)).await.ok()?;
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
        let mut bindings = vec![KeyBinding::new(open, OpenCommandBar, Some(shell::KEY_CONTEXT))];
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
                let watch = cx.observe(&shell, |root: &mut Root, shell, cx| root.tell_site(&shell, cx));
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
                self.shell.update(cx, |shell, cx| shell.command_bar_dismissed(cx));
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
                    root.shell.update(cx, |shell, cx| shell.run(Command::NewTab, "web", cx));
                }))
                .child(self.shell.clone());
            let Some(place) = self.place else {
                return root;
            };
            let viewport = window.viewport_size();
            let (left, top) = match place.centered {
                true => ((viewport.width - px(place.width)) / 2., viewport.height / 2. - px(place.top)),
                false => (px(place.left), px(place.top)),
            };
            // A click anywhere but the bar leaves it, as one outside its window does in the app.
            let outside = div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .occlude()
                .on_mouse_down(MouseButton::Left, cx.listener(|root, _, window, cx| {
                    root.close(true, window, cx)
                }));
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
            self.app.foreground_executor().spawn(async move { app.update(change) }).detach();
        }

        fn later_root(&self, change: impl FnOnce(&mut Root, &mut Window, &mut Context<Root>) + 'static) {
            let (root, window) = (self.root.clone(), self.window);
            self.later(move |cx| {
                window
                    .update(cx, |_, window, cx| root.update(cx, |root, cx| change(root, window, cx)))
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
                .map(|url| Suggestion { url, text: text.clone().into(), ..Default::default() })
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
        fn remove_suggestion(&self, _: &str) {}
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
                shell.set_page(tab, Page { url: url.into(), ..Default::default() }, cx);
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
                shell.set_page(tab, Page { url: url.into(), ..shell.page(tab) }, cx);
            });
        }
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
        fn show(&self, centered: bool, left: f32, top: f32, width: f32, radius: f32) {
            let place = Place { centered, left, top, width, radius };
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
        fn commands(&self, text: &str, cx: &App) -> Vec<shell::BarCommand> {
            shell::bar_commands(text, cx)
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
        assert_eq!(shell.read_with(cx, |shell, _| shell.selected()), 12, "back on another tab");
    }
}
