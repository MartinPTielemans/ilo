//! The panel that sets a Space's look (P6, the drawing's screen 14): it hangs beside the Space's
//! name, 8 right of the sidebar, with the palettes, the pad where the colours' handles lie, the
//! Points chip and the value field, and the bars for the intensity and the glass. Every change
//! shows in the window as it is made and is kept as the pointer lets go; there's no Save. Like the
//! icon picker, it's a window of its own that the host shows over the browser window, as its
//! child; what it asks of that window goes through `Host`.
//!
//! The keyboard (ours: the drawing has none): tab and shift-tab go round the appearance, the
//! palettes, the Points
//! chip, the value field, each bar and Reset, ringed as Focus Sidebar rings a row; left and right
//! go along the appearance, the palettes or the chip, or move a bar by 1, with shift by 10; space and return press
//! what's ringed. The handles stay the mouse's: a palette or a typed value sets a colour.
//!
//! The Boost editor keeps the form today's picker had, its arrow and its card (`corners_of`,
//! `placed_of`, `card_x`, `FORM_WIDTH`).

mod layout;

use std::{cell::OnceCell, rc::Rc, sync::Arc};

use gpui::{
    App, Bounds, Context, DispatchPhase, Div, Entity, FocusHandle, Focusable, FontWeight, Global,
    Hsla, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder,
    Pixels, Point, RenderImage, SharedString, Stateful, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions,
    accesskit::{Action, Role, Toggled},
    actions, canvas, div, fill, linear_color_stop, linear_gradient, point,
    prelude::*,
    px, rgb, rgba, size,
};
use image::{Frame, RgbaImage};

use crate::dismissal::{By, Close, Why};
use crate::text_input::{Colors, TextInput};
use crate::{
    icon_picker,
    palette::{self, Palette, Tone},
    shell::KEY_CONTEXT,
    theme::{self, Appearance, MAX_COLORS, Theme},
};

pub use layout::Bar;
use layout::{BARS, INNER};

actions!(
    theme_picker,
    [
        Dismiss,
        NextStop,
        PreviousStop,
        KeyLeft,
        KeyRight,
        KeyLeftFar,
        KeyRightFar,
        KeyPress
    ]
);

use crate::tokens::shell::{KEYED, KEYED_RING};
use crate::tokens::theme_panel::*;
pub use crate::tokens::theme_picker::SWATCHES;
use crate::tokens::theme_picker::{
    ARROW, ARROW_FOOT_RADIUS, ARROW_TIP_RADIUS, CARD_WIDTH, EDGE_INSET, GAP as FORM_GAP,
    MARGIN as FORM_MARGIN, RADIUS as FORM_RADIUS,
};

/// The panel's window: the panel, with no arrow.
pub const WIDTH: f32 = crate::tokens::theme_panel::WIDTH;
pub const HEIGHT: f32 = layout::height();
/// The form today's picker had, the Boost editor's: its arrow's room and its card.
pub const FORM_WIDTH: f32 = ARROW + CARD_WIDTH;
const TIP_SHORT: f32 = ARROW_TIP_RADIUS * (std::f32::consts::SQRT_2 - 1.);
/// What an assistive app hears of the empty pad.
const HINT: &str = "Tap to pick a color for this space";
/// The keyboard's ring keeps this far out from a disc or the thumb it rings.
const RING_GAP: f32 = 1.;
/// A bar's step by key, a point on its 0 to 100, and with shift ten (the spec's).
const KEY_STEP: f32 = 0.01;
const KEY_FAR: f32 = 0.1;

/// What the panel asks of its window.
pub trait Host {
    /// Shows the panel's window with its top left at `left`, `top` in browser window `window`,
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

/// What else the panel sets for its Space beside its theme: its glass on the Transparency bar as
/// the user moves it, done as `Changed` is; and its colours taken away, by the first palette or
/// Reset.
#[derive(Clone)]
pub struct Also {
    /// Where the Space's glass sits on the bar as the panel opens.
    pub transparency: f32,
    pub glassed: Rc<dyn Fn(f32, bool, &mut App)>,
    pub cleared: Rc<dyn Fn(&mut App)>,
}

/// What a press in the panel lands on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Appearance(Appearance),
    /// A palette's disc, the first being no colour.
    Palette(usize),
    /// A colour's handle, the main one 0.
    Handle(usize),
    Pad,
    /// A colour's disc in the Points chip.
    Point(usize),
    /// The chip's plus.
    Add,
    /// The value field.
    Value,
    Bar(Bar),
    Reset,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    /// A handle, and where in it the pointer took it, from its middle, on the panel.
    Handle(usize, f32, f32),
    Bar(Bar),
}

struct Editing {
    space: u64,
    theme: Option<Theme>,
    /// The Space's glass on its Transparency bar, as the panel makes it.
    glass: f32,
    /// The appearance chosen before there's a colour, for the first one to take.
    appearance: Appearance,
    changed: Changed,
    also: Option<Also>,
    /// Whether it opened to the left of what opened it, from a sidebar on the right.
    left: bool,
    /// False once it's closing: what it shows stays, for the host's shrink out.
    open: bool,
    drag: Option<Drag>,
    /// The colour the chip has chosen, whose value the field shows.
    chosen: usize,
    /// The value field while a value is typed in it.
    typing: Option<Entity<TextInput>>,
    /// What the keyboard is on, ringed, once tab has put it somewhere.
    keyed: Option<Target>,
    /// Whether a frame of it has been drawn on screen since it opened.
    drawn: bool,
    /// How many colour handles the last frame drew at rest, and what it drew held.
    drawn_handles: Option<(usize, Option<Drag>)>,
    /// What it draws with: the Space's palette as it opened, which holds while the user drags
    /// through themes that would change it.
    palette: Palette,
}

pub struct ThemePicker {
    host: Rc<dyn Host>,
    focus: FocusHandle,
    editing: Option<Editing>,
}

struct ThemePickerWindow(WindowHandle<ThemePicker>);

impl Global for ThemePickerWindow {}

/// Opens the panel's window, hidden, for `host` to show over the browser window, ahead of time as
/// the icon picker's is.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<ThemePicker>> {
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(WIDTH), px(HEIGHT)));
    open_at(host, bounds, false, cx)
}

/// Opens the panel's window at `bounds`, shown if `show`, which the preview does.
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
        KeyBinding::new("shift-left", KeyLeftFar, Some("ThemePicker")),
        KeyBinding::new("shift-right", KeyRightFar, Some("ThemePicker")),
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
        // The panel's shape is drawn, so the window around it is clear.
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| {
        cx.new(|cx| ThemePicker {
            host,
            focus: cx.focus_handle(),
            editing: None,
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

/// Where the panel opens in the browser window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum At {
    /// Beside a row: the Create a Space form's Choose a Theme row, or a Library card's brush, at
    /// the middle of the row's end toward the page, the panel's head level with it.
    Row(Point<f32>),
    /// Against the sidebar's edge this far across, at the window's top.
    Edge(f32),
    /// Beside the Space's name, 8 right of the sidebar's edge `edge` across, its top at the name
    /// row's top `top` down (the spec's).
    Beside { edge: f32, top: f32 },
}

/// The panel's corners, clockwise from its top corner on the side it opens from, which it grows
/// in from: its top left, or opened to the left, its top right.
pub fn corners(left: bool) -> Vec<(f32, f32, f32)> {
    let corners = [
        (0., 0., RADIUS),
        (WIDTH, 0., RADIUS),
        (WIDTH, HEIGHT, RADIUS),
        (0., HEIGHT, RADIUS),
    ];
    let mut corners = corners.to_vec();
    if left {
        for corner in &mut corners {
            corner.0 = WIDTH - corner.0;
        }
        corners[1..].reverse();
    }
    corners
}

/// The form's corners from its window's top left, each with the radius it's rounded by, clockwise
/// from where it grows in from: the arrow's tip `arrow_y` down the window, or with no arrow the
/// card's top corner on the sidebar's side, for a card `height` tall, as the Boost editor's is.
pub fn corners_of(arrow_y: Option<f32>, left: bool, height: f32) -> Vec<(f32, f32, f32)> {
    let card = [
        (ARROW, 0., FORM_RADIUS),
        (FORM_WIDTH, 0., FORM_RADIUS),
        (FORM_WIDTH, height, FORM_RADIUS),
        (ARROW, height, FORM_RADIUS),
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
            corner.0 = FORM_WIDTH - corner.0;
        }
        corners[1..].reverse();
    }
    corners
}

/// Where the form's card starts across its window: past the arrow's room on the window's left,
/// or at it with the arrow on its right.
pub fn card_x(left: bool) -> f32 {
    if left { 0. } else { ARROW }
}

/// Where the panel's window goes in a browser window `height` tall: its top left, kept `MARGIN`
/// off the window's bottom and top, and no arrow. `left` opens it to the left of `at`, toward the
/// page from a sidebar on the right.
pub fn placed(at: At, left: bool, height: f32) -> (f32, f32, Option<f32>) {
    let (x, top) = match at {
        At::Beside { edge, top } => (edge, top),
        At::Edge(edge) => (edge, MARGIN),
        // Its head, the first thing read, level with the row.
        At::Row(anchor) => (anchor.x, anchor.y - PADDING - HEAD / 2.),
    };
    let top = top.min(height - MARGIN - HEIGHT).max(MARGIN);
    let x = if left { x - BESIDE - WIDTH } else { x + BESIDE };
    (x, top, None)
}

/// Where the form's window goes, `card` tall, in a browser window `height` tall: its top left,
/// and the arrow's tip down it if it has one. Beside a row, the card centres on the arrow, as
/// Arc's does, unless that takes it past the browser window's top or bottom.
pub fn placed_of(at: At, left: bool, height: f32, card: f32) -> (f32, f32, Option<f32>) {
    let anchor = match at {
        // The window is as wide as the card and its arrow, so it starts that far left of the card.
        At::Edge(x) | At::Beside { edge: x, .. } if left => {
            return (x - EDGE_INSET - CARD_WIDTH, EDGE_INSET, None);
        }
        At::Edge(x) | At::Beside { edge: x, .. } => {
            return (x + EDGE_INSET - ARROW, EDGE_INSET, None);
        }
        At::Row(anchor) => anchor,
    };
    let top = (anchor.y - card / 2.)
        .min(height - FORM_MARGIN - card)
        .max(FORM_MARGIN);
    // The tip keeps clear of the card's rounded corners.
    let reach = FORM_RADIUS + ARROW + ARROW_FOOT_RADIUS;
    let arrow_y = (anchor.y - top).clamp(reach, card - reach);
    let x = if left {
        anchor.x - FORM_GAP + TIP_SHORT - FORM_WIDTH
    } else {
        anchor.x + FORM_GAP - TIP_SHORT
    };
    (x, top, Some(arrow_y))
}

/// Shows the panel for `space`, which has `theme` or none yet, `at` its place in browser window
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
            let (x, top, _) = placed(at, left, height);
            picker.editing = Some(Editing {
                space,
                theme,
                left,
                glass: crate::space_transparency::START,
                appearance: theme.map_or(Appearance::default(), |theme| theme.appearance),
                changed,
                also: None,
                open: true,
                drag: None,
                chosen: 0,
                typing: None,
                keyed: None,
                drawn: false,
                drawn_handles: None,
                // The panel is a dark sheet over a dark or a light window, as the icon picker is.
                palette: Palette::of(theme.as_ref(), Tone::Dark),
            });
            window.focus(&picker.focus, cx);
            picker.host.show(over, x, top, &corners(left));
            cx.notify();
        })
        .ok();
}

/// What the panel shown sets for its Space beside its theme: its glass, from `also.transparency`,
/// and its colours taken away.
pub fn also(also: Also, cx: &mut App) {
    if let Some(window) = cx.try_global::<ThemePickerWindow>().map(|picker| picker.0) {
        window
            .update(cx, |picker, _, cx| {
                if let Some(editing) = picker.editing.as_mut().filter(|editing| editing.open) {
                    editing.glass = also.transparency.clamp(0., 1.);
                    editing.also = Some(also);
                    cx.notify();
                }
            })
            .ok();
    }
}

/// The user left the panel, such as with a click outside it.
pub fn dismissed(cx: &mut App) {
    if let Some(window) = cx.try_global::<ThemePickerWindow>().map(|picker| picker.0) {
        let close = Close::left(cx.active_window().is_none());
        window
            .update(cx, |picker, window, cx| picker.close(close, window, cx))
            .ok();
    }
}

/// The Space it edits is going from under it, as a swipe turns the page: it closes as Esc closes
/// it, where `dismissed` hears of a close the host has made.
pub fn leave(cx: &mut App) {
    if let Some(window) = cx.try_global::<ThemePickerWindow>().map(|picker| picker.0) {
        window
            .update(cx, |picker, window, cx| {
                picker.close(Close::new(Why::FocusLeft, By::Code), window, cx)
            })
            .ok();
    }
}

/// Whether the panel shows, for a click on what opened it to close it rather than open it again.
pub fn is_open(cx: &App) -> bool {
    open_for(cx).is_some()
}

/// The Space the panel shows for, if it shows.
pub fn open_for(cx: &App) -> Option<u64> {
    let picker = cx.try_global::<ThemePickerWindow>()?.0.read(cx).ok()?;
    picker
        .editing
        .as_ref()
        .filter(|editing| editing.open)
        .map(|editing| editing.space)
}

/// The panel, for the window it opens over to watch whether it still shows.
pub fn entity(cx: &App) -> Option<Entity<ThemePicker>> {
    cx.try_global::<ThemePickerWindow>()?.0.entity(cx).ok()
}

/// What a press at `x`, `y` from the panel's top left lands on, with `theme` on the pad.
pub fn target(x: f32, y: f32, theme: Option<&Theme>) -> Option<Target> {
    let within = |left: f32, top: f32, width: f32, height: f32| {
        (left..=left + width).contains(&x) && (top..=top + height).contains(&y)
    };
    for (index, &appearance) in Appearance::ALL.iter().enumerate() {
        let (left, top) = layout::segment(index);
        if within(left, top, SEGMENT_WIDTH, SEGMENT_HEIGHT) {
            return Some(Target::Appearance(appearance));
        }
    }
    for index in 0..layout::palette_count() {
        let (dx, dy) = layout::disc(index);
        if (x - dx).hypot(y - dy) <= SLOT / 2. {
            return Some(Target::Palette(index));
        }
    }
    let count = theme.map_or(0, |theme| theme.count);
    let (chip_left, chip_top, chip_width) = layout::chip(count);
    if theme.is_some() && within(chip_left, chip_top, chip_width, CHIP_HEIGHT) {
        let nearest = (0..=count.min(MAX_COLORS - 1))
            .filter(|&index| index < count || count < MAX_COLORS)
            .min_by(|&a, &b| {
                let (ax, _) = layout::chip_item(count, a);
                let (bx, _) = layout::chip_item(count, b);
                (x - ax).abs().total_cmp(&(x - bx).abs())
            });
        return nearest.map(|index| match index < count {
            true => Target::Point(index),
            false => Target::Add,
        });
    }
    let (field_left, field_top) = layout::value_field();
    if theme.is_some() && within(field_left, field_top, FIELD_WIDTH, CHIP_HEIGHT) {
        return Some(Target::Value);
    }
    let (pad_left, pad_top) = layout::pad();
    if within(pad_left, pad_top, PAD_WIDTH, PAD_HEIGHT) {
        // The main handle lies over the others.
        if let Some(theme) = theme {
            let mut order: Vec<usize> = (1..theme.count).collect();
            order.insert(0, 0);
            for index in order {
                let (hx, hy) = layout::handle(theme, index);
                if (x - hx).hypot(y - hy) <= handle_size(index) / 2. {
                    return Some(Target::Handle(index));
                }
            }
        }
        return Some(Target::Pad);
    }
    let (track_left, length) = layout::track();
    for bar in BARS {
        let middle = layout::track_y(bar);
        if (track_left..=track_left + length).contains(&x) && (y - middle).abs() <= THUMB / 2. {
            return Some(Target::Bar(bar));
        }
    }
    let note = layout::note_top();
    if within(WIDTH - PADDING - RESET_WIDTH, note, RESET_WIDTH, LABEL) {
        return Some(Target::Reset);
    }
    None
}

fn handle_size(index: usize) -> f32 {
    if index == 0 { MAIN_HANDLE } else { HANDLE }
}

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

/// What an assistive app reads of a bar called `name` at `value` of 0 to 1, in percent, a point a
/// step, which its increment and decrement move by, as `step` does.
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
        .aria_numeric_value_step(1.)
        .when_some(value, |slider, value| {
            slider.aria_numeric_value((value * 100.).round() as f64)
        })
        .when(value.is_none(), |slider| {
            slider.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
        })
        .on_a11y_action(Action::Increment, move |_, _, cx| step(KEY_STEP, cx))
        .on_a11y_action(Action::Decrement, move |_, _, cx| down(-KEY_STEP, cx))
}

/// A bar's value in the panel: Intensity's and Grain's the theme's, none without a colour, and
/// Transparency's the Space's glass.
fn bar_value(bar: Bar, theme: Option<&Theme>, glass: f32) -> Option<f32> {
    match bar {
        Bar::Intensity => theme.map(|theme| theme.intensity),
        Bar::Transparency => Some(glass),
        Bar::Grain => theme.map(|theme| theme.grain),
    }
}

/// How bar `bar` reads beside its label: Grain at none is Off, as drawn.
fn reading(bar: Bar, value: f32) -> String {
    match bar {
        Bar::Grain if value <= 0. => "Off".into(),
        _ => percent(value),
    }
}

/// How a bar's value reads beside its label, in whole percent.
fn percent(value: f32) -> String {
    format!("{}%", (value * 100.).round() as i32)
}

/// The stops tab goes round, top to bottom, each a row that left and right go along: the
/// palettes, the chip, the value field, each bar and Reset. The chip, the field and Intensity
/// wait for a colour, as their clicks do.
fn key_stops(theme: Option<&Theme>) -> Vec<Vec<Target>> {
    let appearances = Appearance::ALL.iter().map(|&each| Target::Appearance(each));
    let palettes = (0..layout::palette_count()).map(Target::Palette).collect();
    let mut stops = vec![appearances.collect(), palettes];
    if let Some(theme) = theme {
        let mut points: Vec<Target> = (0..theme.count).map(Target::Point).collect();
        if theme.count < MAX_COLORS {
            points.push(Target::Add);
        }
        stops.push(points);
        stops.push(vec![Target::Value]);
    }
    for bar in BARS {
        if bar_value(bar, theme, 0.).is_some() {
            stops.push(vec![Target::Bar(bar)]);
        }
    }
    stops.push(vec![Target::Reset]);
    stops
}

/// Where tab lands in `stop`: the appearance chosen, the palette the Space has, the colour
/// chosen, else the first.
fn key_entry(stop: &[Target], theme: Option<&Theme>, chosen: usize, look: Appearance) -> Target {
    let palette = layout::palette_of(theme);
    let answers = |target: &&Target| match target {
        Target::Appearance(appearance) => *appearance == look,
        Target::Palette(index) => palette.is_none_or(|palette| palette == *index),
        Target::Point(index) => *index == chosen,
        _ => true,
    };
    *stop.iter().find(answers).unwrap_or(&stop[0])
}

/// A theme of `count` colours with colour `index` made `color`, the rest where they were: the main
/// one moves the theme, as its handle does, and another is placed alone.
fn with_color(theme: &Theme, index: usize, color: u32) -> Theme {
    let made = Theme::of_color(color);
    if index == 0 {
        return Theme {
            x: made.x,
            y: made.y,
            saturation: made.saturation,
            ..*theme
        };
    }
    theme.moved(index, made.x, made.y)
}

impl ThemePicker {
    /// Closes the panel for `close`, hiding it unless the host already has, as the user left it.
    /// A value being typed is kept as Return would keep it, and what's held as letting go would.
    fn close(&mut self, close: Close, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_value(window, cx);
        self.release(cx);
        let Some(editing) = self.editing.as_mut().filter(|editing| editing.open) else {
            return;
        };
        editing.open = false;
        editing.drag = None;
        cx.notify();
        if !matches!(close.why, Why::PressOutside | Why::WindowLeft) {
            // check.sh reads this line.
            log!("browser: theme picker dismissed");
            self.host.hide();
        }
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        // Esc in the value field puts back the value it showed, and the panel stays.
        if let Some(editing) = self.editing.as_mut()
            && editing.typing.take().is_some()
        {
            window.focus(&self.focus, cx);
            cx.notify();
            return;
        }
        self.close(Close::new(Why::Escape, By::Keyboard), window, cx);
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

    /// Hands the glass on: kept, once `done`.
    fn glassed(&mut self, done: bool, cx: &mut Context<Self>) {
        let Some(editing) = &self.editing else {
            return;
        };
        if done {
            log!(
                "browser: theme picker glass {} {:.3}",
                editing.space,
                editing.glass
            );
        }
        if let Some(also) = &editing.also {
            (also.glassed.clone())(editing.glass, done, cx);
        }
        cx.notify();
    }

    /// Takes the Space's colours away.
    fn cleared(&mut self, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        editing.theme = None;
        editing.chosen = 0;
        log!("browser: theme picker cleared {}", editing.space);
        if let Some(also) = &editing.also {
            (also.cleared.clone())(cx);
        }
        cx.notify();
    }

    /// Sets bar `bar` to `value` and hands it on, kept if `done`; Intensity waits for a colour.
    fn set_bar(&mut self, bar: Bar, value: f32, done: bool, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut().filter(|editing| editing.open) else {
            return;
        };
        let value = value.clamp(0., 1.);
        match bar {
            Bar::Intensity => {
                let Some(theme) = editing.theme else {
                    return;
                };
                if theme.intensity == value && !done {
                    return;
                }
                editing.theme = Some(Theme {
                    intensity: value,
                    ..theme
                });
                self.changed(done, cx);
            }
            Bar::Transparency => {
                if editing.glass == value && !done {
                    return;
                }
                editing.glass = value;
                self.glassed(done, cx);
            }
            Bar::Grain => {
                let Some(theme) = editing.theme else {
                    return;
                };
                if theme.grain == value && !done {
                    return;
                }
                editing.theme = Some(Theme {
                    grain: value,
                    ..theme
                });
                self.changed(done, cx);
            }
        }
    }

    /// Moves bar `bar` `by`, as a key or VoiceOver's increment and decrement do, onto the steps,
    /// and keeps it; a bump as it gets to an end.
    fn step(&mut self, bar: Bar, by: f32, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_ref().filter(|editing| editing.open) else {
            return;
        };
        let Some(before) = bar_value(bar, editing.theme.as_ref(), editing.glass) else {
            return;
        };
        // Onto the points, so a thumb a drag left between them lands on one.
        let value = (((before + by) / KEY_STEP).round() * KEY_STEP).clamp(0., 1.);
        if value != before {
            self.set_bar(bar, value, true, cx);
        }
    }

    fn press(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let (x, y) = (f32::from(event.position.x), f32::from(event.position.y));
        let Some(editing) = self.editing.as_ref().filter(|editing| editing.open) else {
            return;
        };
        let theme = editing.theme;
        let target = target(x, y, theme.as_ref());
        // A press off the value field keeps what's typed in it, as Return does.
        if target != Some(Target::Value) {
            self.commit_value(window, cx);
        }
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        // The ring is the keyboard's, and goes as the mouse takes over.
        editing.keyed = None;
        let Some(target) = target else {
            cx.notify();
            return;
        };
        match (target, editing.theme) {
            (
                Target::Appearance(_)
                | Target::Palette(_)
                | Target::Point(_)
                | Target::Add
                | Target::Reset,
                _,
            ) => {
                self.pick(target, window, cx);
            }
            (Target::Value, Some(_)) => self.type_value(window, cx),
            (Target::Bar(bar), theme) => {
                if bar_value(bar, theme.as_ref(), editing.glass).is_some() {
                    editing.drag = Some(Drag::Bar(bar));
                    self.set_bar(bar, layout::value_at(x), false, cx);
                }
            }
            (Target::Handle(index), Some(theme)) => {
                let (hx, hy) = layout::handle(&theme, index);
                editing.drag = Some(Drag::Handle(index, x - hx, y - hy));
                editing.chosen = index;
            }
            // The first colour goes where the pad is pressed, and follows the pointer from there.
            (Target::Pad, None) => {
                let (across, down) = layout::on_pad(x, y);
                let (fx, fy) = layout::field_point(across, down);
                let appearance = editing.appearance;
                editing.theme = Some(Theme {
                    appearance,
                    ..Theme::at(fx, fy)
                });
                editing.drag = Some(Drag::Handle(0, 0., 0.));
                editing.chosen = 0;
                self.changed(false, cx);
            }
            _ => {}
        }
        cx.notify();
    }

    /// Presses a palette, a point, the plus or Reset, by click or key, and says whether it
    /// answered: the plus doesn't at three colours or without one.
    fn pick(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(editing) = self.editing.as_mut().filter(|editing| editing.open) else {
            return false;
        };
        match (target, editing.theme) {
            (Target::Appearance(appearance), theme) => {
                editing.appearance = appearance;
                editing.theme = theme.map(|theme| Theme {
                    appearance,
                    ..theme
                });
                self.changed(true, cx);
            }
            (Target::Palette(0), _) => self.cleared(cx),
            (Target::Palette(index), theme) => {
                let Some(made) = layout::palette_theme(index, theme.as_ref()) else {
                    return false;
                };
                let appearance = theme.map_or(editing.appearance, |theme| theme.appearance);
                editing.theme = Some(Theme { appearance, ..made });
                editing.chosen = 0;
                self.changed(true, cx);
            }
            (Target::Point(index), Some(theme)) if index < theme.count => {
                editing.chosen = index;
                cx.notify();
            }
            (Target::Add, Some(theme)) if theme.count < MAX_COLORS => {
                editing.theme = Some(theme.with_count(theme.count + 1));
                editing.chosen = theme.count;
                self.changed(true, cx);
            }
            (Target::Value, Some(_)) => self.type_value(window, cx),
            (Target::Reset, _) => {
                // The look a new Space comes with: no colour, its glass at the start.
                if editing.theme.is_some() {
                    self.cleared(cx);
                }
                if let Some(editing) = self.editing.as_mut() {
                    editing.glass = crate::space_transparency::START;
                }
                self.glassed(true, cx);
            }
            _ => return false,
        }
        true
    }

    /// Puts the value field into typing, its value chosen, with the keyboard.
    fn type_value(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut().filter(|editing| editing.open) else {
            return;
        };
        if editing.typing.is_some() {
            return;
        }
        let Some(theme) = editing.theme else {
            return;
        };
        let shown = theme.colors().get(editing.chosen).copied().unwrap_or(0);
        let palette = editing.palette;
        let input = cx.new(|cx| {
            TextInput::new("Color value", cx)
                .follows(move |_: &App| Colors::placeholder(palette.on_overlay(palette.tertiary())))
                .spoken("Color value")
        });
        input.update(cx, |input, cx| {
            input.reset(layout::color_text(shown).into(), window, cx)
        });
        window.focus(&input.focus_handle(cx), cx);
        // Or the press that chose it gives the panel the keyboard back.
        window.prevent_default();
        editing.typing = Some(input);
        cx.notify();
    }

    /// Keeps the value typed in the field, if it names a colour: six hex digits, with or without
    /// the sign. Anything else is refused and the old value stays.
    fn commit_value(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        let Some(input) = editing.typing.take() else {
            return;
        };
        let typed = input.read(cx);
        // A value still being composed in an input method is kept as it's shown.
        let typed = if typed.composing() {
            typed.text().to_owned()
        } else {
            typed.committed()
        };
        window.focus(&self.focus, cx);
        let (Some(color), Some(theme)) = (layout::typed_color(&typed), editing.theme) else {
            log!("browser: theme picker value refused");
            cx.notify();
            return;
        };
        editing.theme = Some(with_color(&theme, editing.chosen, color));
        self.changed(true, cx);
    }

    /// Return in the value field keeps it, and is the field's while an input method composes in
    /// it; with no field open the key is the ringed part's.
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let typing = self
            .editing
            .as_ref()
            .and_then(|editing| editing.typing.clone());
        let Some(typing) = typing else {
            return false;
        };
        if crate::dismissal::submits([&typing], cx) {
            self.commit_value(window, cx);
        }
        true
    }

    /// Tab, or shift-tab back: the ring goes to the next stop round, or the first.
    fn key_stop(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_value(window, cx);
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
        let look = editing
            .theme
            .map_or(editing.appearance, |theme| theme.appearance);
        let keyed = key_entry(&stops[next], editing.theme.as_ref(), editing.chosen, look);
        editing.keyed = Some(keyed);
        cx.notify();
    }

    /// Left or right: along the row the ring is in, stopping at its ends, or a bar's thumb `by`
    /// down or up, kept as each press would be.
    fn key_along(&mut self, right: bool, by: f32, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut().filter(|editing| editing.open) else {
            return;
        };
        let Some(keyed) = editing.keyed else {
            return;
        };
        if let Target::Bar(bar) = keyed {
            self.step(bar, if right { by } else { -by }, cx);
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
    fn key_press(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.submit(window, cx) {
            return;
        }
        let open = self.editing.as_ref().filter(|editing| editing.open);
        if let Some(keyed) = open.and_then(|editing| editing.keyed) {
            self.pick(keyed, window, cx);
        }
    }

    fn drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let (x, y) = (f32::from(position.x), f32::from(position.y));
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        let Some(drag) = editing.drag else {
            return;
        };
        match drag {
            Drag::Bar(bar) => {
                let before = bar_value(bar, editing.theme.as_ref(), editing.glass);
                let value = layout::value_at(x);
                let at_end = |value: f32| value <= 0. || value >= 1.;
                // A bump as it gets to an end, not while it's held there (the owner's call).
                if before.is_some_and(|before| before != value && !at_end(before)) && at_end(value)
                {
                    self.host.detent();
                }
                self.set_bar(bar, value, false, cx);
            }
            Drag::Handle(index, grab_x, grab_y) => {
                let Some(theme) = editing.theme else {
                    return;
                };
                let (across, down) = layout::on_pad(x - grab_x, y - grab_y);
                let (fx, fy) = layout::field_point(across, down);
                let moved = theme.moved(index, fx, fy);
                if moved != theme {
                    editing.theme = Some(moved);
                    self.changed(false, cx);
                }
            }
        }
    }

    fn release(&mut self, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        match editing.drag.take() {
            Some(Drag::Bar(Bar::Transparency)) => self.glassed(true, cx),
            Some(_) => self.changed(true, cx),
            None => {}
        }
    }

    /// The Space it's making a theme for, and the theme as it is, while it shows.
    pub fn editing(&self) -> Option<(u64, Option<Theme>)> {
        let editing = self.editing.as_ref().filter(|editing| editing.open)?;
        Some((editing.space, editing.theme))
    }

    /// The glass it's making for its Space, while it shows.
    pub fn glass(&self) -> Option<f32> {
        let editing = self.editing.as_ref().filter(|editing| editing.open)?;
        Some(editing.glass)
    }
}

impl Editing {
    /// Logs the colour handles as they're drawn at rest, and what's held. GPUI draws no frame
    /// between a press and the next mouse event, and only a frame drawn while a handle or a thumb
    /// is held listens for the drag's moves, so check.sh waits for these lines before it presses
    /// and before it drags.
    fn log_handles(&mut self) {
        let held = self.drag.map(|drag| match drag {
            Drag::Handle(index, ..) => Drag::Handle(index, 0., 0.),
            bar => bar,
        });
        let count = self.theme.map_or(0, |theme| theme.count);
        let handles = (self.open && self.drawn).then_some((count, held));
        if handles.is_none() || handles == self.drawn_handles {
            return;
        }
        self.drawn_handles = handles;
        if let Some((count, held)) = handles {
            match held {
                Some(Drag::Handle(index, ..)) => {
                    log!("browser: theme picker handles {count} drawn, {index} held")
                }
                Some(Drag::Bar(bar)) => {
                    let name = bar.name().to_lowercase();
                    log!("browser: theme picker handles {count} drawn, {name} held")
                }
                None => log!("browser: theme picker handles {count} drawn"),
            }
        }
    }
}

thread_local! {
    static DOTS: OnceCell<Arc<RenderImage>> = const { OnceCell::new() };
}

/// The pad's dots, drawn once at 2×, each in the colour a handle takes where it lies, so the pad
/// is a picture of the colours on it.
fn dots_image() -> Arc<RenderImage> {
    DOTS.with(|dots| {
        dots.get_or_init(|| {
            let (width, height) = ((PAD_WIDTH * 2.) as usize, (PAD_HEIGHT * 2.) as usize);
            let mut data = vec![0u8; width * height * 4];
            let (across, down) = (DOTS_ACROSS as usize, DOTS_DOWN as usize);
            let radius = DOT_SIZE; // at 2×, half the dot's size twice over
            for row in 0..down {
                for column in 0..across {
                    let (u, v) = (
                        column as f32 / (across - 1) as f32,
                        row as f32 / (down - 1) as f32,
                    );
                    let (fx, fy) = layout::field_point(u, v);
                    let color = theme::color_at(fx, fy, 1.);
                    let [r, g, b] = [16, 8, 0].map(|shift| ((color >> shift) & 0xff) as u8);
                    let (cx, cy) = (
                        (DOT_LEFT + column as f32 * DOT_PITCH) * 2.,
                        (DOT_TOP + row as f32 * DOT_PITCH) * 2.,
                    );
                    let reach = radius.ceil() as i32 + 1;
                    for dy in -reach..=reach {
                        for dx in -reach..=reach {
                            let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                            if x < 0 || y < 0 || x as usize >= width || y as usize >= height {
                                continue;
                            }
                            let distance = ((x as f32 + 0.5 - cx).hypot(y as f32 + 0.5 - cy)
                                - radius)
                                .clamp(-0.5, 0.5);
                            let cover = 0.5 - distance;
                            if cover <= 0. {
                                continue;
                            }
                            let at = (y as usize * width + x as usize) * 4;
                            let alpha = (DOT_ALPHA * cover * 255.).round() as u8;
                            data[at..at + 4].copy_from_slice(&[r, g, b, alpha]);
                        }
                    }
                }
            }
            let image = RgbaImage::from_raw(width as u32, height as u32, data).expect("sized");
            Arc::new(RenderImage::new(vec![Frame::new(image)]))
        })
        .clone()
    })
}

/// A shadow under a handle or a thumb, as drawn.
fn drop_shadow(color: u32, y: f32, blur: f32) -> Vec<gpui::BoxShadow> {
    vec![gpui::BoxShadow {
        color: rgba(color).into(),
        offset: point(px(0.), px(y)),
        // GPUI's blur is the Gaussian's deviation, half of CSS's.
        blur_radius: px(blur / 2.),
        spread_radius: px(0.),
        inset: false,
    }]
}

/// `color` at `alpha`.
fn at_alpha(color: u32, alpha: f32) -> Hsla {
    Hsla::from(rgb(color)).opacity(alpha)
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
        // the release, the press read against the panel as it is now, and a click as the panel
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
            .on_action(
                cx.listener(|picker, _: &NextStop, window, cx| picker.key_stop(true, window, cx)),
            )
            .on_action(cx.listener(|picker, _: &PreviousStop, window, cx| {
                picker.key_stop(false, window, cx)
            }))
            .on_action(
                cx.listener(|picker, _: &KeyLeft, _, cx| picker.key_along(false, KEY_STEP, cx)),
            )
            .on_action(
                cx.listener(|picker, _: &KeyRight, _, cx| picker.key_along(true, KEY_STEP, cx)),
            )
            .on_action(
                cx.listener(|picker, _: &KeyLeftFar, _, cx| picker.key_along(false, KEY_FAR, cx)),
            )
            .on_action(
                cx.listener(|picker, _: &KeyRightFar, _, cx| picker.key_along(true, KEY_FAR, cx)),
            )
            .on_action(cx.listener(|picker, _: &KeyPress, window, cx| picker.key_press(window, cx)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|picker, event: &MouseDownEvent, window, cx| {
                    picker.press(event, window, cx)
                }),
            )
            .relative()
            .size_full()
            .child(listener);
        let Some(editing) = &mut self.editing else {
            return root.child(picker);
        };
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
        let colors = theme.map_or_else(Vec::new, |theme| theme.colors());
        let palette = editing.palette;
        let keyed = editing.keyed;
        let text = palette.on_overlay(1.);
        let text_2 = palette.on_overlay(palette.secondary());
        let text_3 = palette.on_overlay(palette.tertiary());
        let ring_color = palette.plate(KEYED);
        // The keyboard's ring, as Focus Sidebar's, `RING_GAP` round what it rings, `width` by
        // `height` about `x`, `y`.
        let ring = |x: f32, y: f32, width: f32, height: f32| {
            let reach = 2. * (RING_GAP + KEYED_RING);
            let (width, height) = (width + reach, height + reach);
            div()
                .absolute()
                .left(px(x - width / 2.))
                .top(px(y - height / 2.))
                .w(px(width))
                .h(px(height))
                .rounded(px(width.min(height) / 2.))
                .border(px(KEYED_RING))
                .border_color(ring_color)
        };
        let keyed_ring = keyed.map(|keyed| match keyed {
            Target::Appearance(appearance) => {
                let index = Appearance::ALL.iter().position(|&each| each == appearance);
                let (left, top) = layout::segment(index.unwrap_or(0));
                ring(
                    left + SEGMENT_WIDTH / 2.,
                    top + SEGMENT_HEIGHT / 2.,
                    SEGMENT_WIDTH,
                    SEGMENT_HEIGHT,
                )
            }
            Target::Palette(index) => {
                let (x, y) = layout::disc(index);
                ring(x, y, SLOT, SLOT)
            }
            Target::Point(index) => {
                let (x, y) = layout::chip_item(colors.len(), index);
                ring(x, y, CHIP_DOT, CHIP_DOT)
            }
            Target::Add => {
                let (x, y) = layout::chip_item(colors.len(), colors.len());
                ring(x, y, PLUS, PLUS)
            }
            Target::Value => {
                let (left, top) = layout::value_field();
                ring(
                    left + FIELD_WIDTH / 2.,
                    top + CHIP_HEIGHT / 2.,
                    FIELD_WIDTH,
                    CHIP_HEIGHT,
                )
            }
            Target::Bar(bar) => {
                let value = bar_value(bar, theme.as_ref(), editing.glass).unwrap_or(0.);
                ring(layout::thumb_x(value), layout::track_y(bar), THUMB, THUMB)
            }
            _ => {
                let top = layout::note_top();
                ring(
                    WIDTH - PADDING - RESET_WIDTH / 2.,
                    top + LABEL / 2.,
                    RESET_WIDTH,
                    LABEL,
                )
            }
        });

        let left = editing.left;
        let card_fill = palette::current(cx).card_drawn();
        let card = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                icon_picker::paint_card(bounds.origin, &corners(left), card_fill, window)
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        // The Space's colours over the fill, as drawn: the first fading to the second.
        let tint = colors.first().map(|&first| {
            let second = colors.get(1).copied().unwrap_or(first);
            div()
                .absolute()
                .size_full()
                .rounded(px(RADIUS))
                .bg(linear_gradient(
                    TINT_ANGLE,
                    linear_color_stop(at_alpha(first, TINT_FROM), 0.),
                    linear_color_stop(at_alpha(second, TINT_TO), TINT_REACH),
                ))
        });

        // A label row, its name on the left and what it says on the right.
        let label_row = |top: f32, name: SharedString, name_color: Hsla, value: Option<String>| {
            div()
                .absolute()
                .left(px(PADDING + LABEL_INSET))
                .top(px(top))
                .w(px(INNER - 2. * LABEL_INSET))
                .h(px(LABEL))
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(LABEL_SIZE))
                .line_height(px(LABEL))
                .child(div().text_color(name_color).child(name))
                .children(value.map(|value| div().text_color(text_2).child(value)))
        };

        let head = div()
            .absolute()
            .left(px(PADDING + LABEL_INSET))
            .top(px(PADDING))
            .h(px(HEAD))
            .flex()
            .items_center()
            .text_size(px(TITLE_SIZE))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(text)
            .child("Theme");

        // Auto · Light · Dark, the Space's appearance, at the head's right.
        let look = theme.map_or(editing.appearance, |theme| theme.appearance);
        let frame = 3. * SEGMENT_WIDTH + 2. * SEGMENT_INSET;
        let segments = div()
            .absolute()
            .left(px(WIDTH - PADDING - frame))
            .top(px(PADDING + (HEAD - SEGMENT_FRAME) / 2.))
            .w(px(frame))
            .h(px(SEGMENT_FRAME))
            .rounded(px(SEGMENT_FRAME_RADIUS))
            .bg(palette.plate(crate::palette::FIELD_PLATE));
        let segment_words = Appearance::ALL
            .iter()
            .enumerate()
            .map(|(index, &appearance)| {
                let (word, name) = match appearance {
                    Appearance::Automatic => ("Auto", "Automatic"),
                    Appearance::Light => ("Light", "Light"),
                    Appearance::Dark => ("Dark", "Dark"),
                };
                let chosen = appearance == look;
                let (left, top) = layout::segment(index);
                div()
                    .id(name)
                    // The panel answers presses where they land, so its press is a click there.
                    .when(a11y, |segment| {
                        segment
                            .role(Role::RadioButton)
                            .aria_label(name)
                            .aria_toggled(if chosen {
                                Toggled::True
                            } else {
                                Toggled::False
                            })
                            .a11y_synthetic_children(|builder| {
                                builder.parent_node().add_action(Action::Click)
                            })
                    })
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(SEGMENT_WIDTH))
                    .h(px(SEGMENT_HEIGHT))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(SEGMENT_RADIUS))
                    .text_size(px(LABEL_SIZE))
                    .line_height(px(LABEL))
                    .when(chosen, |segment| {
                        segment
                            .bg(palette.shown(0.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(text)
                    })
                    .when(!chosen, |segment| segment.text_color(text_2))
                    .child(word)
            });

        let chosen_palette = layout::palette_of(theme.as_ref());
        let palettes_label = label_row(
            layout::palettes_top(),
            "Palettes".into(),
            text_3,
            layout::chosen_name(theme.as_ref()).map(str::to_owned),
        );
        let discs = (0..layout::palette_count()).map(|index| {
            let (x, y) = layout::disc(index);
            let chosen = chosen_palette == Some(index);
            let size = if chosen { CHOSEN_DISC } else { DISC };
            let colors = layout::palette_colors(index);
            let fill = match colors.as_slice() {
                [] => rgb(NO_COLOR).into(),
                [one] => rgb(*one).into(),
                [first, second, ..] => linear_gradient(
                    135.,
                    linear_color_stop(rgb(*first), 0.),
                    linear_color_stop(rgb(*second), 1.),
                ),
            };
            let name: SharedString = layout::palette_name(index).unwrap_or_default().into();
            div()
                .id(("palette", index))
                .when(a11y, |slot| {
                    slot.role(Role::RadioButton)
                        .aria_label(name)
                        .aria_toggled(if chosen {
                            Toggled::True
                        } else {
                            Toggled::False
                        })
                        .a11y_synthetic_children(|builder| {
                            builder.parent_node().add_action(Action::Click)
                        })
                })
                .absolute()
                .left(px(x - SLOT / 2.))
                .top(px(y - SLOT / 2.))
                .size(px(SLOT))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .when(chosen, |slot| {
                    slot.border(px(CHOSEN_RING)).border_color(text)
                })
                .child(
                    div()
                        .size(px(size))
                        .rounded_full()
                        .bg(fill)
                        .when(!chosen, |disc| {
                            disc.border_1().border_color(at_alpha(WHITE, DISC_RIM))
                        }),
                )
        });

        // The pad, its dots, its shade, the line between the handles, the handles, the chip and
        // the field, in the pad's own points.
        let (pad_left, pad_top) = layout::pad();
        let dots = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                window
                    .paint_image(
                        bounds,
                        bounds,
                        px(PAD_RADIUS).into(),
                        dots_image(),
                        0,
                        false,
                    )
                    .ok();
            },
        )
        .absolute()
        .size_full();
        let shade = div()
            .absolute()
            .size_full()
            .rounded(px(PAD_RADIUS))
            .bg(linear_gradient(
                180.,
                // The shade fades in from none of it.
                linear_color_stop(Hsla::from(rgba(SHADE)).opacity(0.), SHADE_FROM),
                linear_color_stop(rgba(SHADE), 1.),
            ));
        let handles: Vec<(f32, f32)> = theme.map_or_else(Vec::new, |theme| {
            (0..theme.count)
                .map(|index| {
                    let (x, y) = layout::handle(&theme, index);
                    (x - pad_left, y - pad_top)
                })
                .collect()
        });
        let line = {
            let (handles, colors) = (handles.clone(), colors.clone());
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    for (pair, color) in handles.windows(2).zip(colors.windows(2)) {
                        let (from, to) = (pair[0], pair[1]);
                        let at = |(x, y): (f32, f32)| bounds.origin + point(px(x), px(y));
                        let mut path = PathBuilder::stroke(px(LINE));
                        path.move_to(at(from));
                        path.line_to(at(to));
                        // The blend between the two, from the one to the other along the line.
                        let angle = 90. + (to.1 - from.1).atan2(to.0 - from.0).to_degrees();
                        let blend = linear_gradient(
                            angle,
                            linear_color_stop(rgb(color[0]), 0.),
                            linear_color_stop(rgb(color[1]), 1.),
                        );
                        if let Ok(path) = path.build() {
                            window.paint_path(path, blend);
                        }
                    }
                },
            )
            .absolute()
            .size_full()
        };
        // The main handle last, over the others.
        let handle_dots = (1..handles.len())
            .chain((!handles.is_empty()).then_some(0))
            .map(|index| {
                let (x, y) = handles[index];
                let (size, ring) = match index {
                    0 => (MAIN_HANDLE, MAIN_RING),
                    _ => (HANDLE, HANDLE_RING),
                };
                div()
                    .absolute()
                    .left(px(x - size / 2.))
                    .top(px(y - size / 2.))
                    .size(px(size))
                    .rounded_full()
                    .bg(rgb(colors[index]))
                    .border(px(ring))
                    .border_color(rgb(WHITE))
                    .shadow(drop_shadow(HANDLE_SHADOW, 3., 10.))
            });
        let count = colors.len();
        let chip = (count > 0).then(|| {
            let (left, top, width) = layout::chip(count);
            let (left, top) = (left - pad_left, top - pad_top);
            let points = (0..count).map(|index| {
                let chosen = index == editing.chosen;
                div()
                    .id(("point", index))
                    .when(a11y, |point| {
                        point
                            .role(Role::RadioButton)
                            .aria_label(swatch_name(colors[index]))
                            .aria_toggled(if chosen {
                                Toggled::True
                            } else {
                                Toggled::False
                            })
                            .a11y_synthetic_children(|builder| {
                                builder.parent_node().add_action(Action::Click)
                            })
                    })
                    .size(px(CHIP_DOT))
                    .flex_none()
                    .rounded_full()
                    .bg(rgb(colors[index]))
                    .when(chosen, |point| {
                        point.border(px(CHIP_RING)).border_color(rgb(WHITE))
                    })
            });
            let plus = (count < MAX_COLORS).then(|| {
                let at =
                    |x: f32, y: f32, bounds: Bounds<Pixels>| bounds.origin + point(px(x), px(y));
                div()
                    .id("add-color")
                    .when(a11y, |plus| {
                        plus.role(Role::Button)
                            .aria_label("Add Color")
                            .a11y_synthetic_children(|builder| {
                                builder.parent_node().add_action(Action::Click)
                            })
                    })
                    .size(px(PLUS))
                    .flex_none()
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| {
                                let reach = PLUS * 3.5 / 12.;
                                let middle = PLUS / 2.;
                                let mut path = PathBuilder::stroke(px(PLUS_STROKE));
                                path.move_to(at(middle, middle - reach, bounds));
                                path.line_to(at(middle, middle + reach, bounds));
                                path.move_to(at(middle - reach, middle, bounds));
                                path.line_to(at(middle + reach, middle, bounds));
                                if let Ok(path) = path.build() {
                                    window.paint_path(path, text_2);
                                }
                            },
                        )
                        .size_full(),
                    )
            });
            div()
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(width))
                .h(px(CHIP_HEIGHT))
                .pl(px(CHIP_LEFT))
                .pr(px(CHIP_RIGHT))
                .flex()
                .items_center()
                .gap(px(CHIP_GAP))
                .rounded(px(CHIP_HEIGHT / 2.))
                .bg(rgb(CHIP_FILL))
                .border_1()
                .border_color(at_alpha(WHITE, CHIP_RIM))
                .children(points)
                .children(plus)
        });
        let value_field = (count > 0).then(|| {
            let (left, top) = layout::value_field();
            let shown = colors.get(editing.chosen).copied().unwrap_or(colors[0]);
            let content = match &editing.typing {
                Some(input) => div().w_full().px(px(CHIP_LEFT)).child(input.clone()),
                None => div().child(layout::color_text(shown)),
            };
            div()
                .absolute()
                .left(px(left - pad_left))
                .top(px(top - pad_top))
                .w(px(FIELD_WIDTH))
                .h(px(CHIP_HEIGHT))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(CHIP_HEIGHT / 2.))
                .bg(rgb(CHIP_FILL))
                .border_1()
                .border_color(at_alpha(WHITE, CHIP_RIM))
                .text_size(px(LABEL_SIZE))
                .text_color(text_2)
                .child(content)
        });
        let pad = div()
            .id("theme-pad")
            .when(a11y, |pad| {
                let colours = match count {
                    0 => "Colors, none".to_string(),
                    1 => "Colors, 1".to_string(),
                    count => format!("Colors, {count}"),
                };
                pad.role(Role::Group)
                    .aria_label(colours)
                    // Its press on the empty pad places the first colour in its middle, as a
                    // click does.
                    .when(count == 0, |pad| {
                        pad.aria_description(HINT)
                            .a11y_synthetic_children(|builder| {
                                builder.parent_node().add_action(Action::Click)
                            })
                    })
            })
            .absolute()
            .left(px(pad_left))
            .top(px(pad_top))
            .w(px(PAD_WIDTH))
            .h(px(PAD_HEIGHT))
            .rounded(px(PAD_RADIUS))
            .overflow_hidden()
            .bg(rgb(PAD_FILL))
            .child(dots)
            .child(shade)
            .child(line)
            .children(handle_dots)
            .children(chip)
            .children(value_field)
            .child(
                div()
                    .absolute()
                    .size_full()
                    .rounded(px(PAD_RADIUS))
                    .border_1()
                    .border_color(at_alpha(WHITE, PAD_RIM)),
            );

        // The bars: each a label row, its track a small picture of what it changes, and a thumb.
        let (track_left, track_length) = layout::track();
        let bars = BARS.iter().map(|&bar| {
            let value = bar_value(bar, theme.as_ref(), editing.glass);
            let label = label_row(
                layout::bar_top(bar),
                bar.name().into(),
                text_2,
                value.map(|value| reading(bar, value)),
            );
            let first = colors.first().copied();
            let second = colors.get(1).copied().or(first);
            let field = palette.plate(crate::palette::FIELD_PLATE);
            let track = canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let o = bounds.origin;
                    let part = |from: f32, to: f32| {
                        Bounds::new(
                            o + point(px(track_length * from), px(0.)),
                            size(px(track_length * (to - from)), px(TRACK)),
                        )
                    };
                    let round = px(TRACK / 2.);
                    let left_round = gpui::Corners {
                        top_left: round,
                        bottom_left: round,
                        ..Default::default()
                    };
                    let right_round = gpui::Corners {
                        top_right: round,
                        bottom_right: round,
                        ..Default::default()
                    };
                    match bar {
                        Bar::Intensity => {
                            let (first, second) = match (first, second) {
                                (Some(first), Some(second)) => (first, second),
                                _ => (NO_COLOR, NO_COLOR),
                            };
                            window.paint_quad(
                                fill(
                                    part(0., INTENSITY_FULL),
                                    linear_gradient(
                                        90.,
                                        linear_color_stop(at_alpha(first, INTENSITY_FROM), 0.),
                                        linear_color_stop(at_alpha(first, 1.), 1.),
                                    ),
                                )
                                .corner_radii(left_round),
                            );
                            window.paint_quad(
                                fill(
                                    part(INTENSITY_FULL, 1.),
                                    linear_gradient(
                                        90.,
                                        linear_color_stop(rgb(first), 0.),
                                        linear_color_stop(rgb(second), 1.),
                                    ),
                                )
                                .corner_radii(right_round),
                            );
                        }
                        Bar::Transparency => {
                            window.paint_quad(
                                fill(part(0., GLASS_SOLID), rgb(GLASS_FROM))
                                    .corner_radii(left_round),
                            );
                            window.paint_quad(
                                fill(
                                    part(GLASS_SOLID, 1.),
                                    linear_gradient(
                                        90.,
                                        linear_color_stop(rgb(GLASS_FROM), 0.),
                                        linear_color_stop(rgb(GLASS_TO), 1.),
                                    ),
                                )
                                .corner_radii(right_round),
                            );
                            // Panes of glass, fading in to the clear end, as drawn: dashes 4
                            // long, 4 apart, in three rows, the middle one a dash along.
                            for (row, shift) in [(0., 0.), (1., 4.), (2., 0.)] {
                                let mut x = shift;
                                while x < track_length {
                                    let along = ((x - 20.) / (track_length - 20.)).clamp(0., 1.);
                                    let dash = Bounds::new(
                                        o + point(px(x), px(row * 4.)),
                                        size(px(4f32.min(track_length - x)), px(4.)),
                                    );
                                    window.paint_quad(fill(
                                        dash,
                                        at_alpha(
                                            crate::tokens::palette::PAPER,
                                            GLASS_LINES * along,
                                        ),
                                    ));
                                    x += 8.;
                                }
                            }
                        }
                        Bar::Grain => {
                            window.paint_quad(
                                fill(part(0., 1.), field).corner_radii(gpui::Corners::all(round)),
                            );
                            // Three rows of dots, the middle one half a step along and a dot
                            // shorter at each end, fading in from none at the left.
                            let rows = [(2., 2.5, 270.), (4.5, 6., 270.5), (2., 9.5, 270.)];
                            for (start, y, end) in rows {
                                let mut x = start;
                                while x <= end {
                                    let along = (x / track_length).clamp(0., 1.);
                                    let dot = Bounds::new(
                                        o + point(px(x - GRAIN_DOT / 2.), px(y - GRAIN_DOT / 2.)),
                                        size(px(GRAIN_DOT), px(GRAIN_DOT)),
                                    );
                                    window.paint_quad(
                                        fill(
                                            dot,
                                            at_alpha(
                                                crate::tokens::palette::PAPER,
                                                GRAIN_DOTS * along,
                                            ),
                                        )
                                        .corner_radii(gpui::Corners::all(px(GRAIN_DOT / 2.))),
                                    );
                                    x += GRAIN_PITCH;
                                }
                            }
                        }
                    }
                },
            )
            .absolute()
            .size_full();
            let track_top = layout::track_y(bar) - TRACK / 2.;
            let track = div()
                .absolute()
                .left(px(track_left))
                .top(px(track_top))
                .w(px(track_length))
                .h(px(TRACK))
                .rounded(px(TRACK / 2.))
                .overflow_hidden()
                .child(track)
                .child(
                    div()
                        .absolute()
                        .size_full()
                        .rounded(px(TRACK / 2.))
                        .border_1()
                        .border_color(at_alpha(WHITE, TRACK_RIM)),
                );
            let thumb = div()
                .absolute()
                .left(px(layout::thumb_x(value.unwrap_or(0.)) - THUMB / 2.))
                .top(px(layout::track_y(bar) - THUMB / 2.))
                .size(px(THUMB))
                .rounded_full()
                .bg(rgb(WHITE))
                .shadow(drop_shadow(THUMB_SHADOW, 2., 6.));
            let entity = cx.weak_entity();
            let stepper = move |by: f32, cx: &mut App| {
                entity
                    .update(cx, |picker, cx| picker.step(bar, by, cx))
                    .ok();
            };
            let spoken = div()
                .id(("theme-bar", bar as usize))
                .absolute()
                .left(px(track_left))
                .top(px(layout::track_y(bar) - THUMB / 2.))
                .w(px(track_length))
                .h(px(THUMB));
            let spoken = a11y.then(|| slider(spoken, bar.name(), value, stepper));
            // With no colour Intensity and Grain are dimmed and don't move (the spec's).
            div()
                .absolute()
                .size_full()
                .when(value.is_none(), |bar| bar.opacity(DIMMED))
                .child(label)
                .child(track)
                .child(thumb)
                .children(spoken)
        });

        let note_top = layout::note_top();
        let note = div()
            .absolute()
            .left(px(PADDING + LABEL_INSET))
            .top(px(note_top))
            .h(px(LABEL))
            .flex()
            .items_center()
            .text_size(px(LABEL_SIZE))
            .text_color(text_3)
            .child("For this Space only. Pages stay solid.");
        let reset = div()
            .id("theme-reset")
            .when(a11y, |reset| {
                reset
                    .role(Role::Button)
                    .aria_label("Reset")
                    .a11y_synthetic_children(|builder| {
                        builder.parent_node().add_action(Action::Click)
                    })
            })
            .absolute()
            .left(px(WIDTH - PADDING - RESET_WIDTH))
            .top(px(note_top))
            .w(px(RESET_WIDTH))
            .h(px(LABEL))
            .flex()
            .items_center()
            .justify_end()
            .pr(px(LABEL_INSET))
            .text_size(px(LABEL_SIZE))
            .text_color(text_2)
            .child("Reset");

        root.child(
            picker
                .child(card)
                .children(tint)
                .child(head)
                .child(segments)
                .children(segment_words)
                .child(palettes_label)
                .children(discs)
                .child(pad)
                .children(bars)
                .child(note)
                .child(reset)
                .children(keyed_ring),
        )
    }
}

#[cfg(test)]
mod tests;
