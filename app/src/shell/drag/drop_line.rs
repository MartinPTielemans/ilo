//! A dragged row over the list: the rows stay put and a line marks where it would go.

use gpui::TestAppContext;

use super::detents::{X, at, drag_to, press, row_y, shell_with};
use crate::shell::{Folders, NAME_HEIGHT, Place, ROW_HEIGHT, ROW_PITCH, Shell, Slot};

/// Where the drop line is, down the list, or None.
fn line(shell: &gpui::Entity<Shell>, cx: &mut TestAppContext) -> Option<f32> {
    shell.read_with(cx, |shell, _| shell.drag.as_ref().and_then(|drag| drag.line))
}

/// Halfway between the plates of the row at `top` and the one after it.
fn under(top: f32) -> f32 {
    top + ROW_PITCH - (ROW_PITCH - ROW_HEIGHT) / 2.
}

/// Dragged down past two rows and back up, no other row moves from its place, and the line is in
/// the gap the row would drop into.
#[gpui::test]
fn the_rows_stay_put_and_the_line_marks_the_gap(cx: &mut TestAppContext) {
    let (shell, _) = shell_with(cx, &[], &[1, 2, 3, 4], Folders::default());
    let layout = shell.read_with(cx, |shell, _| shell.layout());
    let y = press(&shell, cx, 1);
    let still = |shell: &gpui::Entity<Shell>, cx: &mut TestAppContext| {
        shell.read_with(cx, |shell, _| {
            let others = [Slot::Tab(2), Slot::Tab(3), Slot::Tab(4), Slot::NewTab];
            shell.slides.is_empty() && others.iter().all(|&slot| shell.slide(slot) == 0.)
        })
    };
    // Where it was, there's no line, since it would drop where it is.
    drag_to(&shell, cx, X, y + 8.);
    assert_eq!(line(&shell, cx), None);
    // Past 2 and 3, it would go under 3.
    drag_to(&shell, cx, X, y + 2. * ROW_PITCH);
    assert!(still(&shell, cx), "a row moved during the drag");
    assert_eq!(line(&shell, cx), Some(under(layout[&Slot::Tab(3)])));
    // Back up past 3, under 2.
    drag_to(&shell, cx, X, y + ROW_PITCH);
    assert!(still(&shell, cx), "a row moved during the drag");
    assert_eq!(line(&shell, cx), Some(under(layout[&Slot::Tab(2)])));
}

/// Into the pinned tabs, the line goes over the first pinned row when it would be first, and under
/// New Tab when it would be Today's first.
#[gpui::test]
fn the_line_marks_the_ends_of_the_lists(cx: &mut TestAppContext) {
    let (shell, _) = shell_with(cx, &[1, 2], &[3, 4], Folders::default());
    let layout = shell.read_with(cx, |shell, _| shell.layout());
    press(&shell, cx, 4);
    let y = shell.read_with(cx, |shell, _| row_y(shell, 1));
    drag_to(&shell, cx, X, y);
    assert_eq!(line(&shell, cx), Some(NAME_HEIGHT - (ROW_PITCH - ROW_HEIGHT) / 2.));
    let y = shell.read_with(cx, |shell, _| row_y(shell, 3));
    drag_to(&shell, cx, X, y);
    assert_eq!(line(&shell, cx), Some(under(layout[&Slot::NewTab])));
}

/// A link or text dragged in from the page or another app goes to the gap nearest the pointer,
/// where its line shows and `drop_in` opens it: under a row once the pointer is past the row's
/// middle, as Arc's does.
#[gpui::test]
fn what_is_dragged_in_goes_to_the_gap_nearest_the_pointer(cx: &mut TestAppContext) {
    let browser = std::rc::Rc::new(crate::shell::recorder::Recorder::default());
    // Drawn, so the list has the bounds a drag in is taken over.
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Work".into(), None, 0, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, Vec::new(), vec![1, 2], Vec::new(), Folders::default(), cx);
    });
    cx.run_until_parked();
    let layout = shell.read_with(cx, |shell, _| shell.layout());
    let (one, two) = shell.read_with(cx, |shell, _| (row_y(shell, 1), row_y(shell, 2)));
    let mut place = |y: f32| {
        shell.update(cx, |shell, cx| assert!(shell.drag_in(at(X, y), cx), "not taken at {y}"));
        shell.read_with(cx, |shell, _| shell.drag.as_ref().map(|drag| (drag.to, drag.line)))
    };
    let today = |index, above: Slot| Some((Place::List(false, index), Some(under(layout[&above]))));
    assert_eq!(place(one - 4.), today(0, Slot::NewTab));
    assert_eq!(place(one + 4.), today(1, Slot::Tab(1)));
    // Past the last row's middle, by as much as pinned.sh lets go of its text, under it.
    assert_eq!(place(two + 12.), today(2, Slot::Tab(2)));
}

/// The press on a folder opens or closes it, and the browser's lists come back while its row is
/// held: the drag goes on over them and lands where it's let go, whether they come before the row
/// moves or as it does (#1118).
#[gpui::test]
fn a_folder_held_over_the_lists_its_press_changed_lands_where_let_go(cx: &mut TestAppContext) {
    let folders = |open| Folders { folders: [(10, ("Work".into(), open))].into(), ..Default::default() };
    for before_moving in [true, false] {
        let (shell, host) = shell_with(cx, &[10, 8], &[9], folders(true));
        let y = press(&shell, cx, 10);
        let report = |cx: &mut TestAppContext| {
            shell.update(cx, |shell, cx| {
                shell.set_tabs(1, vec![10, 8], vec![9], Vec::new(), folders(false), cx)
            })
        };
        if before_moving {
            report(cx);
        }
        drag_to(&shell, cx, X, y + ROW_PITCH + 5.);
        if !before_moving {
            report(cx);
        }
        shell.update(cx, |shell, cx| shell.drop_row(cx));
        assert_eq!(host.take_edits(), ["move folder 10 0 1"], "the lists before it moved: {before_moving}");
    }
}

/// A row held while its tab leaves the lists, closed some other way, goes back, and letting go
/// moves nothing.
#[gpui::test]
fn a_row_held_while_its_tab_goes_goes_back(cx: &mut TestAppContext) {
    let (shell, host) = shell_with(cx, &[], &[1, 2, 3], Folders::default());
    let y = press(&shell, cx, 1);
    drag_to(&shell, cx, X, y + ROW_PITCH + 5.);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), vec![2, 3], Vec::new(), Folders::default(), cx)
    });
    let ended = shell.read_with(cx, |shell, _| (shell.drag.is_none(), shell.putting_back));
    assert_eq!(ended, (true, true), "the drag ended, putting the row back");
    shell.update(cx, |shell, cx| shell.drop_row(cx));
    assert_eq!(host.moves.get(), 0);
}
