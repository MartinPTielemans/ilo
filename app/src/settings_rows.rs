//! The Settings window's groups of rows and their controls, which the views it shows from the
//! library, such as the import, draw with too, and the stops the keyboard goes through them by.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    App, BorderStyle, Bounds, BoxShadow, ElementId, FocusHandle, Hsla, Pixels, Rgba, ScrollHandle,
    SharedString, Window, actions, canvas, div, point, prelude::*, px, quad, rgb, rgba, svg,
    transparent_black, white,
};

use crate::palette::{INK, OVERLAY_DARK, PAPER};
use crate::tokens::palette::DIALOG_LIGHT;
use crate::tokens::radius;
use crate::tokens::settings::{
    BUTTON_GLYPH, CHECK_BOX, CHECK_GAP, CHECK_GLYPH, CHECK_RADIUS, CHEVRONS, CHEVRONS_ALPHA,
    CHEVRONS_END, CONTROL, CONTROL_GAP, CONTROL_HEIGHT, CONTROL_HOVERED, CONTROL_HOVERED_LIGHT,
    CONTROL_LIGHT, CONTROL_PADDING, CONTROL_PRESSED, CONTROL_PRESSED_LIGHT, CONTROL_RADIUS, DETAIL,
    DETAIL_LINE, DETAIL_TOP, END_GAP, GLYPH, GLYPH_OFF, GLYPH_ON, GLYPH_PLATE, GLYPH_PLATE_FILL,
    GLYPH_PLATE_GAP, GROUP, GROUP_LIGHT, HEADING, KNOB, KNOB_ALPHA, KNOB_INSET, KNOB_SHADOW, LINE,
    NAME, NAME_OFF, ON_CONTROL, ON_DETAIL, ON_HOVERED, ON_LINE, ON_PRESSED, ON_SWITCH_OFF, PADDING,
    ROW_HEIGHT, ROW_X, SWITCH_HEIGHT, SWITCH_OFF, SWITCH_OFF_LIGHT, SWITCH_WIDTH,
};
use crate::tokens::shell::{KEYED, KEYED_RING};
use crate::tokens::text::SMALL;

#[cfg(test)]
mod stop_tests;

actions!(settings_rows, [NextStop, PreviousStop, StopDown, StopUp]);

/// Tab and ⇧Tab move the ring round the stops, ↓ and ↑ along them to an end, in `context`, as in
/// the theme picker and Site Controls. The arrows are a field's own while one has the keyboard.
pub fn bind_stop_keys(context: &str, cx: &mut App) {
    let arrows = format!("{context} && !TextInput");
    cx.bind_keys([
        gpui::KeyBinding::new("tab", NextStop, Some(context)),
        gpui::KeyBinding::new("shift-tab", PreviousStop, Some(context)),
        gpui::KeyBinding::new("down", StopDown, Some(&arrows)),
        gpui::KeyBinding::new("up", StopUp, Some(&arrows)),
    ]);
}

/// Where the keyboard's ring goes on a stop: Focus Sidebar's, `KEYED_RING` in the ink at `KEYED`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ring {
    /// Inside a plate with corners of this radius, as on a button.
    Inside(f32),
    /// `RING_GAP` round a control with corners of this radius, as round a switch, whose knob a
    /// ring inside would cover.
    Around(f32),
}

/// The ring is this far outside what it goes round, as round a theme's swatch.
const RING_GAP: f32 = 1.;

/// The controls Tab and the arrows go through in Settings and the views it shows (#1526): each
/// one's focus handle, kept by its id from frame to frame, which GPUI gives space and return to as
/// a click, and those drawn last frame in the order drawn, which is the order the keys go in. A
/// view Settings shows draws into its order (`share`); one shown elsewhere has its own, which no
/// key moves.
#[derive(Default)]
pub struct Stops {
    handles: RefCell<HashMap<ElementId, FocusHandle>>,
    shared: Rc<Shared>,
}

struct Shared {
    drawn: RefCell<Vec<Drawn>>,
    /// The ring shows from a key that moves it until a press, as the theme picker's does.
    keyed: Cell<bool>,
    /// The ring's ink: the window's, white on dark and ink on light.
    ink: Cell<Hsla>,
}

impl Default for Shared {
    fn default() -> Self {
        Shared {
            drawn: RefCell::default(),
            keyed: Cell::new(false),
            ink: Cell::new(white()),
        }
    }
}

/// A stop as last drawn: where, and the area it shows in, which a scrolled list clips it to.
#[derive(Clone, Debug)]
pub struct Drawn {
    pub id: ElementId,
    pub handle: FocusHandle,
    pub frame: Bounds<Pixels>,
    pub shown: Bounds<Pixels>,
}

impl Stops {
    /// Stops of another view that join this one's order, with ids of their own.
    pub fn share(&self) -> Stops {
        Stops {
            handles: RefCell::default(),
            shared: self.shared.clone(),
        }
    }

    /// The focus handle of the stop `id`.
    pub fn handle(&self, id: impl Into<ElementId>, cx: &App) -> FocusHandle {
        let mut handles = self.handles.borrow_mut();
        handles
            .entry(id.into())
            .or_insert_with(|| cx.focus_handle())
            .clone()
    }

    /// Starts the order again, as the window renders a frame.
    pub fn begin(&self) {
        self.shared.drawn.borrow_mut().clear();
    }

    pub fn keyed(&self) -> bool {
        self.shared.keyed.get()
    }

    /// Takes the ring away, as a press does, or shows it on whatever has the keyboard.
    pub fn set_keyed(&self, keyed: bool) {
        self.shared.keyed.set(keyed);
    }

    /// Draws the ring in `ink`, the window's, as it turns light or dark.
    pub fn set_ink(&self, ink: Hsla) {
        self.shared.ink.set(ink);
    }

    /// Gives the keyboard and the ring to the stop after the one that has the keyboard, or the
    /// one before, not `forward`: round from the last to the first and back with `round`, else
    /// staying at the end. With none on a stop, the first, or going back the last.
    pub fn go(
        &self,
        forward: bool,
        round: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Drawn> {
        let next = {
            let drawn = self.shared.drawn.borrow();
            let from = window.focused(cx);
            let at = from.and_then(|from| drawn.iter().position(|each| each.handle == from));
            drawn.get(step(at, drawn.len(), forward, round)?).cloned()
        }?;
        // check.sh waits for this line.
        crate::log!("browser: settings keyed {}", next.id);
        window.focus(&next.handle, cx);
        self.set_keyed(true);
        window.refresh();
        Some(next)
    }

    /// Fills the relative element it's put in: puts `handle`, as `id`, in the order where it's
    /// drawn, and draws `ring` there while the keyboard is on it from a key.
    pub fn mark(
        &self,
        id: impl Into<ElementId>,
        handle: FocusHandle,
        ring: Ring,
    ) -> gpui::Canvas<()> {
        let (id, shared, keyed) = (id.into(), self.shared.clone(), self.shared.clone());
        let painted = handle.clone();
        canvas(
            move |frame, window, _| {
                let shown = window.content_mask().bounds;
                let mut drawn = shared.drawn.borrow_mut();
                drawn.retain(|each| each.handle != handle);
                drawn.push(Drawn {
                    id,
                    handle,
                    frame,
                    shown,
                });
            },
            move |frame, _, window, _| {
                if !keyed.keyed.get() || !painted.is_focused(window) {
                    return;
                }
                let (bounds, radius) = match ring {
                    Ring::Inside(radius) => (frame, radius),
                    Ring::Around(radius) => {
                        let reach = RING_GAP + KEYED_RING;
                        (frame.dilate(px(reach)), radius + reach)
                    }
                };
                let ink = keyed.ink.get().opacity(KEYED);
                let (line, solid) = (px(KEYED_RING), BorderStyle::Solid);
                let ring = quad(bounds, px(radius), transparent_black(), line, ink, solid);
                window.paint_quad(ring);
            },
        )
        .absolute()
        .inset_0()
    }
}

/// Which of `len` stops the keys move to from the one `at`, as `Stops::go` says.
pub fn step(at: Option<usize>, len: usize, forward: bool, round: bool) -> Option<usize> {
    let last = len.checked_sub(1)?;
    Some(match (at, forward) {
        (None, true) => 0,
        (None, false) => last,
        (Some(at), true) if at >= last => {
            if round {
                0
            } else {
                last
            }
        }
        (Some(at), true) => at + 1,
        (Some(0), false) => {
            if round {
                last
            } else {
                0
            }
        }
        (Some(at), false) => at - 1,
    })
}

/// Scrolls `scroll` the least that shows the stop `drawn` whole, with a padding's room round it
/// for its ring, if it's in that list and a part of it is out of sight, or under the `top` and
/// `bottom` of the list that something covers, as Settings' card holds its section's name at its
/// top and fades at its foot.
pub fn reveal(scroll: &ScrollHandle, drawn: &Drawn, top: f32, bottom: f32) {
    let list = scroll.bounds();
    // A stop outside the list, such as Settings' column beside it, shows in another area.
    if drawn.shown.top() < list.top() || drawn.shown.bottom() > list.bottom() {
        return;
    }
    let view = Bounds::from_corners(
        point(list.left(), list.top() + px(top)),
        point(list.right(), list.bottom() - px(bottom)),
    );
    let offset = scroll.offset();
    let y = revealed(offset.y, drawn.frame, view, scroll.max_offset().y);
    if y != offset.y {
        scroll.set_offset(point(offset.x, y));
    }
}

/// The list's offset, `y` now, down to minus `max`, that shows `frame` in `view` with `PADDING`
/// round it, the top first where it's taller.
pub(crate) fn revealed(
    y: Pixels,
    frame: Bounds<Pixels>,
    view: Bounds<Pixels>,
    max: Pixels,
) -> Pixels {
    let room = px(PADDING);
    let moved = if frame.top() - room < view.top() {
        y + (view.top() - (frame.top() - room))
    } else if frame.bottom() + room > view.bottom() {
        let down = frame.bottom() + room - view.bottom();
        let up_to = frame.top() - room - view.top();
        y - down.min(up_to)
    } else {
        y
    };
    moved.clamp(-max, px(0.))
}

/// A control the keyboard goes through: Settings' buttons, pop-ups, switches and tabs.
pub trait Stop: InteractiveElement + ParentElement + Styled + Sized {
    /// Makes this, by its id, a stop of `stops`, with `ring` while the keyboard is on it. It's
    /// made relative, to hold the ring.
    fn stop(mut self, stops: &Stops, ring: Ring, cx: &App) -> Self {
        let Some(id) = self.interactivity().element_id.clone() else {
            return self;
        };
        let handle = stops.handle(id.clone(), cx);
        self.track_focus(&handle)
            .relative()
            .child(stops.mark(id, handle, ring))
    }

    /// A stop ringed inside its plate, as a button's.
    fn button_stop(self, stops: &Stops, cx: &App) -> Self {
        self.stop(stops, Ring::Inside(CONTROL_RADIUS), cx)
    }
}

impl<E: InteractiveElement + ParentElement + Styled> Stop for E {}

/// Fills the relative element it's put in and logs where that is drawn, its centre in the window,
/// as "browser: settings `what` at x y", each time it moves, for check.sh to click it there.
/// `logged` holds the lines logged, one a control; emptied, each is logged again where it's drawn.
pub fn drawn(what: String, logged: Rc<RefCell<Vec<String>>>) -> gpui::Canvas<()> {
    canvas(
        |_, _, _| {},
        move |bounds, _, _, _| {
            let (x, y) = (f32::from(bounds.center().x), f32::from(bounds.center().y));
            let line = format!("browser: settings {what} at {x:.0} {y:.0}");
            let mut logged = logged.borrow_mut();
            // Once a place: forget where it was drawn before.
            let at = format!("browser: settings {what} at ");
            if !logged.contains(&line) {
                crate::log!("{line}");
                logged.retain(|each| !each.starts_with(&at));
                logged.push(line);
            }
        },
    )
    .absolute()
    .inset_0()
}

/// A switch's knob slides across over 150 ms (ours).
pub const SLIDE: f32 = 0.15;

/// The colours rows are drawn in: Settings' own, or those of the ground another view puts them on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// A group's fill.
    pub surface: Rgba,
    /// A row's name.
    pub ink: Rgba,
    pub line: Rgba,
    /// A row's grey text, under its name or at its end.
    pub detail: Rgba,
    pub control: Rgba,
    /// A control under the pointer.
    pub hovered: Rgba,
    pub pressed: Rgba,
    pub switch_off: Rgba,
    /// A switch's knob while it's off. On, it's white on the accent.
    pub knob: Rgba,
    /// The plate a feature's glyph sits on, and the glyph while it's on.
    pub glyph_plate: Rgba,
    pub glyph: Rgba,
    /// A heading over a group, on the ground the groups are on.
    pub heading: Rgba,
    /// A group's edge, where its fill alone wouldn't stand off the ground.
    pub edge: Option<Rgba>,
}

impl Look {
    /// Settings' own, on its card: the paper over the overlay's fill in dark, the ink over the
    /// dialog's in light, as the window's other text is.
    pub fn settings(dark: bool) -> Look {
        let (card, base) = match dark {
            true => (rgb(OVERLAY_DARK), rgb(PAPER)),
            false => (rgb(DIALOG_LIGHT), rgb(INK)),
        };
        let pick = |dark_alpha: f32, light_alpha: f32| if dark { dark_alpha } else { light_alpha };
        let surface = card.blend(base.opacity(pick(GROUP, GROUP_LIGHT)));
        let mix = |amount: f32| surface.blend(base.opacity(amount));
        let switch_off = mix(pick(SWITCH_OFF, SWITCH_OFF_LIGHT));
        Look {
            surface,
            ink: mix(NAME),
            line: mix(LINE),
            detail: mix(DETAIL),
            control: mix(pick(CONTROL, CONTROL_LIGHT)),
            hovered: mix(pick(CONTROL_HOVERED, CONTROL_HOVERED_LIGHT)),
            pressed: mix(pick(CONTROL_PRESSED, CONTROL_PRESSED_LIGHT)),
            switch_off,
            knob: match dark {
                true => switch_off.blend(base.opacity(KNOB_ALPHA)),
                false => gpui::white().into(),
            },
            glyph_plate: mix(GLYPH_PLATE_FILL),
            glyph: mix(GLYPH_ON),
            heading: card.blend(base.opacity(HEADING)),
            edge: None,
        }
    }

    /// Groups filled with `surface`, in `ink`, the rest `ink` over `surface`, and edged in their
    /// line, as the first launch's ground has no card to set them off.
    pub fn on(surface: Rgba, ink: Rgba) -> Look {
        let mix = |amount: f32| surface.blend(ink.opacity(amount));
        let line = mix(ON_LINE);
        Look {
            surface,
            ink,
            line,
            detail: mix(ON_DETAIL),
            control: mix(ON_CONTROL),
            hovered: mix(ON_HOVERED),
            pressed: mix(ON_PRESSED),
            switch_off: mix(ON_SWITCH_OFF),
            knob: gpui::white().into(),
            glyph_plate: mix(GLYPH_PLATE_FILL),
            glyph: mix(GLYPH_ON),
            heading: ink,
            edge: Some(line),
        }
    }

    /// The name of a feature that's off, quieter than one that's on.
    pub fn off(&self) -> Rgba {
        self.surface.blend(self.ink.opacity(NAME_OFF / NAME))
    }

    /// Whether it's dark ink on a light ground, which its pop-ups' menus follow.
    pub fn light(&self) -> bool {
        let lum = |c: Rgba| c.r + c.g + c.b;
        lum(self.ink) < lum(self.surface)
    }
}

/// A group of `rows`, with a hairline between them from where their text starts.
pub fn group(look: &Look, rows: impl IntoIterator<Item = gpui::Div>) -> gpui::Div {
    group_lined(look, rows.into_iter().map(|row| (row, ROW_X)))
}

/// A group of rows, each with the hairline over it starting `x` in, as a feature's own settings
/// have theirs from where their text starts under its name.
pub fn group_lined(look: &Look, rows: impl IntoIterator<Item = (gpui::Div, f32)>) -> gpui::Div {
    let rows = rows.into_iter().enumerate().map(|(index, (row, x))| {
        let line = div()
            .absolute()
            .top_0()
            .left(px(x))
            .right(px(ROW_X))
            .h(px(1.))
            .bg(look.line);
        div()
            .relative()
            .child(row)
            .when(index > 0, |below| below.child(line))
    });
    div()
        .flex()
        .flex_col()
        .rounded(px(radius::ROW))
        .bg(look.surface)
        .when_some(look.edge, |group, edge| group.border_1().border_color(edge))
        .children(rows)
}

/// A row's frame: at least as tall as a row with a line, its text 14 in and its content centred.
pub fn frame() -> gpui::Div {
    div()
        .min_h(px(ROW_HEIGHT))
        .px(px(ROW_X))
        .py(px(PADDING))
        .flex()
        .items_center()
}

/// A row's text: its name, and the line under it that says more, if any, which wraps.
pub fn text(look: &Look, name: impl Into<SharedString>, line: Option<SharedString>) -> gpui::Div {
    let line = line.map(|line| {
        div()
            .mt(px(DETAIL_TOP))
            .text_size(px(SMALL))
            .line_height(px(DETAIL_LINE))
            .text_color(look.detail)
            .child(line)
    });
    div()
        .flex_1()
        .min_w_0()
        .child(div().truncate().child(name.into()))
        .children(line)
}

/// A row's end, its controls 20 after its text and 8 apart.
pub fn end(controls: impl IntoIterator<Item = gpui::AnyElement>) -> gpui::Div {
    div()
        .flex_none()
        .ml(px(END_GAP))
        .flex()
        .items_center()
        .gap(px(CONTROL_GAP))
        .children(controls)
}

/// A row: its name, and its control at the end.
pub fn row(name: impl Into<SharedString>, control: gpui::AnyElement) -> gpui::Div {
    frame()
        .child(div().flex_1().min_w_0().truncate().child(name.into()))
        .child(end([control]))
}

/// A row with a grey line under its name, and its control, if any, at the end.
pub fn detailed_row(
    look: &Look,
    name: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    control: Option<gpui::AnyElement>,
) -> gpui::Div {
    frame()
        .child(text(look, name, Some(detail.into())))
        .children(control.map(|c| end([c])))
}

/// A feature's glyph on its quiet plate, 12 before its name, quieter while it's off.
pub fn glyph_plate(look: &Look, glyph: &'static str, on: bool) -> gpui::Div {
    let color = if on {
        look.glyph
    } else {
        look.surface.blend(look.ink.opacity(GLYPH_OFF))
    };
    div()
        .flex_none()
        .size(px(GLYPH_PLATE))
        .mr(px(GLYPH_PLATE_GAP))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(look.glyph_plate)
        .child(svg().path(glyph).size(px(GLYPH)).text_color(color))
}

/// A push button, a step toward its pressed fill under the pointer, and that fill while pressed.
pub fn button(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    title: impl Into<SharedString>,
) -> gpui::Stateful<gpui::Div> {
    lit(look, div().id(id).px(px(CONTROL_PADDING))).child(title.into())
}

/// A square push button with `glyph` on it, such as a list's + and −, lit as a button is.
pub fn icon_button(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    glyph: &'static str,
) -> gpui::Stateful<gpui::Div> {
    lit(look, div().id(id).w(px(CONTROL_HEIGHT)).justify_center()).child(
        svg()
            .path(glyph)
            .size(px(BUTTON_GLYPH))
            .text_color(look.ink),
    )
}

/// A control's plate, lit under the pointer and pressed.
fn lit(look: &Look, control: gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div> {
    let (hovered, pressed) = (look.hovered, look.pressed);
    control
        .flex_none()
        .h(px(CONTROL_HEIGHT))
        .flex()
        .items_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(look.control)
        .hover(move |button| button.bg(hovered))
        .active(move |button| button.bg(pressed))
}

/// A pop-up button `width` wide showing `label`, with its chevrons at its end, lit as a button is
/// under the pointer and pressed while its menu is `open`. It opens on a press, as a pop-up
/// button does, which its caller listens for.
pub fn popup(look: &Look, label: impl Into<SharedString>, width: f32, open: bool) -> gpui::Div {
    let chevrons = svg()
        .flex_none()
        .path("icons/chevrons-up-down.svg")
        .size(px(CHEVRONS))
        .text_color(look.ink.opacity(CHEVRONS_ALPHA));
    div()
        .flex_none()
        .w(px(width))
        .h(px(CONTROL_HEIGHT))
        .pl(px(CONTROL_PADDING))
        .pr(px(CHEVRONS_END))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(CHEVRONS_END))
        .rounded(px(CONTROL_RADIUS))
        .bg(if open { look.pressed } else { look.control })
        .when(!open, |popup| {
            let hovered = look.hovered;
            popup.hover(move |popup| popup.bg(hovered))
        })
        .child(div().min_w_0().truncate().child(label.into()))
        .child(chevrons)
}

/// A switch, on or off, with its knob `slid` of the way to that side.
pub fn switch(look: &Look, on: bool, slid: f32, accent: Rgba) -> impl IntoElement {
    let travel = SWITCH_WIDTH - KNOB - 2. * KNOB_INSET;
    let way = if on { slid } else { 1. - slid };
    let shadow = BoxShadow {
        color: rgba(KNOB_SHADOW).into(),
        offset: point(px(0.), px(1.)),
        blur_radius: px(2.),
        spread_radius: px(0.),
        inset: false,
    };
    div()
        .flex_none()
        .relative()
        .w(px(SWITCH_WIDTH))
        .h(px(SWITCH_HEIGHT))
        .rounded_full()
        .bg(if on { accent } else { look.switch_off })
        .child(
            div()
                .absolute()
                .top(px(KNOB_INSET))
                .left(px(KNOB_INSET + travel * way))
                .size(px(KNOB))
                .rounded_full()
                .shadow(vec![shadow])
                .bg(if on { gpui::white().into() } else { look.knob }),
        )
}

/// A checkbox, ticked in `accent` with a `tick`, else a line round the switch's off fill.
pub fn checkbox(look: &Look, on: bool, accent: Rgba, tick: Rgba) -> gpui::Div {
    let tick = svg()
        .path("icons/check.svg")
        .size(px(CHECK_GLYPH))
        .text_color(tick);
    div()
        .flex_none()
        .size(px(CHECK_BOX))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CHECK_RADIUS))
        .when(on, |ticked| ticked.bg(accent).child(tick))
        .when(!on, |empty| {
            empty.border_1().border_color(look.line).bg(look.switch_off)
        })
}

/// A row with a checkbox before its name, as a list to tick from has.
pub fn checked_row(
    look: &Look,
    on: bool,
    name: impl Into<SharedString>,
    accent: Rgba,
    tick: Rgba,
) -> gpui::Div {
    frame()
        .gap(px(CHECK_GAP))
        .child(checkbox(look, on, accent, tick))
        .child(div().flex_1().min_w_0().truncate().child(name.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contrast(look: &Look, color: Rgba) -> f32 {
        let lum = |c: Rgba| c.r + c.g + c.b;
        (lum(color) - lum(look.surface)).abs()
    }

    /// In dark and light alike, each step of a control stands further off its group than the
    /// last, its line least and a name most, so a press shows more than the pointer does.
    #[test]
    fn each_step_stands_further_off_the_group() {
        for dark in [true, false] {
            let look = Look::settings(dark);
            let steps = [
                look.line,
                look.control,
                look.hovered,
                look.pressed,
                look.detail,
                look.ink,
            ];
            for pair in steps.windows(2) {
                assert!(
                    contrast(&look, pair[0]) < contrast(&look, pair[1]),
                    "{dark} {pair:?}"
                );
            }
            assert!(contrast(&look, look.off()) < contrast(&look, look.ink));
        }
    }

    /// Dark is the paper over the dark card and light the ink over the light one.
    #[test]
    fn dark_is_light_on_dark() {
        let lum = |c: Rgba| c.r + c.g + c.b;
        let (dark, light) = (Look::settings(true), Look::settings(false));
        assert!(lum(dark.ink) > lum(dark.surface) && lum(light.ink) < lum(light.surface));
        assert!(!dark.light() && light.light());
        assert!(dark.edge.is_none() && Look::on(dark.surface, dark.ink).edge.is_some());
    }
}
