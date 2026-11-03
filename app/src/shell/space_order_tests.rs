//! A Space's dot dragged along the sidebar's switcher, or its card dragged by its handle along the
//! Library's, moves the Space, ⌘Z puts it back, and the dots and cards glide to their places once
//! the core reports the new order (#1580).

use std::rc::Rc;

use gpui::{
    Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px,
};

use crate::shell::{
    Command, FOOTER_HEIGHT, SWITCHER_INSET, Section, Shell, SpaceEntry, recorder::Recorder,
    space_order::Along,
};
use crate::tokens::shell::library::{
    CARD_GAP, CARD_MORE_X, CARD_MORE_Y, CARD_WIDTH, COLUMN_WIDTH, SPACES_INSET,
};

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

/// The core reports the Spaces in `order`, as it does a moment after a move, and the dots and
/// cards glide there.
fn report_gliding(shell: &Entity<Shell>, cx: &mut VisualTestContext, order: &[u64]) {
    let spaces = order.iter().map(|&id| entry(id)).collect();
    shell.update(cx, |shell, cx| shell.set_spaces(spaces, cx));
    cx.run_until_parked();
}

/// The core reports the Spaces in `order`, and the frame after the glides are done is drawn. The
/// glides run on the wall clock, which the test's doesn't drive, so they're ended here.
fn report(shell: &Entity<Shell>, cx: &mut VisualTestContext, order: &[u64]) {
    report_gliding(shell, cx, order);
    shell.update(cx, |shell, cx| {
        shell.space_slides.clear();
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// How far right of its place `space`'s dot or card started its glide, or 0 if it has none.
fn glide_from(shell: &Entity<Shell>, cx: &mut VisualTestContext, along: Along, space: u64) -> f32 {
    shell.read_with(cx, |shell, _| {
        shell.space_slides.get(&(along, space)).map_or(0., |&(from, _)| from)
    })
}

/// The middle of `space`'s dot in the window, as drawn now.
fn dot(shell: &Entity<Shell>, cx: &mut VisualTestContext, space: u64) -> Point<Pixels> {
    shell.read_with(cx, |shell, _| {
        let x = SWITCHER_INSET + shell.space_places()[&(Along::Switcher, space)];
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
    assert!(shell.read_with(cx, |shell, _| shell.space_dragged(1)), "the dot didn't drag");
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
    report_gliding(&shell, cx, &[2, 1, 3]);
    let slides =
        (glide_from(&shell, cx, Along::Switcher, 2), glide_from(&shell, cx, Along::Switcher, 1));
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

/// The window with the Library open on its Spaces, at rest.
fn library(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let (shell, cx, host) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.section = Section::Spaces;
        shell.set_library(true, cx);
        shell.library_motion = None;
        shell.sidebar_fade = None;
    });
    cx.run_until_parked();
    host.take_edits();
    host.shown.take();
    (shell, cx, host)
}

/// The middle of the handle on `space`'s card, at its foot 22 in from its left, as drawn now.
fn handle(shell: &Entity<Shell>, cx: &mut VisualTestContext, space: u64) -> Point<Pixels> {
    shell.read_with(cx, |shell, _| {
        let middle = shell.space_places()[&(Along::Cards, space)] - shell.cards_at();
        let x = COLUMN_WIDTH + middle - CARD_WIDTH / 2. + CARD_MORE_X;
        point(px(x), shell.viewport.height - px(SPACES_INSET + CARD_MORE_Y))
    })
}

/// Draws a frame, as the screen would while the pointer holds a card, so its copy is drawn whole.
fn frame(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// The third Space's card dragged by its handle and let go over the first card's left half moves
/// it first, with the Library left open and no Space shown; ⌘Z moves it back.
#[gpui::test]
fn a_card_dragged_by_its_handle_moves_its_space(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    let (from, to) = (handle(&shell, cx, 3), handle(&shell, cx, 1));
    hold(cx, from, to);
    frame(cx);
    assert!(shell.read_with(cx, |shell, _| shell.space_dragged(3)));
    let_go(cx, to);
    assert_eq!(host.take_edits(), ["move space 3 after 0"]);
    assert!(host.shown.take().is_empty(), "the drag was a click on the card");
    assert!(shell.read_with(cx, |shell, _| shell.library_open && !shell.space_dragged(3)));

    report(&shell, cx, &[3, 1, 2]);
    assert!(shell.update(cx, |shell, cx| shell.undo(cx)));
    assert_eq!(host.take_edits(), ["move space 3 after 2"]);
}

/// Let go between two cards, it goes after the left one; a click on the handle, or a card let go
/// on its own place, over the Library's column, or after Esc, moves nothing and shows nothing.
#[gpui::test]
fn a_card_goes_into_the_gap_it_was_let_go_over_and_nowhere_else(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    let between = handle(&shell, cx, 3) - point(px(CARD_MORE_X + CARD_GAP / 2.), px(0.));
    let from = handle(&shell, cx, 1);
    hold(cx, from, between);
    let_go(cx, between);
    assert_eq!(host.take_edits(), ["move space 1 after 2"]);
    report(&shell, cx, &[2, 1, 3]);

    let at = handle(&shell, cx, 3);
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    let own = handle(&shell, cx, 1) + point(px(40.), px(0.));
    let from = handle(&shell, cx, 1);
    hold(cx, from, own);
    let_go(cx, own);
    let column = point(px(COLUMN_WIDTH / 2.), from.y);
    hold(cx, from, column);
    let_go(cx, column);
    let last = handle(&shell, cx, 3) + point(px(CARD_WIDTH), px(0.));
    hold(cx, from, last);
    assert!(shell.read_with(cx, |shell, _| shell.space_dragged(1)), "the handle didn't drag");
    shell.update(cx, |shell, cx| shell.run(Command::Stop, "key", cx));
    let_go(cx, last);
    assert!(host.take_edits().is_empty());
    assert!(host.shown.take().is_empty());
    assert!(shell.read_with(cx, |shell, _| shell.library_open));
}

/// Once the core reports the order, the card let go glides from where its copy was, held as
/// the drag began 6 past the press on its handle, and the card it passed from its old place.
#[gpui::test]
fn the_cards_glide_to_their_new_places(cx: &mut TestAppContext) {
    let (shell, cx, _) = library(cx);
    let (first, second) = (handle(&shell, cx, 1), handle(&shell, cx, 2));
    let to = first - point(px(12.), px(0.));
    hold(cx, second, to);
    let_go(cx, to);
    report_gliding(&shell, cx, &[2, 1, 3]);
    let slides = (glide_from(&shell, cx, Along::Cards, 2), glide_from(&shell, cx, Along::Cards, 1));
    assert!((slides.0 + 18.).abs() < 1., "the card let go starts at its copy: {slides:?}");
    let pitch = CARD_WIDTH + CARD_GAP;
    assert!((slides.1 + pitch).abs() < 1., "the card passed starts at its old place: {slides:?}");
}
