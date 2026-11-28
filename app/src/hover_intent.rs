//! When a surface that hovering opens comes, one rule for every one in the app (#4108): the
//! pointer rests on a thing for the surface's delay, then it opens; while it's open, or within its
//! grace after it closed, the next thing the pointer comes onto opens at once, as the pointer
//! crosses from one to the next; and only one thing's is open at a time. Lifted from the tooltip's
//! and the link target's, the two that had the grace.
//!
//! It's a state, not a timer: it says when the surface should wake, and the surface schedules that
//! one wake-up, so nothing runs while nothing is hovered and an idle browser is never woken.
//!
//! How to compose it: give the surface an `Intent` with its delay (and `with_grace` if it has
//! one). When the pointer comes onto a thing, `enter`: on `Next::Open` show it now, on
//! `Next::Wait` keep what to show and wake after that long, then show it if `due`, or else wake
//! again after what's `early`. `restart` waits
//! the delay again from now, as the tooltip does on each move. When the pointer leaves before it
//! opened, `leave`; when the surface goes, `close`.

use std::time::{Duration, Instant};

/// Where a hovering surface is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Nothing hovered, or the pointer left before the delay was up.
    Idle,
    /// The pointer is on a thing, since this instant.
    Waiting(Instant),
    Open,
}

/// What the surface does after the pointer came onto a thing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    /// Show it now.
    Open,
    /// Wake after this long, then show it if it's `due`.
    Wait(Duration),
}

/// One hovering surface's intent: its delay, its grace, and where it is.
#[derive(Clone, Debug)]
pub struct Intent {
    delay: Duration,
    grace: Duration,
    phase: Phase,
    /// When it last closed while open.
    closed: Option<Instant>,
}

impl Intent {
    /// A surface that opens once the pointer has rested `delay` on a thing, and at once only while
    /// it's open.
    pub const fn new(delay: Duration) -> Self {
        Self {
            delay,
            grace: Duration::ZERO,
            phase: Phase::Idle,
            closed: None,
        }
    }

    /// And at once for `grace` after it closed.
    pub const fn with_grace(self, grace: Duration) -> Self {
        Self { grace, ..self }
    }

    /// The pointer came onto a thing at `now`: it opens at once while the surface is open or
    /// within its grace after it closed, and otherwise waits the delay from now.
    pub fn enter(&mut self, now: Instant) -> Next {
        let soon = self
            .closed
            .is_some_and(|closed| now.duration_since(closed) < self.grace);
        if self.phase == Phase::Open || soon {
            self.phase = Phase::Open;
            return Next::Open;
        }
        self.phase = Phase::Waiting(now);
        Next::Wait(self.delay)
    }

    /// The pointer moved on the thing it waits on: it waits the delay again from `now`. Whether
    /// the surface should wake, and after how long.
    pub fn restart(&mut self, now: Instant) -> Option<Duration> {
        matches!(self.phase, Phase::Waiting(_)).then(|| {
            self.phase = Phase::Waiting(now);
            self.delay
        })
    }

    /// The surface woke at `now`: whether the delay is up, so it opens.
    pub fn due(&mut self, now: Instant) -> bool {
        let due =
            matches!(self.phase, Phase::Waiting(since) if now.duration_since(since) >= self.delay);
        if due {
            self.phase = Phase::Open;
        }
        due
    }

    /// The surface woke at `now` and it wasn't `due`: while the pointer still waits on the thing,
    /// how much longer, as a platform's timer can wake it a little early (Windows' by up to its
    /// tick, #4108's Windows overlays run), and it waits that out rather than never opening.
    pub fn early(&self, now: Instant) -> Option<Duration> {
        self.wake().map(|at| at.saturating_duration_since(now))
    }

    /// The pointer left the thing before its surface showed it: nothing opens, and no grace starts.
    pub fn leave(&mut self) {
        self.phase = Phase::Idle;
    }

    /// The surface went at `now`, or the thing it waited on did.
    pub fn close(&mut self, now: Instant) {
        if self.phase == Phase::Open {
            self.closed = Some(now);
        }
        self.phase = Phase::Idle;
    }

    pub fn is_open(&self) -> bool {
        self.phase == Phase::Open
    }

    pub fn is_waiting(&self) -> bool {
        matches!(self.phase, Phase::Waiting(_))
    }

    /// When the surface should next wake, if it should: only while the pointer waits on a thing.
    pub fn wake(&self) -> Option<Instant> {
        match self.phase {
            Phase::Waiting(since) => Some(since + self.delay),
            Phase::Idle | Phase::Open => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DELAY: Duration = Duration::from_millis(500);
    const GRACE: Duration = Duration::from_millis(400);
    const MS: Duration = Duration::from_millis(1);

    fn intent() -> (Intent, Instant) {
        (Intent::new(DELAY).with_grace(GRACE), Instant::now())
    }

    /// A wake before the delay is up says how much is left while the pointer waits, and nothing
    /// once it's open or the pointer has left.
    #[test]
    fn an_early_wake_waits_out_what_is_left() {
        let (mut intent, at) = intent();
        intent.enter(at);
        assert!(!intent.due(at + DELAY - 3 * MS));
        assert_eq!(intent.early(at + DELAY - 3 * MS), Some(3 * MS));
        assert!(intent.due(at + DELAY));
        assert_eq!(intent.early(at + DELAY), None, "an open one waits on");
        intent.close(at + DELAY);
        intent.enter(at + DELAY + GRACE);
        intent.leave();
        assert_eq!(
            intent.early(at + DELAY + GRACE),
            None,
            "a left one waits on"
        );
    }

    #[test]
    fn it_opens_after_its_delay_and_not_before() {
        let (mut intent, at) = intent();
        assert_eq!(intent.enter(at), Next::Wait(DELAY));
        assert_eq!(intent.wake(), Some(at + DELAY));
        assert!(!intent.due(at + DELAY - MS), "it opened before its delay");
        assert!(intent.due(at + DELAY));
        assert!(intent.is_open());
    }

    #[test]
    fn a_move_waits_the_delay_again() {
        let (mut intent, at) = intent();
        intent.enter(at);
        assert_eq!(intent.restart(at + DELAY / 2), Some(DELAY));
        assert!(
            !intent.due(at + DELAY),
            "it opened a delay after the pointer came, not rested"
        );
        assert!(intent.due(at + DELAY / 2 + DELAY));
        assert_eq!(
            intent.restart(at + 2 * DELAY),
            None,
            "an open one waited again"
        );
    }

    #[test]
    fn right_after_another_closed_it_opens_at_once_within_its_grace_and_not_after() {
        let (mut intent, at) = intent();
        intent.enter(at);
        intent.due(at + DELAY);
        let closed = at + 2 * DELAY;
        intent.close(closed);
        assert_eq!(intent.enter(closed + GRACE - MS), Next::Open);
        intent.close(closed + GRACE);
        let later = closed + GRACE + GRACE;
        assert_eq!(
            intent.enter(later),
            Next::Wait(DELAY),
            "it opened at once past its grace"
        );
    }

    #[test]
    fn without_a_grace_only_an_open_one_opens_the_next_at_once() {
        let at = Instant::now();
        let mut intent = Intent::new(DELAY);
        intent.enter(at);
        intent.due(at + DELAY);
        intent.close(at + DELAY);
        assert_eq!(intent.enter(at + DELAY), Next::Wait(DELAY));
    }

    /// One thing's is open at a time: the pointer coming onto a second while the first's is open
    /// opens the second's in its place, at once.
    #[test]
    fn a_second_thing_takes_the_firsts_place() {
        let (mut intent, at) = intent();
        intent.enter(at);
        intent.due(at + DELAY);
        assert_eq!(intent.enter(at + DELAY + MS), Next::Open);
        assert!(intent.is_open());
        assert_eq!(intent.wake(), None);
    }

    #[test]
    fn leaving_before_the_delay_opens_nothing() {
        let (mut intent, at) = intent();
        intent.enter(at);
        intent.leave();
        assert!(!intent.due(at + DELAY), "it opened after the pointer left");
        assert!(!intent.is_open());
        assert_eq!(
            intent.enter(at + DELAY),
            Next::Wait(DELAY),
            "a leave counted as a close for the grace"
        );
    }

    /// Nothing hovered, it never asks to wake: not idle, not open, not after a leave or a close.
    #[test]
    fn no_wake_up_while_nothing_is_hovered() {
        let (mut intent, at) = intent();
        assert_eq!(intent.wake(), None);
        intent.enter(at);
        intent.leave();
        assert_eq!(intent.wake(), None);
        intent.enter(at);
        intent.close(at);
        assert_eq!(intent.wake(), None);
        intent.enter(at);
        intent.due(at + DELAY);
        assert_eq!(intent.wake(), None);
        intent.close(at + DELAY);
        assert_eq!(intent.wake(), None);
        assert_eq!(intent.restart(at + DELAY), None);
    }
}
