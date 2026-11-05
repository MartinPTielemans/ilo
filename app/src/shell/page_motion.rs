//! Telling the core while the page's frame moves on its own, so a page that hasn't drawn at its
//! new size yet shows its last frame stretched to fill the view, and the edge that doesn't move,
//! with the scrollbar, stays still (issue #166).

use gpui::Context;

use super::Shell;

impl Shell {
    /// Whether something moves the page's size a step at a time: the sidebar hiding, showing or
    /// settling on a width, its edge or a split view's gap or the side panel's edge dragged. The
    /// Library pushes the card without resizing the page, and the reveal goes over it.
    fn page_moving(&self) -> bool {
        self.sidebar_motion.is_some()
            || self.width_motion.is_some()
            || self.resize.is_some()
            || self.pane_resize.is_some()
            || self.side_panel_resize.is_some()
    }

    /// Tells the core the page starts moving, if it does and the core wasn't told. Before the
    /// motion's first step is laid out, so that step stretches.
    pub(super) fn start_page_motion(&mut self, cx: &mut Context<Self>) {
        if self.page_moving() && !self.page_moving_told {
            self.page_moving_told = true;
            self.browser.page_moving(true, cx);
            // check.sh waits for these lines.
            log!("browser: page moving 1");
        }
    }

    /// Tells the core the page stopped, if it did and the core wasn't told. After the layout that
    /// puts it at rest, which is the motion's last step and stretches too.
    pub(super) fn stop_page_motion(&mut self, cx: &mut Context<Self>) {
        if !self.page_moving() && self.page_moving_told {
            self.page_moving_told = false;
            self.browser.page_moving(false, cx);
            log!("browser: page moving 0");
        }
    }

    /// Lays the page out for this frame, with the core told when it starts and stops moving. A
    /// second ⌘S mid-motion, or a drag right after a settle, keeps it moving without a stop.
    pub(super) fn lay_out_moving_page(&mut self, cx: &mut Context<Self>) {
        self.start_page_motion(cx);
        self.lay_out_page(cx);
        self.stop_page_motion(cx);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        rc::Rc,
        time::{Duration, Instant},
    };

    use gpui::{Entity, MouseButton, MouseDownEvent, TestAppContext, VisualTestContext, point, px};

    use super::super::{Folders, SIDEBAR_WIDTH, Shell, Split, recorder::Recorder};

    /// A shell drawn in a window with a page, or with a split view of two, and the calls it has
    /// made to the core since the first draw.
    fn drawn(
        cx: &mut TestAppContext,
        split: bool,
    ) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
        let host = Rc::new(Recorder::default());
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host.clone(), cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Work".into(), None, 0, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            let splits = match split {
                true => vec![Split {
                    tabs: vec![1, 2],
                    stacked: false,
                    ..Split::default()
                }],
                false => Vec::new(),
            };
            shell.set_tabs(1, Vec::new(), vec![1, 2], splits, Folders::default(), cx);
            shell.set_selected(1, cx);
        });
        cx.run_until_parked();
        host.page_events.borrow_mut().clear();
        (shell, host, cx)
    }

    /// Draws the shell again, as the app's next frame does.
    fn draw(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }

    /// Makes the sidebar's motion `ago` old, so the next draw is that far along.
    fn age_sidebar_motion(shell: &Entity<Shell>, cx: &mut VisualTestContext, ago: Duration) {
        shell.update(cx, |shell, _| {
            let (from, _) = shell.sidebar_motion.expect("the sidebar moves");
            shell.sidebar_motion = Some((from, Instant::now() - ago));
        });
    }

    /// What the shell told the core about the motion, without the layouts between.
    fn told(host: &Recorder) -> Vec<&'static str> {
        let events = host.page_events.borrow();
        events
            .iter()
            .copied()
            .filter(|&event| event != "layout")
            .collect()
    }

    /// Presses the edge or gap at `x`, as the pointer would.
    fn press_at(x: f32) -> MouseDownEvent {
        MouseDownEvent {
            button: MouseButton::Left,
            position: point(px(x), px(300.)),
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }
    }

    /// ⌘S: the core hears the page moves before the first step is laid out, and that it stopped
    /// after the last, which lays it out at rest. Nothing is said at rest.
    #[gpui::test]
    fn hiding_the_sidebar_moves_the_page_from_its_first_step_to_its_last(cx: &mut TestAppContext) {
        let (shell, host, cx) = drawn(cx, false);
        shell.update(cx, |shell, cx| shell.toggle_sidebar(cx));
        age_sidebar_motion(&shell, cx, Duration::from_millis(40));
        draw(&shell, cx);
        assert_eq!(host.page_events.borrow().first(), Some(&"moving"));
        assert_eq!(told(&host), ["moving"]);
        age_sidebar_motion(&shell, cx, Duration::from_secs(1));
        draw(&shell, cx);
        assert_eq!(told(&host), ["moving", "still"]);
        let events = host.page_events.borrow().clone();
        assert_eq!(
            &events[events.len() - 2..],
            ["layout", "still"],
            "{events:?}"
        );
        host.page_events.borrow_mut().clear();
        draw(&shell, cx);
        assert!(host.page_events.borrow().is_empty());
    }

    /// ⌘S again before the motion is over turns it around, and the core still hears one start and
    /// one stop.
    #[gpui::test]
    fn turning_the_sidebar_around_midway_does_not_stop_the_page(cx: &mut TestAppContext) {
        let (shell, host, cx) = drawn(cx, false);
        shell.update(cx, |shell, cx| shell.toggle_sidebar(cx));
        age_sidebar_motion(&shell, cx, Duration::from_millis(40));
        draw(&shell, cx);
        shell.update(cx, |shell, cx| shell.toggle_sidebar(cx));
        age_sidebar_motion(&shell, cx, Duration::from_millis(30));
        draw(&shell, cx);
        assert_eq!(told(&host), ["moving"]);
        age_sidebar_motion(&shell, cx, Duration::from_secs(1));
        draw(&shell, cx);
        assert_eq!(told(&host), ["moving", "still"]);
    }

    /// The sidebar's edge dragged: the page moves from the press until the pointer lets go, which
    /// here is where it was, so there's no width left to settle.
    #[gpui::test]
    fn dragging_the_sidebars_edge_moves_the_page_until_it_is_let_go(cx: &mut TestAppContext) {
        let (shell, host, cx) = drawn(cx, false);
        shell.update(cx, |shell, cx| {
            shell.press_edge(&press_at(shell.width() - 3.), cx);
            shell.drag_edge(SIDEBAR_WIDTH + 40., cx);
        });
        cx.run_until_parked();
        assert_eq!(told(&host), ["moving"]);
        shell.update(cx, |shell, cx| shell.let_go_edge(cx));
        cx.run_until_parked();
        assert_eq!(told(&host), ["moving", "still"]);
        let events = host.page_events.borrow().clone();
        assert_eq!(events.last(), Some(&"still"), "{events:?}");
    }

    /// A split view's gap dragged resizes its pages through the core, not the page's frame: the
    /// core hears the page moves before the first new share, and stops when the pointer lets go.
    #[gpui::test]
    fn dragging_a_splits_gap_moves_its_pages_until_it_is_let_go(cx: &mut TestAppContext) {
        let (shell, host, cx) = drawn(cx, true);
        shell.update(cx, |shell, cx| {
            let gap = shell.shown_pane_edges()[1];
            shell.press_gap(1, &press_at(gap - 3.), cx);
            shell.drag_gap(gap + 60., cx);
        });
        assert_eq!(told(&host), ["moving", "shares"]);
        shell.update(cx, |shell, cx| shell.let_go_gap(cx));
        assert_eq!(told(&host), ["moving", "shares", "still"]);
    }
}
