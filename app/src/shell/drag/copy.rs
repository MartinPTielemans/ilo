//! A row dragged with ⌥ held as it's let go leaves a copy of its tab there (#690), and an ⌥ press
//! let go where it was is still ⌥-click's split.

use gpui::TestAppContext;

use super::detents::{X, at, drag_to, row_y, shell_with};
use crate::shell::{Folders, ROW_PITCH, Shell};

/// Presses `tab`'s row with ⌥ held, and returns how far down it was.
fn alt_press(shell: &gpui::Entity<Shell>, cx: &mut TestAppContext, tab: u64) -> f32 {
    shell.update(cx, |shell, cx| {
        let y = row_y(shell, tab);
        shell.alt_press_list_row(tab, at(X, y), cx);
        y
    })
}

/// Dragged two rows down and let go with ⌥ held, a Today tab stays where it was, and a copy of it
/// goes where it was let go, counted with it; ⌘Z closes the copy. Without ⌥ it moves.
#[gpui::test]
fn a_row_let_go_with_alt_held_leaves_a_copy(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[], &[1, 2, 3, 4], Folders::default());
    let y = alt_press(&shell, cx, 1);
    assert!(host.take_edits().is_empty(), "the ⌥ press split at once");
    drag_to(&shell, cx, X, y + 2. * ROW_PITCH + 5.);
    host.take();
    shell.update(cx, |shell, cx| shell.drop_row_with(true, cx));
    let calls = host.take();
    assert_eq!(calls, ["duplicate 1"], "{calls:?}");
    // The recorder's first tab made.
    let copy = 1001;
    assert_eq!(host.take_edits(), [format!("move {copy} false 0 3")]);
    // ⌘Z closes the copy once the core has reported it.
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), vec![1, 2, 3, copy, 4], Vec::new(), Folders::default(), cx)
    });
    shell.update(cx, |shell, cx| shell.undo(cx));
    assert_eq!(host.take_edits(), [format!("close {copy}")]);

    let (shell, host) = shell_with(cx, &[], &[1, 2, 3, 4], Folders::default());
    let y = alt_press(&shell, cx, 1);
    drag_to(&shell, cx, X, y + 2. * ROW_PITCH + 5.);
    shell.update(cx, |shell, cx| shell.drop_row_with(false, cx));
    assert_eq!(host.take_edits(), ["move 1 false 0 2"], "⌥ let go before the drop still copied");
}

/// Into the pinned tabs, the copy's place counts the pinned tab it's a copy of, which stays.
#[gpui::test]
fn a_pinned_rows_copy_goes_under_it(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[1, 2, 3], &[4], Folders::default());
    let y = alt_press(&shell, cx, 1);
    drag_to(&shell, cx, X, y + ROW_PITCH + 5.);
    host.take_edits();
    shell.update(cx, |shell, cx| shell.drop_row_with(true, cx));
    // The recorder's first tab made.
    let copy = 1001;
    assert_eq!(host.take_edits(), [format!("move {copy} true 0 2")]);
}

/// An ⌥ press let go where it was splits the page shown with the tab, as ⌥-click did at the press.
#[gpui::test]
fn an_alt_press_let_go_in_place_splits(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[], &[1, 2], Folders::default());
    shell.update(cx, |shell, cx| shell.set_selected(1, cx));
    host.take_edits();
    alt_press(&shell, cx, 2);
    assert!(host.take_edits().is_empty(), "the ⌥ press split before it was let go");
    shell.update(cx, |shell, cx| shell.drop_row_with(false, cx));
    let edits = host.take_edits();
    assert!(edits.iter().any(|edit| edit.starts_with("split 2 1")), "{edits:?}");
}
