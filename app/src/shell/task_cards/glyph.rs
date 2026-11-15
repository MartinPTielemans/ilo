//! A task's state in the favicon's column: one shape that moves into the next. A ring in Signal
//! round a dot that circles it while the task works; asking, the dot stops where it is and goes to
//! the middle, grown, as the ring fills in; done, the ring shrinks away and the dot goes for a tick
//! drawing in; stopped, the dot goes and the ring shrinks, in the ink. Working again, the dot
//! circles on from where it stopped. With motion reduced each change is a crossfade and the dot
//! holds still on the ring.

use web_time::Instant;

use gpui::{
    BorderStyle, Bounds, Hsla, PathBuilder, Pixels, Point, Window, fill, point, px, quad,
    transparent_black,
};

use super::motion::{Curve, Ease};
use crate::agents::task::Dot;
use crate::tokens::shell::task::motion::{
    CROSSFADE, DOT_FADE, ORBIT, RING as RING_MOTION, TICK, TICK_DELAY,
};
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

/// How the glyph's parts look in a state, or on their way between two.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Look {
    /// The ring's alpha and its size, and how far its colour has gone from Signal to the ink.
    ring: f32,
    scale: f32,
    grey: f32,
    /// How far the dot has gone from the ring to the middle, its size, and its alpha.
    inward: f32,
    dot_scale: f32,
    dot: f32,
    /// How much of the tick is drawn.
    tick: f32,
}

impl Look {
    fn of(state: State) -> Self {
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

/// The glyph of one task, which eases between its states.
pub(super) struct Glyph {
    state: State,
    ring: Ease,
    scale: Ease,
    grey: Ease,
    inward: Ease,
    dot_scale: Ease,
    dot: Ease,
    tick: Ease,
    /// Where the dot is round the ring, in turns from the top, as it last stopped, and since when
    /// it circles on from there.
    turns: f32,
    circling: Option<Instant>,
    /// With motion reduced, the look it crossfades from and when that began.
    from: Option<(Look, Instant)>,
}

impl Glyph {
    /// At rest in `state`, circling already if it works.
    pub(super) fn new(state: State, now: Instant, still: bool) -> Self {
        let look = Look::of(state);
        Self {
            state,
            ring: Ease::rest(look.ring),
            scale: Ease::rest(look.scale),
            grey: Ease::rest(look.grey),
            inward: Ease::rest(look.inward),
            dot_scale: Ease::rest(look.dot_scale),
            dot: Ease::rest(look.dot),
            tick: Ease::rest(look.tick),
            turns: 0.,
            circling: (state == State::Working && !still).then_some(now),
            from: None,
        }
    }

    pub(super) fn state(&self) -> State {
        self.state
    }

    /// Goes to `state`: each part eases to its look, or, with motion reduced, the whole crossfades.
    pub(super) fn set(&mut self, state: State, now: Instant, still: bool) {
        if state == self.state {
            return;
        }
        if self.state == State::Working {
            self.turns = self.turns_at(now);
            self.circling = None;
        }
        if state == State::Working && !still {
            self.circling = Some(now);
        }
        let to = Look::of(state);
        if still {
            self.from = Some((self.look_at(now), now));
            *self = Self {
                from: self.from,
                turns: self.turns,
                circling: None,
                ..Self::new(state, now, true)
            };
            return;
        }
        self.state = state;
        self.ring.go(to.ring, now, RING_MOTION, 0., Curve::Ease);
        self.scale.go(to.scale, now, RING_MOTION, 0., Curve::Ease);
        self.grey.go(to.grey, now, RING_MOTION, 0., Curve::Ease);
        self.inward.go(to.inward, now, RING_MOTION, 0., Curve::Ease);
        self.dot_scale
            .go(to.dot_scale, now, RING_MOTION, 0., Curve::Ease);
        self.dot.go(to.dot, now, DOT_FADE, 0., Curve::Linear);
        self.tick.go(to.tick, now, TICK, TICK_DELAY, Curve::Ease);
    }

    /// Where the dot is round the ring at `now`, in turns.
    pub(super) fn turns_at(&self, now: Instant) -> f32 {
        let circled = self.circling.map_or(0., |since| {
            now.saturating_duration_since(since).as_secs_f32() / ORBIT
        });
        (self.turns + circled).fract()
    }

    pub(super) fn look_at(&self, now: Instant) -> Look {
        Look {
            ring: self.ring.at(now),
            scale: self.scale.at(now),
            grey: self.grey.at(now),
            inward: self.inward.at(now),
            dot_scale: self.dot_scale.at(now),
            dot: self.dot.at(now),
            tick: self.tick.at(now),
        }
    }

    /// Whether a change is still on its way at `now`, which a frame each draws.
    pub(super) fn moving(&self, now: Instant) -> bool {
        let crossfading = self.from.is_some_and(|(_, since)| {
            now.saturating_duration_since(since).as_secs_f32() < CROSSFADE
        });
        crossfading
            || [
                self.ring,
                self.scale,
                self.grey,
                self.inward,
                self.dot_scale,
                self.dot,
                self.tick,
            ]
            .iter()
            .any(|part| part.moving(now))
    }

    /// Whether the dot circles, which the foot's clock draws.
    pub(super) fn circling(&self) -> bool {
        self.circling.is_some()
    }

    /// What to paint at `now`: the looks, each at its alpha, and where the dot is round the ring.
    pub(super) fn drawn(&self, now: Instant) -> Drawn {
        let look = self.look_at(now);
        let turns = self.turns_at(now);
        let crossfade = self.from.and_then(|(from, since)| {
            let t = now.saturating_duration_since(since).as_secs_f32() / CROSSFADE;
            (t < 1.).then_some((from, t))
        });
        match crossfade {
            Some((from, t)) => Drawn {
                looks: vec![(from, 1. - t), (look, t)],
                turns,
            },
            None => Drawn {
                looks: vec![(look, 1.)],
                turns,
            },
        }
    }
}

/// The glyph as one frame paints it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Drawn {
    looks: Vec<(Look, f32)>,
    turns: f32,
}

/// The tick's three points in the glyph's box, and its length drawn in, as a dash that long.
const TICK_POINTS: [(f32, f32); 3] = [(5.2, 9.3), (7.8, 11.8), (12.8, 6.4)];
const TICK_DASH: f32 = 12.;

/// Paints `drawn` in `bounds`, the glyph's box, in Signal, the stopped ring in `stopped`. On the
/// selected plate's fill, which Signal stands out from less, the ring is as much brighter as the
/// working ring's `ring-working-selected` is.
pub(super) fn paint(
    drawn: &Drawn,
    bounds: Bounds<Pixels>,
    signal: Hsla,
    stopped: Hsla,
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
    for &(look, alpha) in &drawn.looks {
        if alpha <= 0. {
            continue;
        }
        let ring = RING * look.scale;
        let colour = mix(signal, stopped, look.grey);
        if look.ring > 0. && ring > 0. {
            let outer = ring + RING_STROKE / 2.;
            let square = Bounds::new(
                point(centre.x - px(outer), centre.y - px(outer)),
                gpui::size(px(2. * outer), px(2. * outer)),
            );
            let colour = colour.opacity((look.ring * lift).min(1.) * alpha);
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
            let angle = drawn.turns * std::f32::consts::TAU;
            let out = RING * (1. - look.inward);
            let middle = point(
                centre.x + px(out * angle.sin()),
                centre.y - px(out * angle.cos()),
            );
            dot(
                middle,
                DOT * look.dot_scale,
                signal.opacity(look.dot * alpha),
                window,
            );
        }
        if look.tick > 0. {
            let colour = signal.opacity(alpha);
            let mut left = TICK_DASH * look.tick;
            let mut path = PathBuilder::stroke(px(TICK_STROKE));
            let (x, y) = TICK_POINTS[0];
            path.move_to(at(x, y));
            let mut ends = vec![at(x, y)];
            for pair in TICK_POINTS.windows(2) {
                let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
                let length = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
                let way = (left / length).min(1.);
                let end = at(x0 + (x1 - x0) * way, y0 + (y1 - y0) * way);
                path.line_to(end);
                ends.push(end);
                left -= length;
                if left <= 0. {
                    break;
                }
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
    use std::time::{Duration, Instant};

    use super::{Glyph, Look, State};
    use crate::tokens::shell::task::{DOT_ASKS, RING_WORKING};

    /// Working, the dot circles round a faint ring. Asked, it stops where it was and goes to the
    /// middle, grown, in a full ring; answered, it circles on from where it stopped. Done draws a
    /// tick as the ring and the dot go; stopped greys the ring and the dot goes.
    #[test]
    fn the_glyph_moves_from_state_to_state() {
        let now = Instant::now();
        let mut glyph = Glyph::new(State::Working, now, false);
        assert!(glyph.circling());
        assert_eq!(glyph.look_at(now).ring, RING_WORKING);
        let asked = now + Duration::from_millis(350);
        let stopped_at = glyph.turns_at(asked);
        assert!(stopped_at > 0.2 && stopped_at < 0.3, "{stopped_at}");
        glyph.set(State::Asks, asked, false);
        assert!(!glyph.circling());
        assert!(glyph.moving(asked + Duration::from_millis(100)));
        let settled = asked + Duration::from_secs(1);
        assert!(!glyph.moving(settled));
        assert_eq!(glyph.look_at(settled), Look::of(State::Asks));
        assert_eq!(glyph.look_at(settled).dot_scale, DOT_ASKS);
        assert_eq!(
            glyph.turns_at(settled),
            stopped_at,
            "the dot moved while it asked"
        );

        glyph.set(State::Working, settled, false);
        let on = settled + Duration::from_millis(140);
        assert!(
            (glyph.turns_at(on) - (stopped_at + 0.1)).abs() < 0.001,
            "it didn't circle on"
        );

        glyph.set(State::Done, on, false);
        let early = on + Duration::from_millis(60);
        assert_eq!(
            glyph.look_at(early).tick,
            0.,
            "the tick draws after the ring starts going"
        );
        let done = on + Duration::from_secs(1);
        assert_eq!(glyph.look_at(done), Look::of(State::Done));
        glyph.set(State::Stopped, done, false);
        let look = glyph.look_at(done + Duration::from_secs(1));
        assert_eq!((look.grey, look.dot, look.tick), (1., 0., 0.));
    }

    /// With motion reduced, the dot holds still on the ring, and a change crossfades the looks.
    #[test]
    fn with_motion_reduced_the_glyph_crossfades_and_holds_still() {
        let now = Instant::now();
        let mut glyph = Glyph::new(State::Working, now, true);
        assert!(!glyph.circling());
        assert_eq!(glyph.turns_at(now + Duration::from_secs(1)), 0.);
        glyph.set(State::Asks, now, true);
        let drawn = glyph.drawn(now + Duration::from_millis(100));
        assert_eq!(drawn.looks.len(), 2, "no crossfade");
        assert_eq!(drawn.looks[0].0, Look::of(State::Working));
        assert_eq!(drawn.looks[1].0, Look::of(State::Asks));
        assert!((drawn.looks[0].1 + drawn.looks[1].1 - 1.).abs() < 0.001);
        let later = now + Duration::from_secs(1);
        assert_eq!(glyph.drawn(later).looks, vec![(Look::of(State::Asks), 1.)]);
        assert!(!glyph.moving(later));
        glyph.set(State::Working, later, true);
        assert!(!glyph.circling());
    }
}
