//! The trackpad's clicks as a row is dragged, on a host that counts what it's asked.

use std::rc::Rc;

use gpui::{AppContext, Pixels, Point, TestAppContext, point, px};

use crate::shell::{Folders, ROW_HEIGHT, ROW_PITCH, Shell, Slot, TILE_HEIGHT, recorder::Recorder};

/// A shell showing one Space with `pinned` and `today` tabs and `folders`.
pub(in crate::shell) fn shell_with(
    cx: &mut TestAppContext,
    pinned: &[u64],
    today: &[u64],
    folders: Folders,
) -> (gpui::Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Work".into(), None, 0, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, pinned.to_vec(), today.to_vec(), Vec::new(), folders, cx);
    });
    (shell, host)
}

/// Where in the window `tab`'s row is, down its middle.
pub(super) fn row_y(shell: &Shell, tab: u64) -> f32 {
    shell.drawn_list_top() + shell.layout()[&Slot::Tab(tab)] + ROW_HEIGHT / 2.
}

pub(super) fn at(x: f32, y: f32) -> Point<Pixels> {
    point(px(x), px(y))
}

pub(in crate::shell) const X: f32 = 80.;

/// Presses `tab`'s row, in the middle.
pub(in crate::shell) fn press(
    shell: &gpui::Entity<Shell>,
    cx: &mut TestAppContext,
    tab: u64,
) -> f32 {
    shell.update(cx, |shell, _| {
        let y = row_y(shell, tab);
        shell.press_row(tab, at(X, y));
        y
    })
}

pub(in crate::shell) fn drag_to(
    shell: &gpui::Entity<Shell>,
    cx: &mut TestAppContext,
    x: f32,
    y: f32,
) {
    shell.update(cx, |shell, cx| shell.drag_to(at(x, y), cx));
}

/// One click for each row a dragged one passes, and none while it stays at a place, on the way
/// back up as on the way down, nor as it's let go.
#[gpui::test]
fn each_row_passed_gives_a_click(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[], &[1, 2, 3, 4, 5], Folders::default());
    let y = press(&shell, cx, 1);
    // Coming away from the row is no click, nor moving within its place.
    drag_to(&shell, cx, X, y + 6.);
    drag_to(&shell, cx, X, y + 9.);
    assert_eq!(host.detents.get(), 0);
    drag_to(&shell, cx, X, y + ROW_PITCH);
    assert_eq!(host.detents.get(), 1);
    // Holding still, and moving within the place, add none.
    drag_to(&shell, cx, X, y + ROW_PITCH);
    drag_to(&shell, cx, X, y + ROW_PITCH + 5.);
    drag_to(&shell, cx, X + 30., y + ROW_PITCH - 5.);
    assert_eq!(host.detents.get(), 1);
    drag_to(&shell, cx, X, y + 2. * ROW_PITCH);
    assert_eq!(host.detents.get(), 2);
    // Back up past one.
    drag_to(&shell, cx, X, y + ROW_PITCH);
    assert_eq!(host.detents.get(), 3);
    // Letting go moves the tab and clicks no more.
    shell.update(cx, |shell, cx| shell.drop_row(cx));
    assert_eq!(host.moves.get(), 1);
    assert_eq!(host.detents.get(), 3);
}

/// A drag that comes away and goes back where it began, or is cancelled, clicks only for the
/// rows it passed.
#[gpui::test]
fn cancelling_a_drag_gives_no_click(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[], &[1, 2, 3], Folders::default());
    let y = press(&shell, cx, 1);
    drag_to(&shell, cx, X, y + 8.);
    shell.update(cx, |shell, cx| assert!(shell.cancel_drag(cx)));
    assert_eq!(host.detents.get(), 0);
    let y = press(&shell, cx, 2);
    drag_to(&shell, cx, X, y + ROW_PITCH);
    assert_eq!(host.detents.get(), 1);
    shell.update(cx, |shell, cx| assert!(shell.cancel_drag(cx)));
    assert_eq!(host.detents.get(), 1);
    assert_eq!(host.moves.get(), 0);
}

/// Crossing the line between the pinned tabs and Today clicks, as passing a row does.
#[gpui::test]
fn crossing_the_pinned_line_gives_a_click(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[1], &[2, 3], Folders::default());
    let y = press(&shell, cx, 1);
    // Down into Today, past its first and second tab.
    drag_to(&shell, cx, X, y + 3. * ROW_PITCH);
    let down = host.detents.get();
    assert!(down >= 1, "no click crossing into Today");
    drag_to(&shell, cx, X, y + 3. * ROW_PITCH + 3.);
    assert_eq!(host.detents.get(), down);
    // And back up over the line.
    drag_to(&shell, cx, X, y);
    assert!(
        host.detents.get() > down,
        "no click crossing back to the pinned tabs"
    );
}

/// The pointer going right takes a row into the folder above, which is a new place for it, so a
/// click; going right again past the folder's depth changes nothing.
#[gpui::test]
fn going_into_a_folder_gives_a_click(cx: &mut TestAppContext) {
    let docs = [(20, ("Docs".into(), true))];
    let folders = Folders {
        folders: docs.into(),
        ..Default::default()
    };
    let (shell, host) = shell_with(cx, &[20, 21, 22], &[4], folders);
    let y = press(&shell, cx, 22);
    // Up past 21 to the gap under the folder.
    drag_to(&shell, cx, X, y - ROW_PITCH);
    assert_eq!(host.detents.get(), 1);
    // In, a level for each 16 pt.
    drag_to(&shell, cx, X + 20., y - ROW_PITCH);
    assert_eq!(host.detents.get(), 2);
    drag_to(&shell, cx, X + 40., y - ROW_PITCH);
    assert_eq!(host.detents.get(), 2);
    // And out again.
    drag_to(&shell, cx, X - 10., y - ROW_PITCH);
    assert_eq!(host.detents.get(), 3);
}

/// A tile dragged across the favorites clicks at each tile it passes, and again on leaving them
/// for the list.
#[gpui::test]
fn each_tile_passed_gives_a_click(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[], &[1, 2], Folders::default());
    shell.update(cx, |shell, cx| {
        shell.set_favorites(vec![(10, 0), (11, 0), (12, 0)], cx)
    });
    // Where the middle of the tile at `index` of the three is.
    let middle = |shell: &gpui::Entity<Shell>, cx: &mut TestAppContext, index: usize| {
        shell.update(cx, |shell, _| {
            let [left, top, width] = shell.tile_places(3)[index];
            (
                left + width / 2.,
                shell.tiles_top() + top + TILE_HEIGHT / 2.,
            )
        })
    };
    let (x, y) = middle(&shell, cx, 0);
    shell.update(cx, |shell, _| shell.press_tile(10, at(x, y)));
    let (x, y) = middle(&shell, cx, 1);
    drag_to(&shell, cx, x, y);
    assert_eq!(host.detents.get(), 1);
    drag_to(&shell, cx, x + 3., y + 2.);
    assert_eq!(host.detents.get(), 1);
    let (x, y) = middle(&shell, cx, 2);
    drag_to(&shell, cx, x, y);
    assert_eq!(host.detents.get(), 2);
    // Down out of the favorites, into the list.
    let y = shell.update(cx, |shell, _| row_y(shell, 2));
    drag_to(&shell, cx, X, y);
    assert_eq!(host.detents.get(), 3);
}
