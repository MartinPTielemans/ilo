//! The sign above the composer (spec-thread-stage.md, "A wait on the user: the mark moves, the work
//! doesn't"): the product's mark, there while the thread works or waits on the user, and nowhere
//! else. Which of the two is read from the thread's own state as the app holds it, never from what
//! an assistant says.
use crate::agents::task::{Conversation, Live};
use crate::tokens::shell::conversation::sign::{LAP, WORKING_GAP};

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
        Sign::Waiting => (0., 0.),
    }
}
