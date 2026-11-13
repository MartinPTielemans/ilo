//! The popover that makes a Space's theme, Arc's "Choose a Theme": beside the sidebar, its arrow
//! at the row or Space it was opened from, with the appearance buttons over a square field where
//! the colours are placed, − and + for how many, a row of swatches, and a wave slider for the
//! intensity and a dial for the grain (docs/ux-spec.md › Themes). The window's colour follows
//! each move live. Like the icon picker, it's a window of its own that the host shows over the
//! browser window, as its child; what it asks of that window goes through `Host`.
//!
//! The keyboard (#1383, ours: Arc's isn't recorded): tab and shift-tab go round the appearance
//! buttons, − and +, the swatches and the wave, ringed as Focus Sidebar rings a row; left and right
//! go along the buttons or swatches, or move the wave's knob; space and return press what's ringed.
//! The handles and the dial stay the mouse's: a swatch places a colour, as a click on the field
//! does.

use std::{cell::OnceCell, f32::consts::TAU, rc::Rc, sync::Arc};
use web_time::Instant;

use gpui::{
    App, Bounds, Context, DispatchPhase, Div, Entity, FocusHandle, Focusable, FontWeight, Global,
    Hsla, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder,
    Pixels, Point, RenderImage, Stateful, TitlebarOptions, Window, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions,
    accesskit::{Action, Role, Toggled},
    actions, canvas, div, fill, point,
    prelude::*,
    px, quad, rgb, rgba, size, svg,
};
use image::{Frame, RgbaImage};

use crate::{
    icon_picker,
    palette::{self, Palette, Swatch, Tone},
    shell::{KEY_CONTEXT, cubic_bezier},
    theme::{self, Appearance, DEFAULT_INTENSITY, MAX_COLORS, Place, Theme},
};

actions!(
    theme_picker,
    [Dismiss, NextStop, PreviousStop, KeyLeft, KeyRight, KeyPress]
);

use crate::tokens::theme_picker::ARROW;
use crate::tokens::theme_picker::ARROW_FOOT_RADIUS;
use crate::tokens::theme_picker::ARROW_TIP_RADIUS;
use crate::tokens::theme_picker::CARD_WIDTH;
pub use crate::tokens::theme_picker::HEIGHT;
/// How far inside where its sides meet the rounded tip is.
const TIP_SHORT: f32 = ARROW_TIP_RADIUS * (std::f32::consts::SQRT_2 - 1.);
pub const WIDTH: f32 = ARROW + CARD_WIDTH;
use crate::tokens::theme_picker::DOT_ALPHA;
/// Its grid of 1 pt dots.
use crate::tokens::theme_picker::DOT_PITCH;
use crate::tokens::theme_picker::EDGE_INSET;
use crate::tokens::theme_picker::FIELD;
use crate::tokens::theme_picker::FIELD_FILL;
use crate::tokens::theme_picker::FIELD_INSET;
use crate::tokens::theme_picker::FIELD_RADIUS;
use crate::tokens::theme_picker::FIELD_RIM;
use crate::tokens::theme_picker::GAP;
use crate::tokens::theme_picker::MARGIN;
use crate::tokens::theme_picker::RADIUS;
const HINT: &str = "Tap to pick a color for this space";
use crate::tokens::theme_picker::ADD_X;
use crate::tokens::theme_picker::APPEARANCE_CHOSEN;
use crate::tokens::theme_picker::APPEARANCE_COLOR;
use crate::tokens::theme_picker::APPEARANCE_GLYPH;
use crate::tokens::theme_picker::APPEARANCE_PLATE;
use crate::tokens::theme_picker::APPEARANCE_RADIUS;
use crate::tokens::theme_picker::APPEARANCE_X;
/// The appearance buttons, across the field's top: automatic, light and dark.
use crate::tokens::theme_picker::APPEARANCE_Y;
use crate::tokens::theme_picker::COUNT_COLOR;
use crate::tokens::theme_picker::COUNT_DISABLED;
use crate::tokens::theme_picker::COUNT_GLYPH;
use crate::tokens::theme_picker::COUNT_PRESSED;
use crate::tokens::theme_picker::COUNT_TARGET;
use crate::tokens::theme_picker::COUNT_Y;
use crate::tokens::theme_picker::KNOB_HEIGHT;
use crate::tokens::theme_picker::KNOB_LEFT;
use crate::tokens::theme_picker::KNOB_RIGHT;
use crate::tokens::theme_picker::KNOB_WIDTH;
use crate::tokens::theme_picker::REMOVE_X;
use crate::tokens::theme_picker::SLIDER_Y;
use crate::tokens::theme_picker::SWATCH;
use crate::tokens::theme_picker::SWATCH_PITCH;
use crate::tokens::theme_picker::SWATCH_X;
use crate::tokens::theme_picker::SWATCH_Y;
pub use crate::tokens::theme_picker::SWATCHES;
use crate::tokens::theme_picker::TRACK_FILL;
use crate::tokens::theme_picker::TRACK_HEIGHT;
use crate::tokens::theme_picker::TRACK_LEFT;
use crate::tokens::theme_picker::TRACK_RIGHT;
use crate::tokens::theme_picker::WAVE_AMPLITUDE;
use crate::tokens::theme_picker::WAVE_LEFT;
use crate::tokens::theme_picker::WAVE_OFF;
use crate::tokens::theme_picker::WAVE_ON;
use crate::tokens::theme_picker::WAVE_PERIODS;
use crate::tokens::theme_picker::WAVE_RIGHT;
use crate::tokens::theme_picker::WAVE_STROKE;
/// The grain's dial: 24 dots round a disc, those the knob has swept bright.
const DIAL: (f32, f32) = (298.5, 452.);
const DIAL_DOTS: usize = 24;
use crate::tokens::theme_picker::DIAL_DIM;
use crate::tokens::theme_picker::DIAL_DISC;
use crate::tokens::theme_picker::DIAL_DISC_FILL;
use crate::tokens::theme_picker::DIAL_DISC_RIM;
use crate::tokens::theme_picker::DIAL_DOT;
use crate::tokens::theme_picker::DIAL_RING;
use crate::tokens::theme_picker::DIAL_SWEPT;
const DIAL_KNOB: (f32, f32) = (7., 15.);
use crate::tokens::shell::{KEYED, KEYED_RING};
use crate::tokens::text;
use crate::tokens::theme_picker::DIAL_REACH;
use crate::tokens::theme_picker::HANDLE;
use crate::tokens::theme_picker::HANDLE_RING;
use crate::tokens::theme_picker::MAIN_HANDLE;
use crate::tokens::theme_picker::MAIN_RING;
/// How far the ring stands off a round swatch or the wave's knob (ours).
const RING_GAP: f32 = 1.;
/// How far left or right moves the wave's knob (ours: twenty steps end to end).
const KEY_STEP: f32 = 0.05;
/// Pressed, a handle grows by this in 60 ms, and shrinks back as long on release.
const PRESSED: f32 = 48. / 44.;
const PRESS: f32 = 0.06;
/// With a third colour, the second glides from the main one's complement to 150° round in 0.2 s,
/// and from a place of its own to its set angle as the count changes (ours).
const GLIDE: f32 = 0.2;

/// What the picker asks of its window.
pub trait Host {
    /// Shows the picker's window with its top left at `left`, `top` in browser window `window`,
    /// the one that asked for it, with the keyboard, in the shape of `corners` (x, y and radius,
    /// clockwise from the first, which it grows in from).
    fn show(&self, window: u64, left: f32, top: f32, corners: &[(f32, f32, f32)]);
    /// Hides it, and the browser window gets the keyboard back.
    fn hide(&self);
    /// A detent on the trackpad, as a drag reaches a stop, if the user has haptics on.
    fn detent(&self);
}

/// The theme as the user makes it, and whether they're done: false while they drag, true once
/// they let go or click, for it to be kept.
pub type Changed = Rc<dyn Fn(Theme, bool, &mut App)>;

/// What a press in the card lands on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Appearance(Appearance),
    Remove,
    Add,
    Swatch(usize),
    Intensity,
    Grain,
    /// A colour's handle, the main one 0.
    Handle(usize),
    Field,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    /// A handle, and where in it the pointer took it, from its middle, in the field's units.
    Handle(usize, f32, f32),
    Intensity,
    Grain,
}

struct Editing {
    space: u64,
    theme: Option<Theme>,
    /// The appearance chosen before there's a colour, for the first one to take.
    appearance: Appearance,
    changed: Changed,
    /// The arrow's tip, down the window, if it has one.
    arrow_y: Option<f32>,
    /// Whether it opened to the left, its arrow on its right.
    left: bool,
    /// False once it's closing: what it shows stays, for the host's shrink out.
    open: bool,
    drag: Option<Drag>,
    /// A handle's growth: which, since when, and whether it's pressed or let go.
    press: Option<(usize, Instant, bool)>,
    /// The second handle's glide between places: from, to and since when.
    glide: Option<(Place, Place, Instant)>,
    /// − or + while it's pressed.
    pressed: Option<Target>,
    /// What the keyboard is on, ringed, once tab has put it somewhere: an appearance button, − or
    /// +, a swatch or the wave.
    keyed: Option<Target>,
    /// Whether a frame of it has been drawn on screen since it opened.
    drawn: bool,
    /// How many colour handles the last frame drew at rest, and what it drew held: a handle, the
    /// wave's knob or the dial's, its grab left out.
    drawn_handles: Option<(usize, Option<Drag>)>,
    /// What it draws with: the Space's palette as it opened, which holds while the user drags
    /// through themes that would change it.
    palette: Palette,
}

pub struct ThemePicker {
    host: Rc<dyn Host>,
    focus: FocusHandle,
    editing: Option<Editing>,
    /// SWATCHES' marks, worked out as the picker opens, ahead of time.
    swatches: Vec<Swatch>,
}

struct ThemePickerWindow(WindowHandle<ThemePicker>);

impl Global for ThemePickerWindow {}

/// Opens the picker's window, hidden, for `host` to show over the browser window, ahead of time as
/// the icon picker's is.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<ThemePicker>> {
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(WIDTH), px(HEIGHT)));
    open_at(host, bounds, false, cx)
}

/// Opens the picker's window at `bounds`, shown if `show`, which the preview does.
pub fn open_at(
    host: Rc<dyn Host>,
    bounds: Bounds<Pixels>,
    show: bool,
    cx: &mut App,
) -> Option<WindowHandle<ThemePicker>> {
    cx.bind_keys([
        KeyBinding::new("escape", Dismiss, Some("ThemePicker")),
        KeyBinding::new("tab", NextStop, Some("ThemePicker")),
        KeyBinding::new("shift-tab", PreviousStop, Some("ThemePicker")),
        KeyBinding::new("left", KeyLeft, Some("ThemePicker")),
        KeyBinding::new("right", KeyRight, Some("ThemePicker")),
        KeyBinding::new("space", KeyPress, Some("ThemePicker")),
        KeyBinding::new("enter", KeyPress, Some("ThemePicker")),
    ]);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        // The preview's has none, so it hasn't the window buttons the host's takes off.
        titlebar: (!show).then_some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: None,
        }),
        kind: WindowKind::Floating,
        show,
        focus: show,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        // The card's shape is drawn, so the window around it is clear.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| {
        let swatches = SWATCHES
            .iter()
            .map(|&color| Swatch::of_theme(Some(&Theme::of_color(color))))
            .collect();
        cx.new(|cx| ThemePicker {
            host,
            focus: cx.focus_handle(),
            editing: None,
            swatches,
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(ThemePickerWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the theme picker's window: {error}");
            None
        }
    }
}

/// Where the picker opens in the browser window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum At {
    /// Beside a row, its arrow pointing at the middle of the row's end toward the page: the Create
    /// a Space form's Choose a Theme row, or a Library card's brush.
    Row(Point<f32>),
    /// With no arrow, against the sidebar's edge this far across, at the window's top, as Arc
    /// 1.171's opens from the Space's menu, over the page rather than the sidebar it colours.
    Edge(f32),
}

/// The card's corners from the window's top left, each with the radius it's rounded by, clockwise
/// from where it grows in from: the arrow's tip `arrow_y` down the window, or with no arrow the
/// card's top corner on the sidebar's side. `left` opens it to the left of what opened it, from a
/// sidebar on the right, its arrow on its right edge.
pub fn corners(arrow_y: Option<f32>, left: bool) -> Vec<(f32, f32, f32)> {
    corners_of(arrow_y, left, HEIGHT)
}

/// `corners` for a card of the picker's form `height` tall, as the Boost editor's is.
pub fn corners_of(arrow_y: Option<f32>, left: bool, height: f32) -> Vec<(f32, f32, f32)> {
    let card = [
        (ARROW, 0., RADIUS),
        (WIDTH, 0., RADIUS),
        (WIDTH, height, RADIUS),
        (ARROW, height, RADIUS),
    ];
    let mut corners = match arrow_y {
        None => card.to_vec(),
        Some(arrow_y) => {
            let mut corners = vec![
                (0., arrow_y, ARROW_TIP_RADIUS),
                (ARROW, arrow_y - ARROW, ARROW_FOOT_RADIUS),
            ];
            corners.extend(card);
            corners.push((ARROW, arrow_y + ARROW, ARROW_FOOT_RADIUS));
            corners
        }
    };
    if left {
        // Mirrored, the same corners run anticlockwise, so the rest go round the other way from
        // the first.
        for corner in &mut corners {
            corner.0 = WIDTH - corner.0;
        }
        corners[1..].reverse();
    }
    corners
}

/// Where the card starts across its window: past the arrow's room on the window's left, or at it
/// with the arrow on its right.
pub fn card_x(left: bool) -> f32 {
    if left { 0. } else { ARROW }
}

/// Where the window goes in a browser window `height` tall: its top left, and the arrow's tip
/// down it if it has one. Beside a row, the card centres on the arrow, as Arc's does, unless that
/// takes it past the browser window's top or bottom. `left` opens it to the left of `at`, toward
/// the page from a sidebar on the right.
pub fn placed(at: At, left: bool, height: f32) -> (f32, f32, Option<f32>) {
    placed_of(at, left, height, HEIGHT)
}

/// `placed` for a card of the picker's form `card` tall.
pub fn placed_of(at: At, left: bool, height: f32, card: f32) -> (f32, f32, Option<f32>) {
    let anchor = match at {
        // The window is as wide as the card and its arrow, so it starts that far left of the card.
        At::Edge(x) if left => return (x - EDGE_INSET - CARD_WIDTH, EDGE_INSET, None),
        At::Edge(x) => return (x + EDGE_INSET - ARROW, EDGE_INSET, None),
        At::Row(anchor) => anchor,
    };
    let top = (anchor.y - card / 2.)
        .min(height - MARGIN - card)
        .max(MARGIN);
    // The tip keeps clear of the card's rounded corners.
    let reach = RADIUS + ARROW + ARROW_FOOT_RADIUS;
    let arrow_y = (anchor.y - top).clamp(reach, card - reach);
    let x = if left {
        anchor.x - GAP + TIP_SHORT - WIDTH
    } else {
        anchor.x + GAP - TIP_SHORT
    };
    (x, top, Some(arrow_y))
}

/// Shows the picker for `space`, which has `theme` or none yet, `at` its place in browser window
/// `over`, `height` tall. `changed` gets the theme as the user makes it. A context menu in a
/// window behind another asks for it there.
pub fn show(
    over: u64,
    space: u64,
    theme: Option<Theme>,
    at: At,
    left: bool,
    height: f32,
    changed: Changed,
    cx: &mut App,
) {
    crate::lazy_window::ready::<ThemePicker>(cx);
    let Some(window) = cx.try_global::<ThemePickerWindow>().map(|picker| picker.0) else {
        return;
    };
    window
        .update(cx, |picker, window, cx| {
            // check.sh reads this line.
            log!("browser: theme picker for {space}");
            let (x, top, arrow_y) = placed(at, left, height);
            picker.editing = Some(Editing {
                space,
                theme,
                appearance: theme.map_or(Appearance::default(), |theme| theme.appearance),
                changed,
                arrow_y,
                left,
                open: true,
                drag: None,
                press: None,
                glide: None,
                pressed: None,
                keyed: None,
                drawn: false,
                drawn_handles: None,
                // The picker is a dark sheet over a dark or a light window, as the icon picker is.
                palette: Palette::of(theme.as_ref(), Tone::Dark),
            });
            window.focus(&picker.focus, cx);
            picker.host.show(over, x, top, &corners(arrow_y, left));
            cx.notify();
        })
        .ok();
}

/// The user left the picker, such as with a click outside it.
pub fn dismissed(cx: &mut App) {
    if let Some(window) = cx.try_global::<ThemePickerWindow>().map(|picker| picker.0) {
        window.update(cx, |picker, _, cx| picker.close(cx)).ok();
    }
}

/// The Space it edits is going from under it, as a swipe turns the page: it closes as Esc closes
/// it, where `dismissed` hears of a close the host has made.
pub fn leave(cx: &mut App) {
    if let Some(window) = cx.try_global::<ThemePickerWindow>().map(|picker| picker.0) {
        window
            .update(cx, |picker, window, cx| {
                picker.dismiss(&Dismiss, window, cx)
            })
            .ok();
    }
}

/// Whether the picker shows, for a click on what opened it to close it rather than open it again.
pub fn is_open(cx: &App) -> bool {
    open_for(cx).is_some()
}

/// The Space the picker shows for, if it shows.
pub fn open_for(cx: &App) -> Option<u64> {
    let picker = cx.try_global::<ThemePickerWindow>()?.0.read(cx).ok()?;
    picker
        .editing
        .as_ref()
        .filter(|editing| editing.open)
        .map(|editing| editing.space)
}

/// The picker, for the window it opens over to watch whether it still shows.
pub fn entity(cx: &App) -> Option<Entity<ThemePicker>> {
    cx.try_global::<ThemePickerWindow>()?.0.entity(cx).ok()
}

/// What a press at `x`, `y` from the card's top left lands on, with `theme` in the field.
pub fn target(x: f32, y: f32, theme: Option<&Theme>) -> Option<Target> {
    let near = |cx: f32, cy: f32, reach: f32| (x - cx).abs() <= reach && (y - cy).abs() <= reach;
    if let Some(index) = APPEARANCE_X
        .iter()
        .position(|&ax| near(ax, APPEARANCE_Y, 15.75))
    {
        return Some(Target::Appearance(Appearance::ALL[index]));
    }
    if near(REMOVE_X, COUNT_Y, COUNT_TARGET / 2.) {
        return Some(Target::Remove);
    }
    if near(ADD_X, COUNT_Y, COUNT_TARGET / 2.) {
        return Some(Target::Add);
    }
    let in_field = (FIELD_INSET..=FIELD_INSET + FIELD).contains(&x)
        && (FIELD_INSET..=FIELD_INSET + FIELD).contains(&y);
    if in_field {
        // The main handle lies over the others.
        if let Some(theme) = theme {
            let (fx, fy) = field_point(x, y);
            let mut order: Vec<usize> = (0..theme.count).collect();
            order.rotate_left(1);
            for index in order.into_iter().rev() {
                let (hx, hy) = theme.handle(index);
                let reach = handle_size(index) / 2. / (FIELD / 2.);
                if (fx - hx).hypot(fy - hy) <= reach {
                    return Some(Target::Handle(index));
                }
            }
        }
        return Some(Target::Field);
    }
    let swatch = ((x - SWATCH_X) / SWATCH_PITCH).round();
    if (0. ..SWATCHES.len() as f32).contains(&swatch)
        && (x - SWATCH_X - swatch * SWATCH_PITCH).hypot(y - SWATCH_Y) <= SWATCH / 2.
    {
        return Some(Target::Swatch(swatch as usize));
    }
    if (TRACK_LEFT..=TRACK_RIGHT).contains(&x) && (y - SLIDER_Y).abs() <= KNOB_HEIGHT / 2. {
        return Some(Target::Intensity);
    }
    if (x - DIAL.0).hypot(y - DIAL.1) <= DIAL_REACH {
        return Some(Target::Grain);
    }
    None
}

/// The field's point under `x`, `y` in the card, from its middle in half its width.
fn field_point(x: f32, y: f32) -> (f32, f32) {
    let middle = FIELD_INSET + FIELD / 2.;
    ((x - middle) / (FIELD / 2.), (y - middle) / (FIELD / 2.))
}

fn handle_size(index: usize) -> f32 {
    if index == 0 { MAIN_HANDLE } else { HANDLE }
}

/// The intensity knob's middle across the card.
fn knob_x(intensity: f32) -> f32 {
    KNOB_LEFT + (KNOB_RIGHT - KNOB_LEFT) * intensity
}

/// The intensity for the knob's middle at `x` in the card.
/// What an assistive app calls the swatch of `color`: its hue's name, or for one with little
/// colour, how light it is, a bluish grey being slate.
pub(crate) fn swatch_name(color: u32) -> &'static str {
    let [r, g, b] = [16, 8, 0].map(|shift| ((color >> shift) & 0xff) as f32 / 255.);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let chroma = max - min;
    let hue = match () {
        _ if chroma == 0. => 0.,
        _ if max == r => 60. * ((g - b) / chroma).rem_euclid(6.),
        _ if max == g => 60. * ((b - r) / chroma + 2.),
        _ => 60. * ((r - g) / chroma + 4.),
    };
    let lightness = (max + min) / 2.;
    if chroma < 0.14 {
        return match () {
            _ if lightness > 0.85 => "White",
            _ if lightness < 0.15 => "Black",
            _ if chroma > 0.05 && (200. ..260.).contains(&hue) => "Slate",
            _ => "Gray",
        };
    }
    match hue {
        hue if !(15. ..345.).contains(&hue) => "Red",
        hue if hue < 45. => "Orange",
        hue if hue < 70. => "Yellow",
        hue if hue < 170. => "Green",
        hue if hue < 200. => "Teal",
        hue if hue < 260. => "Blue",
        hue if hue < 320. => "Purple",
        _ => "Pink",
    }
}

/// What an assistive app reads of a slider called `name` at `value` of 0 to 1, in percent, a tenth
/// a step, which its increment and decrement move by, as `step` does.
fn slider(
    element: Stateful<Div>,
    name: &'static str,
    value: Option<f32>,
    step: impl Fn(f32, &mut App) + 'static,
) -> Stateful<Div> {
    let step = Rc::new(step);
    let down = step.clone();
    element
        .role(Role::Slider)
        .aria_label(name)
        .aria_min_numeric_value(0.)
        .aria_max_numeric_value(100.)
        .aria_numeric_value_step(10.)
        .when_some(value, |slider, value| {
            slider.aria_numeric_value((value * 100.).round() as f64)
        })
        .when(value.is_none(), |slider| {
            slider.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
        })
        .on_a11y_action(Action::Increment, move |_, _, cx| step(0.1, cx))
        .on_a11y_action(Action::Decrement, move |_, _, cx| down(-0.1, cx))
}

fn intensity_at(x: f32) -> f32 {
    ((x - KNOB_LEFT) / (KNOB_RIGHT - KNOB_LEFT)).clamp(0., 1.)
}

/// Whether what `drag` moves is at a stop it can't go past in `theme`: a handle on the field's
/// rim, the intensity knob at either end, or the dial at the top, from either side.
fn at_stop(drag: Drag, theme: &Theme) -> bool {
    match drag {
        Drag::Handle(..) => theme.x.hypot(theme.y) >= 1. - 1e-4,
        Drag::Intensity => theme.intensity <= 0. || theme.intensity >= 1.,
        Drag::Grain => theme.grain <= 0. || theme.grain >= 1.,
    }
}

/// The stops tab goes round, top to bottom, each a row that left and right go along: the
/// appearance buttons, − and +, the swatches and the wave. − and + and the wave wait for a colour,
/// as their clicks do.
fn key_stops(theme: Option<&Theme>) -> Vec<Vec<Target>> {
    let appearances = Appearance::ALL
        .iter()
        .map(|&appearance| Target::Appearance(appearance));
    let swatches = (0..SWATCHES.len()).map(Target::Swatch).collect();
    match theme {
        Some(_) => vec![
            appearances.collect(),
            vec![Target::Remove, Target::Add],
            swatches,
            vec![Target::Intensity],
        ],
        None => vec![appearances.collect(), swatches],
    }
}

/// Where tab lands in `stop`: the appearance chosen, − or + whichever answers, else the first.
fn key_entry(stop: &[Target], chosen: Appearance, theme: Option<&Theme>) -> Target {
    let answers = |target: &&Target| match (target, theme) {
        (Target::Appearance(appearance), _) => *appearance == chosen,
        (Target::Remove, Some(theme)) => theme.count > 1,
        (Target::Add, Some(theme)) => theme.count < MAX_COLORS,
        _ => true,
    };
    *stop.iter().find(answers).unwrap_or(&stop[0])
}

/// The grain for the pointer at `x`, `y` in the card: how far round the dial from its top,
/// clockwise. A turn past the top stops there rather than going round, from `before`.
fn grain_at(x: f32, y: f32, before: f32) -> f32 {
    let turn = (x - DIAL.0).atan2(DIAL.1 - y).rem_euclid(TAU) / TAU;
    match (before, turn) {
        (before, turn) if before > 0.75 && turn < 0.25 => 1.,
        (before, turn) if before < 0.25 && turn > 0.75 => 0.,
        _ => turn,
    }
}

impl ThemePicker {
    /// Where the card starts across the picker's window, as it opened.
    fn card_x(&self) -> f32 {
        card_x(self.editing.as_ref().is_some_and(|editing| editing.left))
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if let Some(editing) = self.editing.as_mut().filter(|editing| editing.open) {
            editing.open = false;
            editing.drag = None;
            cx.notify();
        }
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.editing.as_ref().is_some_and(|editing| editing.open) {
            // check.sh reads this line.
            log!("browser: theme picker dismissed");
            self.close(cx);
            self.host.hide();
        }
    }

    /// Hands the theme on: kept, once `done`.
    fn changed(&mut self, done: bool, cx: &mut Context<Self>) {
        let Some(editing) = &self.editing else {
            return;
        };
        if let Some(theme) = editing.theme {
            if done {
                // check.sh reads this line.
                log!(
                    "browser: theme picker set {} {}",
                    editing.space,
                    theme.to_text()
                );
            }
            (editing.changed.clone())(theme, done, cx);
        }
        cx.notify();
    }

    /// Moves the intensity or the grain `by`, as VoiceOver's increment and decrement do, and keeps
    /// it.
    fn step(&mut self, drag: Drag, by: f32, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut().filter(|editing| editing.open) else {
            return;
        };
        let Some(theme) = editing.theme else {
            return;
        };
        // To the nearest tenth, so the steps read as whole tens of percent.
        let stepped = |value: f32| (((value + by) * 10.).round() / 10.).clamp(0., 1.);
        let moved = match drag {
            Drag::Intensity => Theme {
                intensity: stepped(theme.intensity),
                ..theme
            },
            Drag::Grain => Theme {
                grain: stepped(theme.grain),
                ..theme
            },
            Drag::Handle(..) => return,
        };
        if moved != theme {
            editing.theme = Some(moved);
            self.changed(true, cx);
        }
    }

    fn press(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let (x, y) = (
            f32::from(event.position.x) - self.card_x(),
            f32::from(event.position.y),
        );
        let Some(editing) = self.editing.as_mut().filter(|editing| editing.open) else {
            return;
        };
        // The ring is the keyboard's, and goes as the mouse takes over.
        editing.keyed = None;
        let Some(target) = target(x, y, editing.theme.as_ref()) else {
            cx.notify();
            return;
        };
        let now = crate::motion::start(cx);
        match (target, editing.theme) {
            (Target::Appearance(_) | Target::Remove | Target::Add | Target::Swatch(_), _) => {
                let counted = matches!(target, Target::Remove | Target::Add);
                if self.pick(target, cx)
                    && counted
                    && let Some(editing) = &mut self.editing
                {
                    editing.pressed = Some(target);
                }
            }
            (Target::Intensity, Some(theme)) => {
                editing.theme = Some(Theme {
                    intensity: intensity_at(x),
                    ..theme
                });
                editing.drag = Some(Drag::Intensity);
                self.changed(false, cx);
            }
            (Target::Grain, Some(theme)) => {
                editing.theme = Some(Theme {
                    grain: grain_at(x, y, theme.grain),
                    ..theme
                });
                editing.drag = Some(Drag::Grain);
                self.changed(false, cx);
            }
            (Target::Handle(index), Some(theme)) => {
                let (fx, fy) = field_point(x, y);
                let (hx, hy) = theme.handle(index);
                editing.drag = Some(Drag::Handle(index, fx - hx, fy - hy));
                editing.press = Some((index, now, true));
                cx.notify();
            }
            // The first colour goes where the field is clicked, and follows the pointer from there.
            (Target::Field, None) => {
                let (fx, fy) = field_point(x, y);
                let appearance = editing.appearance;
                editing.theme = Some(Theme {
                    appearance,
                    ..Theme::at(fx, fy)
                });
                editing.drag = Some(Drag::Handle(0, 0., 0.));
                editing.press = Some((0, now, true));
                self.changed(false, cx);
            }
            _ => {}
        }
        cx.notify();
    }

    /// Presses an appearance button, − or + or a swatch, by click or key, and says whether it
    /// answered: − and + don't at their ends or without a colour.
    fn pick(&mut self, target: Target, cx: &mut Context<Self>) -> bool {
        let Some(editing) = self.editing.as_mut().filter(|editing| editing.open) else {
            return false;
        };
        let now = Instant::now();
        match (target, editing.theme) {
            (Target::Appearance(appearance), theme) => {
                editing.appearance = appearance;
                editing.theme = theme.map(|theme| Theme {
                    appearance,
                    ..theme
                });
            }
            (Target::Remove, Some(theme)) if theme.count > 1 => {
                editing.glide = glide(&theme, theme.count - 1, now);
                editing.theme = Some(theme.with_count(theme.count - 1));
            }
            (Target::Add, Some(theme)) if theme.count < MAX_COLORS => {
                editing.glide = glide(&theme, theme.count + 1, now);
                editing.theme = Some(theme.with_count(theme.count + 1));
            }
            (Target::Swatch(index), theme) => {
                let picked = Theme::of_color(SWATCHES[index]);
                editing.theme = Some(match theme {
                    Some(theme) => {
                        let (x, y, saturation) = (picked.x, picked.y, picked.saturation);
                        Theme {
                            x,
                            y,
                            saturation,
                            third: None,
                            ..theme
                        }
                    }
                    None => Theme {
                        appearance: editing.appearance,
                        ..picked
                    },
                });
            }
            _ => return false,
        }
        self.changed(true, cx);
        true
    }

    /// Tab, or shift-tab back: the ring goes to the next stop round, or the first.
    fn key_stop(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut().filter(|editing| editing.open) else {
            return;
        };
        let stops = key_stops(editing.theme.as_ref());
        let at = editing
            .keyed
            .and_then(|keyed| stops.iter().position(|stop| stop.contains(&keyed)));
        let next = match (at, forward) {
            (None, true) => 0,
            (None, false) => stops.len() - 1,
            (Some(at), true) => (at + 1) % stops.len(),
            (Some(at), false) => (at + stops.len() - 1) % stops.len(),
        };
        let chosen = editing
            .theme
            .map_or(editing.appearance, |theme| theme.appearance);
        let keyed = key_entry(&stops[next], chosen, editing.theme.as_ref());
        editing.keyed = Some(keyed);
        cx.notify();
    }

    /// Left or right: along the row the ring is in, stopping at its ends, or the wave's knob a
    /// step down or up, kept as each press would be.
    fn key_along(&mut self, right: bool, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut().filter(|editing| editing.open) else {
            return;
        };
        let Some(keyed) = editing.keyed else {
            return;
        };
        if keyed == Target::Intensity {
            let Some(theme) = editing.theme else {
                return;
            };
            let step = if right { KEY_STEP } else { -KEY_STEP };
            // Onto the steps, so a knob a drag left between them lands on one.
            let intensity = ((theme.intensity + step) / KEY_STEP).round() * KEY_STEP;
            let intensity = intensity.clamp(0., 1.);
            if intensity != theme.intensity {
                editing.theme = Some(Theme { intensity, ..theme });
                self.changed(true, cx);
            }
            return;
        }
        let stops = key_stops(editing.theme.as_ref());
        let Some(stop) = stops.iter().find(|stop| stop.contains(&keyed)) else {
            return;
        };
        let at = stop.iter().position(|target| *target == keyed).unwrap_or(0);
        let next = if right {
            (at + 1).min(stop.len() - 1)
        } else {
            at.saturating_sub(1)
        };
        editing.keyed = Some(stop[next]);
        cx.notify();
    }

    /// Space or return: presses what's ringed.
    fn key_press(&mut self, cx: &mut Context<Self>) {
        let open = self.editing.as_ref().filter(|editing| editing.open);
        if let Some(keyed) = open.and_then(|editing| editing.keyed) {
            self.pick(keyed, cx);
        }
    }

    fn drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let (x, y) = (f32::from(position.x) - self.card_x(), f32::from(position.y));
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        let (Some(drag), Some(theme)) = (editing.drag, editing.theme) else {
            return;
        };
        let moved = match drag {
            Drag::Handle(index, grab_x, grab_y) => {
                let (fx, fy) = field_point(x, y);
                theme.moved(index, fx - grab_x, fy - grab_y)
            }
            Drag::Intensity => Theme {
                intensity: intensity_at(x),
                ..theme
            },
            Drag::Grain => Theme {
                grain: grain_at(x, y, theme.grain),
                ..theme
            },
        };
        if moved != theme {
            // A bump as it gets to a stop, not while it's held there (the owner's call).
            if !at_stop(drag, &theme) && at_stop(drag, &moved) {
                self.host.detent();
            }
            editing.theme = Some(moved);
            self.changed(false, cx);
        }
    }

    fn release(&mut self, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        let pressed = editing.pressed.take().is_some();
        let Some(drag) = editing.drag.take() else {
            if pressed {
                cx.notify();
            }
            return;
        };
        if let Drag::Handle(index, ..) = drag {
            editing.press = Some((index, crate::motion::start(cx), false));
        }
        self.changed(true, cx);
    }

    /// The Space it's making a theme for, and the theme as it is, while it shows.
    pub fn editing(&self) -> Option<(u64, Option<Theme>)> {
        let editing = self.editing.as_ref().filter(|editing| editing.open)?;
        Some((editing.space, editing.theme))
    }
}

/// The second handle's glide as `theme` goes to `count` colours, if its place changes.
fn glide(theme: &Theme, count: usize, now: Instant) -> Option<(Place, Place, Instant)> {
    let (from, to) = (theme.place(1), theme.with_count(count).place(1));
    (theme.count > 1 && count > 1 && from != to).then_some((from, to, now))
}

impl Editing {
    /// Where handle `index` shows in the field, as the second glides.
    fn handle(&self, index: usize, theme: &Theme) -> (f32, f32) {
        match self.glide {
            Some((from, to, start)) if index == 1 => {
                let t = (start.elapsed().as_secs_f32() / GLIDE).min(1.);
                let t = cubic_bezier(0.25, 0.1, 0.25, 1., t);
                // The short way round.
                let turn = (to.turn - from.turn + 540.).rem_euclid(360.) - 180.;
                let place = Place {
                    turn: from.turn + turn * t,
                    lift: from.lift + (to.lift - from.lift) * t,
                };
                theme::placed(theme.x, theme.y, place)
            }
            _ => theme.handle(index),
        }
    }

    /// How much bigger than at rest handle `index` is, as it's pressed or let go.
    fn growth(&self, index: usize) -> f32 {
        match self.press {
            Some((pressed, start, down)) if pressed == index => {
                let t = (start.elapsed().as_secs_f32() / PRESS).min(1.);
                let t = if down { t } else { 1. - t };
                1. + (PRESSED - 1.) * t
            }
            _ => 1.,
        }
    }

    /// Logs the colour handles as they're drawn at rest, and what's held. GPUI draws no frame
    /// between a press and the next mouse event, and only a frame drawn while a handle or a knob is
    /// held listens for the drag's moves, so check.sh waits for these lines before it presses and
    /// before it drags.
    fn log_handles(&mut self) {
        let gliding = self
            .glide
            .is_some_and(|(.., start)| start.elapsed().as_secs_f32() < GLIDE);
        let held = self.drag.map(|drag| match drag {
            Drag::Handle(index, ..) => Drag::Handle(index, 0., 0.),
            knob => knob,
        });
        let handles = self.theme.filter(|_| self.open && self.drawn && !gliding);
        let handles = handles.map(|theme| (theme.count, held));
        if handles.is_none() || handles == self.drawn_handles {
            return;
        }
        self.drawn_handles = handles;
        if let Some((count, held)) = handles {
            match held {
                Some(Drag::Handle(index, ..)) => {
                    log!("browser: theme picker handles {count} drawn, {index} held")
                }
                Some(Drag::Intensity) => {
                    log!("browser: theme picker handles {count} drawn, intensity held")
                }
                Some(Drag::Grain) => {
                    log!("browser: theme picker handles {count} drawn, grain held")
                }
                None => log!("browser: theme picker handles {count} drawn"),
            }
        }
    }

    /// Whether a handle's growth or the second's glide is still under way.
    fn moving(&self) -> bool {
        let press = self
            .press
            .is_some_and(|(.., start, _)| start.elapsed().as_secs_f32() < PRESS);
        let glide = self
            .glide
            .is_some_and(|(.., start)| start.elapsed().as_secs_f32() < GLIDE);
        press || glide
    }
}

thread_local! {
    static DOTS: OnceCell<Arc<RenderImage>> = const { OnceCell::new() };
}

/// − or +'s glyph: while it answers, raised to the overlay's glyph floor as the appearance glyphs
/// are, and brighter pressed; dim, as it is, when it doesn't.
fn count_color(palette: &Palette, enabled: bool, pressed: bool) -> Hsla {
    match (enabled, pressed) {
        (false, _) => palette.faint(COUNT_DISABLED),
        (true, true) => palette.on_overlay_glyph(COUNT_PRESSED),
        (true, false) => palette.on_overlay_glyph(COUNT_COLOR),
    }
}

/// Whether the field's point `x`, `y` is under − or +'s target.
fn under_count(x: f32, y: f32) -> bool {
    let y = y - (COUNT_Y - FIELD_INSET);
    [REMOVE_X, ADD_X]
        .iter()
        .any(|&at| (x - (at - FIELD_INSET)).hypot(y) <= COUNT_TARGET / 2.)
}

/// The field's dot grid, drawn once at 2×, with none under − and + so they sit on the plain field.
fn dots_image() -> Arc<RenderImage> {
    DOTS.with(|dots| {
        dots.get_or_init(|| {
            let side = (FIELD * 2.) as usize;
            let mut data = vec![0u8; side * side * 4];
            let middle = FIELD / 2.;
            let count = (middle / DOT_PITCH) as i32;
            for row in -count..=count {
                for column in -count..=count {
                    let x = middle + column as f32 * DOT_PITCH;
                    let y = middle + row as f32 * DOT_PITCH;
                    if under_count(x, y) {
                        continue;
                    }
                    let x = (x * 2.).round() as usize - 1;
                    let y = (y * 2.).round() as usize - 1;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let at = ((y + dy) * side + x + dx) * 4;
                        data[at..at + 4].copy_from_slice(&[
                            255,
                            255,
                            255,
                            (DOT_ALPHA * 255.).round() as u8,
                        ]);
                    }
                }
            }
            let image = RgbaImage::from_raw(side as u32, side as u32, data).expect("sized to fit");
            Arc::new(RenderImage::new(vec![Frame::new(image)]))
        })
        .clone()
    })
}

/// A white capsule `width` by `length` with its middle at `at`, its length along `angle` (radians
/// clockwise from up), into a path to fill.
fn capsule(at: Point<Pixels>, width: f32, length: f32, angle: f32) -> PathBuilder {
    let (sin, cos) = angle.sin_cos();
    let along = (sin, -cos);
    let radius = width / 2.;
    let reach = length / 2. - radius;
    let mut path = PathBuilder::fill();
    // Each end's half circle in eight steps, from one side round to the other.
    let ends = [(reach, 0.), (-reach, std::f32::consts::PI)];
    let mut first = true;
    for (offset, start) in ends {
        let (cx, cy) = (along.0 * offset, along.1 * offset);
        for step in 0..=8 {
            let theta = angle + start - std::f32::consts::FRAC_PI_2
                + std::f32::consts::PI * step as f32 / 8.;
            let (s, c) = theta.sin_cos();
            let point = at + point(px(cx + radius * s), px(cy - radius * c));
            if first {
                path.move_to(point);
                first = false;
            } else {
                path.line_to(point);
            }
        }
    }
    path.close();
    path
}

impl Focusable for ThemePicker {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for ThemePicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div()
            .key_context(KEY_CONTEXT)
            .size_full()
            .font_family(crate::face::UI);
        // The pointer's moves anywhere in the window and its release, in every frame: a press and
        // its first move or its release can both come before the next frame, which would
        // otherwise be drawn without a listener for them (#2605). A move with nothing dragged
        // does nothing.
        let entity = cx.weak_entity();
        let listener = canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                let moved = entity.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture {
                        moved
                            .update(cx, |picker, cx| picker.drag(event.position, cx))
                            .ok();
                    }
                });
                let released = entity.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                        released.update(cx, |picker, cx| picker.release(cx)).ok();
                    }
                });
            },
        )
        .absolute()
        .size_0();
        // GPUI hands a press to what the last frame drew, which before an opening's first frame is
        // the empty one drawn as the window was made. So every frame takes the keys, the press and
        // the release, the press read against the picker as it is now, and a click as the picker
        // appears isn't lost.
        let a11y = window.is_a11y_active();
        let picker = div()
            .id("theme-picker")
            .when(a11y, |picker| {
                picker.role(Role::Dialog).aria_label("Space Theme")
            })
            .key_context("ThemePicker")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::dismiss))
            .on_action(cx.listener(|picker, _: &NextStop, _, cx| picker.key_stop(true, cx)))
            .on_action(cx.listener(|picker, _: &PreviousStop, _, cx| picker.key_stop(false, cx)))
            .on_action(cx.listener(|picker, _: &KeyLeft, _, cx| picker.key_along(false, cx)))
            .on_action(cx.listener(|picker, _: &KeyRight, _, cx| picker.key_along(true, cx)))
            .on_action(cx.listener(|picker, _: &KeyPress, _, cx| picker.key_press(cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|picker, event: &MouseDownEvent, _, cx| picker.press(event, cx)),
            )
            .relative()
            .size_full()
            .child(listener);
        let Some(editing) = &mut self.editing else {
            return root.child(picker);
        };
        if editing.moving() {
            window.request_animation_frame();
        }
        // AppKit puts the window on screen some frames after it's asked to, and only then does
        // GPUI draw it.
        if editing.open && !editing.drawn {
            if window.is_visible() {
                editing.drawn = true;
                // check.sh waits for this line.
                log!("browser: theme picker drawn");
            } else {
                window.request_animation_frame();
            }
        }
        editing.log_handles();
        let editing = &*editing;
        let theme = editing.theme;
        let palette = editing.palette;
        let keyed = editing.keyed;
        let card_left = card_x(editing.left);
        let card_x = |x: f32| px(card_left + x);
        // The keyboard's ring, as Focus Sidebar's: inside a button's plate, or `RING_GAP` round a
        // swatch or the wave's knob, `width` by `height` about `x`, `y` in the card.
        let ring = |x: f32, y: f32, width: f32, height: f32, radius: f32| {
            div()
                .absolute()
                .left(card_x(x - width / 2.))
                .top(px(y - height / 2.))
                .w(px(width))
                .h(px(height))
                .rounded(px(radius))
                .border(px(KEYED_RING))
                .border_color(palette.plate(KEYED))
        };
        let round_ring = |x: f32, y: f32, width: f32, height: f32| {
            let reach = 2. * (RING_GAP + KEYED_RING);
            let (width, height) = (width + reach, height + reach);
            ring(x, y, width, height, width.min(height) / 2.)
        };
        let keyed_ring = keyed.map(|keyed| match keyed {
            Target::Appearance(appearance) => {
                let index = Appearance::ALL
                    .iter()
                    .position(|&each| each == appearance)
                    .unwrap_or(0);
                let plate = APPEARANCE_PLATE;
                ring(
                    APPEARANCE_X[index],
                    APPEARANCE_Y,
                    plate,
                    plate,
                    APPEARANCE_RADIUS,
                )
            }
            Target::Remove | Target::Add => {
                let x = if keyed == Target::Remove {
                    REMOVE_X
                } else {
                    ADD_X
                };
                ring(x, COUNT_Y, COUNT_TARGET, COUNT_TARGET, APPEARANCE_RADIUS)
            }
            Target::Swatch(index) => round_ring(
                SWATCH_X + SWATCH_PITCH * index as f32,
                SWATCH_Y,
                SWATCH,
                SWATCH,
            ),
            _ => {
                let intensity = theme.map_or(DEFAULT_INTENSITY, |theme| theme.intensity);
                round_ring(knob_x(intensity), SLIDER_Y, KNOB_WIDTH, KNOB_HEIGHT)
            }
        });
        let (arrow_y, left) = (editing.arrow_y, editing.left);
        let card_fill = palette::current(cx).card_drawn();
        let card = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                icon_picker::paint_card(bounds.origin, &corners(arrow_y, left), card_fill, window)
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        // The field, its dots, and the handles over it.
        let dots = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let dots = dots_image();
                window
                    .paint_image(bounds, bounds, px(FIELD_RADIUS).into(), dots, 0, false)
                    .ok();
            },
        )
        .absolute()
        .size_full();
        let colours = match theme.map_or(0, |theme| theme.count) {
            0 => "Colors, none".to_string(),
            1 => "Colors, 1".to_string(),
            count => format!("Colors, {count}"),
        };
        let mut field = div()
            .id("theme-field")
            // Its press on the empty field places the first colour in its middle, as a click does.
            .when(a11y, |field| {
                field
                    .role(Role::Group)
                    .aria_label(colours)
                    .when(theme.is_none(), |field| {
                        field
                            .aria_description(HINT)
                            .a11y_synthetic_children(|builder| {
                                builder.parent_node().add_action(Action::Click)
                            })
                    })
            })
            .absolute()
            .left(card_x(FIELD_INSET))
            .top(px(FIELD_INSET))
            .size(px(FIELD))
            .rounded(px(FIELD_RADIUS))
            .bg(palette.plate(FIELD_FILL))
            .child(dots)
            .child(
                div()
                    .absolute()
                    .size_full()
                    .rounded(px(FIELD_RADIUS))
                    .border_1()
                    .border_color(palette.plate(FIELD_RIM)),
            );
        match theme {
            None => {
                field = field.child(
                    div()
                        .absolute()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(text::BODY))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(palette.on_overlay(1.))
                        .child(HINT),
                );
            }
            Some(theme) => {
                // The main handle last, over the others.
                for index in (1..theme.count).chain([0]) {
                    let (hx, hy) = editing.handle(index, &theme);
                    let color = (theme.third.filter(|_| index == 2))
                        .unwrap_or_else(|| theme::color_at(hx, hy, theme.saturation));
                    let size = handle_size(index) * crate::motion::scale(editing.growth(index), cx);
                    let ring = if index == 0 { MAIN_RING } else { HANDLE_RING };
                    let (mx, my) = (FIELD / 2. * (1. + hx), FIELD / 2. * (1. + hy));
                    field = field.child(
                        div()
                            .absolute()
                            .left(px(mx - size / 2.))
                            .top(px(my - size / 2.))
                            .size(px(size))
                            .rounded_full()
                            .bg(rgb(color))
                            .border(px(ring))
                            .border_color(palette.ink),
                    );
                }
            }
        }

        let chosen = theme.map_or(editing.appearance, |theme| theme.appearance);
        let appearances = Appearance::ALL
            .iter()
            .zip(APPEARANCE_X)
            .map(|(&appearance, x)| {
                let glyph = match appearance {
                    Appearance::Automatic => "icons/sparkles.svg",
                    Appearance::Light => "icons/sun.svg",
                    Appearance::Dark => "icons/moon-star.svg",
                };
                let name = match appearance {
                    Appearance::Automatic => "Automatic",
                    Appearance::Light => "Light",
                    Appearance::Dark => "Dark",
                };
                let toggled = if appearance == chosen {
                    Toggled::True
                } else {
                    Toggled::False
                };
                div()
                    .id(name)
                    // The picker answers presses where they land, so its press is a click there.
                    .when(a11y, |plate| {
                        plate
                            .role(Role::RadioButton)
                            .aria_label(name)
                            .aria_toggled(toggled)
                            .a11y_synthetic_children(|builder| {
                                builder.parent_node().add_action(Action::Click)
                            })
                    })
                    .absolute()
                    .left(card_x(x - APPEARANCE_PLATE / 2.))
                    .top(px(APPEARANCE_Y - APPEARANCE_PLATE / 2.))
                    .size(px(APPEARANCE_PLATE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(APPEARANCE_RADIUS))
                    .when(appearance == chosen, |plate| {
                        plate.bg(palette.plate(APPEARANCE_CHOSEN))
                    })
                    .child(
                        svg()
                            .path(glyph)
                            .size(px(APPEARANCE_GLYPH))
                            .text_color(palette.on_overlay_glyph(APPEARANCE_COLOR)),
                    )
            });

        let count = theme.map_or(0, |theme| theme.count);
        let count_button = |target: Target, x: f32, glyph: &'static str, enabled: bool| {
            let color = count_color(&palette, enabled, editing.pressed == Some(target));
            let name = if target == Target::Add {
                "Add Color"
            } else {
                "Remove Color"
            };
            div()
                .id(name)
                .when(a11y, |button| {
                    button
                        .role(Role::Button)
                        .aria_label(name)
                        .a11y_synthetic_children(move |builder| {
                            let node = builder.parent_node();
                            node.add_action(Action::Click);
                            if !enabled {
                                node.set_disabled();
                            }
                        })
                })
                .absolute()
                .left(card_x(x - COUNT_TARGET / 2.))
                .top(px(COUNT_Y - COUNT_TARGET / 2.))
                .size(px(COUNT_TARGET))
                .flex()
                .items_center()
                .justify_center()
                .child(svg().path(glyph).size(px(COUNT_GLYPH)).text_color(color))
        };
        let remove = count_button(Target::Remove, REMOVE_X, "icons/minus.svg", count > 1);
        let add = count_button(
            Target::Add,
            ADD_X,
            "icons/plus.svg",
            (1..MAX_COLORS).contains(&count),
        );

        let behind = palette.overlay;
        let swatches = self.swatches.iter().enumerate().map(|(index, swatch)| {
            div()
                .id(("swatch", index))
                .when(a11y, |plate| {
                    plate
                        .role(Role::Button)
                        .aria_label(swatch_name(SWATCHES[index]))
                        .a11y_synthetic_children(|builder| {
                            builder.parent_node().add_action(Action::Click)
                        })
                })
                .absolute()
                .left(card_x(SWATCH_X + SWATCH_PITCH * index as f32 - SWATCH / 2.))
                .top(px(SWATCH_Y - SWATCH / 2.))
                .child(swatch.draw(SWATCH, &[behind]))
        });

        // The wave slider and the dial, which show without knobs, dimmer, until there's a colour.
        let controls = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let at = |x: f32, y: f32| bounds.origin + point(px(card_left + x), px(y));
                let track = Bounds::new(
                    at(TRACK_LEFT, SLIDER_Y - TRACK_HEIGHT / 2.),
                    size(px(TRACK_RIGHT - TRACK_LEFT), px(TRACK_HEIGHT)),
                );
                let track =
                    fill(track, palette.plate(TRACK_FILL)).corner_radii(px(TRACK_HEIGHT / 2.));
                window.paint_quad(track);
                let intensity = theme.map_or(DEFAULT_INTENSITY, |theme| theme.intensity);
                let split = if theme.is_some() {
                    knob_x(intensity)
                } else {
                    WAVE_LEFT
                };
                let amplitude = WAVE_AMPLITUDE * intensity;
                let wave_y = |x: f32| {
                    let phase = (x - WAVE_LEFT) / (WAVE_RIGHT - WAVE_LEFT) * WAVE_PERIODS * TAU;
                    SLIDER_Y - amplitude * phase.sin()
                };
                for (from, to, color) in [
                    (WAVE_LEFT, split, palette.plate(WAVE_ON)),
                    (split, WAVE_RIGHT, palette.plate(WAVE_OFF)),
                ] {
                    if to - from < 0.5 {
                        continue;
                    }
                    let mut wave = PathBuilder::stroke(px(WAVE_STROKE));
                    let steps = ((to - from) / 1.5).ceil() as usize;
                    for step in 0..=steps {
                        let x = from + (to - from) * step as f32 / steps as f32;
                        if step == 0 {
                            wave.move_to(at(x, wave_y(x)));
                        } else {
                            wave.line_to(at(x, wave_y(x)));
                        }
                    }
                    if let Ok(path) = wave.build() {
                        window.paint_path(path, color);
                    }
                }
                let grain = theme.map(|theme| theme.grain);
                let disc = Bounds::new(
                    at(DIAL.0 - DIAL_DISC, DIAL.1 - DIAL_DISC),
                    size(px(DIAL_DISC * 2.), px(DIAL_DISC * 2.)),
                );
                window.paint_quad(quad(
                    disc,
                    px(DIAL_DISC),
                    rgba(if grain.is_some() { DIAL_DISC_FILL } else { 0 }),
                    px(1.),
                    palette.plate(DIAL_DISC_RIM),
                    Default::default(),
                ));
                for dot in 0..DIAL_DOTS {
                    let turn = dot as f32 / DIAL_DOTS as f32;
                    let (sin, cos) = (turn * TAU).sin_cos();
                    let (x, y) = (DIAL.0 + DIAL_RING * sin, DIAL.1 - DIAL_RING * cos);
                    let swept = grain.is_some_and(|grain| turn <= grain + 1e-4);
                    let bounds = Bounds::new(
                        at(x - DIAL_DOT / 2., y - DIAL_DOT / 2.),
                        size(px(DIAL_DOT), px(DIAL_DOT)),
                    );
                    let color = palette.plate(if swept { DIAL_SWEPT } else { DIAL_DIM });
                    window.paint_quad(fill(bounds, color).corner_radii(px(DIAL_DOT / 2.)));
                }
                if let Some(grain) = grain {
                    let (sin, cos) = (grain * TAU).sin_cos();
                    let middle = at(DIAL.0 + DIAL_RING * sin, DIAL.1 - DIAL_RING * cos);
                    let knob = capsule(middle, DIAL_KNOB.0, DIAL_KNOB.1, grain * TAU);
                    if let Ok(path) = knob.build() {
                        window.paint_path(path, palette.ink);
                    }
                    let knob = Bounds::new(
                        at(
                            knob_x(intensity) - KNOB_WIDTH / 2.,
                            SLIDER_Y - KNOB_HEIGHT / 2.,
                        ),
                        size(px(KNOB_WIDTH), px(KNOB_HEIGHT)),
                    );
                    window.paint_quad(fill(knob, palette.ink).corner_radii(px(KNOB_WIDTH / 2.)));
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        // What an assistive app reads of the wave and the dial, which are drawn, as sliders where
        // they lie.
        let sliders = a11y.then(|| {
            let entity = cx.weak_entity();
            let stepper = move |drag: Drag| {
                let entity = entity.clone();
                move |by: f32, cx: &mut App| {
                    entity
                        .update(cx, |picker, cx| picker.step(drag, by, cx))
                        .ok();
                }
            };
            let intensity = div()
                .id("theme-intensity")
                .absolute()
                .left(card_x(TRACK_LEFT))
                .top(px(SLIDER_Y - KNOB_HEIGHT / 2.))
                .w(px(TRACK_RIGHT - TRACK_LEFT))
                .h(px(KNOB_HEIGHT));
            let value = theme.map(|theme| theme.intensity);
            let intensity = slider(intensity, "Intensity", value, stepper(Drag::Intensity));
            let grain = div()
                .id("theme-grain")
                .absolute()
                .left(card_x(DIAL.0 - DIAL_DISC))
                .top(px(DIAL.1 - DIAL_DISC))
                .size(px(DIAL_DISC * 2.));
            let value = theme.map(|theme| theme.grain);
            [
                intensity,
                slider(grain, "Grain", value, stepper(Drag::Grain)),
            ]
        });
        // The grain in the dial's disc, as it lies over the window.
        let dial_grain = theme.filter(|theme| theme.grain > 0.).map(|theme| {
            div()
                .absolute()
                .left(card_x(DIAL.0 - DIAL_DISC))
                .top(px(DIAL.1 - DIAL_DISC))
                .size(px(DIAL_DISC * 2.))
                .opacity(theme.grain_opacity())
                .child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            window
                                .paint_image(
                                    bounds,
                                    bounds,
                                    px(DIAL_DISC).into(),
                                    theme.texture.image(),
                                    0,
                                    false,
                                )
                                .ok();
                        },
                    )
                    .size_full(),
                )
        });

        root.child(
            picker
                .child(card)
                .child(field)
                .children(appearances)
                .child(remove)
                .child(add)
                .children(swatches)
                .child(controls)
                .children(dial_grain)
                .children(sliders.into_iter().flatten())
                .children(keyed_ring),
        )
    }
}

#[cfg(test)]
mod tests;
