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

use std::{
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, App, Bounds, BoxShadow, Context, FocusHandle, FontWeight, Global, Hsla, Image,
    KeyBinding, MouseButton, PathBuilder, Pixels, Point, SharedString, Size, Task,
    TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
    WindowOptions, accesskit::Role, actions, canvas, div, fill, img, point, prelude::*, px, rgb,
    size, svg,
};

use crate::elevation::Level;
use crate::icon_picker::outline;
use crate::palette::{self, Palette, STATE_HOVERED, STATE_PRESSED};
use crate::shell::{cubic_bezier, scaled::scaled};
use crate::tokens::dialog::DIM;
use crate::tokens::link_preview::{
    BUTTON_GAP, CONTROLS, CONTROLS_GAP, DIM_OPACITY, GLYPH, INSET_MAX, INSET_MIN, INSET_SHARE,
    RADIUS, RIM, SITE_END, SITE_GAP, SITE_X,
};
use crate::tokens::shell::{FAVICON_DRAWN, NAV, SECONDARY};
use crate::tokens::text::BODY;

actions!(link_preview, [Close, Keep]);

/// The controls' row over the card: their height and the gap under them.
const OVER_CARD: f32 = CONTROLS + CONTROLS_GAP;

/// The dialog's measured motion, from the link: it grows in over 0.275 s from half its size,
/// opaque after the first 0.135 s, as the window dims. It shrinks back into the link in 0.2 s,
/// as fast as an overlay grows in, and grows into the tab's place in 0.275 s.
const OPEN: f32 = 0.275;
const FADE_IN: f32 = 0.135;
const FROM_SCALE: f32 = 0.5;
const CLOSE: f32 = 0.2;
const KEEP: f32 = 0.275;

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
    /// It stops taking the keyboard and clicks, which the browser window gets back, as it leaves.
    fn release(&self);
    /// Hides its window, once it has left.
    fn hide(&self);
    /// Closes the preview's page, whose tab is `tab`.
    fn close(&self, tab: u64);
    /// Makes `tab` a Today tab of the Space `window` shows, and shows it there, as it is.
    fn keep(&self, window: u64, tab: u64);
    /// The user closed the preview over the browser window `window`, for ⌘Z and ⇧⌘T to open
    /// again; `undone` if Redo closed it, as that history has it already.
    fn closed(&self, window: u64, closed: Closed, undone: bool, cx: &mut App);
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
        size(card.size.width - px(2. * RIM), card.size.height - px(2. * RIM)),
    )
}

/// The card with its page at `page`.
fn card_around(page: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        page.origin - point(px(RIM), px(RIM)),
        size(page.size.width + px(2. * RIM), page.size.height + px(2. * RIM)),
    )
}

/// The controls' row over a card at `card`, as wide as it.
fn controls_over(card: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(card.origin - point(px(0.), px(OVER_CARD)), size(card.size.width, px(CONTROLS)))
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
        point(mix(from.origin.x, to.origin.x), mix(from.origin.y, to.origin.y)),
        size(mix(from.size.width, to.size.width), mix(from.size.height, to.size.height)),
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
    [(0., 0., radius), (width, 0., radius), (width, height, radius), (0., height, radius)]
}

/// The parts of a window of `size` round `hole`: above it, below it, left and right of it.
fn around(hole: Bounds<Pixels>, size: Size<Pixels>) -> [Bounds<Pixels>; 4] {
    let (width, height, rows) = (size.width, size.height, hole.size.height);
    [
        Bounds::new(point(px(0.), px(0.)), gpui::size(width, hole.top())),
        Bounds::new(point(px(0.), hole.bottom()), gpui::size(width, height - hole.bottom())),
        Bounds::new(point(px(0.), hole.top()), gpui::size(hole.left(), rows)),
        Bounds::new(point(hole.right(), hole.top()), gpui::size(width - hole.right(), rows)),
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

/// A round button: its id, its icon and its name, as the header's buttons are, on its own plate.
fn button(
    (id, icon, name): (&'static str, &'static str, &'static str),
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
        .aria_label(name)
        .hover(|style| style.bg(palette.state_over(rest, STATE_HOVERED)))
        .active(|style| style.bg(palette.state_over(rest, STATE_PRESSED)))
        .child(svg().path(icon).size(px(GLYPH)).text_color(palette.on_overlay_glyph(NAV)))
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
    /// Where the link was clicked in the browser window.
    link: Point<Pixels>,
    /// When the app asked for it, from which its first frame is timed.
    asked: Instant,
    /// When its window was first on screen, where the motion starts.
    shown: Option<Instant>,
    settled: bool,
    /// Once it's leaving: when, how, and where the motion was.
    leaving: Option<(Instant, Leave, Pose)>,
    _leave: Option<Task<()>>,
}

impl Preview {
    fn pose(&self) -> Pose {
        match self.leaving {
            Some((at, Leave::Close, from)) => from.closing(at.elapsed().as_secs_f32()),
            Some((at, Leave::Keep, from)) => from.keeping(at.elapsed().as_secs_f32()),
            None => Pose::arriving(self.shown.map_or(0., |shown| shown.elapsed().as_secs_f32())),
        }
    }

    fn moving(&self) -> bool {
        self.leaving.is_some()
            || self.shown.is_none_or(|shown| shown.elapsed().as_secs_f32() < OPEN)
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
}

pub struct LinkPreviewWindow(WindowHandle<LinkPreview>);

impl Global for LinkPreviewWindow {}

impl LinkPreviewWindow {
    /// Whether a key the core reports from `tab`'s page, in the browser window `window`, or 0 for
    /// none, comes from the preview's page. The core names the tab Chromium's window shows for a key
    /// from any of its pages, which the preview's never is, so while the preview shows, with the
    /// keyboard, its window's keys are its own.
    pub fn holds(&self, window: u64, tab: u64, cx: &App) -> bool {
        let shown = self.0.read(cx).ok().and_then(|preview| preview.preview.as_ref());
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
    let mac_keys = [key("cmd-w", Box::new(Close)), key("cmd-enter", Box::new(Keep))];
    cx.bind_keys(mac_keys);
    cx.bind_keys([
        KeyBinding::new("escape", Close, Some("LinkPreview")),
    ]);
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
        cx.new(|cx| LinkPreview { host, focus: cx.focus_handle(), preview: None })
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
    let window = cx.try_global::<LinkPreviewWindow>().map(|preview| preview.0)?;
    window.update(cx, change).ok()
}

/// Shows `tab`'s page, at `url`, over the browser window `window`, whose page is at `page` with
/// corners of that radius, growing from `link` there, where the link in `over`'s page was clicked.
/// A preview showing goes at once.
pub fn show(
    window: u64,
    tab: u64,
    over: u64,
    url: SharedString,
    (page, radius): (Bounds<Pixels>, f32),
    link: Point<Pixels>,
    cx: &mut App,
) {
    update(cx, |preview, gpui_window, cx| {
        preview.finish(cx);
        // check.sh reads this line.
        log!("browser: link preview {tab} {url}");
        let shown = Preview {
            window,
            page,
            radius,
            tab,
            over,
            url,
            favicon: None,
            undone: false,
            link,
            asked: Instant::now(),
            shown: None,
            settled: false,
            leaving: None,
            _leave: None,
        };
        preview.host.place(tab, page_in(card(page)), RADIUS);
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
        host.place(shown.tab, page_in(card(page)), RADIUS);
        cx.notify();
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
    update(cx, |preview, _, _| preview.preview.as_ref().map(|shown| shown.tab)).flatten()
}

/// What `keystroke` from `tab`'s page in the browser window `window` does, if it's the preview's
/// (LinkPreviewWindow::holds): it's done, and whether the page loses it. None for another page.
pub fn key(window: u64, tab: u64, keystroke: &str, reserved: bool, cx: &mut App) -> Option<bool> {
    let holds = cx.try_global::<LinkPreviewWindow>().is_some_and(|it| it.holds(window, tab, cx));
    if !holds {
        return None;
    }
    update(cx, |preview, _, cx| match key_action(keystroke, reserved) {
        Some(KeyAction::Close) => preview.close(keystroke, cx),
        Some(KeyAction::Keep) => preview.keep(keystroke, cx),
        Some(KeyAction::CopyUrl) => {
            let url = preview.preview.as_ref().map(|shown| shown.url.clone()).unwrap_or_default();
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
        if preview.preview.as_ref().is_some_and(|shown| shown.tab == tab) {
            preview.preview = None;
            preview.host.hide();
            cx.notify();
        }
    });
}

impl LinkPreview {
    /// Closes the preview back into its link, giving the browser window the keyboard and the
    /// clicks at once, and its page once it has left. Whether it was showing.
    fn close(&mut self, why: &str, cx: &mut Context<Self>) -> bool {
        self.leave(Leave::Close, why, CLOSE, cx)
    }

    /// Grows the preview into the tab's place, then keeps its page there as a Today tab.
    fn keep(&mut self, why: &str, cx: &mut Context<Self>) -> bool {
        self.leave(Leave::Keep, why, KEEP, cx)
    }

    fn leave(&mut self, how: Leave, why: &str, seconds: f32, cx: &mut Context<Self>) -> bool {
        let Some(shown) = self.preview.as_mut().filter(|shown| shown.leaving.is_none()) else {
            return false;
        };
        shown.leaving = Some((Instant::now(), how, shown.pose()));
        // check.sh reads this line, before the host's that it's released.
        let what = if how == Leave::Keep { "keeping" } else { "closing" };
        log!("browser: link preview {what} ({why})");
        self.host.release();
        let tab = shown.tab;
        shown._leave = Some(cx.spawn(async move |preview, cx| {
            cx.background_executor().timer(Duration::from_secs_f32(seconds)).await;
            preview
                .update(cx, |preview, cx| {
                    // Unless another preview took its place meanwhile.
                    if preview.preview.as_ref().is_some_and(|shown| shown.tab == tab) {
                        preview.finish(cx);
                        cx.notify();
                    }
                })
                .ok();
        }));
        cx.notify();
        true
    }

    /// Ends the preview shown, if any, where its leaving would: its page closed or kept, and its
    /// window hidden. One the user closed is the window's to reopen.
    fn finish(&mut self, cx: &mut App) {
        let Some(shown) = self.preview.take() else {
            return;
        };
        match shown.leaving {
            Some((_, Leave::Keep, _)) => {
                self.host.keep(shown.window, shown.tab);
                // check.sh reads this line.
                log!("browser: link preview kept {}", shown.tab);
                self.host.hide();
            }
            leaving => {
                self.host.hide();
                self.host.close(shown.tab);
                if leaving.is_some() {
                    let Preview { window, over, url, link, undone, .. } = shown;
                    self.host.closed(window, Closed { over, url, link }, undone, cx);
                }
            }
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
        let pose = shown.pose();
        let moving = shown.moving();
        let settling = !moving && !shown.settled;
        shown.settled |= settling;
        if moving {
            window.request_animation_frame();
        }
        let (card_frame, about, page) = frames(pose, shown.page, shown.link);
        let under = self.host.page_under();
        // A page under the window shows only where it rests, and the host hears again as that
        // moves.
        let page = match under {
            true => page.intersect(&page_in(card(shown.page))),
            false => page,
        };
        let page_radius = shown.page_radius(pose.kept);
        if moving || settling || under {
            self.host.drew(page, page_radius * pose.scale, pose.page);
        }
        if settling {
            // check.sh reads this line.
            log!("browser: link preview settled");
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
                let colors = (color, color.opacity(1. - pose.page), Level::Modal.rim_in(&palette));
                let frame = canvas(|_, _, _| {}, move |_, _, window, _| {
                    paint_frame((card_frame, card_radius), hole, colors, window)
                });
                card.child(frame.absolute().size_full())
            }
        };
        let site = site_of(&shown.url).to_string();
        let site = (!site.is_empty()).then(|| (site.into(), shown.favicon.clone()));
        let lift = shadow(Level::Popover);
        let keep = button(("link-preview-keep", "icons/maximize.svg", "Open as Tab"), &palette, lift.clone());
        let close = button(("link-preview-close", "icons/x.svg", "Close"), &palette, lift.clone());
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
            .child(keep.on_click(cx.listener(|preview, _, _, cx| {
                preview.keep("button", cx);
            })))
            .child(close.on_click(cx.listener(|preview, _, _, cx| {
                preview.close("button", cx);
            })));
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
