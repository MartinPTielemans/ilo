//! The edge knot (window-and-threads.md › Sidebar hidden): with the sidebar hidden, a tab at the
//! window's edge shows only while a thread of any Space needs the user, and a press on it brings
//! the sidebar out and does nothing more.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::edge_knot::PRESS_GUARD;
use super::task_cards::tests::card;
use super::{Folders, Shell, recorder::Recorder};
use crate::agents::task::Dot;
use crate::tokens::shell::edge_knot::{HEIGHT, TOP, WIDTH};

/// A drawn window showing tab 10 in Space 1, its sidebar hidden and still.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let (shell, cx) = cx.add_window_view({
        let host = host.clone();
        |_, cx| Shell::new(host, cx)
    });
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        shell.sidebar_hidden = true;
        cx.notify();
    });
    cx.run_until_parked();
    host.take();
    (shell, host, cx)
}

/// Thread 1 in state `dot`. One turning to wait brings the hidden sidebar out once (#1080), and
/// the pointer, not over it, sends it away again.
fn waiting(shell: &Entity<Shell>, dot: Dot, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, cx| shell.set_task_cards(vec![card(1, dot)], cx));
    cx.run_until_parked();
    away(shell, cx);
}

/// The sidebar brought out goes away, its motion done.
fn away(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, cx| {
        shell.set_revealed(false, cx);
        shell.reveal_motion = None;
        cx.notify();
    });
    cx.run_until_parked();
}

fn knot_calls(host: &Recorder) -> Vec<String> {
    (host.take().into_iter())
        .filter(|call| call.starts_with("edge knot"))
        .collect()
}

fn shown(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |shell, _| shell.edge_knot_shown())
}

/// Shown at the window's edge, at its fixed place, as a thread comes to need the user, and gone as
/// none does.
#[gpui::test]
fn it_shows_while_a_thread_needs_the_user(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    waiting(&shell, Dot::Working, cx);
    assert_eq!(
        knot_calls(&host),
        Vec::<String>::new(),
        "a working thread showed it"
    );
    shell.update(cx, |shell, cx| {
        shell.set_task_cards(vec![card(1, Dot::Waiting)], cx)
    });
    cx.run_until_parked();
    assert!(
        !shown(&shell, cx),
        "it showed with the sidebar brought out over the page"
    );
    away(&shell, cx);
    assert_eq!(
        knot_calls(&host),
        [format!("edge knot at {TOP} {WIDTH} {HEIGHT}")]
    );
    waiting(&shell, Dot::Done, cx);
    assert_eq!(knot_calls(&host), ["edge knot hidden"]);
    assert!(!shown(&shell, cx));
}

/// The thread needing the user can be another Space's: the card's Space isn't the window's.
#[gpui::test]
fn a_thread_of_another_space_shows_it(cx: &mut TestAppContext) {
    let (shell, _, cx) = window(cx);
    let space = shell.read_with(cx, |shell, _| shell.space);
    let card = card(1, Dot::Waiting);
    assert_ne!(card.space, space);
    shell.update(cx, |shell, cx| shell.set_task_cards(vec![card], cx));
    cx.run_until_parked();
    away(&shell, cx);
    assert!(shown(&shell, cx));
}

/// Not with the sidebar there, its panel out, the Library open, a page full screen or in a
/// private window.
#[gpui::test]
fn it_shows_only_with_the_sidebar_hidden_and_in(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    waiting(&shell, Dot::Waiting, cx);
    assert!(shown(&shell, cx));
    let gone_and_back = |change: &dyn Fn(&mut Shell, bool), cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            change(shell, true);
            cx.notify();
        });
        cx.run_until_parked();
        let hidden = shown(&shell, cx);
        shell.update(cx, |shell, cx| {
            change(shell, false);
            cx.notify();
        });
        cx.run_until_parked();
        (hidden, shown(&shell, cx))
    };
    assert_eq!(
        gone_and_back(&|shell, on| shell.sidebar_hidden = !on, cx),
        (false, true),
        "docked"
    );
    let reveal = |shell: &mut Shell, on: bool| (shell.revealed, shell.reveal_motion) = (on, None);
    assert_eq!(gone_and_back(&reveal, cx), (false, true), "revealed");
    assert_eq!(
        gone_and_back(&|shell, on| shell.page_fullscreen = on, cx),
        (false, true),
        "full screen"
    );
    assert_eq!(
        gone_and_back(&|shell, on| shell.private = on, cx),
        (false, true),
        "private"
    );
    shell.update(cx, |shell, cx| shell.set_library(true, cx));
    cx.run_until_parked();
    assert!(!shown(&shell, cx), "the Library open");
    host.take();
}

/// A Little window has no sidebar to bring out.
#[gpui::test]
fn a_little_window_has_none(cx: &mut TestAppContext) {
    let (main, _, cx) = window(cx);
    let host = Rc::new(Recorder::default());
    let (little, cx) =
        cx.add_window_view(|_, cx| Shell::little_window(host, 9, 1, 1, Some(&main), cx));
    little.update(cx, |shell, cx| {
        shell.sidebar_hidden = true;
        shell.set_task_cards(vec![card(1, Dot::Waiting)], cx);
    });
    cx.run_until_parked();
    assert!(!shown(&little, cx));
}

/// A press brings the sidebar out and does nothing more, but not in its first 500 ms after it
/// came, when a click meant for the page's edge could land on it.
#[gpui::test]
fn a_press_brings_the_sidebar_out_after_the_guard(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    waiting(&shell, Dot::Waiting, cx);
    host.take();
    let press = |cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.edge_knot_pressed(cx));
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| shell.revealed)
    };
    cx.executor()
        .advance_clock(PRESS_GUARD - Duration::from_millis(1));
    assert!(
        !press(cx),
        "a press right after it came brought the sidebar out"
    );
    let (tab, frame) = shell.read_with(cx, |shell, _| {
        (shell.selected, shell.page_frame(shell.viewport))
    });
    cx.executor().advance_clock(Duration::from_millis(1));
    assert!(
        press(cx),
        "a press after the guard didn't bring the sidebar out"
    );
    shell.read_with(cx, |shell, _| {
        assert_eq!(
            (shell.selected, shell.page_frame(shell.viewport)),
            (tab, frame),
            "it did more"
        );
    });
    assert_eq!(
        knot_calls(&host),
        ["edge knot hidden"],
        "the sidebar's panel came over it"
    );
}

/// Shown, it keeps to the window's edge: a resize, which on the right moves the edge, shows it
/// again for the host to put there.
#[gpui::test]
fn a_resize_shows_it_again(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    waiting(&shell, Dot::Waiting, cx);
    host.take();
    cx.simulate_resize(gpui::size(gpui::px(900.), gpui::px(500.)));
    cx.run_until_parked();
    assert_eq!(
        knot_calls(&host),
        [format!("edge knot at {TOP} {WIDTH} {HEIGHT}")]
    );
}

/// Its tooltip and description name the thread that came to need the user first, by the title its
/// line in the sidebar draws, whichever came after and whatever order the cards are in.
#[gpui::test]
fn it_names_the_thread_that_waited_first(cx: &mut TestAppContext) {
    let (shell, _, cx) = window(cx);
    let titled = |n: u64, dot: Dot, title: &str| crate::agents::task::Card {
        title: title.to_string().into(),
        ..card(n, dot)
    };
    let set = |cards, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.set_task_cards(cards, cx));
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| shell.edge_knot_said())
    };
    assert_eq!(set(vec![titled(1, Dot::Working, "Ferry")], cx), None);
    let ferry = titled(1, Dot::Waiting, "Book the 08:40 ferry");
    assert_eq!(
        set(vec![ferry.clone()], cx).as_deref(),
        Some("Book the 08:40 ferry")
    );
    let flight = titled(2, Dot::Waiting, "Find a flight");
    assert_eq!(
        set(vec![flight.clone(), ferry.clone()], cx).as_deref(),
        Some("Book the 08:40 ferry"),
        "the one that came after, newer and so first in the list, took its name"
    );
    let ferry_done = titled(1, Dot::Done, "Book the 08:40 ferry");
    assert_eq!(
        set(vec![flight, ferry_done], cx).as_deref(),
        Some("Find a flight")
    );
}

/// Idle floor: shown or hidden and still, nothing wakes for it. No timer, no frame, no host call.
#[gpui::test]
fn a_still_knot_wakes_nothing(cx: &mut TestAppContext) {
    let (shell, host, cx) = window(cx);
    let notified = Rc::new(Cell::new(0));
    let _watch = cx.update({
        let notified = notified.clone();
        |_, cx| cx.observe(&shell, move |_, _| notified.set(notified.get() + 1))
    });
    for dot in [Dot::Done, Dot::Waiting] {
        waiting(&shell, dot, cx);
        host.take();
        notified.set(0);
        cx.executor().advance_clock(Duration::from_secs(60));
        cx.run_until_parked();
        assert_eq!(
            (notified.get(), host.take()),
            (0, Vec::<String>::new()),
            "{dot:?}"
        );
    }
    assert!(shown(&shell, cx));
}
