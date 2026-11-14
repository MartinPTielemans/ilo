//! A Boost's editor without code (#695), which the Boost card's Customize… opens: the theme
//! picker's card beside the sidebar, its arrow at what opened it, with the Boost's name over a
//! field of two dots, the page's ground and its links, then Invert lightness, the faces, the size
//! and the case, and Edit CSS…, Edit Script… and Zap at its foot. Each change shows on the site's pages at once;
//! a drag shows as it goes and is kept as it ends. What it sets is one block of the site's file
//! (crate::boost_style), which the user's own lines and Zap's leave alone.
//!
//! Like the theme picker, it's a window of its own that the host shows over the browser window, as
//! its child, and closes the same ways: Esc, or a click outside it. ⌘Z takes back its changes one
//! at a time. The name's menu holds Rename…, Reset All Edits and Delete Boost…. Advanced, at
//! Invert lightness's line's end, opens the page's contrast, brightness and saturation under it,
//! the card growing down, and stays open or closed for the Boost as the user leaves it. The look
//! and the layout are ours, as the owner approved the mock on 2026-11-10: Arc's editor isn't
//! recorded.
//!
//! It has no agent path: a Boost's CSS could carry a page's values out, which agents never get
//! (principle 3, decision 8).

use std::{rc::Rc, sync::Arc};

use gpui::{
    App, Bounds, Context, DispatchPhase, Entity, FocusHandle, Focusable, FontWeight, Global,
    ImageSource, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, RenderImage, SharedString, TitlebarOptions, Window, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions,
    accesskit::{Action, Role},
    actions, canvas, div, img, point,
    prelude::*,
    px, rgb, size, svg,
};
use image::{Frame, RgbaImage};

use crate::{
    boost_style::{
        self, ADJUST_STEP, Adjust, Case, Dot, Face, LARGEST, SIZE_STEP, SMALLEST, Style,
    },
    icon_picker,
    palette::{self, Palette},
    settings_rows::{self, Look},
    shell::KEY_CONTEXT,
    text_input::TextInput,
    theme_picker::{self, At},
};

actions!(boost_editor, [Dismiss, Commit, Undo, Decrease, Increase]);

/// The name's left edge, after the site's icon.
const NAME_LEFT: f32 = INSET + SITE_ICON + GAP;
/// Zap's chip, after Edit CSS… and Edit Script….
const ZAP_X: f32 = INSET + EDIT_WIDTH + GAP + SCRIPT_WIDTH + GAP;

pub use crate::tokens::boost_editor::HEIGHT;
use crate::tokens::boost_editor::{
    ADVANCED_HEIGHT, ADVANCED_WIDTH, BUTTON, CASE_Y, CHIP_PADDING, DIM, EDIT_WIDTH, FIELD_HEIGHT,
    FIELD_TOP, FONT_Y, FOOTER_Y, GAP, HEADER, HINT_RIGHT, HINT_TOP, INSET, INVERT_WIDTH, INVERT_Y,
    LABEL_WIDTH, MENU_LEFT, MENU_TOP, PLATE, PLATE_CHOSEN, PLATE_HOVERED, RESET_WIDTH, RULE, RULES,
    SCRIPT_WIDTH, SEGMENT, SEGMENT_HEIGHT, SEGMENT_PADDING, SITE_ICON, SIZE_Y, SLIDER_FILL_INK,
    SLIDER_PITCH, SLIDER_RING, SLIDER_TICK, SLIDER_TRACK_INK, SLIDER_VALUE, SLIDER_Y,
    STEPPER_VALUE, SWITCH_RIGHT, TILE_GAP, TILE_HEIGHT, TILE_RING, TILE_TEXT, TILES_TOP, ZAP_WIDTH,
};
use crate::tokens::settings::{SWITCH_HEIGHT, SWITCH_WIDTH};
use crate::tokens::shell::{MEDIA_SLIDER_KNOB, MEDIA_SLIDER_TRACK};
use crate::tokens::theme_picker::{
    CARD_WIDTH, DOT_ALPHA, DOT_PITCH, FIELD, FIELD_FILL, FIELD_INSET, FIELD_RADIUS, FIELD_RIM,
    HANDLE, HANDLE_RING, MAIN_HANDLE, MAIN_RING,
};
use crate::tokens::{menu, radius::CONTROL, text};

pub const WIDTH: f32 = theme_picker::WIDTH;
const HINT: &str = "Drag a dot: big for the ground, small for links";
/// Where the dots rest until the user places them, drawn as rings alone: the ground's toward the
/// field's light edge and the links' among the full colours (ours).
const REST: [Dot; 2] = [Dot { x: -0.15, y: 0.8 }, Dot { x: 0.45, y: -0.25 }];
/// Where the controls start along a line, after its label.
const CONTROL_X: f32 = INSET + LABEL_WIDTH + GAP;
/// Where they end.
const CONTROL_RIGHT: f32 = CARD_WIDTH - INSET;
/// Advanced's chip's left edge, at the controls' right edge, and Reset's, left of it.
const ADVANCED_X: f32 = CONTROL_RIGHT - ADVANCED_WIDTH;
const RESET_X: f32 = ADVANCED_X - GAP - RESET_WIDTH;
/// A slider's track's ends, where its knob's middle goes at its least and its most.
const TRACK: (f32, f32) = (CONTROL_X, CONTROL_RIGHT - SLIDER_VALUE);

/// What the editor asks of its window.
pub trait Host {
    /// Shows the editor's window with its top left at `left`, `top` in browser window `window`,
    /// with the keyboard, in the shape of `corners`, as the theme picker's host does.
    fn show(&self, window: u64, left: f32, top: f32, corners: &[(f32, f32, f32)]);
    /// Makes it `height` tall, keeping its top, in the shape of `corners`, as Advanced opens and
    /// closes.
    fn resize(&self, height: f32, corners: &[(f32, f32, f32)]);
    /// Hides it, and the browser window gets the keyboard back.
    fn hide(&self);
}

/// What the user asks of the site's Boost.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// Its edits, kept once the bool says so, and until then only shown, as during a drag.
    Style(Style, bool),
    /// Its switch.
    On(bool),
    EditCss,
    /// Edit Script…: the site's script in the user's editor (#2297).
    EditScript,
    Zap,
    Delete,
}

pub type Requested = Rc<dyn Fn(Request, &mut App)>;

/// The name's menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Item {
    Rename,
    Reset,
    Delete,
}

/// What a press in the card lands on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Name,
    Shuffle,
    Switch,
    /// A dot: the ground's 0 and the links' 1.
    Dot(usize),
    Field,
    Invert,
    /// A face's tile, the site's own none.
    Face(Option<Face>),
    Smaller,
    Larger,
    Case(Option<Case>),
    Advanced,
    /// Advanced's Reset, which puts its three back.
    ResetAdvanced,
    Slider(Adjust),
    EditCss,
    EditScript,
    Zap,
    Item(Item),
}

struct Editing {
    site: String,
    /// The site's icon, its page's favicon, or none for a globe.
    icon: Option<ImageSource>,
    style: Style,
    on: bool,
    requested: Requested,
    /// The styles before each kept change, for ⌘Z.
    undo: Vec<Style>,
    arrow_y: Option<f32>,
    /// Whether it opened to the left, its arrow on its right.
    left: bool,
    /// False once it's closing: what it shows stays, for the host's shrink out.
    open: bool,
    /// What's being dragged, and the style before the drag.
    drag: Option<(Drag, Style)>,
    /// The slider the arrow keys move, the last one pressed while Advanced is open.
    focused: Option<Adjust>,
    menu: bool,
    renaming: Option<Entity<TextInput>>,
    drawn: bool,
    palette: Palette,
}

/// What a drag moves.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    /// A dot, the ground's 0 and the links' 1, and where in it the pointer took it.
    Dot(usize, (f32, f32)),
    Slider(Adjust),
}

pub struct BoostEditor {
    host: Rc<dyn Host>,
    focus: FocusHandle,
    editing: Option<Editing>,
    /// The face each tile draws its sample in, found as the editor first shows.
    families: Option<Vec<(Option<Face>, SharedString)>>,
}

struct BoostEditorWindow(WindowHandle<BoostEditor>);

impl Global for BoostEditorWindow {}

/// How the app makes the editor's window and its host's overlay, the first time it's asked for:
/// unlike the pickers', not at launch, as few windows ever show it.
pub type Opener = Rc<dyn Fn(&mut App) -> Option<WindowHandle<BoostEditor>>>;

struct Lazy(Opener);

impl Global for Lazy {}

/// Has `opener` make the editor's window as it's first shown.
pub fn open_when_asked(opener: Opener, cx: &mut App) {
    cx.set_global(Lazy(opener));
}

/// Opens the editor's window, hidden, for `host` to show over the browser window.
pub fn open(host: Rc<dyn Host>, cx: &mut App) -> Option<WindowHandle<BoostEditor>> {
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(WIDTH), px(HEIGHT)));
    open_at(host, bounds, false, cx)
}

/// Opens the editor's window at `bounds`, shown if `show`, which the preview does.
pub fn open_at(
    host: Rc<dyn Host>,
    bounds: Bounds<Pixels>,
    show: bool,
    cx: &mut App,
) -> Option<WindowHandle<BoostEditor>> {
    cx.bind_keys([
        KeyBinding::new("escape", Dismiss, Some("BoostEditor")),
        KeyBinding::new("enter", Commit, Some("BoostEditor")),
        crate::shell::view_binding("cmd-z", Box::new(Undo), "BoostEditor", cx),
        KeyBinding::new("left", Decrease, Some("BoostEditor")),
        KeyBinding::new("down", Decrease, Some("BoostEditor")),
        KeyBinding::new("right", Increase, Some("BoostEditor")),
        KeyBinding::new("up", Increase, Some("BoostEditor")),
    ]);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
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
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let window = cx.open_window(options, |_, cx| {
        cx.new(|cx| BoostEditor {
            host,
            focus: cx.focus_handle(),
            editing: None,
            families: None,
        })
    });
    match window {
        Ok(window) => {
            cx.set_global(BoostEditorWindow(window));
            Some(window)
        }
        Err(error) => {
            log!("couldn't open the Boost editor's window: {error}");
            None
        }
    }
}

/// Shows the editor for `site`'s Boost, with its `icon`, `style` and `on`, `at` its place in browser window
/// `over`, to its left if `left`, `height` tall, drawn in `palette`, the Space's on a dark sheet as the theme picker's.
/// `requested` gets what the user asks of the Boost.
#[allow(clippy::too_many_arguments)]
pub fn show(
    over: u64,
    site: String,
    icon: Option<ImageSource>,
    style: Style,
    on: bool,
    at: At,
    left: bool,
    height: f32,
    palette: Palette,
    requested: Requested,
    cx: &mut App,
) {
    if !cx.has_global::<BoostEditorWindow>()
        && let Some(opener) = cx.try_global::<Lazy>().map(|lazy| lazy.0.clone())
    {
        opener(cx);
    }
    let Some(window) = cx.try_global::<BoostEditorWindow>().map(|editor| editor.0) else {
        return;
    };
    window
        .update(cx, |editor, window, cx| {
            // check.sh reads this line.
            log!("browser: boost editor for {site}");
            if editor.families.is_none() {
                editor.families = Some(families(&cx.text_system().all_font_names()));
            }
            let card = height_of(&style);
            let (x, top, arrow_y) = theme_picker::placed_of(at, left, height, card);
            editor.editing = Some(Editing {
                site,
                icon,
                style,
                on,
                requested,
                undo: Vec::new(),
                arrow_y,
                left,
                open: true,
                drag: None,
                focused: None,
                menu: false,
                renaming: None,
                drawn: false,
                palette,
            });
            window.focus(&editor.focus, cx);
            let corners = theme_picker::corners_of(arrow_y, left, card);
            // At its height before it shows, as Advanced was left open or closed.
            editor.host.resize(card, &corners);
            editor.host.show(over, x, top, &corners);
            cx.notify();
        })
        .ok();
}

/// The user left the editor, such as with a click outside it.
pub fn dismissed(cx: &mut App) {
    if let Some(window) = cx.try_global::<BoostEditorWindow>().map(|editor| editor.0) {
        window
            .update(cx, |editor, window, cx| editor.close(window, cx))
            .ok();
    }
}

/// The page it edits is going from under it: it closes as Esc closes it.
pub fn leave(cx: &mut App) {
    if let Some(window) = cx.try_global::<BoostEditorWindow>().map(|editor| editor.0) {
        window
            .update(cx, |editor, window, cx| editor.hide(window, cx))
            .ok();
    }
}

/// The site the editor shows for, if it shows.
pub fn open_for(cx: &App) -> Option<String> {
    let editor = cx.try_global::<BoostEditorWindow>()?.0.read(cx).ok()?;
    let editing = editor.editing.as_ref().filter(|editing| editing.open)?;
    Some(editing.site.clone())
}

pub fn is_open(cx: &App) -> bool {
    open_for(cx).is_some()
}

/// The faces the editor offers, the site's own first: those this computer has, so no tile shows
/// another's face in its place.
pub fn faces() -> Vec<Option<Face>> {
    std::iter::once(None)
        .chain(
            Face::ALL
                .into_iter()
                .filter(|face| face.resolves())
                .map(Some),
        )
        .collect()
}

/// The family each tile draws its sample in, of the computer's `fonts`: the first it has of each
/// face's, and the UI's otherwise.
fn families(fonts: &[String]) -> Vec<(Option<Face>, SharedString)> {
    let first = |names: &[&'static str]| -> &'static str {
        names
            .iter()
            .find(|name| fonts.iter().any(|font| font == *name))
            .map_or(crate::face::UI, |name| name)
    };
    faces()
        .into_iter()
        .map(|face| {
            let family = match face {
                None | Some(Face::Sans) => crate::face::UI,
                Some(Face::Serif) => first(&[
                    "New York",
                    "Georgia",
                    "Cambria",
                    "Noto Serif",
                    "DejaVu Serif",
                    "Liberation Serif",
                ]),
                Some(Face::Rounded) => first(&["SF Pro Rounded"]),
                Some(Face::Mono) => first(&[
                    "SF Mono",
                    "Menlo",
                    "Cascadia Mono",
                    "Consolas",
                    "Noto Sans Mono",
                    "DejaVu Sans Mono",
                    "Liberation Mono",
                ]),
            };
            (face, family.into())
        })
        .collect()
}

/// The card's height with `style`, taller while Advanced is open.
pub fn height_of(style: &Style) -> f32 {
    HEIGHT + below(style)
}

/// How far what's under Advanced's sliders moves down while they show.
fn below(style: &Style) -> f32 {
    if style.advanced { ADVANCED_HEIGHT } else { 0. }
}

/// A slider's line's middle.
fn slider_y(adjust: Adjust) -> f32 {
    SLIDER_Y + adjust as usize as f32 * SLIDER_PITCH
}

/// Where along its track `adjust`'s knob lies with `style`, from 0 to 1.
fn along(adjust: Adjust, style: &Style) -> f32 {
    let (least, most) = adjust.range();
    f32::from(adjust.of(style) - least) / f32::from(most - least)
}

/// The value of `adjust` with its knob at `x` in the card, before it's put on its step.
fn value_at(adjust: Adjust, x: f32) -> i32 {
    let (least, most) = adjust.range();
    let along = ((x - TRACK.0) / (TRACK.1 - TRACK.0)).clamp(0., 1.);
    (f32::from(least) + along * f32::from(most - least)).round() as i32
}

/// A tile's left edge and width, of `count`.
fn tile(index: usize, count: usize) -> (f32, f32) {
    let width = (CARD_WIDTH - 2. * INSET - TILE_GAP * (count as f32 - 1.)) / count as f32;
    (INSET + index as f32 * (width + TILE_GAP), width)
}

/// The left edges of Shuffle and the switch, at the header's right end.
fn header_buttons() -> (f32, f32) {
    let switch = CARD_WIDTH - SWITCH_RIGHT - SWITCH_WIDTH;
    (switch - GAP - BUTTON, switch)
}

/// A case segment's left edge, the site's own first.
fn segment_x(index: usize) -> f32 {
    CONTROL_X + SEGMENT_PADDING + index as f32 * (SEGMENT + SEGMENT_PADDING)
}

const CASES: [Option<Case>; 3] = [None, Some(Case::Lower), Some(Case::Upper)];
const ITEMS: [(Item, &str); 3] = [
    (Item::Rename, "Rename…"),
    (Item::Reset, "Reset All Edits"),
    (Item::Delete, "Delete Boost…"),
];

/// An item's top in the name's menu, with a line before Delete Boost…, as destructive items have.
fn item_top(index: usize) -> f32 {
    let line = if index == 2 { menu::LINE } else { 0. };
    MENU_TOP + menu::PADDING + index as f32 * menu::ROW + line
}

fn menu_height() -> f32 {
    item_top(2) + menu::ROW + menu::PADDING - MENU_TOP
}

/// Where `dot` lies in the card.
fn in_card(dot: Dot) -> (f32, f32) {
    (
        FIELD_INSET + (dot.x + 1.) / 2. * FIELD,
        FIELD_TOP + (dot.y + 1.) / 2. * FIELD_HEIGHT,
    )
}

/// The field's point at `x`, `y` in the card, held inside it.
fn field_dot(x: f32, y: f32) -> Dot {
    Dot {
        x: ((x - FIELD_INSET) / FIELD * 2. - 1.).clamp(-1., 1.),
        y: ((y - FIELD_TOP) / FIELD_HEIGHT * 2. - 1.).clamp(-1., 1.),
    }
}

/// A dot as `style` has it, or where it rests until it's placed.
fn dot_of(style: &Style, index: usize) -> Option<Dot> {
    if index == 0 {
        style.ground
    } else {
        style.links
    }
}

fn handle_size(index: usize) -> f32 {
    if index == 0 { MAIN_HANDLE } else { HANDLE }
}

/// What a press at `x`, `y` from the card's top left lands on, with `style` in the field and the
/// name's menu open if `menu`, which takes the press wherever it lands.
pub fn target(x: f32, y: f32, style: &Style, menu: bool) -> Option<Target> {
    let within = |left: f32, width: f32, middle: f32, height: f32| {
        (left..left + width).contains(&x) && (y - middle).abs() <= height / 2.
    };
    if menu {
        let inside = (MENU_LEFT..MENU_LEFT + menu::MIN_WIDTH).contains(&x);
        return ITEMS.iter().enumerate().find_map(|(index, &(item, _))| {
            let top = item_top(index);
            (inside && (top..top + menu::ROW).contains(&y)).then_some(Target::Item(item))
        });
    }
    let (shuffle, switch) = header_buttons();
    if y < HEADER {
        let middle = HEADER / 2.;
        return if within(switch, SWITCH_WIDTH, middle, BUTTON) {
            Some(Target::Switch)
        } else if within(shuffle, BUTTON, middle, BUTTON) {
            Some(Target::Shuffle)
        } else if within(NAME_LEFT - GAP, shuffle - NAME_LEFT, middle, BUTTON) {
            Some(Target::Name)
        } else {
            None
        };
    }
    let in_field = (FIELD_INSET..FIELD_INSET + FIELD).contains(&x)
        && (FIELD_TOP..FIELD_TOP + FIELD_HEIGHT).contains(&y);
    if in_field {
        // The links' dot first, as it's drawn over the ground's.
        let dot = [1, 0].into_iter().find(|&index| {
            let at = in_card(dot_of(style, index).unwrap_or(REST[index]));
            (x - at.0).hypot(y - at.1) <= handle_size(index) / 2.
        });
        return Some(dot.map_or(Target::Field, Target::Dot));
    }
    if within(INSET, INVERT_WIDTH, INVERT_Y, BUTTON) {
        return Some(Target::Invert);
    }
    if within(ADVANCED_X, ADVANCED_WIDTH, INVERT_Y, BUTTON) {
        return Some(Target::Advanced);
    }
    let below = below(style);
    if style.advanced {
        if within(RESET_X, RESET_WIDTH, INVERT_Y, BUTTON) {
            return Some(Target::ResetAdvanced);
        }
        // The track and the knob past its ends, a line tall, so a press needn't hit 4 points.
        let (left, right) = (
            TRACK.0 - MEDIA_SLIDER_KNOB / 2.,
            TRACK.1 + MEDIA_SLIDER_KNOB / 2.,
        );
        let slider = Adjust::ALL
            .into_iter()
            .find(|&adjust| within(left, right - left, slider_y(adjust), SLIDER_PITCH));
        if let Some(adjust) = slider {
            return Some(Target::Slider(adjust));
        }
    }
    let tiles_top = TILES_TOP + below;
    if (tiles_top..tiles_top + TILE_HEIGHT).contains(&y) {
        let faces = faces();
        return faces.iter().enumerate().find_map(|(index, &face)| {
            let (left, width) = tile(index, faces.len());
            (left..left + width)
                .contains(&x)
                .then_some(Target::Face(face))
        });
    }
    if within(CONTROL_X, BUTTON, SIZE_Y + below, BUTTON) {
        return Some(Target::Smaller);
    }
    if within(
        CONTROL_X + BUTTON + STEPPER_VALUE,
        BUTTON,
        SIZE_Y + below,
        BUTTON,
    ) {
        return Some(Target::Larger);
    }
    if let Some(index) = (0..CASES.len()).find(|&index| {
        within(
            segment_x(index),
            SEGMENT,
            CASE_Y + below,
            SEGMENT_HEIGHT + 2. * SEGMENT_PADDING,
        )
    }) {
        return Some(Target::Case(CASES[index]));
    }
    if within(INSET, EDIT_WIDTH, FOOTER_Y + below, BUTTON) {
        return Some(Target::EditCss);
    }
    if within(
        INSET + EDIT_WIDTH + GAP,
        SCRIPT_WIDTH,
        FOOTER_Y + below,
        BUTTON,
    ) {
        return Some(Target::EditScript);
    }
    if within(ZAP_X, ZAP_WIDTH, FOOTER_Y + below, BUTTON) {
        return Some(Target::Zap);
    }
    None
}

impl BoostEditor {
    /// Where the card starts across the editor's window, as it opened.
    fn card_x(&self) -> f32 {
        theme_picker::card_x(self.editing.as_ref().is_some_and(|editing| editing.left))
    }

    /// The host closed it, as a click outside does.
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut().filter(|editing| editing.open) else {
            return;
        };
        editing.open = false;
        editing.menu = false;
        // A rename typed and left by a click outside is kept, as a field's is.
        self.commit_rename(window, cx);
        self.release(cx);
        cx.notify();
    }

    /// Closes it and asks the host to hide it.
    fn hide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.as_ref().is_some_and(|editing| editing.open) {
            self.close(window, cx);
            self.host.hide();
        }
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut().filter(|editing| editing.open) else {
            return;
        };
        if editing.renaming.take().is_some() {
            window.focus(&self.focus, cx);
        } else if editing.menu {
            editing.menu = false;
        } else {
            // check.sh reads this line.
            log!("browser: boost editor dismissed");
            self.hide(window, cx);
        }
        cx.notify();
    }

    fn commit(&mut self, _: &Commit, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_rename(window, cx);
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut().filter(|editing| editing.open) else {
            return;
        };
        let Some(mut before) = editing.undo.pop() else {
            return;
        };
        // The hint, once gone, stays gone, and Advanced stays as it's shown: opening it isn't an
        // edit.
        before.dragged |= editing.style.dragged;
        before.advanced = editing.style.advanced;
        // check.sh reads this line.
        log!("browser: boost editor undo");
        self.set(before, true, cx);
    }

    /// Shows `style`, and hands it on: kept if `kept`.
    fn set(&mut self, style: Style, kept: bool, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        editing.style = style.clone();
        if kept {
            // check.sh reads this line.
            log!(
                "browser: boost editor set {} {}",
                editing.site,
                style.state()
            );
        }
        (editing.requested.clone())(Request::Style(style, kept), cx);
        cx.notify();
    }

    /// Makes `change` to the style and keeps it, as one step for ⌘Z.
    fn keep(&mut self, change: impl FnOnce(&mut Style), cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        let before = editing.style.clone();
        let mut after = before.clone();
        change(&mut after);
        if after != before {
            editing.undo.push(before);
            self.set(after, true, cx);
        }
    }

    /// Hides the editor and asks for `request`, which goes on in the browser window.
    fn leave_for(&mut self, request: Request, window: &mut Window, cx: &mut Context<Self>) {
        let Some(requested) = self
            .editing
            .as_ref()
            .map(|editing| editing.requested.clone())
        else {
            return;
        };
        self.hide(window, cx);
        requested(request, cx);
    }

    fn press(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let (x, y) = (
            f32::from(event.position.x) - self.card_x(),
            f32::from(event.position.y),
        );
        let Some(editing) = self.editing.as_mut().filter(|editing| editing.open) else {
            return;
        };
        if editing.renaming.is_some() {
            // The field takes its own presses; one anywhere else keeps the name.
            if y < HEADER && x < header_buttons().0 {
                return;
            }
            self.commit_rename(window, cx);
        }
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        let target = target(x, y, &editing.style, editing.menu);
        if editing.menu {
            // A press anywhere closes the menu, and on an item chooses it.
            editing.menu = false;
            if let Some(Target::Item(item)) = target {
                self.choose(item, window, cx);
            }
            cx.notify();
            return;
        }
        // The arrow keys move the slider last pressed, until a press lands elsewhere.
        editing.focused = match target {
            Some(Target::Slider(adjust)) => Some(adjust),
            _ => None,
        };
        let Some(target) = target else {
            cx.notify();
            return;
        };
        match target {
            Target::Name => editing.menu = true,
            Target::Shuffle => {
                let seed = web_time::SystemTime::now()
                    .duration_since(web_time::UNIX_EPOCH)
                    .map_or(0, |since| since.as_nanos() as u64);
                self.keep(|style| *style = style.shuffled(seed), cx);
            }
            Target::Switch => {
                editing.on = !editing.on;
                let (on, requested) = (editing.on, editing.requested.clone());
                requested(Request::On(on), cx);
            }
            Target::Dot(index) => {
                let at = in_card(dot_of(&editing.style, index).unwrap_or(REST[index]));
                let grab = (x - at.0, y - at.1);
                editing.drag = Some((Drag::Dot(index, grab), editing.style.clone()));
                self.drag_to(x, y, cx);
            }
            // A press on the field places the ground there, and drags it from there.
            Target::Field => {
                editing.drag = Some((Drag::Dot(0, (0., 0.)), editing.style.clone()));
                self.drag_to(x, y, cx);
            }
            // A press on a track puts the knob there, and drags it from there.
            Target::Slider(adjust) => {
                editing.drag = Some((Drag::Slider(adjust), editing.style.clone()));
                self.drag_to(x, y, cx);
            }
            Target::Advanced => {
                let mut style = editing.style.clone();
                style.advanced = !style.advanced;
                // check.sh reads this line.
                log!("browser: boost editor advanced {}", style.advanced);
                let corners =
                    theme_picker::corners_of(editing.arrow_y, editing.left, height_of(&style));
                self.host.resize(height_of(&style), &corners);
                self.set(style, true, cx);
            }
            Target::ResetAdvanced => self.keep(|style| style.adjust = [100; 3], cx),
            Target::Invert => self.keep(|style| style.invert = !style.invert, cx),
            Target::Face(face) => self.keep(|style| style.face = face, cx),
            Target::Smaller => self.keep(
                |style| style.size = style.size.saturating_sub(SIZE_STEP).max(SMALLEST),
                cx,
            ),
            Target::Larger => self.keep(
                |style| style.size = (style.size + SIZE_STEP).min(LARGEST),
                cx,
            ),
            Target::Case(case) => self.keep(|style| style.case = case, cx),
            Target::EditCss => self.leave_for(Request::EditCss, window, cx),
            Target::EditScript => self.leave_for(Request::EditScript, window, cx),
            Target::Zap => self.leave_for(Request::Zap, window, cx),
            Target::Item(_) => {}
        }
        cx.notify();
    }

    fn choose(&mut self, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        match item {
            Item::Rename => {
                let name: SharedString = editing
                    .style
                    .name
                    .clone()
                    .unwrap_or_else(|| editing.site.clone())
                    .into();
                let palette = editing.palette;
                let input = cx.new(|cx| {
                    TextInput::new("Boost name", cx)
                        .placeholder_color(palette.on_overlay(DIM))
                        .spoken("Boost name")
                });
                input.update(cx, |input, cx| input.reset(name, window, cx));
                window.focus(&input.focus_handle(cx), cx);
                // Or the press that chose it gives the editor the keyboard back.
                window.prevent_default();
                editing.renaming = Some(input);
            }
            Item::Reset => self.keep(
                |style| {
                    *style = Style {
                        dragged: style.dragged,
                        advanced: style.advanced,
                        name: style.name.clone(),
                        ..Style::default()
                    }
                },
                cx,
            ),
            Item::Delete => self.leave_for(Request::Delete, window, cx),
        }
    }

    fn commit_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = self
            .editing
            .as_mut()
            .and_then(|editing| editing.renaming.take())
        else {
            return;
        };
        let typed = input.read(cx);
        // A name still being composed in an input method is kept as it's shown.
        let typed = if typed.composing() {
            typed.text().to_owned()
        } else {
            typed.committed()
        };
        let site = self.editing.as_ref().map(|editing| editing.site.clone());
        let name = boost_style::named(&typed).filter(|name| Some(name) != site.as_ref());
        log!(
            "browser: boost editor renamed {}",
            name.as_deref().unwrap_or("-")
        );
        self.keep(|style| style.name = name, cx);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Moves what's dragged to the pointer at `x`, `y` in the card, shown and not yet kept.
    fn drag_to(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        let Some((drag, _)) = editing.drag else {
            return;
        };
        let mut style = editing.style.clone();
        match drag {
            Drag::Dot(index, grab) => {
                let dot = field_dot(x - grab.0, y - grab.1);
                match index {
                    0 => style.ground = Some(dot),
                    _ => style.links = Some(dot),
                }
                style.dragged = true;
            }
            Drag::Slider(adjust) => adjust.set(&mut style, value_at(adjust, x)),
        }
        if style != editing.style {
            self.set(style, false, cx);
        }
    }

    /// Moves the focused slider a step by `steps`, as an arrow key does.
    fn step(&mut self, steps: i32, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_ref().filter(|editing| editing.open) else {
            return;
        };
        let Some(adjust) = editing.focused.filter(|_| editing.style.advanced) else {
            return;
        };
        if editing.renaming.is_some() || editing.drag.is_some() {
            return;
        }
        self.slide(adjust, steps, cx);
    }

    /// Moves `adjust` by `steps`, kept as one step for ⌘Z.
    fn slide(&mut self, adjust: Adjust, steps: i32, cx: &mut Context<Self>) {
        self.keep(
            |style| {
                let value = i32::from(adjust.of(style)) + steps * i32::from(ADJUST_STEP);
                adjust.set(style, value)
            },
            cx,
        );
    }

    fn decrease(&mut self, _: &Decrease, _: &mut Window, cx: &mut Context<Self>) {
        self.step(-1, cx);
    }

    fn increase(&mut self, _: &Increase, _: &mut Window, cx: &mut Context<Self>) {
        self.step(1, cx);
    }

    fn moved(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let (x, y) = (f32::from(position.x) - self.card_x(), f32::from(position.y));
        self.drag_to(x, y, cx);
    }

    /// The drag ends, and what it made is kept, as one step for ⌘Z.
    fn release(&mut self, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.as_mut() else {
            return;
        };
        let Some((_, before)) = editing.drag.take() else {
            return;
        };
        if editing.style != before {
            editing.undo.push(before);
            let style = editing.style.clone();
            self.set(style, true, cx);
        }
    }
}

thread_local! {
    static DOTS: std::cell::OnceCell<Arc<RenderImage>> = const { std::cell::OnceCell::new() };
}

/// The field's dot grid, the theme picker's, drawn once at 2×.
fn dots_image() -> Arc<RenderImage> {
    DOTS.with(|dots| {
        dots.get_or_init(|| {
            let (width, height) = ((FIELD * 2.) as usize, (FIELD_HEIGHT * 2.) as usize);
            let mut data = vec![0u8; width * height * 4];
            let (middle_x, middle_y) = (FIELD / 2., FIELD_HEIGHT / 2.);
            let (columns, rows) = ((middle_x / DOT_PITCH) as i32, (middle_y / DOT_PITCH) as i32);
            for row in -rows..=rows {
                for column in -columns..=columns {
                    let x = ((middle_x + column as f32 * DOT_PITCH) * 2.).round() as usize;
                    let y = ((middle_y + row as f32 * DOT_PITCH) * 2.).round() as usize;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let (x, y) = ((x + dx).saturating_sub(1), (y + dy).saturating_sub(1));
                        if x >= width || y >= height {
                            continue;
                        }
                        let at = (y * width + x) * 4;
                        let alpha = (DOT_ALPHA * 255.).round() as u8;
                        data[at..at + 4].copy_from_slice(&[255, 255, 255, alpha]);
                    }
                }
            }
            let image =
                RgbaImage::from_raw(width as u32, height as u32, data).expect("sized to fit");
            Arc::new(RenderImage::new(vec![Frame::new(image)]))
        })
        .clone()
    })
}

impl Focusable for BoostEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for BoostEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div()
            .key_context(KEY_CONTEXT)
            .size_full()
            .font_family(crate::face::UI);
        // The pointer's moves anywhere in the window and its release, in every frame, as the theme
        // picker's: a press and its first move or its release can come before the next frame
        // (#2605).
        let entity = cx.weak_entity();
        let listener = canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                let moved = entity.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture {
                        moved
                            .update(cx, |editor, cx| editor.moved(event.position, cx))
                            .ok();
                    }
                });
                let released = entity.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                        released.update(cx, |editor, cx| editor.release(cx)).ok();
                    }
                });
            },
        )
        .absolute()
        .size_0();
        let a11y = window.is_a11y_active();
        let site = (self.editing.as_ref()).map_or(String::new(), |editing| editing.site.clone());
        let editor = div()
            .id("boost-editor")
            .when(a11y, |editor| {
                editor
                    .role(Role::Dialog)
                    .aria_label(format!("Boost for {site}"))
            })
            .key_context("BoostEditor")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::dismiss))
            .on_action(cx.listener(Self::commit))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::decrease))
            .on_action(cx.listener(Self::increase))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, event: &MouseDownEvent, window, cx| {
                    editor.press(event, window, cx)
                }),
            )
            .relative()
            .size_full()
            .child(listener);
        let families = self.families.clone().unwrap_or_default();
        let Some(editing) = &mut self.editing else {
            return root.child(editor);
        };
        // AppKit puts the window on screen some frames after it's asked to, and only then does
        // GPUI draw it.
        if editing.open && !editing.drawn {
            if window.is_visible() {
                editing.drawn = true;
                // check.sh waits for this line.
                log!("browser: boost editor drawn");
            } else {
                window.request_animation_frame();
            }
        }
        let editing = &*editing;
        let style = &editing.style;
        let palette = editing.palette;
        let (arrow_y, left) = (editing.arrow_y, editing.left);
        let card_left = theme_picker::card_x(left);
        let card_x = |x: f32| px(card_left + x);
        let card_fill = palette::current(cx).card_drawn();
        // One card, one shape and one fill, at the height Advanced gives it.
        let card_height = height_of(style);
        let below = below(style);
        let card = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let corners = theme_picker::corners_of(arrow_y, left, card_height);
                icon_picker::paint_card(bounds.origin, &corners, card_fill, window)
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let ink = |alpha: f32| palette.on_overlay(alpha);
        let dim = ink(DIM);
        let plate = |chosen: bool| palette.plate(if chosen { PLATE_CHOSEN } else { PLATE });
        let hovered = palette.plate(PLATE_HOVERED);
        let at = |x: f32, y: f32, width: f32, height: f32| {
            div()
                .absolute()
                .left(card_x(x))
                .top(px(y - height / 2.))
                .w(px(width))
                .h(px(height))
        };
        let label = |name: &'static str, y: f32| {
            at(INSET, y + below, LABEL_WIDTH, BUTTON)
                .flex()
                .items_center()
                .text_size(px(text::SMALL))
                .text_color(dim)
                .child(name)
        };

        // The header: the name and its menu, Shuffle and the switch.
        let (shuffle_x, switch_x) = header_buttons();
        let name = style.name.clone().unwrap_or_else(|| editing.site.clone());
        let name_width = shuffle_x - NAME_LEFT;
        let icon = at(INSET, HEADER / 2., SITE_ICON, SITE_ICON).child(match editing.icon.clone() {
            Some(icon) => img(icon).size(px(SITE_ICON)).into_any_element(),
            None => svg()
                .path("icons/globe.svg")
                .size(px(SITE_ICON))
                .text_color(dim)
                .into_any_element(),
        });
        let title = match &editing.renaming {
            Some(input) => at(NAME_LEFT - GAP / 2., HEADER / 2., name_width, BUTTON)
                .flex()
                .items_center()
                .px(px(GAP / 2.))
                .rounded(px(CONTROL))
                .bg(plate(true))
                .text_size(px(text::TITLE))
                .text_color(palette.ink)
                .child(input.clone())
                .into_any_element(),
            None => at(NAME_LEFT - GAP / 2., HEADER / 2., name_width, BUTTON)
                .flex()
                .items_center()
                .child(
                    div()
                        .id("boost-name")
                        .when(a11y, |name| {
                            name.role(Role::Button).aria_label("Boost name")
                        })
                        .flex()
                        .items_center()
                        .gap(px(GAP / 2.))
                        .px(px(GAP / 2.))
                        .h(px(BUTTON))
                        .max_w_full()
                        .rounded(px(CONTROL))
                        .when(editing.menu, |name| name.bg(hovered))
                        .hover(move |name| name.bg(hovered))
                        .text_size(px(text::TITLE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(palette.ink)
                        .child(div().truncate().child(name))
                        .child(
                            svg()
                                .path("icons/chevron-down.svg")
                                .flex_none()
                                .size(px(text::SMALL))
                                .text_color(dim),
                        ),
                )
                .into_any_element(),
        };
        let shuffle = at(shuffle_x, HEADER / 2., BUTTON, BUTTON)
            .id("boost-shuffle")
            .when(a11y, |button| {
                button.role(Role::Button).aria_label("Shuffle")
            })
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(CONTROL))
            .hover(move |button| button.bg(hovered))
            .child(
                svg()
                    .path("icons/shuffle.svg")
                    .size(px(text::TITLE))
                    .text_color(palette.on_overlay_glyph(1.)),
            );
        let look = Look::on(palette.overlay, palette.ink, palette.accent);
        let switch = at(switch_x, HEADER / 2., SWITCH_WIDTH, SWITCH_HEIGHT)
            .id("boost-on")
            .when(a11y, |switch| {
                switch.role(Role::Switch).aria_label("Boost on")
            })
            .child(settings_rows::switch(&look, editing.on, 1., palette.accent));

        // The field, its dots and the hint.
        let dots = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                window
                    .paint_image(
                        bounds,
                        bounds,
                        px(FIELD_RADIUS).into(),
                        dots_image(),
                        0,
                        false,
                    )
                    .ok();
            },
        )
        .absolute()
        .size_full();
        let mut field = div()
            .id("boost-field")
            .when(a11y, |field| field.role(Role::Group).aria_label("Colors"))
            .absolute()
            .left(card_x(FIELD_INSET))
            .top(px(FIELD_TOP))
            .w(px(FIELD))
            .h(px(FIELD_HEIGHT))
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
        if !style.dragged {
            field = field.child(
                div()
                    .absolute()
                    .top(px(HINT_TOP))
                    .right(px(HINT_RIGHT))
                    .text_size(px(text::SMALL))
                    .text_color(dim)
                    .child(HINT),
            );
        }
        // The ground's dot, then the links' over it. One not yet placed is its ring alone.
        for index in [0, 1] {
            let dot = dot_of(style, index);
            let (x, y) = in_card(dot.unwrap_or(REST[index]));
            let size = handle_size(index);
            let ring = if index == 0 { MAIN_RING } else { HANDLE_RING };
            field = field.child(
                div()
                    .absolute()
                    .left(px(x - FIELD_INSET - size / 2.))
                    .top(px(y - FIELD_TOP - size / 2.))
                    .size(px(size))
                    .rounded_full()
                    .when_some(dot, |handle, dot| handle.bg(rgb(dot.color())))
                    .border(px(ring))
                    .border_color(palette.ink),
            );
        }

        // Invert lightness.
        let invert = at(INSET, INVERT_Y, INVERT_WIDTH, BUTTON)
            .id("boost-invert")
            .when(a11y, |chip| {
                chip.role(Role::Switch).aria_label("Invert lightness")
            })
            .flex()
            .items_center()
            .gap(px(GAP / 2.))
            .px(px(CHIP_PADDING))
            .rounded(px(CONTROL))
            .text_size(px(text::BODY))
            .map(|chip| match style.invert {
                true => chip.bg(palette.accent).text_color(palette.accent_label),
                false => chip
                    .bg(plate(false))
                    .text_color(palette.ink)
                    .hover(move |chip| chip.bg(hovered)),
            })
            .child(
                svg()
                    .path("icons/contrast.svg")
                    .size(px(text::BODY))
                    .text_color(if style.invert {
                        palette.accent_label
                    } else {
                        palette.ink
                    }),
            )
            .child("Invert lightness");
        let rules = RULES
            .map(|y| at(INSET, y + below, CARD_WIDTH - 2. * INSET, 1.).bg(palette.plate(RULE)));

        // Advanced, at the line's end, and while it's open Reset beside it and its sliders.
        let advanced = at(ADVANCED_X, INVERT_Y, ADVANCED_WIDTH, BUTTON)
            .id("boost-advanced")
            .when(a11y, |chip| {
                chip.role(Role::Button)
                    .aria_label("Advanced")
                    .aria_expanded(style.advanced)
            })
            .flex()
            .items_center()
            .justify_center()
            .gap(px(GAP / 2.))
            .rounded(px(CONTROL))
            .bg(plate(style.advanced))
            .hover(move |chip| chip.bg(hovered))
            .text_size(px(text::BODY))
            .text_color(palette.ink)
            .child("Advanced")
            .child(
                svg()
                    .path(if style.advanced {
                        "icons/chevron-up.svg"
                    } else {
                        "icons/chevron-down.svg"
                    })
                    .flex_none()
                    .size(px(text::SMALL))
                    .text_color(dim),
            );
        let untouched = style.adjust == [100; 3];
        let reset = style.advanced.then(|| {
            at(RESET_X, INVERT_Y, RESET_WIDTH, BUTTON)
                .id("boost-advanced-reset")
                .when(a11y, |chip| {
                    chip.role(Role::Button)
                        .aria_label("Reset Contrast, Brightness and Saturation")
                        .when(untouched, |chip| {
                            chip.a11y_synthetic_children(|builder| {
                                builder.parent_node().set_disabled()
                            })
                        })
                })
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(CONTROL))
                .bg(plate(false))
                .when(!untouched, |chip| chip.hover(move |chip| chip.bg(hovered)))
                .text_size(px(text::BODY))
                .text_color(if untouched { dim } else { palette.ink.into() })
                .child("Reset")
        });
        let sliders = (style.advanced)
            .then(|| {
                Adjust::ALL.map(|adjust| {
                    slider_line(
                        adjust,
                        style,
                        editing.focused == Some(adjust),
                        &palette,
                        a11y,
                        card_left,
                        cx.weak_entity(),
                    )
                })
            })
            .into_iter()
            .flatten()
            .flatten();

        // The faces, each in its own.
        let count = families.len();
        let tiles = families
            .into_iter()
            .enumerate()
            .map(|(index, (face, family))| {
                let (left, width) = tile(index, count);
                let chosen = style.face == face;
                let title = face.map_or("Site's", Face::title);
                at(
                    left,
                    TILES_TOP + below + TILE_HEIGHT / 2.,
                    width,
                    TILE_HEIGHT,
                )
                .id(("boost-face", index))
                .when(a11y, |tile| tile.role(Role::RadioButton).aria_label(title))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .rounded(px(CONTROL))
                .bg(plate(chosen))
                .when(chosen, |tile| {
                    tile.border(px(TILE_RING)).border_color(palette.accent)
                })
                .when(!chosen, |tile| tile.hover(move |tile| tile.bg(hovered)))
                .child(
                    div()
                        .font_family(family)
                        .text_size(px(TILE_TEXT))
                        .text_color(palette.ink)
                        .child("Aa"),
                )
                .child(
                    div()
                        .text_size(px(text::SMALL))
                        .text_color(dim)
                        .child(title),
                )
            });

        // The size and the case.
        let step = |id: &'static str, glyph: &'static str, x: f32, enabled: bool| {
            at(x, SIZE_Y + below, BUTTON, BUTTON)
                .id(id)
                .when(a11y, |button| button.role(Role::Button).aria_label(id))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(CONTROL))
                .when(enabled, |button| {
                    button.hover(move |button| button.bg(hovered))
                })
                .child(
                    svg()
                        .path(glyph)
                        .size(px(text::BODY))
                        .text_color(if enabled {
                            palette.on_overlay_glyph(1.)
                        } else {
                            palette.faint(PLATE_CHOSEN)
                        }),
                )
        };
        let stepper_width = 2. * BUTTON + STEPPER_VALUE;
        let stepper = at(CONTROL_X, SIZE_Y + below, stepper_width, BUTTON)
            .rounded(px(CONTROL))
            .bg(plate(false));
        let size_value = at(CONTROL_X + BUTTON, SIZE_Y + below, STEPPER_VALUE, BUTTON)
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(text::BODY))
            .text_color(palette.ink)
            .child(format!("{} %", style.size));
        let smaller = step(
            "Smaller",
            "icons/minus.svg",
            CONTROL_X,
            style.size > SMALLEST,
        );
        let larger = step(
            "Larger",
            "icons/plus.svg",
            CONTROL_X + BUTTON + STEPPER_VALUE,
            style.size < LARGEST,
        );
        let segments_width = 3. * SEGMENT + 4. * SEGMENT_PADDING;
        let segments = at(
            CONTROL_X,
            CASE_Y + below,
            segments_width,
            SEGMENT_HEIGHT + 2. * SEGMENT_PADDING,
        )
        .rounded(px(CONTROL))
        .bg(plate(false));
        let cases = CASES.iter().enumerate().map(|(index, &case)| {
            let (sample, name) = match case {
                None => ("Aa", "As the site has it"),
                Some(Case::Lower) => ("aa", "Lowercase"),
                Some(Case::Upper) => ("AA", "Uppercase"),
            };
            let chosen = style.case == case;
            at(segment_x(index), CASE_Y + below, SEGMENT, SEGMENT_HEIGHT)
                .id(("boost-case", index))
                .when(a11y, |segment| {
                    segment.role(Role::RadioButton).aria_label(name)
                })
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(CONTROL - SEGMENT_PADDING))
                .text_size(px(text::BODY))
                .text_color(palette.ink)
                .when(chosen, |segment| {
                    segment.bg(plate(true)).font_weight(FontWeight::MEDIUM)
                })
                .when(!chosen, |segment| {
                    segment.hover(move |segment| segment.bg(hovered))
                })
                .child(sample)
        });

        // The footer: what the card under the URL field also offers.
        let chip = |id: &'static str, label: &'static str, x: f32, width: f32| {
            at(x, FOOTER_Y + below, width, BUTTON)
                .id(id)
                .when(a11y, |chip| chip.role(Role::Button).aria_label(label))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(CONTROL))
                .bg(plate(false))
                .hover(move |chip| chip.bg(hovered))
                .text_size(px(text::BODY))
                .text_color(palette.ink)
                .child(label)
        };
        let edit = chip("boost-editor-css", "Edit CSS…", INSET, EDIT_WIDTH);
        let script = chip(
            "boost-editor-script",
            "Edit Script…",
            INSET + EDIT_WIDTH + GAP,
            SCRIPT_WIDTH,
        );
        let zap = chip("boost-editor-zap", "Zap", ZAP_X, ZAP_WIDTH);

        // The name's menu, ilo's own menu's look, inside the card.
        let menu = editing.menu.then(|| {
            let tone = palette::current(cx).tone;
            let (fill, rim, label, destructive) = match tone {
                palette::Tone::Dark => (
                    menu::DARK_FILL,
                    menu::DARK_RIM,
                    menu::DARK_LABEL,
                    menu::DARK_DESTRUCTIVE,
                ),
                palette::Tone::Light => (
                    menu::LIGHT_FILL,
                    menu::LIGHT_RIM,
                    menu::LIGHT_LABEL,
                    menu::LIGHT_DESTRUCTIVE,
                ),
            };
            let accent = palette.accent;
            let accent_label = palette.accent_label;
            let items = ITEMS
                .iter()
                .enumerate()
                .map(move |(index, &(item, title))| {
                    let delete = item == Item::Delete;
                    div()
                        .id(("boost-menu", index))
                        .when(a11y, |row| row.role(Role::MenuItem).aria_label(title))
                        .absolute()
                        .left(px(menu::PADDING))
                        .right(px(menu::PADDING))
                        .top(px(item_top(index) - MENU_TOP))
                        .h(px(menu::ROW))
                        .px(px(menu::ROW_X))
                        .flex()
                        .items_center()
                        .rounded(px(menu::ROW_RADIUS))
                        .text_size(px(menu::TEXT))
                        .text_color(gpui::rgba(if delete { destructive } else { label }))
                        .hover(move |row| match delete {
                            true => row
                                .bg(gpui::rgba(menu::DESTRUCTIVE_FILL))
                                .text_color(gpui::rgba(menu::DESTRUCTIVE_LABEL)),
                            false => row.bg(accent).text_color(accent_label),
                        })
                        .child(title)
                });
            div()
                .id("boost-menu")
                .when(a11y, |menu| menu.role(Role::Menu))
                .absolute()
                .left(card_x(MENU_LEFT))
                .top(px(MENU_TOP))
                .w(px(menu::MIN_WIDTH))
                .h(px(menu_height()))
                .rounded(px(menu::RADIUS))
                .bg(gpui::rgba(fill))
                .border_1()
                .border_color(gpui::rgba(rim))
                .child(
                    div()
                        .absolute()
                        .left(px(menu::ROW_X))
                        .right(px(menu::ROW_X))
                        .top(px(item_top(2) - MENU_TOP - menu::LINE / 2.))
                        .h(px(1.))
                        .bg(gpui::rgba(rim)),
                )
                .children(items)
        });

        root.child(
            editor
                .child(card)
                .child(icon)
                .child(title)
                .child(shuffle)
                .child(switch)
                .child(field)
                .child(invert)
                .child(advanced)
                .children(reset)
                .children(sliders)
                .children(rules)
                .child(label("Font", FONT_Y))
                .children(tiles)
                .child(label("Size", SIZE_Y))
                .child(stepper)
                .child(size_value)
                .child(smaller)
                .child(larger)
                .child(label("Case", CASE_Y))
                .child(segments)
                .children(cases)
                .child(edit)
                .child(script)
                .child(zap)
                .children(menu),
        )
    }
}

/// One of Advanced's lines: its label, its slider filled from 100 % in the middle to the knob,
/// and its value. Its knob has the accent's ring while the arrow keys move it.
fn slider_line(
    adjust: Adjust,
    style: &Style,
    focused: bool,
    palette: &Palette,
    a11y: bool,
    card_left: f32,
    editor: gpui::WeakEntity<BoostEditor>,
) -> Vec<gpui::AnyElement> {
    let y = slider_y(adjust);
    let value = adjust.of(style);
    let (least, most) = adjust.range();
    let width = TRACK.1 - TRACK.0;
    let knob = TRACK.0 + along(adjust, style) * width;
    let middle = TRACK.0 + width / 2.;
    let line = |left: f32, width: f32, height: f32| {
        div()
            .absolute()
            .left(px(card_left + left))
            .top(px(y - height / 2.))
            .w(px(width))
            .h(px(height))
    };
    let label = line(INSET, LABEL_WIDTH + GAP, BUTTON)
        .flex()
        .items_center()
        .whitespace_nowrap()
        .text_size(px(text::SMALL))
        .text_color(palette.on_overlay(DIM))
        .child(adjust.title());
    // The track as it is, not raised to the overlay's text floor as the fill is, or it reads as
    // bright as the fill and the fill from 100 % can't be seen.
    let track = line(TRACK.0, width, MEDIA_SLIDER_TRACK)
        .rounded_full()
        .bg(palette.faint(SLIDER_TRACK_INK));
    let fill = line(knob.min(middle), (knob - middle).abs(), MEDIA_SLIDER_TRACK)
        .rounded_full()
        .bg(palette.on_overlay(SLIDER_FILL_INK));
    let tick = line(middle - 0.5, 1., SLIDER_TICK).bg(palette.on_overlay(DIM));
    let size = MEDIA_SLIDER_KNOB;
    let knob = line(knob - size / 2., size, size)
        .rounded_full()
        .bg(palette.on_overlay(1.))
        .when(focused, |knob| {
            knob.border(px(SLIDER_RING)).border_color(palette.accent)
        });
    let shown = line(CONTROL_RIGHT - SLIDER_VALUE, SLIDER_VALUE, BUTTON)
        .flex()
        .items_center()
        .justify_end()
        .text_size(px(text::BODY))
        .text_color(palette.ink)
        .child(format!("{value} %"));
    let (up, down) = (editor.clone(), editor);
    let slider = line(TRACK.0, width, BUTTON)
        .id(("boost-slider", adjust as usize))
        .when(a11y, |slider| {
            slider
                .role(Role::Slider)
                .aria_label(adjust.title())
                .aria_min_numeric_value(f64::from(least))
                .aria_max_numeric_value(f64::from(most))
                .aria_numeric_value_step(f64::from(ADJUST_STEP))
                .aria_numeric_value(f64::from(value))
                .on_a11y_action(Action::Increment, move |_, _, cx| {
                    (up.update(cx, |editor, cx| editor.slide(adjust, 1, cx))).ok();
                })
                .on_a11y_action(Action::Decrement, move |_, _, cx| {
                    (down.update(cx, |editor, cx| editor.slide(adjust, -1, cx))).ok();
                })
        });
    vec![
        label.into_any_element(),
        track.into_any_element(),
        fill.into_any_element(),
        tick.into_any_element(),
        knob.into_any_element(),
        shown.into_any_element(),
        slider.into_any_element(),
    ]
}

#[cfg(test)]
mod tests;
