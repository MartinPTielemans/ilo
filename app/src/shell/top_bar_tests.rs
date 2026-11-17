//! Top bar (#3160): off by default, where nothing moves; on, the card starts under the bar, the
//! URL field is in it from the sidebar's edge to the card's right edge whether the sidebar shows
//! or not, a click on it opens the command bar over it, and the bar's own right-click, the
//! sidebar's, the command bar and ⇧⌘D turn it off again.

use std::rc::Rc;

use gpui::{
    Bounds, Entity, Modifiers, MouseButton, Pixels, TestAppContext, VisualTestContext, point, px,
    size,
};

use super::commands::{SHORTCUTS, ShellHandle, bar_commands};
use super::{
    CARD_INSET, Command, Folders, Page, Shell, SidebarRight, TopBar, WINDOW_BUTTONS,
    recorder::Recorder,
};
use crate::tokens::shell::{
    FULL_SCREEN_SIDEBAR_BUTTON_X, NAV_SIZE, NAV_TOP, SIDEBAR_BUTTON_X, TOP_BAR, URL_FIELD_HEIGHT,
    WINDOWS_BUTTON_PLATE,
};

const WINDOWS: bool = cfg!(target_os = "windows");

const WIDTH: f32 = 1000.;
const HEIGHT: f32 = 700.;

/// A window 1000 by 700 showing a Space with two tabs, the first selected, drawn.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let recorder = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host, cx));
    cx.simulate_resize(size(px(WIDTH), px(HEIGHT)));
    shell.update(cx, |shell, cx| {
        shell.set_profiles(vec![(0, "Default".into())], cx);
        let space = (1, "Work".into(), None, 0, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        let tabs = vec![10, 11];
        shell.set_tabs(1, Vec::new(), tabs, Vec::new(), Folders::default(), cx);
        for tab in [10, 11] {
            let url = format!("https://{tab}.test/").into();
            shell.set_page(
                tab,
                Page {
                    url,
                    ..Page::default()
                },
                cx,
            );
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

/// Settings' Top bar as the user sets it: every window reads it as it draws next.
fn bar(on: bool, shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        cx.set_global(TopBar(on));
        cx.refresh_windows();
    });
    draw(shell, cx);
}

/// The sidebar hidden, at rest, as ⌘S leaves it once its motion ends.
fn hide_sidebar(shell: &Entity<Shell>, cx: &mut VisualTestContext) {
    shell.update(cx, |shell, _| {
        shell.sidebar_hidden = true;
        shell.sidebar_motion = None;
    });
    draw(shell, cx);
}

/// Where the page is laid out.
fn laid(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Bounds<Pixels> {
    shell.read_with(cx, |shell, _| shell.page_laid.expect("no page laid out").0)
}

fn field(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Bounds<Pixels> {
    shell.read_with(cx, |shell, _| {
        shell.url_field_bounds().expect("no URL field")
    })
}

/// Off, as it is by default, the page, the field and the list are where they were before the bar
/// existed. On, only the card's top moves, down to the bar's bottom; the list rises into the
/// field's place, and the find bar, the zoom HUD and the toast go down with the card. Off again,
/// everything is back.
#[gpui::test]
fn on_the_card_starts_under_the_bar_and_off_nothing_moved(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx);
    assert!(
        !shell.read_with(cx, |shell, _| shell.top_bar),
        "on by default"
    );
    let (page, list, corner) = (
        laid(&shell, cx),
        shell.read_with(cx, |shell, _| shell.drawn_list_top()),
        shell.read_with(cx, |shell, _| shell.focused_pane_corner()),
    );
    assert_eq!(page.top(), px(CARD_INSET));

    bar(true, &shell, cx);
    let under = laid(&shell, cx);
    assert_eq!(under.top(), px(TOP_BAR));
    assert_eq!((under.left(), under.right()), (page.left(), page.right()));
    assert_eq!(under.bottom(), page.bottom());
    let risen = shell.read_with(cx, |shell, _| shell.drawn_list_top());
    assert_eq!(list - risen, URL_FIELD_HEIGHT + 8.);
    let dropped = shell.read_with(cx, |shell, _| shell.focused_pane_corner());
    assert_eq!(dropped.2 - corner.2, TOP_BAR - CARD_INSET);

    bar(false, &shell, cx);
    assert_eq!(laid(&shell, cx), page);
    assert_eq!(shell.read_with(cx, |shell, _| shell.drawn_list_top()), list);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.focused_pane_corner()),
        corner
    );
}

/// The field is in the bar, from the sidebar's edge to the card's right edge, or on Windows to the
/// window's buttons there, and centred on the header's line, and stays there with the sidebar
/// hidden. With the sidebar on the right it's the left's mirrored.
#[gpui::test]
fn the_field_is_in_the_bar_with_the_sidebar_shown_or_hidden(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx);
    bar(true, &shell, cx);
    let width = shell.read_with(cx, |shell, _| shell.width());
    let shown = field(&shell, cx);
    // Windows' buttons, minimize at WINDOW_BUTTONS' last place from the corner.
    let buttons = match WINDOWS {
        true => WINDOW_BUTTONS[2].0 + WINDOWS_BUTTON_PLATE / 2.,
        false => 0.,
    };
    let expected = Bounds::new(
        point(px(width), px((TOP_BAR - URL_FIELD_HEIGHT) / 2.)),
        size(
            px(WIDTH - buttons - CARD_INSET - width),
            px(URL_FIELD_HEIGHT),
        ),
    );
    assert_eq!(shown, expected);
    assert_eq!(shown.right(), laid(&shell, cx).right() - px(buttons));

    hide_sidebar(&shell, cx);
    assert_eq!(
        field(&shell, cx),
        shown,
        "the field moved with the sidebar hidden"
    );
    assert_eq!(laid(&shell, cx).left(), px(CARD_INSET));

    cx.update(|_, cx| cx.set_global(SidebarRight(true)));
    draw(&shell, cx);
    let right = field(&shell, cx);
    assert_eq!(right.left(), px(CARD_INSET));
    assert_eq!(right.right(), px(WIDTH - width));
}

/// On Windows the bar has the window's buttons at its right end, where Windows has them, drawn by
/// the window whether the sidebar shows or not, and the host offers Snap layouts there. With the
/// sidebar on the right they're in its column again, as they are with the bar off. The Mac's and
/// Linux's never move.
#[gpui::test]
fn on_windows_the_window_buttons_go_to_the_bars_end(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    host.take();
    let at_end = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.buttons_at_bar_end())
    };
    let told = |right: bool, host: &Recorder| {
        host.take()
            .contains(&format!("window buttons right {right}"))
    };

    bar(true, &shell, cx);
    assert_eq!(at_end(&shell, cx), WINDOWS);
    assert_eq!(told(true, &host), WINDOWS);
    hide_sidebar(&shell, cx);
    assert_eq!(at_end(&shell, cx), WINDOWS);
    assert!(shell.read_with(cx, |shell, _| shell.window_buttons_shown()));

    cx.update(|_, cx| cx.set_global(SidebarRight(true)));
    draw(&shell, cx);
    assert!(!at_end(&shell, cx));
    assert_eq!(told(false, &host), WINDOWS);

    cx.update(|_, cx| cx.set_global(SidebarRight(false)));
    draw(&shell, cx);
    assert_eq!(told(true, &host), WINDOWS);
    bar(false, &shell, cx);
    assert!(!at_end(&shell, cx));
    assert_eq!(told(false, &host), WINDOWS);
}

/// A click on the bar's field opens the command bar over the field, as a click on the sidebar's
/// does, the sidebar hidden too.
/// On Windows, with the window's buttons gone to the bar's end, the sidebar button takes the corner
/// they left, where full screen has it, and back, forward and reload stay where they are. With the
/// bar off it follows the window's buttons again. The Mac's and Linux's stays after their buttons.
#[gpui::test]
fn on_windows_the_sidebar_button_takes_the_corner_the_buttons_left(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx);
    let hides = |x: f32, shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        cx.simulate_click(
            point(px(x + NAV_SIZE / 2.), px(NAV_TOP + NAV_SIZE / 2.)),
            Modifiers::none(),
        );
        cx.run_until_parked();
        let hidden = shell.update(cx, |shell, _| {
            let hidden = shell.sidebar_hidden;
            shell.sidebar_hidden = false;
            shell.sidebar_motion = None;
            hidden
        });
        draw(shell, cx);
        hidden
    };

    bar(true, &shell, cx);
    let corner = match WINDOWS {
        true => FULL_SCREEN_SIDEBAR_BUTTON_X,
        false => SIDEBAR_BUTTON_X,
    };
    assert!(hides(corner, &shell, cx), "no sidebar button at {corner}");
    bar(false, &shell, cx);
    assert!(
        hides(SIDEBAR_BUTTON_X, &shell, cx),
        "the sidebar button didn't go back"
    );
}

#[gpui::test]
fn a_click_on_the_bars_field_opens_the_command_bar_over_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    bar(true, &shell, cx);
    for hidden in [false, true] {
        if hidden {
            hide_sidebar(&shell, cx);
        }
        let field = field(&shell, cx);
        host.command_bars.borrow_mut().clear();
        // Left of the field's buttons, on its words.
        let at = point(field.left() + px(40.), field.center().y);
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
        let opened = host.command_bars.borrow().last().copied();
        assert_eq!(opened, Some(Some(field)), "sidebar hidden {hidden}");
        assert_eq!(host.bar_edits.borrow().last().copied(), Some(Some(10)));
        host.bar_open.set(false);
    }
}

/// The command bar lists the one that acts, with its key, ⇧⌘D, unreserved as ⌘S is, and runs it
/// in every window through Settings' own setter.
#[gpui::test]
fn the_command_bar_and_its_key_turn_it_on_and_off(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx);
    let key = SHORTCUTS
        .iter()
        .find(|(_, command, _)| *command == Command::ToggleTopBar)
        .map(|&(key, _, reserved)| (key, reserved));
    assert_eq!(key, Some(("shift-cmd-d", false)));
    let listed = |cx: &mut VisualTestContext| {
        let found = cx.update(|_, cx| bar_commands("top bar", cx));
        (found.iter())
            .filter(|row| row.command == Command::ToggleTopBar)
            .map(|row| (row.name.to_string(), row.shortcut.is_some()))
            .collect::<Vec<_>>()
    };
    assert_eq!(listed(cx), [("Show Top Bar".to_owned(), true)]);
    shell.update(cx, |shell, cx| {
        shell.run(Command::ToggleTopBar, "command bar", cx)
    });
    draw(&shell, cx);
    assert!(shell.read_with(cx, |shell, _| shell.top_bar));
    assert_eq!(laid(&shell, cx).top(), px(TOP_BAR));
    assert_eq!(listed(cx), [("Hide Top Bar".to_owned(), true)]);
    shell.update(cx, |shell, cx| shell.run(Command::ToggleTopBar, "ui", cx));
    draw(&shell, cx);
    assert!(!shell.read_with(cx, |shell, _| shell.top_bar));
    assert_eq!(laid(&shell, cx).top(), px(CARD_INSET));
}

/// The bar's own right-click is its way out, where it's seen: Hide Top Bar alone. The sidebar's
/// empty space has the same item after New Tab and New Folder…, which says Show Top Bar while
/// it's off.
#[gpui::test]
fn the_bars_right_click_and_the_sidebars_turn_it_off(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    let right_click = |at, cx: &mut VisualTestContext| {
        host.menus.borrow_mut().clear();
        cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
        cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
        let menus = host.menus.borrow();
        let last = menus.last().cloned().unwrap_or_default();
        last.into_iter()
            .map(|(title, _)| title.to_string())
            .collect::<Vec<_>>()
    };
    let empty = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        let y = shell.read_with(cx, |shell, _| shell.drawn_list_top()) + 200.;
        point(px(100.), px(y))
    };
    let at = empty(&shell, cx);
    let menu = right_click(at, cx);
    assert_eq!(
        menu.get(3).map(String::as_str),
        Some("Show Top Bar"),
        "{menu:?}"
    );
    cx.update(|_, cx| host.choose("Show Top Bar", cx));
    draw(&shell, cx);
    assert!(shell.read_with(cx, |shell, _| shell.top_bar));

    // Over the field's left end, above its top: the bar's empty space.
    let field = field(&shell, cx);
    let menu = right_click(point(field.left() + px(20.), px(2.)), cx);
    assert_eq!(menu, ["Hide Top Bar"]);
    // And on the field itself, which keeps no right-click of its own.
    let menu = right_click(point(field.left() + px(40.), field.center().y), cx);
    assert_eq!(menu, ["Hide Top Bar"]);
    cx.update(|_, cx| host.choose("Hide Top Bar", cx));
    draw(&shell, cx);
    assert!(!shell.read_with(cx, |shell, _| shell.top_bar));
    assert_eq!(laid(&shell, cx).top(), px(CARD_INSET));
}

/// A page full screen takes the window, the bar with the sidebar.
#[gpui::test]
fn a_full_screen_page_takes_the_bar_too(cx: &mut TestAppContext) {
    let (shell, cx, _) = window(cx);
    bar(true, &shell, cx);
    shell.update(cx, |shell, _| shell.page_fullscreen = true);
    draw(&shell, cx);
    let page = laid(&shell, cx);
    assert_eq!(page.top(), px(0.));
    assert!(!shell.read_with(cx, |shell, _| shell.bar_on()));
}
