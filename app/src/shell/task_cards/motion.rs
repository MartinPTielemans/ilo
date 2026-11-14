//! A value the foot eases from where it is to where it's going, which a change midway picks up
//! from where it had got to, so nothing jumps.

use std::time::Duration;
use web_time::Instant;

use crate::shell::cubic_bezier;
use crate::tokens::shell::task::motion::EASE;

/// How a value goes to its end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Curve {
    /// The motion's ease.
    Ease,
    Linear,
}

/// How far the motion's ease has gone `t` of the way through.
pub(super) fn eased(t: f32) -> f32 {
    cubic_bezier(EASE[0], EASE[1], EASE[2], EASE[3], t.clamp(0., 1.))
}

/// A value going from `from` to `to`, `length` seconds long once `delay` has passed since
/// `since`, or at rest at `to` with no `since`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Ease {
    from: f32,
    to: f32,
    since: Option<Instant>,
    delay: f32,
    length: f32,
    curve: Curve,
}

impl Ease {
    pub(super) fn rest(at: f32) -> Self {
        Self {
            from: at,
            to: at,
            since: None,
            delay: 0.,
            length: 0.,
            curve: Curve::Ease,
        }
    }

    /// Where it is at `now`.
    pub(super) fn at(&self, now: Instant) -> f32 {
        let Some(since) = self.since else {
            return self.to;
        };
        let t = now.saturating_duration_since(since).as_secs_f32() - self.delay;
        if t <= 0. {
            return self.from;
        }
        if self.length <= 0. || t >= self.length {
            return self.to;
        }
        let x = t / self.length;
        let way = match self.curve {
            Curve::Ease => eased(x),
            Curve::Linear => x,
        };
        self.from + (self.to - self.from) * way
    }

    /// Where it's going.
    pub(super) fn end(&self) -> f32 {
        self.to
    }

    /// Whether it's still on its way at `now`.
    pub(super) fn moving(&self, now: Instant) -> bool {
        self.since.is_some_and(|since| {
            let length = Duration::from_secs_f32((self.delay + self.length).max(0.));
            now.saturating_duration_since(since) < length
        })
    }

    /// Sets off from where it is at `now` to `to`, `length` seconds long after `delay`, unless it's
    /// going there already. With no length it's there at once.
    pub(super) fn go(&mut self, to: f32, now: Instant, length: f32, delay: f32, curve: Curve) {
        if to == self.to {
            return;
        }
        if length <= 0. && delay <= 0. {
            *self = Self::rest(to);
            return;
        }
        *self = Self {
            from: self.at(now),
            to,
            since: Some(now),
            delay,
            length,
            curve,
        };
    }

    /// Settles it once it has got there, so it isn't asked again.
    pub(super) fn settle(&mut self, now: Instant) {
        if self.since.is_some() && !self.moving(now) {
            *self = Self::rest(self.to);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{Curve, Ease};

    /// It eases to its end, and a change midway sets off from where it had got to.
    #[test]
    fn a_value_eases_and_picks_up_midway() {
        let now = Instant::now();
        let mut value = Ease::rest(0.);
        value.go(10., now, 0.2, 0., Curve::Linear);
        assert_eq!(value.at(now), 0.);
        assert!((value.at(now + Duration::from_millis(100)) - 5.).abs() < 0.01);
        assert!(value.moving(now + Duration::from_millis(100)));
        let mid = now + Duration::from_millis(100);
        value.go(0., mid, 0.2, 0., Curve::Ease);
        assert!((value.at(mid) - 5.).abs() < 0.01, "it jumped");
        assert_eq!(value.at(mid + Duration::from_secs(1)), 0.);
        assert!(!value.moving(mid + Duration::from_secs(1)));
        // A delay holds it where it was.
        let mut later = Ease::rest(1.);
        later.go(0., now, 0.1, 0.1, Curve::Linear);
        assert_eq!(later.at(now + Duration::from_millis(50)), 1.);
        // No length is there at once.
        later.go(3., now, 0., 0., Curve::Ease);
        assert_eq!(later.at(now), 3.);
        assert!(!later.moving(now));
    }
}
