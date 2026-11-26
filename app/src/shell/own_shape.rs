//! A line of the sidebar drawn in the colour of what it opens in the page area, as a rounded shape
//! of its own beside it: the open tab's line in its page's colour, the shown thread's card in its
//! conversation's. The sidebar shows between the two.

#[cfg(test)]
use gpui::{Bounds, Pixels};
use gpui::{Corners, Edges, Hsla, IntoElement, Styled, canvas, fill, px};

use crate::tokens::shell::PAGE_OUTLINE_WIDTH;

#[cfg(test)]
thread_local! {
    /// What `own_shape` last painted, for a test of where a line's fill lies and what it's drawn in.
    pub(super) static OWN_SHAPE: std::cell::Cell<Option<OwnShape>> =
        const { std::cell::Cell::new(None) };
}

/// A painted shape: its box in the window, its corners, its fill and the line round it.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct OwnShape {
    pub shape: Bounds<Pixels>,
    pub corners: Corners<Pixels>,
    pub fill: Hsla,
    pub outline: Option<Hsla>,
}

/// The shape, as big as it's laid out: `color` with all four corners at `corner`. With `outline`,
/// a line of it runs round the shape inside its edge, for a fill the window wouldn't tell from
/// itself.
pub(super) fn own_shape(color: Hsla, outline: Option<Hsla>, corner: f32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |shape, _, window, _| {
            let corners = Corners::all(px(corner));
            #[cfg(test)]
            OWN_SHAPE.set(Some(OwnShape {
                shape,
                corners,
                fill: color,
                outline,
            }));
            let quad = fill(shape, color).corner_radii(corners);
            window.paint_quad(match outline {
                Some(outline) => quad
                    .border_widths(Edges::all(px(PAGE_OUTLINE_WIDTH)))
                    .border_color(outline),
                None => quad,
            });
        },
    )
    .absolute()
    .size_full()
}

#[cfg(test)]
mod tests;
