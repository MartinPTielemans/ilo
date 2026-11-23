//! Off the Mac, the footer's menu control opens ilo's menu, what the Mac's menu bar holds under
//! ilo's name, above itself, and each of its items does what the menu bar's does (#3148).

use std::rc::Rc;

use gpui::{
    Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px,
};

use crate::shell::{
    FOOTER_HEIGHT, PLUS_INSET, SWITCHER_INSET, Shell,
    commands::{Command, bind_keys},
    recorder::Recorder,
};
use crate::tokens::shell::{NAV_PITCH, NAV_SIZE, SPACE_PITCH};

const TITLES: [&str; 7] = [
    "Settings…",
    "Import from Other Browsers…",
    "Privacy Policy",
    "",
    concat!("About ", env!("APP_NAME")),
    "",
    concat!("Quit ", env!("APP_NAME")),
];

/// A window with two Spaces, drawn with the menu control as off the Mac if `control`.
fn window(
    cx: &mut TestAppContext,
    control: bool,
) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    cx.update(bind_keys);
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| {
        shell.menu_control = control;
        let spaces = [1, 2]
            .map(|id| {
                (
                    id,
                    format!("Space {id}").into(),
                    None,
                    1,
                    "".into(),
                    "".into(),
                    "".into(),
                )
            })
            .to_vec();
        shell.set_spaces(spaces, cx);
        shell.set_space(1, cx);
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    (shell, cx, host)
}

/// The menu control's plate's top left, as the footer places it before the +.
fn plate(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Point<Pixels> {
    shell.read_with(cx, |shell, _| {
        let x = shell.width() - PLUS_INSET - NAV_PITCH - NAV_SIZE;
        let y = f32::from(shell.viewport.height) - (FOOTER_HEIGHT + NAV_SIZE) / 2.;
        point(px(x), px(y))
    })
}

/// Clicks the middle of the menu control, as the user does.
fn click_control(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    let middle = plate(shell, cx) + point(px(NAV_SIZE / 2.), px(NAV_SIZE / 2.));
    cx.simulate_click(middle, Modifiers::none());
    cx.run_until_parked();
}

fn last_menu(host: &Recorder) -> Vec<String> {
    let menus = host.menus.borrow();
    let items = menus.last().expect("a menu");
    items.iter().map(|(title, _)| title.to_string()).collect()
}

/// A click opens the menu with Settings… first, its foot above the control's plate and its left
/// edge on the plate's, so the control and the + stay in sight, and the control stays lit while
/// it's up.
#[gpui::test]
fn the_menu_control_opens_ilos_menu_above_itself(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, true);
    click_control(&shell, cx);
    assert_eq!(last_menu(&host), TITLES);
    let at = host.menu_at.get().expect("the menu's place");
    let plate = plate(&shell, cx);
    assert_eq!(at.x, plate.x, "the menu's left edge isn't on the control's");
    let entries = host.entries.borrow().clone();
    let foot = f32::from(at.y) + crate::menu::height(&entries);
    assert_eq!(
        foot,
        f32::from(plate.y) - crate::menu::FROM_CONTROL,
        "the menu's foot isn't above the control's plate"
    );
    assert!(shell.read_with(cx, |shell, _| shell.app_menu && shell.menu_open));

    // Closed without a choice, as Esc or a click outside closes it, nothing runs and the
    // control lets go.
    cx.update(|_, cx| host.dismiss(cx));
    cx.run_until_parked();
    assert!(host.app_asks.borrow().is_empty());
    assert!(shell.read_with(cx, |shell, _| !shell.app_menu && !shell.menu_open));
}

/// The menu opens on the click's release, not its press: it opens above the control, away from
/// the pointer, so a release it saw after opening would read as a drag let go outside it and
/// close it, as Windows' did.
#[gpui::test]
fn the_menu_control_opens_its_menu_on_the_release(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, true);
    let middle = plate(&shell, cx) + point(px(NAV_SIZE / 2.), px(NAV_SIZE / 2.));
    cx.simulate_mouse_down(middle, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert!(host.menus.borrow().is_empty(), "the press opened the menu");
    cx.simulate_mouse_up(middle, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(last_menu(&host), TITLES);
}

/// Each item does what the menu bar's does.
#[gpui::test]
fn each_of_the_menus_items_does_what_the_menu_bars_does(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, true);
    for title in TITLES.into_iter().filter(|title| !title.is_empty()) {
        click_control(&shell, cx);
        cx.update(|_, cx| host.choose(title, cx));
        cx.run_until_parked();
    }
    assert_eq!(
        *host.app_asks.borrow(),
        ["settings", "import", "about", "quit"]
    );
    assert_eq!(
        *host.opened_pages.borrow(),
        [crate::build_info::PRIVACY_POLICY]
    );
}

/// On the Mac the menu bar holds ilo's menu, so the footer has no control and its place is the
/// switcher's.
#[gpui::test]
fn the_mac_has_no_menu_control(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, false);
    click_control(&shell, cx);
    assert!(host.menus.borrow().is_empty());
}

/// The switcher keeps its centre over what's left between the Library and the control, so at the
/// sidebar's 228 three Spaces' dots fit at their pitch where four did.
#[gpui::test]
fn the_switcher_leaves_the_control_its_pitch(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx, true);
    let span = shell.update(cx, |shell, _| {
        shell.sidebar_width = 228.;
        shell.switcher_span()
    });
    assert_eq!(span, 228. - 2. * SWITCHER_INSET - NAV_PITCH);
    let full = |count: usize| {
        crate::shell::footer::space_cells(span, count, 0)
            .iter()
            .all(|&cell| cell == SPACE_PITCH)
    };
    assert!(full(3) && !full(4));
}

/// Focus ilo Menu as F10 from `from` runs it: the page's, or the UI's.
fn focus_menu(shell: &Entity<Shell>, from: &str, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, cx| shell.run(Command::FocusMenu, from, cx));
    cx.run_until_parked();
}

fn keyed(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |shell, _| shell.keyed_menu.is_some())
}

/// Focus ilo Menu, F10 off the Mac, rings the menu control with the UI holding the keyboard, as
/// Chrome's rings its menu button, and a screen reader says the control. Return, Space and Down
/// each open its menu where a click does, the ring giving way to the menu (#4030).
#[gpui::test]
fn focus_menu_rings_the_control_and_its_keys_open_the_menu(cx: &mut TestAppContext) {
    for key in ["enter", "space", "down"] {
        let (shell, cx, host) = window(cx, true);
        cx.simulate_a11y_activation();
        focus_menu(&shell, "page", cx);
        assert!(keyed(&shell, cx), "{key}");
        let drawn = shell.read_with(cx, |shell, _| shell.menu_ring_drawn.get());
        assert!(drawn, "{key}: the ring wasn't drawn");
        assert!(host.take().contains(&"focus ui".to_owned()), "{key}");
        let tree = cx.a11y_tree().expect("GPUI built a tree");
        let (_, node) = (tree.nodes.iter())
            .find(|(id, _)| *id == tree.focus)
            .expect("something has the keyboard");
        assert_eq!(
            node.label().unwrap_or_default().to_string(),
            concat!(env!("APP_NAME"), " menu"),
            "what a screen reader says"
        );
        assert!(
            host.menus.borrow().is_empty(),
            "{key}: a menu before the key"
        );
        cx.simulate_keystrokes(key);
        cx.run_until_parked();
        assert_eq!(last_menu(&host), TITLES, "{key}");
        let entries = host.entries.borrow().clone();
        let foot =
            f32::from(host.menu_at.get().expect("a place").y) + crate::menu::height(&entries);
        assert_eq!(
            foot,
            f32::from(plate(&shell, cx).y) - crate::menu::FROM_CONTROL,
            "{key}: the menu isn't where a click opens it"
        );
        assert!(!keyed(&shell, cx), "{key}: the ring stayed over the menu");
    }
}

/// Escape, or Focus ilo Menu again, gives the keyboard back where it was: to the page if F10 came
/// from it, else the window keeps it. Up goes nowhere, and nothing opens.
#[gpui::test]
fn escape_or_focus_menu_again_gives_the_keyboard_back(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, true);
    for leave in ["escape", "focus menu"] {
        focus_menu(&shell, "page", cx);
        cx.simulate_keystrokes("up");
        assert!(keyed(&shell, cx), "up left the control");
        host.take();
        match leave {
            "escape" => cx.simulate_keystrokes("escape"),
            _ => focus_menu(&shell, "ui", cx),
        }
        cx.run_until_parked();
        assert!(!keyed(&shell, cx), "{leave}");
        assert_eq!(host.take(), ["focus page"], "{leave}");
    }
    focus_menu(&shell, "ui", cx);
    host.take();
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!keyed(&shell, cx));
    assert!(
        host.take().is_empty(),
        "the page took a keyboard it hadn't had"
    );
    let root = cx.update(|window, cx| shell.read(cx).focus_handle.is_focused(window));
    assert!(root, "the window doesn't hold the keyboard");
    assert!(host.menus.borrow().is_empty());
}

/// Focus Sidebar from the control moves the ring to the tab shown's row, and a press anywhere ends
/// it, as it ends a row's.
#[gpui::test]
fn the_sidebars_ring_and_a_press_end_the_controls(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx, true);
    shell.update(cx, |shell, cx| {
        let none = crate::shell::Folders::default();
        shell.set_tabs(1, Vec::new(), vec![20], Vec::new(), none, cx);
        shell.set_selected(20, cx);
    });
    focus_menu(&shell, "page", cx);
    shell.update(cx, |shell, cx| shell.run(Command::FocusSidebar, "ui", cx));
    cx.run_until_parked();
    assert!(!keyed(&shell, cx));
    assert_eq!(shell.read_with(cx, |shell, _| shell.keyed), Some(20));

    focus_menu(&shell, "page", cx);
    assert!(keyed(&shell, cx));
    assert_eq!(shell.read_with(cx, |shell, _| shell.keyed), None);
    cx.simulate_click(point(px(600.), px(300.)), Modifiers::none());
    assert!(!keyed(&shell, cx));
}

/// On the Mac the menu bar holds ilo's menu, so there's no control to ring.
#[gpui::test]
fn the_mac_has_no_control_to_ring(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, false);
    focus_menu(&shell, "page", cx);
    assert!(!keyed(&shell, cx));
    assert!(!host.take().contains(&"focus ui".to_owned()));
}

/// The command bar finds Focus ilo Menu by its name, by "keyboard" as it finds Focus Sidebar, and
/// by "open menu", which a user looking for the menu types.
#[test]
fn the_bar_finds_focus_ilo_menu() {
    let name = concat!("Focus ", env!("APP_NAME"), " Menu");
    assert_eq!(Command::FocusMenu.title(), name);
    for typed in ["focus", "keyboard", "open", "open menu"] {
        let found = crate::shell::find_commands(typed, &Default::default(), |name, _| Some(name));
        assert!(
            found.iter().any(|row| row.command == Command::FocusMenu),
            "{typed} didn't find {name}"
        );
    }
}
