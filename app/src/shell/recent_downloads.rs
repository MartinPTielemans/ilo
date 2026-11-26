//! The Library button's plate of recent downloads: with the pointer resting on the button, the
//! button turns into a plate that grows upward out of the foot, inside the sidebar, and lists the
//! last four downloads, newest first, a running one with how far it's got. Leaving it folds it
//! back (docs/ux-spec.md › Downloads).

use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::{sync::Arc, time::Duration};

use gpui::{
    AnyElement, BoxShadow, Context, FontWeight, Hsla, Image, ObjectFit, PathBuilder, Pixels, Point,
    SharedString, Task, Window, canvas, div, img, point, prelude::*, px, relative, rgba, svg,
};

use super::{
    Download, DownloadState, FOOTER_HEIGHT, Section, Shell, cubic_bezier, host,
    library::{LIBRARY_BACK_INSET, bytes, is_video},
};
use crate::palette::{Palette, Tone};
use crate::tokens::shell::recent_downloads as t;
use crate::tokens::shell::recent_downloads::motion::{EASE, FOLD, GROW, REST, ROWS_FROM};
use crate::tokens::shell::{FOOT_ROW, LIBRARY_BUTTON};

/// How long the pointer rests on the button before the plate opens, so one passing it opens none.
const RECENT_REST: Duration = crate::motion::ms(REST);
/// How many downloads the plate lists.
const RECENT_COUNT: usize = t::COUNT as usize;

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

/// The plate, and where the pointer is that keeps it.
#[derive(Default)]
pub(super) struct RecentDownloads {
    over_button: bool,
    over_plate: bool,
    /// The rest before it opens, from the pointer coming onto the button.
    wait: Option<Task<()>>,
    /// The downloads listed, newest first. Empty while the plate is shut.
    rows: Vec<u64>,
    /// It's folding back, and takes no pointer.
    going: bool,
    /// When it turned toward open or shut, from the first frame drawn after it did.
    turned: Option<Instant>,
    /// How far open it was as it turned.
    from: f32,
    /// Its motion has ended, and its line is logged.
    settled: bool,
}

impl RecentDownloads {
    /// Whether the Library button is the plate's foot: while any of the plate shows.
    pub(super) fn plated(&self) -> bool {
        !self.rows.is_empty()
    }
}

/// How far open the plate is, 0 shut to 1 open, `t` seconds after it turned toward going or coming
/// from `from`: the whole way open takes GROW, and the whole way shut FOLD.
fn opened(going: bool, from: f32, t: f32) -> f32 {
    match going {
        false => (from + t / GROW).min(1.),
        true => (from - t / FOLD).max(0.),
    }
}

/// How much of its size the plate has when it's `open`: on its ease, so it leaves the button fast
/// and settles. Shut and open are exact, which the curve's solver only comes close to.
fn grown(open: f32) -> f32 {
    match open {
        open if open <= 0. => 0.,
        open if open >= 1. => 1.,
        open => cubic_bezier(EASE[0], EASE[1], EASE[2], EASE[3], open),
    }
}

/// How strong the heading's ink is on the plate in a `tone` window.
fn heading_ink(tone: Tone) -> f32 {
    match tone {
        Tone::Dark => t::HEADING_INK,
        Tone::Light => t::LIGHT_HEADING_INK,
    }
}

/// The plate's colour in a `tone` window whose own look, opaque, is `ground`: the floating level's
/// fill over it. GPUI draws a box's shadow under the box too, so through the fill alone the plate
/// would be as much darker than the button's box under it, which has no shadow, and the tab list
/// would show through both.
fn plate(tone: Tone, ground: Hsla) -> Hsla {
    ground.blend(Hsla::from(rgba(match tone {
        Tone::Dark => crate::tokens::floating::FILL,
        Tone::Light => crate::tokens::floating::LIGHT_FILL,
    })))
}

/// The inks of what the plate says, where it's `plate`: a name's, a detail's and the heading's.
/// They lie on the plate, so their floor is its own, and a row's the plate's under the one the
/// row wears under the pointer. The window's floor answers for its material over any backdrop,
/// and on a dark window lifts the quiet inks all the way to a name's.
fn inks(palette: &Palette, plate: Hsla) -> [Hsla; 3] {
    let lit = plate.blend(palette.plate(t::ROW_HOVERED));
    [
        palette.text_on(t::NAME, lit),
        palette.text_on(palette.secondary(), lit),
        palette.text_on(heading_ink(palette.tone), plate),
    ]
}

/// The plate and the button's box under it as one shape: a body `width` by `height` over a box
/// `LIBRARY_BUTTON` by `FOOT_ROW` at its near side, with a curve taking the box's far side into
/// the body's foot. While the body is growing its corners are as round as it has room for.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Outline {
    width: f32,
    height: f32,
    /// The body's top corners and its far bottom one.
    top: f32,
    far: f32,
    /// The curve from the body's foot into the box's far side.
    curve: f32,
}

impl Outline {
    fn of(width: f32, height: f32) -> Self {
        let past = (width - LIBRARY_BUTTON).max(0.);
        let curve = t::CURVE.min(past / 2.);
        let top = t::RADIUS.min(width / 2.).min(height / 2.);
        Outline {
            width,
            height,
            top,
            far: top.min(past - curve),
            curve,
        }
    }

    /// Its line, `inset` inside its edge, from `origin`, the body's top near corner.
    fn trace(&self, path: &mut PathBuilder, origin: Point<Pixels>, inset: f32) {
        let at = |x: f32, y: f32| origin + point(px(x), px(y));
        // A corner `radius` round at the edge, turning with the line or against it.
        let turn = |path: &mut PathBuilder, radius: f32, with: bool, to: Point<Pixels>| match radius
            > 0.
        {
            true => path.arc_to(point(px(radius), px(radius)), px(0.), false, with, to),
            false => path.line_to(to),
        };
        let (right, foot) = (self.width - inset, self.height - inset);
        let (box_right, box_foot) = (LIBRARY_BUTTON - inset, self.height + FOOT_ROW - inset);
        let (top, far, curve, round) = (self.top, self.far, self.curve, t::STRIP_RADIUS);
        path.move_to(at(top, inset));
        path.line_to(at(self.width - top, inset));
        turn(path, top - inset, true, at(right, top));
        path.line_to(at(right, self.height - far));
        turn(path, far - inset, true, at(self.width - far, foot));
        path.line_to(at(LIBRARY_BUTTON + curve, foot));
        turn(
            path,
            curve + inset,
            false,
            at(box_right, self.height + curve),
        );
        path.line_to(at(box_right, self.height + FOOT_ROW - round));
        turn(
            path,
            round - inset,
            true,
            at(LIBRARY_BUTTON - round, box_foot),
        );
        path.line_to(at(round, box_foot));
        turn(
            path,
            round - inset,
            true,
            at(inset, self.height + FOOT_ROW - round),
        );
        path.line_to(at(inset, top));
        turn(path, top - inset, true, at(top, inset));
        path.close();
    }
}

/// How opaque the rows are when the plate is `open`: they come in over the second half of the way.
fn rows_shown(open: f32) -> f32 {
    ((open - ROWS_FROM) / (1. - ROWS_FROM)).clamp(0., 1.)
}

/// Whether the plate lists a download: a finished one with a file, and one still coming, as a
/// download that runs on shouldn't hide the files before it. A held one lists too, as nothing
/// else says it's waiting to be kept or deleted.
fn lists(download: &Download) -> bool {
    match download.state {
        DownloadState::Done => !download.path.is_empty(),
        DownloadState::Running | DownloadState::Paused => true,
        DownloadState::Cancelled | DownloadState::Failed => false,
    }
}

/// The downloads the plate lists, newest first.
fn recent(downloads: &[Download]) -> Vec<u64> {
    let mut files: Vec<_> = downloads.iter().filter(|each| lists(each)).collect();
    files.sort_by(|a, b| b.started.total_cmp(&a.started));
    files.truncate(RECENT_COUNT);
    files.iter().map(|each| each.id).collect()
}

/// How far a download still coming has got, 0 to 1, if its size is known and it isn't held.
fn progress(download: &Download) -> Option<f32> {
    let coming = matches!(
        download.state,
        DownloadState::Running | DownloadState::Paused
    );
    (coming && !download.held && download.total > 0)
        .then(|| (download.received as f32 / download.total as f32).clamp(0., 1.))
}

/// A number of a unit as the plate writes it: whole from 10 up and with one decimal under it, so
/// a small one still moves as its file comes, but never a decimal that is nought. One that rounds
/// up to 10 is written whole.
fn amount(of: f64) -> String {
    let tenths = (of * 10.).round();
    match tenths {
        t if t >= 100. => format!("{of:.0}"),
        t if t % 10. == 0. => format!("{:.0}", t / 10.),
        t => format!("{:.1}", t / 10.),
    }
}

/// How much of a download has come, beside its bar, where the Library's words for it don't fit:
/// both sizes in the whole one's unit, said once ("48 of 122 MB", "1.2 of 4.8 GB"). Kilobytes
/// and bytes are always whole.
fn come(received: i64, total: i64) -> String {
    if total <= 0 {
        return bytes(received);
    }
    let (received, total) = (received.clamp(0, total) as f64, total as f64);
    let (unit, name) = match total {
        t if t < 1e3 => return format!("{received:.0} of {t:.0} bytes"),
        t if t < 1e6 => return format!("{:.0} of {:.0} KB", received / 1e3, t / 1e3),
        t if t < 1e9 => (1e6, "MB"),
        _ => (1e9, "GB"),
    };
    format!(
        "{} of {} {name}",
        amount(received / unit),
        amount(total / unit)
    )
}

/// The glyph on the tile of a file with no picture: a page, or the warning triangle for one the
/// browser holds until it's kept or deleted.
fn tile_glyph(download: &Download) -> &'static str {
    match download.held {
        true => "icons/triangle-alert.svg",
        false => "icons/file.svg",
    }
}

/// What a row says under its file's name: a held one and a finished one as their rows in the
/// Library's Downloads do, and one still coming how much of it has. A paused one's bar says how
/// far it got, and "Paused" with the sizes doesn't fit beside it.
fn detail(download: &Download) -> SharedString {
    match download.state {
        _ if download.held => "May harm your computer".into(),
        DownloadState::Paused if download.total > 0 => "Paused".into(),
        DownloadState::Paused => format!("Paused · {}", bytes(download.received)).into(),
        DownloadState::Done => {
            format!("{} · {}", bytes(download.received), host(&download.url)).into()
        }
        _ => come(download.received, download.total).into(),
    }
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
    /// The pointer came onto the Library button, or the plate, or left. It leaves one as it comes
    /// onto the other in the same move, in either order, so what that means waits for both.
    pub(super) fn hover_recent_downloads(
        &mut self,
        plate: bool,
        hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match plate {
            true => self.recent_downloads.over_plate = hovered,
            false => self.recent_downloads.over_button = hovered,
        }
        cx.defer_in(window, |shell, _, cx| shell.settle_recent_downloads(cx));
    }

    /// Opens the plate once the pointer has rested on the button, keeps it while the pointer is on
    /// the button or it, and folds it back once it's on neither. The rest is one timer, started as
    /// the pointer comes and dropped as it leaves: nothing here runs while the pointer is elsewhere.
    pub(super) fn settle_recent_downloads(&mut self, cx: &mut Context<Self>) {
        // A row's menu takes the pointer from the plate as it comes on screen over it, and the
        // plate stays while it's open, as the reveal does, for it to act on. It settles as the
        // menu closes.
        if self.menu_open {
            return;
        }
        let recent = &self.recent_downloads;
        if recent.over_plate && !recent.going && !recent.rows.is_empty() {
            // check.sh reads this line, which says the pointer keeps it.
            log!("browser: recent downloads under the pointer");
        }
        if !recent.over_button && !(recent.over_plate && !recent.going) {
            self.recent_downloads.wait = None;
            return self.hide_recent_downloads(cx);
        }
        if !recent.rows.is_empty() {
            // Back on the button as it folds, it grows again from where it is.
            if recent.going {
                self.turn_recent_downloads(false, cx);
            }
            return;
        }
        if recent.wait.is_some() || !self.recent_downloads_can_show() {
            return;
        }
        self.recent_downloads.wait = Some(cx.spawn(async move |shell, cx| {
            cx.background_executor().timer(RECENT_REST).await;
            shell
                .update(cx, |shell, cx| {
                    shell.recent_downloads.wait = None;
                    shell.open_recent_downloads(cx);
                })
                .ok();
        }));
    }

    /// Whether the plate can open: the sidebar is still, docked or out in the reveal, with the
    /// Library closed.
    fn recent_downloads_can_show(&self) -> bool {
        let still = self.sidebar_motion.is_none() && self.library.is_none();
        let shown = !self.sidebar_hidden || self.revealed;
        still && shown && !self.page_fullscreen && self.recent_downloads.over_button
    }

    /// Lists the newest downloads, if there are any, and asks for the pictures it hasn't yet.
    fn open_recent_downloads(&mut self, cx: &mut Context<Self>) {
        if !self.recent_downloads_can_show() {
            return;
        }
        let rows = recent(&self.downloads);
        if rows.is_empty() {
            return;
        }
        let recent = &mut self.recent_downloads;
        recent.rows = rows;
        recent.going = false;
        recent.turned = None;
        recent.from = 0.;
        recent.settled = false;
        // The plate lists the download the bin holds.
        self.bin_leaves(cx);
        self.ask_download_previews(cx);
        cx.notify();
    }

    /// Asks once for the picture of each listed file that has finished and whose name says it's an
    /// image or a video: one still coming has no file to look at yet, and any other file's tile
    /// shows the page glyph.
    fn ask_download_previews(&mut self, cx: &mut Context<Self>) {
        let path = |id: u64| {
            let each = self.downloads.iter().find(|each| each.id == id)?;
            let finished = each.state == DownloadState::Done && !each.held && !each.path.is_empty();
            (finished && is_video(&each.name).is_some()).then(|| each.path.clone())
        };
        let asked: Vec<_> = (self.recent_downloads.rows.iter())
            .filter(|id| !self.download_previews.contains_key(id))
            .filter_map(|&id| Some((id, path(id)?)))
            .collect();
        for (id, path) in asked {
            self.download_previews.insert(id, None);
            self.browser.file_preview(id, &path, cx);
        }
    }

    /// Folds the plate back, from where it is, if it shows.
    pub(super) fn hide_recent_downloads(&mut self, cx: &mut Context<Self>) {
        if !self.recent_downloads.rows.is_empty() && !self.recent_downloads.going {
            self.turn_recent_downloads(true, cx);
        }
    }

    /// Takes the plate away at once, as what it shows over goes.
    pub(super) fn recent_downloads_gone(&mut self, cx: &mut Context<Self>) {
        let recent = &mut self.recent_downloads;
        recent.wait = None;
        recent.over_plate = false;
        if recent.rows.is_empty() {
            return;
        }
        recent.rows.clear();
        // check.sh waits for this line.
        log!("browser: recent downloads hidden");
        cx.notify();
    }

    /// Turns the plate toward folding back, or growing, from as far open as it is.
    fn turn_recent_downloads(&mut self, going: bool, cx: &mut Context<Self>) {
        let now = cx.background_executor().now();
        let from = self.recent_downloads_open(now, cx);
        let recent = &mut self.recent_downloads;
        recent.going = going;
        recent.settled = false;
        if going {
            recent.over_plate = false;
        }
        recent.from = from;
        // Now, or from the first frame if none has been drawn yet.
        recent.turned = recent.turned.map(|_| now);
        cx.notify();
    }

    /// How far open the plate is at `now` on the executor's clock, which a test moves on. Under
    /// Reduce Motion it's where it's going at once: it appears and goes.
    fn recent_downloads_open(&self, now: Instant, cx: &gpui::App) -> f32 {
        let recent = &self.recent_downloads;
        if crate::motion::still(cx) {
            return if recent.going { 0. } else { 1. };
        }
        let t = recent.turned.map_or(0., |turned| {
            now.saturating_duration_since(turned).as_secs_f32()
        });
        opened(recent.going, recent.from, t)
    }

    /// The downloads changed: the plate keeps those still listed, and goes with none.
    pub(super) fn recent_downloads_changed(&mut self, cx: &mut Context<Self>) {
        if self.recent_downloads.rows.is_empty() {
            return;
        }
        let downloads = &self.downloads;
        (self.recent_downloads.rows)
            .retain(|id| downloads.iter().any(|each| each.id == *id && lists(each)));
        if self.recent_downloads.rows.is_empty() {
            // check.sh waits for this line.
            log!("browser: recent downloads hidden");
        }
        self.ask_download_previews(cx);
        // A row still coming shows its bytes, which change with each report, and only while the
        // pointer keeps the plate.
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
            if in_media || self.recent_downloads.rows.contains(&id) {
                cx.notify();
            }
        }
    }

    /// Steps the plate's motion for a frame, and once it's done, says so, and lets the plate go if
    /// it folded. With the plate shut it returns at once, so a tab switch or a launch asks for no
    /// frame here.
    pub(super) fn step_recent_downloads(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.recent_downloads.rows.is_empty() || self.recent_downloads.settled {
            return;
        }
        let now = cx.background_executor().now();
        self.recent_downloads.turned.get_or_insert(now);
        let end = if self.recent_downloads.going { 0. } else { 1. };
        if self.recent_downloads_open(now, cx) != end {
            window.request_animation_frame();
            return;
        }
        let recent = &mut self.recent_downloads;
        recent.settled = true;
        recent.from = end;
        // check.sh waits for these lines, logged with the frame that ends the motion.
        if recent.going {
            recent.rows.clear();
            log!("browser: recent downloads hidden");
        } else {
            log!("browser: recent downloads shown {}", recent.rows.len());
        }
    }

    /// The plate where it's all open: as wide as the sidebar's rows, and as tall as its heading
    /// and rows inside its padding.
    fn recent_downloads_size(&self) -> (f32, f32) {
        let rows = self.recent_downloads.rows.len() as f32;
        (
            self.width() - 2. * LIBRARY_BACK_INSET,
            2. * t::PADDING + t::HEADING + rows * t::ROW,
        )
    }

    /// The plate, drawn by the sidebar inside its own bounds, under the Library button: a menu's
    /// shadow, then the plate and the button's own box as one shape in its fill with its line
    /// along the whole of it, and the body with what it lists, growing upward from the box. While
    /// it shows it takes the tab list's pointer.
    pub(super) fn recent_download_rows(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let recent = &self.recent_downloads;
        if recent.rows.is_empty() || self.page_fullscreen {
            return Vec::new();
        }
        let live = !recent.going;
        let open = self.recent_downloads_open(cx.background_executor().now(), cx);
        let (size, shown) = (grown(open), rows_shown(open));
        let tone = self.palette.tone;
        // The window's look at its foot, where the plate lies.
        let fill = plate(tone, self.window_fill_at(1.));
        let rim = Hsla::from(rgba(match tone {
            Tone::Dark => crate::tokens::floating::RIM,
            Tone::Light => crate::tokens::floating::LIGHT_RIM,
        }));
        let [_, _, heading_ink] = inks(&self.palette, fill);
        let (full_width, full_height) = self.recent_downloads_size();
        let width = LIBRARY_BUTTON + (full_width - LIBRARY_BUTTON) * size;
        let height = full_height * size;
        let outline = Outline::of(width, height);
        let button_top = (FOOTER_HEIGHT + FOOT_ROW) / 2.;
        let rows: Vec<_> = (recent.rows.iter())
            .map(|&id| self.recent_row(id, live, cx))
            .collect();
        let heading = div()
            .flex_none()
            .h(px(t::HEADING))
            .px(px(t::ROW_INSET))
            .flex()
            .items_center()
            .text_size(px(t::DETAIL_SIZE))
            .line_height(px(t::DETAIL_LINE))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(heading_ink)
            .child("Downloads");
        // The body's box, which the shadow lies under and what it lists is cut to.
        let boxed = || {
            div()
                .absolute()
                .left(px(LIBRARY_BACK_INSET))
                .bottom(px(button_top))
                .w(px(width))
                .h(px(height))
                .rounded_tl(px(outline.top))
                .rounded_tr(px(outline.top))
                .rounded_br(px(outline.far))
        };
        // What floats' shadows, which come with the plate as the shape does.
        let shadows: Vec<_> = (crate::elevation::menu_shadow().into_iter())
            .map(|shadow| BoxShadow {
                color: shadow.color.opacity(size),
                ..shadow
            })
            .collect();
        let shadow = boxed().shadow(shadows);
        // One shape, so the fill has no seam where the body meets the box and the line runs
        // round both: GPUI's borders follow one box each.
        let shape = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let mut filled = PathBuilder::fill();
                outline.trace(&mut filled, bounds.origin, 0.);
                if let Ok(filled) = filled.build() {
                    window.paint_path(filled, fill.opacity(size));
                }
                let mut line = PathBuilder::stroke(px(t::LINE));
                outline.trace(&mut line, bounds.origin, t::LINE / 2.);
                if let Ok(line) = line.build() {
                    window.paint_path(line, rim.opacity(size));
                }
            },
        )
        .absolute()
        .left(px(LIBRARY_BACK_INSET))
        .bottom(px((FOOTER_HEIGHT - FOOT_ROW) / 2.))
        .w(px(width))
        .h(px(height + FOOT_ROW));
        let body = boxed()
            .id("recent-downloads")
            .overflow_hidden()
            .when(live, |body| {
                body.block_mouse_except_scroll().on_hover(cx.listener(
                    |shell, hovered: &bool, window, cx| {
                        shell.hover_recent_downloads(true, *hovered, window, cx)
                    },
                ))
            })
            // Laid out as it is all open, from its top: the body shows as much as it has grown.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(full_width))
                    .h(px(full_height))
                    .p(px(t::PADDING))
                    .flex()
                    .flex_col()
                    .child(heading)
                    .child(div().flex().flex_col().opacity(shown).children(rows)),
            )
            .children(live.then(|| self.download_press_listeners(cx)));
        vec![
            shadow.into_any_element(),
            shape.into_any_element(),
            body.into_any_element(),
        ]
    }

    /// A row: the file's picture, or the page glyph on a tile, then its name over how much of it
    /// has come, with a bar while it's coming, or its size and where it's from; and while the
    /// plate shows, what its row in the Library does.
    fn recent_row(&self, id: u64, live: bool, cx: &mut Context<Self>) -> AnyElement {
        let Some(download) = self.downloads.iter().find(|each| each.id == id) else {
            return div().flex_none().h(px(t::ROW)).into_any_element();
        };
        let tile = div()
            .flex_none()
            .size(px(t::TILE))
            .rounded(px(t::TILE_RADIUS));
        let tile = match self.download_previews.get(&id).cloned().flatten() {
            Some(preview) if preview.photo => tile.overflow_hidden().child(
                img(preview.image)
                    .size(px(t::TILE))
                    .rounded(px(t::TILE_RADIUS))
                    .object_fit(ObjectFit::Cover),
            ),
            _ => tile
                .flex()
                .items_center()
                .justify_center()
                .bg(self.palette.plate(t::TILE_FILL))
                .child(
                    svg()
                        .path(tile_glyph(download))
                        .size(px(t::TILE_GLYPH))
                        .text_color(self.palette.glyph(t::TILE_GLYPH_INK)),
                ),
        };
        let on = plate(self.palette.tone, self.window_fill_at(1.));
        let [name_ink, quiet, _] = inks(&self.palette, on);
        let line = |text: SharedString| {
            div()
                .min_w_0()
                .h(px(t::DETAIL_LINE))
                .flex()
                .items_center()
                .text_size(px(t::DETAIL_SIZE))
                .text_color(quiet)
                .child(div().truncate().child(text))
        };
        let under = match progress(download) {
            Some(done) => div()
                .mt(px(t::BAR_ABOVE))
                .flex()
                .items_center()
                .gap(px(t::BAR_GAP))
                .child(
                    div()
                        .flex_none()
                        .w(px(t::BAR_WIDTH))
                        .h(px(t::BAR_HEIGHT))
                        .rounded(px(t::BAR_HEIGHT / 2.))
                        .bg(self.palette.faint(t::BAR_TRACK))
                        .child(
                            div()
                                .h_full()
                                .w(relative(done))
                                .rounded(px(t::BAR_HEIGHT / 2.))
                                .bg(self.palette.glyph(t::BAR_FILL)),
                        ),
                )
                .child(line(detail(download))),
            None => div()
                .mt(px(t::DETAIL_ABOVE))
                .flex()
                .child(line(detail(download))),
        };
        let name = div()
            .h(px(t::NAME_LINE))
            .flex()
            .items_center()
            .text_size(px(t::NAME_SIZE))
            .font_weight(FontWeight::MEDIUM)
            .text_color(name_ink)
            .child(div().truncate().child(download.name.clone()));
        let row = div()
            .id(("recent-download", id))
            .flex_none()
            .h(px(t::ROW))
            .px(px(t::ROW_INSET))
            .flex()
            .items_center()
            .gap(px(t::ROW_GAP))
            .rounded(px(t::ROW_RADIUS));
        let row = match live {
            true => {
                let lit = self.palette.plate(t::ROW_HOVERED);
                self.download_row(row, id, "recent downloads", lit, cx)
            }
            false => row,
        };
        row.child(tile)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(name)
                    .child(under),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use std::{rc::Rc, time::Duration};

    use gpui::{
        Entity, Hsla, Modifiers, MouseButton, Rgba, TestAppContext, VisualTestContext, black,
        point, px, white,
    };

    use super::{
        super::{
            Captured, Download, DownloadState, FOOTER_HEIGHT, Shell,
            library::{LIBRARY_BACK_INSET, Section},
            recorder::Recorder,
        },
        DAY, HOUR, MINUTE, Outline, RECENT_REST, WEEK, age, come, detail, grown, heading_ink, inks,
        now, opened, plate, rows_shown, t, tile_glyph,
    };
    use crate::palette::{Palette, TEXT, Tone, contrast, over};
    use crate::tokens::shell::recent_downloads::motion::{FOLD, GROW, ROWS_FROM};
    use crate::tokens::shell::{FOOT_ROW, LIBRARY_BUTTON};

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

    fn move_to(cx: &mut VisualTestContext, at: (f32, f32)) {
        cx.simulate_mouse_move(point(px(at.0), px(at.1)), None, Modifiers::none());
        cx.run_until_parked();
    }

    fn wait(cx: &mut VisualTestContext, time: Duration) {
        cx.executor().advance_clock(time);
        cx.run_until_parked();
    }

    /// A frame drawn, which steps the plate's motion.
    fn frame(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }

    /// The downloads listed, newest first, and whether the plate is folding back.
    fn rows(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (Vec<u64>, bool) {
        shell.read_with(cx, |shell, _| {
            let recent = &shell.recent_downloads;
            (recent.rows.clone(), recent.going)
        })
    }

    /// The middle of the Library button, and of row `index` of the `count` the plate lists, the
    /// newest at its top; and a point over the page, on neither.
    fn button(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (f32, f32) {
        let height = shell.read_with(cx, |shell, _| f32::from(shell.viewport.height));
        (
            LIBRARY_BACK_INSET + LIBRARY_BUTTON / 2.,
            height - FOOTER_HEIGHT / 2.,
        )
    }

    fn row(
        shell: &Entity<Shell>,
        cx: &mut VisualTestContext,
        count: usize,
        index: usize,
    ) -> (f32, f32) {
        let height = shell.read_with(cx, |shell, _| f32::from(shell.viewport.height));
        let plate = 2. * t::PADDING + t::HEADING + count as f32 * t::ROW;
        let top = height - (FOOTER_HEIGHT + FOOT_ROW) / 2. - plate;
        (
            120.,
            top + t::PADDING + t::HEADING + (index as f32 + 0.5) * t::ROW,
        )
    }

    const AWAY: (f32, f32) = (400., 300.);

    /// The plate opens only once the pointer has rested on the button, and a pointer passing over
    /// it starts nothing that outlives its leaving. With no downloads, or only ones that were
    /// cancelled or failed, it doesn't open at all.
    #[gpui::test]
    fn the_plate_opens_after_the_rest_and_never_with_none(cx: &mut TestAppContext) {
        let (shell, cx, _) = window(cx);
        let waits = |cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, _| shell.recent_downloads.wait.is_some())
        };
        let at = button(&shell, cx);
        assert!(!waits(cx), "nothing waits at launch");

        move_to(cx, at);
        wait(cx, RECENT_REST * 2);
        assert!(rows(&shell, cx).0.is_empty(), "it opens with no downloads");
        move_to(cx, AWAY);

        let mut cancelled = done(1, MINUTE);
        cancelled.state = DownloadState::Cancelled;
        let mut failed = done(2, HOUR);
        failed.state = DownloadState::Failed;
        shell.update(cx, |shell, cx| {
            shell.set_downloads(vec![cancelled, failed], cx)
        });
        move_to(cx, at);
        wait(cx, RECENT_REST * 2);
        assert!(
            rows(&shell, cx).0.is_empty(),
            "it opens with none that lists"
        );
        move_to(cx, AWAY);

        shell.update(cx, |shell, cx| shell.set_downloads(vec![done(3, HOUR)], cx));
        cx.run_until_parked();
        assert!(!waits(cx), "a download wakes nothing by itself");
        move_to(cx, at);
        assert!(waits(cx), "the rest starts as the pointer comes");
        wait(cx, RECENT_REST / 2);
        move_to(cx, AWAY);
        assert!(!waits(cx), "and is dropped as it leaves");
        wait(cx, RECENT_REST * 2);
        assert!(
            rows(&shell, cx).0.is_empty(),
            "passing over the button opens it"
        );

        move_to(cx, at);
        wait(cx, RECENT_REST - Duration::from_millis(10));
        assert!(rows(&shell, cx).0.is_empty(), "it opens before the rest");
        wait(cx, Duration::from_millis(10));
        assert_eq!(rows(&shell, cx), (vec![3], false));
        assert!(!waits(cx), "the rest is one shot");
    }

    /// It lists the last four downloads and nothing else, newest first whatever order the core
    /// gives them in: those still coming and a held one among the finished, none that was
    /// cancelled, and none of the Library's captures. It asks once for the picture of each
    /// finished file that is one, and for a running one's once it has finished. A click on the
    /// button still opens the Library where it was, and takes the plate away at once.
    #[gpui::test]
    fn the_plate_lists_the_last_four_downloads_newest_first(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        let mut running = done(9, 0.);
        (running.state, running.name) = (DownloadState::Running, "clip.mp4".into());
        (running.received, running.total) = (4, 10);
        let mut held = done(8, MINUTE);
        (held.state, held.held) = (DownloadState::Running, true);
        let mut cancelled = done(7, 2. * MINUTE);
        cancelled.state = DownloadState::Cancelled;
        let mut picture = done(5, 2. * HOUR);
        picture.name = "picture.png".into();
        let downloads = vec![
            done(1, 10. * DAY),
            done(4, 4. * DAY),
            picture,
            done(2, 9. * DAY),
            cancelled,
            running,
            done(3, 8. * DAY),
            held,
        ];
        let capture = Captured {
            path: "/media/Capture.png".into(),
            name: "Capture.png".into(),
            saved: now() - 10_000.,
        };
        shell.update(cx, |shell, cx| {
            shell.set_downloads(downloads, cx);
            shell.set_captures(vec![capture], cx);
        });
        cx.run_until_parked();
        let at = button(&shell, cx);
        host.take();

        move_to(cx, at);
        wait(cx, RECENT_REST);
        assert_eq!(rows(&shell, cx), (vec![9, 8, 5, 4], false));
        let calls = host.take();
        assert!(
            calls.iter().any(|call| call == "tooltip View Library ⇧⌘L"),
            "{calls:?}"
        );
        let asked: Vec<_> = (calls.iter())
            .filter(|call| !call.starts_with("tooltip"))
            .collect();
        assert_eq!(asked, ["file preview 5"], "only the finished picture's");

        // The running one finishing keeps its row and gets its picture.
        let mut finished = shell.read_with(cx, |shell, _| shell.downloads.clone());
        let clip = finished.iter_mut().find(|each| each.id == 9).unwrap();
        clip.state = DownloadState::Done;
        shell.update(cx, |shell, cx| shell.set_downloads(finished, cx));
        cx.run_until_parked();
        assert_eq!(rows(&shell, cx), (vec![9, 8, 5, 4], false));
        let previews: Vec<_> = (host.take().into_iter())
            .filter(|call| call.starts_with("file preview"))
            .collect();
        assert_eq!(previews, ["file preview 9"]);

        // A later rest asks for none again.
        shell.update(cx, |shell, cx| shell.recent_downloads_gone(cx));
        move_to(cx, AWAY);
        move_to(cx, at);
        wait(cx, RECENT_REST);
        assert_eq!(rows(&shell, cx).0.len(), 4);
        assert!(
            (host.take().iter()).all(|call| !call.starts_with("file preview")),
            "a picture is asked for once"
        );

        cx.simulate_click(point(px(at.0), px(at.1)), Modifiers::none());
        cx.run_until_parked();
        assert!(
            rows(&shell, cx).0.is_empty(),
            "the Library takes the plate away at once"
        );
        let (open, archive) = shell.read_with(cx, |shell, _| {
            (shell.library_open, shell.section == Section::Archive)
        });
        assert!(open && archive, "a click opens the Library where it was");
    }

    /// A click on the button opens the Library with no plate up too: before the rest is over,
    /// which the click ends, and with nothing to list.
    #[gpui::test]
    fn a_click_on_the_button_opens_the_library_with_no_plate_up(cx: &mut TestAppContext) {
        let (shell, cx, _) = window(cx);
        shell.update(cx, |shell, cx| shell.set_downloads(vec![done(1, HOUR)], cx));
        let at = button(&shell, cx);
        move_to(cx, at);
        cx.simulate_click(point(px(at.0), px(at.1)), Modifiers::none());
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| shell.library_open));
        wait(cx, RECENT_REST * 2);
        assert!(
            rows(&shell, cx).0.is_empty(),
            "no plate opens over the Library"
        );
    }

    /// The plate stays as the pointer crosses from the button onto it, folds back once the pointer
    /// is on neither, grows again from the button while it folds, and is gone when its fold ends.
    #[gpui::test]
    fn the_plate_stays_under_the_pointer_and_folds_back_as_it_leaves(cx: &mut TestAppContext) {
        let (shell, cx, _) = window(cx);
        let downloads = vec![done(2, HOUR), done(1, 2. * HOUR)];
        shell.update(cx, |shell, cx| shell.set_downloads(downloads, cx));
        let at = button(&shell, cx);
        move_to(cx, at);
        wait(cx, RECENT_REST);
        frame(&shell, cx);
        wait(cx, Duration::from_secs_f32(GROW));
        frame(&shell, cx);
        assert!(
            shell.read_with(cx, |shell, _| shell.recent_downloads.settled),
            "it's open after its growth"
        );

        // Up from the button onto the plate's foot, then its rows, the oldest and the newest.
        move_to(cx, (at.0, at.1 - FOOT_ROW / 2. - 2.));
        assert_eq!(rows(&shell, cx), (vec![2, 1], false), "its foot keeps it");
        let (oldest, newest) = (row(&shell, cx, 2, 1), row(&shell, cx, 2, 0));
        move_to(cx, oldest);
        move_to(cx, newest);
        assert_eq!(rows(&shell, cx), (vec![2, 1], false), "its rows keep it");

        move_to(cx, AWAY);
        assert_eq!(
            rows(&shell, cx),
            (vec![2, 1], true),
            "it folds with the pointer on neither"
        );
        // Folding, it takes no pointer: only the button brings it back.
        move_to(cx, newest);
        assert!(rows(&shell, cx).1);
        move_to(cx, at);
        assert_eq!(
            rows(&shell, cx),
            (vec![2, 1], false),
            "the button brings it back"
        );

        move_to(cx, AWAY);
        frame(&shell, cx);
        wait(cx, Duration::from_secs_f32(FOLD + GROW));
        frame(&shell, cx);
        assert!(
            rows(&shell, cx).0.is_empty(),
            "it's gone when its fold ends"
        );
        frame(&shell, cx);
        assert!(
            shell.read_with(cx, |shell, _| shell.recent_downloads.wait.is_none()),
            "and nothing waits once it's gone"
        );
    }

    /// A right-click on a row shows its menu, which takes the pointer as it comes on screen over
    /// the plate, and the plate stays while it's open for it to act on, then folds back once it
    /// closes with the pointer on neither it nor the button.
    #[gpui::test]
    fn the_plate_stays_under_a_rows_menu(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        let downloads = vec![done(2, HOUR), done(1, 2. * HOUR)];
        shell.update(cx, |shell, cx| shell.set_downloads(downloads, cx));
        let at = button(&shell, cx);
        move_to(cx, at);
        wait(cx, RECENT_REST);
        frame(&shell, cx);
        wait(cx, Duration::from_secs_f32(GROW));
        frame(&shell, cx);
        assert_eq!(rows(&shell, cx).0.len(), 2);

        let newest = row(&shell, cx, 2, 0);
        move_to(cx, newest);
        let on = point(px(newest.0), px(newest.1));
        cx.simulate_mouse_down(on, MouseButton::Right, Modifiers::none());
        cx.simulate_mouse_up(on, MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
        let titles: Vec<_> = (host.menus.borrow().last().unwrap().iter())
            .map(|(title, _)| title.to_string())
            .collect();
        assert!(
            titles.iter().any(|title| title == "Move to Trash"),
            "{titles:?}"
        );
        move_to(cx, AWAY);
        let (kept, going) = rows(&shell, cx);
        assert!(kept.len() == 2 && !going, "the menu keeps it");

        cx.update(|_, cx| host.dismiss(cx));
        cx.run_until_parked();
        assert!(
            rows(&shell, cx).1,
            "it folds as the menu closes, the pointer on neither"
        );
    }

    /// A download running alone lists, and so does a held one, which Chrome keeps running until
    /// it's kept or deleted, whose row's click offers Keep and Delete, as its row in the Library
    /// does.
    #[gpui::test]
    fn the_plate_lists_a_held_download_whose_row_offers_keep_and_delete(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        let mut running = done(2, 0.);
        running.state = DownloadState::Running;
        shell.update(cx, |shell, cx| {
            shell.set_downloads(vec![running.clone()], cx)
        });
        let at = button(&shell, cx);
        move_to(cx, at);
        wait(cx, RECENT_REST);
        assert_eq!(rows(&shell, cx).0, [2], "a running one lists");

        shell.update(cx, |shell, cx| shell.recent_downloads_gone(cx));
        move_to(cx, AWAY);
        running.held = true;
        shell.update(cx, |shell, cx| shell.set_downloads(vec![running], cx));
        move_to(cx, at);
        wait(cx, RECENT_REST);
        frame(&shell, cx);
        wait(cx, Duration::from_secs_f32(GROW));
        frame(&shell, cx);
        assert_eq!(rows(&shell, cx).0, [2], "a held one lists");
        let only = row(&shell, cx, 1, 0);
        move_to(cx, only);
        cx.simulate_click(point(px(only.0), px(only.1)), Modifiers::none());
        cx.run_until_parked();
        let menu = host.menus.borrow().last().cloned().unwrap_or_default();
        let titles: Vec<_> = (menu.iter().take(2))
            .map(|(title, _)| title.to_string())
            .collect();
        assert_eq!(titles, ["Keep", "Delete"]);
    }

    /// Under Reduce Motion the plate appears, open, after the same rest, and goes at once.
    #[gpui::test]
    fn under_reduce_motion_the_plate_appears_and_goes(cx: &mut TestAppContext) {
        let (shell, cx, _) = window(cx);
        cx.update(|_, cx| cx.set_reduce_motion(true));
        shell.update(cx, |shell, cx| shell.set_downloads(vec![done(1, HOUR)], cx));
        let at = button(&shell, cx);
        let open = |cx: &mut VisualTestContext| {
            shell.read_with(cx, |shell, cx| {
                shell.recent_downloads_open(cx.background_executor().now(), cx)
            })
        };
        move_to(cx, at);
        wait(cx, RECENT_REST - Duration::from_millis(10));
        assert!(rows(&shell, cx).0.is_empty(), "the rest is kept");
        wait(cx, Duration::from_millis(10));
        assert_eq!(rows(&shell, cx), (vec![1], false));
        assert_eq!(open(cx), 1., "it's open on its first frame");
        frame(&shell, cx);
        assert!(shell.read_with(cx, |shell, _| shell.recent_downloads.settled));

        // No clock moves between the pointer leaving and the plate being gone.
        move_to(cx, AWAY);
        frame(&shell, cx);
        assert!(rows(&shell, cx).0.is_empty(), "it goes at once");
    }

    /// It grows from the button over GROW and folds back over FOLD, each from as far open as it is
    /// when it turns; its size leaves the button fast and settles, and its rows come in over the
    /// second half of the way.
    #[test]
    fn the_plate_grows_out_of_the_button_and_folds_back() {
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        assert_eq!(opened(false, 0., 0.), 0.);
        assert!(close(opened(false, 0., GROW / 2.), 0.5));
        assert_eq!(opened(false, 0., GROW * 2.), 1.);
        assert_eq!(opened(true, 1., 0.), 1.);
        assert!(close(opened(true, 1., FOLD / 2.), 0.5));
        assert_eq!(opened(true, 1., FOLD * 2.), 0.);
        // Turned half open: a quarter of either way on from there.
        assert!(close(opened(false, 0.5, GROW / 4.), 0.75));
        assert!(close(opened(true, 0.5, FOLD / 4.), 0.25));
        assert!(FOLD < GROW, "it folds back faster than it grows");

        assert_eq!((grown(0.), grown(1.)), (0., 1.));
        assert!(grown(0.5) > 0.8, "most of its size by half its time");
        assert!((1..10).all(|step| grown(step as f32 / 10.) > grown((step - 1) as f32 / 10.)));

        assert_eq!(rows_shown(0.), 0.);
        assert_eq!(rows_shown(ROWS_FROM), 0.);
        assert!(close(rows_shown((ROWS_FROM + 1.) / 2.), 0.5));
        assert_eq!(rows_shown(1.), 1.);
    }

    /// A file with no picture has a page on its tile, and one the browser holds the warning
    /// triangle, so the row that asks something of the user is told from the rest before it's read.
    #[test]
    fn a_held_files_tile_wears_the_warning_triangle() {
        let mut download = done(1, HOUR);
        assert_eq!(tile_glyph(&download), "icons/file.svg");
        download.state = DownloadState::Running;
        assert_eq!(tile_glyph(&download), "icons/file.svg");
        download.held = true;
        assert_eq!(tile_glyph(&download), "icons/triangle-alert.svg");
    }

    /// A row says what its row in the Library's Downloads does, but one still coming, whose words
    /// there don't fit beside its bar: both sizes in the whole one's unit, said once, each whole
    /// from 10 up and with one decimal under it, never a nought after the point, and kilobytes
    /// and bytes always whole.
    #[test]
    fn a_row_says_how_much_has_come_in_one_unit() {
        for (received, total, says) in [
            (48_200_000, 121_500_000, "48 of 122 MB"),
            (1_200_000_000, 4_800_000_000, "1.2 of 4.8 GB"),
            (0, 4_800_000_000, "0 of 4.8 GB"),
            (4_000_000_000, 4_800_000_000, "4 of 4.8 GB"),
            (2_000, 4_000, "2 of 4 KB"),
            (12_300_000_000, 48_700_000_000, "12 of 49 GB"),
            (2_400_000, 122_000_000, "2.4 of 122 MB"),
            (4_040_000, 122_000_000, "4 of 122 MB"),
            (9_940_000, 10_000_000, "9.9 of 10 MB"),
            (9_960_000, 122_000_000, "10 of 122 MB"),
            (2_400, 9_400, "2 of 9 KB"),
            (300, 900, "300 of 900 bytes"),
            (5, 9, "5 of 9 bytes"),
        ] {
            assert_eq!(come(received, total), says);
        }
        assert_eq!(come(5_000, 4_000), "4 of 4 KB", "never more than there is");
        assert_eq!(
            come(1_200_000, 0),
            "1.2 MB",
            "a size not known says what has come"
        );

        let mut download = done(1, HOUR);
        (download.received, download.total) = (18_400, 18_400);
        assert_eq!(detail(&download), "18 KB · example.org");
        (download.state, download.received) = (DownloadState::Running, 9_000);
        assert_eq!(detail(&download), "9 of 18 KB");
        download.state = DownloadState::Paused;
        assert_eq!(detail(&download), "Paused", "its bar says how far");
        download.total = 0;
        assert_eq!(
            detail(&download),
            "Paused · 9 KB",
            "with no bar, what has come"
        );
        download.total = 18_400;
        (download.state, download.held) = (DownloadState::Running, true);
        assert_eq!(detail(&download), "May harm your computer");
    }

    /// What the plate says reads on it, the floating fill over the sidebar, in both looks: a name
    /// at 7:1, and its detail and the heading, small text both, at the text's floor, as all of a
    /// row is on the plate it wears under the pointer. The heading's ink is its token's there,
    /// which no floor lifts. Over a theme the sidebar's own look is another colour, and the floors
    /// hold where it's as dark and as light as a colour can be.
    #[test]
    fn the_plates_text_reads_on_its_fill() {
        let mut short = Vec::new();
        for tone in [Tone::Dark, Tone::Light] {
            let palette = Palette::of(None, tone);
            let on = |ground: Hsla, ink: Hsla| {
                let (ground, ink) = (Rgba::from(ground), Rgba::from(ink));
                contrast(over(ground, ink, ink.a), ground)
            };
            let sidebar = Hsla::from(palette.underlay);
            for (ground, most) in [(sidebar, 7.), (black(), TEXT), (white(), TEXT)] {
                let plate = plate(tone, ground);
                let [name, quiet, heading] = inks(&palette, plate);
                let tile = plate.blend(palette.plate(t::TILE_FILL));
                let lit = plate.blend(palette.plate(t::ROW_HOVERED));
                for (what, read, floor) in [
                    ("a name", on(plate, name), most),
                    ("a detail", on(plate, quiet), TEXT),
                    ("the heading", on(plate, heading), TEXT),
                    (
                        "the page glyph",
                        on(tile, palette.glyph(t::TILE_GLYPH_INK)),
                        3.,
                    ),
                    ("a name on a row under the pointer", on(lit, name), most),
                    ("a detail on a row under the pointer", on(lit, quiet), TEXT),
                ] {
                    if read < floor {
                        short.push(format!("{tone:?}: {what} reads at {read:.2}"));
                    }
                }
            }
            let [_, _, heading] = inks(&palette, plate(tone, sidebar));
            assert_eq!(heading.a, heading_ink(tone), "{tone:?}");
        }
        assert!(short.is_empty(), "{}", short.join("\n"));
    }

    /// The plate's corners are a panel's once it's open, and never rounder than it has room for
    /// on the way: none while it's no bigger than the button, and the curve into the button's
    /// side never past the middle of what runs on beyond it.
    #[test]
    fn the_outlines_corners_fit_the_plate_as_it_grows() {
        let open = Outline::of(224., 200.);
        assert_eq!(
            (open.top, open.far, open.curve),
            (t::RADIUS, t::RADIUS, t::CURVE)
        );
        let shut = Outline::of(LIBRARY_BUTTON, 0.);
        assert_eq!((shut.top, shut.far, shut.curve), (0., 0., 0.));
        for step in 0..=20 {
            let size = step as f32 / 20.;
            let (width, height) = (LIBRARY_BUTTON + (224. - LIBRARY_BUTTON) * size, 200. * size);
            let outline = Outline::of(width, height);
            assert!(2. * outline.top <= width.min(height) + 1e-3);
            assert!(outline.top + outline.far <= height + 1e-3);
            assert!(outline.far + outline.curve <= width - LIBRARY_BUTTON + 1e-3);
            assert!(outline.far >= 0. && outline.curve >= 0.);
        }
        assert!(
            t::CURVE <= FOOT_ROW - t::STRIP_RADIUS,
            "the curve ends above the box's foot"
        );
        assert!(2. * t::STRIP_RADIUS <= LIBRARY_BUTTON.min(FOOT_ROW));
    }
}
