//! Off the Mac, the footer's menu control opens ilo's menu, what the Mac's menu bar holds under
//! ilo's name, above itself, and each of its items does what the menu bar's does (#3148).

use std::rc::Rc;

use gpui::{
    Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext, point, px,
};

use crate::shell::{FOOTER_HEIGHT, PLUS_INSET, SWITCHER_INSET, Shell, recorder::Recorder};
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
