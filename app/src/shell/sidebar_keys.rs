//! The sidebar with the keyboard (#676, #1384): Focus Sidebar gives it the keys, and a ring marks
//! the row they act on. Up and down move the ring over the rows ⌘1–9 count and the folders, right
//! opens a folder and left closes it or goes up to the folder a row is in, return shows the ringed
//! tab and gives its page the keyboard, as a click on its row does, or opens or closes a folder,
//! Close Tab closes the ringed row as its ✕ does, shift-F10 or the menu key opens its menu, and
//! escape gives the page the keyboard back. New Tab's row is one of the rows, return on it opening a
//! new tab as its click does, so an empty Space still has a row to ring and a screen reader a
//! control to name (#3151). Arc 1.170 has this, but its key isn't measured and ⌘.
//! is Stop Loading here, so it has no key until the user binds one in Settings › Shortcuts. A key
//! in the page, a press anywhere, a field taking the keyboard or the sidebar going away ends it.
//! It moves only the user's view, so agents get no path, as with Reveal Tab in Sidebar.

use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{
    App, Bounds, Context, KeyBinding, Pixels, Role, Window, actions, canvas, div, point,
    prelude::*, px,
};

use super::{Command, Shell, Slot, commands::Run};
use crate::tokens::shell::{KEYED, KEYED_RING};

/// The sidebar's key context while it has the keyboard, under the Browser one, so its keys win over
/// the shortcuts on the same keys, escape's Stop among them.
pub(super) const CONTEXT: &str = "SidebarKeys";

/// Added to the sidebar's context only while the ring is on the menu control, which then has the
/// keyboard itself. Space is a text key: under the sidebar's own context it would be on the path of
/// every field in the sidebar, a rename's or an input method's convert key among them.
const MENU_CONTEXT: &str = "MenuKeys";

/// How long the ring stays on the tab Close and Keep Pinned shows. Interim: Arc 1.170's was still
/// there 2.5 s after it was drawn, in the one recording of it, which ended there.
const ATTENTION: Duration = Duration::from_millis(2500);

/// New Tab's row among the rows the ring goes over, which are tabs' and folders' ids otherwise.
/// The core counts those from 1.
pub(super) const NEW_TAB: u64 = 0;

actions!(
    sidebar_keys,
    [
        KeyUp, KeyDown, KeyShow, KeyLeave, KeyOpen, KeyOut, KeyMenu, TabIn, BackTabIn, KeyTab,
        KeyBackTab, KeySpace
    ]
);

pub(super) fn bind(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", KeyUp, Some(CONTEXT)),
        KeyBinding::new("down", KeyDown, Some(CONTEXT)),
        KeyBinding::new("enter", KeyShow, Some(CONTEXT)),
        // Space opens the menu control's menu, as Return does (menu_keys.rs).
        KeyBinding::new("space", KeySpace, Some(MENU_CONTEXT)),
        KeyBinding::new("escape", KeyLeave, Some(CONTEXT)),
        KeyBinding::new("right", KeyOpen, Some(CONTEXT)),
        KeyBinding::new("left", KeyOut, Some(CONTEXT)),
        // The context menu's keys on Windows and Linux, which a Mac keyboard has only the first of.
        KeyBinding::new("shift-f10", KeyMenu, Some(CONTEXT)),
        KeyBinding::new("menu", KeyMenu, Some(CONTEXT)),
        // From the ring, into the page or on to the strip's next stop (page_ends.rs).
        KeyBinding::new("tab", KeyTab, Some(CONTEXT)),
        KeyBinding::new("shift-tab", KeyBackTab, Some(CONTEXT)),
        KeyBinding::new("tab", TabIn, Some(super::KEY_CONTEXT)),
        KeyBinding::new("shift-tab", BackTabIn, Some(super::KEY_CONTEXT)),
    ]);
}

impl Shell {
    /// Focus Sidebar: shows the sidebar if it's hidden, docking the reveal's, and rings the tab
    /// shown's row, else the first, with the UI holding the keyboard.
    pub(super) fn focus_sidebar(&mut self, cx: &mut Context<Self>) {
        if !self.dock(cx) && self.sidebar_hidden {
            self.toggle_sidebar(cx);
        }
        // A task shown is at the foot: its header, or the row of its page watched.
        let at_foot = self.shown_at_foot();
        if let Some(stop) = at_foot.filter(|stop| self.foot_stops().contains(stop)) {
            self.take_keyboard(cx);
            self.focus_keys = true;
            return self.key_strip(stop, cx);
        }
        let rows = self.key_rows();
        let shown =
            Some(self.row_of(self.selected)).filter(|row| *row != NEW_TAB && rows.contains(row));
        let Some(row) = shown.or_else(|| rows.first().copied()) else {
            return;
        };
        self.take_keyboard(cx);
        self.focus_keys = true;
        self.key_row(row, cx);
    }

    /// Tab and Shift+Tab with the window itself holding the keyboard, no field, row or page
    /// having it, ring a row as Focus Sidebar does. Otherwise they'd go nowhere, and the first
    /// keys a screen-reader user tries would say nothing (#3237).
    /// With a task's page watched they ring the strip under it instead, whose ring that page is
    /// in (page_ends.rs).
    pub(super) fn tab_in(&mut self, reverse: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.onboarding.is_some() || !self.focus_handle.is_focused(window) {
            return cx.propagate();
        }
        if self.tab_into_library(cx) || self.tab_into_strip(reverse, cx) {
            return;
        }
        self.run(Command::FocusSidebar, "tab", cx);
    }

    pub(super) fn key_row(&mut self, row: u64, cx: &mut Context<Self>) {
        self.keyed = Some(row);
        self.keyed_strip = None;
        self.leave_menu_keys("sidebar", cx);
        match row {
            NEW_TAB => self.scroll_to_slot(Slot::NewTab, "new tab"),
            row => self.scroll_to_row(row),
        }
        // check.sh waits for this line.
        log!("browser: sidebar keyed {row}");
        cx.notify();
    }

    /// Moves the ring to the next row down or up, stopping at the ends.
    fn key_step(&mut self, down: bool, cx: &mut Context<Self>) {
        // On the menu control Down opens its menu, which opens above it, and Up has nowhere to go.
        if self.keyed_menu.is_some() {
            if down {
                self.key_open_menu(cx);
            }
            return;
        }
        // The foot's stops go on from the list's last row: each task's header, its chip and its
        // pages' rows. With a task's page watched the list's rows aren't stops, so Up ends at the
        // foot's first.
        let foot = self.foot_stops();
        if let Some(at) = (self.keyed_strip).and_then(|stop| foot.iter().position(|&s| s == stop)) {
            let last = self.key_rows().last().copied();
            match (down, at.checked_sub(1)) {
                (true, _) => {
                    if let Some(&next) = foot.get(at + 1) {
                        self.key_strip(next, cx);
                    }
                }
                (false, Some(before)) => self.key_strip(foot[before], cx),
                (false, None) => {
                    if let Some(row) = last.filter(|_| self.watched_task_page().is_none()) {
                        self.key_row(row, cx);
                    }
                }
            }
            return;
        }
        let Some(keyed) = self.keyed else {
            return cx.propagate();
        };
        let rows = self.key_rows();
        let at = rows.iter().position(|&row| row == keyed);
        if down
            && at.is_some_and(|at| at + 1 == rows.len())
            && let Some(&stop) = foot.first()
        {
            return self.key_strip(stop, cx);
        }
        let next = match at {
            Some(at) if down => rows.get(at + 1),
            Some(at) => at.checked_sub(1).and_then(|at| rows.get(at)),
            // Its row went, closed or folded away.
            None => rows.first(),
        };
        if let Some(&next) = next {
            self.key_row(next, cx);
        }
    }

    /// The rows the ring goes over, in the sidebar's order: the favorites' tiles, then the Space's
    /// pinned rows and folders, then New Tab and Today's, past what collapsed pinned tabs and closed
    /// folders hide. Unlike `sidebar_rows`, folders are in it, as a row the keys can open.
    pub(super) fn key_rows(&self) -> Vec<u64> {
        let (pinned, today) = self.rows();
        let space = self.shown_space();
        let pinned = pinned
            .into_iter()
            .filter(|&row| !space.is_some_and(|space| space.shut(row)));
        [
            self.favorites.clone(),
            pinned.collect(),
            vec![NEW_TAB],
            today,
        ]
        .concat()
    }

    /// Opens or closes the ringed folder, as a press on its row does, the ring staying on it.
    fn key_fold(&mut self, folder: u64, open: bool, cx: &mut Context<Self>) {
        if self.folder_open(folder) == open {
            return;
        }
        log!("browser: command toggle-folder via sidebar keys");
        self.folding = Some(open);
        self.browser.set_folder_open(folder, open, cx);
        cx.notify();
    }

    /// Right: opens the ringed folder.
    fn key_open(&mut self, cx: &mut Context<Self>) {
        let Some(keyed) = self.keyed else {
            return cx.propagate();
        };
        if self.is_folder(keyed) {
            self.key_fold(keyed, true, cx);
        }
    }

    /// Left: closes the ringed folder, or, from a row in a folder, rings that folder, so a second
    /// left closes it.
    fn key_out(&mut self, cx: &mut Context<Self>) {
        let Some(keyed) = self.keyed else {
            return cx.propagate();
        };
        if self.is_folder(keyed) && self.folder_open(keyed) {
            return self.key_fold(keyed, false, cx);
        }
        let holder = self.shown_space().map_or(0, |space| space.folder_of(keyed));
        if holder != 0 {
            self.key_row(holder, cx);
        }
    }

    /// Close Tab, by its key or the menu, while a row is ringed: closes that row as its ✕ or −
    /// does, rather than the tab shown, and the ring goes to the row after it if it went. A folder
    /// isn't a tab; its menu deletes it. New Tab isn't one either.
    fn key_run(&mut self, run: &Run, cx: &mut Context<Self>) {
        // Focus Sidebar again, F6 unless the user gave it other keys, comes back round to the page,
        // as Chrome's F6 does, where it was a dead key a screen reader said nothing for (#4030).
        if run.0 == Command::FocusSidebar && self.keyed.is_some() {
            self.leave_keys("focus-sidebar", cx);
            return self.browser.focus_page(cx);
        }
        let Some(row) = self.keyed.filter(|_| run.0 == Command::CloseTab) else {
            return cx.propagate();
        };
        if row == NEW_TAB || self.is_folder(row) {
            return;
        }
        let rows = self.key_rows();
        let goes = self.today().contains(&row);
        log!("browser: command close-tab via sidebar keys");
        self.close_tab(row, true, cx);
        if goes {
            // Ahead of the core's word that it's gone, which `took_place` would answer the same.
            let left: Vec<u64> = rows.iter().copied().filter(|&each| each != row).collect();
            self.key_took_place(row, &rows, &left, cx);
        }
    }

    /// The ring's row `row` went from `before`, leaving `now`: the ring goes to the row that took
    /// its place (list_keys' rule), or ends with none left.
    fn key_took_place(&mut self, row: u64, before: &[u64], now: &[u64], cx: &mut Context<Self>) {
        match crate::list_keys::took_place(&row, before, now) {
            Some(next) => self.key_row(next, cx),
            None => self.leave_keys("close", cx),
        }
    }

    /// Shift-F10 or the menu key: the ringed row's menu, as a right-click on it shows, under it,
    /// with its first item highlighted, as a field's from the keyboard is.
    fn key_menu(&mut self, cx: &mut Context<Self>) {
        let Some(row) = self.keyed else {
            return cx.propagate();
        };
        // New Tab's row has no menu, as a right-click on it shows none.
        if row == NEW_TAB {
            return;
        }
        let drawn = self.keyed_drawn.get().filter(|&(drawn, _)| drawn == row);
        if let Some((_, bounds)) = drawn {
            self.tab_menu(row, point(bounds.left(), bounds.bottom()), cx);
            // After the host's own deferred show, where overlays open for a shortcut.
            cx.defer(crate::menu::highlight_first);
        }
    }

    fn key_show(&mut self, cx: &mut Context<Self>) {
        if self.key_open_menu(cx) {
            return;
        }
        if let Some(stop) = self.keyed_strip {
            return self.key_strip_show(stop, cx);
        }
        if let Some(folder) = self.keyed.filter(|&row| self.is_folder(row)) {
            return self.key_fold(folder, !self.folder_open(folder), cx);
        }
        let Some(tab) = self.keyed.take() else {
            return cx.propagate();
        };
        if tab == NEW_TAB {
            cx.notify();
            return self.run(Command::NewTab, "sidebar keys", cx);
        }
        log!("browser: command select via sidebar keys");
        self.select(tab, cx);
        self.browser.focus_page(cx);
        cx.notify();
    }

    fn key_leave(&mut self, cx: &mut Context<Self>) {
        if self.keyed_menu.is_some() {
            return self.give_back_menu_keys("escape", cx);
        }
        // The window keeps the keyboard: the strip's page may be an agent's.
        if self.keyed_strip.is_some() {
            return self.leave_strip("escape", cx);
        }
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
        if gone {
            self.leave_keys("sidebar", cx);
            self.leave_strip("sidebar", cx);
            self.leave_menu_keys("sidebar", cx);
        }
        // The ring's row went, closed by the user, the app or an agent, or folded away.
        if let Some(keyed) = self.keyed {
            let rows = self.key_rows();
            if !rows.contains(&keyed) {
                let before = std::mem::take(&mut self.keyed_rows);
                self.key_took_place(keyed, &before, &rows, cx);
            }
            self.keyed_rows = rows;
        } else {
            self.keyed_rows.clear();
        }
        // The strip's stop went with its page, or as its chips changed, or the foot's with its
        // task or as its group closed.
        if let Some(stop) = self.keyed_strip
            && !self.ring_stops().contains(&Some(stop))
            && !self.foot_stops().contains(&stop)
        {
            self.leave_strip("strip", cx);
        }
        let ringed = self.ring_up();
        if std::mem::take(&mut self.focus_keys) && ringed {
            window.focus(&self.sidebar_keys, cx);
        } else if ringed && !self.sidebar_keys.is_focused(window) {
            self.leave_keys("focus", cx);
            self.leave_strip("focus", cx);
            self.leave_menu_keys("focus", cx);
        }
        let ringed = self.ring_up();
        if !ringed && self.sidebar_keys.is_focused(window) {
            window.focus(&self.focus_handle, cx);
        }
    }

    /// Whether the keys' ring is on a row, the strip or the menu control.
    fn ring_up(&self) -> bool {
        self.keyed.is_some() || self.keyed_strip.is_some() || self.keyed_menu.is_some()
    }

    /// The sidebar's element, taking its keys while it has the keyboard.
    /// An assistive app reads it as the sidebar, and, as it has the keyboard, the ringed row.
    pub(super) fn with_keys(
        &self,
        sidebar: gpui::Div,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        sidebar
            .id("sidebar")
            .when(self.a11y, |sidebar| {
                sidebar.role(Role::Group).aria_label("Sidebar")
            })
            .key_context({
                let mut context = gpui::KeyContext::default();
                context.add(CONTEXT);
                if self.keyed_menu.is_some() {
                    context.add(MENU_CONTEXT);
                }
                context
            })
            .track_focus(&self.sidebar_keys)
            .on_action(cx.listener(|shell, _: &KeyUp, _, cx| shell.key_step(false, cx)))
            .on_action(cx.listener(|shell, _: &KeyDown, _, cx| shell.key_step(true, cx)))
            .on_action(cx.listener(|shell, _: &KeyShow, _, cx| shell.key_show(cx)))
            .on_action(cx.listener(|shell, _: &KeySpace, window, cx| {
                // Only the sidebar's own element holds the menu's ring, never a field in it.
                if !(shell.sidebar_keys.is_focused(window) && shell.key_open_menu(cx)) {
                    cx.propagate()
                }
            }))
            .on_action(cx.listener(|shell, _: &KeyLeave, _, cx| shell.key_leave(cx)))
            .on_action(cx.listener(|shell, _: &KeyOpen, _, cx| shell.key_open(cx)))
            .on_action(cx.listener(|shell, _: &KeyOut, _, cx| shell.key_out(cx)))
            .on_action(cx.listener(|shell, _: &KeyMenu, _, cx| shell.key_menu(cx)))
            .on_action(cx.listener(|shell, _: &KeyTab, _, cx| shell.key_tab(false, cx)))
            .on_action(cx.listener(|shell, _: &KeyBackTab, _, cx| shell.key_tab(true, cx)))
            .on_action(cx.listener(|shell, run: &Run, _, cx| shell.key_run(run, cx)))
    }

    /// The ring over `tab`'s row or tile, with corners of `radius`, if the keys are on it. It logs
    /// where it's drawn as that changes.
    pub(super) fn keyed_ring(&self, tab: u64, radius: f32) -> Option<impl IntoElement + use<>> {
        if self.ringed() != Some(tab) {
            return None;
        }
        let drawn = self.keyed_drawn.clone();
        let frame = canvas(
            move |bounds, _, _| log_ring(&drawn, tab, bounds),
            |_, _, _, _| {},
        );
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
        if self.ringed().is_none() && self.keyed_drawn.take().is_some() {
            log!("browser: sidebar ring hidden");
        }
    }

    /// The row with the ring: the keys', else the one Close and Keep Pinned showed, while it's
    /// still the tab shown.
    fn ringed(&self) -> Option<u64> {
        let shown = self.tab_asked();
        self.keyed.or_else(|| {
            self.attention
                .as_ref()
                .map(|&(tab, _)| tab)
                .filter(|&tab| tab == shown)
        })
    }

    /// Rings `tab`'s row for a moment, as Arc 1.170 does on the tab Close and Keep Pinned shows in
    /// the unloaded one's place, so the eye finds it. Its ring is its keyboard ring, so ours is too.
    pub(super) fn ring_for_attention(&mut self, tab: u64, cx: &mut Context<Self>) {
        let off = cx.spawn(async move |shell, cx| {
            cx.background_executor().timer(ATTENTION).await;
            shell
                .update(cx, |shell, cx| {
                    shell.attention = None;
                    cx.notify();
                })
                .ok();
        });
        self.attention = Some((tab, off));
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
    use std::{collections::HashMap, rc::Rc, time::Duration};

    use gpui::{Entity, Role, TestAppContext, VisualTestContext};

    use super::NEW_TAB;
    use crate::shell::{
        Folders, Page, Shell,
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
            shell.set_tabs(
                1,
                vec![10, 11],
                vec![20, 21],
                Vec::new(),
                Folders::default(),
                cx,
            );
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

    /// With the window itself holding the keyboard, Tab and Shift+Tab ring the tab shown's row as
    /// Focus Sidebar does, so a screen reader has a named row to say (#3237). Once the ring has the
    /// keyboard, Tab from any of its rows goes into the page, at its first control, and Shift+Tab
    /// at its last (#3220).
    #[gpui::test]
    fn tab_from_the_window_rings_the_tab_shown(cx: &mut TestAppContext) {
        for (key, end) in [("tab", "first"), ("shift-tab", "last")] {
            let (shell, cx, host) = window(cx);
            cx.simulate_keystrokes(key);
            cx.run_until_parked();
            assert_eq!(ringed(&shell, cx), (Some(20), Some(20)), "{key}");
            assert!(host.take().contains(&"focus ui".to_owned()), "{key}");
            cx.simulate_keystrokes("down");
            cx.simulate_keystrokes(key);
            cx.run_until_parked();
            assert_eq!(
                host.take(),
                [format!("focus page {end}")],
                "{key} from the ring"
            );
            assert_eq!(ringed(&shell, cx).0, None, "{key} took the ring off");
        }
    }

    /// Tab out of the page shown at either end rings its row, the UI taking the keyboard, and the
    /// core hears the app took it. Tab from the ring goes back in at the other end.
    #[gpui::test]
    fn tab_out_of_the_page_rings_its_row_and_back_in(cx: &mut TestAppContext) {
        for (reverse, back, end) in [(false, "shift-tab", "last"), (true, "tab", "first")] {
            let (shell, cx, host) = window(cx);
            let took = shell.update(cx, |shell, cx| shell.tab_focus_leaving(20, reverse, cx));
            cx.run_until_parked();
            assert!(took, "the app didn't take the keyboard");
            assert_eq!(
                ringed(&shell, cx),
                (Some(20), Some(20)),
                "reverse {reverse}"
            );
            assert_eq!(host.take(), ["focus ui"]);
            let focused = cx.update(|window, cx| shell.read(cx).sidebar_keys.is_focused(window));
            assert!(focused, "the ring hasn't the keyboard");
            // No copy of the page's Tab reaches the ring on any OS, Wayland included: GPUI reads
            // its own with the page's, while the page has the keys, and drops it. So the next Tab
            // goes back in however soon it comes.
            cx.simulate_keystrokes(back);
            cx.run_until_parked();
            assert_eq!(host.take(), [format!("focus page {end}")]);
        }
    }

    /// A page with nothing to focus hands the ring's Tab straight back: the ring stays on the row
    /// it was on, and nothing goes back into the page.
    #[gpui::test]
    fn a_page_with_nothing_to_focus_never_bounces(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        cx.simulate_keystrokes("tab");
        cx.simulate_keystrokes("down");
        cx.run_until_parked();
        host.take();
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(host.take(), ["focus page first"]);
        let took = shell.update(cx, |shell, cx| shell.tab_focus_leaving(20, false, cx));
        cx.run_until_parked();
        assert!(took);
        assert_eq!(ringed(&shell, cx), (Some(21), Some(21)), "the ring moved");
        assert_eq!(host.take(), ["focus ui"], "it went back into the page");
    }

    /// With no row to ring the app leaves the keyboard in the page, which wraps: a hidden
    /// sidebar, full screen, the top bar, and a tab the window doesn't show, as Peek's or another
    /// window's.
    #[gpui::test]
    fn with_no_row_to_ring_the_page_wraps(cx: &mut TestAppContext) {
        type Set = fn(&mut Shell);
        let cases: [(&str, u64, Set); 5] = [
            ("sidebar hidden", 20, |shell| shell.sidebar_hidden = true),
            ("full screen", 20, |shell| shell.page_fullscreen = true),
            ("top bar", 20, |shell| shell.top_bar = true),
            ("another window's tab", 99, |_| {}),
            ("a tab not shown", 21, |_| {}),
        ];
        for (what, tab, set) in cases {
            let (shell, cx, host) = window(cx);
            let took = shell.update(cx, |shell, cx| {
                set(shell);
                shell.tab_focus_leaving(tab, false, cx)
            });
            assert!(!took, "{what}");
            assert!(host.take().is_empty(), "{what}");
            assert_eq!(ringed(&shell, cx).0, None, "{what}");
        }
    }

    /// In a Space with no tabs, Tab rings New Tab's row, which a screen reader names, and return
    /// on it opens a new tab as its click does (#3151). Its menu key and Close Tab do nothing.
    #[gpui::test]
    fn tab_in_an_empty_space_rings_new_tab(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        cx.simulate_a11y_activation();
        shell.update(cx, |shell, cx| {
            let none = Folders::default();
            shell.set_tabs(1, Vec::new(), Vec::new(), Vec::new(), none, cx);
            shell.set_selected(0, cx);
        });
        cx.run_until_parked();
        host.take();
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx), (Some(NEW_TAB), Some(NEW_TAB)));
        let tree = cx.a11y_tree().expect("GPUI built a tree");
        let (_, node) = (tree.nodes.iter())
            .find(|(id, _)| *id == tree.focus)
            .expect("something has the keyboard");
        assert_eq!(
            (node.role(), node.label().unwrap_or_default().to_string()),
            (Role::Button, "New Tab".into()),
            "what a screen reader says"
        );
        cx.simulate_keystrokes("down up shift-f10 cmd-w");
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx).0, Some(NEW_TAB));
        let calls = host.take();
        assert!(
            !calls.iter().any(|call| call.starts_with("close")),
            "{calls:?}"
        );
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert!(host.bar_open.get(), "return opened no new tab");
        assert_eq!(ringed(&shell, cx), (None, None));
    }

    /// Space 1 holds `pinned` and `today` and shows `tab`, as the core has it once a page the
    /// command bar opened shows, or `0` for none.
    fn shows(
        shell: &Entity<Shell>,
        pinned: &[u64],
        today: &[u64],
        tab: u64,
        cx: &mut VisualTestContext,
    ) {
        shell.update(cx, |shell, cx| {
            let none = Folders::default();
            shell.set_tabs(1, pinned.to_vec(), today.to_vec(), Vec::new(), none, cx);
            shell.set_selected(tab, cx);
        });
        cx.run_until_parked();
    }

    /// New Tab, by its shortcut from a ring on a tab's row or on New Tab's, or by return on New
    /// Tab's, ends the ring as return on a tab's row does, so once the page it opens shows, Tab
    /// from the window rings that page's row rather than going nowhere (#3151).
    #[gpui::test]
    fn a_new_tab_ends_the_ring(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx).0, Some(20));
        cx.simulate_keystrokes("cmd-t");
        cx.run_until_parked();
        assert!(host.bar_open.take(), "the shortcut opened no new tab");
        assert_eq!(ringed(&shell, cx), (None, None));
        shows(&shell, &[10, 11], &[30, 20, 21], 30, cx);
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx), (Some(30), Some(30)));

        shows(&shell, &[], &[], 0, cx);
        shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx).0, Some(NEW_TAB));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert!(host.bar_open.take(), "return opened no new tab");
        assert_eq!(ringed(&shell, cx), (None, None));
        shows(&shell, &[], &[31], 31, cx);
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx), (Some(31), Some(31)));
    }

    /// Escape in the bar New Tab's shortcut opened from the ring leaves the window holding the
    /// keyboard, and Tab from there rings a row again: New Tab's, in an empty Space, which a screen
    /// reader names.
    #[gpui::test]
    fn escape_from_a_new_tab_leaves_tab_a_row_to_ring(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        cx.simulate_a11y_activation();
        shows(&shell, &[], &[], 0, cx);
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx).0, Some(NEW_TAB));
        cx.simulate_keystrokes("cmd-t");
        cx.run_until_parked();
        assert!(host.bar_open.take(), "the shortcut opened no new tab");
        shell.update(cx, |shell, cx| shell.command_bar_dismissed(cx));
        cx.run_until_parked();
        let root = cx.update(|window, cx| shell.read(cx).focus_handle.is_focused(window));
        assert!(root, "the window doesn't hold the keyboard");
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx), (Some(NEW_TAB), Some(NEW_TAB)));
        let tree = cx.a11y_tree().expect("GPUI built a tree");
        let (_, node) = (tree.nodes.iter())
            .find(|(id, _)| *id == tree.focus)
            .expect("something has the keyboard");
        assert_eq!(
            (node.role(), node.label().unwrap_or_default().to_string()),
            (Role::Button, "New Tab".into()),
            "what a screen reader says"
        );
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
        assert_eq!(
            ringed(&shell, cx),
            (Some(21), Some(21)),
            "the ring went past the last row"
        );
        cx.simulate_keystrokes("up up");
        assert_eq!(
            ringed(&shell, cx),
            (Some(NEW_TAB), Some(NEW_TAB)),
            "New Tab is a row"
        );
        cx.simulate_keystrokes("up up up");
        assert_eq!(
            ringed(&shell, cx),
            (Some(10), Some(10)),
            "the ring went past the first row"
        );
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

    /// Close and Keep Pinned on the tab shown shows the tab shown before it that's still open, a
    /// pinned one with its page ahead of the top Today tab the core would pick, and rings its row
    /// until the moment is over or another tab shows. A pinned tab without its page is passed over,
    /// so closing one loads no other.
    #[gpui::test]
    fn close_and_keep_pinned_shows_the_tab_before_and_rings_it(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        let show = |cx: &mut VisualTestContext, tab| {
            shell.update(cx, |shell, cx| shell.set_selected(tab, cx));
            cx.run_until_parked();
        };
        let close = |cx: &mut VisualTestContext| {
            host.take();
            shell.update(cx, |shell, cx| shell.run(Command::CloseTab, "ui", cx));
            let calls = host.take();
            let asked = calls
                .iter()
                .filter(|call| call.starts_with("select") || call.starts_with("close"));
            asked.cloned().collect::<Vec<_>>()
        };
        show(cx, 11);
        show(cx, 10);
        assert_eq!(close(cx), ["select 11", "close 10"]);
        show(cx, 11);
        assert_eq!(ringed(&shell, cx), (None, Some(11)));
        cx.executor().advance_clock(Duration::from_millis(2400));
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx), (None, Some(11)));
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx), (None, None), "the moment is over");

        // The core let go of 10's page.
        shell.update(cx, |shell, cx| {
            let page = Page {
                unloaded: true,
                ..shell.page(10)
            };
            shell.set_page(10, page, cx);
        });
        assert_eq!(close(cx), ["select 20", "close 11"]);
        show(cx, 20);
        assert_eq!(ringed(&shell, cx), (None, Some(20)));
        show(cx, 21);
        assert_eq!(ringed(&shell, cx), (None, None), "another tab shows");
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
        let shut = Folders {
            pinned_shut: true,
            ..Folders::default()
        };
        shell.update(cx, |shell, cx| {
            shell.set_tabs(1, vec![10, 11], vec![20, 21], Vec::new(), shut, cx);
            shell.run(Command::FocusSidebar, "ui", cx);
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("up up");
        assert_eq!(
            ringed(&shell, cx).0,
            Some(NEW_TAB),
            "the ring went to a collapsed row"
        );
        host.take();
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        let calls = host.take();
        assert!(calls.contains(&"focus page".to_owned()), "{calls:?}");
        assert!(
            !calls.iter().any(|call| call.starts_with("select")),
            "{calls:?}"
        );
        assert_eq!(ringed(&shell, cx), (None, None));
    }

    /// Focus Sidebar again with the ring up gives the page the keyboard, as escape does, so F6
    /// goes round to the page as Chrome's does (#4030).
    #[gpui::test]
    fn focus_sidebar_again_gives_the_page_the_keyboard(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        shell.update(cx, |shell, cx| {
            shell.set_tabs(
                1,
                Vec::new(),
                vec![20, 21],
                Vec::new(),
                Folders::default(),
                cx,
            );
            shell.run(Command::FocusSidebar, "ui", cx);
        });
        cx.run_until_parked();
        assert!(
            ringed(&shell, cx).0.is_some(),
            "Focus Sidebar didn't ring a row"
        );
        host.take();
        cx.simulate_keystrokes("f6");
        cx.run_until_parked();
        let calls = host.take();
        assert!(calls.contains(&"focus page".to_owned()), "{calls:?}");
        assert!(
            !calls.iter().any(|call| call.starts_with("select")),
            "{calls:?}"
        );
        assert_eq!(ringed(&shell, cx), (None, None));
    }

    /// The ring stops on a folder, right opens it and left closes it, from a row in it left goes up
    /// to it, and return opens or closes it with the ring staying on it.
    #[gpui::test]
    fn folders_take_the_ring_and_the_arrows_open_and_close_them(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        let folders = |open: bool| Folders {
            folders: HashMap::from([(5, ("Reading".into(), open))]),
            parents: HashMap::from([(12, 5)]),
            ..Folders::default()
        };
        let set = |shell: &Entity<Shell>, open: bool, cx: &mut VisualTestContext| {
            shell.update(cx, |shell, cx| {
                shell.set_tabs(
                    1,
                    vec![10, 5, 12, 11],
                    vec![20, 21],
                    Vec::new(),
                    folders(open),
                    cx,
                );
            });
            cx.run_until_parked();
        };
        set(&shell, false, cx);
        shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
        cx.run_until_parked();
        cx.simulate_keystrokes("up up");
        assert_eq!(
            ringed(&shell, cx).0,
            Some(11),
            "the ring went into a closed folder"
        );
        cx.simulate_keystrokes("up");
        assert_eq!(ringed(&shell, cx), (Some(5), Some(5)));
        host.take();
        cx.simulate_keystrokes("left");
        assert!(
            host.take().is_empty(),
            "left asked to close a closed folder"
        );
        cx.simulate_keystrokes("right");
        assert_eq!(host.take(), ["folder open 5 true"]);
        set(&shell, true, cx);
        cx.simulate_keystrokes("down");
        assert_eq!(ringed(&shell, cx).0, Some(12));
        cx.simulate_keystrokes("left");
        assert_eq!(
            ringed(&shell, cx).0,
            Some(5),
            "left didn't go up to the folder"
        );
        assert!(host.take().is_empty());
        cx.simulate_keystrokes("left");
        assert_eq!(host.take(), ["folder open 5 false"]);
        set(&shell, false, cx);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(host.take(), ["folder open 5 true"]);
        assert_eq!(
            ringed(&shell, cx),
            (Some(5), Some(5)),
            "return on a folder left the ring"
        );
    }

    /// Close Tab closes the ringed row, not the tab shown, and the ring goes to the row after it;
    /// on the last row it goes to the one before, New Tab's for Today's first, where Close Tab does
    /// nothing, and on a pinned row it stays. Elsewhere it's the shown tab's again.
    #[gpui::test]
    fn close_tab_closes_the_ringed_row(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
        cx.run_until_parked();
        host.take();
        cx.simulate_keystrokes("cmd-w");
        assert_eq!(host.take(), ["close 20"]);
        assert_eq!(ringed(&shell, cx).0, Some(21));
        shell.update(cx, |shell, cx| {
            shell.set_tabs(
                1,
                vec![10, 11],
                vec![21],
                Vec::new(),
                Folders::default(),
                cx,
            );
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("cmd-w");
        assert_eq!(host.take(), ["close 21"]);
        assert_eq!(ringed(&shell, cx).0, Some(NEW_TAB));
        cx.simulate_keystrokes("cmd-w");
        assert!(
            host.take().is_empty(),
            "Close Tab on New Tab closed something"
        );
        cx.simulate_keystrokes("up cmd-w");
        assert_eq!(host.take(), ["close 11"]);
        assert_eq!(
            ringed(&shell, cx).0,
            Some(11),
            "the ring left a pinned row that stays"
        );
        cx.simulate_keystrokes("escape");
        shell.update(cx, |shell, cx| shell.set_selected(10, cx));
        cx.run_until_parked();
        host.take();
        cx.simulate_keystrokes("cmd-w");
        assert!(host.take().contains(&"close 10".to_owned()));
    }

    /// ⌘W that comes while the App is in the update that opens a tab in front, as a nested event
    /// pump delivers it, waits for that update and closes the tab it opened, the one in front by
    /// then, once (#2672).
    #[gpui::test]
    fn close_tab_that_comes_as_a_tab_opens_closes_that_tab(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        let mut platform = cx.test_window(gpui::VisualContext::window_handle(cx));
        let keystroke = gpui::Keystroke::parse("cmd-w").unwrap();
        shell.update(cx, |shell, cx| {
            let today = vec![20, 21, 22];
            shell.set_tabs(1, vec![10, 11], today, Vec::new(), Folders::default(), cx);
            shell.set_selected(22, cx);
            platform.simulate_input(gpui::PlatformInput::KeyDown(gpui::KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            }));
            platform.simulate_input(gpui::PlatformInput::KeyUp(gpui::KeyUpEvent { keystroke }));
        });
        cx.run_until_parked();
        let closed: Vec<_> = (host.take().into_iter())
            .filter(|call| call.starts_with("close "))
            .collect();
        assert_eq!(closed, ["close 22"]);
    }

    /// Shift-F10 and the menu key open the ringed row's menu under it, as a right-click does.
    #[gpui::test]
    fn the_menu_key_opens_the_ringed_rows_menu(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
        cx.run_until_parked();
        cx.simulate_keystrokes("down shift-f10");
        let ring = shell
            .read_with(cx, |shell, _| shell.keyed_drawn.get())
            .expect("a ring")
            .1;
        assert_eq!(
            host.menu_at.get(),
            Some(gpui::point(ring.left(), ring.bottom()))
        );
        let menu = host.menus.borrow().last().cloned().expect("a menu");
        assert!(
            menu.iter()
                .any(|(title, _)| title.as_ref() == "Archive Tab"),
            "{menu:?}"
        );
        host.take();
        cx.update(|_, cx| host.choose("Archive Tab", cx));
        cx.run_until_parked();
        assert!(host.take().contains(&"close 21".to_owned()));
    }

    /// The ringed row closed by the app or an agent, not the keys, gives the ring to the row that
    /// took its place: the next, else the one before (#4108).
    #[gpui::test]
    fn a_ringed_row_closed_elsewhere_gives_the_ring_to_the_row_in_its_place(
        cx: &mut TestAppContext,
    ) {
        let (shell, cx, _) = window(cx);
        let today = |tabs: Vec<u64>, shell: &Entity<Shell>, cx: &mut VisualTestContext| {
            shell.update(cx, |shell, cx| {
                shell.set_tabs(1, vec![10, 11], tabs, Vec::new(), Folders::default(), cx)
            });
            cx.run_until_parked();
        };
        today(vec![20, 21, 22], &shell, cx);
        shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx).0, Some(20));
        today(vec![21, 22], &shell, cx);
        assert_eq!(
            ringed(&shell, cx),
            (Some(21), Some(21)),
            "the next row didn't take it"
        );
        cx.simulate_keystrokes("down");
        today(vec![21], &shell, cx);
        assert_eq!(
            ringed(&shell, cx).0,
            Some(21),
            "the last row's ring went nowhere"
        );
        cx.simulate_keystrokes("down");
        assert_eq!(
            ringed(&shell, cx).0,
            Some(21),
            "the ring didn't stop at the end"
        );
    }

    /// A choice in the ringed row's menu that leaves the row gives the keyboard back to it: the
    /// user is still where they were (#4108).
    #[gpui::test]
    fn a_choice_in_the_ringed_rows_menu_leaves_the_ring_on_it(cx: &mut TestAppContext) {
        let (shell, cx, host) = window(cx);
        shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
        cx.run_until_parked();
        cx.simulate_keystrokes("down shift-f10");
        cx.update(|_, cx| host.choose("Duplicate", cx));
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx).0, Some(21));
        assert!(
            cx.update(|window, cx| shell.read(cx).sidebar_keys.is_focused(window)),
            "the ring's row lost the keyboard"
        );
    }

    /// A press anywhere, or the sidebar hiding, ends the ring.
    #[gpui::test]
    fn a_press_or_hiding_the_sidebar_ends_the_ring(cx: &mut TestAppContext) {
        let (shell, cx, _) = window(cx);
        shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
        cx.run_until_parked();
        cx.simulate_click(
            gpui::point(gpui::px(600.), gpui::px(300.)),
            gpui::Modifiers::none(),
        );
        assert_eq!(ringed(&shell, cx), (None, None));

        shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx).0, Some(20));
        shell.update(cx, |shell, cx| shell.run(Command::ToggleSidebar, "ui", cx));
        cx.run_until_parked();
        assert_eq!(ringed(&shell, cx), (None, None));
    }
}
