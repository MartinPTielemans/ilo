//! A line of the sidebar drawn as one shape with what it opens in the page area: its fill runs to
//! the sidebar's edge, where two curves take it into the page's side.

use gpui::{Bounds, Hsla, IntoElement, PathBuilder, Pixels, Styled, canvas, point, px};

use crate::tokens::shell::{JOIN_RADIUS, PAGE_OUTLINE_WIDTH, PLATE_RADIUS};

#[cfg(test)]
thread_local! {
    /// What `join` last painted, for a test of where a line's fill lies and what it's drawn in.
    pub(super) static JOINED: std::cell::Cell<Option<Joined>> = const { std::cell::Cell::new(None) };
}

/// A painted join: the line's box in the window, its fill and its outline, and whether the page
/// is on its left.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Joined {
    pub line: Bounds<Pixels>,
    pub fill: Hsla,
    pub outline: Option<Hsla>,
    pub page_left: bool,
}

/// The join, as big as it's laid out: a line in `fill` from its own rounded end to the edge the
/// page starts at, `page_left` with the sidebar on the right, with a curve above and below it
/// there, so the line and the page of that fill are one shape. With `outline`, a line of it runs
/// round the shape, where the page's own carries on, for a fill the window wouldn't tell from
/// itself. A page's tab and a task's card over its stage are both this, each with its own fill.
pub(super) fn join(fill: Hsla, outline: Option<Hsla>, page_left: bool) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |line, _, window, _| {
            #[cfg(test)]
            JOINED.set(Some(Joined {
                line,
                fill,
                outline,
                page_left,
            }));
            let mut shape = PathBuilder::fill();
            trace(&mut shape, line, page_left, 0.);
            shape.close();
            if let Ok(shape) = shape.build() {
                window.paint_path(shape, fill);
            }
            let Some(outline) = outline else {
                return;
            };
            // The page's own line runs down its whole side, under the join: the fill takes its
            // place between the curves, where the two are one shape and no line parts them.
            let (width, curve) = (px(PAGE_OUTLINE_WIDTH), px(JOIN_RADIUS));
            let edge = match page_left {
                true => line.left(),
                false => line.right() - width,
            };
            let side = Bounds::from_corners(
                point(edge, line.top() - curve),
                point(edge + width, line.bottom() + curve),
            );
            window.paint_quad(gpui::fill(side, fill));
            let mut round = PathBuilder::stroke(width);
            trace(&mut round, line, page_left, PAGE_OUTLINE_WIDTH / 2.);
            if let Ok(round) = round.build() {
                window.paint_path(round, outline);
            }
        },
    )
    .absolute()
    .size_full()
}

/// The shape's edge, from the page's side above the line round the line's far end to the page's
/// side below it, `out` outside the shape itself, for a line drawn round it.
fn trace(path: &mut PathBuilder, line: Bounds<Pixels>, page_left: bool, out: f32) {
    let out = px(out);
    let (curve, corner) = (px(JOIN_RADIUS) - out, px(PLATE_RADIUS) + out);
    let (top, bottom) = (line.top() - out, line.bottom() + out);
    // Along the line from the page's edge: 1 toward the left, -1 with the page on the left.
    let (edge, way) = match page_left {
        true => (line.left() + out, -1.),
        false => (line.right() - out, 1.),
    };
    let far = match page_left {
        true => line.right() + out,
        false => line.left() - out,
    };
    let at = |along: Pixels, y: Pixels| point(edge - along * way, y);
    let from_far = |along: Pixels, y: Pixels| point(far + along * way, y);
    let arc = |path: &mut PathBuilder, radius: Pixels, inward: bool, to| {
        // Mirrored, each arc turns the other way.
        path.arc_to(
            point(radius, radius),
            px(0.),
            false,
            inward != page_left,
            to,
        );
    };
    path.move_to(at(px(0.), top - curve));
    arc(path, curve, true, at(curve, top));
    path.line_to(from_far(corner, top));
    arc(path, corner, false, from_far(px(0.), top + corner));
    path.line_to(from_far(px(0.), bottom - corner));
    arc(path, corner, false, from_far(corner, bottom));
    path.line_to(at(curve, bottom));
    arc(path, curve, true, at(px(0.), bottom + curve));
}

#[cfg(test)]
mod tests;
