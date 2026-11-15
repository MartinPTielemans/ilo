//! The Library button's recent downloads: the newest files, in rows over the foot of the sidebar,
//! the newest next to the button, coming and going as Arc's do, of the kinds picked from the
//! button's menu, the Library's captures among them (docs/ux-spec.md › Downloads).

use std::{sync::Arc, time::Duration};
use web_time::{Instant, SystemTime, UNIX_EPOCH};

use gpui::{
    AnyElement, Context, Image, MouseButton, MouseDownEvent, Pixels, Point, SharedString, Task,
    Window, div, img, linear_color_stop, linear_gradient, prelude::*, px, svg,
};

use super::{
    Download, DownloadState, FOOTER_HEIGHT, LIST_TOP, MenuEntry, NAV_SIZE, SECONDARY, Section,
    Shell,
    library::{capture_id, is_video, of},
};
use crate::palette::{STATE_HOVERED, STATE_PRESSED};

/// What the hover lists, bits the button's menu turns on and off: images, videos and other files
/// downloaded, and the captures saved to the Library (Arc's screenshots).
pub const HOVER_IMAGES: u32 = 1;
pub const HOVER_VIDEOS: u32 = 2;
pub const HOVER_OTHER_FILES: u32 = 4;
pub const HOVER_CAPTURES: u32 = 8;
pub const HOVER_ALL: u32 = 15;
/// The menu's rows, in its order, with their names in the log.
const HOVER_MENU: [(&str, &str, u32); 4] = [
    ("Images", "images", HOVER_IMAGES),
    ("Videos", "videos", HOVER_VIDEOS),
    ("Other Files", "other files", HOVER_OTHER_FILES),
    ("Captures", "captures", HOVER_CAPTURES),
];

/// The bit a file's name puts it under.
fn kind(name: &str) -> u32 {
    match is_video(name) {
        Some(false) => HOVER_IMAGES,
        Some(true) => HOVER_VIDEOS,
        None => HOVER_OTHER_FILES,
    }
}

/// Arc's: the rows show 0.2 s into a rest on the button, so a pointer passing it leaves them. The
/// bottom one comes first and each next one 25 ms later, fading in over 0.18 s as it rises 5 pt
/// over 0.12 s.
const RECENT_WAIT: Duration = Duration::from_millis(200);
const RECENT_STAGGER_IN: f32 = 0.025;
const RECENT_FADE_IN: f32 = 0.18;
const RECENT_RISE: f32 = 5.;
const RECENT_RISE_TIME: f32 = 0.12;
/// Arc's: they go 70 ms after the pointer leaves, the bottom one first and the top one 50 ms after
/// it, each fading out over 0.13 s where it is.
const RECENT_LINGER: f32 = 0.07;
const RECENT_STAGGER_OUT: f32 = 0.05 / (RECENT_COUNT - 1) as f32;
const RECENT_FADE_OUT: f32 = 0.13;
/// Arc's: four rows 56 apart, a 40 pt column at 8 for an image's preview 40 wide or the file's
/// icon 26 wide, and the name at 61 over its age.
const RECENT_COUNT: usize = 4;
use crate::tokens::shell::recent_downloads::AGE_LINE;
use crate::tokens::shell::recent_downloads::AGE_SIZE;
use crate::tokens::shell::recent_downloads::COLUMN;
use crate::tokens::shell::recent_downloads::COLUMN_X;
use crate::tokens::shell::recent_downloads::ICON;
use crate::tokens::shell::recent_downloads::NAME;
use crate::tokens::shell::recent_downloads::NAME_LINE;
use crate::tokens::shell::recent_downloads::NAME_SIZE;
use crate::tokens::shell::recent_downloads::PLATE_INSET;
use crate::tokens::shell::recent_downloads::PLATE_RADIUS;
use crate::tokens::shell::recent_downloads::PREVIEW_HEIGHT;
use crate::tokens::shell::recent_downloads::PREVIEW_RADIUS;
use crate::tokens::shell::recent_downloads::RECENT_PITCH;
use crate::tokens::shell::recent_downloads::SHADE;
use crate::tokens::shell::recent_downloads::SHADE_GAP;
use crate::tokens::shell::recent_downloads::TEXT_END;
use crate::tokens::shell::recent_downloads::TEXT_X;

const MINUTE: f64 = 60_000.;
const HOUR: f64 = 60. * MINUTE;
const DAY: f64 = 24. * HOUR;
const WEEK: f64 = 7. * DAY;
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// What `Browser::file_preview` found for a file: an image's thumbnail, else its icon, `width` by
/// `height` pixels.
#[derive(Clone)]
pub struct FilePreview {
    pub image: Arc<Image>,
    pub width: f32,
    pub height: f32,
    /// It's the image's own, not an icon.
    pub photo: bool,
}

/// The recent downloads' rows, and where the pointer is that keeps them.
#[derive(Default)]
pub(super) struct RecentDownloads {
    over_button: bool,
    over_rows: bool,
    /// The wait before they show, from the pointer coming onto the button.
    wait: Option<Task<()>>,
    /// The downloads shown, newest first, with their ages as they opened. Empty while none show.
    rows: Vec<(u64, SharedString)>,
    /// They're going, and take no pointer.
    going: bool,
    /// When the fade toward where they're going started, from the first frame drawn after it was
    /// set.
    fade: Option<Instant>,
    /// How opaque each row was as the fade started, newest first, where it turned on its way; none
    /// for rows that started from nothing, or from all there.
    from: Vec<f32>,
    /// The fade has ended, and its line is logged.
    settled: bool,
}

impl RecentDownloads {
    /// How opaque the row `index` places from the bottom was as the fade started.
    fn from(&self, index: usize) -> f32 {
        let start = if self.going { 1. } else { 0. };
        self.from.get(index).copied().unwrap_or(start)
    }
}

/// How opaque the row `index` places from the bottom draws `t` seconds into its fade from `from`,
/// toward going or coming. One starting from nothing, or from all there, waits its turn; one
/// turned on its way turns at once.
fn row_opacity(going: bool, index: usize, from: f32, t: f32) -> f32 {
    let index = index as f32;
    match going {
        false => {
            let start = if from == 0. {
                index * RECENT_STAGGER_IN
            } else {
                0.
            };
            (from + (t - start).max(0.) / RECENT_FADE_IN).min(1.)
        }
        true => {
            let start = RECENT_LINGER
                + if from == 1. {
                    index * RECENT_STAGGER_OUT
                } else {
                    0.
                };
            (from - (t - start).max(0.) / RECENT_FADE_OUT).max(0.)
        }
    }
}

/// How far under its place a row coming in draws at `opacity`: it rises out of the ease as it's
/// two thirds in, since its rise takes 0.12 s of the fade's 0.18.
fn row_drop(opacity: f32) -> f32 {
    let risen = (opacity * RECENT_FADE_IN / RECENT_RISE_TIME).min(1.);
    RECENT_RISE * (1. - risen).powi(3)
}

/// The downloads the rows can show, of `kinds`, newest first: the finished ones with a file, and
/// those still coming, as a download that runs on shouldn't hide the files before it. A held one
/// shows too, as nothing else says it's waiting to be kept or deleted.
fn recent(downloads: &[Download], kinds: u32) -> impl Iterator<Item = &Download> {
    let shown = move |each: &&Download| {
        kind(&each.name) & kinds != 0
            && match each.state {
                DownloadState::Done => !each.path.is_empty(),
                DownloadState::Running | DownloadState::Paused => true,
                DownloadState::Cancelled | DownloadState::Failed => false,
            }
    };
    downloads.iter().filter(shown)
}

/// How long ago `started` is at `now`, in milliseconds since the Unix epoch, in words, with
/// `today` the start of today in the user's calendar: "just now", minutes, hours, "yesterday",
/// days, weeks, then the date.
pub(super) fn age(started: f64, now: f64, today: f64) -> String {
    let ago = (now - started).max(0.);
    let count = |unit: f64, name: &str| match (ago / unit).floor() as i64 {
        1 => format!("1 {name} ago"),
        n => format!("{n} {name}s ago"),
    };
    match ago {
        _ if ago < MINUTE => "just now".into(),
        _ if ago < HOUR => count(MINUTE, "minute"),
        _ if ago < DAY => count(HOUR, "hour"),
        _ if ago < 2. * DAY => "yesterday".into(),
        _ if ago < WEEK => count(DAY, "day"),
        _ if ago < 5. * WEEK => count(WEEK, "week"),
        _ => date(started, today),
    }
}

/// The day `at`, before `today`, falls on in the user's calendar, as "Sep 12", with the year if
/// it isn't this year's.
fn date(at: f64, today: f64) -> String {
    // Local midnight is within 12 hours of UTC's on the same date in all but the furthest zones.
    let today_day = (today / DAY).round() as i64;
    let back = ((today - at) / DAY).ceil().max(0.) as i64;
    let (year, month, day) = civil(today_day - back);
    let month = MONTHS[month as usize - 1];
    match civil(today_day).0 == year {
        true => format!("{month} {day}"),
        false => format!("{month} {day}, {year}"),
    }
}

/// The year, month and day `days` after 1 January 1970, in the Gregorian calendar (Howard
/// Hinnant's `civil_from_days`).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

pub(super) fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0., |since| since.as_secs_f64() * 1e3)
}

impl Shell {
    /// What the rows show, newest first, each a download's id or a capture's (`capture_id`), with
    /// when it started or was saved.
    fn recent_files(&self, cx: &mut Context<Self>) -> Vec<(u64, f64)> {
        let kinds = self.browser.hover_kinds(cx);
        let downloads = recent(&self.downloads, kinds).map(|each| (each.id, each.started));
        let captures = (self.captures.iter())
            .filter(|_| kinds & HOVER_CAPTURES != 0)
            .map(|each| (capture_id(&each.path), each.saved));
        let mut files: Vec<_> = downloads.chain(captures).collect();
        files.sort_by(|a, b| b.1.total_cmp(&a.1));
        files.truncate(RECENT_COUNT);
        files
    }

    /// The menu a right-click on the Library button shows: what its hover lists, each ticked while
    /// it does.
    pub(super) fn hover_kinds_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.recent_downloads_gone(cx);
        let kinds = self.browser.hover_kinds(cx);
        // The files' kinds, a line, then the captures.
        let rows = [Some(0), Some(1), Some(2), None, Some(3)];
        let rows = rows.map(|row| row.map(|at| HOVER_MENU[at]));
        let entries = (rows.iter())
            .map(|row| match row {
                Some((title, _, bit)) => MenuEntry::new(title, true).checked(kinds & bit != 0),
                None => MenuEntry::line(),
            })
            .collect();
        let in_reveal = self.library.is_none() && self.sidebar_hidden;
        self.context_menu_in(
            entries,
            position,
            in_reveal,
            Box::new(move |chosen, shell, cx| {
                let Some((_, _, bit)) = chosen.and_then(|at| rows.get(at).copied().flatten())
                else {
                    return;
                };
                let kinds = shell.browser.hover_kinds(cx) ^ bit;
                shell.browser.set_hover_kinds(kinds, cx);
                let named: Vec<_> = (HOVER_MENU.iter())
                    .filter(|(_, _, bit)| kinds & bit != 0)
                    .map(|(_, name, _)| *name)
                    .collect();
                // check.sh waits for this line.
                log!("browser: hover kinds {}", named.join(", "));
            }),
            cx,
        );
    }

    /// The pointer came onto the Library button, or the rows, or left. It leaves one as it comes
    /// onto the other in the same move, in either order, so what that means waits for both.
    pub(super) fn hover_recent_downloads(
        &mut self,
        rows: bool,
        hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match rows {
            true => self.recent_downloads.over_rows = hovered,
            false => self.recent_downloads.over_button = hovered,
        }
        cx.defer_in(window, |shell, _, cx| shell.settle_recent_downloads(cx));
    }

    /// Shows the rows a moment after the pointer comes onto the button, keeps them while it's on
    /// the button or them, and sends them away once it's on neither.
    pub(super) fn settle_recent_downloads(&mut self, cx: &mut Context<Self>) {
        // A row's menu takes the pointer from them as it comes on screen over them, and they stay
        // while it's open, as the reveal does, for it to act on them. They settle as it closes.
        if self.menu_open {
            return;
        }
        let recent = &self.recent_downloads;
        if recent.over_rows && !recent.going && !recent.rows.is_empty() {
            // check.sh reads this line, which says the pointer keeps them.
            log!("browser: recent downloads under the pointer");
        }
        if !recent.over_button && !(recent.over_rows && !recent.going) {
            self.recent_downloads.wait = None;
            return self.hide_recent_downloads(cx);
        }
        if !recent.rows.is_empty() {
            // Back on the button as they go, they come back from where they are.
            if recent.going {
                self.fade_recent_downloads(false, cx);
            }
            return;
        }
        if recent.wait.is_some() || !self.recent_downloads_can_show() {
            return;
        }
        // Listed again as the wait starts, so they're current when it ends.
        if self.browser.hover_kinds(cx) & HOVER_CAPTURES != 0 {
            self.browser.list_captures(cx);
        }
        self.recent_downloads.wait = Some(cx.spawn(async move |shell, cx| {
            cx.background_executor().timer(RECENT_WAIT).await;
            shell
                .update(cx, |shell, cx| {
                    shell.recent_downloads.wait = None;
                    shell.open_recent_downloads(cx);
                })
                .ok();
        }));
    }

    /// Whether the rows can show: the sidebar is still, docked or out in the reveal, with the
    /// Library closed.
    fn recent_downloads_can_show(&self) -> bool {
        let still = self.sidebar_motion.is_none() && self.library.is_none();
        let shown = !self.sidebar_hidden || self.revealed;
        still && shown && !self.page_fullscreen && self.recent_downloads.over_button
    }

    /// Lists the newest downloads with their ages now, if there are any, and asks for the
    /// previews it hasn't yet.
    fn open_recent_downloads(&mut self, cx: &mut Context<Self>) {
        if !self.recent_downloads_can_show() {
            return;
        }
        let (now, today) = (now(), self.browser.day_starts()[0]);
        let rows: Vec<_> = (self.recent_files(cx).into_iter())
            .map(|(id, at)| (id, age(at, now, today).into()))
            .collect();
        if rows.is_empty() {
            // check.sh waits for this line.
            log!("browser: recent downloads none");
            return;
        }
        let recent = &mut self.recent_downloads;
        recent.rows = rows;
        recent.going = false;
        recent.fade = None;
        recent.from.clear();
        recent.settled = false;
        // The list shows the download the bin holds.
        self.bin_leaves(cx);
        self.ask_download_previews(cx);
        cx.notify();
    }

    /// Asks once for the preview of each row's file that has finished, as one still coming has
    /// no file to look at yet.
    fn ask_download_previews(&mut self, cx: &mut Context<Self>) {
        let path = |id: u64| match self.downloads.iter().find(|each| each.id == id) {
            Some(each) => {
                (each.state == DownloadState::Done && !each.held && !each.path.is_empty())
                    .then(|| each.path.clone())
            }
            None => (self.captures.iter())
                .find(|each| capture_id(&each.path) == id)
                .map(|each| each.path.clone()),
        };
        let asked: Vec<_> = (self.recent_downloads.rows.iter())
            .filter(|(id, _)| !self.download_previews.contains_key(id))
            .filter_map(|&(id, _)| Some((id, path(id)?)))
            .collect();
        for (id, path) in asked {
            self.download_previews.insert(id, None);
            self.browser.file_preview(id, &path, cx);
        }
    }

    /// Sends the rows away with their fade, from where they are, if they show.
    pub(super) fn hide_recent_downloads(&mut self, cx: &mut Context<Self>) {
        if !self.recent_downloads.rows.is_empty() && !self.recent_downloads.going {
            self.fade_recent_downloads(true, cx);
        }
    }

    /// Takes the rows away at once, as what they show over goes.
    pub(super) fn recent_downloads_gone(&mut self, cx: &mut Context<Self>) {
        let recent = &mut self.recent_downloads;
        recent.wait = None;
        recent.over_rows = false;
        if recent.rows.is_empty() {
            return;
        }
        recent.rows.clear();
        // check.sh waits for this line.
        log!("browser: recent downloads hidden");
        cx.notify();
    }

    /// Turns the rows' fade toward going, or coming, each row from the opacity it's at.
    fn fade_recent_downloads(&mut self, going: bool, cx: &mut Context<Self>) {
        let now = cx.background_executor().now();
        let from = self.recent_row_opacities(now);
        let recent = &mut self.recent_downloads;
        recent.going = going;
        recent.settled = false;
        if going {
            recent.over_rows = false;
        }
        recent.from = from;
        // Now, or from the first frame if none has been drawn yet.
        recent.fade = recent.fade.map(|_| now);
        cx.notify();
    }

    /// How opaque each row draws, newest first, at `now` on the executor's clock, which a test
    /// moves on.
    fn recent_row_opacities(&self, now: Instant) -> Vec<f32> {
        let recent = &self.recent_downloads;
        let t = recent
            .fade
            .map_or(0., |fade| now.saturating_duration_since(fade).as_secs_f32());
        let row = |index| row_opacity(recent.going, index, recent.from(index), t);
        (0..recent.rows.len()).map(row).collect()
    }

    /// The downloads or captures changed: the rows keep those still there, and go with none.
    pub(super) fn recent_downloads_changed(&mut self, cx: &mut Context<Self>) {
        if self.recent_downloads.rows.is_empty() {
            return;
        }
        let files = self.recent_files(cx);
        let recent = &mut self.recent_downloads;
        let kept: Vec<_> = (recent.rows.iter())
            .map(|(id, _)| files.iter().any(|(file, _)| file == id))
            .collect();
        let mut keeps = kept.iter();
        recent.rows.retain(|_| keeps.next() == Some(&true));
        let mut keeps = kept.iter();
        recent.from.retain(|_| keeps.next() == Some(&true));
        if recent.rows.is_empty() {
            // check.sh waits for this line.
            log!("browser: recent downloads hidden");
        }
        self.ask_download_previews(cx);
        // A row still coming shows its bytes, which change with each report, and only while the
        // pointer keeps the rows.
        cx.notify();
    }

    /// The preview `Browser::file_preview` asked for.
    pub fn set_download_preview(
        &mut self,
        id: u64,
        preview: Option<FilePreview>,
        cx: &mut Context<Self>,
    ) {
        if let Some(entry) = self.download_previews.get_mut(&id) {
            *entry = preview;
            let in_media = self.library.is_some() && self.section == Section::Media;
            if in_media || self.recent_downloads.rows.iter().any(|(row, _)| *row == id) {
                cx.notify();
            }
        }
    }

    /// Steps the rows' fade for a frame, and once it's done, says so, and lets them go if they
    /// went.
    pub(super) fn step_recent_downloads(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.recent_downloads.rows.is_empty() || self.recent_downloads.settled {
            return;
        }
        let now = cx.background_executor().now();
        self.recent_downloads.fade.get_or_insert(now);
        let end = if self.recent_downloads.going { 0. } else { 1. };
        if self
            .recent_row_opacities(now)
            .iter()
            .any(|&opacity| opacity != end)
        {
            window.request_animation_frame();
            return;
        }
        let recent = &mut self.recent_downloads;
        recent.settled = true;
        // check.sh waits for these lines, logged with the frame that ends the fade.
        if recent.going {
            recent.rows.clear();
            log!("browser: recent downloads hidden");
        } else {
            log!("browser: recent downloads shown {}", recent.rows.len());
        }
    }

    /// The rows, over the tab list's foot and taking its pointer while they show, and the gap down
    /// to the button, which keeps them as the pointer crosses it; and behind them, the sidebar's
    /// own look hiding the list from just over the top row, coming back over the shade above it.
    /// The shade takes no pointer.
    pub(super) fn recent_download_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let recent = &self.recent_downloads;
        if recent.rows.is_empty() || self.page_fullscreen {
            return Vec::new();
        }
        let live = !recent.going;
        let button_top = (FOOTER_HEIGHT + NAV_SIZE) / 2.;
        let height = recent.rows.len() as f32 * RECENT_PITCH;
        let window = f32::from(self.viewport.height).max(1.);
        let opacities = self.recent_row_opacities(cx.background_executor().now());
        let still = crate::motion::still(cx);
        let rows: Vec<_> = (recent.rows.iter().zip(&opacities).enumerate().rev())
            .map(|(index, ((id, age), &opacity))| {
                let down = match (still, recent.going) {
                    (true, _) => 0.,
                    (false, true) => row_drop(recent.from(index)),
                    (false, false) => row_drop(opacity),
                };
                self.recent_row(*id, age, live, (opacity, down), cx)
            })
            .collect();
        // Never over the list's top, above which are the URL field and the window's buttons.
        let bottom = window - FOOTER_HEIGHT;
        let hidden = bottom - height - SHADE_GAP;
        let top = (hidden - SHADE).max(LIST_TOP.min(hidden));
        let fill = |y: f32| self.window_fill_at(y / window);
        let shade = div()
            .absolute()
            .left_0()
            .w(px(self.width()))
            .top(px(top))
            .h(px(bottom - top))
            .opacity(opacities.iter().copied().fold(0., f32::max))
            .flex()
            .flex_col()
            .child(div().h(px(hidden - top)).bg(linear_gradient(
                180.,
                linear_color_stop(fill(top).opacity(0.), 0.),
                linear_color_stop(fill(hidden), 1.),
            )))
            .child(div().flex_1().bg(linear_gradient(
                180.,
                linear_color_stop(fill(hidden), 0.),
                linear_color_stop(fill(bottom), 1.),
            )));
        let area = div()
            .id("recent-downloads")
            .absolute()
            .left_0()
            .w(px(self.width()))
            .bottom(px(button_top))
            .h(px(height + FOOTER_HEIGHT - button_top))
            .when(live, |area| {
                area.block_mouse_except_scroll().on_hover(cx.listener(
                    |shell, hovered: &bool, window, cx| {
                        shell.hover_recent_downloads(true, *hovered, window, cx)
                    },
                ))
            })
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(height))
                    .flex()
                    .flex_col()
                    .children(rows),
            )
            .children(live.then(|| self.download_press_listeners(cx)).flatten());
        vec![shade.into_any_element(), area.into_any_element()]
    }

    /// A row: the file's preview or icon in its column, its name over its age, and while the rows
    /// show, what a Library row does; at its `opacity`, `down` under its place as it comes.
    fn recent_row(
        &self,
        id: u64,
        age: &SharedString,
        live: bool,
        (opacity, down): (f32, f32),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let download = self.downloads.iter().find(|each| each.id == id);
        let capture =
            (self.captures.iter()).find(|each| download.is_none() && capture_id(&each.path) == id);
        let name = match (download, capture) {
            (Some(download), _) => download.name.clone(),
            (None, Some(capture)) => capture.name.clone(),
            (None, None) => return div().h(px(RECENT_PITCH)).into_any_element(),
        };
        let preview = match self.download_previews.get(&id).cloned().flatten() {
            Some(preview) if preview.photo => {
                let (width, height) = (preview.width.max(1.), preview.height.max(1.));
                let scale = (COLUMN / width).min(PREVIEW_HEIGHT / height);
                img(preview.image)
                    .w(px(width * scale))
                    .h(px(height * scale))
                    .rounded(px(PREVIEW_RADIUS))
                    .into_any_element()
            }
            Some(preview) => img(preview.image).size(px(ICON)).into_any_element(),
            // The generic file glyph, as the Library's rows show, until the preview comes.
            None => svg()
                .path("icons/file.svg")
                .size(px(ICON * 0.75))
                .text_color(self.palette.text(SECONDARY))
                .into_any_element(),
        };
        // One still coming says how much has come, and a held one why, as its Library row does,
        // and its age once done.
        let detail: SharedString = match download {
            Some(download) if download.held => "May harm your computer".into(),
            Some(download) if download.state == DownloadState::Running => {
                of(download.received, download.total).into()
            }
            Some(download) if download.state == DownloadState::Paused => {
                format!("Paused · {}", of(download.received, download.total)).into()
            }
            _ => age.clone(),
        };
        let line = |text: SharedString, size: f32, height: f32, color: f32| {
            div()
                .h(px(height))
                .flex()
                .items_center()
                .text_size(px(size))
                .text_color(self.palette.text(color))
                .child(div().truncate().child(text))
        };
        let plate = div()
            .id(("recent-download", id))
            .absolute()
            .left(px(PLATE_INSET))
            .right(px(PLATE_INSET))
            .top(px(2.))
            .bottom(px(2.))
            .rounded(px(PLATE_RADIUS));
        let plate = match (live, capture) {
            (false, _) => plate,
            (true, None) => self.download_row(plate, id, "recent downloads", cx),
            // A capture's row opens it as its tile in Media does, and has its menu.
            (true, Some(capture)) => {
                let path = capture.path.clone();
                plate
                    .hover(|style| style.bg(self.palette.plate(STATE_HOVERED)))
                    .active(|style| style.bg(self.palette.plate(STATE_PRESSED)))
                    .on_click({
                        let path = path.clone();
                        cx.listener(move |shell, _, _, cx| {
                            shell.open_media(&path, cx);
                            shell.hide_recent_downloads(cx);
                        })
                    })
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                            shell.capture_menu(path.clone(), event.position, cx)
                        }),
                    )
            }
        };
        let row = div()
            .absolute()
            .left_0()
            .right_0()
            .top(px(down))
            .h(px(RECENT_PITCH))
            .child(plate)
            .child(
                div()
                    .absolute()
                    .left(px(COLUMN_X))
                    .top_0()
                    .bottom_0()
                    .w(px(COLUMN))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(preview),
            )
            .child(
                div()
                    .absolute()
                    .left(px(TEXT_X))
                    .right(px(TEXT_END))
                    .top_0()
                    .bottom_0()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .child(line(name, NAME_SIZE, NAME_LINE, NAME))
                    .child(line(detail, AGE_SIZE, AGE_LINE, SECONDARY)),
            );
        let row = div()
            .flex_none()
            .relative()
            .h(px(RECENT_PITCH))
            .opacity(opacity)
            .child(row);
        row.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use std::{rc::Rc, time::Duration};

    use gpui::{Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px};

    use super::{
        super::{
            Captured, Download, DownloadState, Shell,
            library::{Section, capture_id},
            recorder::Recorder,
        },
        DAY, HOUR, MINUTE, RECENT_FADE_IN, RECENT_FADE_OUT, RECENT_LINGER, RECENT_STAGGER_IN,
        RECENT_STAGGER_OUT, RECENT_WAIT, WEEK, age, now, row_drop, row_opacity,
    };

    /// Ages count minutes, hours, then yesterday, days and weeks, and past four weeks give the
    /// date, with its year if it isn't this one's.
    #[test]
    fn ages_say_how_long_ago_in_words() {
        let now = 1_790_000_000_000.;
        let ago = |by: f64| age(now - by, now, now - 3. * HOUR);
        assert_eq!(ago(10_000.), "just now");
        assert_eq!(
            ago(-5_000.),
            "just now",
            "a clock that went back isn't the future"
        );
        assert_eq!(ago(MINUTE), "1 minute ago");
        assert_eq!(ago(59. * MINUTE), "59 minutes ago");
        assert_eq!(ago(HOUR), "1 hour ago");
        assert_eq!(ago(2. * HOUR + 59. * MINUTE), "2 hours ago");
        assert_eq!(ago(DAY + HOUR), "yesterday");
        assert_eq!(ago(4. * DAY), "4 days ago");
        assert_eq!(ago(WEEK + DAY), "1 week ago");
        assert_eq!(ago(4. * WEEK + 6. * DAY), "4 weeks ago");

        // 26 October 2026, at midnight in UTC+2.
        let today = 1_792_965_600_000.;
        let at = |date: f64| age(date, today + 9. * HOUR, today);
        // 7 September, in the evening, and the first minute of that day.
        assert_eq!(at(today - 49. * DAY + 20. * HOUR), "Sep 7");
        assert_eq!(at(today - 49. * DAY + MINUTE), "Sep 7");
        assert_eq!(at(today - 50. * DAY + HOUR), "Sep 6");
        // 31 December 2025, a leap of years and months back.
        assert_eq!(at(today - 299. * DAY + HOUR), "Dec 31, 2025");
        assert_eq!(at(today - 300. * DAY + 23. * HOUR), "Dec 30, 2025");
    }

    /// A finished download `id`, started `ago` milliseconds back.
    fn done(id: u64, ago: f64) -> Download {
        Download {
            id,
            name: format!("file {id}.zip").into(),
            url: "https://example.org/file".into(),
            path: format!("/tmp/file {id}.zip").into(),
            received: 10,
            total: 10,
            state: DownloadState::Done,
            open_when_done: false,
            held: false,
            started: now() - ago,
            profile: 0,
            private: false,
        }
    }

    fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
        });
        cx.run_until_parked();
        (shell, cx, host)
    }

    fn move_to(cx: &mut VisualTestContext, x: f32, y: f32) {
        cx.simulate_mouse_move(point(px(x), px(y)), None, Modifiers::none());
        cx.run_until_parked();
    }

    fn wait(cx: &mut VisualTestContext, time: Duration) {
        cx.executor().advance_clock(time);
        cx.run_until_parked();
    }

    /// The rows listed, newest first, with their ages, and whether they're going.
    fn rows(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (Vec<(u64, String)>, bool) {
        shell.read_with(cx, |shell, _| {
            let recent = &shell.recent_downloads;
            let rows = recent
                .rows
                .iter()
                .map(|(id, age)| (*id, age.to_string()))
                .collect();
            (rows, recent.going)
        })
    }

    /// A right-click on the Library button picks what its hover lists, each kind of file and the
    /// Library's captures, which list among the downloads by when they were saved.
    #[gpui::test]
    fn the_library_button_s_menu_picks_what_its_hover_lists(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        let mut picture = done(2, 2. * HOUR);
        picture.name = "picture.png".into();
        let path = "/media/Capture.png";
        let name = "Capture.png".into();
        let capture = Captured {
            path: path.into(),
            name,
            saved: now() - MINUTE,
        };
        shell.update(cx, |shell, cx| {
            shell.set_downloads(vec![picture, done(1, DAY)], cx);
            shell.set_captures(vec![capture], cx);
        });
        cx.run_until_parked();
        let height = shell.read_with(cx, |shell, _| f32::from(shell.viewport.height));
        let button = point(px(24.), px(height - 25.));
        let pick = |title: &str, cx: &mut VisualTestContext| {
            cx.simulate_mouse_down(button, MouseButton::Right, Modifiers::none());
            cx.simulate_mouse_up(button, MouseButton::Right, Modifiers::none());
            cx.run_until_parked();
            let titles: Vec<_> = (host.menus.borrow().last().unwrap().iter())
                .map(|(title, _)| title.to_string())
                .collect();
            assert_eq!(titles, ["Images", "Videos", "Other Files", "", "Captures"]);
            cx.update(|_, cx| host.choose(title, cx));
            cx.run_until_parked();
            // Off the button, so the next rest shows the rows anew.
            move_to(cx, 400., 300.);
        };
        let listed = |cx: &mut VisualTestContext| {
            move_to(cx, 24., height - 25.);
            wait(cx, RECENT_WAIT);
            let ids = rows(&shell, cx)
                .0
                .into_iter()
                .map(|(id, _)| id)
                .collect::<Vec<_>>();
            move_to(cx, 400., 300.);
            wait(cx, RECENT_WAIT);
            ids
        };
        assert_eq!(listed(cx), [capture_id(path), 2, 1]);
        pick("Other Files", cx);
        assert_eq!(listed(cx), [capture_id(path), 2]);
        pick("Captures", cx);
        assert_eq!(listed(cx), [2]);
        pick("Other Files", cx);
        assert_eq!(listed(cx), [2, 1]);
    }

    /// Resting on the Library button for a moment shows the four newest downloads, the
    /// running and held ones with the finished ones, with their ages, and asks for the finished
    /// ones' previews once, and the running one's once it finishes. They stay as the pointer crosses
    /// from the button onto them, go once it's on neither, and come back from going when it's on
    /// the button again. While the download runs, the button still names itself View Library, and
    /// a click opens the Library where it was, not on Downloads, and takes the rows away.
    #[gpui::test]
    fn the_recent_downloads_show_while_the_pointer_rests_on_the_button(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        let mut running = done(9, 0.);
        running.state = DownloadState::Running;
        let mut held = done(8, MINUTE);
        (held.state, held.held) = (DownloadState::Running, true);
        let downloads = vec![
            running,
            held,
            done(5, 2. * HOUR),
            done(4, 4. * DAY),
            done(3, 8. * DAY),
            done(2, 9. * DAY),
            done(1, 10. * DAY),
        ];
        shell.update(cx, |shell, cx| shell.set_downloads(downloads, cx));
        cx.run_until_parked();
        let height = shell.read_with(cx, |shell, _| f32::from(shell.viewport.height));
        let button = (24., height - 25.);
        host.take();

        move_to(cx, button.0, button.1);
        wait(cx, RECENT_WAIT - Duration::from_millis(100));
        assert!(rows(&shell, cx).0.is_empty(), "they wait for a rest");
        wait(cx, Duration::from_millis(100));
        let shown = vec![
            (9, "just now".to_string()),
            (8, "1 minute ago".to_string()),
            (5, "2 hours ago".to_string()),
            (4, "4 days ago".to_string()),
        ];
        assert_eq!(rows(&shell, cx), (shown.clone(), false));
        // The Library's captures are listed again as the wait starts.
        let asked = ["list captures", "file preview 5", "file preview 4"];
        let calls = host.take();
        assert!(
            calls.iter().any(|call| call == "tooltip View Library ⇧⌘L"),
            "{calls:?}"
        );
        let previews: Vec<_> = calls.iter().filter(|c| !c.starts_with("tooltip")).collect();
        assert_eq!(previews, asked);

        // Up across the gap onto the newest row, at the bottom, and the oldest, at the top.
        move_to(cx, 24., height - 45.);
        move_to(cx, 120., height - 50. - 28.);
        move_to(cx, 120., height - 50. - 3. * 56. - 28.);
        assert_eq!(
            rows(&shell, cx),
            (shown.clone(), false),
            "the rows keep them"
        );

        // Their fade in ends on the test's clock, which stands still from here, so they're still
        // going when the pointer comes back however long the machine takes.
        wait(
            cx,
            Duration::from_secs_f32(3. * RECENT_STAGGER_IN + RECENT_FADE_IN),
        );
        move_to(cx, 120., 200.);
        assert_eq!(
            rows(&shell, cx),
            (shown.clone(), true),
            "they go with the pointer on neither"
        );
        move_to(cx, button.0, button.1);
        assert_eq!(
            rows(&shell, cx),
            (shown.clone(), false),
            "the button brings them back"
        );

        // The running one finishing keeps its row and gets its preview.
        host.take();
        let mut finished = shell.read_with(cx, |shell, _| shell.downloads.clone());
        finished[0].state = DownloadState::Done;
        shell.update(cx, |shell, cx| shell.set_downloads(finished, cx));
        cx.run_until_parked();
        assert_eq!(rows(&shell, cx), (shown.clone(), false));
        let calls = host.take();
        let previews: Vec<_> = calls
            .iter()
            .filter(|c| c.starts_with("file preview"))
            .collect();
        assert_eq!(previews, ["file preview 9"]);
        let mut running = shell.read_with(cx, |shell, _| shell.downloads.clone());
        running[0].state = DownloadState::Running;
        shell.update(cx, |shell, cx| shell.set_downloads(running, cx));

        // A later rest asks for no preview again.
        shell.update(cx, |shell, cx| shell.recent_downloads_gone(cx));
        move_to(cx, 120., 200.);
        move_to(cx, button.0, button.1);
        wait(cx, RECENT_WAIT);
        assert_eq!(rows(&shell, cx).0.len(), 4);
        assert!(
            host.take()
                .iter()
                .all(|call| !call.starts_with("file preview"))
        );

        cx.simulate_click(point(px(button.0), px(button.1)), Modifiers::none());
        cx.run_until_parked();
        assert!(
            rows(&shell, cx).0.is_empty(),
            "the Library takes them away at once"
        );
        let (open, archive) = shell.read_with(cx, |shell, _| {
            (shell.library_open, shell.section == Section::Archive)
        });
        assert!(open && archive, "a click opens the Library where it was");
    }

    /// A right-click on a row shows its menu, which takes the pointer as it comes on screen over
    /// them, and the rows stay while it's open for it to act on, then go once it closes with the
    /// pointer on neither them nor the button.
    #[gpui::test]
    fn the_recent_downloads_stay_under_a_rows_menu(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        let downloads = vec![done(2, HOUR), done(1, 2. * HOUR)];
        shell.update(cx, |shell, cx| shell.set_downloads(downloads, cx));
        let height = shell.read_with(cx, |shell, _| f32::from(shell.viewport.height));
        move_to(cx, 24., height - 25.);
        wait(cx, RECENT_WAIT);
        wait(
            cx,
            Duration::from_secs_f32(RECENT_STAGGER_IN + RECENT_FADE_IN),
        );
        assert_eq!(rows(&shell, cx).0.len(), 2);

        let row = point(px(120.), px(height - 50. - 28.));
        move_to(cx, 120., height - 50. - 28.);
        cx.simulate_mouse_down(row, MouseButton::Right, Modifiers::none());
        cx.simulate_mouse_up(row, MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
        let titles: Vec<_> = (host.menus.borrow().last().unwrap().iter())
            .map(|(title, _)| title.to_string())
            .collect();
        assert!(
            titles.iter().any(|title| title == "Move to Trash"),
            "{titles:?}"
        );
        move_to(cx, 120., 200.);
        let (kept, going) = rows(&shell, cx);
        assert!(kept.len() == 2 && !going, "the menu keeps them");

        cx.update(|_, cx| host.dismiss(cx));
        cx.run_until_parked();
        assert!(
            rows(&shell, cx).1,
            "they go as it closes, the pointer on neither"
        );
    }

    /// The rows come in from the bottom, each 25 ms after the one under it, fading in over 0.18 s
    /// as they rise 5 pt over the first 0.12 s. They go 70 ms after the pointer leaves, the bottom
    /// one first and the top one 50 ms later, each fading out over 0.13 s; one turned on its way
    /// turns at once.
    #[test]
    fn the_rows_come_up_from_the_bottom_and_go_bottom_first() {
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert_eq!(row_opacity(false, 0, 0., 0.), 0.);
        assert!(close(row_opacity(false, 0, 0., 0.09), 0.5));
        assert_eq!(row_opacity(false, 3, 0., 3. * RECENT_STAGGER_IN), 0.);
        assert!(close(
            row_opacity(false, 3, 0., 3. * RECENT_STAGGER_IN + 0.09),
            0.5
        ));
        assert_eq!(
            row_opacity(false, 3, 0., 3. * RECENT_STAGGER_IN + RECENT_FADE_IN + 1e-4),
            1.
        );
        assert_eq!(row_drop(0.), 5.);
        assert!(row_drop(0.5) > 0. && row_drop(0.5) < 0.2);
        assert_eq!(row_drop(2. / 3.), 0.);

        assert_eq!(row_opacity(true, 0, 1., RECENT_LINGER), 1.);
        assert!(close(
            row_opacity(true, 0, 1., RECENT_LINGER + RECENT_FADE_OUT / 2.),
            0.5
        ));
        let top = RECENT_LINGER + 3. * RECENT_STAGGER_OUT;
        assert!(close(top - RECENT_LINGER, 0.05));
        assert_eq!(row_opacity(true, 3, 1., top), 1.);
        assert_eq!(row_opacity(true, 3, 1., top + RECENT_FADE_OUT + 1e-4), 0.);
        // Turned at half: back in over half the fade, and out after the linger alone.
        assert!(close(row_opacity(false, 3, 0.5, RECENT_FADE_IN / 4.), 0.75));
        assert!(close(
            row_opacity(true, 3, 0.5, RECENT_LINGER + RECENT_FADE_OUT / 4.),
            0.25
        ));
    }

    /// A pointer passing over the button leaves no rows. A download running alone shows as one, and
    /// so does a held one, which Chrome keeps running until it's kept or deleted, whose row's
    /// click offers Keep and Delete.
    #[gpui::test]
    fn the_recent_downloads_need_a_rest_and_show_a_held_download(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        shell.update(cx, |shell, cx| shell.set_downloads(vec![done(1, HOUR)], cx));
        let height = shell.read_with(cx, |shell, _| f32::from(shell.viewport.height));
        move_to(cx, 24., height - 25.);
        wait(cx, RECENT_WAIT / 2);
        move_to(cx, 300., 200.);
        wait(cx, RECENT_WAIT * 2);
        assert!(
            rows(&shell, cx).0.is_empty(),
            "passing over the button shows nothing"
        );

        let mut running = done(2, 0.);
        running.state = DownloadState::Running;
        shell.update(cx, |shell, cx| {
            shell.set_downloads(vec![running.clone()], cx)
        });
        move_to(cx, 24., height - 25.);
        wait(cx, RECENT_WAIT * 2);
        assert_eq!(
            rows(&shell, cx).0,
            [(2, "just now".to_string())],
            "a running one shows"
        );

        move_to(cx, 300., 200.);
        wait(cx, RECENT_WAIT * 2);
        shell.update(cx, |shell, cx| shell.recent_downloads_gone(cx));
        running.held = true;
        shell.update(cx, |shell, cx| shell.set_downloads(vec![running], cx));
        move_to(cx, 24., height - 25.);
        wait(cx, RECENT_WAIT * 2);
        assert_eq!(
            rows(&shell, cx).0,
            [(2, "just now".to_string())],
            "a held one shows"
        );
        move_to(cx, 24., height - 45.);
        cx.simulate_click(point(px(120.), px(height - 50. - 28.)), Modifiers::none());
        cx.run_until_parked();
        let menu = host.menus.borrow().last().cloned().unwrap_or_default();
        let titles: Vec<_> = menu
            .iter()
            .take(2)
            .map(|(title, _)| title.to_string())
            .collect();
        assert_eq!(titles, ["Keep", "Delete"]);
    }
}
