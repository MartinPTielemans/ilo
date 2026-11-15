//! Esc in a window full screen of its own goes to the page first, and one the page doesn't use
//! leaves full screen. Pressed twice within ESCAPE_TWICE, or held for ESCAPE_HELD, it leaves even
//! when the page keeps it, as Arc's does, so a page that keeps Esc, a game or an editor's vim mode,
//! never keeps the user in full screen (docs/ux-spec.md › Window full screen).

use std::time::Duration;
use web_time::Instant;

use gpui::Context;

use crate::shell::{Shell, browser::WindowButton};

/// How soon a second Esc leaves.
const ESCAPE_TWICE: Duration = Duration::from_millis(500);
/// How long a held Esc takes to leave.
const ESCAPE_HELD: Duration = Duration::from_millis(600);

/// The Esc presses a page's window full screen has seen.
#[derive(Default)]
pub struct EscapeToLeave {
    /// When the last press that wasn't a repeat came, and whether it has left already.
    pressed: Option<Instant>,
    left: bool,
}

impl EscapeToLeave {
    /// Whether this Esc, a key repeating as it's held if `repeat`, leaves full screen at `now`:
    /// the second within ESCAPE_TWICE of the first, or the press held ESCAPE_HELD. Once a press
    /// has left, its repeats don't count again.
    pub fn press(&mut self, repeat: bool, now: Instant) -> bool {
        if repeat {
            let held = (self.pressed).is_some_and(|at| now.duration_since(at) >= ESCAPE_HELD);
            if held && !self.left {
                self.left = true;
                return true;
            }
            return false;
        }
        let twice = (self.pressed).is_some_and(|at| now.duration_since(at) <= ESCAPE_TWICE);
        self.left = twice;
        self.pressed = (!twice).then_some(now);
        twice
    }
}

impl Shell {
    /// Whether Esc in the page can take the window out of full screen: it's full screen of its
    /// own, not for the page, whose full screen Esc ends as Chrome's does.
    pub fn escape_leaves_full_screen(&self) -> bool {
        self.full_screen && !self.page_fullscreen
    }

    /// An Esc the page has first, in full screen: whether it leaves now, held or pressed twice,
    /// which it does here, before the page sees it.
    pub fn escape_in_full_screen(
        &mut self,
        repeat: bool,
        now: Instant,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.escape_leaves_full_screen() || !self.escapes.press(repeat, now) {
            return false;
        }
        // check.sh reads this line.
        let how = if repeat { "held" } else { "twice" };
        log!("browser: escape {how} leaves full screen");
        self.browser.press_window_button(WindowButton::Zoom, cx);
        true
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::EscapeToLeave;

    fn ms(at: Instant, ms: u64) -> Instant {
        at + Duration::from_millis(ms)
    }

    /// Twice within 500 ms leaves; slower, each press is a first.
    #[test]
    fn twice_within_half_a_second_leaves() {
        let start = Instant::now();
        let mut escapes = EscapeToLeave::default();
        assert!(!escapes.press(false, start));
        assert!(
            escapes.press(false, ms(start, 400)),
            "a second Esc 400 ms on didn't leave"
        );
        let mut slow = EscapeToLeave::default();
        assert!(!slow.press(false, start));
        assert!(
            !slow.press(false, ms(start, 700)),
            "a second Esc 700 ms on left"
        );
        assert!(
            slow.press(false, ms(start, 1000)),
            "the third, 300 ms after the second, didn't"
        );
    }

    /// Held, the repeats leave once 600 ms have passed since the press, and only once.
    #[test]
    fn held_for_600_ms_leaves_once() {
        let start = Instant::now();
        let mut escapes = EscapeToLeave::default();
        assert!(!escapes.press(false, start));
        assert!(
            !escapes.press(true, ms(start, 300)),
            "a repeat at 300 ms left"
        );
        assert!(
            escapes.press(true, ms(start, 650)),
            "the repeat at 650 ms didn't leave"
        );
        assert!(
            !escapes.press(true, ms(start, 700)),
            "a later repeat left again"
        );
    }

    /// A press after a double that left is a first again, so a third Esc doesn't count.
    #[test]
    fn a_press_after_leaving_starts_again() {
        let start = Instant::now();
        let mut escapes = EscapeToLeave::default();
        escapes.press(false, start);
        assert!(escapes.press(false, ms(start, 200)));
        assert!(
            !escapes.press(false, ms(start, 400)),
            "the third Esc counted as a second"
        );
    }
}
