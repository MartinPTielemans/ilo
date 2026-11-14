//! A Space card's name, typed over in place in the Library, and the icon slot and brush beside it.

use std::{cell::RefCell, rc::Rc};

use gpui::{Entity, Modifiers, Pixels, Point, TestAppContext, VisualTestContext, point, px};

use super::{
    CARD_BRUSH, CARD_BRUSH_INSET, CARD_FIELD_INSET, CARD_MORE_X, CARD_MORE_Y, CARD_NAME_GAP,
    CARD_NAME_HEIGHT, CARD_NAME_TOP, CARD_SLOT, CARD_SLOT_X, CARD_WIDTH, SPACES_INSET, card_x,
};
use crate::icon_picker::{self, Recents, Subject};
use crate::shell::{Command, Naming, Pending, Section, Shell, recorder::Recorder};
use crate::theme_picker;
use crate::tokens::icon_picker::ARROW_X;

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

/// Where a picker's window was put each time it showed: its top left in the browser window.
type Placed = Rc<RefCell<Vec<(f32, f32)>>>;

struct IconHost(Placed);

impl icon_picker::Host for IconHost {
    fn show(&self, _: u64, left: f32, top: f32) {
        self.0.borrow_mut().push((left, top));
    }

    fn hide(&self) {}

    fn keep_recents(&self, _: &str) {}
}

struct ThemeHost(Placed);

impl theme_picker::Host for ThemeHost {
    fn show(&self, _: u64, left: f32, top: f32, _: &[(f32, f32, f32)]) {
        self.0.borrow_mut().push((left, top));
    }

    fn hide(&self) {}

    fn detent(&self) {}
}

/// The icon and theme pickers' windows, for `library` to open over, and where each was put.
fn pickers(cx: &mut TestAppContext) -> (Placed, Placed) {
    let (icons, themes) = (Placed::default(), Placed::default());
    let (icon_host, theme_host) = (IconHost(icons.clone()), ThemeHost(themes.clone()));
    cx.update(|cx| {
        icon_picker::open(Rc::new(icon_host), Recents::default(), cx);
        theme_picker::open(Rc::new(theme_host), cx);
    });
    (icons, themes)
}

fn shown(cx: &mut VisualTestContext) -> (Option<Subject>, Option<u64>) {
    cx.update(|_, cx| (icon_picker::open_for(cx), theme_picker::open_for(cx)))
}

/// The icon slot names itself on hover and hangs the icon picker for its Space from itself, the
/// Library staying open and the Space shown where it was, so another can be changed next (#1074).
#[gpui::test]
fn the_icon_slot_hangs_the_icon_picker_from_itself(cx: &mut TestAppContext) {
    let (icons, _) = pickers(cx);
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
    assert_eq!(shown(cx), (Some(Subject::Space(2)), None));
    let left = icons.borrow().last().map(|&(left, _)| left);
    assert_eq!(
        left.map(|left| left + ARROW_X),
        Some(f32::from(slot(1).x)),
        "not at the slot"
    );
    assert!(library_open(&shell, cx), "the Library closed");
    assert!(host.shown.borrow().is_empty(), "the slot showed the Space");
    assert_eq!(renaming_in_card(&shell, cx), None, "the slot renamed");
    let pending = shell.read_with(cx, |shell, _| shell.picker_pending.is_some());
    assert!(!pending, "the sidebar's picker is waiting too");
}

/// The brush opens the theme picker for its Space beside itself, the Library staying open.
#[gpui::test]
fn the_brush_opens_the_theme_picker_beside_itself(cx: &mut TestAppContext) {
    let (_, themes) = pickers(cx);
    let (shell, cx, host) = library(cx);
    click(cx, brush(0));
    assert_eq!(shown(cx), (None, Some(1)));
    let height = cx.update(|window, _| f32::from(window.viewport_size().height));
    let end = point(
        f32::from(brush(0).x) + CARD_BRUSH / 2.,
        f32::from(brush(0).y),
    );
    let (left, top, _) = theme_picker::placed(theme_picker::At::Row(end), false, height);
    assert_eq!(
        themes.borrow().last().copied(),
        Some((left, top)),
        "not beside the brush"
    );
    assert!(library_open(&shell, cx), "the Library closed");
    assert!(host.shown.borrow().is_empty(), "the brush showed the Space");
    assert_eq!(renaming_in_card(&shell, cx), None, "the brush renamed");
}

/// A click on another card's brush or slot moves the picker there, once the press has closed it,
/// whichever the picker hears of first. One on the brush whose picker shows closes it, as the
/// press outside the picker does, rather than opening it again.
#[gpui::test]
fn another_card_s_brush_or_slot_moves_the_picker_and_its_own_closes_it(cx: &mut TestAppContext) {
    let (icons, themes) = pickers(cx);
    let (_, cx, _) = library(cx);
    let dismiss_theme = |cx: &mut VisualTestContext| {
        cx.update(|_, cx| theme_picker::dismissed(cx));
        cx.run_until_parked();
    };
    click(cx, brush(0));
    // The host closes the picker on the press, which the picker hears after the click on Linux,
    click(cx, brush(1));
    assert_eq!(
        themes.borrow().len(),
        1,
        "the picker moved before it had closed"
    );
    dismiss_theme(cx);
    assert_eq!(shown(cx), (None, Some(2)));
    assert_eq!(themes.borrow().len(), 2);
    // and before it on the Mac.
    dismiss_theme(cx);
    click(cx, brush(0));
    assert_eq!(shown(cx), (None, Some(1)));
    click(cx, brush(0));
    assert_eq!(
        themes.borrow().len(),
        3,
        "the press on the picker's own brush opened it again"
    );
    dismiss_theme(cx);
    assert_eq!(shown(cx), (None, None));
    click(cx, slot(0));
    click(cx, slot(1));
    cx.update(|_, cx| icon_picker::dismissed(cx));
    cx.run_until_parked();
    assert_eq!(shown(cx), (Some(Subject::Space(2)), None));
    assert_eq!(icons.borrow().len(), 2);
}

/// While a picker shows for a card, its field stays lit, though the pointer has left it for the
/// picker; and once the picker closes, Esc closes the Library (the owner, #1074).
#[gpui::test]
fn a_card_s_field_stays_lit_while_its_picker_shows_and_escape_then_closes_the_library(
    cx: &mut TestAppContext,
) {
    let _ = pickers(cx);
    let (shell, cx, _) = library(cx);
    click(cx, brush(1));
    move_to(
        cx,
        point(px(card_x(1) + CARD_WIDTH / 2.), px(SPACES_INSET + 150.)),
    );
    assert_eq!(
        fields_shown(&shell, cx),
        (false, true),
        "the picker's card went to rest"
    );
    cx.update(|_, cx| theme_picker::dismissed(cx));
    cx.run_until_parked();
    assert_eq!(
        fields_shown(&shell, cx),
        (false, false),
        "the card stayed lit"
    );
    assert!(
        library_open(&shell, cx),
        "closing the picker closed the Library"
    );
    shell.update(cx, |shell, cx| shell.run(Command::Stop, "key", cx));
    cx.run_until_parked();
    assert!(!library_open(&shell, cx), "Esc left the Library open");
}

/// The command bar's Edit Theme…, Arc's, is the first command "theme" finds, and opens the theme
/// picker for the Space shown, as its menu's Edit Theme Color… does, once the Library has gone:
/// with its card 14 inside the sidebar's edge and the window's top, as Arc 1.171's is inside its
/// window there.
#[gpui::test]
fn edit_theme_in_the_command_bar_opens_the_theme_picker(cx: &mut TestAppContext) {
    let keys = crate::shell::UserKeys::default();
    let found = crate::shell::find_commands("theme", &keys, |name, _| Some(name));
    assert_eq!(
        found.first().map(|row| row.command),
        Some(Command::EditTheme)
    );
    let (_, themes) = pickers(cx);
    let (shell, cx, _) = library(cx);
    shell.update(cx, |shell, cx| {
        shell.run(Command::EditTheme, "command-bar", cx)
    });
    let pending = shell.read_with(cx, |shell, _| {
        shell.picker_pending.map(|(picker, _)| picker)
    });
    assert_eq!(pending, Some(Pending::Theme(1)));
    // It waits for the Library to have gone, its motion ended here, and for the name row to draw
    // where it drew the frame before.
    shell.update(cx, |shell, _| {
        (shell.library_motion, shell.sidebar_fade) = (None, None)
    });
    for _ in 0..4 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    assert_eq!(shown(cx), (None, Some(1)), "the picker didn't open");
    let edge = shell.read_with(cx, |shell, _| shell.width());
    let arrow = crate::tokens::theme_picker::ARROW;
    let card = themes
        .borrow()
        .last()
        .map(|&(left, top)| (left + arrow, top));
    assert_eq!(
        card,
        Some((edge + 14., 14.)),
        "not against the sidebar's edge at the top"
    );
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
/// and not the card's, the slot and the brush included (#1074), and keeps them while its name is
/// typed over (#811).
#[gpui::test]
fn a_card_s_field_shows_its_plate_under_the_pointer_or_while_renamed(cx: &mut TestAppContext) {
    let (shell, cx, _) = library(cx);
    assert_eq!(fields_shown(&shell, cx), (false, false), "at rest");
    move_to(cx, name(1));
    assert_eq!(
        fields_shown(&shell, cx),
        (false, true),
        "the pointer on the second's name"
    );
    move_to(cx, brush(1));
    assert_eq!(
        fields_shown(&shell, cx),
        (false, true),
        "the pointer on the second's brush"
    );
    move_to(cx, slot(1));
    assert_eq!(
        fields_shown(&shell, cx),
        (false, true),
        "the pointer on the second's slot"
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

/// The middle of the `index`th card's •••, at its foot.
fn more(index: usize, cx: &mut VisualTestContext) -> Point<Pixels> {
    let height = cx.update(|window, _| window.viewport_size().height);
    let foot = height - px(SPACES_INSET + CARD_MORE_Y);
    point(px(card_x(index) + CARD_WIDTH - CARD_MORE_X), foot)
}

/// The ••• at a card's foot opens that Space's menu, as a right-click on the card does, and
/// leaves the Space unshown and the Library open.
#[gpui::test]
fn a_card_s_more_button_opens_its_space_s_menu(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    let at = more(1, cx);
    move_to(cx, at);
    click(cx, at);
    {
        let menus = host.menus.borrow();
        assert_eq!(menus.len(), 1, "no menu, or more than one");
        assert!(
            menus[0]
                .iter()
                .any(|(title, _)| title.as_ref() == "Rename Space…"),
            "not the Space's"
        );
    }
    assert_eq!(
        host.menu_at.get(),
        Some(at),
        "the menu isn't at the pointer"
    );
    assert!(host.shown.borrow().is_empty(), "the ••• showed the Space");
    assert!(library_open(&shell, cx), "the Library closed");
    // It's the second card's Space's menu: its Rename Space… names Work, in the sidebar.
    cx.update(|_, cx| host.choose("Rename Space…", cx));
    cx.run_until_parked();
    let renamed = shell.read_with(cx, |shell, _| {
        shell.renaming.as_ref().map(|renaming| renaming.naming)
    });
    assert!(matches!(renamed, Some(Naming::Space(2))), "not Work's menu");
}

/// A click on one card's slot and then another's, with no frame between them, as the check's
/// driver sends them under Wayland, shows no Space and leaves the Library open: the first card
/// keeps no click of its own for the second's release (#2689).
#[gpui::test]
fn a_slot_s_click_leaves_its_card_no_click_for_the_next_release(cx: &mut TestAppContext) {
    use gpui::{MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput};
    let _ = pickers(cx);
    let (shell, cx, host) = library(cx);
    cx.update(|window, cx| {
        for at in [slot(1), slot(0)] {
            let modifiers = Modifiers::none();
            let events = [
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: at,
                    pressed_button: None,
                    modifiers,
                }),
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position: at,
                    modifiers,
                    click_count: 1,
                    first_mouse: false,
                }),
                PlatformInput::MouseUp(MouseUpEvent {
                    button: MouseButton::Left,
                    position: at,
                    modifiers,
                    click_count: 1,
                }),
            ];
            for event in events {
                window.dispatch_event(event, cx);
            }
        }
    });
    cx.run_until_parked();
    assert!(
        host.shown.borrow().is_empty(),
        "the first card showed its Space"
    );
    assert!(library_open(&shell, cx), "the Library closed");
}
