//! The list's scroller, Arc 1.170's, which is AppKit's own: a knob with no track at the
//! sidebar's edge, shown as the rows scroll and then faded, or always while they overflow when the
//! system shows scroll bars always, with the rows giving up room for it (docs/ux-spec.md › Tab
//! rows).

use std::time::Duration;
use web_time::Instant;

use gpui::{App, Context, Global, Task, Window, div, prelude::*, px};

use crate::tokens::shell::scroller::{
    EDGE, END, GUTTER, KNOB, KNOB_ALPHA, KNOB_INSET, KNOB_MIN, KNOB_SHOWN, KNOB_SHOWN_ALPHA,
    motion::{FADE, HOLD},
};

use super::Shell;

/// Whether the system shows scroll bars always, as macOS's Show scroll bars: Always, or only
/// while scrolling. main reads it from the system at launch and as the app comes back from
/// another, such as System Settings; without it they show while scrolling.
pub struct ScrollBarsShown(pub bool);

impl Global for ScrollBarsShown {}

/// Reads the system's scroll bars' setting into `ScrollBarsShown`, redrawing the windows if it
/// changed.
pub fn follow_scroll_bars(cx: &mut App) {
    let shown = !cx.should_auto_hide_scrollbars();
    if cx.try_global::<ScrollBarsShown>().map(|bars| bars.0) != Some(shown) {
        // check.sh reads this line.
        crate::log!(
            "browser: scroll bars {}",
            if shown { "always" } else { "while scrolling" }
        );
        cx.set_global(ScrollBarsShown(shown));
        cx.refresh_windows();
    }
}

/// Where the list was scrolled at the last frame, and since when the user has stopped scrolling
/// it, for the overlay knob.
#[derive(Default)]
pub(super) struct Scroller {
    at: f32,
    moved: Option<Instant>,
    /// Wakes the window once the knob has held long enough to fade.
    wake: Option<Task<()>>,
    /// Whether the legacy knob was drawn, as it is while the list overflows.
    legacy_shown: bool,
}

impl Shell {
    /// Whether the knob is the legacy one, always there with room of its own, as the system
    /// shows scroll bars always.
    fn scroll_bars_shown(&self, cx: &App) -> bool {
        cx.try_global::<ScrollBarsShown>()
            .is_some_and(|bars| bars.0)
    }

    /// Whether the list's rows go past its foot, as it was last laid out.
    fn list_overflows(&self) -> bool {
        f32::from(self.list_scroll.max_offset().y) > 0.5
    }

    /// What the rows give up at the sidebar's edge for the knob: the legacy knob's room while the
    /// list overflows, as Arc's rows do.
    pub(super) fn scroller_gutter(&self) -> f32 {
        if self.legacy_scroller && self.list_overflows() {
            GUTTER
        } else {
            0.
        }
    }

    /// How wide the list's rows are, the knob's room taken out.
    pub(super) fn rows_width(&self) -> f32 {
        self.width() - self.scroller_gutter()
    }

    /// Notes the list scrolled `to`, unless `glided` there: the knob comes as the user scrolls
    /// and fades once they've stopped, and a glide to a row brings none (ours: AppKit shows its
    /// overlay knob for the user's scrolls only).
    pub(super) fn step_scroller(
        &mut self,
        to: f32,
        glided: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.legacy_scroller = self.scroll_bars_shown(cx);
        let legacy_shown = self.legacy_scroller && self.list_overflows();
        if legacy_shown != self.scroller.legacy_shown {
            self.scroller.legacy_shown = legacy_shown;
            // check.sh reads these lines.
            let shown = if legacy_shown { "shown" } else { "hidden" };
            crate::log!("browser: legacy scroller {shown}");
        }
        if (to - self.scroller.at).abs() > 0.5 {
            self.scroller.at = to;
            if !glided && !self.legacy_scroller {
                if self.scroller.moved.is_none() && self.list_overflows() {
                    // check.sh waits for these lines.
                    crate::log!("browser: scroller shown");
                }
                self.scroller.moved = Some(Instant::now());
                self.wake_scroller(cx);
            }
        }
        let Some(moved) = self.scroller.moved else {
            return;
        };
        let since = moved.elapsed().as_secs_f32();
        if since >= HOLD + FADE {
            self.scroller.moved = None;
            crate::log!("browser: scroller hidden");
        } else if since >= HOLD {
            window.request_animation_frame();
        }
    }

    /// Wakes the window as the knob starts fading, waiting on until it's held as long since the
    /// last scroll, so a scroll draws no frames of its own while the knob holds.
    fn wake_scroller(&mut self, cx: &mut Context<Self>) {
        if self.scroller.wake.is_some() {
            return;
        }
        self.scroller.wake = Some(cx.spawn(async move |shell, cx| {
            loop {
                let left = shell.update(cx, |shell, cx| {
                    let left = shell
                        .scroller
                        .moved
                        .map(|moved| Duration::from_secs_f32(HOLD).saturating_sub(moved.elapsed()));
                    if left.is_none_or(|left| left.is_zero()) {
                        shell.scroller.wake = None;
                        cx.notify();
                    }
                    left.filter(|left| !left.is_zero())
                });
                match left {
                    Ok(Some(left)) => cx.background_executor().timer(left).await,
                    _ => return,
                }
            }
        }));
    }

    /// How far the knob shows: always while the legacy one's list overflows, and the overlay one
    /// while the user scrolls, fading once they've stopped.
    fn scroller_shown(&self) -> f32 {
        if !self.list_overflows() {
            return 0.;
        }
        if self.legacy_scroller {
            return 1.;
        }
        let Some(moved) = self.scroller.moved else {
            return 0.;
        };
        let since = moved.elapsed().as_secs_f32();
        (1. - (since - HOLD).max(0.) / FADE).clamp(0., 1.)
    }

    /// Where the knob's rect is down the list's scrolled part, and how long: as long against its
    /// track as the rows in view are against all of them, down it as far as the list is scrolled.
    /// Its track is the scrolled part less `END` at each end.
    fn scroller_place(&self) -> (f32, f32) {
        let height = f32::from(self.list_scroll.bounds().size.height);
        let most = f32::from(self.list_scroll.max_offset().y).max(0.);
        let track = (height - 2. * END).max(0.);
        let length = (track * height / (height + most).max(1.))
            .max(KNOB_MIN)
            .min(track);
        let scrolled = (-f32::from(self.list_scroll.offset().y)).clamp(0., most);
        let down = if most > 0. { scrolled / most } else { 0. };
        (END + (track - length) * down, length)
    }

    /// The knob at the sidebar's edge, `head` down the list's place, where its scrolled part
    /// starts. The legacy knob is drawn `KNOB_INSET` inside its rect, as AppKit's is.
    pub(super) fn scroller_knob(&self, head: f32) -> Option<impl IntoElement + use<>> {
        let shown = self.scroller_shown();
        let (top, length) = self.scroller_place();
        let (width, alpha, inset) = match self.legacy_scroller {
            true => (KNOB_SHOWN, KNOB_SHOWN_ALPHA, KNOB_INSET),
            false => (KNOB, KNOB_ALPHA, 0.),
        };
        (shown > 0. && length > 2. * inset).then(|| {
            div()
                .absolute()
                .top(px(head + top + inset))
                .right(px(EDGE))
                .w(px(width))
                .h(px(length - 2. * inset))
                .rounded(px(width / 2.))
                .bg(self.palette.faint(alpha))
                .opacity(shown)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{rc::Rc, time::Duration};

    use gpui::{Entity, TestAppContext, VisualTestContext, point, px, size};

    use super::{FADE, GUTTER, HOLD, ScrollBarsShown};
    use crate::shell::{Folders, Shell, recorder::Recorder};

    /// A Space whose Today tabs overflow its list, drawn.
    fn overflowing(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
        list_of((10..40).collect(), 340., cx)
    }

    /// A Space with `today`'s tabs in a window `height` tall, drawn.
    fn list_of(
        today: Vec<u64>,
        height: f32,
        cx: &mut TestAppContext,
    ) -> (Entity<Shell>, &mut VisualTestContext) {
        let host = Rc::new(Recorder::default());
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host, cx));
        cx.simulate_resize(size(px(1000.), px(height)));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(1, Vec::new(), today, Vec::new(), Folders::default(), cx);
        });
        draw(&shell, cx);
        draw(&shell, cx);
        (shell, cx)
    }

    fn draw(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
        shell.update(cx, |shell, cx| {
            shell.land_list_glide();
            cx.notify();
        });
        cx.run_until_parked();
    }

    /// Puts the user's last scroll `seconds` further back, as the frames since would have it.
    fn age(shell: &Entity<Shell>, seconds: f32, cx: &mut VisualTestContext) {
        shell.update(cx, |shell, _| {
            let back = Duration::from_secs_f32(seconds);
            shell.scroller.moved = shell.scroller.moved.map(|moved| moved - back);
        });
        draw(shell, cx);
    }

    fn shown(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> f32 {
        shell.read_with(cx, |shell, _| shell.scroller_shown())
    }

    #[gpui::test]
    fn the_knob_shows_as_the_user_scrolls_and_fades_once_they_stop(cx: &mut TestAppContext) {
        let (shell, cx) = overflowing(cx);
        assert_eq!(shown(&shell, cx), 0., "the knob showed before a scroll");
        shell.update(cx, |shell, _| {
            shell.list_scroll.set_offset(point(px(0.), px(-200.)))
        });
        draw(&shell, cx);
        assert_eq!(shown(&shell, cx), 1., "a scroll didn't bring the knob");
        age(&shell, HOLD + FADE / 2., cx);
        let half = shown(&shell, cx);
        assert!(half > 0. && half < 1., "the knob wasn't fading: {half}");
        age(&shell, FADE, cx);
        assert_eq!(shown(&shell, cx), 0., "the knob stayed");
    }

    /// The knob is as long against its track as the rows in view against all of them, and at the
    /// track's foot once the list is scrolled all the way, 3 from the sidebar's edge.
    #[gpui::test]
    fn the_knob_is_where_the_rows_in_view_are(cx: &mut TestAppContext) {
        let (shell, cx) = overflowing(cx);
        // Tall enough that the knob is longer than its shortest.
        cx.simulate_resize(size(px(1000.), px(700.)));
        shell.update(cx, |shell, _| {
            shell.list_scroll.set_offset(point(px(0.), px(-1e4)))
        });
        draw(&shell, cx);
        draw(&shell, cx);
        let (height, most) = shell.read_with(cx, |shell, _| {
            let scroll = &shell.list_scroll;
            (
                f32::from(scroll.bounds().size.height),
                f32::from(scroll.max_offset().y),
            )
        });
        assert!(most > 0., "the list doesn't overflow");
        let knob = shell.read_with(cx, |shell, _| shell.scroller_place());
        let track = height - 6.;
        let length = track * height / (height + most);
        assert!(
            (knob.1 - length).abs() < 0.01,
            "the knob is {} long, not {length}",
            knob.1
        );
        assert!(
            (knob.0 + knob.1 - (3. + track)).abs() < 0.01,
            "the knob isn't at the track's foot"
        );
    }

    #[gpui::test]
    fn a_glide_to_a_row_brings_no_knob(cx: &mut TestAppContext) {
        let (shell, cx) = overflowing(cx);
        shell.update(cx, |shell, _| shell.scroll_to_row(39));
        draw(&shell, cx);
        let scrolled = shell.read_with(cx, |shell, _| -f32::from(shell.list_scroll.offset().y));
        assert!(scrolled > 0., "the list didn't glide to the last row");
        assert_eq!(shown(&shell, cx), 0., "the glide brought the knob");
    }

    /// With the system's scroll bars shown always, the knob stays while the rows overflow, and
    /// they give up the legacy knob's room, their plates ending that much further in.
    #[gpui::test]
    fn with_scroll_bars_always_the_knob_stays_and_the_rows_give_up_room(cx: &mut TestAppContext) {
        let (shell, cx) = overflowing(cx);
        let plate = |cx: &mut VisualTestContext| {
            let frame = shell.read_with(cx, |shell, _| shell.tip_frames.borrow().get(&10).copied());
            f32::from(frame.expect("the first row has no frame").size.width)
        };
        let wide = plate(cx);
        assert_eq!(shown(&shell, cx), 0., "the overlay knob showed at rest");
        cx.update(|_, cx| cx.set_global(ScrollBarsShown(true)));
        draw(&shell, cx);
        draw(&shell, cx);
        assert_eq!(shown(&shell, cx), 1., "the legacy knob didn't show at rest");
        assert_eq!(
            plate(cx),
            wide - GUTTER,
            "the rows didn't give up the knob's room"
        );
        // A list that fits, the room after its last row too, has neither.
        let (fits, cx) = list_of(vec![10, 11], 700., cx);
        cx.update(|_, cx| cx.set_global(ScrollBarsShown(true)));
        draw(&fits, cx);
        let width = fits.read_with(cx, |shell, _| shell.width() - shell.rows_width());
        assert_eq!(
            (shown(&fits, cx), width),
            (0., 0.),
            "a list that fits kept the knob"
        );
    }
}
