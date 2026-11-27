//! How the user left the window, which the core keeps with it for the next launch and never syncs
//! (#4415): the hidden sidebar, Stay On Top, ⌃Tab's order and each Space's scroll. A private
//! window's core keeps none of it.

use gpui::{App, Context, point, px};

use super::Shell;

impl Shell {
    /// Takes the view the core brought the window back with: as the window is made, and again for
    /// the launch's first window once the core has brought the Spaces back into it.
    pub fn read_view(&mut self, cx: &mut Context<Self>) {
        let view = self.browser.saved_view();
        self.sidebar_hidden = view.sidebar_hidden;
        self.sidebar_motion = None;
        let on_top = view.on_top && self.browser.can_stay_on_top();
        if on_top != self.on_top {
            self.on_top = on_top;
            self.browser.set_on_top(on_top, cx);
            self.update_menus(cx);
        }
        // Tabs shown since go first.
        for tab in view.recent {
            if !self.recent.contains(&tab) {
                self.recent.push(tab);
            }
        }
        self.space_scrolls.clear();
        let space = self.space;
        self.scroll_to_space(space);
        cx.notify();
    }

    /// Hides the docked sidebar or shows it, and keeps that for the next launch.
    pub(super) fn set_sidebar_hidden(&mut self, hidden: bool, cx: &mut App) {
        if std::mem::replace(&mut self.sidebar_hidden, hidden) != hidden {
            self.browser.keep_sidebar_hidden(hidden, cx);
        }
    }

    /// Keeps how far down the shown Space's list is scrolled: as the window leaves the Space,
    /// closes or quits, and as the pointer leaves the sidebar it scrolled. Not as it scrolls.
    pub(super) fn keep_space_scroll(&mut self, cx: &mut App) {
        let space = self.space;
        let at = (-f32::from(self.list_scroll.offset().y)).max(0.);
        if space == 0 || self.space_scrolls.get(&space) == Some(&at) {
            return;
        }
        self.space_scrolls.insert(space, at);
        self.browser.keep_space_scroll(space, at, cx);
    }

    /// Scrolls the list to where `space`'s was left, in this run or the last.
    pub(super) fn scroll_to_space(&mut self, space: u64) {
        if space == 0 {
            return;
        }
        let browser = self.browser.clone();
        let at =
            *(self.space_scrolls.entry(space)).or_insert_with(|| browser.saved_space_scroll(space));
        self.list_scroll.set_offset(point(px(0.), px(-at)));
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui::{Entity, TestAppContext, VisualTestContext, point, px};

    use crate::shell::{Shell, recorder::Recorder};

    /// A window on `host`, as a relaunch makes one where the core kept the last one's view.
    fn window<'a>(
        host: &Rc<Recorder>,
        cx: &'a mut TestAppContext,
    ) -> (Entity<Shell>, &'a mut VisualTestContext) {
        let browser = host.clone();
        cx.add_window_view(|_, cx| Shell::new(browser, cx))
    }

    /// The hidden sidebar, Stay On Top and ⌃Tab's order the user left come back with the next
    /// launch's window, by the core: the recorder keeps them as it does.
    #[gpui::test]
    fn a_relaunch_brings_back_the_hidden_sidebar_stay_on_top_and_the_order(
        cx: &mut TestAppContext,
    ) {
        let host = Rc::new(Recorder::default());
        let (shell, vcx) = window(&host, cx);
        shell.update(vcx, |shell, cx| {
            shell.set_sidebar_hidden(true, cx);
            shell.toggle_on_top(cx);
            for tab in [3, 1, 2] {
                shell.shown(tab, cx);
            }
        });
        let (relaunched, vcx) = window(&host, cx);
        relaunched.read_with(vcx, |shell, _| {
            assert!(shell.sidebar_hidden, "the hidden sidebar came back shown");
            assert!(shell.on_top, "Stay On Top didn't come back");
            assert_eq!(shell.recent, [2, 1, 3], "⌃Tab's order didn't come back");
        });
        // Shown again, and back on top no longer, it's kept so.
        relaunched.update(vcx, |shell, cx| {
            shell.set_sidebar_hidden(false, cx);
            shell.toggle_on_top(cx);
        });
        let (again, vcx) = window(&host, cx);
        again.read_with(vcx, |shell, _| {
            assert!(!shell.sidebar_hidden && !shell.on_top)
        });
    }

    /// Each Space keeps its own scroll: the list leaves one where it was and takes the next's,
    /// and the next launch takes them back from the core.
    #[gpui::test]
    fn each_space_keeps_its_scroll_across_a_relaunch(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let (shell, vcx) = window(&host, cx);
        let scrolled = |shell: &Shell| -f32::from(shell.list_scroll.offset().y);
        shell.update(vcx, |shell, cx| {
            shell.set_space(1, cx);
            shell.list_scroll.set_offset(point(px(0.), px(-120.)));
            shell.set_space(2, cx);
            assert_eq!(scrolled(shell), 0., "Space 2's list took Space 1's scroll");
            shell.list_scroll.set_offset(point(px(0.), px(-40.)));
            shell.set_space(1, cx);
            assert_eq!(scrolled(shell), 120., "Space 1's list lost its scroll");
            shell.about_to_close(cx);
        });
        let (relaunched, vcx) = window(&host, cx);
        relaunched.update(vcx, |shell, cx| {
            shell.set_space(2, cx);
            assert_eq!(scrolled(shell), 40.);
            shell.set_space(1, cx);
            assert_eq!(scrolled(shell), 120.);
        });
    }
}
