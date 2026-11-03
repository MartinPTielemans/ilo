//! A Space's dot dragged along the sidebar's switcher moves the Space, ⌘Z puts it back, and the
//! dots glide to their places once the core reports the new order (#1580).

use std::rc::Rc;

use gpui::{
    Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px,
};

use crate::shell::{Command, FOOTER_HEIGHT, SWITCHER_INSET, Shell, SpaceEntry, recorder::Recorder};

fn entry(id: u64) -> SpaceEntry {
    (id, format!("Space {id}").into(), None, 1, "".into(), "".into(), "".into())
}

/// A window with Spaces 1, 2 and 3, the first shown.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, _| window.activate_window());
    report(&shell, cx, &[1, 2, 3]);
    shell.update(cx, |shell, cx| shell.set_space(1, cx));
    cx.run_until_parked();
    host.take_edits();
    host.shown.take();
    (shell, cx, host)
}

/// The core reports the Spaces in `order`, as it does a moment after a move.
fn report(shell: &Entity<Shell>, cx: &mut VisualTestContext, order: &[u64]) {
    let spaces = order.iter().map(|&id| entry(id)).collect();
    shell.update(cx, |shell, cx| shell.set_spaces(spaces, cx));
    cx.run_until_parked();
}

/// The middle of `space`'s dot in the window, as drawn now.
fn dot(shell: &Entity<Shell>, cx: &mut VisualTestContext, space: u64) -> Point<Pixels> {
    shell.read_with(cx, |shell, _| {
        let x = SWITCHER_INSET + shell.dot_places()[&space];
        point(px(x), shell.viewport.height - px(FOOTER_HEIGHT / 2.))
    })
}

/// Presses at `from` and drags to `to`, without letting go.
fn hold(cx: &mut VisualTestContext, from: Point<Pixels>, to: Point<Pixels>) {
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(from + point(px(6.), px(0.)), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

fn let_go(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

/// The third Space's dot let go left of the first's moves it first, which ⌘Z undoes and ⇧⌘Z does
/// again. The drag isn't a click: it shows no Space.
#[gpui::test]
fn a_dot_dragged_before_the_first_moves_its_space_first(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    let to = dot(&shell, cx, 1) - point(px(12.), px(0.));
    let from = dot(&shell, cx, 3);
    hold(cx, from, to);
    assert!(shell.read_with(cx, |shell, _| shell.space_dragged(3)));
    let_go(cx, to);
    assert_eq!(host.take_edits(), ["move space 3 after 0"]);
    assert!(host.shown.take().is_empty(), "the drag was a click on the dot");
    assert!(!shell.read_with(cx, |shell, _| shell.space_dragged(3)));

    report(&shell, cx, &[3, 1, 2]);
    assert!(shell.update(cx, |shell, cx| shell.undo(cx)));
    assert_eq!(host.take_edits(), ["move space 3 after 2"]);
    report(&shell, cx, &[1, 2, 3]);
    assert!(shell.update(cx, |shell, cx| shell.redo(cx)));
    assert_eq!(host.take_edits(), ["move space 3 after 0"]);
}

/// Let go between two dots, it goes after the left one; let go on its own place, off the footer,
/// or after Esc, it moves nothing.
#[gpui::test]
fn a_dot_goes_into_the_gap_it_was_let_go_over_and_nowhere_else(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    let (two, three) = (dot(&shell, cx, 2), dot(&shell, cx, 3));
    let between = point((two.x + three.x) / 2., two.y);
    let from = dot(&shell, cx, 1);
    hold(cx, from, between);
    let_go(cx, between);
    assert_eq!(host.take_edits(), ["move space 1 after 2"]);
    report(&shell, cx, &[2, 1, 3]);

    let own = dot(&shell, cx, 1) + point(px(4.), px(0.));
    let from = dot(&shell, cx, 1);
    hold(cx, from, own);
    let_go(cx, own);
    let above = dot(&shell, cx, 3) - point(px(0.), px(FOOTER_HEIGHT));
    let from = dot(&shell, cx, 1);
    hold(cx, from, above);
    let_go(cx, above);
    let last = dot(&shell, cx, 3) + point(px(20.), px(0.));
    let from = dot(&shell, cx, 1);
    hold(cx, from, last);
    shell.update(cx, |shell, cx| shell.run(Command::Stop, "key", cx));
    let_go(cx, last);
    assert!(host.take_edits().is_empty());
    assert!(host.shown.take().is_empty());
}

/// Once the core reports the order, the dot let go glides from where its copy was, and the dot it
/// passed from its old place. The copy is held where the pointer was as the drag began, 6 past
/// the press, so it was let go 18 left of the first place.
#[gpui::test]
fn the_dots_glide_to_their_new_places(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx);
    let (first, second) = (dot(&shell, cx, 1), dot(&shell, cx, 2));
    let to = first - point(px(12.), px(0.));
    hold(cx, second, to);
    let_go(cx, to);
    report(&shell, cx, &[2, 1, 3]);
    let slides = shell.read_with(cx, |shell, _| (shell.dot_slide(2), shell.dot_slide(1)));
    let pitch = f32::from(second.x - first.x);
    assert!((slides.0 + 18.).abs() < 1., "the dot let go starts at its copy: {slides:?}");
    assert!((slides.1 + pitch).abs() < 1., "the dot passed starts at its old place: {slides:?}");
}

/// A press, one move and the release, as the check's driver sends a drag: the move starts the
/// drag, so none says where it is, and the release finds the gap itself.
#[gpui::test]
fn a_flick_with_one_move_still_moves_the_space(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    let (from, to) = (dot(&shell, cx, 1), dot(&shell, cx, 3) + point(px(20.), px(0.)));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(host.take_edits(), ["move space 1 after 3"]);
}
