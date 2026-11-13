//! The tooltip: the name Arc shows after a moment over a folder and over a sidebar button, with
//! its shortcut's keys on chips after it (docs/ux-spec.md › Tooltips). A favorite's tile, which
//! Arc's names too, has its hover card instead (hover_card.rs), which names its tab. GPUI can't draw over the page's native view, and a tooltip may
//! cross the sidebar's edge onto it, so as the toast is, it's a window of its own that the host
//! shows over the browser window, as its child, and the mouse goes through it. What it asks of its
//! window goes through `Host`.

use std::{rc::Rc, sync::Arc, time::Duration};
use web_time::Instant;

use gpui::{
    App, Bounds, Context, Div, FontWeight, Global, Pixels, Rgba, SharedString, Size, Task, TextRun,
    TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind,
    WindowOptions, WindowTextSystem, div, font, point, prelude::*, px, rgb, size,
};

/// How long the pointer rests on a tile, row or button, from its last move, before its tooltip
/// shows (Arc 1.170, twice), so a pointer passing over on its way somewhere brings none.
pub const DELAY: Duration = Duration::from_millis(1600);
/// How soon after a tooltip went the next shows at once, as the pointer crosses the gap between
/// two tiles (ours: the system's half second).
pub const GRACE: Duration = Duration::from_millis(500);
/// It shows in a frame and fades out over 0.27 s, ease-out (Arc).
pub const FADE_IN: Duration = Duration::ZERO;
pub const FADE_OUT: Duration = Duration::from_millis(270);
pub use crate::tokens::tooltip::BESIDE;
pub use crate::tokens::tooltip::EDGE_INSET;
pub use crate::tokens::tooltip::GAP;
use crate::tokens::tooltip::HEIGHT;
/// Its window, wide enough for a long name, which truncates past it.
pub use crate::tokens::tooltip::WINDOW_WIDTH;
pub use crate::tokens::tooltip::{BUTTON_X, BUTTON_Y};
pub const WINDOW_HEIGHT: f32 = HEIGHT;
/// Round at half its height, but the corner at what it names, which is nearly square.
const RADIUS: f32 = HEIGHT / 2.;
use crate::elevation::{Level, WINDOW_MARGIN};
use crate::palette::{INK, OVERLAY_DARK, PAPER, over};
use crate::tokens::tooltip::CHIP;
use crate::tokens::tooltip::CHIP_AFTER_TEXT;
use crate::tokens::tooltip::CHIP_BODY;
use crate::tokens::tooltip::CHIP_GAP;
use crate::tokens::tooltip::CHIP_LIP;
use crate::tokens::tooltip::CHIP_LIP_FILL;
use crate::tokens::tooltip::CHIP_PADDING;
use crate::tokens::tooltip::CHIP_RADIUS;
use crate::tokens::tooltip::CHIP_TEXT_SIZE;
use crate::tokens::tooltip::END;
use crate::tokens::tooltip::NEAR_RADIUS;
use crate::tokens::tooltip::RIM;
use crate::tokens::tooltip::TEXT_SIZE;
use crate::tokens::tooltip::TEXT_X;

/// What a tooltip shows over, which holds what it names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Over {
    /// A browser window, by its id.
    Window(u64),
    /// One of the host's overlays over the browser window in front, such as the command bar, by
    /// its number. The overlay is a window of its own over the page, and the tooltip goes over it.
    Overlay(i32),
}

/// What the tooltip asks of its window.
pub trait Host {
    /// Shows the tooltip's window with its top left at (`left`, `top`) in the window it's `over`,
    /// the one the pointer is in, or moves it there if it shows. The UI has kept it inside that
    /// window.
    fn show(&self, over: Over, left: f32, top: f32);
    /// Hides it.
    fn hide(&self);
}

/// What a tooltip says: a name, and the keys of the shortcut that does the same, if one does.
#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub text: SharedString,
    pub keys: Vec<SharedString>,
}

impl Tip {
    /// A name alone.
    pub fn named(text: SharedString) -> Self {
        Tip {
            text,
            keys: Vec::new(),
        }
    }
}

/// Where a tooltip goes from what it names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    /// Off the bottom right of its glyph, the tooltip's square top left corner BUTTON_X right of
    /// the anchor's middle and BUTTON_Y below it, or its top right corner as far left of the
    /// middle, going left, if it would cross the window's right edge. That corner is the pointer.
    Under,
    /// Under it, the tooltip's text starting at the anchor's x, so it lines up with the row's
    /// title and its plate hangs outside.
    Text,
    /// Right of it, starting BESIDE past the anchor's x, its right edge, and centred on it
    /// top to bottom, as Arc's is beside the footer's Library button.
    Beside,
    /// `Beside` to its left, ending BESIDE short of the anchor's x, its left edge: toward the page
    /// from a sidebar on the right.
    BesideLeft,
}

/// What a tooltip hangs from: the tile, row or button it names, in the browser window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub x: f32,
    pub align: Align,
    /// The tile's, row's or button's top and bottom.
    pub top: f32,
    pub bottom: f32,
    /// The window's size, which the tooltip stays inside.
    pub window: (f32, f32),
}

impl Anchor {
    /// Off a tile or button whose frame is `frame`, its square corner off the middle of it.
    pub fn under(frame: Bounds<Pixels>, window: Size<Pixels>) -> Self {
        Anchor {
            x: f32::from(frame.origin.x + frame.size.width / 2.),
            align: Align::Under,
            top: f32::from(frame.origin.y),
            bottom: f32::from(frame.bottom()),
            window: (window.width.into(), window.height.into()),
        }
    }

    /// Under a row whose frame is `frame`, its text above the row's title, `title` in from the
    /// row's left.
    pub fn titled(frame: Bounds<Pixels>, title: f32, window: Size<Pixels>) -> Self {
        let x = f32::from(frame.origin.x) + title;
        Anchor {
            x,
            align: Align::Text,
            ..Self::under(frame, window)
        }
    }

    /// Beside a button whose plate's frame is `frame`, on its right, or on its `left`.
    pub fn beside(frame: Bounds<Pixels>, window: Size<Pixels>, left: bool) -> Self {
        let (x, align) = match left {
            true => (f32::from(frame.left()), Align::BesideLeft),
            false => (f32::from(frame.right()), Align::Beside),
        };
        Anchor {
            x,
            align,
            ..Self::under(frame, window)
        }
    }
}

/// Whether a tooltip `width` wide under what it hangs from has its square corner on the right, for
/// want of room to its right: it goes left from the anchor then, if there's room that side.
fn mirrored(anchor: Anchor, width: f32) -> bool {
    match anchor.align {
        Align::Under => {
            anchor.x + BUTTON_X + width > anchor.window.0 - EDGE_INSET
                && anchor.x - BUTTON_X - width >= EDGE_INSET
        }
        Align::BesideLeft => true,
        Align::Text | Align::Beside => false,
    }
}

/// Where a tooltip of `size` has its top left: off the bottom right of a button's glyph, under a
/// row, or above either the same way if there's no room below, or beside a button, and inside the
/// window, EDGE_INSET from its sides.
pub fn place(anchor: Anchor, (width, height): (f32, f32)) -> (f32, f32) {
    let left = match anchor.align {
        Align::Under if mirrored(anchor, width) => anchor.x - BUTTON_X - width,
        Align::Under => anchor.x + BUTTON_X,
        Align::Text => anchor.x - TEXT_X,
        Align::Beside => anchor.x + BESIDE,
        Align::BesideLeft => anchor.x - BESIDE - width,
    };
    // Past the edges it keeps EDGE_INSET from them, the left one first if it can't be both.
    let left = left
        .min(anchor.window.0 - width - EDGE_INSET)
        .max(EDGE_INSET);
    let middle = (anchor.top + anchor.bottom) / 2.;
    let top = match anchor.align {
        Align::Beside | Align::BesideLeft => middle - height / 2.,
        Align::Under if above(anchor, height) => middle - BUTTON_Y - height,
        Align::Under => middle + BUTTON_Y,
        _ if above(anchor, height) => anchor.top - GAP - height,
        _ => anchor.bottom + GAP,
    };
    (left, top.min(anchor.window.1 - height).max(0.))
}

/// Whether a tooltip `height` tall goes above what it hangs from, for want of room below.
fn above(anchor: Anchor, height: f32) -> bool {
    match anchor.align {
        Align::Beside | Align::BesideLeft => false,
        Align::Under => (anchor.top + anchor.bottom) / 2. + BUTTON_Y + height > anchor.window.1,
        Align::Text => anchor.bottom + GAP + height > anchor.window.1,
    }
}

/// Which corner of a tooltip is nearly square: the one at what it names. Arc's is the top left,
/// toward the button the tooltip is beside, and off a button's glyph it's the top left, at the
/// glyph's bottom right. Ours, *interim*: the top right if the tooltip goes left from there, and
/// the bottom ones when it goes above; a row's is the top left.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Near {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

fn near(anchor: Anchor, width: f32, height: f32) -> Near {
    match (above(anchor, height), mirrored(anchor, width)) {
        (false, false) => Near::TopLeft,
        (false, true) => Near::TopRight,
        (true, false) => Near::BottomLeft,
        (true, true) => Near::BottomRight,
    }
}

/// Where a tooltip's text and chips go, from its left, and how wide it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub width: f32,
    /// How wide the text draws, which is less than it would take if it truncates.
    pub text: f32,
    pub truncated: bool,
    /// Each chip's left and width.
    pub chips: Vec<(f32, f32)>,
}

/// Lays out a tooltip whose text takes `text` across, with chips `chips` wide after it, in its
/// window: a long name truncates, and the keys stay whole.
pub fn layout(text: f32, chips: &[f32]) -> Layout {
    let keys = match chips.len() {
        0 => 0.,
        count => CHIP_AFTER_TEXT + chips.iter().sum::<f32>() + CHIP_GAP * (count - 1) as f32,
    };
    let room = (WINDOW_WIDTH - TEXT_X - keys - END).max(0.);
    let (text, truncated) = (text.min(room), text > room);
    let mut left = TEXT_X + text + CHIP_AFTER_TEXT;
    let chips = (chips.iter())
        .map(|&width| {
            let chip = (left, width);
            left += width + CHIP_GAP;
            chip
        })
        .collect();
    Layout {
        width: TEXT_X + text + keys + END,
        text,
        truncated,
        chips,
    }
}

/// How wide `text` is in `size` pt of the system's font at `weight`.
pub(crate) fn text_width(
    text: &SharedString,
    size: f32,
    weight: FontWeight,
    system: &WindowTextSystem,
) -> f32 {
    let run = TextRun {
        len: text.len(),
        font: gpui::Font {
            weight,
            ..font(crate::face::UI)
        },
        color: rgb(PAPER).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    f32::from(
        system
            .shape_line(text.clone(), px(size), &[run], None)
            .width,
    )
}

/// How `tip` lays out: its text, then a chip for each key, as wide as its glyph and its padding
/// but no less than square.
fn measure(tip: &Tip, system: &WindowTextSystem) -> Layout {
    let text = text_width(&tip.text, TEXT_SIZE, FontWeight::SEMIBOLD, system);
    let chips: Vec<f32> = (tip.keys.iter())
        .map(|key| {
            let glyph = text_width(key, CHIP_TEXT_SIZE, FontWeight::SEMIBOLD, system);
            (glyph + 2. * CHIP_PADDING).max(CHIP)
        })
        .collect();
    layout(text, &chips)
}

/// What a tooltip says over something named `name`: nothing if it has no name.
pub fn text_for(name: &str) -> Option<SharedString> {
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string().into())
}

pub struct Tooltip {
    host: Rc<dyn Host>,
    /// For how wide a name and its keys are.
    system: Arc<WindowTextSystem>,
    /// What shows.
    shown: Option<Tip>,
    /// What the window draws, which stays as it fades out after the tooltip went.
    drawn: Option<(Tip, Layout, Near)>,
    /// The one-shot timer a tooltip waits on, and what it shows when it fires. Dropping it
    /// cancels it.
    wait: Option<(Task<()>, Waiting)>,
    /// When a tooltip last went.
    went: Option<Instant>,
}

/// A tooltip waiting to show: the window it shows over, what it says and what it hangs from.
type Waiting = (Over, Tip, Anchor);

struct TooltipWindow(WindowHandle<Tooltip>);

impl Global for TooltipWindow {}

/// Opens the tooltip's window, hidden, for `host` to show over the browser window, ahead of time
/// as the toast's is.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<Tooltip>> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(
                px(WINDOW_WIDTH + 2. * WINDOW_MARGIN),
                px(WINDOW_HEIGHT + 2. * WINDOW_MARGIN),
            ),
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
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |window, cx| {
        let system = window.text_system().clone();
        cx.new(|_| Tooltip {
            host,
            system,
            shown: None,
            drawn: None,
            wait: None,
            went: None,
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(TooltipWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the tooltip's window: {error}");
            None
        }
    }
}

/// The pointer has come onto the tile, row or button that says `tip` in browser window `window`,
/// which the tooltip hangs from at `anchor` in that window, wherever the pointer is on it. The
/// tooltip shows DELAY after the pointer last moved (`moved`), one timer that `dismiss` cancels,
/// or at once if another showed less than GRACE ago, as the system's does when the pointer moves
/// from one tile to the next.
pub fn hover(window: u64, tip: Tip, anchor: Anchor, cx: &mut App) {
    hover_over(Over::Window(window), tip, anchor, cx);
}

/// As `hover`, with what the tooltip names in the window it's `over`, which may be an overlay's.
pub fn hover_over(over: Over, tip: Tip, anchor: Anchor, cx: &mut App) {
    crate::lazy_window::ready::<Tooltip>(cx);
    let Some(tooltip) = cx.try_global::<TooltipWindow>().map(|tooltip| tooltip.0) else {
        return;
    };
    tooltip
        .update(cx, |tooltip, _, cx| {
            let now = cx.background_executor().now();
            let soon = tooltip
                .went
                .is_some_and(|went| now.duration_since(went) < GRACE);
            if tooltip.shown.is_some() || soon {
                tooltip.wait = None;
                tooltip.show(over, tip, anchor, cx);
                return;
            }
            tooltip.wait_for((over, tip, anchor), cx);
        })
        .ok();
}

/// The pointer has moved over a browser window: a tooltip waiting to show waits DELAY again, so it
/// shows once the pointer rests, as Arc's does. One showing stays.
pub fn moved(cx: &mut App) {
    let Some(tooltip) = cx.try_global::<TooltipWindow>().map(|tooltip| tooltip.0) else {
        return;
    };
    // Most moves have no tooltip waiting, and a read costs the window nothing.
    if tooltip.read(cx).is_ok_and(|tooltip| tooltip.wait.is_none()) {
        return;
    }
    tooltip
        .update(cx, |tooltip, _, cx| {
            if let Some((_, waiting)) = tooltip.wait.take() {
                tooltip.wait_for(waiting, cx);
            }
        })
        .ok();
}

/// The pointer has left, or a click, a scroll, a Space switch or the window losing the keyboard
/// ends the tooltip: its timer is dropped, and its window goes if it shows.
pub fn dismiss(cx: &mut App) {
    let Some(window) = cx.try_global::<TooltipWindow>().map(|tooltip| tooltip.0) else {
        return;
    };
    window.update(cx, |tooltip, _, cx| tooltip.dismiss(cx)).ok();
}

impl Tooltip {
    /// Shows `waiting` after DELAY, on a new timer that drops any before it.
    fn wait_for(&mut self, waiting: Waiting, cx: &mut Context<Self>) {
        let (over, tip, anchor) = waiting.clone();
        let timer = cx.spawn(async move |tooltip, cx| {
            cx.background_executor().timer(DELAY).await;
            tooltip
                .update(cx, |tooltip, cx| {
                    tooltip.wait = None;
                    tooltip.show(over, tip, anchor, cx)
                })
                .ok();
        });
        self.wait = Some((timer, waiting));
    }

    fn show(&mut self, over: Over, tip: Tip, anchor: Anchor, cx: &mut Context<Self>) {
        let layout = measure(&tip, &self.system);
        let (left, top) = place(anchor, (layout.width, HEIGHT));
        // check.sh reads this line, to see the tooltip hangs from what it names.
        log!("browser: tooltip at {left} {top} {} {HEIGHT}", layout.width);
        if self.shown.as_ref() != Some(&tip) {
            // check.sh reads this line.
            log!("browser: tooltip shown {}", label(&tip));
        }
        self.shown = Some(tip.clone());
        let near = near(anchor, layout.width, HEIGHT);
        self.drawn = Some((tip, layout, near));
        self.host.show(over, left, top);
        cx.notify();
    }

    /// The window fades out with what it drew, so this draws nothing new.
    fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.wait = None;
        if self.shown.take().is_some() {
            self.went = Some(cx.background_executor().now());
            self.host.hide();
            // check.sh reads this line.
            log!("browser: tooltip hidden");
        }
    }
}

/// How the log names `tip`: its text, then its keys as a menu shows them, Ctrl+Shift+L off the Mac.
fn label(tip: &Tip) -> String {
    let joint = if crate::shell::OFF_THE_MAC { "+" } else { "" };
    match tip.keys.is_empty() {
        true => tip.text.to_string(),
        false => format!("{} {}", tip.text, tip.keys.join(joint)),
    }
}

impl Render for Tooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // The plate is inset by the margin, which its shadow draws in.
        let root = div()
            .size_full()
            .p(px(WINDOW_MARGIN))
            .font_family(crate::face::UI);
        let Some((tip, layout, near)) = self.drawn.clone() else {
            return root;
        };
        log!("browser: tooltip drawn {}", layout.width);
        let text = div()
            .absolute()
            .left(px(TEXT_X))
            .h_full()
            .flex()
            .items_center()
            .text_size(px(TEXT_SIZE))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(PAPER))
            // Untruncated, it's as wide as it shaped, and a box that wide could round under it.
            .when(layout.truncated, |text| text.w(px(layout.text)).truncate())
            .child(tip.text);
        let chip_top = (HEIGHT - CHIP_BODY) / 2.;
        let chips = tip
            .keys
            .into_iter()
            .zip(layout.chips)
            .flat_map(|(key, (left, width))| {
                let chip = |top: f32, fill: Rgba| {
                    div()
                        .absolute()
                        .left(px(left))
                        .top(px(top))
                        .w(px(width))
                        .h(px(CHIP_BODY))
                        .rounded(px(CHIP_RADIUS))
                        .bg(fill)
                };
                // The lip is the chip's shape again, lower, under it.
                let body = (chip(chip_top, rgb(PAPER))
                    .flex()
                    .items_center()
                    .justify_center())
                .text_size(px(CHIP_TEXT_SIZE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(INK))
                .child(key);
                let lip = over(rgb(OVERLAY_DARK), rgb(PAPER), CHIP_LIP_FILL);
                [chip(chip_top + CHIP_LIP, lip), body]
            });
        // Round at half the height but for the corner toward what it names.
        let shape = |shape: Div| {
            let shape = shape.rounded(px(RADIUS));
            match near {
                Near::TopLeft => shape.rounded_tl(px(NEAR_RADIUS)),
                Near::TopRight => shape.rounded_tr(px(NEAR_RADIUS)),
                Near::BottomLeft => shape.rounded_bl(px(NEAR_RADIUS)),
                Near::BottomRight => shape.rounded_br(px(NEAR_RADIUS)),
            }
        };
        // The rim is its own layer, so the border doesn't shift what the fill's box holds.
        let rim = shape(div().absolute().top_0().left_0().size_full())
            .border(px(RIM))
            .border_color(Level::Popover.rim());
        root.child(
            shape(div().relative().w(px(layout.width)).h(px(HEIGHT)))
                .shadow(Level::Popover.shadow())
                .bg(rgb(OVERLAY_DARK))
                .child(text)
                .children(chips)
                .child(rim),
        )
    }
}

#[cfg(test)]
mod tests;
