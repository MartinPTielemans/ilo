//! The sign above the composer (spec-thread-stage.md, "A wait on the user: the mark moves, the work
//! doesn't"): the product's mark, there while the thread works or waits on the user, and nowhere
//! else. Which of the two is read from the thread's own state as the app holds it, never from what
//! an assistant says.
use crate::agents::task::{Conversation, Live};
use crate::shell::cubic_bezier;
use crate::tokens::shell::conversation::sign::{HOLD, LAP, OPEN, REST, WAITING_GAP, WORKING_GAP};
use crate::tokens::shell::task::motion::EASE;

/// What the sign says of its thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sign {
    /// A step, or the turn, is under way.
    Working,
    /// The thread waits on the user: a page handed to them or taken over, a step ilo refused, or
    /// its own question.
    Waiting,
}

impl Sign {
    /// The sign for `shown`, none once its thread can't be stopped: it has ended, or it is a kept
    /// log read back, which never draws the sign.
    pub fn of(shown: &Conversation) -> Option<Self> {
        shown.stoppable.then_some(match shown.live {
            Live::Working => Self::Working,
            Live::Refused | Live::Handed | Live::Yours { .. } | Live::Asked => Self::Waiting,
        })
    }
}

/// Where the gap in the sign's cord is `gone` seconds into its state: how far along the cord it
/// starts and how much of the cord it leaves out, each a fraction of the cord.
pub(crate) fn gap(sign: Sign, gone: f32) -> (f32, f32) {
    match sign {
        // At an even pace, from where the cord starts.
        Sign::Working => ((gone / LAP).fract(), WORKING_GAP),
        // Over and over: at the top of the upper right loop a gap opens, holds, closes, and the
        // knot rests whole, from the gap's opening as the wait starts.
        Sign::Waiting => {
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
