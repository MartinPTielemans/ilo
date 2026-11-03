//! The Settings window's boxes of rows and their controls, which the views it shows from the
//! library, such as the import, draw with too, and the stops the keyboard goes through them by.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    App, BorderStyle, Bounds, ElementId, FocusHandle, Pixels, Rgba, ScrollHandle, SharedString,
    Window, actions, canvas, div, point, prelude::*, px, quad, rgb, svg, transparent_black, white,
};

use crate::tokens::settings::{
    BOX, BOX_LINE, CHECK_BOX, CHECK_GAP, CHECK_GLYPH, CHECK_RADIUS, CHEVRONS, CONTROL,
    CONTROL_HEIGHT, CONTROL_HOVERED, CONTROL_PADDING, CONTROL_PRESSED, CONTROL_RADIUS, DETAIL,
    KNOB, KNOB_HEIGHT, KNOB_INSET, KNOB_ON, KNOB_WIDTH, LINE_INSET, ON_CONTROL, ON_DETAIL,
    ON_HOVERED, ON_LINE, ON_PRESSED, ON_SWITCH_OFF, PADDING, RADIUS, SWITCH_HEIGHT, SWITCH_OFF,
    SWITCH_WIDTH, TEXT, TILE, TILE_GAP, TILE_GLYPH, TILE_GLYPH_COLOR, TILE_RADIUS,
};
use crate::tokens::shell::{KEYED, KEYED_RING};
use crate::tokens::text::SMALL;

#[cfg(test)]
mod tests;

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

#[derive(Default)]
struct Shared {
    drawn: RefCell<Vec<Drawn>>,
    /// The ring shows from a key that moves it until a press, as the theme picker's does.
    keyed: Cell<bool>,
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
        Stops { handles: RefCell::default(), shared: self.shared.clone() }
    }

    /// The focus handle of the stop `id`.
    pub fn handle(&self, id: impl Into<ElementId>, cx: &App) -> FocusHandle {
        let mut handles = self.handles.borrow_mut();
        handles.entry(id.into()).or_insert_with(|| cx.focus_handle()).clone()
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
                drawn.push(Drawn { id, handle, frame, shown });
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
                let ink = white().opacity(KEYED);
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
        (Some(at), true) if at >= last => if round { 0 } else { last },
        (Some(at), true) => at + 1,
        (Some(0), false) => if round { last } else { 0 },
        (Some(at), false) => at - 1,
    })
}

/// Scrolls `scroll` the least that shows the stop `drawn` whole, with a padding's room round it
/// for its ring, if it's in that list and a part of it is out of sight.
pub fn reveal(scroll: &ScrollHandle, drawn: &Drawn) {
    let view = scroll.bounds();
    // A stop outside the list, such as Settings' tabs over it, shows in another area.
    if drawn.shown.top() < view.top() || drawn.shown.bottom() > view.bottom() {
        return;
    }
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
        self.track_focus(&handle).relative().child(stops.mark(id, handle, ring))
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

/// A switch is 36 by 16, with a knob 21 by 13, 1.5 in, that slides across over 150 ms (ours).
pub const SLIDE: f32 = 0.15;

/// The colours rows are drawn in: Settings' own, or those of the ground another view puts them on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// A box's fill.
    pub surface: Rgba,
    /// The text.
    pub ink: Rgba,
    pub line: Rgba,
    /// A row's grey text, under its name or at its end.
    pub detail: Rgba,
    pub control: Rgba,
    /// A control under the pointer.
    pub hovered: Rgba,
    pub pressed: Rgba,
    pub switch_off: Rgba,
}

impl Look {
    pub fn settings() -> Look {
        Look {
            surface: rgb(BOX),
            ink: rgb(TEXT),
            line: rgb(BOX_LINE),
            detail: rgb(DETAIL),
            control: rgb(CONTROL),
            hovered: rgb(CONTROL_HOVERED),
            pressed: rgb(CONTROL_PRESSED),
            switch_off: rgb(SWITCH_OFF),
        }
    }

    /// Boxes filled with `surface`, in `ink`, the rest `ink` over `surface` as Settings' are, so
    /// they read the same on a light ground as on Settings' dark one.
    pub fn on(surface: Rgba, ink: Rgba) -> Look {
        let mix = |amount: f32| surface.blend(ink.opacity(amount));
        Look {
            surface,
            ink,
            line: mix(ON_LINE),
            detail: mix(ON_DETAIL),
            control: mix(ON_CONTROL),
            hovered: mix(ON_HOVERED),
            pressed: mix(ON_PRESSED),
            switch_off: mix(ON_SWITCH_OFF),
        }
    }
}

/// A box of `rows`, with lines between them.
pub fn group(look: &Look, rows: impl IntoIterator<Item = gpui::Div>) -> gpui::Div {
    let rows = rows.into_iter().enumerate().map(|(index, row)| {
        div()
            .when(index > 0, |below| {
                below.child(div().mx(px(LINE_INSET)).h(px(1.)).bg(look.line))
            })
            .child(row)
    });
    div()
        .flex()
        .flex_col()
        .rounded(px(RADIUS))
        .border_1()
        .border_color(look.line)
        .bg(look.surface)
        .children(rows)
}

/// A row: its icon if any, its name, and its control at the right.
pub fn row(
    icon: Option<(&'static str, u32)>,
    name: impl Into<SharedString>,
    control: gpui::AnyElement,
) -> gpui::Div {
    div()
        .px(px(PADDING))
        .py(px(PADDING))
        .flex()
        .items_center()
        .children(icon.map(|(glyph, color)| tile(glyph, color)))
        .child(div().flex_1().min_w_0().truncate().child(name.into()))
        .child(div().flex_none().ml(px(PADDING)).child(control))
}

/// A row with a grey line under its name, and its control, if any, at the right.
pub fn detailed_row(
    name: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    control: Option<gpui::AnyElement>,
) -> gpui::Div {
    div()
        .px(px(PADDING))
        .py(px(PADDING))
        .flex()
        .items_center()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(div().truncate().child(name.into()))
                .child(div().text_size(px(SMALL)).text_color(rgb(DETAIL)).child(detail.into())),
        )
        .children(control.map(|control| div().flex_none().ml(px(PADDING)).child(control)))
}

/// A row's icon: a white glyph on a tile of colour.
pub fn tile(glyph: &'static str, color: u32) -> gpui::Div {
    div()
        .flex_none()
        .size(px(TILE))
        .mr(px(TILE_GAP))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(TILE_RADIUS))
        .bg(rgb(color))
        .child(
            svg()
                .path(glyph)
                .size(px(TILE_GLYPH))
                .text_color(rgb(TILE_GLYPH_COLOR)),
        )
}

/// A push button, a step toward its pressed fill under the pointer, and that fill while pressed.
pub fn button(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    title: impl Into<SharedString>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .h(px(CONTROL_HEIGHT))
        .px(px(CONTROL_PADDING))
        .flex()
        .items_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(look.control)
        .hover({
            let hovered = look.hovered;
            move |button| button.bg(hovered)
        })
        .active({
            let pressed = look.pressed;
            move |button| button.bg(pressed)
        })
        .child(title.into())
}

/// A square push button with `glyph` on it, such as a list's + and −, lit as a button is.
pub fn icon_button(
    look: &Look,
    id: impl Into<gpui::ElementId>,
    glyph: &'static str,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex_none()
        .size(px(CONTROL_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(look.control)
        .hover({
            let hovered = look.hovered;
            move |button| button.bg(hovered)
        })
        .active({
            let pressed = look.pressed;
            move |button| button.bg(pressed)
        })
        .child(svg().path(glyph).size(px(TILE_GLYPH)).text_color(look.ink))
}

/// A pop-up button `width` wide showing `label`, with its chevrons at its end, lit as a button is
/// under the pointer and pressed while its menu is `open`. It opens on a press, as a pop-up
/// button does, which its caller listens for.
pub fn popup(look: &Look, label: impl Into<SharedString>, width: f32, open: bool) -> gpui::Div {
    div()
        .flex_none()
        .w(px(width))
        .h(px(CONTROL_HEIGHT))
        .pl(px(CONTROL_PADDING))
        .pr(px(PADDING - (CHEVRONS - 8.) / 2.))
        .flex()
        .items_center()
        .justify_between()
        .rounded(px(CONTROL_RADIUS))
        .bg(if open { look.pressed } else { look.control })
        .when(!open, |popup| {
            let hovered = look.hovered;
            popup.hover(move |popup| popup.bg(hovered))
        })
        .child(div().min_w_0().truncate().child(label.into()))
        .child(svg().path("icons/chevrons-up-down.svg").size(px(CHEVRONS)).text_color(look.ink))
}

/// A switch, on or off, with its knob `slid` of the way to that side.
pub fn switch(look: &Look, on: bool, slid: f32, accent: Rgba) -> impl IntoElement {
    let travel = SWITCH_WIDTH - KNOB_WIDTH - 2. * KNOB_INSET;
    let way = if on { slid } else { 1. - slid };
    div()
        .relative()
        .w(px(SWITCH_WIDTH))
        .h(px(SWITCH_HEIGHT))
        .rounded_full()
        .bg(if on { accent } else { look.switch_off })
        .child(
            div()
                .absolute()
                .top(px((SWITCH_HEIGHT - KNOB_HEIGHT) / 2.))
                .left(px(KNOB_INSET + travel * way))
                .w(px(KNOB_WIDTH))
                .h(px(KNOB_HEIGHT))
                .rounded_full()
                .bg(rgb(if on { KNOB_ON } else { KNOB })),
        )
}

/// A checkbox, ticked in `accent` with a `tick`, else a line round the switch's off fill.
pub fn checkbox(look: &Look, on: bool, accent: Rgba, tick: Rgba) -> gpui::Div {
    let tick = svg().path("icons/check.svg").size(px(CHECK_GLYPH)).text_color(tick);
    div()
        .flex_none()
        .size(px(CHECK_BOX))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CHECK_RADIUS))
        .when(on, |ticked| ticked.bg(accent).child(tick))
        .when(!on, |empty| empty.border_1().border_color(look.line).bg(look.switch_off))
}

/// A row with a checkbox before its name, as a list to tick from has.
pub fn checked_row(
    look: &Look,
    on: bool,
    name: impl Into<SharedString>,
    accent: Rgba,
    tick: Rgba,
) -> gpui::Div {
    div()
        .px(px(PADDING))
        .py(px(PADDING))
        .flex()
        .items_center()
        .gap(px(CHECK_GAP))
        .child(checkbox(look, on, accent, tick))
        .child(div().flex_1().min_w_0().truncate().child(name.into()))
}
