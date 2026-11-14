//! Reduce Motion, in one place. Under it nothing moves or scales, and fades stay (ux-spec ›
//! Motion). Every motion goes through here, in one of two ways:
//!
//! - What changes in place, such as rows making room, a folder opening, the Library pushing the
//!   page aside or a Space's pages settling, starts its clock with `start`, which under Reduce
//!   Motion is long past, so the change shows where it ends from its first frame. A fade that comes
//!   with it keeps `Instant::now()`.
//! - What comes and goes, such as a toast, a dialog or Peek, keeps its own clock for its fade and
//!   is drawn where it rests: `scale` and `offset` take its motion away, and `shell::scaled`
//!   draws every scaled thing through `scale`.
//!
//! What the fingers hold, such as a swipe, follows them all the same: that's the hand, not
//! motion. What they let go settles through `start`.

use std::time::Duration;
use web_time::Instant;

use gpui::App;

/// Long past the end of any motion here.
const LONG_AGO: Duration = Duration::from_secs(10);

/// Whether things are to keep still: the system's Reduce Motion (`App::reduce_motion`).
pub fn still(cx: &App) -> bool {
    cx.reduce_motion()
}

/// When a change in place starts moving: now, or long enough ago under Reduce Motion that its
/// motion is over by its first frame.
pub fn start(cx: &App) -> Instant {
    start_at(Instant::now(), cx)
}

/// `start` on the clock `now` is read from, such as the executor's, which a test moves on itself.
pub fn start_at(now: Instant, cx: &App) -> Instant {
    match still(cx) {
        true => now.checked_sub(LONG_AGO).unwrap_or(now),
        false => now,
    }
}

/// The size something growing or shrinking is drawn at, `scale` of its own as its motion has it:
/// its own under Reduce Motion.
pub fn scale(scale: f32, cx: &App) -> f32 {
    if still(cx) { 1. } else { scale }
}

/// How far from its place something coming or going is drawn, `offset` as its motion has it:
/// in its place under Reduce Motion.
pub fn offset(offset: f32, cx: &App) -> f32 {
    if still(cx) { 0. } else { offset }
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use super::*;

    #[gpui::test]
    fn under_reduce_motion_a_change_starts_at_its_end_and_what_comes_rests(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            assert!(start(cx).elapsed() < LONG_AGO / 2);
            assert_eq!((scale(0.7, cx), offset(32., cx)), (0.7, 32.));
            cx.set_reduce_motion(true);
            assert!(start(cx).elapsed() >= LONG_AGO);
            assert_eq!((scale(0.7, cx), offset(32., cx)), (1., 0.));
        });
    }
}
