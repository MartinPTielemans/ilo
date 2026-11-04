//! A tab's row dragged out of its window: the system's drag of the tab, another window's list taking
//! it, or a window of its own where nothing does (docs/ux-spec.md › Dragging out of the list).

use std::rc::Rc;

use gpui::{TestAppContext, point, px};

use super::detents::{X, at, drag_to, press, row_y, shell_with};
use crate::shell::{Folders, ROW_HEIGHT, Shell, WindowDragEnd, recorder::Recorder};

/// A drawn window, so its list has the bounds a drag in is taken over, showing Space 1, its Today
/// `today`, beside Space 2.
fn drawn(
    cx: &mut TestAppContext,
    today: Vec<u64>,
) -> (
    gpui::Entity<Shell>,
    &mut gpui::VisualTestContext,
    Rc<Recorder>,
) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = |id, name: &str| (id, name.into(), None, 0, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space(1, "One"), space(2, "Two")], cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, Vec::new(), today, Vec::new(), Folders::default(), cx);
    });
    cx.run_until_parked();
    (shell, cx, host)
}

/// The host's calls that start with `prefix`.
fn calls(host: &Recorder, prefix: &str) -> Vec<String> {
    host.take()
        .into_iter()
        .filter(|call| call.starts_with(prefix))
        .collect()
}

/// Dragged past the window's edge, a tab's row asks for the system's drag of the tab, showing its
/// band of the sidebar where it was pressed, held where it was pressed, and goes back to its place.
/// A folder's row stays in the window, and so does every row where the system can't drag a tab.
#[gpui::test]
fn a_tabs_row_dragged_past_the_windows_edge_leaves_as_the_systems_drag(cx: &mut TestAppContext) {
    let folders = Folders {
        folders: [(7, ("Docs".into(), true))].into(),
        parents: [(8, 7)].into(),
        ..Default::default()
    };
    let (shell, host) = shell_with(cx, &[7, 8], &[3, 4], folders);
    shell.update(cx, |shell, _| {
        shell.viewport = gpui::size(px(1000.), px(700.))
    });
    // Where the system can't, the row stays in the window, as it always has.
    let y = press(&shell, cx, 4);
    drag_to(&shell, cx, X, y + 10.);
    drag_to(&shell, cx, -40., y + 10.);
    assert!(calls(&host, "drag tab").is_empty());
    assert!(shell.read_with(cx, |shell, _| shell.drag.as_ref().is_some_and(|d| d.moving)));
    shell.update(cx, |shell, cx| shell.cancel_drag(cx));
    cx.run_until_parked();

    host.drags_tabs.set(true);
    let y = press(&shell, cx, 4);
    drag_to(&shell, cx, X, y + 10.);
    assert!(
        calls(&host, "drag tab").is_empty(),
        "left inside the window"
    );
    drag_to(&shell, cx, -40., y + 10.);
    let top = y - ROW_HEIGHT / 2.;
    let width = shell.read_with(cx, |shell, _| shell.width());
    assert_eq!(
        calls(&host, "drag tab"),
        [format!(
            "drag tab 4 0 {top} {width}x{ROW_HEIGHT} at {X} {}",
            ROW_HEIGHT / 2.
        )]
    );
    let window_drag = shell.read_with(cx, |shell, _| shell.window_drag.as_ref().map(|d| d.tab));
    assert_eq!(window_drag, Some(4));
    shell.update(cx, |shell, cx| {
        shell.window_drag_ended(4, WindowDragEnd::Taken, cx)
    });
    cx.run_until_parked();

    // A folder's row doesn't leave.
    let y = press(&shell, cx, 7);
    drag_to(&shell, cx, X, y + 10.);
    drag_to(&shell, cx, X, -30.);
    assert!(calls(&host, "drag tab").is_empty(), "a folder left");
}

/// Let go where nothing takes it, the tab opens in a window of its own at the point less where it
/// was pressed, so its row comes in under the pointer, and the window it left shows the tab used
/// before it. Taken by another window's list, no window opens.
#[gpui::test]
fn let_go_where_nothing_takes_it_the_tab_opens_in_a_window_under_the_pointer(
    cx: &mut TestAppContext,
) {
    let (shell, host) = shell_with(cx, &[], &[3, 4], Folders::default());
    shell.update(cx, |shell, cx| {
        shell.viewport = gpui::size(px(1000.), px(700.));
        shell.set_selected(3, cx);
        shell.set_selected(4, cx);
    });
    host.drags_tabs.set(true);
    let y = press(&shell, cx, 4);
    drag_to(&shell, cx, X, y + 10.);
    drag_to(&shell, cx, 1100., y + 10.);
    host.take();
    shell.update(cx, |shell, cx| {
        shell.window_drag_ended(4, WindowDragEnd::LetGo(Some(point(1500., 400.))), cx)
    });
    assert_eq!(
        host.take()
            .into_iter()
            .filter(|call| call.starts_with("new window") || call.starts_with("select"))
            .collect::<Vec<_>>(),
        [
            format!("new window for 4 at 1500 400, held at {X} {y}"),
            "select 3".to_string(),
        ]
    );

    let y = press(&shell, cx, 4);
    drag_to(&shell, cx, X, y + 10.);
    drag_to(&shell, cx, 1100., y + 10.);
    host.take();
    shell.update(cx, |shell, cx| shell.set_selected(4, cx));
    shell.update(cx, |shell, cx| {
        shell.window_drag_ended(4, WindowDragEnd::Taken, cx)
    });
    let asked = host.take();
    assert!(
        !asked.iter().any(|call| call.starts_with("new window")),
        "{asked:?}"
    );
    assert!(asked.contains(&"select 3".to_string()), "{asked:?}");
}

/// Another window's list takes the tab where its line marks, among its rows without the tab's own,
/// shows it there, its page moving there live, and ⌘Z there puts it back where it was and shows it
/// in the window it came from.
#[gpui::test]
fn another_windows_list_takes_the_tab_at_its_gap_and_undo_puts_it_back(cx: &mut TestAppContext) {
    // On the same Space, listing 3, 4 and 5; 4 shows in window 1.
    let (shell, cx, host) = drawn(cx, vec![3, 4, 5]);
    host.tab_windows.borrow_mut().insert(4, 1);
    let above_three = shell.read_with(cx, |shell, _| row_y(shell, 3)) - ROW_HEIGHT / 2. + 2.;
    host.take();
    host.take_edits();
    assert!(shell.update(cx, |shell, cx| shell.drag_tab_in(at(X, above_three), 4, cx)));
    assert!(shell.update(cx, |shell, cx| shell.drop_tab_in(at(X, above_three), 4, cx)));
    assert_eq!(host.take_edits(), ["move 4 false 0 0"]);
    assert!(calls(&host, "select").contains(&"select 4".to_string()));
    // Below 5, past its own row, it's the last place among the others.
    shell.update(cx, |shell, _| shell.drag = None);
    let below_five = shell.read_with(cx, |shell, _| row_y(shell, 5)) + ROW_HEIGHT / 2. - 2.;
    assert!(shell.update(cx, |shell, cx| shell.drop_tab_in(at(X, below_five), 4, cx)));
    assert_eq!(host.take_edits(), ["move 4 false 0 2"]);
    shell.update(cx, |shell, cx| shell.undo(cx));
    host.take_edits();
    host.take();

    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![4, 3, 5],
            Vec::new(),
            Folders::default(),
            cx,
        );
    });
    cx.run_until_parked();
    assert!(shell.update(cx, |shell, cx| shell.undo(cx)));
    assert_eq!(host.take_edits(), ["move 4 false 0 1"]);
    assert_eq!(calls(&host, "show"), ["show 4 in window 1"]);
}

/// From another Space, the tab moves into the one this window shows, then to its gap.
#[gpui::test]
fn a_tab_from_another_space_moves_into_the_one_shown(cx: &mut TestAppContext) {
    let (shell, cx, host) = drawn(cx, vec![3]);
    host.tab_windows.borrow_mut().insert(9, 2);
    let below = shell.read_with(cx, |shell, _| row_y(shell, 3)) + ROW_HEIGHT / 2. - 2.;
    host.take_edits();
    assert!(shell.update(cx, |shell, cx| shell.drop_tab_in(at(X, below), 9, cx)));
    assert_eq!(host.take_edits(), ["space 9 1", "move 9 false 0 1"]);
}

/// A private window's list takes no tab from another window, whose Space is the user's.
#[gpui::test]
fn a_private_windows_list_takes_no_tab(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[], &[3], Folders::default());
    shell.update(cx, |shell, _| {
        shell.viewport = gpui::size(px(1000.), px(700.));
        shell.private = true;
    });
    let y = shell.read_with(cx, |shell, _| row_y(shell, 3));
    host.take_edits();
    assert!(!shell.update(cx, |shell, cx| shell.drop_tab_in(at(X, y), 9, cx)));
    assert!(host.take_edits().is_empty());
}

/// The window a drag opened for a tab: ⌘Z there shows the tab in the window it came from again and
/// closes this one, which holds nothing else of the user's.
#[gpui::test]
fn undo_in_the_window_a_drag_opened_sends_the_tab_back_and_closes_it(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[], &[3, 4], Folders::default());
    shell.update(cx, |shell, cx| shell.came_by_window_drag(4, 1, cx));
    host.take();
    assert!(shell.update(cx, |shell, cx| shell.undo(cx)));
    let asked = host.take();
    assert!(
        asked.contains(&"show 4 in window 1".to_string()),
        "{asked:?}"
    );
    assert!(asked.contains(&"close window".to_string()), "{asked:?}");
}

/// Cancelled, as escape cancels it, the drag changes nothing: no window opens and the window it
/// left still shows the tab.
#[gpui::test]
fn a_cancelled_drag_changes_nothing(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[], &[3, 4], Folders::default());
    shell.update(cx, |shell, cx| {
        shell.viewport = gpui::size(px(1000.), px(700.));
        shell.set_selected(3, cx);
        shell.set_selected(4, cx);
    });
    host.drags_tabs.set(true);
    let y = press(&shell, cx, 4);
    drag_to(&shell, cx, X, y + 10.);
    drag_to(&shell, cx, 1100., y + 10.);
    host.take();
    shell.update(cx, |shell, cx| {
        shell.window_drag_ended(4, WindowDragEnd::Cancelled, cx)
    });
    let asked = host.take();
    assert!(
        !asked
            .iter()
            .any(|call| call.starts_with("new window") || call.starts_with("select")),
        "{asked:?}"
    );
    assert!(shell.read_with(cx, |shell, _| shell.window_drag.is_none()));
}
