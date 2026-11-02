//! A tab dragged to the sidebar's edge, paging to the Space beside and dropped there (Arc).

use std::collections::HashMap;

use gpui::{AppContext, TestAppContext};

use super::{EDGE_WAIT, SPLIT_REST};
use super::detents::{X, drag_to, press, row_y};
use crate::shell::{Folders, Place, Shell, Slot, recorder::Recorder};

/// The Spaces One, Two and an agent's, One shown with Today 3 and 4, Two with 5 pinned and 6.
fn shell(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, std::rc::Rc<Recorder>) {
    let host = std::rc::Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        let space = |id, name: &str, agent: &str| {
            (id, name.into(), None, 0, "".into(), agent.into(), "".into())
        };
        let spaces = vec![space(1, "One", ""), space(2, "Two", ""), space(9, "Agent", "claude")];
        shell.set_spaces(spaces, cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, Vec::new(), vec![3, 4], Vec::new(), Folders::default(), cx);
        shell.set_tabs(2, vec![5], vec![6], Vec::new(), Folders::default(), cx);
    });
    (shell, host)
}

fn wait(cx: &mut TestAppContext) {
    cx.executor().advance_clock(EDGE_WAIT);
    cx.run_until_parked();
}

/// Held at the right edge, a row pages to the next Space once it has waited there, and goes on
/// with the drag; let go over the next Space's Today, it moves there as Move to moves it.
#[gpui::test]
fn a_row_held_at_the_edge_pages_to_the_next_space_and_drops_there(cx: &mut TestAppContext) {
    let (shell, host) = shell(cx);
    let y = press(&shell, cx, 4);
    let width = shell.read_with(cx, |shell, _| shell.width());
    drag_to(&shell, cx, X, y + 10.);
    drag_to(&shell, cx, width - 10., y + 10.);
    cx.run_until_parked();
    assert!(host.shown.take().is_empty(), "paged at once");
    wait(cx);
    assert_eq!(host.shown.take(), [2]);
    // The core shows it, and the drag goes on over it.
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    assert!(shell.read_with(cx, |shell, _| shell.dragging() == Some(Slot::Tab(4))));
    // Off the edge it pages no further, and over Today it would go at its top.
    let today = shell.read_with(cx, |shell, _| row_y(shell, 6));
    drag_to(&shell, cx, X, today);
    wait(cx);
    assert!(host.shown.take().is_empty());
    let to = shell.read_with(cx, |shell, _| shell.drag.as_ref().map(|drag| drag.to));
    assert!(to == Some(Place::List(false, 0)), "not at the top of Today");
    host.take_edits();
    shell.update(cx, |shell, cx| shell.drop_row(cx));
    assert_eq!(host.take_edits(), ["space 4 2"]);
    assert!(shell.read_with(cx, |shell, _| shell.drag.is_none()));
}

/// Held at the edge over another row's middle, it pages and never joins that row in a split, whose
/// wait is the shorter, even where it rested before it got there (#1089).
#[gpui::test]
fn a_row_held_at_the_edge_over_another_rests_on_no_split(cx: &mut TestAppContext) {
    let (shell, host) = shell(cx);
    let y = press(&shell, cx, 4);
    let width = shell.read_with(cx, |shell, _| shell.width());
    let three = shell.read_with(cx, |shell, _| row_y(shell, 3));
    let onto = |cx: &mut TestAppContext| {
        shell.read_with(cx, |shell, _| shell.drag.as_ref().and_then(|drag| drag.onto))
    };
    drag_to(&shell, cx, X, y + 10.);
    // Resting on 3's middle for part of the split's wait, then on to the edge, still on it.
    drag_to(&shell, cx, X, three);
    cx.executor().advance_clock(SPLIT_REST / 2);
    drag_to(&shell, cx, width - 10., three);
    cx.executor().advance_clock(SPLIT_REST);
    cx.run_until_parked();
    assert_eq!(onto(cx), None, "joined 3 at the edge");
    wait(cx);
    assert_eq!(host.shown.take(), [2]);
    assert_eq!(onto(cx), None, "joined 3 as it paged");
}

/// Over another Space's pinned tabs it pins the tab there, under them; at the left edge it pages
/// back, and an agent's Space past the last is never paged to.
#[gpui::test]
fn the_edge_pages_either_way_but_not_into_an_agents_space(cx: &mut TestAppContext) {
    let (shell, host) = shell(cx);
    let y = press(&shell, cx, 3);
    let width = shell.read_with(cx, |shell, _| shell.width());
    drag_to(&shell, cx, X, y + 10.);
    drag_to(&shell, cx, width - 10., y + 10.);
    wait(cx);
    assert_eq!(host.shown.take(), [2]);
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    // Held on at the edge: the agent's Space is next, so nothing.
    wait(cx);
    assert!(host.shown.take().is_empty());
    drag_to(&shell, cx, 10., y + 10.);
    wait(cx);
    assert_eq!(host.shown.take(), [1]);
    shell.update(cx, |shell, cx| shell.set_space(1, cx));
    // Back in its own Space it drops as any drag does, and paging on to Two again.
    drag_to(&shell, cx, X, y);
    drag_to(&shell, cx, width - 10., y);
    wait(cx);
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    let pinned = shell.read_with(cx, |shell, _| row_y(shell, 5));
    drag_to(&shell, cx, X, pinned);
    host.take_edits();
    shell.update(cx, |shell, cx| shell.drop_row(cx));
    let edits = host.take_edits();
    assert!(edits.contains(&"space 3 2".to_string()), "{edits:?}");
    assert!(edits.contains(&"pinned 3 true".to_string()), "{edits:?}");
}

/// A folder held at the edge pages too, and in the next Space it goes whole under the pinned tabs,
/// where Move to Space puts it, never into Today (#1353).
#[gpui::test]
fn a_folder_held_at_the_edge_goes_under_the_next_spaces_pinned_tabs(cx: &mut TestAppContext) {
    let (shell, host) = shell(cx);
    let folders = Folders {
        folders: HashMap::from([(7, ("Docs".into(), true))]),
        parents: HashMap::from([(8, 7)]),
        ..Default::default()
    };
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, vec![7, 8], vec![3, 4], Vec::new(), folders, cx)
    });
    let y = press(&shell, cx, 7);
    let width = shell.read_with(cx, |shell, _| shell.width());
    drag_to(&shell, cx, X, y + 10.);
    drag_to(&shell, cx, width - 10., y + 10.);
    wait(cx);
    assert_eq!(host.shown.take(), [2]);
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    assert!(shell.read_with(cx, |shell, _| shell.dragging() == Some(Slot::Tab(7))));
    // Over Two's Today it would still go under its pinned tab.
    let today = shell.read_with(cx, |shell, _| row_y(shell, 6));
    drag_to(&shell, cx, X, today);
    let to = shell.read_with(cx, |shell, _| shell.drag.as_ref().map(|drag| drag.to));
    assert!(to == Some(Place::List(true, 1)), "not under the pinned tabs");
    host.take_edits();
    shell.update(cx, |shell, cx| shell.drop_row(cx));
    assert_eq!(host.take_edits(), ["space folder 7 2"]);
}
