//! The Library's Spaces cards once they don't fit: they scroll across under two fingers and a
//! wheel, as Arc 1.171's do, one to one and elastic at both ends.

use web_time::Instant;

use gpui::Context;

use super::{
    Section, Shell, WHEEL_LINE, WHEEL_REST, band,
    library::{LIBRARY_DAMPING, LIBRARY_RESPONSE, spaces_content},
    spring_to,
};
use crate::tokens::shell::library::COLUMN_WIDTH;

/// Past either end fingers take the cards on less and less, 17 for a 340 pull as Arc's, and never
/// as far as the Library's own band.
const CARD_BAND: f32 = 80.;
const CARD_BAND_STIFFNESS: f32 = 0.0635;

/// How far the Spaces cards have scrolled across.
#[derive(Default)]
pub(super) struct CardStrip {
    /// How far left the cards have gone, in points, where fingers or the wheel last left them.
    at: f32,
    /// Springing back to the end fingers let them go past: from where, and since when.
    settle: Option<(f32, Instant)>,
    /// Whether the momentum after the fingers lift still carries the cards.
    coasting: bool,
    /// When the wheel last moved them, as at their end it leaves the Library only after a rest.
    wheeled: Option<Instant>,
}

/// Where fingers that took the cards `raw` left have them in a strip that has `room` to scroll:
/// one to one between its ends, and past them on the band.
pub(super) fn card_band(raw: f32, room: f32) -> f32 {
    match raw {
        raw if raw < 0. => -band(-raw, CARD_BAND, CARD_BAND_STIFFNESS),
        raw if raw > room => room + band(raw - room, CARD_BAND, CARD_BAND_STIFFNESS),
        raw => raw,
    }
}

impl Shell {
    /// How far the Spaces cards can scroll: what they need past the Library's width, or 0 while
    /// they fit or another section shows.
    pub(super) fn cards_room(&self) -> f32 {
        if self.library.is_none() || self.section != Section::Spaces {
            return 0.;
        }
        (spaces_content(self.spaces.len()) - self.library_width()).max(0.)
    }

    /// How far left the Spaces cards are: under the fingers, springing back, or at rest, never past
    /// an end once the room shrinks.
    pub(super) fn cards_at(&self) -> f32 {
        let room = self.cards_room();
        if let Some(raw) = self.swipe.as_ref().and_then(|swipe| swipe.cards) {
            return card_band(raw, room);
        }
        match self.card_settle(room) {
            Some((at, _)) => at,
            None => self.card_strip.at.clamp(0., room),
        }
    }

    /// Where the cards springing back are, and how fast they go, or None once they're there.
    fn card_settle(&self, room: f32) -> Option<(f32, f32)> {
        let (from, start) = self.card_strip.settle?;
        let to = if from < 0. { 0. } else { room };
        let t = start.elapsed().as_secs_f32();
        spring_to(t, from, to, 0., LIBRARY_RESPONSE, LIBRARY_DAMPING)
    }

    /// Whether `x` across the window is over the Spaces cards, with the Library open where it
    /// opens and the cards with room to scroll.
    pub(super) fn over_cards(&self, x: f32) -> bool {
        let (panel, width) = (self.library_x(), self.library_width());
        // On the right they're left of the column.
        let (left, right) = match self.right {
            true => (panel, panel + width - COLUMN_WIDTH),
            false => (panel + COLUMN_WIDTH, panel + width),
        };
        let open = self.library_open && self.library_motion.is_none();
        open && self.cards_room() > 0. && x >= left && x < right
    }

    /// Fingers came down at `x`: the momentum of the last swipe stops, and over the cards, which
    /// way they go first decides whether they scroll them (`catch_cards`).
    pub(super) fn fingers_down(&mut self, x: f32) -> Option<f32> {
        self.card_strip.coasting = false;
        self.over_cards(x).then_some(0.)
    }

    /// Fingers that came down over the cards and went `across` first: they scroll the cards,
    /// catching them as they spring back or coast, unless the cards are at rest at the end they
    /// push past, when the swipe is the Library's (Arc 1.171). Returns where they have the cards.
    pub(super) fn catch_cards(&mut self, across: f32) -> Option<f32> {
        let (room, at) = (self.cards_room(), self.cards_at());
        let at_end = match across {
            across if across > 0. => at <= 0.,
            _ => at >= room,
        };
        self.card_strip.coasting = false;
        if at_end && self.card_strip.settle.is_none() {
            return None;
        }
        self.card_strip.settle = None;
        Some(at)
    }

    /// Fingers on the cards, which had them `raw` left, go `across`: one to one, as a scroll
    /// view's content. Returns where they have them now, before the band.
    pub(super) fn move_cards(&mut self, raw: f32, across: f32, cx: &mut Context<Self>) -> f32 {
        let (raw, room) = (raw - across, self.cards_room());
        // Where they stay if the swipe goes without a lift, as when the Library closes under it.
        self.card_strip.at = card_band(raw, room).clamp(0., room);
        cx.notify();
        raw
    }

    /// The fingers lift from the cards `raw` left: past an end they spring back, and between
    /// them the momentum carries them on.
    pub(super) fn release_cards(&mut self, raw: f32, cx: &mut Context<Self>) {
        let room = self.cards_room();
        let at = card_band(raw, room);
        self.card_strip.at = at.clamp(0., room);
        self.card_strip.coasting = (0. ..=room).contains(&at);
        if !self.card_strip.coasting {
            self.card_strip.settle = Some((at, Instant::now()));
        }
        cx.notify();
    }

    /// The momentum after fingers lifted from the cards, `across` points, carries them up to an
    /// end. Returns whether the cards took it.
    pub(super) fn coast_cards(&mut self, across: f32, cx: &mut Context<Self>) -> bool {
        if !self.card_strip.coasting {
            return false;
        }
        self.card_strip.at = (self.card_strip.at - across).clamp(0., self.cards_room());
        cx.notify();
        true
    }

    /// A wheel's `lines` across over the cards at `x` scroll them 10 a line (Arc 1.171). At their
    /// end it goes on to the Library's paging, which leaves it, but only once the wheel has rested
    /// since it last moved them. Returns whether the cards took the ticks.
    pub(super) fn wheel_cards(&mut self, lines: f32, x: f32, cx: &mut Context<Self>) -> bool {
        if !self.over_cards(x) {
            return false;
        }
        let now = Instant::now();
        let (room, at) = (self.cards_room(), self.cards_at());
        let to = (at - lines * WHEEL_LINE).clamp(0., room);
        let rested = self
            .card_strip
            .wheeled
            .is_none_or(|wheeled| now - wheeled >= WHEEL_REST);
        if to == at && rested {
            return false;
        }
        self.card_strip = CardStrip {
            at: to,
            settle: None,
            coasting: false,
            wheeled: Some(now),
        };
        cx.notify();
        true
    }

    /// Steps the cards springing back for a frame. Returns whether they're still going.
    pub(super) fn step_cards(&mut self) -> bool {
        if self.card_strip.settle.is_none() {
            return false;
        }
        if self.card_settle(self.cards_room()).is_some() {
            return true;
        }
        self.card_strip.settle = None;
        // check.sh waits for this line.
        log!("browser: library cards at {:.0}", self.cards_at());
        false
    }

    /// The cards start at their first again, as the Library shows its Spaces anew.
    pub(super) fn reset_cards(&mut self) {
        self.card_strip = CardStrip::default();
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use gpui::{
        Entity, Modifiers, MouseButton, Point, ScrollDelta, ScrollEvent, TestAppContext,
        TouchPhase, VisualTestContext, point, px, size,
    };

    use super::card_band;
    use crate::shell::{Section, Shell, recorder::Recorder};

    /// Arc's measured window: 8 Spaces, 1100 wide, the Library open on them at rest, so the
    /// Library is 1044 and its cards, 2328 across, have 1284 to scroll.
    fn eight_spaces(
        cx: &mut TestAppContext,
    ) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        cx.update(|window, _| window.activate_window());
        cx.simulate_resize(size(px(1100.), px(700.)));
        cx.run_until_parked();
        shell.update(cx, |shell, cx| {
            let space = |id: u64| {
                (
                    id,
                    format!("Space {id}").into(),
                    None,
                    1,
                    "".into(),
                    "".into(),
                    "".into(),
                )
            };
            shell.set_spaces((1..=8).map(space).collect(), cx);
            shell.set_space(1, cx);
            shell.section = Section::Spaces;
            shell.set_library(true, cx);
            shell.library_motion = None;
            shell.library_resize = None;
            shell.sidebar_fade = None;
        });
        cx.run_until_parked();
        host.shown.take();
        let widths = read(&shell, cx, |shell| {
            (shell.library_width(), shell.cards_room())
        });
        assert_eq!(widths, (1044., 1284.));
        (shell, cx, host)
    }

    fn read<T>(shell: &Entity<Shell>, cx: &mut VisualTestContext, f: impl Fn(&Shell) -> T) -> T {
        shell.read_with(cx, |shell, _| f(shell))
    }

    /// Two fingers at `x`, down, across by each of `moves`, and, if `lift`, up.
    fn fingers(cx: &mut VisualTestContext, x: f32, moves: &[f32], lift: bool) {
        let event = |touch_phase, dx: f32| ScrollEvent {
            position: point(px(x), px(350.)),
            delta: ScrollDelta::Pixels(point(px(dx), px(0.))),
            touch_phase,
            ..Default::default()
        };
        cx.simulate_event(event(TouchPhase::Started, 0.));
        for &dx in moves {
            cx.simulate_event(event(TouchPhase::Moved, dx));
        }
        if lift {
            cx.simulate_event(event(TouchPhase::Ended, 0.));
        }
        cx.run_until_parked();
    }

    /// The fingers at `x` lift.
    fn lift(cx: &mut VisualTestContext, x: f32) {
        cx.simulate_event(ScrollEvent {
            position: point(px(x), px(350.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(0.))),
            touch_phase: TouchPhase::Ended,
            ..Default::default()
        });
        cx.run_until_parked();
    }

    /// A wheel's `lines` across at `x`, as a shift held with a wheel's turn sends them.
    fn wheel(cx: &mut VisualTestContext, x: f32, lines: f32) {
        cx.simulate_event(ScrollEvent {
            position: point(px(x), px(350.)),
            delta: ScrollDelta::Lines(point(lines, 0.)),
            touch_phase: TouchPhase::Moved,
            ..Default::default()
        });
        cx.run_until_parked();
    }

    fn click(cx: &mut VisualTestContext, at: Point<gpui::Pixels>) {
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    }

    /// Over the cards, two fingers across scroll them one to one, the Library staying where it
    /// opens; the momentum after they lift carries them on, a click lands on the card now under
    /// it, and fingers going along don't move them.
    #[gpui::test]
    fn fingers_over_the_cards_scroll_them_one_to_one(cx: &mut TestAppContext) {
        let (shell, cx, host) = eight_spaces(cx);
        fingers(cx, 600., &[-100., -100., -100.], false);
        assert_eq!(
            read(&shell, cx, |shell| (shell.cards_at(), shell.library_out())),
            (300., 1.)
        );
        lift(cx, 600.);
        fingers(cx, 600., &[-100.], true);
        let coast = ScrollEvent {
            position: point(px(600.), px(350.)),
            delta: ScrollDelta::Pixels(point(px(-50.), px(0.))),
            touch_phase: TouchPhase::Moved,
            ..Default::default()
        };
        cx.simulate_event(coast.clone());
        cx.run_until_parked();
        assert_eq!(read(&shell, cx, |shell| shell.cards_at()), 450.);
        let along = ScrollEvent {
            delta: ScrollDelta::Pixels(point(px(0.), px(-80.))),
            touch_phase: TouchPhase::Started,
            ..coast
        };
        cx.simulate_event(along.clone());
        cx.simulate_event(ScrollEvent {
            touch_phase: TouchPhase::Moved,
            ..along.clone()
        });
        cx.simulate_event(ScrollEvent {
            touch_phase: TouchPhase::Ended,
            ..along
        });
        cx.run_until_parked();
        assert_eq!(read(&shell, cx, |shell| shell.cards_at()), 450.);
        // Space 4's card is at 112 + 28 + 3 × 264 = 932 before the scroll, 482 after it.
        click(cx, point(px(482. + 120.), px(350.)));
        assert_eq!(*host.shown.borrow(), [4]);
    }

    /// A swipe that starts in the cards stays theirs: past either end the cards give on the band,
    /// the Library staying open, and spring back once the fingers lift.
    #[gpui::test]
    fn a_swipe_in_the_cards_never_leaves_the_library(cx: &mut TestAppContext) {
        let (shell, cx, _) = eight_spaces(cx);
        fingers(cx, 600., &[-100., 440.], false);
        let pulled = read(&shell, cx, |shell| shell.cards_at());
        assert!(
            (pulled + 17.).abs() < 0.5,
            "340 past the start, the cards are at {pulled}"
        );
        lift(cx, 600.);
        assert!(read(&shell, cx, |shell| shell.library_open
            && shell.library_out() == 1.));
        shell.update(cx, |shell, _| {
            let settle = shell
                .card_strip
                .settle
                .as_mut()
                .expect("the cards spring back");
            settle.1 -= Duration::from_secs(1);
        });
        shell.update(cx, |shell, _| assert!(!shell.step_cards()));
        assert_eq!(read(&shell, cx, |shell| shell.cards_at()), 0.);

        fingers(cx, 600., &[-100., -3000.], false);
        let (at, out) = read(&shell, cx, |shell| (shell.cards_at(), shell.library_out()));
        assert!(
            at > 1284. && at < 1284. + 80.,
            "pushed past the end, the cards are at {at}"
        );
        assert_eq!(out, 1., "the Library moved");
        lift(cx, 600.);
        assert!(read(&shell, cx, |shell| shell.library_open));
    }

    /// A swipe that starts with the cards at rest at the end it pushes past is the Library's:
    /// out at their last, taking the Library away, and at their first, taking it on past where it
    /// opens. Over the column, fingers move the Library wherever the cards are.
    #[gpui::test]
    fn a_swipe_from_the_cards_end_is_the_librarys(cx: &mut TestAppContext) {
        let (shell, cx, _) = eight_spaces(cx);
        fingers(cx, 600., &[40.], false);
        let (at, out) = read(&shell, cx, |shell| (shell.cards_at(), shell.library_out()));
        assert_eq!(at, 0.);
        assert!(
            out > 1.,
            "fingers past the first card didn't take the Library on: {out}"
        );
        lift(cx, 600.);
        shell.update(cx, |shell, _| shell.library_motion = None);

        fingers(cx, 600., &[-1284.], true);
        assert_eq!(read(&shell, cx, |shell| shell.cards_at()), 1284.);
        fingers(cx, 600., &[-200.], false);
        let out = read(&shell, cx, |shell| shell.library_out());
        assert!(
            out < 1.,
            "fingers past the last card didn't take the Library out: {out}"
        );
        assert_eq!(read(&shell, cx, |shell| shell.cards_at()), 1284.);
        fingers(cx, 600., &[-1200.], false);
        lift(cx, 600.);
        assert!(
            !read(&shell, cx, |shell| shell.library_open),
            "the Library stayed"
        );

        let (shell, cx, _) = eight_spaces(cx);
        fingers(cx, 600., &[-300.], true);
        fingers(cx, 50., &[-100.], false);
        let out = read(&shell, cx, |shell| shell.library_out());
        assert!(
            out < 1.,
            "fingers over the column didn't move the Library: {out}"
        );
        assert_eq!(read(&shell, cx, |shell| shell.cards_at()), 300.);
    }

    /// A wheel's ticks across scroll the cards 10 a line. At their last they go no further, and
    /// once the wheel has rested, the next ticks take the Library away (Arc 1.171).
    #[gpui::test]
    fn the_wheel_scrolls_the_cards_and_leaves_at_their_end(cx: &mut TestAppContext) {
        let (shell, cx, _) = eight_spaces(cx);
        wheel(cx, 600., -3.);
        assert_eq!(read(&shell, cx, |shell| shell.cards_at()), 30.);
        wheel(cx, 600., 1.);
        assert_eq!(read(&shell, cx, |shell| shell.cards_at()), 20.);
        for _ in 0..20 {
            wheel(cx, 600., -10.);
        }
        assert_eq!(read(&shell, cx, |shell| shell.cards_at()), 1284.);
        wheel(cx, 600., -1.);
        assert!(
            read(&shell, cx, |shell| shell.library_open),
            "ticks going on at the end left"
        );
        shell.update(cx, |shell, _| {
            shell.card_strip.wheeled = Some(Instant::now() - Duration::from_secs(1));
        });
        wheel(cx, 600., -1.);
        assert!(
            !read(&shell, cx, |shell| shell.library_open),
            "a tick after a rest stayed"
        );
    }

    /// Between their ends the cards go one to one with the fingers, and past either end they give
    /// as Arc's do, 17 for a 340 pull, and less and less after.
    #[test]
    fn the_cards_follow_the_fingers_and_give_past_their_ends() {
        assert_eq!(card_band(0., 500.), 0.);
        assert_eq!(card_band(240., 500.), 240.);
        assert_eq!(card_band(500., 500.), 500.);
        let pulled = -card_band(-340., 500.);
        assert!(
            (pulled - 17.).abs() < 0.5,
            "340 past the start gives {pulled}"
        );
        let pushed = card_band(840., 500.) - 500.;
        assert!(
            (pushed - pulled).abs() < 0.01,
            "both ends give alike: {pushed} and {pulled}"
        );
        assert!(card_band(-2000., 500.) > -80., "the band never gives 80");
        let (pulled, pushed) = (card_band(-100., 0.), card_band(100., 0.));
        assert!(pulled < 0. && pushed > 0., "with no room both ends give");
    }
}
