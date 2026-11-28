//! A Space's card dragged by its handle along the Library's moves the Space, ⌘Z puts it back, and
//! the cards glide to their places once the core reports the new order (#1580).

use std::rc::Rc;

use gpui::{
    Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px,
};

use crate::shell::{Command, Section, Shell, SpaceEntry, recorder::Recorder};
use crate::tokens::shell::library::{
    CARD_GAP, CARD_MORE_X, CARD_MORE_Y, CARD_WIDTH, COLUMN_WIDTH, SPACES_INSET,
};

fn entry(id: u64) -> SpaceEntry {
    (
        id,
        format!("Space {id}").into(),
        None,
        1,
        "".into(),
        "".into(),
        "".into(),
    )
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

/// The core reports the Spaces in `order`, as it does a moment after a move, and the cards
/// glide there.
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
        shell.space_sent.slides.clear();
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// How far right of its place `space`'s card started its glide, or 0 if it has none.
fn glide_from(shell: &Entity<Shell>, cx: &mut VisualTestContext, space: u64) -> f32 {
    shell.read_with(cx, |shell, _| {
        shell
            .space_sent
            .slides
            .get(&space)
            .map_or(0., |&(from, _)| from)
    })
}

/// Presses at `from` and drags to `to`, without letting go.
fn hold(cx: &mut VisualTestContext, from: Point<Pixels>, to: Point<Pixels>) {
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
        let middle = shell.space_places()[&space] - shell.cards_at();
        let x = COLUMN_WIDTH + middle - CARD_WIDTH / 2. + CARD_MORE_X;
        point(
            px(x),
            shell.viewport.height - px(SPACES_INSET + CARD_MORE_Y),
        )
    })
}

/// Draws a frame, as the screen would while the pointer holds a card, so its copy is drawn whole.
#[allow(dead_code)]
fn frame(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// One cell of the row: a card and its gap.
const PITCH: f32 = CARD_WIDTH + CARD_GAP;

/// Where the pointer holds `space`'s handle after the card's middle has gone `cells` cells right
/// and `extra` more.
fn moved(
    shell: &Entity<Shell>,
    cx: &mut VisualTestContext,
    space: u64,
    cells: f32,
    extra: f32,
) -> Point<Pixels> {
    handle(shell, cx, space) + point(px(cells * PITCH + extra), px(0.))
}

/// The cards sent aside, by Space, as the drag has them.
fn aside(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<(u64, f32)> {
    let mut aside: Vec<_> = shell.read_with(cx, |shell, _| {
        shell
            .space_sent
            .to
            .iter()
            .map(|(&id, &by)| (id, by))
            .collect()
    });
    aside.sort_by(|a, b| a.0.cmp(&b.0));
    aside
}

/// The first Space's card dragged by its handle past the second's middle moves only the second,
/// one cell left, and the card follows the pointer along x.
#[gpui::test]
fn a_card_passing_one_neighbour_moves_only_that_one(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    let from = handle(&shell, cx, 1);
    let to = moved(&shell, cx, 1, 1., 20.);
    hold(cx, from, to);
    assert!(shell.read_with(cx, |shell, _| shell.space_dragged(1)));
    assert_eq!(aside(&shell, cx), [(2, -PITCH)]);
    let at = shell.read_with(cx, |shell, _| shell.space_shift(1));
    assert!(
        (at - (PITCH + 20. - 6.)).abs() < 1.,
        "the card follows the pointer: {at}"
    );
    assert!(
        host.take_edits().is_empty(),
        "nothing reaches the core mid-drag"
    );
}

/// Past two middles, two cards move.
#[gpui::test]
fn a_card_passing_two_neighbours_moves_two(cx: &mut TestAppContext) {
    let (shell, cx, _) = library(cx);
    let from = handle(&shell, cx, 1);
    let to = moved(&shell, cx, 1, 2., 20.);
    hold(cx, from, to);
    assert_eq!(aside(&shell, cx), [(2, -PITCH), (3, -PITCH)]);
}

/// Let go, the card lands in the slot the others left open, tells the core once, and ⌘Z puts it
/// back; the others are already where the order puts them.
#[gpui::test]
fn the_release_lands_in_the_open_slot(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    let from = handle(&shell, cx, 1);
    let to = moved(&shell, cx, 1, 1., 20.);
    hold(cx, from, to);
    let_go(cx, to);
    assert_eq!(host.take_edits(), ["move space 1 after 2"]);
    assert!(!shell.read_with(cx, |shell, _| shell.space_dragged(1)));
    assert_eq!(aside(&shell, cx), [(1, PITCH), (2, -PITCH)]);
    assert!(
        host.shown.take().is_empty(),
        "the drag was a click on the card"
    );

    report_gliding(&shell, cx, &[2, 1, 3]);
    let from = glide_from(&shell, cx, 2);
    assert!(
        (from - PITCH).abs() < 1.,
        "the card passed starts where it was drawn: {from}"
    );
    assert!(aside(&shell, cx).is_empty());
    assert!(shell.update(cx, |shell, cx| shell.undo(cx)));
    assert_eq!(host.take_edits(), ["move space 1 after 0"]);
}

/// Esc sends everything back, with no word to the core, and the release after it moves nothing.
#[gpui::test]
fn escape_puts_every_card_back(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    let from = handle(&shell, cx, 1);
    let to = moved(&shell, cx, 1, 1., 20.);
    hold(cx, from, to);
    let before = shell.read_with(cx, |shell, _| shell.space_shift(2));
    shell.update(cx, |shell, cx| shell.run(Command::Stop, "key", cx));
    assert!(aside(&shell, cx).is_empty());
    let after = shell.read_with(cx, |shell, _| shell.space_shift(2));
    assert!(
        (after - before).abs() < 1.,
        "the card passed springs back from where it is drawn: {before} {after}"
    );
    assert!(glide_from(&shell, cx, 1) > 0., "the card springs home");
    let_go(cx, to);
    assert!(host.take_edits().is_empty());
    assert!(host.shown.take().is_empty());
    assert!(shell.read_with(cx, |shell, _| shell.library_open));
}

/// A card taken across and back to its start leaves no gap open, and its release says nothing; so
/// does one let go over the Library's column, or only clicked.
#[gpui::test]
fn a_card_back_where_it_started_moves_nothing(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    let from = handle(&shell, cx, 1);
    let out = moved(&shell, cx, 1, 1., 20.);
    hold(cx, from, out);
    assert_eq!(aside(&shell, cx).len(), 1);
    cx.simulate_mouse_move(from, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(aside(&shell, cx).is_empty());
    let_go(cx, from);
    assert!(host.take_edits().is_empty());

    let column = point(px(COLUMN_WIDTH / 2.), from.y);
    hold(cx, from, out);
    cx.simulate_mouse_move(column, MouseButton::Left, Modifiers::none());
    let_go(cx, column);
    assert!(host.take_edits().is_empty());
    assert!(aside(&shell, cx).is_empty());

    let third = handle(&shell, cx, 3);
    cx.simulate_click(third, Modifiers::none());
    cx.run_until_parked();
    assert!(host.take_edits().is_empty());
    assert!(shell.read_with(cx, |shell, _| shell.library_open));
}
