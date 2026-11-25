//! Today's folders in the list (#4252): a tab dragged into one and out, a folder over the divider
//! both ways, and the rows and menu they have.

use gpui::TestAppContext;

use super::detents::{X, drag_to, press, row_y, shell_with};
use crate::shell::{Folders, ROW_PITCH};
use crate::tokens::shell::FOLDER_INDENT;

/// Pinned 1, then Today's 4, the today's folder 20 holding 5 and 6, and 7.
fn today_folder(open: bool) -> Folders {
    Folders {
        folders: [(20, ("Trips".into(), open))].into(),
        parents: [(5, 20), (6, 20)].into(),
        ..Default::default()
    }
}

/// The browser's header has today's folders, so this build draws and moves them.
#[test]
fn this_header_makes_todays_folders() {
    const { assert!(cfg!(today_folders)) };
}

/// A today's folder's line is above its tabs, which a closed one hides.
#[gpui::test]
fn a_todays_folder_is_a_line_above_its_tabs(cx: &mut TestAppContext) {
    let (shell, _) = shell_with(cx, &[1], &[4, 5, 6, 7], today_folder(true));
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.rows().1),
        [4, 20, 5, 6, 7]
    );
    let numbered = shell.read_with(cx, |shell, _| shell.sidebar_rows());
    assert_eq!(numbered, [1, 4, 5, 6, 7]);
    let (shell, _) = shell_with(cx, &[1], &[4, 5, 6, 7], today_folder(false));
    assert_eq!(shell.read_with(cx, |shell, _| shell.rows().1), [4, 20, 7]);
    // Numbered without the folder, and without what it hides.
    let numbered = shell.read_with(cx, |shell, _| shell.sidebar_rows());
    assert_eq!(numbered, [1, 4, 7]);
}

/// A tab dragged between a today's folder's tabs goes into it, and one dragged out of it, and
/// left, leaves it; ⌘Z puts it back in its folder.
#[gpui::test]
fn a_tab_goes_into_a_todays_folder_and_out(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[1], &[4, 5, 6, 7], today_folder(true));
    let y = press(&shell, cx, 7);
    drag_to(&shell, cx, X, y - ROW_PITCH);
    shell.update(cx, |shell, cx| shell.drop_row(cx));
    assert_eq!(host.take_edits(), ["move 7 false 20 1"]);

    let (shell, host) = shell_with(cx, &[1], &[4, 5, 6, 7], today_folder(true));
    let y = press(&shell, cx, 5);
    drag_to(&shell, cx, X - FOLDER_INDENT, y + 2. * ROW_PITCH);
    shell.update(cx, |shell, cx| shell.drop_row(cx));
    assert_eq!(host.take_edits(), ["move 5 false 0 3"]);
    shell.update(cx, |shell, cx| assert!(shell.undo(cx)));
    assert_eq!(host.take_edits(), ["move 5 false 20 0"]);
}

/// A today's folder dragged over the divider goes into the pinned tabs whole, and a pinned one
/// dragged under it goes into Today whole; ⌘Z takes each back.
#[gpui::test]
fn a_folder_crosses_the_divider_both_ways(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[1], &[4, 5, 6, 7], today_folder(true));
    press(&shell, cx, 20);
    let y = shell.read_with(cx, |shell, _| row_y(shell, 1));
    drag_to(&shell, cx, X, y);
    shell.update(cx, |shell, cx| shell.drop_row(cx));
    assert_eq!(host.take_edits(), ["move folder 20 0 0"]);
    shell.update(cx, |shell, cx| assert!(shell.undo(cx)));
    assert_eq!(host.take_edits(), ["today folder 20 1"]);

    let pinned = Folders {
        folders: [(10, ("Work".into(), true))].into(),
        parents: [(2, 10)].into(),
        ..Default::default()
    };
    let (shell, host) = shell_with(cx, &[10, 2], &[4, 7], pinned);
    press(&shell, cx, 10);
    // Out of the pinned tabs, its row leaves them, and Today's first place is a row above 4's.
    let y = shell.read_with(cx, |shell, _| row_y(shell, 4));
    drag_to(&shell, cx, X, y - ROW_PITCH);
    shell.update(cx, |shell, cx| shell.drop_row(cx));
    assert_eq!(host.take_edits(), ["today folder 10 0"]);
    shell.update(cx, |shell, cx| assert!(shell.undo(cx)));
    assert_eq!(host.take_edits(), ["move folder 10 0 0"]);
}

/// A pinned folder holding no tab has no place under the divider (the lead's answer 3), nor does
/// a today's folder inside another.
#[gpui::test]
fn a_folder_goes_into_today_only_whole_and_only_with_tabs(cx: &mut TestAppContext) {
    let empty = Folders {
        folders: [(11, ("Empty".into(), true))].into(),
        ..Default::default()
    };
    let (shell, host) = shell_with(cx, &[1, 11], &[4, 7], empty);
    press(&shell, cx, 11);
    let y = shell.read_with(cx, |shell, _| row_y(shell, 7));
    drag_to(&shell, cx, X, y);
    shell.update(cx, |shell, cx| shell.drop_row(cx));
    let edits = host.take_edits();
    assert!(
        edits.iter().all(|edit| !edit.starts_with("today")),
        "{edits:?}"
    );

    let (shell, host) = shell_with(cx, &[1], &[4, 5, 6, 7], today_folder(true));
    press(&shell, cx, 20);
    let y = shell.read_with(cx, |shell, _| row_y(shell, 7));
    drag_to(&shell, cx, X, y + ROW_PITCH);
    shell.update(cx, |shell, cx| shell.drop_row(cx));
    // Past its own tabs and 7, it's Today's last row.
    assert_eq!(host.take_edits(), ["today folder 20 2"]);
}

/// A today's folder's menu names it or lets it go, nothing else.
#[gpui::test]
fn a_todays_folder_menu_renames_and_deletes(cx: &mut TestAppContext) {
    let (shell, _) = shell_with(cx, &[1], &[4, 5, 6, 7], today_folder(true));
    let items = shell.read_with(cx, |shell, _| {
        (shell.folder_menu(20).into_iter())
            .map(|(_, id, _)| id)
            .filter(|id| !id.is_empty())
            .collect::<Vec<_>>()
    });
    assert_eq!(items, ["rename-folder", "delete-folder"]);
}
