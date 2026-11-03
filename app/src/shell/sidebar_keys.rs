//! The sidebar with the keyboard (#676): Focus Sidebar gives it the keys, and a ring marks the row
//! they act on. Up and down move the ring over the rows ⌘1–9 count, return shows the ringed tab and
//! gives its page the keyboard, as a click on its row does, and escape gives the page the keyboard
//! back. Arc 1.170 has this, but its key isn't measured and ⌘. is Stop Loading here, so it has no
//! key until the user binds one in Settings › Shortcuts. A key in the page, a press anywhere, a
//! field taking the keyboard or the sidebar going away ends it. It moves only the user's view, so
//! agents get no path, as with Reveal Tab in Sidebar.

use std::{cell::Cell, rc::Rc};

use gpui::{
    App, Bounds, Context, KeyBinding, Pixels, Window, actions, canvas, div, prelude::*, px,
};

use super::Shell;
use crate::tokens::shell::{KEYED, KEYED_RING};

/// The sidebar's key context while it has the keyboard, under the Browser one, so its keys win over
/// the shortcuts on the same keys, escape's Stop among them.
pub(super) const CONTEXT: &str = "SidebarKeys";

actions!(sidebar_keys, [KeyUp, KeyDown, KeyShow, KeyLeave]);

pub(super) fn bind(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", KeyUp, Some(CONTEXT)),
        KeyBinding::new("down", KeyDown, Some(CONTEXT)),
        KeyBinding::new("enter", KeyShow, Some(CONTEXT)),
        KeyBinding::new("escape", KeyLeave, Some(CONTEXT)),
    ]);
}

impl Shell {
    /// Focus Sidebar: shows the sidebar if it's hidden, docking the reveal's, and rings the tab
    /// shown's row, else the first, with the UI holding the keyboard.
    pub(super) fn focus_sidebar(&mut self, cx: &mut Context<Self>) {
        if !self.dock(cx) && self.sidebar_hidden {
            self.toggle_sidebar(cx);
        }
        let rows = self.sidebar_rows();
        let shown = Some(self.row_of(self.selected)).filter(|row| rows.contains(row));
        let Some(row) = shown.or_else(|| rows.first().copied()) else {
            return;
        };
        self.browser.focus_ui(cx);
        self.focus_keys = true;
        self.key_row(row, cx);
    }

    fn key_row(&mut self, row: u64, cx: &mut Context<Self>) {
        self.keyed = Some(row);
        self.scroll_to_row(row);
        // check.sh waits for this line.
        log!("browser: sidebar keyed {row}");
        cx.notify();
    }

    /// Moves the ring to the next row down or up, stopping at the ends.
    fn key_step(&mut self, down: bool, cx: &mut Context<Self>) {
        let Some(keyed) = self.keyed else {
            return cx.propagate();
        };
        let rows = self.sidebar_rows();
        let next = match rows.iter().position(|&row| row == keyed) {
            Some(at) if down => rows.get(at + 1),
            Some(at) => at.checked_sub(1).and_then(|at| rows.get(at)),
            // Its row went, closed or folded away.
            None => rows.first(),
        };
        if let Some(&next) = next {
            self.key_row(next, cx);
        }
    }

    fn key_show(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.keyed.take() else {
            return cx.propagate();
        };
        log!("browser: command select via sidebar keys");
        self.select(tab, cx);
        self.browser.focus_page(cx);
        cx.notify();
    }

    fn key_leave(&mut self, cx: &mut Context<Self>) {
        if self.keyed.is_none() {
            return cx.propagate();
        }
        self.leave_keys("escape", cx);
        self.browser.focus_page(cx);
    }

    /// Takes the ring away, for `why`, if a row has it.
    pub fn leave_keys(&mut self, why: &str, cx: &mut Context<Self>) {
        if self.keyed.take().is_some() {
            // check.sh waits for this line.
            log!("browser: sidebar keys left by {why}");
            cx.notify();
        }
    }

    /// Before the frame's drawn: gives the sidebar the keyboard Focus Sidebar asked for, and ends
    /// the ring once something else has it or the sidebar's gone. With no ring the root takes the
    /// keyboard back, so escape stops a load again.
    pub(super) fn step_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let gone = self.sidebar_hidden || self.library.is_some() || self.page_fullscreen;
        if self.keyed.is_some() && gone {
            self.leave_keys("sidebar", cx);
        }
        if std::mem::take(&mut self.focus_keys) && self.keyed.is_some() {
            window.focus(&self.sidebar_keys, cx);
        } else if self.keyed.is_some() && !self.sidebar_keys.is_focused(window) {
            self.leave_keys("focus", cx);
        }
        if self.keyed.is_none() && self.sidebar_keys.is_focused(window) {
            window.focus(&self.focus_handle, cx);
        }
    }

    /// The sidebar's element, taking its keys while it has the keyboard.
    pub(super) fn with_keys(&self, sidebar: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        sidebar
            .key_context(CONTEXT)
            .track_focus(&self.sidebar_keys)
            .on_action(cx.listener(|shell, _: &KeyUp, _, cx| shell.key_step(false, cx)))
            .on_action(cx.listener(|shell, _: &KeyDown, _, cx| shell.key_step(true, cx)))
            .on_action(cx.listener(|shell, _: &KeyShow, _, cx| shell.key_show(cx)))
            .on_action(cx.listener(|shell, _: &KeyLeave, _, cx| shell.key_leave(cx)))
    }

    /// The ring over `tab`'s row or tile, with corners of `radius`, if the keys are on it. It logs
    /// where it's drawn as that changes.
    pub(super) fn keyed_ring(&self, tab: u64, radius: f32) -> Option<impl IntoElement + use<>> {
        if self.keyed != Some(tab) {
            return None;
        }
        let drawn = self.keyed_drawn.clone();
        let frame = canvas(move |bounds, _, _| log_ring(&drawn, tab, bounds), |_, _, _, _| {});
        Some(
            div()
                .absolute()
                .inset_0()
                .rounded(px(radius))
                .border(px(KEYED_RING))
                .border_color(self.palette.plate(KEYED))
                .child(frame.absolute().size_full()),
        )
    }

    /// Whether a ring drawn last frame isn't any more, which the log says.
    pub(super) fn step_keyed_drawn(&self) {
        if self.keyed.is_none() && self.keyed_drawn.take().is_some() {
            log!("browser: sidebar ring hidden");
        }
    }
}

fn log_ring(drawn: &Rc<Cell<Option<(u64, Bounds<Pixels>)>>>, tab: u64, bounds: Bounds<Pixels>) {
    if drawn.replace(Some((tab, bounds))) != Some((tab, bounds)) {
        let (origin, size) = (bounds.origin, bounds.size);
        log!(
            "browser: sidebar ring {tab} at {} {} {} {}",
            f32::from(origin.x),
            f32::from(origin.y),
            f32::from(size.width),
            f32::from(size.height),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{Entity, TestAppContext, VisualTestContext};

    use crate::shell::{
        Folders, Shell,
        commands::{Command, bind_keys},
        recorder::Recorder,
    };

    fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        cx.update(bind_keys);
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            shell.set_tabs(1, vec![10, 11], vec![20, 21], Vec::new(), Folders::default(), cx);
            shell.set_selected(20, cx);
        });
        cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
        cx.run_until_parked();
        host.take();
        (shell, cx, host)
    }

    fn ringed(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (Option<u64>, Option<u64>) {
        let drawn = |shell: &Shell| shell.keyed_drawn.get().map(|(tab, _)| tab);
        shell.read_with(cx, |shell, _| (shell.keyed, drawn(shell)))
    }

    /// Focus Sidebar rings the tab shown's row, the arrows move the ring over the rows, stopping at
    /// the ends, and return shows the ringed tab and gives its page the keyboard.
    #[gpui::test]
    fn the_arrows_move_the_ring_and_return_shows_its_tab(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx), (Some(20), Some(20)));
        assert!(host.take().contains(&"focus ui".to_owned()));
        cx.simulate_keystrokes("down down");
        assert_eq!(ringed(&shell, cx), (Some(21), Some(21)), "the ring went past the last row");
        cx.simulate_keystrokes("up up up up");
        assert_eq!(ringed(&shell, cx), (Some(10), Some(10)));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        let calls = host.take();
        assert!(calls.contains(&"select 10".to_owned()), "{calls:?}");
        assert!(calls.contains(&"focus page".to_owned()), "{calls:?}");
        assert_eq!(ringed(&shell, cx), (None, None));
        // The root has the keyboard again, so the arrows are nobody's.
        cx.simulate_keystrokes("down");
        assert_eq!(ringed(&shell, cx), (None, None));
    }

    /// F6, Chrome's key for moving the keyboard out of the page, is Focus Sidebar's at first.
    #[gpui::test]
    fn f6_focuses_the_sidebar(cx: &mut TestAppContext) {
        let (shell, cx, _) = window(cx);
        cx.simulate_keystrokes("f6");
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx), (Some(20), Some(20)));
    }

    /// Escape gives the page the keyboard without showing another tab, and a collapsed Space's
    /// pinned rows are skipped as ⌘1–9 skip them.
    #[gpui::test]
    fn escape_leaves_and_collapsed_pinned_rows_are_skipped(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        let shut = Folders { pinned_shut: true, ..Folders::default() };
        shell.update(cx, |shell, cx| {
            shell.set_tabs(1, vec![10, 11], vec![20, 21], Vec::new(), shut, cx);
            shell.run(Command::FocusSidebar, "ui", cx);
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("up");
        assert_eq!(ringed(&shell, cx).0, Some(20), "the ring went to a collapsed row");
        host.take();
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        let calls = host.take();
        assert!(calls.contains(&"focus page".to_owned()), "{calls:?}");
        assert!(!calls.iter().any(|call| call.starts_with("select")), "{calls:?}");
        assert_eq!(ringed(&shell, cx), (None, None));
    }

    /// A press anywhere, or the sidebar hiding, ends the ring.
    #[gpui::test]
    fn a_press_or_hiding_the_sidebar_ends_the_ring(cx: &mut TestAppContext) {
        let (shell, cx, _) = window(cx);
        shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
        cx.run_until_parked();
        cx.simulate_click(gpui::point(gpui::px(600.), gpui::px(300.)), gpui::Modifiers::none());
        assert_eq!(ringed(&shell, cx), (None, None));

        shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx).0, Some(20));
        shell.update(cx, |shell, cx| shell.run(Command::ToggleSidebar, "ui", cx));
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx), (None, None));
    }
}
