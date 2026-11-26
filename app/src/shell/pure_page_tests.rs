//! With the sidebar hidden the page is the whole window: no rim, no rounded corners, and the strip
//! that brings the sidebar back is the window's resize edge, which the core leaves to the UI over
//! the page.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AppContext, Bounds, Entity, Pixels, Size, TestAppContext, VisualTestContext, point, px, size,
};

use super::drag::detents::shell_with;
use super::panes::pane_frames;
use super::task_cards::tests::card;
use super::{
    CARD_INSET, CARD_RADIUS, Folders, Page, REVEAL_WAIT, SIDEBAR_HIDE, SPLIT_GAP, Shell, SidePanel,
    Split, recorder::Recorder,
};
use crate::agents::task::Dot;

const VIEWPORT: Size<Pixels> = size(px(1000.), px(700.));

/// A window's shell, not drawn, showing tab 10 in Space 1, the window `VIEWPORT`.
fn shell(cx: &mut TestAppContext) -> Entity<Shell> {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        shell.viewport = VIEWPORT;
    });
    shell
}

/// ⌘S, the sidebar's motion done.
fn toggle(shell: &Entity<Shell>, cx: &mut TestAppContext) {
    shell.update(cx, |shell, cx| {
        shell.toggle_sidebar(cx);
        shell.sidebar_motion = None;
    });
}

fn frame(shell: &Entity<Shell>, cx: &mut TestAppContext) -> (Bounds<Pixels>, f32) {
    shell.read_with(cx, |shell, _| shell.page_frame(VIEWPORT))
}

fn whole_window() -> Bounds<Pixels> {
    Bounds::new(point(px(0.), px(0.)), VIEWPORT)
}

#[gpui::test]
fn hidden_the_page_is_the_whole_window_and_shown_it_has_its_rim_back(cx: &mut TestAppContext) {
    let shell = shell(cx);
    let docked = frame(&shell, cx);
    let width = shell.read_with(cx, |shell, _| shell.width());
    assert_eq!(
        docked,
        (
            Bounds::new(
                point(px(width), px(CARD_INSET)),
                size(
                    VIEWPORT.width - px(width + CARD_INSET),
                    VIEWPORT.height - px(2. * CARD_INSET)
                )
            ),
            CARD_RADIUS
        ),
        "beside the docked sidebar the card has its rim and its corners"
    );
    toggle(&shell, cx);
    assert_eq!(
        frame(&shell, cx),
        (whole_window(), 0.),
        "with the sidebar hidden the page isn't the whole window, square"
    );
    toggle(&shell, cx);
    assert_eq!(
        frame(&shell, cx),
        docked,
        "shown again, the card isn't where it was"
    );
}

/// The rim and the corners go with the sidebar's motion, not on a clock of their own.
#[gpui::test]
fn the_rim_and_the_corners_go_with_the_sidebar(cx: &mut TestAppContext) {
    let shell = shell(cx);
    shell.update(cx, |shell, cx| {
        shell.toggle_sidebar(cx);
        let (from, _) = shell.sidebar_motion.expect("⌘S started no motion");
        let halfway = Instant::now() - Duration::from_secs_f32(SIDEBAR_HIDE / 2.);
        shell.sidebar_motion = Some((from, halfway));
    });
    shell.read_with(cx, |shell, _| {
        let (frame, radius) = shell.page_frame(VIEWPORT);
        let rim = shell.card_rim();
        assert!(
            0. < rim && rim < CARD_INSET,
            "halfway, the rim is {rim}, not between none and {CARD_INSET}"
        );
        // Each read is on the real clock the motion runs on, a little later than the last.
        let near = |a: f32, b: f32| (a - b).abs() < 0.05;
        let top = f32::from(frame.top());
        assert!(near(top, rim), "the card's top {top} isn't its rim {rim}");
        let left = f32::from(frame.left());
        // The edge crosses the window in a tenth of a second, so it gets a point.
        let edge = shell.card_left();
        assert!(
            (left - edge).abs() < 1.,
            "the card's left {left} isn't its edge {edge}"
        );
        assert!(
            near(radius / CARD_RADIUS, rim / CARD_INSET),
            "the corners ({radius}) didn't go with the rim ({rim})"
        );
    });
}

/// A Little window has no sidebar to hide, and keeps its rim and corners.
#[gpui::test]
fn a_little_window_keeps_its_rim(cx: &mut TestAppContext) {
    let main = shell(cx);
    let host = Rc::new(Recorder::default());
    let little = cx.new(|cx| Shell::little_window(host, 9, 1, 1, Some(&main), cx));
    little.update(cx, |shell, cx| {
        shell.set_tabs(9, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        shell.viewport = VIEWPORT;
    });
    // Its top is under its bar.
    let (frame, radius) = frame(&little, cx);
    assert_eq!(radius, CARD_RADIUS);
    assert_eq!(f32::from(frame.left()), CARD_INSET);
    assert_eq!(frame.right(), VIEWPORT.width - px(CARD_INSET));
    assert_eq!(frame.bottom(), VIEWPORT.height - px(CARD_INSET));
}

fn panel() -> SidePanel {
    SidePanel {
        extension: "notes".into(),
        name: "Notes".into(),
        icon: None,
        title: "Notes".into(),
        width: 0.,
    }
}

#[gpui::test]
fn a_full_screen_page_is_the_window_without_its_panel(cx: &mut TestAppContext) {
    let shell = shell(cx);
    shell.update(cx, |shell, cx| {
        shell.set_side_panel(Some(panel()), cx);
        shell.page_fullscreen = true;
    });
    assert_eq!(frame(&shell, cx), (whole_window(), 0.));
    let panel = shell.read_with(cx, |shell, _| shell.card_frames(VIEWPORT).1);
    assert_eq!(panel, None, "a full-screen page left its side panel");
}

/// With the sidebar hidden a side panel still shows, at the window's right edge, the gap between
/// it and the page kept.
#[gpui::test]
fn a_side_panel_shows_at_the_window_s_edge_with_the_sidebar_hidden(cx: &mut TestAppContext) {
    let shell = shell(cx);
    shell.update(cx, |shell, cx| shell.set_side_panel(Some(panel()), cx));
    toggle(&shell, cx);
    let (page, panel) = shell.read_with(cx, |shell, _| shell.card_frames(VIEWPORT));
    let panel = panel.expect("the side panel went with the sidebar");
    assert_eq!(page.origin, point(px(0.), px(0.)));
    assert_eq!(panel.right(), VIEWPORT.width);
    assert_eq!((panel.top(), panel.bottom()), (px(0.), VIEWPORT.height));
    assert_eq!(panel.left() - page.right(), px(CARD_INSET));
}

/// A split's outer pages reach the window's edges, the gaps between them kept, and its handles,
/// which sit in the rim over the card, go as they do in full screen.
#[gpui::test]
fn a_split_reaches_the_window_s_edges_with_its_gaps(cx: &mut TestAppContext) {
    let shell = shell(cx);
    shell.update(cx, |shell, cx| {
        let split = Split {
            tabs: vec![10, 11],
            shares: vec![0.5, 0.5],
            expanded: 10,
            ..Split::default()
        };
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11],
            vec![split],
            Folders::default(),
            cx,
        );
        shell.set_selected(10, cx);
    });
    let handles = |shell: &Entity<Shell>, cx: &mut TestAppContext| {
        shell.update(cx, |shell, cx| shell.pane_handles(cx).len())
    };
    assert_eq!(handles(&shell, cx), 2, "the docked split has no handles");
    toggle(&shell, cx);
    let (frame, _) = frame(&shell, cx);
    let shares = shell.read_with(cx, |shell, _| shell.shares(10));
    let panes = pane_frames(frame, 2, false, &shares);
    assert_eq!(panes[0].left(), px(0.));
    assert_eq!(panes[1].right(), VIEWPORT.width);
    assert_eq!(panes[1].left() - panes[0].right(), px(SPLIT_GAP));
    for pane in &panes {
        assert_eq!((pane.top(), pane.bottom()), (px(0.), VIEWPORT.height));
    }
    assert_eq!(handles(&shell, cx), 0, "the hidden split kept its handles");
}

/// The Library opened over the hidden sidebar brings the rim back as the sidebar's would, and
/// closed it goes again.
#[gpui::test]
fn the_library_brings_the_rim_back_while_it_shows(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_reduce_motion(true));
    let shell = shell(cx);
    toggle(&shell, cx);
    shell.update(cx, |shell, cx| shell.set_library(true, cx));
    shell.read_with(cx, |shell, _| {
        assert_eq!(
            (shell.card_rim(), shell.card_radius()),
            (CARD_INSET, CARD_RADIUS),
            "the Library showed over a page with no rim"
        );
    });
    shell.update(cx, |shell, cx| shell.set_library(false, cx));
    assert_eq!(frame(&shell, cx), (whole_window(), 0.));
}

/// The strip that brings the hidden sidebar out is the window's resize edge, 3 points wide: what
/// the core leaves to the UI over a page reaching the window's side.
#[gpui::test]
fn the_reveal_strip_is_the_resize_edge(cx: &mut TestAppContext) {
    let (shell, _) = shell_with(cx, &[], &[1], Default::default());
    shell.update(cx, |shell, _| shell.sidebar_hidden = true);
    let to = |shell: &Entity<Shell>, x: f32, y: f32, cx: &mut TestAppContext| {
        shell.update(cx, |shell, cx| shell.pointer_over(point(px(x), px(y)), cx));
        cx.executor().advance_clock(REVEAL_WAIT * 2);
        shell.read_with(cx, |shell, _| shell.revealed)
    };
    // Out of the strip, then down it.
    assert!(!to(&shell, 400., 300., cx));
    assert!(!to(&shell, 4., 300., cx));
    assert!(!to(&shell, 4., 304., cx), "4 points in brought it out");
    assert!(!to(&shell, 400., 300., cx));
    assert!(!to(&shell, 2., 300., cx));
    assert!(to(&shell, 2., 304., cx), "2 points in didn't bring it out");
}

/// A task turning to wait on the user moves nothing on the page.
#[gpui::test]
fn a_waiting_task_leaves_the_page_where_it_is(cx: &mut TestAppContext) {
    let shell = shell(cx);
    toggle(&shell, cx);
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![card(1, Dot::Working)], cx)
    });
    let before = frame(&shell, cx);
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![card(1, Dot::Waiting)], cx)
    });
    assert_eq!(frame(&shell, cx), before);
    assert_eq!(before, (whole_window(), 0.));
}

/// A drawn window with Developer Mode on for its tab 1, the sidebar hidden or not.
fn developer_window(
    hidden: bool,
    cx: &mut TestAppContext,
) -> (Entity<Shell>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, Vec::new(), vec![1], Vec::new(), Folders::default(), cx);
        let page = Page {
            url: "http://localhost:3000/".into(),
            developer_mode: true,
            ..Page::default()
        };
        shell.set_page(1, page, cx);
        shell.set_selected(1, cx);
        shell.sidebar_hidden = hidden;
        cx.notify();
    });
    cx.run_until_parked();
    (shell, cx)
}

/// Developer Mode's outline is the shown tab's row's, in the sidebar: with it hidden, nothing on
/// the page has one.
#[gpui::test]
fn developer_mode_outlines_nothing_with_the_sidebar_hidden(cx: &mut TestAppContext) {
    let (docked, cx) = developer_window(false, cx);
    let drawn = docked.read_with(cx, |shell, _| shell.developer_outline_drawn.get());
    assert_eq!(
        drawn.map(|(tab, _)| tab),
        Some(1),
        "the docked row had none"
    );
    let (hidden, cx) = developer_window(true, cx);
    let drawn = hidden.read_with(cx, |shell, _| shell.developer_outline_drawn.get());
    assert_eq!(drawn, None, "an outline drew with the sidebar hidden");
}
