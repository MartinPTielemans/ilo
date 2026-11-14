//! The link preview: a link clicked in a pinned tab or a favorite, to another site than the tab's,
//! opens over the window in a card rather than taking the tab away from its own site (Arc's Peek,
//! docs/ux-spec.md › Phase 4 features). The page is a real one, in a tab of no Space, in its
//! native view, which the host puts over this window's card: the page is the card, with the site
//! and the buttons floating over the dim above it. ⌘↩ or the keep button keeps it as a Today tab,
//! as it is. GPUI can't draw over the page's native view, so the dim and the card are a window of
//! their own that the host shows over the browser window, as its child, as the dialog's are, and
//! nothing of the preview's own lies over the page. Where no window of the app's can hold a page,
//! as on Linux, the page shows in the browser window under this one, which leaves it a hole and
//! draws the card's rim round it. What it asks of its window and the core goes through `Host`.

use std::{rc::Rc, sync::Arc, time::Duration};
use web_time::Instant;

use gpui::{
    AnyElement, App, Bounds, BoxShadow, Context, FocusHandle, FontWeight, Global, Hsla, Image,
    KeyBinding, MouseButton, PathBuilder, Pixels, Point, SharedString, Size, Task, TitlebarOptions,
    Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions,
    accesskit::Role, actions, canvas, div, fill, img, point, prelude::*, px, rgb, size, svg,
};

use crate::elevation::Level;
use crate::icon_picker::outline;
use crate::palette::{self, Palette, STATE_HOVERED, STATE_PRESSED};
use crate::shell::{cubic_bezier, keystroke_keys, scaled::scaled};
use crate::tokens::dialog::DIM;
use crate::tokens::link_preview::{
    BUTTON_GAP, CONTROLS, CONTROLS_GAP, DIM_OPACITY, GLYPH, INSET_MAX, INSET_MIN, INSET_SHARE,
    RADIUS, RIM, SITE_END, SITE_GAP, SITE_X,
};
use crate::tokens::shell::{FAVICON_DRAWN, NAV, SECONDARY};
use crate::tokens::text::BODY;
use crate::tooltip::{Anchor, Tip};

actions!(link_preview, [Close, Keep]);

/// The controls' row over the card: their height and the gap under them.
const OVER_CARD: f32 = CONTROLS + CONTROLS_GAP;

/// The dialog's measured motion, from the link: it grows in over 0.275 s from half its size,
/// opaque after the first 0.135 s, as the window dims. It shrinks back into the link in 0.2 s,
/// as fast as an overlay grows in, and grows into the tab's place in 0.275 s. A task's page watched
/// in the page area zooms out of its thumbnail and back on the same curves (shell/task_area.rs).
pub(crate) const OPEN: f32 = 0.275;
const FADE_IN: f32 = 0.135;
const FROM_SCALE: f32 = 0.5;
pub(crate) const CLOSE: f32 = 0.2;
const KEEP: f32 = 0.275;
/// How long a page under the window is waited for, at most, to be on screen where it was laid out
/// (page_shown): far past the 26–37 ms it took on Wayland's runner, so it runs out only when the
/// core can't say, such as for a window with nothing on screen, and the preview goes on as if it
/// had (#2640).
const SHOWN_WAIT: Duration = Duration::from_millis(150);

thread_local! {
    static PROBE_LEFT: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) };
}

/// What the preview asks of its window and the core.
pub trait Host {
    /// Puts `tab`'s page in the preview's window at `page`, from its top left, with corners of
    /// `radius`, with the keyboard.
    fn place(&self, tab: u64, page: Bounds<Pixels>, radius: f32);
    /// Shows the preview's window over all of the browser window `window`, as it draws now, with
    /// the keyboard.
    fn show(&self, window: u64);
    /// It drew a frame of its motion, with the page at `page`, with corners of `radius`, and at
    /// `opacity`, which the page's view follows.
    fn drew(&self, page: Bounds<Pixels>, radius: f32, opacity: f32);
    /// Whether the page shows under the preview's window, through a hole it leaves, as no window
    /// of the app's can hold one. It's laid out where it rests then, and doesn't scale with the
    /// motion: what of it is in the frame drawn shows.
    fn page_under(&self) -> bool;
    /// Whether a page under the window shows through the hole only once the core says it's on
    /// screen there, not from the frame after it's laid out at rest: only where the window's page
    /// showed through on main (#2640).
    fn open_waits(&self) -> bool;
    /// It stops taking the keyboard and clicks, which the browser window gets back, as it leaves.
    fn release(&self);
    /// Hides its window, once it has left.
    fn hide(&self);
    /// Closes the preview's page, whose tab is `tab`.
    fn close(&self, tab: u64);
    /// Makes `tab` a Today tab of the Space `window` shows, and shows it there, as it is. A preview
    /// of the archived tab `archived` goes to the Space that tab closed in, as a restore does, and
    /// takes it out of the archive.
    fn keep(&self, window: u64, tab: u64, archived: Option<u64>);
    /// The kept preview was of the archived tab `archived`, which is the window's tab now, as a tab
    /// restored from it is.
    fn kept_archived(&self, _window: u64, _archived: u64, _cx: &mut App) {}
    /// The user closed the preview over the browser window `window`, for ⌘Z and ⇧⌘T to open
    /// again; `undone` if Redo closed it, as that history has it already.
    fn closed(&self, window: u64, closed: Closed, undone: bool, cx: &mut App);
    /// Shows `tip` hanging from a button at its anchor, in the browser window, after the tooltip's
    /// delay, or ends the tooltip, as the shell's buttons ask for theirs.
    fn tooltip(&self, tip: Option<(Tip, Anchor)>, cx: &mut App);
}

/// A preview the user closed: the tab it was over, the page it was at and where its link was.
#[derive(Clone, Debug, PartialEq)]
pub struct Closed {
    pub over: u64,
    pub url: SharedString,
    pub link: Point<Pixels>,
}

/// What a key in the preview does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyAction {
    Close,
    Keep,
    /// Copies the preview's address, as ⇧⌘C copies a tab's (Arc).
    CopyUrl,
}

/// What `keystroke` does in the preview, as the page hands it back: before the page sees it if
/// it's `reserved`, else once the page hasn't kept it. Esc is reserved, so it closes the preview
/// even on a page that keeps it, as Chromium's stop does. Nothing else of the window's keys acts on
/// it, as they would on the window's own tab.
pub fn key_action(keystroke: &str, reserved: bool) -> Option<KeyAction> {
    match (keystroke, reserved) {
        ("cmd-w" | "escape", true) => Some(KeyAction::Close),
        ("cmd-enter", false) => Some(KeyAction::Keep),
        ("shift-cmd-c", false) => Some(KeyAction::CopyUrl),
        _ => None,
    }
}

/// Whether `url` joins a call, which opens as a tab, not in a preview, as Arc's meeting links do:
/// Google Meet's, Zoom's, Microsoft Teams', Webex's, Whereby's and Jitsi Meet's.
pub fn is_meeting(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let (host, path) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));
    let host = host.rsplit_once('@').map_or(host, |(_, host)| host);
    let host = host.split(':').next().unwrap_or(host).to_ascii_lowercase();
    let on = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    let under = |paths: &[&str]| paths.iter().any(|each| path.starts_with(each));
    let room = path.len() > 1;
    match host.as_str() {
        "meet.google.com" | "whereby.com" | "meet.jit.si" => room,
        "teams.microsoft.com" => under(&["/l/meetup-join/", "/meet/"]),
        "teams.live.com" => under(&["/meet/"]),
        _ if on("zoom.us") => under(&["/j/", "/my/", "/w/", "/s/", "/wc/"]),
        _ if on("webex.com") => under(&["/meet/", "/join/"]) || path.contains("j.php"),
        _ => false,
    }
}

/// The site a URL is on, as the preview shows it: its host, without "www." or a port.
pub fn site_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host.rsplit_once('@').map_or(host, |(_, host)| host);
    // An IPv6 address keeps its colons, in its brackets.
    let host = match host.find(']') {
        Some(end) if host.starts_with('[') => &host[..=end],
        _ => host.split(':').next().unwrap_or(host),
    };
    host.strip_prefix("www.").unwrap_or(host)
}

/// Where the card goes over a window's page at `page`: in from its sides and bottom by a share of
/// it, and from its top by that and the controls' row over the card.
fn card(page: Bounds<Pixels>) -> Bounds<Pixels> {
    let side = f32::from(page.size.width.min(page.size.height));
    let inset = (side * INSET_SHARE).clamp(INSET_MIN, INSET_MAX);
    let top = inset + OVER_CARD;
    Bounds::new(
        page.origin + point(px(inset), px(top)),
        size(
            (page.size.width - px(2. * inset)).max(px(0.)),
            (page.size.height - px(top + inset)).max(px(0.)),
        ),
    )
}

/// Where the page goes in `card`: all of it inside the rim.
fn page_in(card: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        card.origin + point(px(RIM), px(RIM)),
        size(
            card.size.width - px(2. * RIM),
            card.size.height - px(2. * RIM),
        ),
    )
}

/// Where a page under the window waits while it isn't to show, at its size where it rests, `rest`:
/// left of the window's content, past any margin round it, where the window clips it. It moves by
/// whole points, so the core lays it out at the same whole size as where it rests.
fn out_of_sight(rest: Bounds<Pixels>) -> Bounds<Pixels> {
    let by = f32::from(rest.right()).ceil() + 1000.;
    Bounds::new(point(rest.left() - px(by), rest.top()), rest.size)
}

/// Where a preview's page rests over a window's page at `page`: the core makes it that size before
/// it loads, so its first script reads the size it's shown at (#2539).
pub fn resting(page: Bounds<Pixels>) -> Bounds<Pixels> {
    page_in(card(page))
}

/// The card with its page at `page`.
fn card_around(page: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        page.origin - point(px(RIM), px(RIM)),
        size(
            page.size.width + px(2. * RIM),
            page.size.height + px(2. * RIM),
        ),
    )
}

/// The controls' row over a card at `card`, as wide as it.
fn controls_over(card: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        card.origin - point(px(0.), px(OVER_CARD)),
        size(card.size.width, px(CONTROLS)),
    )
}

/// Where the keep and close buttons are, at the right end of the controls' row `row`.
fn buttons_in(row: Bounds<Pixels>) -> [Bounds<Pixels>; 2] {
    let at = |from_right: f32| {
        let origin = point(row.right() - px(from_right), row.origin.y);
        Bounds::new(origin, size(px(CONTROLS), px(CONTROLS)))
    };
    [at(2. * CONTROLS + BUTTON_GAP), at(CONTROLS)]
}

fn scaled_about(frame: Bounds<Pixels>, scale: f32, about: Point<Pixels>) -> Bounds<Pixels> {
    let at = |edge: Pixels, about: Pixels| about + (edge - about) * scale;
    Bounds::new(
        point(at(frame.origin.x, about.x), at(frame.origin.y, about.y)),
        size(frame.size.width * scale, frame.size.height * scale),
    )
}

fn between(from: Bounds<Pixels>, to: Bounds<Pixels>, t: f32) -> Bounds<Pixels> {
    let mix = |a: Pixels, b: Pixels| a + (b - a) * t;
    Bounds::new(
        point(
            mix(from.origin.x, to.origin.x),
            mix(from.origin.y, to.origin.y),
        ),
        size(
            mix(from.size.width, to.size.width),
            mix(from.size.height, to.size.height),
        ),
    )
}

/// Where the motion is.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Pose {
    /// The card's size about the link, 1 at rest.
    scale: f32,
    /// How far the page has grown into the tab's place, from 0 to 1.
    kept: f32,
    /// The page's opacity, and the card's around it.
    page: f32,
    card: f32,
    /// How far the window under it has dimmed, from 0 to 1.
    dim: f32,
}

impl Pose {
    /// `seconds` after the window was first on screen.
    fn arriving(seconds: f32) -> Self {
        let t = (seconds / OPEN).min(1.);
        let opacity = (seconds / FADE_IN).min(1.);
        Pose {
            scale: FROM_SCALE + (1. - FROM_SCALE) * cubic_bezier(0., 0., 0.58, 1., t),
            kept: 0.,
            page: opacity,
            card: opacity,
            dim: t,
        }
    }

    /// `seconds` after it started leaving back into the link, from `self`.
    fn closing(self, seconds: f32) -> Self {
        let t = (seconds / CLOSE).min(1.);
        let shrink = cubic_bezier(0.42, 0., 1., 1., t);
        Pose {
            scale: self.scale + (FROM_SCALE - self.scale) * shrink,
            page: self.page * (1. - t),
            card: self.card * (1. - t),
            dim: self.dim * (1. - t),
            ..self
        }
    }

    /// `seconds` after it started growing into the tab's place, from `self`.
    fn keeping(self, seconds: f32) -> Self {
        let t = cubic_bezier(0.42, 0., 0.58, 1., (seconds / KEEP).min(1.));
        Pose {
            scale: self.scale + (1. - self.scale) * t,
            kept: self.kept + (1. - self.kept) * t,
            page: self.page + (1. - self.page) * t,
            card: self.card * (1. - t),
            dim: self.dim * (1. - t),
        }
    }
}

/// Where the card and its page are drawn in `pose`, over a window's page at `page`, growing from
/// `link`: the card's frame before its scale about the returned point, and the page's after it.
fn frames(
    pose: Pose,
    page: Bounds<Pixels>,
    link: Point<Pixels>,
) -> (Bounds<Pixels>, Point<Pixels>, Bounds<Pixels>) {
    let rest = card(page);
    let about = point(
        link.x.clamp(rest.left(), rest.right()),
        link.y.clamp(rest.top(), rest.bottom()),
    );
    let card = between(rest, card_around(page), pose.kept);
    let shown = between(page_in(rest), page, pose.kept);
    (card, about, scaled_about(shown, pose.scale, about))
}

/// The corners of a rectangle of `size` from its top left, clockwise, rounded by `radius`.
fn corners(size: Size<Pixels>, radius: f32) -> [(f32, f32, f32); 4] {
    let (width, height) = (f32::from(size.width), f32::from(size.height));
    let radius = radius.min(width / 2.).min(height / 2.);
    [
        (0., 0., radius),
        (width, 0., radius),
        (width, height, radius),
        (0., height, radius),
    ]
}

/// The parts of a window of `size` round `hole`: above it, below it, left and right of it.
fn around(hole: Bounds<Pixels>, size: Size<Pixels>) -> [Bounds<Pixels>; 4] {
    let (width, height, rows) = (size.width, size.height, hole.size.height);
    [
        Bounds::new(point(px(0.), px(0.)), gpui::size(width, hole.top())),
        Bounds::new(
            point(px(0.), hole.bottom()),
            gpui::size(width, height - hole.bottom()),
        ),
        Bounds::new(point(px(0.), hole.top()), gpui::size(hole.left(), rows)),
        Bounds::new(
            point(hole.right(), hole.top()),
            gpui::size(width - hole.right(), rows),
        ),
    ]
}

/// The card at `card` with corners of `radius`, as a frame round the hole a page under the window
/// shows in, at `hole` with corners of `page_radius`: in `color`, with `coming` over the hole as
/// the page comes in, and the modal level's `rim` inside its edge.
fn paint_frame(
    (card, radius): (Bounds<Pixels>, f32),
    (hole, page_radius): (Bounds<Pixels>, f32),
    (color, coming, rim): (Hsla, Hsla, Hsla),
    window: &mut Window,
) {
    let mut frame = PathBuilder::fill();
    outline(&mut frame, card.origin, &corners(card.size, radius), 0.);
    if hole.size.width >= px(1.) && hole.size.height >= px(1.) {
        let corners = corners(hole.size, page_radius);
        outline(&mut frame, hole.origin, &corners, 0.);
        window.paint_quad(fill(hole, coming).corner_radii(px(corners[0].2)));
    }
    if let Ok(path) = frame.build() {
        window.paint_path(path, color);
    }
    let mut edge = PathBuilder::stroke(px(1.));
    outline(&mut edge, card.origin, &corners(card.size, radius), 0.5);
    if let Ok(path) = edge.build() {
        window.paint_path(path, rim);
    }
}

/// A control's plate, floating over the dim: the overlay's fill with the popover level, `shadow`
/// as the window allows, round at its ends. Clicks on it stay off the dim under it.
fn plate(id: &'static str, palette: &Palette, shadow: Vec<BoxShadow>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .h(px(CONTROLS))
        .rounded_full()
        .bg(palette.overlay)
        .border(px(RIM))
        .border_color(Level::Popover.rim_in(palette))
        .shadow(shadow)
        .occlude()
}

/// A round button: its id, its icon and how the preview leaves on its click, which names it, as
/// the header's buttons are, on its own plate.
fn button(
    (id, icon, how): (&'static str, &'static str, Leave),
    palette: &Palette,
    shadow: Vec<BoxShadow>,
) -> gpui::Stateful<gpui::Div> {
    let rest = palette.overlay;
    plate(id, palette, shadow)
        .w(px(CONTROLS))
        .flex()
        .items_center()
        .justify_center()
        .role(Role::Button)
        .aria_label(how.named().0)
        .hover(|style| style.bg(palette.state_over(rest, STATE_HOVERED)))
        .active(|style| style.bg(palette.state_over(rest, STATE_PRESSED)))
        .child(
            svg()
                .path(icon)
                .size(px(GLYPH))
                .text_color(palette.on_overlay_glyph(NAV)),
        )
}

/// The site, quietly, on a plate: its favicon, or a globe until it has one, in the plate's round
/// end, and its host, which ends in an ellipsis where the row is too narrow for it.
fn site_plate(
    (site, favicon): (SharedString, Option<Arc<Image>>),
    palette: &Palette,
    shadow: Vec<BoxShadow>,
) -> gpui::Stateful<gpui::Div> {
    let icon: AnyElement = match favicon {
        Some(favicon) => img(favicon).size(px(FAVICON_DRAWN)).into_any_element(),
        None => svg()
            .path("icons/globe.svg")
            .size(px(FAVICON_DRAWN))
            .text_color(palette.on_overlay_glyph(NAV))
            .into_any_element(),
    };
    plate("link-preview-site", palette, shadow)
        .flex_shrink(1.)
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(SITE_GAP))
        // From the plate's edge, outside its rim.
        .pl(px(SITE_X - RIM))
        .pr(px(SITE_END - RIM))
        .child(div().flex_none().child(icon))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(px(BODY))
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.on_overlay(SECONDARY))
                .child(site),
        )
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Leave {
    Close,
    Keep,
}

impl Leave {
    /// The name of the button that leaves this way, and the key that does the same, which its
    /// tooltip shows.
    fn named(self) -> (&'static str, &'static str) {
        match self {
            Leave::Keep => ("Open as Tab", "cmd-enter"),
            Leave::Close => ("Close", "escape"),
        }
    }
}

/// The preview of one page, from the click that opens it until it has left.
struct Preview {
    /// The browser window it shows over, and its page's frame and corners there.
    window: u64,
    page: Bounds<Pixels>,
    radius: f32,
    tab: u64,
    /// The tab whose link it opened from.
    over: u64,
    url: SharedString,
    /// Its page's favicon, once the core has one.
    favicon: Option<Arc<Image>>,
    /// Redo closed it, so ⌘Z's history has it already.
    undone: bool,
    /// The archived tab it previews, from the Library, which keeping it restores and closing it
    /// leaves in the archive.
    archived: Option<u64>,
    /// Where the link was clicked in the browser window.
    link: Point<Pixels>,
    /// When the app asked for it, from which its first frame is timed.
    asked: Instant,
    /// When its window was first on screen, where the motion starts.
    shown: Option<Instant>,
    settled: bool,
    /// Where its page was laid out last.
    placed: Bounds<Pixels>,
    /// Whether a page under the window shows through its hole.
    cut: bool,
    /// Whether the core has said a page under the window is on screen where it was laid out last,
    /// or the wait for that ran out.
    there: bool,
    /// That wait's limit, a one-shot dropped as the core says.
    _wait: Option<Task<()>>,
    /// Whether the window's page is to have the keyboard back once a page under the window has
    /// left, as it's closing.
    releasing: bool,
    /// Whether the hole has closed over a page under the window that's leaving, which it leaves
    /// the frame after.
    covered: bool,
    /// Whether the card holds at rest as it closes, until a page under the window has left.
    held: bool,
    /// The frame check.sh last read the card holds the page at, in pixels, and the scale.
    logged: Option<([i32; 4], f32)>,
    /// Once it's leaving: when, how, and where the motion was.
    leaving: Option<(Instant, Leave, Pose)>,
    _leave: Option<Task<()>>,
}

impl Preview {
    fn pose(&self) -> Pose {
        match self.leaving {
            Some((_, Leave::Close, from)) if self.held => from.closing(0.),
            Some((at, Leave::Close, from)) => from.closing(at.elapsed().as_secs_f32()),
            Some((at, Leave::Keep, from)) => from.keeping(at.elapsed().as_secs_f32()),
            None => Pose::arriving(self.shown.map_or(0., |shown| shown.elapsed().as_secs_f32())),
        }
    }

    /// Where it's drawn: as its motion has it, or under Reduce Motion at its size, and in the
    /// tab's place from the moment it's kept, fading.
    fn drawn(&self, cx: &App) -> Pose {
        let pose = self.pose();
        let kept = match self.leaving {
            Some((_, Leave::Keep, _)) => 1. - crate::motion::offset(1. - pose.kept, cx),
            _ => pose.kept,
        };
        Pose {
            scale: crate::motion::scale(pose.scale, cx),
            kept,
            ..pose
        }
    }

    /// Lays a page under the window out at `at`, unless it's there.
    fn lay_out(&mut self, host: &dyn Host, at: Bounds<Pixels>) {
        if self.placed != at {
            self.placed = at;
            self.there = false;
            self._wait = None;
            host.place(self.tab, at, RADIUS);
        }
    }

    /// Waits for the core to say a page under the window is on screen where it was laid out last,
    /// for SHOWN_WAIT at most, and draws again once it has.
    fn wait(&mut self, cx: &mut Context<LinkPreview>) {
        if self.there || self._wait.is_some() {
            return;
        }
        let (tab, at) = (self.tab, self.placed);
        self._wait = Some(cx.spawn(async move |preview, cx| {
            cx.background_executor().timer(SHOWN_WAIT).await;
            preview
                .update(cx, |preview, cx| {
                    let waited = preview.preview.as_mut();
                    let Some(shown) = waited.filter(|shown| (shown.tab, shown.placed) == (tab, at))
                    else {
                        return;
                    };
                    // check.sh reads this line.
                    log!("browser: link preview page not shown in 150 ms");
                    shown.there = true;
                    cx.notify();
                })
                .ok();
        }));
    }

    fn moving(&self) -> bool {
        self.leaving.is_some()
            || self
                .shown
                .is_none_or(|shown| shown.elapsed().as_secs_f32() < OPEN)
    }

    /// The page's corners, which the card's follow outside its rim: the preview's own at rest, and
    /// the window's page's once `kept` has grown it into the tab's place, so it lands with them.
    fn page_radius(&self, kept: f32) -> f32 {
        RADIUS + (self.radius - RADIUS) * kept
    }
}

pub struct LinkPreview {
    host: Rc<dyn Host>,
    focus: FocusHandle,
    preview: Option<Preview>,
    /// The button whose tooltip it asked for, until the pointer leaves it or the preview goes,
    /// since a button that goes from under the pointer sends no hover-out.
    tipped: Option<Leave>,
}

pub struct LinkPreviewWindow(WindowHandle<LinkPreview>);

impl Global for LinkPreviewWindow {}

impl LinkPreviewWindow {
    /// Whether a key the core reports from `tab`'s page, in the browser window `window`, or 0 for
    /// none, comes from the preview's page. The core names the tab Chromium's window shows for a key
    /// from any of its pages, which the preview's never is, so while the preview shows, with the
    /// keyboard, its window's keys are its own.
    pub fn holds(&self, window: u64, tab: u64, cx: &App) -> bool {
        let shown = self
            .0
            .read(cx)
            .ok()
            .and_then(|preview| preview.preview.as_ref());
        shown.is_some_and(|shown| {
            shown.tab == tab || (window != 0 && shown.window == window && shown.leaving.is_none())
        })
    }
}

/// Opens the preview's window, hidden, for `host` to show over a browser window, ahead of time as
/// the dialog's is.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<LinkPreview>> {
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(800.), px(600.)));
    open_at(host, bounds, false, cx)
}

/// Opens the preview's window at `bounds`, shown if `show`, as the preview binary has it over its
/// window, whose host can't place one.
pub fn open_at(
    host: Rc<dyn Host>,
    bounds: Bounds<Pixels>,
    show: bool,
    cx: &mut App,
) -> Option<WindowHandle<LinkPreview>> {
    let key = |key, action| crate::shell::view_binding(key, action, "LinkPreview", cx);
    let mac_keys = [
        key("cmd-w", Box::new(Close)),
        key("cmd-enter", Box::new(Keep)),
    ];
    cx.bind_keys(mac_keys);
    cx.bind_keys([KeyBinding::new("escape", Close, Some("LinkPreview"))]);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        kind: WindowKind::Floating,
        show,
        focus: false,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // The dim and the card are drawn.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| {
        cx.new(|cx| LinkPreview {
            host,
            focus: cx.focus_handle(),
            preview: None,
            tipped: None,
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(LinkPreviewWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the link preview's window: {error}");
            None
        }
    }
}

fn update<R>(
    cx: &mut App,
    change: impl FnOnce(&mut LinkPreview, &mut Window, &mut Context<LinkPreview>) -> R,
) -> Option<R> {
    let window = cx
        .try_global::<LinkPreviewWindow>()
        .map(|preview| preview.0)?;
    window.update(cx, change).ok()
}

/// Shows `tab`'s page, at `url`, over the browser window `window`, whose page is at `page` with
/// corners of that radius, growing from `link` there, where the link in `over`'s page was clicked,
/// or the row of the archived tab `archived` it previews. A preview showing goes at once.
#[allow(clippy::too_many_arguments)]
pub fn show(
    window: u64,
    tab: u64,
    over: u64,
    url: SharedString,
    (page, radius): (Bounds<Pixels>, f32),
    link: Point<Pixels>,
    archived: Option<u64>,
    cx: &mut App,
) {
    crate::lazy_window::ready::<LinkPreview>(cx);
    update(cx, |preview, gpui_window, cx| {
        preview.finish(cx);
        // check.sh reads this line.
        log!("browser: link preview {tab} {url}");
        let mut shown = Preview {
            window,
            page,
            radius,
            tab,
            over,
            url,
            favicon: None,
            undone: false,
            archived,
            link,
            asked: Instant::now(),
            shown: None,
            settled: false,
            placed: page_in(card(page)),
            cut: false,
            there: false,
            _wait: None,
            releasing: false,
            covered: false,
            held: false,
            logged: None,
            leaving: None,
            _leave: None,
        };
        // A page under the window is laid out at its size from the first, and waits out of sight
        // while the card grows (render).
        if preview.host.page_under() {
            shown.placed = out_of_sight(shown.placed);
        }
        preview.host.place(tab, shown.placed, RADIUS);
        preview.host.show(window);
        preview.preview = Some(shown);
        // The page has the keyboard, and the preview's keys are there once a click gives it to the
        // controls.
        gpui_window.focus(&preview.focus, cx);
        cx.notify();
    });
}

/// The browser window `window` laid its page out at `page`, as it resized.
pub fn page_moved(window: u64, (page, radius): (Bounds<Pixels>, f32), cx: &mut App) {
    update(cx, |preview, _, cx| {
        let host = preview.host.clone();
        let Some(shown) = preview.preview.as_mut() else {
            return;
        };
        let moved = (shown.page, shown.radius) != (page, radius);
        if shown.window != window || shown.leaving.is_some() || !moved {
            return;
        }
        shown.page = page;
        shown.radius = radius;
        // A page under the window is laid out by the frame it draws next.
        if !host.page_under() {
            host.place(shown.tab, page_in(card(page)), RADIUS);
        }
        cx.notify();
    });
}

/// The core says `tab`'s page, under the window, is on screen at `page`, where the preview laid it
/// out.
pub fn page_shown(tab: u64, page: Bounds<Pixels>, cx: &mut App) {
    update(cx, |preview, _, cx| {
        let shown = preview.preview.as_mut();
        if let Some(shown) = shown.filter(|shown| (shown.tab, shown.placed) == (tab, page)) {
            shown.there = true;
            shown._wait = None;
            cx.notify();
        }
    });
}

/// `tab`'s page is at `url` now, as it navigated.
pub fn set_url(tab: u64, url: SharedString, cx: &mut App) {
    update(cx, |preview, _, cx| {
        if let Some(shown) = preview.preview.as_mut().filter(|shown| shown.tab == tab) {
            shown.url = url;
            cx.notify();
        }
    });
}

/// `tab`'s page has `favicon` now, or none.
pub fn set_favicon(tab: u64, favicon: Option<Arc<Image>>, cx: &mut App) {
    update(cx, |preview, _, cx| {
        if let Some(shown) = preview.preview.as_mut().filter(|shown| shown.tab == tab) {
            shown.favicon = favicon;
            cx.notify();
        }
    });
}

/// The preview's tab, while one shows.
pub fn tab(cx: &mut App) -> Option<u64> {
    update(cx, |preview, _, _| {
        preview.preview.as_ref().map(|shown| shown.tab)
    })
    .flatten()
}

/// What `keystroke` from `tab`'s page in the browser window `window` does, if it's the preview's
/// (LinkPreviewWindow::holds): it's done, and whether the page loses it. None for another page.
pub fn key(window: u64, tab: u64, keystroke: &str, reserved: bool, cx: &mut App) -> Option<bool> {
    let holds = cx
        .try_global::<LinkPreviewWindow>()
        .is_some_and(|it| it.holds(window, tab, cx));
    if !holds {
        return None;
    }
    update(cx, |preview, _, cx| match key_action(keystroke, reserved) {
        Some(KeyAction::Close) => preview.close(keystroke, cx),
        Some(KeyAction::Keep) => preview.keep(keystroke, cx),
        Some(KeyAction::CopyUrl) => {
            let url = preview
                .preview
                .as_ref()
                .map(|shown| shown.url.clone())
                .unwrap_or_default();
            crate::shell::copy_url_of_preview(&url, cx);
            true
        }
        None => false,
    })
}

/// Redo closes the preview over `over` in the browser window `window`, if it shows. Whether it did.
pub fn close_over(window: u64, over: u64, cx: &mut App) -> bool {
    update(cx, |preview, _, cx| {
        let shown = preview.preview.as_mut();
        let Some(shown) = shown.filter(|shown| shown.window == window && shown.over == over) else {
            return false;
        };
        shown.undone = true;
        preview.close("redo", cx)
    })
    .unwrap_or(false)
}

/// The user left the preview for another window. The host has released it already.
pub fn dismissed(cx: &mut App) {
    update(cx, |preview, _, cx| preview.close("dismissed", cx));
}

/// The tab closed, and its preview goes with it at once.
pub fn tab_closed(tab: u64, cx: &mut App) {
    update(cx, |preview, _, cx| {
        if preview
            .preview
            .as_ref()
            .is_some_and(|shown| shown.tab == tab)
        {
            preview.preview = None;
            preview.end_tip(cx);
            preview.host.hide();
            cx.notify();
        }
    });
}

impl LinkPreview {
    /// Closes the preview back into its link, giving the browser window the keyboard and the
    /// clicks at once, and its page once it has left. Whether it was showing.
    fn close(&mut self, why: &str, cx: &mut Context<Self>) -> bool {
        // A page under the window can't leave once the user has left it, as laying it out would
        // take the keyboard after them, so the preview goes at once rather than shrink over it.
        let seconds = match why == "dismissed" && self.host.page_under() {
            true => 0.,
            false => CLOSE,
        };
        self.leave(Leave::Close, why, seconds, cx)
    }

    /// Grows the preview into the tab's place, then keeps its page there as a Today tab.
    fn keep(&mut self, why: &str, cx: &mut Context<Self>) -> bool {
        self.leave(Leave::Keep, why, KEEP, cx)
    }

    fn leave(&mut self, how: Leave, why: &str, seconds: f32, cx: &mut Context<Self>) -> bool {
        // A click on a button ends its tooltip, as on other buttons, and a key does too.
        self.end_tip(cx);
        let Some(shown) = self
            .preview
            .as_mut()
            .filter(|shown| shown.leaving.is_none())
        else {
            return false;
        };
        shown.leaving = Some((Instant::now(), how, shown.pose()));
        // check.sh reads this line, before the host's that it's released.
        let what = if how == Leave::Keep {
            "keeping"
        } else {
            "closing"
        };
        log!("browser: link preview {what} ({why})");
        PROBE_LEFT.set(Some(Instant::now()));
        // A page under the window that shows leaves in the card's frames first (render), while it
        // still has the keyboard, as laying it out takes it.
        shown.releasing = how == Leave::Close && shown.cut && why != "dismissed";
        // And the card holds at rest until it has, as shrinking would show it round the card.
        shown.held = shown.releasing;
        if !shown.releasing {
            self.host.release();
        }
        if !shown.held {
            shown._leave = Some(finish_after(shown.tab, seconds, cx));
        }
        cx.notify();
        true
    }

    /// Ends the preview shown, if any, where its leaving would: its page closed or kept, and its
    /// window hidden. One the user closed is the window's to reopen.
    fn finish(&mut self, cx: &mut App) {
        self.end_tip(cx);
        let Some(shown) = self.preview.take() else {
            return;
        };
        // Proof only (#2640): when it ended, from the key or click that closed or kept it.
        if let Some(at) = PROBE_LEFT.take() {
            let ms = at.elapsed().as_secs_f64() * 1000.;
            log!("browser: probe close {ms:.1} ms");
        }
        match shown.leaving {
            Some((_, Leave::Keep, _)) => {
                self.host.keep(shown.window, shown.tab, shown.archived);
                // check.sh reads this line.
                log!("browser: link preview kept {}", shown.tab);
                self.host.hide();
                if let Some(archived) = shown.archived {
                    self.host.kept_archived(shown.window, archived, cx);
                }
            }
            leaving => {
                self.host.hide();
                self.host.close(shown.tab);
                // An archived tab's preview leaves it in the archive, which is how to get back to
                // it: nothing for ⌘Z to reopen.
                if leaving.is_some() && shown.archived.is_none() {
                    let Preview {
                        window,
                        over,
                        url,
                        link,
                        undone,
                        ..
                    } = shown;
                    self.host
                        .closed(window, Closed { over, url, link }, undone, cx);
                }
            }
        }
    }
}

/// Ends `tab`'s preview after `seconds`, as its leaving does, unless another took its place.
fn finish_after(tab: u64, seconds: f32, cx: &mut Context<LinkPreview>) -> Task<()> {
    cx.spawn(async move |preview, cx| {
        cx.background_executor()
            .timer(Duration::from_secs_f32(seconds))
            .await;
        preview
            .update(cx, |preview, cx| {
                if preview
                    .preview
                    .as_ref()
                    .is_some_and(|shown| shown.tab == tab)
                {
                    preview.finish(cx);
                    cx.notify();
                }
            })
            .ok();
    })
}

impl LinkPreview {
    /// The pointer has come onto or left the button that leaves `how`, whose tooltip names it and
    /// its key, as the header's buttons' do: Open as Tab ⌘↩, and Close ⎋, written off the Mac as
    /// Settings writes them there. It hangs from the button where it rests.
    fn hover_button(&mut self, how: Leave, hovered: bool, window: Size<Pixels>, cx: &mut App) {
        let resting = self
            .preview
            .as_ref()
            .filter(|shown| shown.leaving.is_none());
        match resting.filter(|_| hovered) {
            Some(shown) => {
                let [keep, close] = buttons_in(controls_over(card(shown.page)));
                let frame = if how == Leave::Keep { keep } else { close };
                let (name, keystroke) = how.named();
                let keys = keystroke_keys(keystroke)
                    .into_iter()
                    .map(Into::into)
                    .collect();
                self.tipped = Some(how);
                self.host.tooltip(
                    Some((
                        Tip {
                            text: name.into(),
                            keys,
                        },
                        Anchor::under(frame, window),
                    )),
                    cx,
                );
            }
            // Unless the pointer went from it onto the other button, whose tooltip took its place.
            None if self.tipped == Some(how) => self.end_tip(cx),
            None => {}
        }
    }

    /// Ends the tooltip of the button it asked for one for, if any.
    fn end_tip(&mut self, cx: &mut App) {
        if self.tipped.take().is_some() {
            self.host.tooltip(None, cx);
        }
    }
}

impl Render for LinkPreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div()
            .key_context("LinkPreview")
            .track_focus(&self.focus)
            .on_action(cx.listener(|preview, _: &Close, _, cx| {
                preview.close("escape", cx);
            }))
            .on_action(cx.listener(|preview, _: &Keep, _, cx| {
                preview.keep("cmd-enter", cx);
            }))
            // A tooltip shows once the pointer rests (tooltip.rs).
            .on_mouse_move(|_, _, cx| crate::tooltip::moved(cx))
            .size_full()
            .font_family(".SystemUIFont");
        let Some(shown) = self.preview.as_mut() else {
            return root;
        };
        // AppKit puts the window on screen some frames after it's asked to, and only then does
        // GPUI draw it, so the motion starts there rather than skip its start.
        if shown.shown.is_none() && window.is_visible() {
            shown.shown = Some(Instant::now());
            // check.sh reads this line, for the motion's recording to start from.
            let ms = shown.asked.elapsed().as_secs_f64() * 1000.;
            log!("browser: link preview first frame in {ms:.1} ms");
        }
        let pose = shown.drawn(cx);
        let moving = shown.moving();
        let settling = !moving && !shown.settled;
        shown.settled |= settling;
        let (card_frame, about, page) = frames(pose, shown.page, shown.link);
        let under = self.host.page_under();
        let page_radius = shown.page_radius(pose.kept);
        // A page under the window shows only through the window's hole, where nothing of the
        // window's draws over it, and the host hears again as that moves. It can't scale or fade,
        // and isn't resized for the motion, as a page reads its size as it loads (#2519): it waits
        // out of sight while the card grows with a plate where it will show, comes in under the
        // plate the frame after the card first draws at rest, and shows through the hole from the
        // frame after that, or under Wayland once the core says it's on screen there. As it
        // closes, the plate is in the hole the frame before the hole closes, the page leaves the
        // frame after, and the card shrinks back once the core says the page has gone. The hole
        // changes at once, and the frames of the page and the window reach the screen later, each
        // in its own time, so each step waits for the frame that covers it to be there (#2640).
        let mut leave_to = None;
        let (page, plate) = match under {
            false => (page, false),
            true => {
                let rest = page_in(card(shown.page));
                let none = Bounds::new(about, Size::default());
                let how = shown.leaving.as_ref().map(|(_, how, _)| *how);
                match (how, shown.cut) {
                    (None, true) => {
                        shown.lay_out(&*self.host, rest);
                        (rest, false)
                    }
                    (None, false) if !moving => {
                        if !settling {
                            let waits = self.host.open_waits();
                            shown.cut = shown.placed == rest && (shown.there || !waits);
                            // Proof only (#2640): when the hole opens, from the motion's start.
                            if shown.cut {
                                let ms = shown
                                    .shown
                                    .map_or(0., |at| at.elapsed().as_secs_f64() * 1000.);
                                log!("browser: probe open {ms:.1} ms");
                            }
                            shown.lay_out(&*self.host, rest);
                            if waits {
                                shown.wait(cx);
                            }
                        }
                        if shown._wait.is_none() {
                            window.request_animation_frame();
                        }
                        (if shown.cut { rest } else { none }, false)
                    }
                    (Some(Leave::Close), true) => {
                        shown.cut = false;
                        window.request_animation_frame();
                        (shown.placed, true)
                    }
                    (Some(Leave::Keep), true) => (page.intersect(&rest), false),
                    // The page leaves the frame after the hole has closed over it: the hole closes
                    // at once, and until the frame with the plate in it is on screen, the one
                    // before shows there, clear where the page showed.
                    (Some(Leave::Close), false) if shown.releasing && !shown.covered => {
                        shown.covered = true;
                        window.request_animation_frame();
                        (none, false)
                    }
                    _ => {
                        if how.is_none() || shown.releasing {
                            leave_to = Some(out_of_sight(rest));
                        }
                        (none, false)
                    }
                }
            }
        };
        if moving || settling || under {
            self.host.drew(page, page_radius * pose.scale, pose.page);
        }
        // The page leaves once the hole has closed over it.
        if let Some(at) = leave_to {
            shown.lay_out(&*self.host, at);
            if std::mem::take(&mut shown.releasing) {
                self.host.release();
            }
        }
        // And the card shrinks back once it has left.
        if shown.held && !shown.releasing {
            shown.wait(cx);
            if shown.there {
                shown.held = false;
                if let Some((at, ..)) = shown.leaving.as_mut() {
                    *at = Instant::now();
                }
                shown._leave = Some(finish_after(shown.tab, CLOSE, cx));
            }
        }
        // Nothing moves while it waits for the core, which draws it again once the page is there.
        if moving && shown._wait.is_none() {
            window.request_animation_frame();
        }
        if settling {
            // check.sh reads these lines, the button's before it waits on the other, to rest on it.
            let [keep, _] = buttons_in(controls_over(card(shown.page)));
            let (x, y) = (f32::from(keep.center().x), f32::from(keep.center().y));
            log!("browser: link preview keep button at {x} {y}");
            // check/peek-motion.sh reads where the page rests and the point it grows from, to
            // tell where the card is in each frame of the motion it films.
            let rest = page_in(card(shown.page));
            let (left, top) = (f32::from(rest.left()), f32::from(rest.top()));
            let (width, height) = (f32::from(rest.size.width), f32::from(rest.size.height));
            let (from_x, from_y) = (f32::from(about.x), f32::from(about.y));
            log!(
                "browser: link preview page at {left} {top} {width} {height} from {from_x} {from_y}"
            );
            log!("browser: link preview settled");
        }
        if shown.settled && shown.leaving.is_none() {
            // check.sh reads where the card holds the page, in the pixels it's drawn in, against
            // the page's frame in the browser window's, at rest and again as either moves (#2541).
            let (frame, scale) = (page_in(card(shown.page)), window.scale_factor());
            let pixels = |at: Pixels| (f32::from(at) * scale).round() as i32;
            let drawn = [
                frame.left(),
                frame.top(),
                frame.size.width,
                frame.size.height,
            ]
            .map(pixels);
            if shown.logged != Some((drawn, scale)) {
                shown.logged = Some((drawn, scale));
                let [left, top, width, height] = drawn;
                log!(
                    "browser: link preview page frame {left} {top} {width} {height} at scale {scale}"
                );
            }
        }
        let palette = palette::current(cx);
        let card_radius = page_radius + RIM;
        // Nothing casts a shadow where the page shows under the window, as it would fall on it.
        let shadow = |level: Level| if under { Vec::new() } else { level.shadow() };
        let card = div()
            .id("link-preview-card")
            .absolute()
            .left(card_frame.left())
            .top(card_frame.top())
            .w(card_frame.size.width)
            .h(card_frame.size.height)
            .rounded(px(card_radius))
            .opacity(pose.card)
            // Clicks on the card stay off the dim under it.
            .occlude();
        let card = match under {
            // The page lies over all of it but the rim.
            false => card
                .bg(palette.overlay)
                .border(px(RIM))
                .border_color(Level::Modal.rim_in(&palette))
                .shadow(shadow(Level::Modal)),
            true => {
                let hole = (scaled_about(page, 1. / pose.scale, about), page_radius);
                let color = Hsla::from(palette.overlay);
                let coming = match plate {
                    true => color,
                    false => color.opacity(1. - pose.page),
                };
                let colors = (color, coming, Level::Modal.rim_in(&palette));
                let frame = canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        paint_frame((card_frame, card_radius), hole, colors, window)
                    },
                );
                card.child(frame.absolute().size_full())
            }
        };
        let site = site_of(&shown.url).to_string();
        let site = (!site.is_empty()).then(|| (site.into(), shown.favicon.clone()));
        let lift = shadow(Level::Popover);
        let keep = button(
            ("link-preview-keep", "icons/maximize.svg", Leave::Keep),
            &palette,
            lift.clone(),
        );
        let close = button(
            ("link-preview-close", "icons/x.svg", Leave::Close),
            &palette,
            lift.clone(),
        );
        let row = controls_over(card_frame);
        let controls = div()
            .absolute()
            .left(row.left())
            .top(row.top())
            .w(row.size.width)
            .h(row.size.height)
            .flex()
            .items_center()
            .gap(px(BUTTON_GAP))
            .opacity(pose.card)
            .children(site.map(|site| site_plate(site, &palette, lift)))
            .child(div().flex_1())
            .child(
                keep.on_hover(cx.listener(|preview, hovered, window, cx| {
                    preview.hover_button(Leave::Keep, *hovered, window.viewport_size(), cx);
                }))
                .on_click(cx.listener(|preview, _, _, cx| {
                    preview.keep("button", cx);
                })),
            )
            .child(
                close
                    .on_hover(cx.listener(|preview, hovered, window, cx| {
                        preview.hover_button(Leave::Close, *hovered, window.viewport_size(), cx);
                    }))
                    .on_click(cx.listener(|preview, _, _, cx| {
                        preview.close("button", cx);
                    })),
            );
        let dim = div()
            .id("link-preview-dim")
            .absolute()
            .size_full()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|preview, _, _, cx| {
                    preview.close("click outside", cx);
                }),
            );
        // Round a page under the window, which it leaves as it is.
        let dim_color = Hsla::from(rgb(DIM)).opacity(DIM_OPACITY * pose.dim);
        let dim = match under {
            false => dim.bg(dim_color),
            true => dim.children(around(page, window.viewport_size()).map(|each| {
                div()
                    .absolute()
                    .left(each.left())
                    .top(each.top())
                    .w(each.size.width)
                    .h(each.size.height)
                    .bg(dim_color)
            })),
        };
        // The controls grow from the link and leave with the card, as part of it.
        let preview = div().absolute().size_full().child(card).child(controls);
        root.child(dim).child(scaled(preview, pose.scale, about))
    }
}

#[cfg(test)]
mod tests;
