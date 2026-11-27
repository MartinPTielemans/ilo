//! The open tab's line, and the shown thread's card, as a pane lifted out of the sidebar's glass
//! (#4398): the same pane in every Space and over every page, with a faint light in the icon's
//! main colour behind the icon. It's drawn from tokens alone, so a tab switch lays the same few
//! quads the plain plate did and decodes nothing it hasn't (tile_theme's `main_colour`).

use gpui::{
    BoxShadow, Div, Hsla, Rgba, Styled, div, linear_color_stop, linear_gradient, point, prelude::*,
    px, rgba,
};

use crate::palette::Tone;
use crate::tokens::shell::pane::{
    DARK_BOTTOM, DARK_EDGE, DARK_HAIRLINE, DARK_SHADOW, DARK_SHADOW_BLUR, DARK_SHADOW_Y, DARK_TOP,
    GLOW_DARK, GLOW_LIGHT, GLOW_REACH, HOVERED, LIGHT_FILL, LIGHT_HAIRLINE, LIGHT_SHADOW,
    LIGHT_SHADOW_BLUR, LIGHT_SHADOW_Y,
};

#[cfg(test)]
thread_local! {
    /// The panes drawn, for a test of which line gets the pane, where, and what light it has.
    pub(super) static PANES: std::cell::RefCell<Vec<Drawn>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A pane drawn: what with, and its box in the window.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Drawn {
    pub look: Pane,
    pub shape: gpui::Bounds<gpui::Pixels>,
}

/// What a pane is drawn with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Pane {
    pub tone: Tone,
    /// The icon's main colour, 0…1, and its middle's distance from the pane's leading edge.
    pub glow: Option<([f32; 3], f32)>,
    /// How far the pointer's step has come, 0…1.
    pub hovered: f32,
}

/// The pane, as big as it's laid out, with corners of `corner`, at `opacity`.
pub(super) fn pane(look: Pane, corner: f32, opacity: f32) -> Div {
    let step = HOVERED * look.hovered;
    let stronger = |color: u32| stronger(color, look.hovered, opacity);
    let faded = |color: u32| Hsla::from(rgba(color)).opacity(opacity);
    let shadow = |color: u32, y: f32, blur: f32| BoxShadow {
        color: faded(color),
        offset: point(px(0.), px(y)),
        // A CSS blur is twice the Gaussian's deviation, which GPUI takes.
        blur_radius: px(blur / 2.),
        spread_radius: px(0.),
        inset: false,
    };
    let base = div()
        .absolute()
        .inset_0()
        .rounded(px(corner))
        .border(px(1.));
    let (base, glow_at) = match look.tone {
        Tone::Dark => (
            base.bg(linear_gradient(
                180.,
                linear_color_stop(stronger(DARK_TOP), 0.),
                linear_color_stop(stronger(DARK_BOTTOM), 1.),
            ))
            .border_color(faded(DARK_HAIRLINE))
            .shadow(vec![shadow(DARK_SHADOW, DARK_SHADOW_Y, DARK_SHADOW_BLUR)])
            // The top edge catches the light, inside the corners' curve.
            .child(
                div()
                    .absolute()
                    .top(px(-1.))
                    .left(px(corner))
                    .right(px(corner))
                    .h(px(1.))
                    .bg(faded(DARK_EDGE)),
            ),
            GLOW_DARK,
        ),
        Tone::Light => (
            base.bg(stronger(LIGHT_FILL))
                .border_color(faded(LIGHT_HAIRLINE))
                .shadow(vec![shadow(
                    LIGHT_SHADOW,
                    LIGHT_SHADOW_Y,
                    LIGHT_SHADOW_BLUR,
                )]),
            GLOW_LIGHT,
        ),
    };
    // Its box is the border's, outside the room its children are laid in.
    #[cfg(test)]
    let base = base.child(
        gpui::canvas(
            move |shape, _, _| PANES.with_borrow_mut(|panes| panes.push(Drawn { look, shape })),
            |_, _, _, _| {},
        )
        .absolute()
        .top(px(-1.))
        .bottom(px(-1.))
        .left(px(-1.))
        .right(px(-1.)),
    );
    // The light lies over the fill from the leading edge, full to the icon's middle and gone
    // GLOW_REACH past it, in the pane's own leading corners, so none of it spills outside.
    base.children(look.glow.map(|([r, g, b], middle)| {
        let width = middle + GLOW_REACH;
        let color = |a: f32| {
            Hsla::from(Rgba {
                r,
                g,
                b,
                a: a * opacity,
            })
        };
        let strength = glow_at + step;
        div()
            .absolute()
            .top(px(-1.))
            .bottom(px(-1.))
            .left(px(-1.))
            .w(px(width))
            .rounded_l(px(corner))
            .bg(linear_gradient(
                90.,
                linear_color_stop(color(strength), middle / width),
                linear_color_stop(color(0.), 1.),
            ))
    }))
}

/// `color` a step stronger as far as the pointer's `hovered` has come, at `opacity`.
pub(super) fn stronger(color: u32, hovered: f32, opacity: f32) -> Hsla {
    let color = rgba(color);
    Hsla::from(Rgba {
        a: (color.a + HOVERED * hovered) * opacity,
        ..color
    })
}

#[cfg(test)]
mod tests;
