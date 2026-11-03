//! A Space card's name, typed over in place in the Library, and the icon slot and brush beside it.

use std::rc::Rc;

use gpui::{Entity, Modifiers, Pixels, Point, TestAppContext, VisualTestContext, point, px};

use super::{
    CARD_BRUSH, CARD_BRUSH_INSET, CARD_FIELD_INSET, CARD_NAME_GAP, CARD_NAME_HEIGHT, CARD_NAME_TOP,
    CARD_SLOT, CARD_SLOT_X, CARD_WIDTH, SPACES_INSET, card_x,
};
use crate::icon_picker::Subject;
use crate::shell::{Command, Naming, Pending, Section, Shell, recorder::Recorder};

/// A window with the Library open on Spaces, at rest, over the Spaces Home and Work, Home shown.
fn library(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    cx.update(|cx| {
        crate::shell::commands::bind_keys(cx);
        crate::text_input::bind_keys(cx);
    });
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
        shell.set_spaces(vec![space(1, "Home"), space(2, "Work")], cx);
        shell.set_space(1, cx);
        shell.section = Section::Spaces;
        shell.set_library(true, cx);
        shell.library_motion = None;
        shell.sidebar_fade = None;
    });
    cx.run_until_parked();
    host.take();
    (shell, cx, host)
}

/// `x` in from the left of the `index`th card's field, halfway down it.
fn in_field(index: usize, x: f32) -> Point<Pixels> {
    let y = SPACES_INSET + CARD_NAME_TOP + CARD_NAME_HEIGHT / 2.;
    point(px(card_x(index) + CARD_FIELD_INSET + x), px(y))
}

fn name(index: usize) -> Point<Pixels> {
    in_field(index, CARD_SLOT_X + CARD_SLOT + CARD_NAME_GAP + 8.)
}

fn slot(index: usize) -> Point<Pixels> {
    in_field(index, CARD_SLOT_X + CARD_SLOT / 2.)
}

fn brush(index: usize) -> Point<Pixels> {
    in_field(
        index,
        CARD_WIDTH - 2. * CARD_FIELD_INSET - CARD_BRUSH_INSET - CARD_BRUSH / 2.,
    )
}

fn click(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

/// The Space whose name is a field in its card, if one is.
fn renaming_in_card(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<u64> {
    shell.read_with(cx, |shell, _| match shell.renaming.as_ref() {
        Some(renaming) if renaming.in_library => match renaming.naming {
            Naming::Space(space) => Some(space),
            _ => None,
        },
        _ => None,
    })
}

fn library_open(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |shell, _| shell.library_open)
}

/// A click on a card's name makes it a field there, with the Space left unshown and the Library
/// open, and return keeps what was typed over it, as Rename Space does.
#[gpui::test]
fn a_click_on_a_card_s_name_edits_it_in_place(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    click(cx, name(1));
    assert_eq!(
        renaming_in_card(&shell, cx),
        Some(2),
        "the name isn't a field"
    );
    assert!(host.shown.borrow().is_empty(), "the click showed the Space");
    assert!(library_open(&shell, cx), "the Library closed");
    cx.simulate_input("Play");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(host.take_edits(), ["rename space 2 Play"]);
    assert_eq!(renaming_in_card(&shell, cx), None, "the field stayed");
    assert!(library_open(&shell, cx), "return closed the Library");
}

/// Escape puts the old name back and leaves the Library open.
#[gpui::test]
fn escape_puts_back_the_card_s_old_name(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    click(cx, name(1));
    cx.simulate_input("Play");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(host.take_edits().is_empty(), "the Space was renamed");
    assert_eq!(renaming_in_card(&shell, cx), None, "the field stayed");
    assert!(library_open(&shell, cx), "escape closed the Library");
}

/// A name left empty is no name: the Space keeps its old one.
#[gpui::test]
fn an_empty_name_puts_back_the_old_one(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    click(cx, name(1));
    cx.simulate_keystrokes("backspace enter");
    cx.run_until_parked();
    assert!(host.take_edits().is_empty(), "the Space was renamed");
    assert_eq!(renaming_in_card(&shell, cx), None, "the field stayed");
}

/// A click outside the field keeps what was typed, and does what it would anyway: a click on
/// the Library's empty top leaves it open, and one on the rest of a card shows its Space.
#[gpui::test]
fn a_click_outside_the_field_keeps_the_name(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    click(cx, name(1));
    cx.simulate_input("Play");
    click(
        cx,
        point(px(card_x(1) + CARD_WIDTH / 2.), px(SPACES_INSET / 2.)),
    );
    assert_eq!(host.take_edits(), ["rename space 2 Play"]);
    assert_eq!(renaming_in_card(&shell, cx), None, "the field stayed");
    assert!(library_open(&shell, cx), "the click closed the Library");

    click(cx, name(0));
    cx.simulate_input("Rest");
    let height = cx.update(|window, _| f32::from(window.viewport_size().height));
    click(cx, point(px(card_x(1) + CARD_WIDTH / 2.), px(height / 2.)));
    assert_eq!(host.take_edits(), ["rename space 1 Rest"]);
    assert_eq!(*host.shown.borrow(), [2], "the card's Space isn't shown");
    assert!(!library_open(&shell, cx), "the Library stayed open");
}

/// A click in the field places the caret rather than showing the card's Space, and a click on
/// another card's name keeps this one's first.
#[gpui::test]
fn clicks_in_the_field_stay_in_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    click(cx, name(1));
    cx.simulate_input("Play");
    click(cx, name(1));
    assert_eq!(renaming_in_card(&shell, cx), Some(2), "the field closed");
    assert!(host.shown.borrow().is_empty(), "the click showed the Space");
    assert!(library_open(&shell, cx), "the Library closed");
    click(cx, name(0));
    assert_eq!(host.take_edits(), ["rename space 2 Play"]);
    assert_eq!(
        renaming_in_card(&shell, cx),
        Some(1),
        "the other name isn't a field"
    );
}

/// Closing the Library, however it closes, keeps the name being typed in a card.
#[gpui::test]
fn closing_the_library_keeps_the_name(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    click(cx, name(1));
    cx.simulate_input("Play");
    shell.update(cx, |shell, cx| shell.set_library(false, cx));
    cx.run_until_parked();
    assert_eq!(host.take_edits(), ["rename space 2 Play"]);
    assert_eq!(renaming_in_card(&shell, cx), None, "the field stayed");
}

/// The icon slot names itself on hover and opens the icon picker for its Space, without renaming.
#[gpui::test]
fn the_icon_slot_opens_the_icon_picker(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    cx.simulate_mouse_move(slot(1), None, Modifiers::none());
    cx.run_until_parked();
    assert!(
        host.take()
            .iter()
            .any(|line| line == "tooltip Edit Space icon "),
        "no tooltip"
    );
    click(cx, slot(1));
    let pending = shell.read_with(cx, |shell, _| {
        shell.picker_pending.map(|(picker, _)| picker)
    });
    assert_eq!(pending, Some(Pending::Icon(Subject::Space(2))));
    assert_eq!(renaming_in_card(&shell, cx), None, "the slot renamed");
}

/// The brush opens the theme picker for its Space, without renaming.
#[gpui::test]
fn the_brush_opens_the_theme_picker(cx: &mut TestAppContext) {
    let (shell, cx, _) = library(cx);
    click(cx, brush(0));
    let pending = shell.read_with(cx, |shell, _| {
        shell.picker_pending.map(|(picker, _)| picker)
    });
    assert_eq!(pending, Some(Pending::Theme(1)));
    assert_eq!(renaming_in_card(&shell, cx), None, "the brush renamed");
}

/// The command bar's Edit Theme…, Arc's, is the first command "theme" finds, and opens the theme
/// picker for the Space shown, as its menu's Edit Theme Color… does, once the Library has gone.
#[gpui::test]
fn edit_theme_in_the_command_bar_opens_the_theme_picker(cx: &mut TestAppContext) {
    let keys = crate::shell::UserKeys::default();
    let found = crate::shell::find_commands("theme", &keys, |name, _| Some(name));
    assert_eq!(
        found.first().map(|row| row.command),
        Some(Command::EditTheme)
    );
    let (shell, cx, _) = library(cx);
    shell.update(cx, |shell, cx| {
        shell.run(Command::EditTheme, "command-bar", cx)
    });
    let pending = shell.read_with(cx, |shell, _| {
        shell.picker_pending.map(|(picker, _)| picker)
    });
    assert_eq!(pending, Some(Pending::Theme(1)));
}

/// Whether each card's field was last drawn with its plate, dashes and brush, by Space.
fn fields_shown(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> (bool, bool) {
    shell.read_with(cx, |shell, _| {
        let drawn = shell.fields_drawn.borrow();
        (drawn[&1], drawn[&2])
    })
}

fn move_to(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.run_until_parked();
}

/// A card's field draws its plate, the slot's dashes and the brush only under the pointer, its own
/// and not the card's, and keeps them while its name is typed over (#811).
#[gpui::test]
fn a_card_s_field_shows_its_plate_under_the_pointer_or_while_renamed(cx: &mut TestAppContext) {
    let (shell, cx, _) = library(cx);
    assert_eq!(fields_shown(&shell, cx), (false, false), "at rest");
    move_to(cx, brush(1));
    assert_eq!(
        fields_shown(&shell, cx),
        (false, true),
        "the pointer on the second's brush"
    );
    move_to(
        cx,
        point(px(card_x(1) + CARD_WIDTH / 2.), px(SPACES_INSET + 150.)),
    );
    assert_eq!(
        fields_shown(&shell, cx),
        (false, false),
        "the pointer on the card, off its field"
    );
    click(cx, name(0));
    move_to(
        cx,
        point(px(card_x(1) + CARD_WIDTH / 2.), px(SPACES_INSET + 150.)),
    );
    assert_eq!(
        fields_shown(&shell, cx),
        (true, false),
        "the first's name typed over"
    );
}
