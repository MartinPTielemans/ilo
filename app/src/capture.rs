//! Capture: Arc's ⇧⌘2, which captures a portion of the page, and Capture Full Page
//! (docs/ux-spec.md › Capture). Capture mode covers the page with a grey picture of it and shows
//! the element under the pointer in its colours; a click takes that element, a drag the rectangle
//! dragged, and a menu at its corner says what to do with it. GPUI can't draw over the page's
//! native view, so capture mode is a window of its own that the host shows over the page, as the
//! browser window's child, with the keyboard. What it asks of its window, the core and the system
//! goes through `Host`.

use std::{rc::Rc, sync::Arc, time::Duration};

use gpui::{
    App, Bounds, Context, Corners, Div, FocusHandle, FontWeight, Global, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, RenderImage, SharedString,
    Stateful, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions, actions, canvas, div, point, prelude::*, px, rgba, size, svg,
};
use image::{Frame, RgbaImage};

use crate::elevation::Level;
use crate::menu::Look;
use crate::toast;

actions!(capture, [Leave]);

use crate::tokens::capture::RING;
use crate::tokens::capture::RING_RADIUS;
use crate::tokens::capture::DIM;
use crate::tokens::capture::PILL_WIDTH;
use crate::tokens::capture::PILL_HEIGHT;
use crate::tokens::capture::PILL_RADIUS;
use crate::tokens::capture::PILL_BOTTOM;
use crate::tokens::capture::PILL_TEXT_SIZE;
use crate::tokens::capture::MENU_LEFT;
use crate::tokens::capture::GLYPH;
use crate::tokens::capture::MARGIN;
// The menu is ilo's (menu.rs), in its tokens, the narrowest it comes.
use crate::tokens::menu::{GAP, LINE, PADDING, RADIUS, ROW, ROW_RADIUS, ROW_X, TEXT};
use crate::tokens::menu::MIN_WIDTH as MENU_WIDTH;
/// How far the pointer moves with the button down before a click is a drag (ours).
const DRAG: f32 = 4.;
/// How long the toast after a capture stays, and longer while the pointer is over it (ours).
const TOAST_TIME: Duration = Duration::from_secs(5);
/// How long "Capture in Progress" can stay, which the capture's toast replaces when it comes.
const PROGRESS_TIME: Duration = Duration::from_secs(60);
const COPIED: &str = "Screenshot Copied to Clipboard";
const SAVED: &str = "Saved to Library";

/// What the menu does with what's chosen, in Arc's order without Edit and New Easel, which aren't
/// built: one is a variant here, a row in ROWS and an arm in `Capture::done`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// The field at the menu's top: the system's share menu (interim: Arc's field lists people and
    /// apps of its own).
    SendTo,
    /// Into the Library's Media, which a private window has none of.
    SaveToLibrary,
    Copy,
    Save,
    Messages,
    /// Back to choosing.
    Retake,
}

/// The menu's rows under its field, a line between groups. Messages is the Mac's alone.
const ROWS: &[Option<Action>] = if cfg!(target_os = "macos") {
    &[
        Some(Action::SaveToLibrary),
        Some(Action::Copy),
        Some(Action::Save),
        Some(Action::Messages),
        None,
        Some(Action::Retake),
    ]
} else {
    &[
        Some(Action::SaveToLibrary),
        Some(Action::Copy),
        Some(Action::Save),
        None,
        Some(Action::Retake),
    ]
};

/// Whether the system has a share menu. Linux has none, so there sharing copies, and says so.
const SHARES: bool = !cfg!(target_os = "linux");

impl Action {
    fn label(self) -> &'static str {
        match self {
            Action::SendTo => "Send To…",
            Action::SaveToLibrary => "Save to Library",
            Action::Copy => "Copy",
            Action::Save => "Save…",
            Action::Messages => "Send via iMessage…",
            Action::Retake => "Retake",
        }
    }

    /// Lucide's nearest to Arc's SF Symbols.
    fn glyph(self) -> &'static str {
        match self {
            Action::SendTo | Action::Copy => "icons/copy.svg",
            Action::SaveToLibrary => "icons/library.svg",
            Action::Save => "icons/download.svg",
            Action::Messages => "icons/message-circle.svg",
            Action::Retake => "icons/rotate-ccw.svg",
        }
    }

    /// For the logs.
    fn name(self) -> &'static str {
        match self {
            Action::SendTo => "send-to",
            Action::SaveToLibrary => "save-to-library",
            Action::Copy => "copy",
            Action::Save => "save",
            Action::Messages => "messages",
            Action::Retake => "retake",
        }
    }
}

/// Where the system's share menu shows from.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Share {
    /// The menu's field, in the browser window's content.
    Menu(Bounds<Pixels>),
    /// The toast's Share button, at the pointer in the toast's window.
    Toast(Point<Pixels>),
    /// A new message in Messages.
    Messages,
}

/// What Capture asks of its window, the core and the system.
pub trait Host {
    /// Shows capture mode's window over the page, at `frame` in the browser window's content, as it
    /// draws now, with the keyboard.
    fn show(&self, frame: Bounds<Pixels>);
    fn hide(&self);
    /// The pointer is the camera, or the arrow while the menu shows.
    fn camera(&self, camera: bool);
    /// Asks the core for the box of the element at `at` in the page, which comes in `element_box`.
    fn element_at(&self, tab: u64, at: Point<Pixels>);
    /// Asks the core for a picture of `rect` of the page, of all of the page's view without one,
    /// or with `full_page` of the whole page, which comes in `captured` as `request`.
    fn capture(&self, tab: u64, request: u64, full_page: bool, rect: Option<Bounds<Pixels>>);
    /// The core can let go of what it held for capturing the tab's page.
    fn end(&self, tab: u64);
    fn copy(&self, png: &[u8]);
    /// Asks where to save it, and saves it.
    fn save(&self, png: &[u8]);
    /// Saves it in the Library's Media, away from the main thread.
    fn save_to_library(&self, png: &[u8]);
    fn share(&self, png: &[u8], share: Share);
}

/// The picture of the page that capture mode draws: as it is, and grey around what's chosen.
struct Snapshot {
    colour: Arc<RenderImage>,
    grey: Arc<RenderImage>,
}

/// Capture mode over one tab's page, from ⇧⌘2 until it leaves.
struct Session {
    tab: u64,
    /// The page in the browser window's content, which the window covers, and its corners' radius.
    frame: Bounds<Pixels>,
    radius: f32,
    /// The request for the picture of the page, which the window shows with.
    snapshot_request: u64,
    snapshot: Option<Snapshot>,
    /// The element under the pointer, in the page, in its view.
    hover: Option<Bounds<Pixels>>,
    /// Where the button went down, while it's down.
    press: Option<Point<Pixels>>,
    /// The rectangle dragged, once the press has moved far enough.
    drag: Option<Bounds<Pixels>>,
    /// What the user chose, which the menu shows at.
    chosen: Option<Bounds<Pixels>>,
    /// The picture asked for of what's chosen, and what to do with it once it comes.
    pending: Option<(u64, Action)>,
    /// Whether the menu offers Save to Library: not in a private window.
    library: bool,
    /// The browser window's menus' look, which the menu, the pill and the ring take.
    look: Look,
    /// What the log last said was drawn: the pill or the menu, and whether in the light look.
    drawn: Option<(&'static str, bool)>,
}

impl Session {
    fn page(&self) -> Bounds<Pixels> {
        Bounds::new(point(px(0.), px(0.)), self.frame.size)
    }

    /// What's lit: what's chosen, else the rectangle dragged, else the element under the pointer.
    fn lit(&self) -> Option<Bounds<Pixels>> {
        self.chosen.or(self.drag).or(self.hover)
    }

    /// Where the menu goes: 10 in from the ring's left and level with its top, inside the page.
    fn menu_origin(&self, chosen: Bounds<Pixels>) -> Point<Pixels> {
        let page = self.page();
        let x = f32::from(chosen.left()) - RING + MENU_LEFT;
        let y = f32::from(chosen.top()) - RING;
        let right = f32::from(page.size.width) - MENU_WIDTH - MARGIN;
        let bottom = f32::from(page.size.height) - self.menu_height() - MARGIN;
        point(px(x.min(right).max(MARGIN)), px(y.min(bottom).max(MARGIN)))
    }

    /// The menu's rows, without Save to Library where there's no Library to save to.
    fn rows(&self) -> impl Iterator<Item = Option<Action>> + use<> {
        let library = self.library;
        ROWS.iter().copied().filter(move |row| library || *row != Some(Action::SaveToLibrary))
    }

    /// The field, a line under it, the rows and their lines, inside the panel's padding.
    fn menu_height(&self) -> f32 {
        let rows = self.rows().map(|row| if row.is_some() { ROW } else { LINE }).sum::<f32>();
        PADDING + ROW + LINE + rows + PADDING
    }
}

pub struct Capture {
    host: Rc<dyn Host>,
    focus: FocusHandle,
    session: Option<Session>,
    /// A full page capture on its way: its tab and request.
    full_page: Option<(u64, u64)>,
    requests: u64,
}

struct CaptureWindow(WindowHandle<Capture>);

impl Global for CaptureWindow {}

/// Opens capture mode's window, hidden, for `host` to show over the page, ahead of time as the
/// command bar's is.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<Capture>> {
    cx.bind_keys([KeyBinding::new("escape", Leave, Some("Capture"))]);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(800.), px(600.)),
        ))),
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        kind: WindowKind::Floating,
        show: false,
        focus: false,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // The page's card has rounded corners, which the picture of it keeps.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| {
        cx.new(|cx| Capture {
            host,
            focus: cx.focus_handle(),
            session: None,
            full_page: None,
            requests: 0,
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(CaptureWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open Capture's window: {error}");
            None
        }
    }
}

fn update(cx: &mut App, change: impl FnOnce(&mut Capture, &mut Window, &mut Context<Capture>)) {
    if let Some(window) = cx.try_global::<CaptureWindow>().map(|capture| capture.0) {
        window.update(cx, change).ok();
    }
}

/// Starts capture mode over `tab`'s page, at `frame` in the browser window's content with corners
/// of `radius`, once the core has a picture of it, its menu offering Save to Library with
/// `library`, in the window's menus' `look`. If it's on, leaves it instead, as ⇧⌘2's second press
/// does.
pub fn start(
    tab: u64,
    frame: Bounds<Pixels>,
    radius: f32,
    library: bool,
    look: Look,
    cx: &mut App,
) {
    // Once the update that asked is over, as off the Mac ⇧⌘2's second press comes from capture
    // mode's own window, which can't be updated inside its own update.
    cx.defer(move |cx| start_now(tab, frame, radius, library, look, cx));
}

fn start_now(
    tab: u64,
    frame: Bounds<Pixels>,
    radius: f32,
    library: bool,
    look: Look,
    cx: &mut App,
) {
    update(cx, |capture, window, cx| {
        if capture.session.is_some() {
            return capture.leave("toggle", window, cx);
        }
        let request = capture.request();
        capture.session = Some(Session {
            tab,
            frame,
            radius,
            snapshot_request: request,
            snapshot: None,
            hover: None,
            press: None,
            drag: None,
            chosen: None,
            pending: None,
            library,
            look,
            drawn: None,
        });
        // check.sh reads this line.
        log!("browser: capture started");
        capture.host.capture(tab, request, false, None);
    });
}

/// The browser window over `tab` draws its menus in `look` now, as its appearance or its Space's
/// theme changed: capture mode over it, if it's on, draws in it from now on.
pub fn set_look(tab: u64, look: Look, cx: &mut App) {
    // Once the update that asked is over, as start's, since the shell asks as it draws.
    cx.defer(move |cx| {
        update(cx, |capture, _, cx| {
            let Some(session) = capture.session.as_mut().filter(|session| session.tab == tab) else {
                return;
            };
            if session.look != look {
                session.look = look;
                cx.notify();
            }
        });
    });
}

/// Captures all of `tab`'s page, beyond its view, and copies it.
pub fn full_page(tab: u64, cx: &mut App) {
    // Once the update that asked is over, as start's.
    cx.defer(move |cx| {
        update(cx, |capture, _, _| {
            if capture.full_page.is_some() {
                return;
            }
            let request = capture.request();
            capture.full_page = Some((tab, request));
            capture.host.capture(tab, request, true, None);
        });
    });
    toast::show("Capture in Progress", None, PROGRESS_TIME, cx);
}

/// The core's picture for `request`, a PNG, or none if the page couldn't give one.
pub fn captured(tab: u64, request: u64, png: Option<Vec<u8>>, cx: &mut App) {
    update(cx, |capture, window, cx| capture.captured(tab, request, png, window, cx));
}

/// The core's box of the element at `at` in `tab`'s page, if there's one.
pub fn element_box(tab: u64, at: Point<Pixels>, box_: Option<Bounds<Pixels>>, cx: &mut App) {
    update(cx, |capture, _, cx| {
        let Some(session) = capture.session.as_mut().filter(|session| session.tab == tab) else {
            return;
        };
        if session.chosen.is_some() || session.press.is_some() {
            return;
        }
        let page = session.page();
        let hover = box_.map(|box_| box_.intersect(&page)).filter(|box_| !box_.is_empty());
        if hover != session.hover {
            session.hover = hover;
            if let Some(box_) = hover {
                // check.sh reads this line.
                let (x, y) = (f32::from(at.x), f32::from(at.y));
                log!("browser: capture element at {x} {y}: {}", describe(box_));
            }
            cx.notify();
        }
    });
}

/// The user left capture mode: a click outside it, another window or another app. The host has
/// hidden it already.
pub fn dismissed(cx: &mut App) {
    update(cx, |capture, window, cx| capture.leave("dismissed", window, cx));
}

/// The tab closed, and capture mode over it.
pub fn tab_closed(tab: u64, cx: &mut App) {
    update(cx, |capture, window, cx| {
        if capture.full_page.is_some_and(|(full, _)| full == tab) {
            capture.full_page = None;
        }
        if capture.session.as_ref().is_some_and(|session| session.tab == tab) {
            capture.leave("tab closed", window, cx);
        }
    });
}

/// A rectangle for the log: x, y, width and height, in points.
fn describe(rect: Bounds<Pixels>) -> String {
    let Bounds { origin, size } = rect;
    let (x, y) = (f32::from(origin.x), f32::from(origin.y));
    format!("{x} {y} {} {}", f32::from(size.width), f32::from(size.height))
}

/// The picture of the page decoded, and a grey copy, in the BGRA GPUI draws.
fn snapshot(png: &[u8]) -> Option<Snapshot> {
    let decoded = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let decoded = decoded.into_rgba8();
    let (width, height) = decoded.dimensions();
    let mut colour = decoded.into_raw();
    let mut grey = colour.clone();
    for (colour, grey) in colour.chunks_exact_mut(4).zip(grey.chunks_exact_mut(4)) {
        let [r, g, b] = [colour[0], colour[1], colour[2]].map(f32::from);
        let level = ((0.299 * r + 0.587 * g + 0.114 * b) * DIM).round() as u8;
        grey.copy_from_slice(&[level, level, level, 255]);
        colour.swap(0, 2);
    }
    let image = |data| {
        let image = RgbaImage::from_raw(width, height, data)?;
        Some(Arc::new(RenderImage::new(vec![Frame::new(image)])))
    };
    Some(Snapshot { colour: image(colour)?, grey: image(grey)? })
}

impl Capture {
    fn request(&mut self) -> u64 {
        self.requests += 1;
        self.requests
    }

    fn captured(
        &mut self,
        tab: u64,
        request: u64,
        png: Option<Vec<u8>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.full_page == Some((tab, request)) {
            self.full_page = None;
            if self.session.as_ref().is_none_or(|session| session.tab != tab) {
                self.host.end(tab);
            }
            return self.done(Action::Copy, png, cx);
        }
        let Some(session) = self.session.as_mut().filter(|session| session.tab == tab) else {
            return;
        };
        if session.snapshot_request == request {
            let Some(png) = png else {
                return self.leave("no picture of the page", window, cx);
            };
            // Decoded away from the main thread, as a full screen page is millions of pixels.
            let decoded = cx.background_spawn(async move { snapshot(&png) });
            cx.spawn_in(window, async move |capture, cx| {
                let decoded = decoded.await;
                capture
                    .update_in(cx, |capture, window, cx| capture.show(request, decoded, window, cx))
                    .ok();
            })
            .detach();
        } else if let Some((_, action)) = session.pending.filter(|&(at, _)| at == request) {
            session.pending = None;
            let frame = session.frame;
            let field = session.chosen.map(|chosen| {
                let origin = session.menu_origin(chosen);
                Bounds::new(
                    frame.origin + origin + point(px(PADDING), px(PADDING)),
                    size(px(MENU_WIDTH - 2. * PADDING), px(ROW)),
                )
            });
            self.leave(action.name(), window, cx);
            match (action, field) {
                (Action::SendTo, _) if !SHARES => self.done(Action::Copy, png, cx),
                (Action::SendTo, Some(field)) => {
                    if let Some(png) = png {
                        self.host.share(&png, Share::Menu(field));
                    }
                }
                _ => self.done(action, png, cx),
            }
        }
    }

    /// The picture of the page is ready: the window shows it.
    fn show(
        &mut self,
        request: u64,
        snapshot: Option<Snapshot>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self.session.as_mut().filter(|s| s.snapshot_request == request) else {
            return;
        };
        let Some(snapshot) = snapshot else {
            return self.leave("no picture of the page", window, cx);
        };
        session.snapshot = Some(snapshot);
        let frame = session.frame;
        window.focus(&self.focus, cx);
        self.host.show(frame);
        // check.sh reads this line.
        log!("browser: capture mode at {}", describe(frame));
        cx.notify();
    }

    /// Does `action` with the picture that came of what was chosen.
    fn done(&mut self, action: Action, png: Option<Vec<u8>>, cx: &mut Context<Self>) {
        let Some(png) = png else {
            log!("browser: capture failed");
            return;
        };
        let png = Rc::new(png);
        match action {
            Action::Copy => {
                self.host.copy(&png);
                let save = {
                    let (host, png) = (self.host.clone(), png.clone());
                    toast::Button {
                        label: toast::Label::Text("Save".into()),
                        name: "save",
                        run: Rc::new(move |_, _| host.save(&png)),
                    }
                };
                let share = {
                    let (host, png) = (self.host.clone(), png.clone());
                    toast::Button {
                        label: toast::Label::Glyph("icons/share.svg"),
                        name: "share",
                        run: Rc::new(move |at, _| host.share(&png, Share::Toast(at))),
                    }
                };
                let buttons = if SHARES { vec![save, share] } else { vec![save] };
                // After this update, as the toast is another window's.
                cx.defer(|cx| toast::show_with_buttons(COPIED, buttons, TOAST_TIME, cx));
            }
            Action::SaveToLibrary => {
                self.host.save_to_library(&png);
                cx.defer(|cx| toast::show(SAVED, None, TOAST_TIME, cx));
            }
            Action::Save => self.host.save(&png),
            Action::Messages => self.host.share(&png, Share::Messages),
            Action::SendTo | Action::Retake => {}
        }
    }

    /// Leaves capture mode: the window hides, the page has the keyboard again.
    fn leave(&mut self, why: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.session.take() else {
            return;
        };
        self.host.hide();
        if self.full_page.is_none_or(|(tab, _)| tab != session.tab) {
            self.host.end(session.tab);
        }
        if let Some(snapshot) = session.snapshot {
            // Off the GPU at once: a picture of the page is tens of megabytes.
            window.drop_image(snapshot.colour).ok();
            window.drop_image(snapshot.grey).ok();
        }
        // check.sh reads this line.
        log!("browser: capture left ({why})");
        cx.notify();
    }

    fn moved(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(session) = self.session.as_mut().filter(|session| session.chosen.is_none())
        else {
            return;
        };
        let Some(press) = session.press else {
            // One question for each move: the core drops those the pointer has left behind.
            return self.host.element_at(session.tab, at);
        };
        let moved = (at - press).magnitude();
        if session.drag.is_none() && moved < f64::from(DRAG) {
            return;
        }
        let page = session.page();
        let at = point(at.x.clamp(px(0.), page.right()), at.y.clamp(px(0.), page.bottom()));
        session.drag = Some(Bounds::from_corners(
            point(press.x.min(at.x), press.y.min(at.y)),
            point(press.x.max(at.x), press.y.max(at.y)),
        ));
        cx.notify();
    }

    fn pressed(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        // A click away from the menu leaves, as a click outside any menu closes it (interim).
        if session.chosen.is_some() {
            return self.leave("click", window, cx);
        }
        session.press = Some(at);
    }

    fn released(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.as_mut().filter(|session| session.chosen.is_none())
        else {
            return;
        };
        if session.press.take().is_none() {
            return;
        }
        // A drag takes the rectangle dragged, a click the element under the pointer.
        let chosen = match session.drag.take() {
            Some(drag) => Some(drag).filter(|drag| !drag.is_empty()),
            None => session.hover,
        };
        let Some(chosen) = chosen else {
            return cx.notify();
        };
        session.chosen = Some(chosen);
        self.host.camera(false);
        // check.sh reads this line.
        log!("browser: capture chose {}", describe(chosen));
        cx.notify();
    }

    fn run(&mut self, action: Action, cx: &mut Context<Self>) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let Some(chosen) = session.chosen.filter(|_| session.pending.is_none()) else {
            return;
        };
        // check.sh reads this line.
        log!("browser: capture run {}", action.name());
        if action == Action::Retake {
            session.chosen = None;
            session.hover = None;
            self.host.camera(true);
            return cx.notify();
        }
        let tab = session.tab;
        self.requests += 1;
        let request = self.requests;
        if let Some(session) = self.session.as_mut() {
            session.pending = Some((request, action));
        }
        self.host.capture(tab, request, false, Some(chosen));
    }

    fn menu(
        &self,
        origin: Point<Pixels>,
        rows: Vec<Option<Action>>,
        look: Look,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let palette = look.palette();
        let highlight = look.highlight;
        // A row of ilo's menu: the Space's accent under the pointer, with the label solved on it.
        let row = |id: &'static str| {
            div()
                .id(id)
                .group(id)
                .h(px(ROW))
                .mx(px(PADDING))
                .px(px(ROW_X))
                .flex()
                .items_center()
                .gap(px(GAP))
                .rounded(px(ROW_RADIUS))
                .hover(|style| style.bg(highlight.fill).text_color(highlight.label))
                .active(|style| style.bg(highlight.pressed))
        };
        let line = || {
            div()
                .h(px(LINE))
                .mx(px(PADDING))
                .flex()
                .items_center()
                .child(div().mx(px(ROW_X)).h(px(1.)).flex_1().bg(rgba(palette.line)))
                .into_any_element()
        };
        let rows: Vec<_> = (rows.into_iter())
            .map(|row_| match row_ {
                Some(action) => row(action.name())
                    .on_click(cx.listener(move |capture, _, _, cx| capture.run(action, cx)))
                    .child(
                        svg()
                            .path(action.glyph())
                            .size(px(GLYPH))
                            .flex_none()
                            .text_color(rgba(palette.secondary))
                            .group_hover(action.name(), |style| style.text_color(highlight.label)),
                    )
                    .child(action.label())
                    .into_any_element(),
                None => line(),
            })
            .collect();
        // Arc's field, as a row tinted the menu's line colour. Its words are the label's, as the
        // secondary colour falls under 4.5:1 on the tint in the dark look.
        let field = row(Action::SendTo.name())
            .bg(rgba(palette.line))
            .on_click(cx.listener(|capture, _, _, cx| capture.run(Action::SendTo, cx)))
            .child(Action::SendTo.label());
        div()
            .id("capture-menu")
            .absolute()
            .left(origin.x)
            .top(origin.y)
            .w(px(MENU_WIDTH))
            .py(px(PADDING))
            .flex()
            .flex_col()
            .rounded(px(RADIUS))
            .bg(rgba(palette.fill))
            .shadow(Level::Popover.shadow())
            .text_size(px(TEXT))
            .text_color(rgba(palette.label))
            .occlude()
            // The rim is its own layer, so its border doesn't move the rows.
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .rounded(px(RADIUS))
                    .border_1()
                    .border_color(rgba(palette.rim)),
            )
            .child(field)
            .child(line())
            .children(rows)
    }
}

impl Render for Capture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The browser's shortcuts work here too, below capture mode's own keys, as the Mac's menu
        // bar has them in every window: a second ⇧⌘2 leaves.
        div().key_context(crate::shell::KEY_CONTEXT).size_full().child(self.mode(cx))
    }
}

impl Capture {
    fn mode(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let root = div()
            .id("capture")
            .key_context("Capture")
            .track_focus(&self.focus)
            .on_action(cx.listener(|capture, _: &Leave, window, cx| {
                capture.leave("escape", window, cx)
            }))
            .size_full()
            .font_family(crate::face::UI);
        if let Some(session) = self.session.as_mut().filter(|session| session.snapshot.is_some()) {
            let what = if session.chosen.is_some() { "menu" } else { "pill" };
            let drawn = (what, session.look.light);
            if session.drawn != Some(drawn) {
                session.drawn = Some(drawn);
                // check.sh waits for this line.
                let tone = if drawn.1 { "light" } else { "dark" };
                log!("browser: capture {} drawn {tone}", drawn.0);
            }
        }
        let Some(session) = &self.session else {
            return root;
        };
        let Some(snapshot) = &session.snapshot else {
            return root;
        };
        let (colour, grey) = (snapshot.colour.clone(), snapshot.grey.clone());
        let lit = session.lit();
        let radius = px(session.radius);
        let (look, palette) = (session.look, session.look.palette());
        let picture = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let corners = Corners::all(radius);
                window.paint_image(bounds, bounds, corners, grey.clone(), 0, false).ok();
                if let Some(lit) = lit {
                    let lit = Bounds::new(bounds.origin + lit.origin, lit.size);
                    let square = Corners::default();
                    window.paint_image(lit, bounds, square, colour.clone(), 0, false).ok();
                }
            },
        )
        .absolute()
        .size_full();
        let ring = lit.map(|lit| {
            div()
                .absolute()
                .left(lit.left() - px(RING))
                .top(lit.top() - px(RING))
                .w(lit.size.width + px(2. * RING))
                .h(lit.size.height + px(2. * RING))
                .border(px(RING))
                .border_color(look.highlight.fill)
                .rounded(px(RING_RADIUS))
        });
        let pill = session.chosen.is_none().then(|| {
            let page = session.page();
            div()
                .absolute()
                .left((page.size.width - px(PILL_WIDTH)) / 2.)
                .top(page.size.height - px(PILL_BOTTOM + PILL_HEIGHT))
                .w(px(PILL_WIDTH))
                .h(px(PILL_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(PILL_RADIUS))
                .bg(rgba(palette.fill))
                .border_1()
                .border_color(rgba(palette.rim))
                .shadow(Level::Popover.shadow())
                .text_size(px(PILL_TEXT_SIZE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgba(palette.label))
                .child(SharedString::from("Click or drag to capture a portion of this page"))
        });
        let menu = session.chosen.map(|chosen| {
            let origin = session.menu_origin(chosen);
            self.menu(origin, session.rows().collect(), look, cx)
        });
        root.on_mouse_move(cx.listener(|capture, event: &MouseMoveEvent, _, cx| {
            capture.moved(event.position, cx)
        }))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|capture, event: &MouseDownEvent, window, cx| {
                capture.pressed(event.position, window, cx)
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|capture, _: &MouseUpEvent, _, cx| capture.released(cx)),
        )
        .child(picture)
        .children(ring)
        .children(pill)
        .children(menu)
    }
}

#[cfg(test)]
mod tests;
