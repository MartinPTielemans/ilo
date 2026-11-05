//! A tab dragged from one Space's card to another's in the Library (Arc).

use std::rc::Rc;

use gpui::{
    Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px,
};

use super::{CARD_NAME_HEIGHT, CARD_NAME_TOP, CARD_WIDTH, SPACES_INSET, card_x};
use crate::shell::{Folders, Section, Shell, recorder::Recorder};
use crate::tokens::shell::ROW_HEIGHT;

/// The Library open on Spaces over Home, with 3 pinned, and Work, which is empty, shown, so a click
/// on Home's card would show it.
fn library(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| {
        let space = |id: u64, name: &str| {
            (
                id,
                name.to_string().into(),
                None,
                1,
                "".into(),
                "".into(),
                "".into(),
            )
        };
        let spaces = vec![space(1, "Home"), space(2, "Work")];
        shell.set_spaces(spaces, cx);
        shell.set_space(2, cx);
        shell.set_tabs(1, vec![3], Vec::new(), Vec::new(), Folders::default(), cx);
        shell.section = Section::Spaces;
        shell.set_library(true, cx);
        shell.library_motion = None;
        shell.sidebar_fade = None;
    });
    cx.run_until_parked();
    host.take();
    host.take_edits();
    host.shown.take();
    (shell, cx, host)
}

/// The middle of the first row of the `index`th card.
fn first_row(index: usize) -> Point<Pixels> {
    let y = SPACES_INSET + CARD_NAME_TOP + CARD_NAME_HEIGHT + ROW_HEIGHT / 2.;
    point(px(card_x(index) + CARD_WIDTH / 2.), px(y))
}

/// Presses at `from`, drags to `to` and lets go there.
fn drag(cx: &mut VisualTestContext, from: Point<Pixels>, to: Point<Pixels>) {
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        from + point(px(10.), px(0.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

fn library_open(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |shell, _| shell.library_open)
}

/// Let go over another Space's card, an empty one too, a tab moves there as Move to moves it,
/// and the Library stays open with no Space shown.
#[gpui::test]
fn a_tab_dragged_to_another_spaces_card_moves_there(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    drag(cx, first_row(0), first_row(1));
    assert_eq!(host.take_edits(), ["space 3 2"]);
    assert!(host.shown.take().is_empty());
    assert!(library_open(&shell, cx));
}

/// Let go over its own card it stays, and isn't a click that shows the Space.
#[gpui::test]
fn its_own_card_takes_no_tab(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    drag(cx, first_row(0), first_row(0) + point(px(0.), px(40.)));
    assert!(host.take_edits().is_empty());
    assert!(
        host.shown.take().is_empty(),
        "the drag was a click on its card"
    );
    assert!(library_open(&shell, cx));
    // Where a click on its card does show it.
    cx.simulate_click(first_row(0) + point(px(0.), px(40.)), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(host.shown.take(), [1]);
}
