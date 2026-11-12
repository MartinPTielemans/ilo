//! A split view's pages in the card, and the gaps between them, which drag to resize
//! them (docs/ux-spec.md › Split view).

use gpui::{
    AnyElement, Bounds, Context, CursorStyle, DispatchPhase, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, Size, canvas, div, point, prelude::*, px, size,
};

use super::{
    CARD_INSET, COPIED_TIME, EXPANDED_OTHER, MAX_PANES, MenuAction, MenuEntry, PANE_LEAST,
    PANE_SNAP, SPLIT_GAP, Shell, access::Accessible,
};
use crate::tokens::shell::split_controls::{DOT, DOT_GAP, DOTS, DOTS_HOVERED, INSET, WIDTH};

/// Arc's toast for a page that a full split view can't take (`maxNumberPanes`).
const FULL_SPLIT: &str = "Cannot Add New Pane";

impl Shell {
    /// The gaps between the pages of the split view shown, which drag the pages' shares of the
    /// card and even them on a double-click.
    pub(super) fn pane_gaps(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let panes = self.panes(self.selected);
        if panes.len() < 2 || self.adding_pane.is_some() || self.page_fullscreen {
            return Vec::new();
        }
        let (frame, _) = self.page_frame(self.viewport);
        let stacked = self.stacked(self.selected);
        let frames = pane_frames(frame, panes.len(), stacked, &self.shares(self.selected));
        let cursor = if stacked {
            CursorStyle::ResizeUpDown
        } else {
            CursorStyle::ResizeLeftRight
        };
        let mut gaps: Vec<AnyElement> = frames
            .windows(2)
            .enumerate()
            .map(|(index, pair)| {
                let gap = match stacked {
                    true => Bounds::from_corners(pair[0].bottom_left(), pair[1].top_right()),
                    false => Bounds::from_corners(pair[0].top_right(), pair[1].bottom_left()),
                };
                div()
                    .id(("pane-gap", index))
                    .absolute()
                    .left(gap.left())
                    .top(gap.top())
                    .w(gap.size.width)
                    .h(gap.size.height)
                    .cursor(cursor)
                    .block_mouse_except_scroll()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                            shell.press_gap(index + 1, event, cx)
                        }),
                    )
                    .into_any_element()
            })
            .collect();
        // The pointer's moves and release anywhere in the window, while a gap is dragged.
        let resizing = self.pane_resize.is_some();
        let shell = cx.weak_entity();
        let listener = canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                if resizing {
                    window.set_window_cursor_style(cursor);
                }
                let moved = shell.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture {
                        let at = if stacked {
                            event.position.y
                        } else {
                            event.position.x
                        };
                        moved
                            .update(cx, |shell, cx| shell.drag_gap(at.into(), cx))
                            .ok();
                    }
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                        shell.update(cx, |shell, cx| shell.let_go_gap(cx)).ok();
                    }
                });
            },
        );
        gaps.push(listener.absolute().size_0().into_any_element());
        gaps
    }

    /// Each page's split controls: a handle in the strip over it, the window's margin over the card
    /// or the gap under the page above it stacked, whose menu changes the split's orientation, takes
    /// the page out of it or closes it. Its look and place are ours, as Arc's aren't measured.
    pub(super) fn pane_handles(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let panes = self.panes(self.selected);
        if panes.len() < 2 || self.adding_pane.is_some() || self.page_fullscreen {
            return Vec::new();
        }
        let (frame, _) = self.page_frame(self.viewport);
        let stacked = self.stacked(self.selected);
        let frames = pane_frames(frame, panes.len(), stacked, &self.shares(self.selected));
        let (rest, hovered) = (self.palette.faint(DOTS), self.palette.faint(DOTS_HOVERED));
        let plate = self.palette.plate(crate::palette::HOVERED_PLATE);
        let mut handles = Vec::new();
        for (index, (tab, pane)) in panes.into_iter().zip(frames).enumerate() {
            let strip = if stacked && index > 0 {
                SPLIT_GAP
            } else {
                f32::from(frame.top())
            };
            let dots = (0..3).map(|_| div().size(px(DOT)).rounded_full().bg(rest));
            let handle = div()
                .id(("split-controls", index))
                .group("split-controls")
                .when(self.a11y, |handle| {
                    handle
                        .button_named("Split Controls")
                        .pressed_by_mouse_down(true, false)
                })
                .absolute()
                .left(pane.center().x - px(WIDTH / 2.))
                .top(pane.top() - px(strip))
                .w(px(WIDTH))
                .h(px(strip))
                .flex()
                .items_center()
                .justify_center()
                .block_mouse_except_scroll()
                .on_mouse_down(
                    MouseButton::Left,
                    // At the press, which the menu takes for the click that opened it, so the
                    // click's release leaves it open.
                    cx.listener(move |shell, event: &MouseDownEvent, _, cx| {
                        shell.split_controls(tab, event.position, cx)
                    }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap(px(DOT_GAP))
                        .w_full()
                        .h(px(strip - 2. * INSET))
                        .rounded(px((strip - 2. * INSET) / 2.))
                        .group_hover("split-controls", |style| style.bg(plate))
                        .children(dots.map(move |dot| {
                            dot.group_hover("split-controls", move |style| style.bg(hovered))
                        })),
                );
            handles.push(handle.into_any_element());
        }
        handles
    }

    /// The menu of `tab`'s split controls, under its handle at `position`: Convert to Vertical or
    /// Horizontal Split View for the split, and Separate Page from Split View and Close this Split
    /// Pane for `tab`, as the View menu's commands do for the focused page.
    pub(super) fn split_controls(
        &mut self,
        tab: u64,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let turn = super::commands::orientation_title(self.stacked(tab));
        let items: Vec<(MenuEntry, MenuAction)> = vec![
            (
                MenuEntry::new(turn, true),
                Box::new(move |shell, cx| {
                    // check.sh waits for this line.
                    log!("browser: command change-orientation via split controls");
                    let stacked = shell.stacked(tab);
                    shell.stack_split(tab, !stacked, cx)
                }),
            ),
            (
                MenuEntry::new("Separate Page from Split View", true),
                Box::new(move |shell, cx| {
                    log!("browser: command separate-pane via split controls");
                    shell.separate_tab(tab, cx)
                }),
            ),
            (
                MenuEntry::new("Close this Split Pane", true),
                Box::new(move |shell, cx| {
                    log!("browser: command close-split-pane via split controls");
                    shell.close_tab(tab, false, cx)
                }),
            ),
        ];
        // Over the card, never in the hidden sidebar's reveal.
        self.show_menu_in(items, position, None, false, cx);
    }

    /// Where the pages of the split view shown start, and one after them would.
    pub(super) fn shown_pane_edges(&self) -> Vec<f32> {
        let (frame, _) = self.page_frame(self.viewport);
        let count = self.panes(self.selected).len();
        pane_edges(
            frame,
            count,
            self.stacked(self.selected),
            &self.shares(self.selected),
        )
    }

    pub(super) fn press_gap(
        &mut self,
        pane: usize,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        if event.click_count == 2 {
            self.pane_resize = None;
            return self.browser.set_split_shares(self.selected, None, cx);
        }
        let at = match self.stacked(self.selected) {
            true => event.position.y,
            false => event.position.x,
        };
        let edges = self.shown_pane_edges();
        self.pane_resize = edges.get(pane).map(|&edge| (pane, f32::from(at) - edge));
        cx.notify();
    }

    /// The pointer let go of the gap it dragged. The pages are where it left them, and the core,
    /// which laid them out as the gap moved, hears they're still.
    pub(super) fn let_go_gap(&mut self, cx: &mut Context<Self>) {
        self.pane_resize = None;
        self.stop_page_motion(cx);
    }

    /// Moves the start of the page being resized with the pointer, `at` across the window, or
    /// down it stacked, taking from the page before it or giving to it.
    pub(super) fn drag_gap(&mut self, at: f32, cx: &mut Context<Self>) {
        let Some((pane, grab)) = self.pane_resize else {
            return;
        };
        let mut edges = self.shown_pane_edges();
        let count = edges.len() - 1;
        if pane == 0 || pane >= count {
            return;
        }
        let (start, span) = (edges[0], edges[count] - edges[0]);
        let least = ((span - SPLIT_GAP) * 0.1)
            .clamp(PANE_LEAST.0, PANE_LEAST.1)
            .round();
        let low = edges[pane - 1] + least + SPLIT_GAP;
        let high = edges[pane + 1] - SPLIT_GAP - least;
        if low > high {
            return;
        }
        let even = |index: usize| start + (span * index as f32 / count as f32).floor();
        let mut edge = (at - grab).round().clamp(low, high);
        if (edge - even(pane)).abs() < PANE_SNAP {
            edge = even(pane);
        }
        if edge == edges[pane] {
            return;
        }
        edges[pane] = edge;
        // The core lays the pages out as it takes the shares, not in the next frame.
        self.start_page_motion(cx);
        let evened = (0..=count).all(|index| edges[index] == even(index));
        let shares = edge_shares(&edges);
        self.browser
            .set_split_shares(self.selected, (!evened).then_some(shares.as_slice()), cx);
    }

    /// ⌃+: opens the command bar over a new pane right of the pages shown, which make room for it
    /// in the same frame (Arc). What it opens shows there, in a split view with the tab shown.
    pub(super) fn add_pane(&mut self, cx: &mut Context<Self>) {
        let with = self.selected;
        let count = self.panes(with).len();
        if with == 0 || self.split_full(with, cx) {
            return;
        }
        self.adding_pane = Some(with);
        self.lay_out_page(cx);
        let (card, _) = self.card_frames(self.viewport);
        let pane = pane_frames(card, count + 1, self.stacked(with), &[])[count];
        self.browser.split_bar(with, pane, card, cx);
        cx.notify();
    }

    /// A tab menu's Add Split View: ⌃+ beside `tab`, which shows first if it isn't. Agents reach
    /// the same split through `bc_tab_split_with`.
    pub(super) fn add_split_to(&mut self, tab: u64, cx: &mut Context<Self>) {
        if self.panes(self.selected).contains(&tab) {
            return self.add_pane(cx);
        }
        self.split_pending = Some(tab);
        self.select(tab, cx);
    }

    /// Duplicate on a split's row copies the whole split, as Arc's does: each page with its history,
    /// joined again in the same order and orientation, with the split's name and icon. A tab alone
    /// is copied alone.
    pub(super) fn duplicate_row(&mut self, tab: u64, cx: &mut Context<Self>) {
        let Some((panes, look)) = self.split_of(tab) else {
            self.browser.duplicate(tab, cx);
            return;
        };
        let copies: Vec<u64> = panes
            .iter()
            .map(|&pane| self.browser.duplicate(pane, cx))
            .collect();
        if !copies.contains(&0) {
            self.join_again(&copies, &look, cx);
        }
    }

    /// ⌥-click on `tab`'s row or segment in the sidebar, as Arc's: a split's tab leaves it, the
    /// page shown alone splits with a copy of itself, and another tab joins the page shown, unless
    /// its split is full. Returns false where none applies, so the press shows the tab as a plain
    /// one does.
    pub(super) fn alt_press_row(&mut self, tab: u64, cx: &mut Context<Self>) -> bool {
        if self.is_folder(tab) || self.selected == 0 {
            return false;
        }
        log!("browser: command split via sidebar");
        if self.panes(tab).len() > 1 {
            self.separate_tab(tab, cx);
        } else if tab == self.selected {
            let copy = self.browser.duplicate(tab, cx);
            if copy != 0 {
                self.browser.split_with(copy, tab, false, cx);
                self.split_opened(copy, cx);
            }
        } else if !self.split_full(self.selected, cx) {
            self.join_split(tab, self.selected, false, false, cx);
        }
        cx.notify();
        true
    }

    /// Whether `tab`'s split view has as many pages as one can: a page asked to join it doesn't,
    /// and Arc's toast says why.
    pub(super) fn split_full(&self, tab: u64, cx: &mut Context<Self>) -> bool {
        let full = self.panes(tab).len() >= MAX_PANES;
        if full {
            self.browser.toast(FULL_SPLIT, None, COPIED_TIME, cx);
        }
        full
    }

    /// Expand Current Split: the focused page takes the card and each other page keeps 220 pt, in
    /// the same frame (Arc). The core holds it as a flag on the page and works the widths out at
    /// every layout, so a resized window keeps the 220. Dragging a gap, evening the pages or a
    /// page joining or leaving ends it. Done again it asks the core for nothing.
    pub(super) fn expand_split(&mut self, cx: &mut Context<Self>) {
        if self.panes(self.selected).len() > 1 && self.expanded(self.selected) != self.selected {
            self.browser.set_split_expanded(self.selected, true, cx);
        }
    }

    /// Focuses the split view's page right of the focused one, or left of it, round the ends.
    pub(super) fn focus_pane(&mut self, forward: bool, cx: &mut Context<Self>) {
        let panes = self.panes(self.selected);
        let count = panes.len();
        let Some(at) = panes
            .iter()
            .position(|&tab| tab == self.selected)
            .filter(|_| count > 1)
        else {
            return;
        };
        let next = if forward {
            (at + 1) % count
        } else {
            (at + count - 1) % count
        };
        self.select(panes[next], cx);
    }

    /// Focuses the split view's page at `index` from the left, if it has one there.
    pub(super) fn focus_pane_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let panes = self.panes(self.selected);
        if panes.len() > 1
            && let Some(&tab) = panes.get(index)
            && tab != self.selected
        {
            self.select(tab, cx);
        }
    }
}

impl Shell {
    /// The tabs whose pages show in the card, in their order, with where the core puts each.
    pub fn shown_pages(&self) -> Vec<(u64, Bounds<Pixels>)> {
        self.shown_pages_at(self.viewport)
    }

    /// As `shown_pages`, in a window whose content is `viewport`: a resize lays the pages out at
    /// the window's new size before the shell draws there (`window_page_frame`).
    pub fn shown_pages_at(&self, viewport: Size<Pixels>) -> Vec<(u64, Bounds<Pixels>)> {
        if self.selected == 0 {
            return Vec::new();
        }
        let panes = self.panes(self.selected);
        let (frame, _) = self.page_frame(viewport);
        let frames = pane_frames(
            frame,
            panes.len(),
            self.stacked(self.selected),
            &self.shares(self.selected),
        );
        panes.into_iter().zip(frames).collect()
    }

    /// A swipe across the window at `at`, as three fingers or a mouse's swipe button send one
    /// where System Settings has them swipe between pages, goes back, or forward, in the history
    /// of the page under it, as the toolbar's buttons do (Chrome). Taken only over a page.
    pub fn swiped(&mut self, at: Point<Pixels>, back: bool, cx: &mut Context<Self>) -> bool {
        let Some(tab) = self
            .shown_pages()
            .into_iter()
            .find(|(_, frame)| frame.contains(&at))
        else {
            return false;
        };
        // check.sh reads this line.
        log!(
            "browser: swiped {} {}",
            if back { "back" } else { "forward" },
            tab.0
        );
        match back {
            true => self.browser.go_back(tab.0, cx),
            false => self.browser.go_forward(tab.0, cx),
        }
        true
    }
}

/// Where the page goes in a window whose content is `viewport`: in the card, whose left edge is at
/// `card_left`, or filling the window for a full-screen page, at 0. The Library pushes the card
/// `push` further right at the same width, past the window's edge.
pub fn page_frame(viewport: Size<Pixels>, card_left: f32, push: f32) -> Bounds<Pixels> {
    if card_left == 0. {
        return Bounds::new(point(px(0.), px(0.)), viewport);
    }
    Bounds::new(
        point(px(card_left + push), px(CARD_INSET)),
        size(
            viewport.width - px(card_left + CARD_INSET),
            viewport.height - px(2. * CARD_INSET),
        ),
    )
}

/// The pages of a split view of `count` in `frame`, in their order, where the core puts them:
/// SPLIT_GAP apart, side by side or stacked, as `pane_edges` has them.
pub(super) fn pane_frames(
    frame: Bounds<Pixels>,
    count: usize,
    stacked: bool,
    shares: &[f64],
) -> Vec<Bounds<Pixels>> {
    let edges = pane_edges(frame, count, stacked, shares);
    (0..count)
        .map(|index| {
            let (from, to) = (px(edges[index]), px(edges[index + 1] - SPLIT_GAP));
            match stacked {
                true => Bounds::from_corners(point(frame.left(), from), point(frame.right(), to)),
                false => Bounds::from_corners(point(from, frame.top()), point(to, frame.bottom())),
            }
        })
        .collect()
}

/// The shares of `count` pages across a card `length` long, or tall stacked, with the `focused`
/// one taking all but what the others keep: EXPANDED_OTHER each, no more than an even share, so
/// the focused page is never the smaller one on a card too short for both. The core works out the
/// same at its layout (`ExpandedShares` in core/window.cc), which keeps the others at 220 pt as
/// the window resizes.
pub(super) fn expanded_shares(count: usize, focused: usize, length: f32) -> Vec<f64> {
    let span = f64::from(length + SPLIT_GAP);
    let other = f64::from(EXPANDED_OTHER + SPLIT_GAP).min(span / count as f64) / span;
    let mut shares = vec![other; count];
    shares[focused] = 1. - other * (count - 1) as f64;
    shares
}

/// The shares of a split view whose pages start at `edges`, with where one after them would last,
/// as `pane_edges` takes them.
pub(super) fn edge_shares(edges: &[f32]) -> Vec<f64> {
    let span = f64::from(edges[edges.len() - 1] - edges[0]);
    edges
        .windows(2)
        .map(|pair| f64::from(pair[1] - pair[0]) / span)
        .collect()
}

/// Where each of a split view's `count` pages starts across `frame`, or down it stacked, then where
/// one after them would, as the core has it: each page takes its share of the frame with the gap
/// after it, rounded to the point, else is as wide or as tall as the others to the point.
pub(super) fn pane_edges(
    frame: Bounds<Pixels>,
    count: usize,
    stacked: bool,
    shares: &[f64],
) -> Vec<f32> {
    let (start, length) = match stacked {
        true => (frame.origin.y, frame.size.height),
        false => (frame.origin.x, frame.size.width),
    };
    let span = f32::from(length) + SPLIT_GAP;
    let even = shares.len() != count;
    let mut before = 0.;
    (0..=count)
        .map(|index| {
            let edge = match even {
                true => (span * index as f32 / count as f32).floor(),
                false => (f64::from(span) * before).round() as f32,
            };
            before += shares.get(index).copied().unwrap_or(0.);
            f32::from(start) + edge
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::{edge_shares, expanded_shares, pane_edges, pane_frames};
    use crate::shell::{Command, Folders, Page, Shell, Split, recorder::Recorder};
    use gpui::{
        AppContext, Bounds, Entity, Modifiers, MouseButton, MouseDownEvent, TestAppContext, point,
        px, size,
    };

    /// A dragged gap's shares put the pages back where it was dragged to, as the core lays them
    /// out with the same rounding, so the cards behind the pages line up with them.
    #[test]
    fn shares_from_a_dragged_gap_lay_the_pages_out_there() {
        for length in [700., 911., 1174., 1503.] {
            let frame = Bounds::new(point(px(238.), px(10.)), size(px(length), px(length)));
            for count in [2, 3] {
                for moved in [-157., -1., 1., 16., 233.] {
                    for stacked in [false, true] {
                        let mut edges = pane_edges(frame, count, stacked, &[]);
                        edges[1] += moved;
                        let shares = edge_shares(&edges);
                        assert_eq!(pane_edges(frame, count, stacked, &shares), edges);
                    }
                }
            }
        }
    }

    /// A Space whose Today holds `tabs`, all in one split view, with `expanded` the tab that took
    /// the card, as the core reports it, on a host that records what the shell asks. `selected`
    /// is focused, in a window `width` wide.
    fn split_of(
        tabs: &[u64],
        selected: u64,
        expanded: u64,
        width: f32,
        cx: &mut TestAppContext,
    ) -> (Entity<Shell>, Rc<Recorder>) {
        let host = Rc::new(Recorder::default());
        let shell = cx.new(|cx| Shell::new(host.clone(), cx));
        shell.update(cx, |shell, cx| {
            shell.set_spaces(
                vec![(
                    1,
                    "Space 1".into(),
                    None,
                    0,
                    "".into(),
                    "".into(),
                    "".into(),
                )],
                cx,
            );
            shell.set_space(1, cx);
            shell.set_selected(selected, cx);
            shell.viewport = size(px(width), px(800.));
        });
        reported(&shell, tabs, &[], expanded, cx);
        (shell, host)
    }

    /// A full split view takes no more pages: ⌃+, an ⌥-click on another tab's row and New Note
    /// Split each show Arc's toast and change nothing.
    #[gpui::test]
    fn a_full_split_says_it_takes_no_more_pages(cx: &mut TestAppContext) {
        let (shell, host) = split_of(&[10, 11, 12], 11, 0, 1200., cx);
        shell.update(cx, |shell, cx| {
            let split = Split {
                tabs: vec![10, 11, 12],
                ..Default::default()
            };
            let today = vec![10, 11, 12, 13];
            shell.set_tabs(1, Vec::new(), today, vec![split], Folders::default(), cx);
        });
        host.take_edits();
        shell.update(cx, |shell, cx| {
            shell.run(Command::AddSplit, "keys", cx);
            assert_eq!(shell.adding_pane, None);
            assert!(
                shell.alt_press_row(13, cx),
                "the ⌥-click is taken, not a plain click"
            );
            shell.run(Command::NewNoteSplit, "keys", cx);
        });
        assert_eq!(host.toasts.take(), ["Cannot Add New Pane"; 3]);
        assert!(host.take_edits().is_empty());
        assert!(host.notes.borrow().is_empty());
    }

    /// Arc's ⌃⇧1–⌃⇧4 focus the split view's page at that place from the left, either way GPUI names
    /// the key; one past its pages, or on a tab in no split view, does nothing.
    #[gpui::test]
    fn the_pane_keys_focus_the_page_at_their_place(cx: &mut TestAppContext) {
        cx.update(crate::shell::commands::bind_keys);
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_space(1, cx);
            let split = Split {
                tabs: vec![10, 11, 12],
                ..Default::default()
            };
            let today = vec![10, 11, 12, 13];
            shell.set_tabs(1, Vec::new(), today, vec![split], Folders::default(), cx);
        });
        cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));
        let press = |cx: &mut gpui::VisualTestContext, shown: u64, key: &str| {
            shell.update(cx, |shell, cx| shell.set_selected(shown, cx));
            cx.run_until_parked();
            host.take();
            cx.simulate_keystrokes(key);
            cx.run_until_parked();
            host.take()
                .into_iter()
                .filter(|call| call.starts_with("select "))
                .collect::<Vec<_>>()
        };
        for (key, tab) in [
            ("ctrl-shift-1", 10),
            ("ctrl-shift-!", 10),
            ("ctrl-shift-3", 12),
            ("ctrl-shift-#", 12),
        ] {
            assert_eq!(press(cx, 11, key), [format!("select {tab}")], "{key}");
        }
        assert!(press(cx, 11, "ctrl-shift-4").is_empty(), "a fourth page");
        assert!(press(cx, 11, "ctrl-shift-2").is_empty(), "the page focused");
        assert!(
            press(cx, 13, "ctrl-shift-1").is_empty(),
            "a tab in no split"
        );
    }

    /// The core reports the split with these `shares` and the tab that took the card.
    fn reported(
        shell: &Entity<Shell>,
        tabs: &[u64],
        shares: &[f64],
        expanded: u64,
        cx: &mut TestAppContext,
    ) {
        shell.update(cx, |shell, cx| {
            let (tabs, shares) = (tabs.to_vec(), shares.to_vec());
            let split = Split {
                tabs: tabs.clone(),
                shares,
                expanded,
                ..Split::default()
            };
            shell.set_tabs(1, Vec::new(), tabs, vec![split], Folders::default(), cx);
        });
    }

    fn run(shell: &Entity<Shell>, cx: &mut TestAppContext) {
        shell.update(cx, |shell, cx| shell.run(Command::ExpandSplit, "ui", cx));
    }

    /// The widths of the pages shown, left to right.
    fn widths(shell: &Entity<Shell>, count: usize, cx: &mut TestAppContext) -> Vec<f32> {
        shell.read_with(cx, |shell, _| {
            let (frame, _) = shell.page_frame(shell.viewport);
            let shares = shell.shares(shell.selected);
            let frames = pane_frames(frame, count, false, &shares);
            frames
                .iter()
                .map(|frame| f32::from(frame.size.width))
                .collect()
        })
    }

    fn resize(shell: &Entity<Shell>, width: f32, cx: &mut TestAppContext) {
        shell.update(cx, |shell, _| shell.viewport = size(px(width), px(800.)));
    }

    #[gpui::test]
    fn expanding_asks_the_core_to_give_the_focused_page_the_card(cx: &mut TestAppContext) {
        for width in [900., 1200., 1800.] {
            let (shell, host) = split_of(&[10, 11], 11, 0, width, cx);
            host.take();
            run(&shell, cx);
            assert_eq!(host.take(), ["split expanded 11 true"]);
            reported(&shell, &[10, 11], &[], 11, cx);
            let widths = widths(&shell, 2, cx);
            assert!(
                (widths[0] - 220.).abs() < 1.,
                "at {width} the other is {}",
                widths[0]
            );
            let (card, _) = shell.read_with(cx, |shell, _| shell.page_frame(shell.viewport));
            assert!((widths[0] + 10. + widths[1] - f32::from(card.size.width)).abs() < 1.);
        }
    }

    #[gpui::test]
    fn the_page_focused_is_the_one_that_takes_the_card(cx: &mut TestAppContext) {
        let (shell, host) = split_of(&[10, 11, 12], 11, 0, 1400., cx);
        host.take();
        run(&shell, cx);
        assert_eq!(host.take(), ["split expanded 11 true"]);
        reported(&shell, &[10, 11, 12], &[], 11, cx);
        let widths = widths(&shell, 3, cx);
        assert!(
            (widths[0] - 220.).abs() < 1. && (widths[2] - 220.).abs() < 1.,
            "{widths:?}"
        );
        assert!(widths[1] > 600., "{widths:?}");
    }

    /// Done again it changes nothing, and so after a quit, when the core hands the flag back with
    /// the split. Run from the other pane it moves the card there.
    #[gpui::test]
    fn expanding_again_changes_nothing(cx: &mut TestAppContext) {
        let (shell, host) = split_of(&[10, 11], 10, 10, 1200., cx);
        host.take();
        run(&shell, cx);
        assert_eq!(host.take(), Vec::<String>::new());
        assert!((widths(&shell, 2, cx)[1] - 220.).abs() < 1.);
        shell.update(cx, |shell, cx| shell.set_selected(11, cx));
        host.take();
        run(&shell, cx);
        assert_eq!(host.take(), ["split expanded 11 true"]);
    }

    /// The other page stays 220 pt as the window gets wider and narrower, with nothing asked of
    /// the core.
    #[gpui::test]
    fn a_resize_keeps_the_other_page_at_220(cx: &mut TestAppContext) {
        let (shell, host) = split_of(&[10, 11], 11, 11, 1600., cx);
        host.take();
        for width in [1000., 2400., 1300.] {
            resize(&shell, width, cx);
            let widths = widths(&shell, 2, cx);
            assert!(
                (widths[0] - 220.).abs() < 1.,
                "at {width} the other is {}",
                widths[0]
            );
        }
        assert_eq!(host.take(), Vec::<String>::new());
    }

    /// The find bar sits at the focused page's top right corner, so its buttons' tooltips hang
    /// from it there: over the left page of a split when that's focused, and the right one's.
    #[gpui::test]
    fn the_find_bar_is_placed_over_the_focused_page(cx: &mut TestAppContext) {
        // Its place, as find_bar.rs gives it: twice the card's inset in, and 300 wide.
        let inset = crate::shell::CARD_INSET * 2.;
        let mut corners = Vec::new();
        for (focused, index) in [(10, 0), (11, 1)] {
            let (shell, _) = split_of(&[10, 11], focused, focused, 1400., cx);
            let (origin, window) =
                shell.read_with(cx, |shell, _| shell.at_page_corner(inset, inset, 300.));
            assert_eq!(window, size(px(1400.), px(800.)));
            let pane = shell.read_with(cx, |shell, _| {
                let (frame, _) = shell.page_frame(shell.viewport);
                pane_frames(frame, 2, false, &shell.shares(shell.selected))[index]
            });
            let (left, right) = (f32::from(origin.x), f32::from(origin.x) + 300.);
            assert!(
                f32::from(pane.left()) < left && right < f32::from(pane.right()),
                "{pane:?}"
            );
            corners.push((
                f32::from(pane.right()) - right,
                f32::from(origin.y - pane.top()),
            ));
        }
        assert_eq!(corners[0], corners[1]);
    }

    /// On a card too short for 220 and a larger page, the pages are even.
    #[gpui::test]
    fn a_card_too_short_for_it_leaves_the_pages_even(cx: &mut TestAppContext) {
        let (shell, _) = split_of(&[10, 11], 11, 11, 560., cx);
        let widths = widths(&shell, 2, cx);
        assert!((widths[0] - widths[1]).abs() <= 1., "{widths:?}");
        for (count, length) in [(2, 300.), (3, 500.), (3, 1400.)] {
            for focused in 0..count {
                let shares = expanded_shares(count, focused, length);
                assert!((shares.iter().sum::<f64>() - 1.).abs() < 1e-9);
                assert!(shares.iter().all(|share| *share <= shares[focused] + 1e-9));
            }
        }
    }

    /// Dragging the gap asks for the widths it was dragged to, which the core takes instead of the
    /// flag, and a double-click on it asks for even ones: the ways out.
    #[gpui::test]
    fn a_drag_or_a_double_click_on_the_gap_ends_it(cx: &mut TestAppContext) {
        let (shell, host) = split_of(&[10, 11], 11, 11, 1200., cx);
        host.take();
        shell.update(cx, |shell, cx| {
            let edge = shell.shown_pane_edges()[1];
            let press = |click_count| MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(edge - 5.), px(300.)),
                modifiers: Default::default(),
                click_count,
                first_mouse: false,
            };
            shell.press_gap(1, &press(1), cx);
            shell.drag_gap(edge + 95., cx);
            shell.press_gap(1, &press(2), cx);
        });
        let calls = host.take();
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert!(calls[0].starts_with("split shares 11 ") && !calls[0].ends_with("even"));
        assert_eq!(calls[1], "split shares 11 even");
        // The core, taking either, no longer has the card expanded.
        reported(&shell, &[10, 11], &[], 0, cx);
        let evened = widths(&shell, 2, cx);
        assert!((evened[0] - evened[1]).abs() <= 1., "{evened:?}");
        run(&shell, cx);
        assert_eq!(host.take(), ["split expanded 11 true"]);
    }

    /// A swipe goes back, or forward, in the page under the pointer, and only over a page.
    #[gpui::test]
    fn a_swipe_moves_the_history_of_the_page_under_it(cx: &mut TestAppContext) {
        let (shell, host) = split_of(&[10, 11], 10, 0, 1200., cx);
        host.take();
        let frames = shell.read_with(cx, |shell, _| {
            let (frame, _) = shell.page_frame(shell.viewport);
            pane_frames(frame, 2, false, &[])
        });
        let swipe = |at, back, cx: &mut TestAppContext| {
            shell.update(cx, |shell, cx| shell.swiped(at, back, cx))
        };
        assert!(swipe(frames[1].center(), true, cx));
        assert!(swipe(frames[0].center(), false, cx));
        assert_eq!(host.take(), ["back 11", "forward 10"]);
        let beside = point(frames[0].left() - px(20.), frames[0].center().y);
        assert!(!swipe(beside, true, cx), "the sidebar has no history");
        assert!(host.take().is_empty());
    }

    /// Each page of a split has its controls, whose menu changes the split's orientation, and
    /// separates or closes that page, focused or not, and a page full screen has none.
    #[gpui::test]
    fn each_pages_split_controls_act_on_that_page(cx: &mut TestAppContext) {
        let (shell, host) = split_of(&[10, 11, 12], 10, 0, 1200., cx);
        let handles =
            |cx: &mut TestAppContext| shell.update(cx, |shell, cx| shell.pane_handles(cx).len());
        assert_eq!(handles(cx), 3);
        host.take();
        for (title, call) in [
            ("Convert to Vertical Split View", "split stacked 11 true"),
            ("Separate Page from Split View", "separate 11"),
            ("Close this Split Pane", "close 11"),
        ] {
            shell.update(cx, |shell, cx| {
                shell.split_controls(11, point(px(600.), px(10.)), cx)
            });
            cx.update(|cx| host.choose(title, cx));
            let mut calls = host.take();
            calls.extend(host.take_edits());
            assert!(calls.iter().any(|each| each == call), "{title}: {calls:?}");
        }
        shell.update(cx, |shell, _| shell.page_fullscreen = true);
        assert_eq!(handles(cx), 0, "a page full screen has the whole window");
        let (alone, _) = split_of(&[10], 10, 0, 1200., cx);
        assert_eq!(
            alone.update(cx, |shell, cx| shell.pane_handles(cx).len()),
            0
        );
    }

    /// The command bar and the split controls name the orientation's turn as Arc does: Convert to
    /// Vertical Split View for a split side by side and Horizontal for a stacked one, which
    /// Dia's Change Orientation still finds, as Separate All Pages finds Separate All Tabs.
    #[gpui::test]
    fn the_orientations_turn_has_arcs_name(cx: &mut TestAppContext) {
        use crate::shell::commands::{ShellHandle, bar_commands};
        let (shell, host) = split_of(&[10, 11], 10, 0, 1200., cx);
        cx.update(|cx| cx.set_global(ShellHandle(shell.downgrade())));
        let names = |typed: &str, cx: &mut TestAppContext| {
            let rows = cx.update(|cx| bar_commands(typed, cx));
            rows.into_iter()
                .map(|row| row.name.to_string())
                .collect::<Vec<_>>()
        };
        let controls = |cx: &mut TestAppContext| {
            shell.update(cx, |shell, cx| {
                shell.split_controls(11, point(px(600.), px(10.)), cx)
            });
            let titles = host.menus.borrow().last().cloned().unwrap_or_default();
            titles.first().map(|(title, _)| title.to_string())
        };
        assert_eq!(
            names("change orientation", cx),
            ["Convert to Vertical Split View"]
        );
        assert_eq!(
            controls(cx).as_deref(),
            Some("Convert to Vertical Split View")
        );
        shell.update(cx, |shell, cx| {
            let split = Split {
                tabs: vec![10, 11],
                stacked: true,
                ..Default::default()
            };
            shell.set_tabs(
                1,
                Vec::new(),
                vec![10, 11],
                vec![split],
                Folders::default(),
                cx,
            );
        });
        assert_eq!(names("convert", cx), ["Convert to Horizontal Split View"]);
        assert_eq!(
            controls(cx).as_deref(),
            Some("Convert to Horizontal Split View")
        );
        assert_eq!(names("separate all pages", cx), ["Separate All Tabs"]);
    }

    /// A click on a page's split controls opens their menu where it pressed, which the menu takes
    /// for the click that opened it, so the click's release, anywhere on the controls, leaves the
    /// menu open: from the pane's top edge, it would close as a drag let go outside.
    #[gpui::test]
    fn a_click_on_split_controls_opens_their_menu_at_the_press(cx: &mut TestAppContext) {
        let host = Rc::new(Recorder::default());
        let browser = host.clone();
        let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
        shell.update(cx, |shell, cx| {
            shell.set_spaces(
                vec![(
                    1,
                    "Space 1".into(),
                    None,
                    0,
                    "".into(),
                    "".into(),
                    "".into(),
                )],
                cx,
            );
            shell.set_space(1, cx);
            shell.set_selected(10, cx);
        });
        reported(&shell, &[10, 11], &[], 0, cx);
        cx.run_until_parked();
        let left = shell.read_with(cx, |shell, _| {
            let (frame, _) = shell.page_frame(shell.viewport);
            pane_frames(frame, 2, false, &shell.shares(shell.selected))[0]
        });
        // Off the handle's middle, and above the pane's top, as a click on it lands.
        let at = point(left.center().x + px(10.), left.top() - px(5.));
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let titles = host.menus.borrow().last().cloned().unwrap_or_default();
        let first = titles.first().map(|(title, _)| title.as_ref());
        assert_eq!(first, Some("Convert to Vertical Split View"));
        assert_eq!(host.menu_at.get(), Some(at));
    }

    /// In a split, the URL field is the focused page's, as Arc's address bar is the panel's: ⌘L,
    /// a click on the field and Copy URL act on that page, and on the other once a click in it
    /// focuses it, as the core reports.
    #[gpui::test]
    fn the_url_field_is_the_focused_pages(cx: &mut TestAppContext) {
        let (left, right) = ("https://example.org/left", "https://example.com/right");
        let (shell, host) = split_of(&[10, 11], 11, 0, 1200., cx);
        shell.update(cx, |shell, cx| {
            for (tab, url) in [(10, left), (11, right)] {
                shell.set_page(
                    tab,
                    Page {
                        url: url.into(),
                        ..Page::default()
                    },
                    cx,
                );
            }
        });
        for (focused, url) in [(11, right), (10, left)] {
            shell.update(cx, |shell, cx| {
                shell.set_selected(focused, cx);
                shell.run(Command::EditUrl, "page", cx);
                shell.run(Command::EditUrl, "sidebar", cx);
                shell.run(Command::CopyUrl, "ui", cx);
            });
            assert_eq!(host.bar_edits.take(), [Some(focused), Some(focused)]);
            assert_eq!(host.copied.take(), [url]);
        }
    }

    #[gpui::test]
    fn a_page_alone_has_nothing_to_expand(cx: &mut TestAppContext) {
        let (shell, host) = split_of(&[10], 10, 0, 1200., cx);
        host.take();
        run(&shell, cx);
        assert_eq!(host.take(), Vec::<String>::new());
        assert!(!shell.read_with(cx, |shell, _| shell.offers(Command::ExpandSplit)));
    }
}
