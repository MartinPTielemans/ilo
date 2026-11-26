//! The ring a thread's running work wears, wherever it shows (spec-thread-stage.md): a track round
//! a quarter arc that turns on the shell's breath, so it draws only while something works and an
//! idle window draws nothing. With motion reduced the arc holds still, at the top.
use std::f32::consts::TAU;

use gpui::{Hsla, IntoElement, PathBuilder, Pixels, Point, canvas, div, point, prelude::*, px};

/// A ring `size` across, its line `stroke` wide: the whole circle in `track`, and a quarter of it
/// in `arc`, `turned` of the way round from the top.
pub(super) fn ring(
    size: f32,
    stroke: f32,
    turned: f32,
    track: Hsla,
    arc: Hsla,
) -> impl IntoElement {
    let paint = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let centre = bounds.center();
            let radius = (size - stroke) / 2.;
            let stroked = |from: f32, way: f32, color: Hsla, window: &mut gpui::Window| {
                let at = |way: f32| on_circle(centre, radius, from + way);
                let mut path = PathBuilder::stroke(px(stroke));
                path.move_to(at(0.));
                // Half the way round or less at a time, so a whole ring closes.
                let mut done = 0.;
                while done < way {
                    done += (way - done).min(0.5);
                    path.arc_to(point(px(radius), px(radius)), px(0.), false, true, at(done));
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            };
            stroked(0., 1., track, window);
            stroked(turned, 0.25, arc, window);
        },
    )
    .size_full();
    div().flex_none().size(px(size)).child(paint)
}

/// The point `way` round a circle from its top, clockwise.
fn on_circle(centre: Point<Pixels>, radius: f32, way: f32) -> Point<Pixels> {
    let angle = TAU * way;
    centre + point(px(radius * angle.sin()), px(-radius * angle.cos()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A quarter turn from the top is the circle's right side, and a whole turn is the top again.
    #[test]
    fn the_arc_starts_at_the_top_and_turns_clockwise() {
        let centre = point(px(10.), px(10.));
        let near = |a: Point<Pixels>, b: Point<Pixels>| {
            (f32::from(a.x) - f32::from(b.x)).abs() < 0.01
                && (f32::from(a.y) - f32::from(b.y)).abs() < 0.01
        };
        assert!(near(on_circle(centre, 5., 0.), point(px(10.), px(5.))));
        assert!(near(on_circle(centre, 5., 0.25), point(px(15.), px(10.))));
        assert!(near(on_circle(centre, 5., 1.), point(px(10.), px(5.))));
    }
}
