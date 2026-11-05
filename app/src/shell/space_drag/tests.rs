//! A Space dragged by its dot along the switcher, and by its card's handle along the Library's
//! Spaces, as the user drags them (#1580): the order the core is asked for, what shows while it's
//! dragged, and ⌘Z.

use std::rc::Rc;

use gpui::{
    Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px,
};

use super::drop_index;
use crate::shell::library::card_x;
use crate::shell::{Section, Shell, SpaceEntry, recorder::Recorder};
use crate::tokens::shell::library::{CARD_HANDLE_X, CARD_HANDLE_Y, SPACES_INSET};

fn space(id: u64, name: &str) -> SpaceEntry {
    (
        id,
        name.to_string().into(),
        None,
        1,
        "".into(),
        "".into(),
        "".into(),
    )
}

/// Home, Work and Play, with Home shown, drawn, and what the host was asked by then forgotten.
fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![space(1, "Home"), space(2, "Work"), space(3, "Play")],
            cx,
        );
        shell.set_space(1, cx);
    });
    cx.run_until_parked();
    host.take();
    host.take_edits();
    host.shown.take();
    (shell, cx, host)
}

/// The middle of the Space's dot in the switcher, where it was drawn.
fn dot(shell: &Entity<Shell>, cx: &mut VisualTestContext, space: u64) -> Point<Pixels> {
    let frame = shell.read_with(cx, |shell, _| {
        shell.dot_frames.borrow().get(&space).copied()
    });
    frame.expect("the dot wasn't drawn").center()
}

/// Presses at `from` and drags past the threshold to `to`, without letting go.
fn take(cx: &mut VisualTestContext, from: Point<Pixels>, to: Point<Pixels>) {
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        from + point(px(6.), px(0.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

fn let_go(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

/// The Spaces' order as the core now has it, after it moved one, which each window hears.
fn reorder(shell: &Entity<Shell>, cx: &mut VisualTestContext, order: [u64; 3]) {
    let name = |id: u64| ["", "Home", "Work", "Play"][id as usize];
    shell.update(cx, |shell, cx| {
        shell.set_spaces(order.map(|id| space(id, name(id))).to_vec(), cx)
    });
    cx.run_until_parked();
}

/// The dragged one takes the place past each other's middle the pointer is.
#[test]
fn a_drag_takes_the_place_past_the_middles_it_passes() {
    let middle = |index: usize| 10. + index as f32 * 20.;
    assert_eq!(drop_index(3, 2, 0., middle), 0);
    assert_eq!(drop_index(3, 2, 15., middle), 1);
    assert_eq!(drop_index(3, 2, 45., middle), 2);
    assert_eq!(drop_index(3, 0, 35., middle), 1);
    assert_eq!(drop_index(3, 0, 55., middle), 2);
}

/// Play's dot dragged before Home's: nothing moves while it's held, a line marks the gap it would
/// go into, and let go there the core is asked to move it first, not shown and with no click on
/// its dot. ⌘Z asks for it back at its place, and ⇧⌘Z first again.
#[gpui::test]
fn a_dot_dragged_along_the_switcher_moves_its_space(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    let (home, play) = (dot(&shell, cx, 1), dot(&shell, cx, 3));
    take(cx, play, home - point(px(12.), px(0.)));
    assert!(
        host.take_edits().is_empty(),
        "it moved before it was let go"
    );
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.space_gap(false)),
        Some(0)
    );
    let_go(cx, home - point(px(12.), px(0.)));
    assert_eq!(host.take_edits(), ["move space 3 to 0"]);
    assert!(
        host.shown.take().is_empty(),
        "the drag was a click on its dot"
    );
    assert_eq!(shell.read_with(cx, |shell, _| shell.space_gap(false)), None);

    reorder(&shell, cx, [3, 1, 2]);
    shell.update(cx, |shell, cx| assert!(shell.undo(cx)));
    assert_eq!(host.take_edits(), ["move space 3 to 2"]);
    reorder(&shell, cx, [1, 2, 3]);
    shell.update(cx, |shell, cx| assert!(shell.redo(cx)));
    assert_eq!(host.take_edits(), ["move space 3 to 0"]);
}

/// Let go where it was, between its neighbours, there's no gap to mark and nothing moves.
#[gpui::test]
fn a_dot_let_go_where_it_was_stays(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    let work = dot(&shell, cx, 2);
    take(cx, work, work + point(px(3.), px(0.)));
    assert_eq!(shell.read_with(cx, |shell, _| shell.space_gap(false)), None);
    let_go(cx, work + point(px(3.), px(0.)));
    assert!(host.take_edits().is_empty());
    let undo = shell.update(cx, |shell, cx| shell.undo(cx));
    assert!(!undo, "a drop where it was left a step to undo");
}

/// In the Library, Home's card dragged by its handle past Work's goes second, the Library staying
/// open with no Space shown; the card's click still shows its Space. ⌘Z asks for it first again.
#[gpui::test]
fn a_card_dragged_by_its_handle_moves_its_space(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    shell.update(cx, |shell, cx| {
        shell.set_space(2, cx);
        shell.section = Section::Spaces;
        shell.set_library(true, cx);
        shell.library_motion = None;
        shell.sidebar_fade = None;
    });
    cx.run_until_parked();
    host.take();
    host.shown.take();
    let height = shell.read_with(cx, |shell, _| shell.card_height.get());
    assert!(height > 0., "the cards weren't drawn");
    let handle = point(
        px(card_x(0) + CARD_HANDLE_X),
        px(SPACES_INSET + height - CARD_HANDLE_Y),
    );
    // Past the middle of Work's card, the second.
    let past_work = point(px(card_x(1) + 200.), handle.y);
    take(cx, handle, past_work);
    assert!(
        host.take_edits().is_empty(),
        "it moved before it was let go"
    );
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.space_gap(true)),
        Some(2)
    );
    let_go(cx, past_work);
    assert_eq!(host.take_edits(), ["move space 1 to 1"]);
    assert!(
        host.shown.take().is_empty(),
        "the drag was a click on its card"
    );
    assert!(shell.read_with(cx, |shell, _| shell.library_open));

    reorder(&shell, cx, [2, 1, 3]);
    shell.update(cx, |shell, cx| assert!(shell.undo(cx)));
    assert_eq!(host.take_edits(), ["move space 1 to 0"]);
}
