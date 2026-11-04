//! ⌘N's and ⇧⌘N's window opens with ⌘T's command bar, as Arc's does, so typing goes straight to a
//! page: once the window's first frame is on screen, and with the window it came from full screen,
//! once it's full screen too (docs/ux-spec.md › Window › New window).

use std::time::Instant;

use gpui::{Context, Window};

use crate::shell::{Command, Shell};

/// A window ⌘N or ⇧⌘N opened, until its command bar opens.
pub(crate) struct Opening {
    /// When the key was pressed, for the first frame's time.
    asked: Instant,
    /// Whether it goes full screen first, as the window it came from was.
    full_screen: bool,
    /// Whether it has asked to go full screen.
    entering: bool,
    /// Whether its first frame on screen has been drawn.
    drawn: bool,
}

impl Shell {
    /// Whether the window is full screen of its own, not for a page: a window ⌘N opens from it
    /// goes full screen too.
    pub fn in_window_full_screen(&self) -> bool {
        self.full_screen && !self.page_fullscreen
    }

    /// The window was opened by ⌘N or ⇧⌘N at `asked`: it opens the command bar once it's on
    /// screen, going full screen first if `full_screen`.
    pub fn open_bar_when_shown(&mut self, asked: Instant, full_screen: bool) {
        self.opening = Some(Opening {
            asked,
            full_screen,
            entering: false,
            drawn: false,
        });
    }

    /// Before the frame's drawn: once the window is on screen, the command bar, or first full
    /// screen. GPUI draws a window only once it's visible, some frames after it's put on screen,
    /// so a bar opened before would open over nothing.
    pub(super) fn step_opening(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(opening) = self.opening.as_mut() else {
            return;
        };
        if !window.is_visible() {
            return;
        }
        if !std::mem::replace(&mut opening.drawn, true) {
            // check.sh reads this line, the new window's time from the key to its first frame.
            let ms = opening.asked.elapsed().as_secs_f64() * 1000.;
            log!("browser: new window first frame at {ms:.1} ms");
        }
        // The bar waits for the window's full screen, as macOS moves the window to a desktop of
        // its own on the way, and the bar would be left on the one it came from. AppKit can have
        // taken it there itself, as it does a window opened from a full-screen one.
        let entering = opening.entering;
        if opening.full_screen && !self.full_screen {
            opening.entering = true;
            if entering {
                return;
            }
            let already = self.browser.enter_full_screen(cx);
            // check.sh reads this line.
            log!(
                "browser: new window asked for full screen, already {}",
                already as u8
            );
            if !already {
                return;
            }
            // AppKit tells of the full screen it gave the window as it opened before the window
            // can hear it, so the shell takes it from here: its layout and the note follow.
            self.set_full_screen(true, cx);
        }
        if self.opening.take().is_some_and(|opening| opening.entering) {
            // check.sh reads this line.
            log!("browser: new window went full screen");
        }
        // After this frame, as running it updates the shell.
        cx.defer_in(window, |shell, _, cx| {
            shell.run(Command::NewTab, "new window", cx)
        });
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
            shell.open_bar_when_shown(Instant::now(), false)
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

    /// From a full-screen window, it goes full screen first, and the bar waits for that.
    #[gpui::test]
    fn a_new_window_from_full_screen_goes_full_screen_before_its_bar(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        shell.update(cx, |shell, _| {
            shell.open_bar_when_shown(Instant::now(), true)
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
            shell.open_bar_when_shown(Instant::now(), true)
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
}
