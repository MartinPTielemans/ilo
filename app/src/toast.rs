//! The toast: Arc's note at the top right of the window after something it does without showing
//! it, such as "Space deleted! Use ⌘Z to undo." (docs/ux-spec.md › Toast). GPUI can't draw over
//! the page's native view, so as the zoom HUD is, it's a window of its own that the host shows
//! over the browser window's top right corner, as its child, and the mouse goes through it to the
//! page, unless it has buttons, as Capture's does. What it asks of its window goes through `Host`.

use std::{cell::Cell, rc::Rc, time::Duration};
use web_time::Instant;

use gpui::{
    App, Bounds, ClickEvent, Context, FontWeight, Global, Hsla, Pixels, Point, Role, SharedString,
    Task, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions, accesskit::Live, canvas, div, linear_color_stop, linear_gradient,
    point, prelude::*, px, size, svg,
};

use crate::palette::{Palette, STATE_HOVERED, STATE_PRESSED};
use crate::shell::{CARD_INSET, cubic_bezier, scaled::scaled_from_top, spring};

// Arc's, measured at 2× (docs/ux-spec.md › Toast): 46.5 tall, 8 inside the card's top right
// corner, its text 12.5 in from its left and 13.5 from its right, and a key's chip 8 from the words
// each side of it.
use crate::tokens::toast::HEIGHT;
const RIGHT: f32 = CARD_INSET + 8.;
const TOP: f32 = CARD_INSET + 8.;
use crate::elevation::{Level, WINDOW_MARGIN};
use crate::tokens::toast::CHIP_FILL;
use crate::tokens::toast::CHIP_GAP;
use crate::tokens::toast::CHIP_GLYPH;
use crate::tokens::toast::CHIP_HEIGHT;
use crate::tokens::toast::CHIP_RADIUS;
use crate::tokens::toast::CHIP_X;
use crate::tokens::toast::KEY_COLOR;
use crate::tokens::toast::KEY_GAP;
use crate::tokens::toast::RADIUS;
use crate::tokens::toast::SHADE;
use crate::tokens::toast::TEXT_LEFT;
use crate::tokens::toast::TEXT_RIGHT;
use crate::tokens::toast::TEXT_SIZE;
pub use crate::tokens::toast::WINDOW_HEIGHT;
/// Arc's motion: it grows from 0.7 of its size about the middle of its top edge on a spring of
/// 0.32 s and ζ 0.52, fading in over the first 0.1 s, and leaves the way it came over 0.07 s,
/// speeding up.
const FROM: f32 = 0.7;
const RESPONSE: f32 = 0.32;
const DAMPING: f32 = 0.52;
const FADE_IN: f32 = 0.1;
const LEAVE: f32 = 0.07;
/// Asked for again while it's in place, it stays and bumps (Arc 1.171, measured by an outside
/// reverse-engineering, *interim*, #1404): to 1.044 of its size in 40 ms, easing out, and back by
/// 0.2 s.
const BUMP: f32 = 1.044;
const BUMP_UP: f32 = 0.04;
const BUMP_TIME: f32 = 0.2;
/// The note a window shows as it goes full screen (Arc 1.170 rec at 2×): 41 tall, 4 from the
/// window's top and its right edge on the card's, in the toast's look and motion, for 4.8 s. Off
/// the Mac, Chrome's words there, as keys are named in words off the Mac (#772).
const NOTE: &str = match crate::shell::OFF_THE_MAC {
    true => "Press Esc to exit full screen",
    false => "To exit full screen, press ⎋.",
};
use crate::tokens::toast::NOTE_HEIGHT;
use crate::tokens::toast::NOTE_TOP;
const NOTE_RIGHT: f32 = CARD_INSET;
const NOTE_TIME: Duration = Duration::from_millis(4800);

/// The name of the Undo button, for the logs and the checks.
const UNDO_BUTTON: &str = "undo";

/// What the toast asks of its window.
pub trait Host {
    /// Shows the toast's window at the browser window's top right.
    fn show(&self);
    /// Hides it.
    fn hide(&self);
    /// It takes clicks, while it has buttons, or lets the mouse through to the page. Even then
    /// only its plate does, and the margin round it, where its shadow falls, lets them through.
    fn take_clicks(&self, takes: bool);
    /// Where the plate is in the window, which changes with the words and the buttons, and is
    /// told again once the toast has hidden.
    fn plate(&self, bounds: Bounds<Pixels>);
}

/// What a button after the toast's words shows.
#[derive(Clone, Debug, PartialEq)]
pub enum Label {
    Text(SharedString),
    /// A glyph's path in the assets.
    Glyph(&'static str),
    /// Words and a key's name after them, in the button's chip: Undo ⌘Z.
    Keyed(SharedString, SharedString),
}

/// A button after the toast's words, such as Capture's Save. A click runs it with where the
/// pointer is in the toast's window, and the toast leaves.
#[derive(Clone)]
pub struct Button {
    pub label: Label,
    /// For the logs and the checks.
    pub name: &'static str,
    pub run: Rc<dyn Fn(Point<Pixels>, &mut App)>,
}

/// How big the toast is drawn, from 0.7 to its size (1) and a little past it, and how opaque.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pose {
    scale: f32,
    opacity: f32,
}

impl Pose {
    /// `seconds` after its window was first on screen.
    fn arriving(seconds: f32) -> Self {
        Pose {
            scale: FROM + (1. - FROM) * spring(seconds, RESPONSE, DAMPING).unwrap_or(1.),
            opacity: (seconds / FADE_IN).min(1.),
        }
    }

    /// `seconds` after it was asked for again in place: a little bigger, and back.
    fn bumped(self, seconds: f32) -> Self {
        Pose {
            scale: self.scale * bump(seconds).unwrap_or(1.),
            ..self
        }
    }

    /// `seconds` after it started leaving, from where the motion was then.
    fn leaving(self, seconds: f32) -> Self {
        let t = cubic_bezier(0.42, 0., 1., 1., (seconds / LEAVE).min(1.));
        Pose {
            scale: self.scale + (FROM - self.scale) * t,
            opacity: self.opacity * (1. - t),
        }
    }
}

/// How much bigger the bump makes the toast `seconds` in, none once it's over.
fn bump(seconds: f32) -> Option<f32> {
    let up = if seconds < BUMP_UP {
        cubic_bezier(0., 0., 0.58, 1., seconds.max(0.) / BUMP_UP)
    } else if seconds < BUMP_TIME {
        1. - cubic_bezier(
            0.42,
            0.,
            0.58,
            1.,
            (seconds - BUMP_UP) / (BUMP_TIME - BUMP_UP),
        )
    } else {
        return None;
    };
    Some(1. + (BUMP - 1.) * up)
}

struct Shown {
    text: SharedString,
    /// A key's chip after the text, and the words after it.
    key: Option<(SharedString, SharedString)>,
    buttons: Vec<Button>,
    /// The pointer is over it, which keeps a toast with buttons until it leaves.
    hovered: bool,
    /// Its time is up, and it leaves once the pointer does.
    expired: bool,
    /// When its window was first on screen, where the motion starts.
    shown: Option<Instant>,
    /// When it was last asked for again while in place, which bumps it.
    bumped: Option<Instant>,
    /// Once it's leaving: when, and where the motion was.
    leaving: Option<(Instant, Pose)>,
    settled: bool,
    /// Whether it's the note for full screen, which goes when the window leaves full screen.
    full_screen: bool,
    /// The tab a held toast is about, which stays until it's answered or `hide_held` takes it.
    held: Option<u64>,
    /// Whether it's the resting toast, which comes back after the others.
    resting: bool,
    _timer: Task<()>,
}

impl Shown {
    fn pose(&self) -> Pose {
        let arrived = Pose::arriving(self.shown.map_or(0., |shown| shown.elapsed().as_secs_f32()));
        match self.leaving {
            Some((at, from)) => from.leaving(at.elapsed().as_secs_f32()),
            None => match self.bumped {
                Some(at) => arrived.bumped(at.elapsed().as_secs_f32()),
                None => arrived,
            },
        }
    }

    /// Whether it's still on its way in or out, or bumping. Under Reduce Motion it's in once it
    /// has faded in, as it's drawn at its size and never bumps.
    fn moving(&self, cx: &App) -> bool {
        let bumping = self
            .bumped
            .is_some_and(|at| bump(at.elapsed().as_secs_f32()).is_some());
        match (self.shown, self.leaving) {
            (_, Some(_)) | (None, _) => true,
            (Some(shown), None) if crate::motion::still(cx) => {
                shown.elapsed().as_secs_f32() < FADE_IN
            }
            (Some(shown), None) => {
                bumping || spring(shown.elapsed().as_secs_f32(), RESPONSE, DAMPING).is_some()
            }
        }
    }
}

pub struct Toast {
    host: Rc<dyn Host>,
    /// The window's colour, which the toast takes, and the palette its text is drawn in.
    look: Box<dyn Fn(&App) -> (Hsla, Palette)>,
    shown: Option<Shown>,
    /// Whether the host was last told it takes clicks.
    takes_clicks: bool,
    /// Where the host was last told the plate is.
    plate: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// What shows whenever no other toast does, until `hide_resting`, as while a trace records.
    resting: Option<(SharedString, Vec<Button>)>,
}

struct ToastWindow(WindowHandle<Toast>);

impl Global for ToastWindow {}

/// Opens the toast's window, hidden, for `host` to show over the browser window, ahead of time as
/// the find bar's is. The toast takes `look`'s colour and palette. The host makes the window as
/// wide as the page it shows over, and a toast too long for that shortens its words.
pub fn open(
    host: Rc<dyn Host>,
    look: impl Fn(&App) -> (Hsla, Palette) + 'static,
    cx: &mut App,
) -> Option<WindowHandle<Toast>> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            point(px(0.), px(0.)),
            size(px(WINDOW_HEIGHT), px(WINDOW_HEIGHT)),
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
        // The toast and its shadow are drawn.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| {
        cx.new(|_| Toast {
            host,
            look: Box::new(look),
            shown: None,
            takes_clicks: false,
            plate: Rc::default(),
            resting: None,
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(ToastWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the toast's window: {error}");
            None
        }
    }
}

/// Shows `text`, then `key`'s chip and the words after it if there's a key, for `time` from now,
/// in place of any toast showing.
pub fn show(text: &str, key: Option<(&str, &str)>, time: Duration, cx: &mut App) {
    present(text, key, Vec::new(), Some(time), false, None, cx);
}

/// Shows `text` with `buttons` after it, for `time` from now and for as long as the pointer is
/// over it, in place of any toast showing.
pub fn show_with_buttons(text: &str, buttons: Vec<Button>, time: Duration, cx: &mut App) {
    present(text, None, buttons, Some(time), false, None, cx);
}

/// Shows `text` about `tab` with `buttons` after it, in place of any toast showing, until a button
/// is pressed or `hide_held` takes it away, as for a page that stopped answering.
pub fn show_held(text: &str, buttons: Vec<Button>, tab: u64, cx: &mut App) {
    present(text, None, buttons, None, false, Some(tab), cx);
}

/// Shows `text` with `buttons` after it, in place of any toast showing, until a button is pressed
/// or another toast takes its place, as for a question that can wait, such as whether to send a
/// crash's report.
pub fn show_until_answered(text: &str, buttons: Vec<Button>, cx: &mut App) {
    present(text, None, buttons, None, false, None, cx);
}

/// Shows `text` with `buttons` after it, in place of any toast showing, until a button is pressed
/// or `hide_resting` takes it away. Another toast takes its place for that one's time, and it comes
/// back once that one has gone, as it says what keeps on, such as a trace recording.
pub fn show_resting(text: &str, buttons: Vec<Button>, cx: &mut App) {
    present(text, None, buttons.clone(), None, false, None, cx);
    let Some(window) = cx.try_global::<ToastWindow>().map(|toast| toast.0) else {
        return;
    };
    window
        .update(cx, |toast, _, _| {
            toast.resting = Some((text.to_string().into(), buttons));
            if let Some(shown) = &mut toast.shown {
                shown.resting = true;
            }
        })
        .ok();
}

/// Takes away the resting toast, now and for good.
pub fn hide_resting(cx: &mut App) {
    let Some(window) = cx.try_global::<ToastWindow>().map(|toast| toast.0) else {
        return;
    };
    window
        .update(cx, |toast, _, cx| {
            toast.resting = None;
            if toast.shown.as_ref().is_some_and(|shown| shown.resting) {
                toast.leave(cx);
            }
        })
        .ok();
}

/// Takes away the held toast about `tab`, if it shows.
pub fn hide_held(tab: u64, cx: &mut App) {
    let Some(window) = cx.try_global::<ToastWindow>().map(|toast| toast.0) else {
        return;
    };
    window
        .update(cx, |toast, _, cx| {
            if toast
                .shown
                .as_ref()
                .is_some_and(|shown| shown.held == Some(tab))
            {
                toast.leave(cx);
            }
        })
        .ok();
}

/// Shows `text` with an Undo button after it, which runs `undo` as ⌘Z does, in place of any toast
/// showing.
pub fn show_undo(text: &str, undo: Rc<dyn Fn(&mut App)>, time: Duration, cx: &mut App) {
    let undo = Button {
        label: Label::Keyed("Undo".into(), crate::shell::keystroke_label("cmd-z").into()),
        name: UNDO_BUTTON,
        run: Rc::new(move |_, cx| undo(cx)),
    };
    show_with_buttons(text, vec![undo], time, cx);
}

/// Whether a toast with `text` shows and isn't leaving, so a question asked again isn't shown
/// twice.
pub fn showing(text: &str, cx: &App) -> bool {
    let Some(window) = cx.try_global::<ToastWindow>().map(|toast| toast.0) else {
        return false;
    };
    (window.read(cx).ok())
        .and_then(|toast| toast.shown.as_ref())
        .is_some_and(|shown| shown.leaving.is_none() && shown.text == text)
}

/// The undo it offered has been done some other way: its toast goes, if it shows, whether it
/// offers the undo with its button or names its key, as "Space deleted! Use ⌘Z to undo." does.
pub fn hide_undo(cx: &mut App) {
    let Some(window) = cx.try_global::<ToastWindow>().map(|toast| toast.0) else {
        return;
    };
    let undo_key = crate::shell::keystroke_label("cmd-z");
    window
        .update(cx, |toast, _, cx| {
            let undoing = toast.shown.as_ref().is_some_and(|shown| {
                shown
                    .buttons
                    .iter()
                    .any(|button| button.name == UNDO_BUTTON)
                    || shown
                        .key
                        .as_ref()
                        .is_some_and(|(key, _)| **key == *undo_key)
            });
            if undoing {
                toast.leave(cx);
            }
        })
        .ok();
}

/// Shows how to leave full screen, as the window has gone full screen or another Space shows in
/// it, in place of any toast showing.
pub fn show_full_screen(cx: &mut App) {
    present(NOTE, None, Vec::new(), Some(NOTE_TIME), true, None, cx);
}

/// The window has left full screen: the note on how to goes, if it shows.
pub fn hide_full_screen(cx: &mut App) {
    let Some(window) = cx.try_global::<ToastWindow>().map(|toast| toast.0) else {
        return;
    };
    window
        .update(cx, |toast, _, cx| {
            if toast.shown.as_ref().is_some_and(|shown| shown.full_screen) {
                toast.leave(cx);
            }
        })
        .ok();
}

fn present(
    text: &str,
    key: Option<(&str, &str)>,
    buttons: Vec<Button>,
    time: Option<Duration>,
    full_screen: bool,
    held: Option<u64>,
    cx: &mut App,
) {
    crate::lazy_window::ready::<Toast>(cx);
    let Some(window) = cx.try_global::<ToastWindow>().map(|toast| toast.0) else {
        return;
    };
    window
        .update(cx, |toast, _, cx| {
            // check.sh reads this line.
            let words = key.map_or(String::new(), |(key, rest)| format!(" {key} {rest}"));
            let names: Vec<&str> = buttons.iter().map(|button| button.name).collect();
            let names = if names.is_empty() {
                String::new()
            } else {
                format!(" [{}]", names.join(", "))
            };
            log!("browser: toast shown {text}{words}{names}");
            let timer = match time {
                Some(time) => cx.spawn(async move |toast, cx| {
                    cx.background_executor().timer(time).await;
                    toast.update(cx, |toast, cx| toast.expire(cx)).ok();
                }),
                None => Task::ready(()),
            };
            // One already in place stays there with the new words and time, and bumps, as nothing
            // else would show it was asked for again; one leaving comes back. Under Reduce Motion
            // it doesn't scale.
            let shown = toast.shown.take().filter(|shown| shown.leaving.is_none());
            let in_place = shown.as_ref().is_some_and(|shown| shown.shown.is_some());
            let bumped = (in_place && !crate::motion::still(cx)).then(|| {
                // check.sh reads this line.
                log!("browser: toast bumped");
                Instant::now()
            });
            toast.shown = Some(Shown {
                text: text.to_string().into(),
                key: key.map(|(key, rest)| (key.to_string().into(), rest.to_string().into())),
                buttons,
                hovered: shown.as_ref().is_some_and(|shown| shown.hovered),
                expired: false,
                shown: shown.and_then(|shown| shown.shown),
                bumped,
                leaving: None,
                settled: false,
                full_screen,
                held,
                resting: false,
                _timer: timer,
            });
            toast.take_clicks(cx);
            toast.host.show();
            cx.notify();
        })
        .ok();
}

impl Toast {
    /// Tells the host whether the toast takes clicks: while it has buttons and isn't leaving.
    fn take_clicks(&mut self, _: &mut Context<Self>) {
        let takes = self
            .shown
            .as_ref()
            .is_some_and(|shown| !shown.buttons.is_empty() && shown.leaving.is_none());
        if takes != self.takes_clicks {
            self.takes_clicks = takes;
            self.host.take_clicks(takes);
        }
    }

    /// Its time is up: it leaves, unless the pointer is over its buttons, until it leaves them.
    fn expire(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = &mut self.shown else {
            return;
        };
        if shown.hovered && !shown.buttons.is_empty() {
            shown.expired = true;
            return;
        }
        self.leave(cx);
    }

    fn hover(&mut self, hovered: bool, cx: &mut Context<Self>) {
        let Some(shown) = self.shown.as_mut().filter(|shown| shown.leaving.is_none()) else {
            return;
        };
        if shown.hovered != hovered {
            // check.sh reads this line: a toast with the pointer on it stays past its time.
            log!("browser: toast hovered {hovered}");
        }
        shown.hovered = hovered;
        if !hovered && shown.expired {
            self.leave(cx);
        }
    }

    fn press(&mut self, index: usize, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(button) = self
            .shown
            .as_ref()
            .and_then(|shown| shown.buttons.get(index))
        else {
            return;
        };
        let run = button.run.clone();
        // check.sh reads this line.
        log!("browser: toast button {}", button.name);
        if self.shown.as_ref().is_some_and(|shown| shown.resting) {
            self.resting = None;
        }
        self.leave(cx);
        cx.defer(move |cx| run(position, cx));
    }

    fn leave(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = &mut self.shown else {
            return;
        };
        if shown.leaving.is_some() {
            return;
        }
        shown.leaving = Some((Instant::now(), shown.pose()));
        self.take_clicks(cx);
        let Some(shown) = &mut self.shown else {
            return;
        };
        shown._timer = cx.spawn(async move |toast, cx| {
            cx.background_executor()
                .timer(Duration::from_secs_f32(LEAVE))
                .await;
            toast
                .update(cx, |toast, cx| {
                    toast.shown = None;
                    toast.plate.set(None);
                    toast.host.hide();
                    // check.sh reads this line.
                    log!("browser: toast hidden");
                    if let Some((text, buttons)) = toast.resting.clone() {
                        cx.defer(move |cx| show_resting(&text, buttons, cx));
                    }
                    cx.notify();
                })
                .ok();
        });
        cx.notify();
    }
}

impl Render for Toast {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div().size_full().flex().justify_end().items_start();
        let Some(shown) = &mut self.shown else {
            return root;
        };
        let (height, top, right) = match shown.full_screen {
            true => (NOTE_HEIGHT, NOTE_TOP, NOTE_RIGHT),
            false => (HEIGHT, TOP, RIGHT),
        };
        let root = root.pt(px(top)).pr(px(right)).font_family(crate::face::UI);
        // Its window is the page's width, and it keeps the margin its shadow falls in from the
        // window's left edge too: words that would make it wider shorten, where the window's edge
        // cut them (#1462).
        let widest = (window.viewport_size().width - px(right + WINDOW_MARGIN)).max(px(0.));
        // AppKit puts the window on screen some frames after it's asked to, and only then does
        // GPUI draw it, so the motion starts there rather than skip its start.
        if shown.shown.is_none() && window.is_visible() {
            shown.shown = Some(Instant::now());
        }
        let at_rest = !shown.moving(cx);
        if !at_rest {
            window.request_animation_frame();
        } else if !shown.settled {
            shown.settled = true;
            // check.sh reads this line.
            log!("browser: toast settled");
        }
        let pose = shown.pose();
        let (fill, palette) = (self.look)(cx);
        let bottom = Hsla {
            l: fill.l * SHADE,
            ..fill
        };
        let a11y = window.is_a11y_active();
        let buttons: Vec<_> = (shown.buttons.iter().enumerate())
            .map(|(index, button)| {
                let chip = div()
                    .id(("toast-button", index))
                    .h(px(CHIP_HEIGHT))
                    .px(px(CHIP_X))
                    .flex()
                    .items_center()
                    .rounded(px(CHIP_RADIUS))
                    .bg(palette.plate(CHIP_FILL))
                    .hover(move |style| style.bg(palette.state(CHIP_FILL, STATE_HOVERED)))
                    .active(move |style| style.bg(palette.state(CHIP_FILL, STATE_PRESSED)))
                    .font_weight(FontWeight::SEMIBOLD)
                    .when(a11y, |chip| {
                        let name = match &button.label {
                            // A keyed button goes by its words: Undo.
                            Label::Text(text) | Label::Keyed(text, _) => text.to_string(),
                            // A glyph's button goes by its name, "share" read as Share.
                            Label::Glyph(_) => {
                                let mut name = button.name.to_owned();
                                name[..1].make_ascii_uppercase();
                                name
                            }
                        };
                        chip.role(Role::Button).aria_label(name)
                    })
                    .on_click(cx.listener(move |toast, event: &ClickEvent, _, cx| {
                        toast.press(index, event.position(), cx)
                    }));
                match &button.label {
                    Label::Text(text) => chip.child(text.clone()),
                    Label::Glyph(path) => chip.child(
                        svg()
                            .path(*path)
                            .size(px(CHIP_GLYPH))
                            .text_color(palette.ink),
                    ),
                    // The key is fainter than the word it goes with.
                    Label::Keyed(text, key) => chip
                        .gap(px(KEY_GAP))
                        .child(text.clone())
                        .child(div().text_color(palette.text(KEY_COLOR)).child(key.clone())),
                }
            })
            .collect();
        // What it says, which VoiceOver says as it comes, wherever its cursor is.
        let said = match &shown.key {
            Some((key, rest)) => format!("{} {key} {rest}", shown.text),
            None => shown.text.to_string(),
        };
        let toast = div()
            .id("toast")
            .when(a11y, |toast| {
                toast
                    .role(Role::Status)
                    .aria_value(said)
                    .a11y_synthetic_children(|builder| builder.parent_node().set_live(Live::Polite))
            })
            .on_hover(cx.listener(|toast, hovered: &bool, _, cx| toast.hover(*hovered, cx)))
            .h(px(height))
            .flex_none()
            .max_w(widest)
            .pl(px(TEXT_LEFT))
            .pr(px(TEXT_RIGHT))
            .flex()
            .items_center()
            .gap(px(CHIP_GAP))
            .rounded(px(RADIUS))
            .border(px(0.5))
            .border_color(Level::Popover.rim())
            .bg(linear_gradient(
                180.,
                linear_color_stop(fill, 0.),
                linear_color_stop(bottom, 1.),
            ))
            .shadow(Level::Popover.shadow())
            .opacity(pose.opacity)
            .text_size(px(TEXT_SIZE))
            .font_weight(FontWeight::MEDIUM)
            .text_color(palette.ink)
            // Only the words give way; the buttons after them keep their size.
            .child(div().min_w_0().truncate().child(shown.text.clone()))
            .children(shown.key.clone().map(|(key, rest)| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(CHIP_GAP))
                    .child(
                        div()
                            .h(px(CHIP_HEIGHT))
                            .px(px(CHIP_X))
                            .flex()
                            .items_center()
                            .rounded(px(CHIP_RADIUS))
                            .bg(palette.plate(CHIP_FILL))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(key),
                    )
                    .child(rest)
            }))
            .children(buttons)
            // The plate is what takes clicks, so the host is told where it is once it has come to
            // rest, since it's drawn scaled until then, and the plate under a click is where it
            // was drawn.
            .child(
                canvas(
                    {
                        let (host, told) = (self.host.clone(), self.plate.clone());
                        move |bounds, window, _| {
                            // Inside the plate's 0.5 border.
                            let bounds = bounds.dilate(px(0.5));
                            if at_rest && told.replace(Some(bounds)) != Some(bounds) {
                                let (x, y) =
                                    (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
                                let (width, height) = (bounds.size.width, bounds.size.height);
                                let window = window.viewport_size();
                                // check.sh reads this line, to click just outside the plate and to see
                                // the window holds it whole.
                                log!(
                                    "browser: toast plate {x:.0} {y:.0} {:.0} {:.0} in {:.0} {:.0}",
                                    f32::from(width),
                                    f32::from(height),
                                    f32::from(window.width),
                                    f32::from(window.height)
                                );
                                host.plate(bounds);
                            }
                        }
                    },
                    |_, _, _, _| {},
                )
                // Over the plate's whole inside, not after its padding.
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        root.child(scaled_from_top(toast, pose.scale))
    }
}

#[cfg(test)]
mod tests;
