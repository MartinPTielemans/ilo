//! The Site Controls' look (#4404): the plates, the hairline, the labels and the switch, in one
//! place, so a change of the tokens' names (#4401) is a change here alone.

use gpui::accesskit::Role;
use gpui::{Bounds, Div, ElementId, Hsla, Pixels, SharedString, Stateful, div, prelude::*, px};

use crate::palette::{Float, Palette, STATE_HOVERED, STATE_PRESSED};
use crate::tokens::site_controls::{
    HAIRLINE, LABEL_LINE, LABEL_SIZE, PLATE, SECONDARY, SWITCH_HEIGHT, SWITCH_KNOB, SWITCH_OFF,
    SWITCH_ON, SWITCH_WIDTH, TILE_RADIUS,
};

/// Every plate's corners, concentric with the body's.
pub(super) const RADIUS: f32 = TILE_RADIUS;

/// `plate` as it lies on the panel's material.
fn on_panel(palette: &Palette, plate: Hsla) -> Hsla {
    palette.on_float(Float::SiteControls, plate)
}

fn framed(div: Stateful<Div>, frame: Bounds<Pixels>) -> Stateful<Div> {
    div.absolute()
        .left(frame.origin.x)
        .top(frame.origin.y)
        .w(frame.size.width)
        .h(frame.size.height)
}

/// A control at `frame` on the ink's plate at `rest`, none at 0, a step of the ladder under the
/// pointer and another pressed: a tile, an extension, "…", the chip, the connection's words.
pub(super) fn pressable(
    palette: &Palette,
    id: impl Into<ElementId>,
    frame: Bounds<Pixels>,
    rest: f32,
) -> Stateful<Div> {
    let palette = *palette;
    framed(div().id(id), frame)
        .rounded(px(RADIUS))
        .when(rest > 0., |plate| {
            plate.bg(on_panel(&palette, palette.plate(rest)))
        })
        .hover(move |style| style.bg(on_panel(&palette, palette.state(rest, STATE_HOVERED))))
        .active(move |style| style.bg(on_panel(&palette, palette.state(rest, STATE_PRESSED))))
}

/// A tile or an extension's: the flat plate, as pressable.
pub(super) fn tile(
    palette: &Palette,
    id: impl Into<ElementId>,
    frame: Bounds<Pixels>,
) -> Stateful<Div> {
    pressable(palette, id, frame, PLATE)
}

/// The site's plate under its rows, which take the pointer themselves.
pub(super) fn group(palette: &Palette, frame: Bounds<Pixels>) -> Div {
    div()
        .absolute()
        .left(frame.origin.x)
        .top(frame.origin.y)
        .w(frame.size.width)
        .h(frame.size.height)
        .rounded(px(RADIUS))
        .bg(on_panel(palette, palette.plate(PLATE)))
}

/// A row of the site's plate at `frame`, the ladder under the pointer, rounded as the plate is
/// where it's the plate's `first` or `last`.
pub(super) fn row(
    palette: &Palette,
    id: impl Into<ElementId>,
    frame: Bounds<Pixels>,
    (first, last): (bool, bool),
) -> Stateful<Div> {
    let palette = *palette;
    framed(div().id(id), frame)
        .when(first, |row| row.rounded_t(px(RADIUS)))
        .when(last, |row| row.rounded_b(px(RADIUS)))
        .hover(move |style| style.bg(on_panel(&palette, palette.state(PLATE, STATE_HOVERED))))
        .active(move |style| style.bg(on_panel(&palette, palette.state(PLATE, STATE_PRESSED))))
}

/// A line one device pixel thick at `frame`'s top, `scale` pixels to a point.
pub(super) fn hairline(palette: &Palette, frame: Bounds<Pixels>, scale: f32) -> Div {
    div()
        .absolute()
        .left(frame.origin.x)
        .top(frame.origin.y)
        .w(frame.size.width)
        .h(px(1. / scale.max(1.)))
        .bg(palette.plate(HAIRLINE))
}

/// "This site" or "Extensions" at (`x`, `y`), over its plates, read as a heading while an
/// assistive app is on (`a11y`).
pub(super) fn label(
    palette: &Palette,
    text: &'static str,
    (x, y): (f32, f32),
    a11y: bool,
) -> Stateful<Div> {
    div()
        .id(text)
        .when(a11y, |label| label.role(Role::Heading).aria_label(text))
        .absolute()
        .left(px(x))
        .top(px(y))
        .h(px(LABEL_LINE))
        .line_height(px(LABEL_LINE))
        .text_size(px(LABEL_SIZE))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(palette.on_overlay(SECONDARY))
        .child(SharedString::from(text))
}

/// The switch, in the ink, no accent: its track strong while `on` and faint while off, its knob
/// the panel's own fill, at the end it's turned to.
pub(super) fn switch(palette: &Palette, on: bool) -> Div {
    let track = palette.plate(if on { SWITCH_ON } else { SWITCH_OFF });
    let inset = (SWITCH_HEIGHT - SWITCH_KNOB) / 2.;
    let knob = match on {
        true => SWITCH_WIDTH - inset - SWITCH_KNOB,
        false => inset,
    };
    div()
        .relative()
        .flex_none()
        .w(px(SWITCH_WIDTH))
        .h(px(SWITCH_HEIGHT))
        .rounded_full()
        .bg(track)
        .child(
            div()
                .absolute()
                .top(px(inset))
                .left(px(knob))
                .size(px(SWITCH_KNOB))
                .rounded_full()
                .bg(Float::SiteControls.fill(palette.tone)),
        )
}
