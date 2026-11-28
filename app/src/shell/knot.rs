//! The knot: the product's mark at its small form as the sign of a thread at work or waiting on
//! the user (spec-thread-stage.md, "A wait on the user: the mark moves, the work doesn't"), one
//! element wherever a thread's state is drawn with it. Which of its two motions it moves in is read
//! from the thread's own state as the app holds it, never from what an assistant says, and it
//! moves only where it can be seen to, on the frames something already asks for.
use std::time::Instant;

use gpui::{App, Styled as _, Svg, Window};

use crate::agents::task::Live;
use crate::palette::Palette;
use crate::shell::cubic_bezier;
use crate::tokens::shell::conversation::sign::{HOLD, LAP, OPEN, REST, WAITING_GAP, WORKING_GAP};
use crate::tokens::shell::task::motion::EASE;

/// What the knot says of its thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Knot {
    /// A step, or the turn, is under way.
    Working,
    /// The thread waits on the user: a page handed to them or taken over, a step ilo refused, or
    /// its own question.
    Waiting,
}

impl Knot {
    /// The knot of a thread `live` says this of, none once it can't be stopped: it has ended, or
    /// it is a kept log read back, which never draws it moving.
    pub fn of(stoppable: bool, live: &Live) -> Option<Self> {
        stoppable.then_some(match live {
            Live::Working => Self::Working,
            Live::Refused | Live::Handed | Live::Yours { .. } | Live::Asked => Self::Waiting,
        })
    }
}

/// When a knot came to say what it says, which its motion runs from.
#[derive(Default)]
pub(crate) struct Timed(Option<(Knot, Instant)>);

impl Timed {
    /// The gap in `knot`'s cord drawn at `now`, timed from when it came to be that knot, or none
    /// while it stands whole and still: with no knot, or where it can't be seen to move (`moves`).
    pub fn gap(&mut self, knot: Option<Knot>, now: Instant, moves: bool) -> Option<(f32, f32)> {
        if self.0.map(|(was, _)| was) != knot {
            self.0 = knot.map(|knot| (knot, now));
        }
        let (knot, since) = self.0.filter(|_| moves)?;
        Some(gap(knot, now.duration_since(since).as_secs_f32()))
    }
}

/// Whether a knot drawn now in `window` can be seen to move: the system shows the window and
/// motion isn't reduced. Where it can't, it stands whole and asks for no frame.
pub(crate) fn moves(window: &Window, cx: &App) -> bool {
    window.is_visible() && !crate::motion::still(cx)
}

/// The knot `size` across, saying `says`: in the mark's ink, or amber while its thread waits on
/// the user (#4402), with `gap` left out of its cord, or whole.
pub(crate) fn knot(
    size: f32,
    palette: &Palette,
    says: Option<Knot>,
    gap: Option<(f32, f32)>,
) -> Svg {
    let mark = crate::product::mark(size, palette);
    let mark = match says {
        Some(Knot::Waiting) => mark.text_color(palette.needs_you),
        _ => mark,
    };
    match gap {
        Some((at, gap)) => mark.path(crate::product::gapped_mark_path(at, gap)),
        None => mark,
    }
}

/// Where the gap in the knot's cord is `gone` seconds into its state: how far along the cord it
/// starts and how much of the cord it leaves out, each a fraction of the cord.
pub(crate) fn gap(knot: Knot, gone: f32) -> (f32, f32) {
    match knot {
        // At an even pace, from where the cord starts.
        Knot::Working => ((gone / LAP).fract(), WORKING_GAP),
        // Over and over: at the top of the upper right loop a gap opens, holds, closes, and the
        // knot rests whole, from the gap's opening as the wait starts.
        Knot::Waiting => {
            let at = gone.rem_euclid(OPEN + HOLD + OPEN + REST);
            let open = match at {
                at if at < OPEN => ease(at / OPEN),
                at if at < OPEN + HOLD => 1.,
                at if at < OPEN + HOLD + OPEN => 1. - ease((at - OPEN - HOLD) / OPEN),
                _ => 0.,
            };
            let gap = WAITING_GAP * open;
            (crate::product::crown() - gap / 2., gap)
        }
    }
}

/// `x` of the way through a motion, on the task's ease.
fn ease(x: f32) -> f32 {
    let [x1, y1, x2, y2] = EASE;
    cubic_bezier(x1, y1, x2, y2, x)
}
