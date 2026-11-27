//! What keeps a surface that leaves by a timer, such as the toast, one rule for every one in the
//! app (#4108): each reason it's held (the pointer on it, the keyboard in it, the app away) is set
//! and cleared by itself, its time stops while any is set, and goes on once none is, with the time
//! already counted kept, so a toast the user looked away from still has the rest of its time when
//! they come back.
//!
//! It's a state, not a timer, as `hover_intent` is: it says when the surface should wake, and the
//! surface schedules that one wake-up, so nothing runs while it's held and an idle browser is
//! never woken.
//!
//! How to compose it: keep one `Hold` for the surface, `set` each reason as it comes and goes, and
//! `start` it with each showing's time, which keeps the reasons, as the pointer still on the
//! surface or the app still away hold the next showing too. After either, schedule one wake-up at
//! `wake`, dropping the one before, and leave on waking. Every instant is the executor's
//! (`BackgroundExecutor::now`), whose clock a test moves.

use std::time::{Duration, Instant};

/// Why a surface is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The pointer is on it.
    Pointer,
    /// The keyboard is in it.
    Focus,
    /// The app isn't the active one.
    AppAway,
}

impl Reason {
    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// A timed surface's time: how long it shows, how much of that has been counted, and what holds
/// it. Untimed until `start`ed with a time, as a toast that waits for its answer is.
#[derive(Clone, Debug, Default)]
pub struct Hold {
    time: Option<Duration>,
    /// Counted before `since`, or all of it while held.
    counted: Duration,
    /// Since when it counts, while nothing holds it.
    since: Option<Instant>,
    reasons: u8,
}

impl Hold {
    /// Counts `time` afresh from `now`, or nothing with no time, keeping what holds it.
    pub fn start(&mut self, time: Option<Duration>, now: Instant) {
        self.time = time;
        self.counted = Duration::ZERO;
        self.since = (!self.is_held()).then_some(now);
    }

    /// `reason` holds it (`held`) or no longer does, at `now`. Its time stops as the first reason
    /// comes and goes on as the last one goes.
    pub fn set(&mut self, reason: Reason, held: bool, now: Instant) {
        let was = self.is_held();
        match held {
            true => self.reasons |= reason.bit(),
            false => self.reasons &= !reason.bit(),
        }
        match (was, self.is_held()) {
            (false, true) => {
                if let Some(since) = self.since.take() {
                    self.counted += now.saturating_duration_since(since);
                }
            }
            (true, false) => self.since = Some(now),
            _ => {}
        }
    }

    pub fn is_held(&self) -> bool {
        self.reasons != 0
    }

    /// When its time is up, where the surface wakes and leaves: none while it's held, nor without
    /// a time.
    pub fn wake(&self) -> Option<Instant> {
        Some(self.since? + self.time?.saturating_sub(self.counted))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIME: Duration = Duration::from_secs(5);
    const S: Duration = Duration::from_secs(1);

    fn started(at: Instant) -> Hold {
        let mut hold = Hold::default();
        hold.start(Some(TIME), at);
        hold
    }

    #[test]
    fn it_wakes_after_its_time() {
        let at = Instant::now();
        let hold = started(at);
        assert_eq!(hold.wake(), Some(at + TIME));
    }

    #[test]
    fn without_a_time_it_never_wakes() {
        let at = Instant::now();
        let mut hold = Hold::default();
        hold.start(None, at);
        hold.set(Reason::Pointer, true, at);
        hold.set(Reason::Pointer, false, at + S);
        assert_eq!(hold.wake(), None);
    }

    /// Held at 2 s for 10 s, it has 3 s left when let go: the time it had counted is kept.
    #[test]
    fn a_hold_stops_its_time_and_letting_go_keeps_what_was_counted() {
        let at = Instant::now();
        let mut hold = started(at);
        hold.set(Reason::Pointer, true, at + 2 * S);
        assert_eq!(hold.wake(), None, "a held one asked to wake");
        hold.set(Reason::Pointer, false, at + 12 * S);
        assert_eq!(hold.wake(), Some(at + 15 * S));
    }

    #[test]
    fn two_reasons_hold_until_both_go() {
        let at = Instant::now();
        let mut hold = started(at);
        hold.set(Reason::Pointer, true, at + S);
        hold.set(Reason::AppAway, true, at + 2 * S);
        hold.set(Reason::Pointer, false, at + 3 * S);
        assert!(hold.is_held(), "one reason going let the other's hold go");
        assert_eq!(hold.wake(), None);
        hold.set(Reason::AppAway, false, at + 9 * S);
        // 1 s counted before the pointer came, so 4 s from here.
        assert_eq!(hold.wake(), Some(at + 13 * S));
    }

    /// A reason set twice, or cleared when it wasn't set, changes nothing.
    #[test]
    fn a_reason_is_set_and_cleared_by_itself() {
        let at = Instant::now();
        let mut hold = started(at);
        hold.set(Reason::Focus, false, at + S);
        assert_eq!(
            hold.wake(),
            Some(at + TIME),
            "clearing an unset reason restarted the time"
        );
        hold.set(Reason::Focus, true, at + S);
        hold.set(Reason::Focus, true, at + 3 * S);
        hold.set(Reason::Focus, false, at + 4 * S);
        assert_eq!(
            hold.wake(),
            Some(at + 8 * S),
            "setting it again counted the time between"
        );
    }

    /// Started again while held, as the next toast is with the pointer still on the first, its
    /// whole time waits for the hold to go.
    #[test]
    fn started_while_held_it_counts_from_the_hold_going() {
        let at = Instant::now();
        let mut hold = started(at);
        hold.set(Reason::Pointer, true, at + S);
        hold.start(Some(TIME), at + 2 * S);
        assert_eq!(hold.wake(), None);
        hold.set(Reason::Pointer, false, at + 7 * S);
        assert_eq!(hold.wake(), Some(at + 12 * S));
    }
}
