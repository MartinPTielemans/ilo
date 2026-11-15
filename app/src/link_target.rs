//! The link target: the address of the link the pointer is on, in a plate in the card's bottom left
//! corner, where Chrome has its status bubble at the window's (docs/ux-spec.md › Link target). GPUI
//! can't draw over the page's native view, so as the tooltip is, it's a window of its own that the
//! host shows over the browser window, as its child, and the mouse goes through it. What it asks of
//! its window goes through `Host`; where it goes, what it says and when the pointer is near it are
//! the shell's, which calls `plate` and `place`.

use std::{ops::Range, rc::Rc, time::Duration};
use web_time::Instant;

use gpui::{
    App, Bounds, Context, FontWeight, Global, Hsla, Pixels, SharedString, Task, TitlebarOptions,
    Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions,
    WindowTextSystem, div, point, prelude::*, px, size,
};

use crate::{palette::Palette, tooltip};

/// Ours (*interim*, Arc's isn't recorded): it waits this long with the pointer on a link, so a
/// pointer crossing a page toward the sidebar shows nothing, and Chrome's 80 ms is a flash at each
/// link it passes.
pub const DELAY: Duration = Duration::from_millis(150);
/// How soon after the plate went the next link's shows at once, as the pointer crosses from one
/// link to the next.
pub const GRACE: Duration = Duration::from_millis(400);
/// It shows in a frame, as a hover fill does, and fades out over this, so the gap between two
/// links that touch isn't a flicker.
pub const FADE_IN: Duration = Duration::ZERO;
pub const FADE_OUT: Duration = Duration::from_millis(120);
/// Arc's, from its release notes and help centre: the pointer resting on a link this long shows its
/// whole address. Chrome's status bubble waits 1.6 s.
pub const WHOLE_AFTER: Duration = Duration::from_millis(1500);
/// How often it asks whether the button is still down, while a press holds back what its window
/// would do. No OS tells the UI of a release in a page, and the plate should follow it within a frame.
pub const RELEASE_POLL: Duration = Duration::from_millis(16);
pub use crate::tokens::link_target::HEIGHT;
pub use crate::tokens::link_target::MARGIN;
pub use crate::tokens::link_target::NEAR;
pub use crate::tokens::link_target::RESTING_WIDTH;
/// Its window, wide enough for the whole address, which elides past it.
pub use crate::tokens::link_target::WINDOW_WIDTH;
pub const WINDOW_HEIGHT: f32 = HEIGHT;
use crate::elevation::{Level, WINDOW_MARGIN};
use crate::tokens::link_target::PAD_X;
use crate::tokens::link_target::PRIMARY;
use crate::tokens::link_target::RADIUS;
use crate::tokens::link_target::RIM;
/// The address in the secondary text colour, but its site in the primary, so one that differs from
/// the site it passes for stands out.
use crate::tokens::link_target::SECONDARY;
use crate::tokens::link_target::TEXT_SIZE;
const ELLIPSIS: &str = "…";
/// A longer tail than the plate could ever show is cut before it's measured: a data: link can be
/// a megabyte.
const LONG: usize = 480;
const LONG_HEAD: usize = 320;

/// What the core reports of a link: its address as the user sees it, and the bytes of it that
/// are the site's registrable domain, if it has a host.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub text: SharedString,
    pub domain: Range<usize>,
}

/// Which bottom corner of the card the plate is in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side {
    #[default]
    Left,
    Right,
}

impl Side {
    pub fn other(self) -> Self {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }
}

/// What the plate draws: the address in three runs, as laid out to fit, and how wide that makes
/// the plate.
#[derive(Clone, Debug, PartialEq)]
pub struct Plate {
    pub before: SharedString,
    pub site: SharedString,
    pub after: SharedString,
    pub width: f32,
}

impl Plate {
    /// The address as one string, for the logs and the checks.
    pub fn text(&self) -> String {
        format!("{}{}{}", self.before, self.site, self.after)
    }
}

/// How wide `text` is at the plate's size.
pub fn text_width(text: &str, system: &WindowTextSystem) -> f32 {
    tooltip::text_width(
        &text.to_string().into(),
        TEXT_SIZE,
        FontWeight::NORMAL,
        system,
    )
}

/// `text` cut to `room` wide by `measure`, with an ellipsis where the rest went: `share` of what
/// stays is from its start, so 1 cuts its end, 0 its start and a half its middle.
fn fit(text: &str, room: f32, share: f32, measure: &impl Fn(&str) -> f32) -> String {
    let chars: Vec<char> = text.chars().collect();
    if measure(text) <= room {
        return text.to_string();
    }
    let cut = |keep: usize| -> String {
        let head = (keep as f32 * share).ceil() as usize;
        let tail = keep - head;
        let mut cut: String = chars[..head].iter().collect();
        cut.push_str(ELLIPSIS);
        cut.extend(&chars[chars.len() - tail..]);
        cut
    };
    // The most it can keep: its width only grows with it.
    let (mut fits, mut over) = (0, chars.len());
    while fits + 1 < over {
        let keep = (fits + over) / 2;
        match measure(&cut(keep)) <= room {
            true => fits = keep,
            false => over = keep,
        }
    }
    cut(fits)
}

/// The plate for `target` in at most `max` wide, `measure` giving how wide a string is. The site
/// always stays: a long address loses the middle of its path first, then its subdomains from the
/// left, and the site itself only when it can't fit alone.
pub fn plate(target: &Target, max: f32, measure: impl Fn(&str) -> f32) -> Plate {
    let room = (max - 2. * PAD_X).max(0.);
    let text = target.text.as_ref();
    let domain = match text.get(target.domain.clone()) {
        Some(_) => target.domain.clone(),
        None => 0..0,
    };
    let (before, site, after) = (
        &text[..domain.start],
        &text[domain.clone()],
        &text[domain.end..],
    );
    // A tail this long is cut in the middle first, so measuring it stays quick.
    let long;
    let after = match after.chars().count() > LONG {
        true => {
            let chars: Vec<char> = after.chars().collect();
            long = format!(
                "{}{ELLIPSIS}{}",
                chars[..LONG_HEAD].iter().collect::<String>(),
                chars[chars.len() - (LONG - LONG_HEAD)..]
                    .iter()
                    .collect::<String>()
            );
            long.as_str()
        }
        false => after,
    };
    let head = measure(&format!("{before}{site}"));
    let (before, site, after) = if head + measure(after) <= room {
        (before.to_string(), site.to_string(), after.to_string())
    } else if head + measure(ELLIPSIS) <= room {
        (
            before.to_string(),
            site.to_string(),
            fit(after, room - head, 0.5, &measure),
        )
    } else if measure(site) + measure(ELLIPSIS) <= room {
        (
            fit(before, room - measure(site), 0., &measure),
            site.to_string(),
            String::new(),
        )
    } else {
        (String::new(), fit(site, room, 1., &measure), String::new())
    };
    let width = measure(&format!("{before}{site}{after}")) + 2. * PAD_X;
    Plate {
        before: before.into(),
        site: site.into(),
        after: after.into(),
        width: width.min(max),
    }
}

/// The plate's frame in the window, in `side` of the card, which is `card`: `MARGIN` in from the
/// card's edge and up from its bottom.
pub fn place(card: Bounds<Pixels>, width: f32, side: Side) -> Bounds<Pixels> {
    let left = match side {
        Side::Left => f32::from(card.left()) + MARGIN,
        Side::Right => f32::from(card.right()) - MARGIN - width,
    };
    let top = f32::from(card.bottom()) - MARGIN - HEIGHT;
    Bounds::new(
        point(px(left.max(0.)), px(top.max(0.))),
        size(px(width), px(HEIGHT)),
    )
}

/// Where the plate's window starts in the browser window, for the plate at `frame` in `side` of
/// the card: at the plate on the left, and ending at it on the right, so the window, wider than
/// the plate, reaches into the card rather than past the browser window's edge.
pub fn window_left(frame: Bounds<Pixels>, side: Side) -> f32 {
    match side {
        Side::Left => f32::from(frame.left()),
        Side::Right => f32::from(frame.right()) - WINDOW_WIDTH,
    }
}

/// Whether the pointer at (`x`, `y`) in the window is on `frame` or within `NEAR` of it.
pub fn near(frame: Bounds<Pixels>, x: f32, y: f32) -> bool {
    let (left, top) = (
        f32::from(frame.left()) - NEAR,
        f32::from(frame.top()) - NEAR,
    );
    let (right, bottom) = (
        f32::from(frame.right()) + NEAR,
        f32::from(frame.bottom()) + NEAR,
    );
    (left..=right).contains(&x) && (top..=bottom).contains(&y)
}

/// What the link target asks of its window.
pub trait Host {
    /// Shows the plate's window with its top left at (`left`, `top`) in browser window `window`,
    /// the one whose page has the link, or moves it there if it shows. The UI has kept it inside
    /// the window.
    fn show(&self, window: u64, left: f32, top: f32);
    /// Hides it.
    fn hide(&self);
    /// Whether a mouse button is down. While one is, the window doesn't show, move or hide. On
    /// Windows a window that does makes a move at the pointer, which a page with the button down
    /// takes as a drag. A press on a link with Alt held is then a selection, and its click is
    /// lost (#2666). Nothing should appear under a press on any OS.
    fn pressed(&self) -> bool;
}

/// The plate that waits for its delay, and where it goes.
struct Pending {
    window: u64,
    plate: Plate,
    side: Side,
    left: f32,
    top: f32,
}

pub struct LinkTarget {
    host: Rc<dyn Host>,
    /// The window's colour, which the plate takes, and the palette its text is drawn in.
    look: Box<dyn Fn(&App) -> (Hsla, Palette)>,
    /// The browser window the plate shows over, if it shows.
    shown: Option<u64>,
    /// What the window draws, at its left or right end, which stays as it fades out after the plate
    /// went.
    drawn: Option<(Plate, Side)>,
    pending: Option<Pending>,
    /// The one-shot timer a plate waits on. Dropping it cancels it.
    wait: Option<Task<()>>,
    /// When the plate last went.
    went: Option<Instant>,
    /// The plate went while a button was down, and its window waits for the release to go.
    hide_held: bool,
    /// Asks every RELEASE_POLL whether the button is up, while a press holds back the window.
    release: Option<Task<()>>,
}

struct LinkTargetWindow(WindowHandle<LinkTarget>);

impl Global for LinkTargetWindow {}

/// Opens the plate's window, hidden, for `host` to show over the browser window, ahead of time as
/// the tooltip's is. The plate takes `look`'s colour and palette.
pub fn open(
    host: Rc<dyn Host>,
    look: impl Fn(&App) -> (Hsla, Palette) + 'static,
    cx: &mut App,
) -> Option<WindowHandle<LinkTarget>> {
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
    let window = cx.open_window(options, |_, cx| {
        cx.new(|_| LinkTarget {
            host,
            look: Box::new(look),
            shown: None,
            drawn: None,
            pending: None,
            wait: None,
            went: None,
            hide_held: false,
            release: None,
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(LinkTargetWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the link target's window: {error}");
            None
        }
    }
}

/// The pointer is on a link in a page of browser window `window`, or the plate or its place
/// changed: `plate` shows over that window at `frame` in it, in `side` of the card. After DELAY,
/// one timer that `hide` cancels, or at once if a plate shows or went less than GRACE ago. A call
/// while the timer runs changes what it shows, and where.
pub fn show(window: u64, plate: Plate, frame: Bounds<Pixels>, side: Side, cx: &mut App) {
    crate::lazy_window::ready::<LinkTarget>(cx);
    let Some(target) = cx.try_global::<LinkTargetWindow>().map(|target| target.0) else {
        return;
    };
    let (left, top) = (window_left(frame, side), f32::from(frame.top()));
    target
        .update(cx, |target, _, cx| {
            target.pending = Some(Pending {
                window,
                plate,
                side,
                left,
                top,
            });
            let now = cx.background_executor().now();
            let soon = target
                .went
                .is_some_and(|went| now.duration_since(went) < GRACE);
            if target.shown.is_some() || soon {
                target.wait = None;
                target.reveal(cx);
            } else if target.wait.is_none() {
                target.wait = Some(cx.spawn(async move |target, cx| {
                    cx.background_executor().timer(DELAY).await;
                    target
                        .update(cx, |target, cx| {
                            target.wait = None;
                            target.reveal(cx)
                        })
                        .ok();
                }));
            }
        })
        .ok();
}

/// The pointer has left the link, or the page, the tab or the window's layout no longer has it
/// show: its timer is dropped, and its window goes if it shows.
pub fn hide(cx: &mut App) {
    let Some(window) = cx.try_global::<LinkTargetWindow>().map(|target| target.0) else {
        return;
    };
    window.update(cx, |target, _, cx| target.hide(cx)).ok();
}

/// The browser window the plate shows over, whose shell hears the pointer's moves while it shows.
pub fn over(cx: &App) -> Option<u64> {
    let window = cx.try_global::<LinkTargetWindow>()?.0;
    window.read(cx).ok()?.shown
}

impl LinkTarget {
    fn reveal(&mut self, cx: &mut Context<Self>) {
        if self.pending.is_some() && self.host.pressed() {
            return self.await_release(cx);
        }
        let Some(Pending {
            window,
            plate,
            side,
            left,
            top,
        }) = self.pending.take()
        else {
            return;
        };
        if self.shown.is_none() {
            // check.sh reads this line.
            log!("browser: link target plate {}", plate.text());
        }
        self.shown = Some(window);
        self.drawn = Some((plate, side));
        // Showing it moves it from where it went, so it needn't go first.
        self.hide_held = false;
        self.host.show(window, left, top);
        cx.notify();
    }

    /// The window fades out with what it drew, so this draws nothing new.
    fn hide(&mut self, cx: &mut Context<Self>) {
        self.wait = None;
        self.pending = None;
        if self.shown.take().is_some() {
            self.went = Some(cx.background_executor().now());
            if self.host.pressed() {
                self.hide_held = true;
                return self.await_release(cx);
            }
            self.hide_window();
        }
    }

    fn hide_window(&mut self) {
        self.hide_held = false;
        self.host.hide();
        // check.sh reads this line.
        log!("browser: link target hidden");
    }

    /// Once no button is down, does what the press held back: the plate due shows, if the pointer
    /// is still on its link, or else the window goes if the plate did.
    fn await_release(&mut self, cx: &mut Context<Self>) {
        if self.release.is_some() {
            return;
        }
        // check.sh reads this line.
        log!("browser: link target waits for the release");
        self.release = Some(cx.spawn(async move |target, cx| {
            loop {
                cx.background_executor().timer(RELEASE_POLL).await;
                let released = target
                    .update(cx, |target, cx| {
                        if target.host.pressed() {
                            return false;
                        }
                        target.release = None;
                        if target.pending.is_some() {
                            target.reveal(cx);
                        } else if target.hide_held {
                            target.hide_window();
                        }
                        true
                    })
                    .unwrap_or(true);
                if released {
                    break;
                }
            }
        }));
    }
}

impl Render for LinkTarget {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The plate is inset by the margin, which its shadow draws in.
        let root = div()
            .size_full()
            .p(px(WINDOW_MARGIN))
            .font_family(crate::face::UI)
            .flex();
        let Some((plate, side)) = self.drawn.clone() else {
            return root;
        };
        let root = match side {
            Side::Left => root,
            Side::Right => root.justify_end(),
        };
        let (fill, palette) = (self.look)(cx);
        let text = div()
            .absolute()
            .left(px(PAD_X))
            .h_full()
            .flex()
            .items_center()
            .whitespace_nowrap()
            .text_size(px(TEXT_SIZE))
            .text_color(palette.text(SECONDARY))
            .child(plate.before)
            .child(div().text_color(palette.text(PRIMARY)).child(plate.site))
            .child(plate.after);
        // The rim is its own layer, so the border doesn't shift what the fill's box holds.
        let rim = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded(px(RADIUS))
            .border(px(RIM))
            .border_color(Level::Popover.rim_in(&palette));
        root.child(
            div()
                .relative()
                .w(px(plate.width))
                .h(px(HEIGHT))
                .rounded(px(RADIUS))
                .shadow(Level::Popover.shadow())
                .bg(fill)
                .child(text)
                .child(rim),
        )
    }
}

#[cfg(test)]
mod tests;
