//! Sidebar position (#2375): Right puts the sidebar, the window's buttons and the reveal on the
//! window's right, and everything placed by the side is the left's mirrored across the window.
//! Left is the default, and back on it everything is where it was.

use std::rc::Rc;

use gpui::{
    Bounds, Entity, Modifiers, MouseButton, MouseDownEvent, Pixels, TestAppContext,
    VisualTestContext, point, px, size,
};

use super::commands::{ShellHandle, bar_commands};
use super::header::nav_places;
use super::{
    Command, Folders, NAV_SIZE, Page, REVEAL_WAIT, SIDEBAR_MAX, Section, Shell, SidebarRight,
    recorder::Recorder,
};
use crate::tokens::shell::library::{COLUMN_WIDTH, SECTION_WIDTH, SECTION_X};

const WIDTH: f32 = 1000.;

/// A window 1000 by 700 showing a Space with two tabs, the first selected, drawn.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let recorder = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host, cx));
    cx.simulate_resize(size(px(WIDTH), px(700.)));
    shell.update(cx, |shell, cx| {
        shell.set_profiles(vec![(0, "Default".into())], cx);
        let space = (1, "Work".into(), None, 0, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        );
        for tab in [10, 11] {
            let url = format!("https://{tab}.test/").into();
            let page = Page {
                url,
                ..Page::default()
            };
            shell.set_page(tab, page, cx);
        }
        shell.set_selected(10, cx);
    });
    cx.update(|_, cx| cx.set_global(ShellHandle(shell.downgrade())));
    draw(&shell, cx);
    (shell, cx, recorder)
}

fn draw(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

/// Settings' Sidebar position as the user sets it: every window reads it as it draws next.
fn side(right: bool, shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        cx.set_global(SidebarRight(right));
        cx.refresh_windows();
    });
    draw(shell, cx);
}

/// Where the page is laid out.
fn laid(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Bounds<Pixels> {
    shell.read_with(cx, |shell, _| shell.page_laid.expect("no page laid out").0)
}

fn mirrored(frame: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(point(px(WIDTH) - frame.right(), frame.top()), frame.size)
}

/// The page goes to the left of the sidebar, as far in from the window's left as it was from its
/// right, and the host hears to put the window's buttons and the reveal on the right. Back on the
/// left, it's where it was.
#[gpui::test]
fn on_the_right_the_page_is_the_left_mirrored(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    let left = laid(&shell, cx);
    host.take();
    side(true, &shell, cx);
    assert_eq!(laid(&shell, cx), mirrored(left));
    assert!(host.take().contains(&"sidebar right true".to_owned()));
    let (column, width) = shell.read_with(cx, |shell, _| (shell.column_x(), shell.width()));
    assert_eq!(column, WIDTH - width);

    side(false, &shell, cx);
    assert_eq!(laid(&shell, cx), left, "back on the left, the page moved");
    assert!(host.take().contains(&"sidebar right false".to_owned()));
    // Read again with nothing changed, it asks the host nothing.
    side(false, &shell, cx);
    assert!(
        !host
            .take()
            .iter()
            .any(|call| call.starts_with("sidebar right"))
    );
}

/// The header's buttons in B's order: from the right edge the window's buttons, the sidebar
/// button, and back, forward and reload in their usual order, as far from the page as they were
/// from it on the left.
#[gpui::test]
fn the_header_is_in_bs_order_on_the_right(_: &mut TestAppContext) {
    for full_screen in [false, true] {
        let left = nav_places(240., full_screen, false);
        let right = nav_places(240., full_screen, true);
        let sidebar = left[0].unwrap();
        assert_eq!(right[0], Some(240. - sidebar - NAV_SIZE));
        let navs = |places: [Option<f32>; 4]| places[1..].iter().flatten().copied().collect();
        let (left_navs, right_navs): (Vec<f32>, Vec<f32>) = (navs(left), navs(right));
        assert_eq!(left_navs.len(), right_navs.len());
        // Back, forward and reload stay in their order, left to right.
        assert!(right_navs.windows(2).all(|pair| pair[0] < pair[1]));
        // As far from the page: the last one's right from the column's right edge on the left,
        // the first one's left from its left edge on the right.
        let to_page = 240. - (left_navs.last().unwrap() + NAV_SIZE);
        assert_eq!(right_navs[0], to_page);
    }
}

/// The hidden sidebar comes out from the right edge, with the same move along it as on the left,
/// and the left edge brings nothing out.
#[gpui::test]
fn the_hidden_sidebar_comes_out_from_the_right_edge(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx);
    side(true, &shell, cx);
    shell.update(cx, |shell, _| shell.sidebar_hidden = true);
    let to = |x: f32, y: f32, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.pointer_over(point(px(x), px(y)), cx));
        cx.executor().advance_clock(REVEAL_WAIT * 2);
        shell.read_with(cx, |shell, _| shell.revealed)
    };
    assert!(!to(1., 300., cx));
    assert!(!to(1., 306., cx), "the left edge brought it out");
    assert!(!to(400., 306., cx));
    assert!(
        !to(WIDTH - 1., 300., cx),
        "a jump into the edge brought it out"
    );
    assert!(
        to(WIDTH - 1., 306., cx),
        "the right edge didn't bring it out"
    );
}

/// Out from the right, the pointer keeps the sidebar out or sends it away where it would at the
/// mirrored place on the left, and the pointer gone sends it away on both.
#[gpui::test]
fn the_revealed_sidebar_hears_the_pointer_mirrored(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx);
    let after = |right: bool, x: f32, cx: &mut VisualTestContext| {
        side(right, &shell, cx);
        shell.update(cx, |shell, cx| {
            shell.sidebar_hidden = true;
            shell.set_revealed(true, cx);
            shell.reveal_motion = None;
            shell.pointer_at(x, cx);
        });
        cx.executor().advance_clock(REVEAL_WAIT * 10);
        cx.run_until_parked();
        shell.read_with(cx, |shell, _| shell.revealed)
    };
    for x in [5., 120., 239., 260., 300., 420., 700.] {
        let left = after(false, x, cx);
        assert_eq!(
            after(true, WIDTH - x, cx),
            left,
            "at {x} from the sidebar's edge"
        );
    }
    assert!(!after(true, f32::MAX, cx), "the pointer gone kept it out");
}

/// The column's edge drags the width from the left of the column, between the same limits.
#[gpui::test]
fn the_edge_drags_the_width_from_the_columns_left(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx);
    side(true, &shell, cx);
    let drag = |to: f32, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| {
            let at = WIDTH - shell.width() + 3.;
            let event = MouseDownEvent {
                button: MouseButton::Left,
                position: point(px(at), px(300.)),
                modifiers: Modifiers::none(),
                click_count: 1,
                first_mouse: false,
            };
            shell.press_edge(&event, cx);
            shell.drag_edge(WIDTH - to + 3., cx);
            shell.let_go_edge(cx);
        });
        shell.read_with(cx, |shell, _| shell.sidebar_width)
    };
    assert_eq!(drag(300., cx), 300.);
    assert_eq!(drag(900., cx), SIDEBAR_MAX);
}

/// The command bar lists the move that acts, and runs it in every window through Settings' own
/// setter, with the page keeping the keyboard: nothing asks for it.
#[gpui::test]
fn the_command_bar_moves_it_and_the_page_keeps_the_keyboard(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    let listed = |cx: &mut VisualTestContext| {
        let found = cx.update(|_, cx| bar_commands("sidebar", cx));
        (found.iter())
            .filter(|row| row.command == Command::MoveSidebar)
            .map(|row| (row.name.to_string(), row.shortcut.is_some()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        listed(cx),
        [("Move Sidebar to the Right".to_owned(), false)]
    );
    let found = cx.update(|_, cx| bar_commands("position", cx));
    assert!(found.iter().any(|row| row.command == Command::MoveSidebar));
    host.take();
    shell.update(cx, |shell, cx| {
        shell.run(Command::MoveSidebar, "command bar", cx)
    });
    draw(&shell, cx);
    assert!(shell.read_with(cx, |shell, _| shell.right));
    let calls = host.take();
    assert!(
        calls.contains(&"sidebar right true".to_owned()),
        "{calls:?}"
    );
    assert!(
        !calls.iter().any(|call| call.starts_with("focus")),
        "{calls:?}"
    );
    assert_eq!(listed(cx), [("Move Sidebar to the Left".to_owned(), false)]);
    // What sidebar-right.sh types, which lists that move alone.
    let names = cx.update(|_, cx| bar_commands("move sidebar", cx));
    let names: Vec<_> = names.iter().map(|row| row.name.to_string()).collect();
    assert_eq!(names, ["Move Sidebar to the Left"]);
    shell.update(cx, |shell, cx| {
        shell.run(Command::MoveSidebar, "command bar", cx)
    });
    draw(&shell, cx);
    assert!(!shell.read_with(cx, |shell, _| shell.right));
}

/// In full screen the page fills the window either way, and the side still moves the hidden
/// sidebar's edge and the header.
#[gpui::test]
fn in_full_screen_the_side_moves_the_edge(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx);
    shell.update(cx, |shell, cx| shell.set_full_screen(true, cx));
    draw(&shell, cx);
    let left = laid(&shell, cx);
    side(true, &shell, cx);
    assert_eq!(laid(&shell, cx), mirrored(left));
    shell.update(cx, |shell, _| shell.sidebar_hidden = true);
    let to = |x: f32, y: f32, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.pointer_over(point(px(x), px(y)), cx));
        cx.executor().advance_clock(REVEAL_WAIT * 2);
        shell.read_with(cx, |shell, _| shell.revealed)
    };
    assert!(!to(WIDTH - 1., 300., cx));
    assert!(
        to(WIDTH - 1., 306., cx),
        "the right edge didn't bring it out"
    );
}

/// Open on the right, the Library takes the sidebar's side, list then sections, so the sections
/// stay at the window's edge: the card's edge meets the list's, the empty list's line is drawn in
/// the list, and a right-click on Archived Tabs, the fourth button, is heard where the left's is
/// mirrored. Back on the left, the button is where it was.
#[gpui::test]
fn on_the_right_the_library_s_columns_swap(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    side(true, &shell, cx);
    shell.update(cx, |shell, cx| {
        shell.section = Section::Archive;
        shell.set_library(true, cx);
        // At rest where it opens.
        shell.library_motion = None;
        shell.sidebar_fade = None;
    });
    draw(&shell, cx);
    let (library, width, card, drawn) = shell.read_with(cx, |shell, _| {
        let (card, _) = shell.card_frames(shell.viewport);
        let drawn = shell.empty_drawn.borrow().clone();
        (shell.library_x(), shell.library_width(), card, drawn)
    });
    assert_eq!(library + width, WIDTH);
    assert_eq!(f32::from(card.right()), library);
    let words: Vec<&str> = drawn.split(' ').collect();
    let (x, across): (f32, f32) = (words[2].parse().unwrap(), words[3].parse().unwrap());
    assert!(
        x >= library && x + across <= WIDTH - COLUMN_WIDTH,
        "the empty line isn't in the list: {drawn}"
    );
    // The column's buttons are centred 5 above its middle, 100 apart.
    let archive = |x: f32| point(px(x), px(700. / 2. + 45.));
    let menu = |at, cx: &mut VisualTestContext| {
        host.menus.borrow_mut().clear();
        cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
        cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
        let titles = host.menus.borrow().last().cloned().unwrap_or_default();
        titles.iter().any(|(title, _)| title == "Clear Archive")
    };
    let middle = SECTION_X + SECTION_WIDTH / 2.;
    assert!(
        menu(archive(WIDTH - middle), cx),
        "no Clear Archive on the right"
    );
    side(false, &shell, cx);
    assert!(
        menu(archive(middle), cx),
        "no Clear Archive back on the left"
    );
}
