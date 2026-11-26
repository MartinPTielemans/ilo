//! A finished task's mark at rest, as the Library and an ended card show it, and a card's mark.
//! The task glyph: asking, a grown dot in the middle of a full ring, both in the title's ink;
//! done, a tick in the state line's ink; stopped, a grey ring. At work it's the card's ring, held
//! at the top. No mark is green.

use gpui::{
    BorderStyle, Bounds, Hsla, PathBuilder, Pixels, Point, Window, fill, point, px, quad,
    transparent_black,
};

use crate::agents::task::Dot;
use crate::tokens::shell::task::{
    DOT, DOT_ASKS, DOT_DONE, RING, RING_DONE, RING_STOPPED, RING_STROKE, RING_WORKING,
    RING_WORKING_SELECTED, TICK_STROKE,
};

/// What the glyph says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Working,
    Asks,
    Done,
    Stopped,
}

impl From<Dot> for State {
    fn from(dot: Dot) -> Self {
        match dot {
            Dot::Working => State::Working,
            Dot::Waiting => State::Asks,
            Dot::Done => State::Done,
            Dot::Ended => State::Stopped,
        }
    }
}

/// How the glyph's parts look in a state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Look {
    /// The ring's alpha and its size.
    ring: f32,
    scale: f32,
    /// How far the ring's and the dot's ink is from the title's to the stopped ring's.
    grey: f32,
    /// How far the dot is from the ring to the middle, its size, and its alpha.
    inward: f32,
    dot_scale: f32,
    dot: f32,
    /// How much of the tick is drawn.
    tick: f32,
}

impl Look {
    pub(super) fn of(state: State) -> Self {
        let rest = Look {
            ring: RING_WORKING,
            scale: 1.,
            grey: 0.,
            inward: 0.,
            dot_scale: 1.,
            dot: 1.,
            tick: 0.,
        };
        match state {
            State::Working => rest,
            State::Asks => Look {
                ring: 1.,
                inward: 1.,
                dot_scale: DOT_ASKS,
                ..rest
            },
            State::Done => Look {
                ring: 0.,
                scale: RING_DONE,
                // The ring greys as it goes, and comes back from grey.
                grey: 1.,
                inward: 1.,
                dot_scale: DOT_DONE,
                dot: 0.,
                tick: 1.,
                ..rest
            },
            State::Stopped => Look {
                ring: 1.,
                scale: RING_STOPPED,
                grey: 1.,
                dot: 0.,
                ..rest
            },
        }
    }
}

/// The tick's three points in the glyph's box.
const TICK_POINTS: [(f32, f32); 3] = [(5.2, 9.3), (7.8, 11.8), (12.8, 6.4)];

/// The glyph's colours: asking, the ring and the dot are in the title's ink, Done's tick is in the
/// state line's, and the stopped ring has its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Inks {
    pub(super) title: Hsla,
    pub(super) status: Hsla,
    pub(super) stopped: Hsla,
}

impl Inks {
    /// The ring's and the dot's colour in `look`.
    fn of(&self, look: Look) -> Hsla {
        mix(self.title, self.stopped, look.grey)
    }
}

/// The mark a task at rest in `state` draws in place of today's glyph, as the Library shows it: at
/// work the card's ring, held at the top; none in any other state.
pub(super) fn at_rest(state: State) -> Option<Mark> {
    (state == State::Working).then_some(Mark::Ring { turns: 0. })
}

/// Paints `look` in `bounds`, the glyph's box, in `inks`. On the selected plate's fill the ring is as
/// much brighter as the working ring's `ring-working-selected` is.
pub(super) fn paint(
    look: Look,
    bounds: Bounds<Pixels>,
    inks: Inks,
    selected: bool,
    window: &mut Window,
) {
    let lift = if selected {
        RING_WORKING_SELECTED / RING_WORKING
    } else {
        1.
    };
    let centre = bounds.center();
    let at = |x: f32, y: f32| point(bounds.origin.x + px(x), bounds.origin.y + px(y));
    let ring = RING * look.scale;
    let colour = inks.of(look);
    if look.ring > 0. && ring > 0. {
        let outer = ring + RING_STROKE / 2.;
        let square = Bounds::new(
            point(centre.x - px(outer), centre.y - px(outer)),
            gpui::size(px(2. * outer), px(2. * outer)),
        );
        let colour = colour.opacity((look.ring * lift).min(1.));
        window.paint_quad(quad(
            square,
            px(outer),
            transparent_black(),
            px(RING_STROKE),
            colour,
            BorderStyle::Solid,
        ));
    }
    if look.dot > 0. {
        // The dot stands at the ring's top, or in the middle once it has gone in.
        let middle = point(centre.x, centre.y - px(RING * (1. - look.inward)));
        dot(
            middle,
            DOT * look.dot_scale,
            colour.opacity(look.dot),
            window,
        );
    }
    if look.tick > 0. {
        let colour = inks.status;
        let ends = TICK_POINTS.map(|(x, y)| at(x, y));
        let mut path = PathBuilder::stroke(px(TICK_STROKE));
        path.move_to(ends[0]);
        for &end in &ends[1..] {
            path.line_to(end);
        }
        if let Ok(path) = path.build() {
            window.paint_path(path, colour);
        }
        // Round caps and join, as the stroke has none of its own.
        for end in ends {
            dot(end, TICK_STROKE / 2., colour, window);
        }
    }
}

/// A thread card's mark (spec-thread-card), from its task's state alone: at work the turning ring,
/// waiting the paused disc, done the check in its circle. An ended task's is today's glyph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Mark {
    /// `turns` of the way round from the top.
    Ring {
        turns: f32,
    },
    /// Its circle stronger while its conversation hasn't shown since it finished.
    Check {
        unseen: bool,
    },
    Pause,
}

/// A card mark's colours: the window's ink, and the label's on a filled button.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MarkInks {
    pub(super) ink: Hsla,
    pub(super) on_primary: Hsla,
}

/// Paints `mark` in `bounds`, the card's glyph box.
pub(super) fn paint_mark(mark: Mark, bounds: Bounds<Pixels>, inks: MarkInks, window: &mut Window) {
    use crate::tokens::shell::task_card::{
        CHECK_STROKE, CIRCLE, CIRCLE_UNSEEN, GLYPH as BOX, PAUSE, PAUSE_BOTTOM, PAUSE_LEFT,
        PAUSE_RIGHT, PAUSE_STROKE, PAUSE_TOP, RING as CARD_RING, RING_STROKE as CARD_STROKE, TRACK,
    };
    let centre = bounds.center();
    let at = |x: f32, y: f32| point(bounds.origin.x + px(x), bounds.origin.y + px(y));
    let circle = |radius: f32, stroke: f32, colour: Hsla, window: &mut Window| {
        let outer = radius + stroke / 2.;
        let square = Bounds::new(
            point(centre.x - px(outer), centre.y - px(outer)),
            gpui::size(px(2. * outer), px(2. * outer)),
        );
        let ring = quad(
            square,
            px(outer),
            transparent_black(),
            px(stroke),
            colour,
            BorderStyle::Solid,
        );
        window.paint_quad(ring);
    };
    // A stroke through `points` with round caps and joins, as a path's stroke has none.
    let stroke = |points: &[Point<Pixels>], width: f32, colour: Hsla, window: &mut Window| {
        let mut path = PathBuilder::stroke(px(width));
        path.move_to(points[0]);
        for &each in &points[1..] {
            path.line_to(each);
        }
        if let Ok(path) = path.build() {
            window.paint_path(path, colour);
        }
        for &end in points {
            dot(end, width / 2., colour, window);
        }
    };
    match mark {
        Mark::Ring { turns } => {
            circle(CARD_RING, CARD_STROKE, inks.ink.opacity(TRACK), window);
            let on = |turn: f32| {
                let angle = turn * std::f32::consts::TAU;
                point(
                    centre.x + px(CARD_RING * angle.sin()),
                    centre.y - px(CARD_RING * angle.cos()),
                )
            };
            let (from, to) = (on(turns), on(turns + 0.25));
            let mut path = PathBuilder::stroke(px(CARD_STROKE));
            path.move_to(from);
            let radii = point(px(CARD_RING), px(CARD_RING));
            path.arc_to(radii, px(0.), false, true, to);
            if let Ok(path) = path.build() {
                window.paint_path(path, inks.ink);
            }
            for end in [from, to] {
                dot(end, CARD_STROKE / 2., inks.ink, window);
            }
        }
        Mark::Check { unseen } => {
            let alpha = if unseen { CIRCLE_UNSEEN } else { CIRCLE };
            circle(CARD_RING, CHECK_STROKE, inks.ink.opacity(alpha), window);
            // The task glyph's tick, moved from its box into the card's.
            let inset = (crate::tokens::shell::task::GLYPH - BOX) / 2.;
            let points = TICK_POINTS.map(|(x, y)| at(x - inset, y - inset));
            stroke(&points, CHECK_STROKE, inks.ink, window);
        }
        Mark::Pause => {
            dot(centre, PAUSE, inks.ink, window);
            for x in [PAUSE_LEFT, PAUSE_RIGHT] {
                let bar = [at(x, PAUSE_TOP), at(x, PAUSE_BOTTOM)];
                stroke(&bar, PAUSE_STROKE, inks.on_primary, window);
            }
        }
    }
}

fn dot(middle: Point<Pixels>, radius: f32, colour: Hsla, window: &mut Window) {
    let square = Bounds::new(
        point(middle.x - px(radius), middle.y - px(radius)),
        gpui::size(px(2. * radius), px(2. * radius)),
    );
    window.paint_quad(fill(square, colour).corner_radii(px(radius)));
}

/// `from` gone `way` of the way to `to`.
fn mix(from: Hsla, to: Hsla, way: f32) -> Hsla {
    if way <= 0. {
        return from;
    }
    if way >= 1. {
        return to;
    }
    let (a, b) = (gpui::Rgba::from(from), gpui::Rgba::from(to));
    let lerp = |a: f32, b: f32| a + (b - a) * way;
    gpui::Rgba {
        r: lerp(a.r, b.r),
        g: lerp(a.g, b.g),
        b: lerp(a.b, b.b),
        a: lerp(a.a, b.a),
    }
    .into()
}

#[cfg(test)]
mod tests {
    use gpui::{Hsla, hsla};

    use super::{Inks, Look, Mark, State, at_rest};

    /// No mark is green (spec-bar-composer, "The greens still worn"): at work a task at rest draws
    /// the card's ring, held at the top, in the window's ink; asking, the ring and the dot are in
    /// the title's ink, and the stopped ring is in its own.
    #[test]
    fn no_glyph_is_green_and_work_is_the_cards_ring() {
        let ink = |h: f32| -> Hsla { hsla(h, 0.5, 0.5, 1.) };
        let inks = Inks {
            title: ink(0.),
            status: ink(0.6),
            stopped: ink(0.8),
        };
        assert_eq!(at_rest(State::Working), Some(Mark::Ring { turns: 0. }));
        for state in [State::Asks, State::Done, State::Stopped] {
            assert_eq!(at_rest(state), None, "{state:?} keeps today's glyph");
        }
        assert_eq!(inks.of(Look::of(State::Asks)), inks.title);
        assert_eq!(inks.of(Look::of(State::Stopped)), inks.stopped);
        // Done has neither ring nor dot, so its tick, in the state line's ink, is all of it.
        let done = Look::of(State::Done);
        assert_eq!((done.ring, done.dot, done.tick), (0., 0., 1.));
    }
}
