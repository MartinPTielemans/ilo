//! ⌘N's and ⇧⌘N's window opens with ⌘T's command bar, as Arc's does, so typing goes straight to a
//! page: once the window's first frame is on screen, and with the window it came from full screen,
//! once it's full screen too (docs/ux-spec.md › Window › New window).

use web_time::Instant;

use gpui::{Context, Window};

use crate::shell::{Command, Shell};

mod life;

use life::{Actions, Event, Life};

/// A window ⌘N or ⇧⌘N opened, until its command bar opens.
#[derive(Clone, Copy)]
pub(crate) struct Opening {
    /// When the key was pressed, for the first frame's time.
    asked: Instant,
    life: Life,
}

impl Shell {
    /// Whether the window is full screen of its own, not for a page: a window ⌘N opens from it
    /// goes full screen too. As the window has it, which the shell hears a frame or more later: a
    /// ⌘N between opened a window that went full screen without waiting for its desktop (#2553).
    pub fn in_window_full_screen(&self) -> bool {
        (self.full_screen || self.browser.full_screen_now()) && !self.page_fullscreen
    }

    /// The window was opened by ⌘N or ⇧⌘N at `asked`: it opens the command bar once it's on
    /// screen, going full screen first if `full_screen`. `drawn` if it came on screen with its
    /// first frame, whose time is told then.
    pub fn open_bar_when_shown(&mut self, asked: Instant, full_screen: bool, drawn: bool) {
        self.opening = Some(Opening {
            asked,
            life: life::start(full_screen, drawn),
        });
    }

    /// Before the frame's drawn: once the window is on screen, the command bar, or first full
    /// screen and the window's desktop (`life.rs`).
    pub(super) fn step_opening(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Opening { asked, mut life }) = self.opening else {
            return;
        };
        if !window.is_visible() {
            return;
        }
        loop {
            let frame = Event::Frame {
                full_screen: self.full_screen,
                on_space: self.browser.on_active_space(),
            };
            let (next, mut actions) = life::step(life, frame);
            life = next;
            if actions.has(Actions::ASK) {
                let already = self.browser.enter_full_screen(cx);
                log!(
                    "browser: new window asked for full screen, already {}",
                    already as u8
                );
                let (next, answer) = life::step(life, Event::Asked { already });
                life = next;
                actions = actions | answer;
            }
            // Kept before what follows, as taking the full screen steps it again.
            self.opening = (life != Life::Open).then_some(Opening { asked, life });
            if actions.has(Actions::FIRST_FRAME) {
                // check.sh reads this line, the new window's time from the key to its first frame.
                let ms = asked.elapsed().as_secs_f64() * 1000.;
                log!("browser: new window first frame at {ms:.1} ms");
            }
            if actions.has(Actions::TAKE) {
                // AppKit tells of the full screen it gave the window as it opened before the window
                // can hear it, so the shell takes it from here: its layout and the note follow.
                self.set_full_screen(true, cx);
                life = self.opening.map_or(Life::Open, |opening| opening.life);
            }
            if actions.has(Actions::WENT) {
                // check.sh reads this line.
                log!("browser: new window went full screen");
            }
            if actions.has(Actions::OPEN_BAR) {
                // After this frame, as running it updates the shell.
                cx.defer_in(window, |shell, _, cx| {
                    shell.run(Command::NewTab, "new window", cx)
                });
            }
            if actions.has(Actions::NEXT_FRAME) {
                window.request_animation_frame();
            }
            if !actions.has(Actions::AGAIN) {
                return;
            }
        }
    }

    /// The host said the window went full screen or left it: a new window waiting for that steps
    /// at its next frame.
    pub(super) fn opening_heard(&mut self, full_screen: bool, cx: &mut Context<Self>) {
        let Some(opening) = self.opening.as_mut() else {
            return;
        };
        let (life, actions) = life::step(opening.life, Event::Heard { full_screen });
        opening.life = life;
        if actions.has(Actions::NEXT_FRAME) {
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{rc::Rc, time::Instant};

    use gpui::{Entity, TestAppContext, VisualTestContext};

    use crate::shell::{Folders, Shell, recorder::Recorder};

    fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(
                1,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Folders::default(),
                cx,
            );
        });
        cx.run_until_parked();
        host.take();
        (shell, cx, host)
    }

    fn bars(host: &Recorder) -> usize {
        host.command_bars.borrow().len()
    }

    /// ⌘N's window opens ⌘T's bar once it's drawn on screen, once, with "+ New Tab" selected as
    /// ⌘T's does, and Esc gives back the empty card.
    #[gpui::test]
    fn a_new_window_opens_the_command_bar_once_its_drawn(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        shell.update(cx, |shell, _| {
            shell.open_bar_when_shown(Instant::now(), false, false)
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert_eq!(bars(&host), 1, "the new window's bar didn't open");
        assert!(
            shell.read_with(cx, |shell, _| shell.new_tab),
            "the bar opened as ⌘L's, not ⌘T's"
        );
        assert!(
            host.bar_edits.borrow().last() == Some(&None),
            "the new window's bar edits a tab"
        );
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert_eq!(bars(&host), 1, "the bar opened again on the next frame");
        shell.update(cx, |shell, cx| shell.command_bar_dismissed(cx));
        cx.run_until_parked();
        assert!(
            shell.read_with(cx, |shell, _| !shell.new_tab && shell.selected() == 0),
            "closing the bar didn't leave the empty card"
        );
    }

    /// A window that came on screen with its first frame drawn, as on the Mac (#2141), still
    /// opens its bar, once.
    #[gpui::test]
    fn a_new_window_drawn_as_it_came_on_screen_opens_the_command_bar(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        shell.update(cx, |shell, _| {
            shell.open_bar_when_shown(Instant::now(), false, true)
        });
        for _ in 0..2 {
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
        }
        assert_eq!(bars(&host), 1, "the drawn window's bar didn't open once");
    }

    /// A window that has gone full screen is, for ⌘N, before the shell has heard so, and isn't while
    /// a page has it full screen (#2553).
    #[gpui::test]
    fn a_window_is_full_screen_for_cmd_n_before_the_shell_hears(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        host.full_screen.set(true);
        assert!(
            shell.read_with(cx, |shell, _| shell.in_window_full_screen()),
            "the window full screen wasn't, until the shell heard"
        );
        shell.update(cx, |shell, _| shell.page_fullscreen = true);
        assert!(
            shell.read_with(cx, |shell, _| !shell.in_window_full_screen()),
            "the window full screen for a page was full screen of its own"
        );
    }

    /// From a full-screen window, it goes full screen first, and the bar waits for that.
    #[gpui::test]
    fn a_new_window_from_full_screen_goes_full_screen_before_its_bar(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        shell.update(cx, |shell, _| {
            shell.open_bar_when_shown(Instant::now(), true, false)
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert_eq!(
            bars(&host),
            0,
            "the bar opened before the window went full screen"
        );
        assert_eq!(
            host.take()
                .iter()
                .filter(|call| *call == "enter full screen")
                .count(),
            1,
            "the window didn't ask once to go full screen"
        );
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!(
            !host.take().iter().any(|call| call == "enter full screen"),
            "it asked again"
        );
        shell.update(cx, |shell, cx| shell.set_full_screen(true, cx));
        cx.run_until_parked();
        assert_eq!(
            bars(&host),
            1,
            "the bar didn't open once it was full screen"
        );
    }

    /// A window AppKit took full screen itself, as it does one opened from a full-screen window,
    /// isn't asked again, which would take it out, and opens its bar at once.
    #[gpui::test]
    fn a_new_window_already_full_screen_opens_its_bar_at_once(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        host.full_screen.set(true);
        shell.update(cx, |shell, _| {
            shell.open_bar_when_shown(Instant::now(), true, false)
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert_eq!(
            bars(&host),
            1,
            "the bar waited for a full screen the window had"
        );
        assert!(
            shell.read_with(cx, |shell, _| shell.full_screen),
            "the shell didn't take the full screen the window had"
        );
        assert_eq!(
            (host.take().iter())
                .filter(|call| *call == "enter full screen")
                .count(),
            1,
            "it didn't ask once whether the window was full screen"
        );
    }

    /// A window the shell heard was full screen before its first frame, as from the window's
    /// bounds when AppKit took it there as it opened, opens its bar at once, never asking, which
    /// would take it out (#2482).
    #[gpui::test]
    fn a_new_window_heard_full_screen_before_its_frame_opens_its_bar_at_once(
        cx: &mut TestAppContext,
    ) {
        let (shell, cx, host) = window(cx);
        shell.update(cx, |shell, cx| {
            shell.open_bar_when_shown(Instant::now(), true, false);
            shell.set_full_screen(true, cx);
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert_eq!(
            bars(&host),
            1,
            "the bar waited for a full screen the shell had"
        );
        assert!(
            !host.take().iter().any(|call| call == "enter full screen"),
            "it asked for a full screen it had"
        );
        assert!(
            shell.read_with(cx, |shell, _| shell.full_screen && shell.opening.is_none()),
            "the shell lost the full screen or kept waiting"
        );
    }

    /// While the command bar is open over the empty card, the card draws nothing, as its one offer
    /// is the bar; Esc brings it back, fading in as when the card first showed (#669).
    #[gpui::test]
    fn the_empty_card_draws_nothing_under_the_bar(cx: &mut TestAppContext) {
        use crate::shell::{Command, empty::FADE};
        use std::time::Duration;
        let (shell, cx, _) = window(cx);
        // The fade's clock is the executor's, which only the test moves on.
        let later = |cx: &mut VisualTestContext| cx.executor().now() + Duration::from_secs(1);
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let shown = |cx: &mut VisualTestContext| {
            let at = later(cx);
            shell.read_with(cx, |shell, _| shell.card_opacity(at))
        };
        assert!(shown(cx) > 0.99, "the empty card didn't show");
        for opens in [Command::NewTab, Command::EditUrl] {
            shell.update(cx, |shell, cx| shell.run(opens, "ui", cx));
            assert_eq!(
                shown(cx),
                0.,
                "{opens:?}'s bar left the card's content under it"
            );
            let at = later(cx);
            assert!(
                shell.read_with(cx, |shell, _| shell.empty_opacity(at)) > 0.99,
                "the URL field's words went with the card under {opens:?}'s bar"
            );
            shell.update(cx, |shell, cx| shell.command_bar_dismissed(cx));
            let now = cx.executor().now();
            let back = shell.read_with(cx, |shell, _| shell.card_opacity(now));
            assert!(
                back < 0.5,
                "the card came back at once, not fading in: {back}"
            );
            let after = now + Duration::from_secs_f32(FADE * 2.);
            let faded = shell.read_with(cx, |shell, _| shell.card_opacity(after));
            assert!(
                faded > 0.99,
                "the card didn't come back once the bar had gone"
            );
        }
    }
}
