//! What the back, forward and reload buttons ask of the core when held, right-clicked,
//! ⌘-clicked or middle-clicked, on a host that records it.

use std::rc::Rc;

use gpui::{
    AppContext, Entity, Modifiers, MouseButton, Pixels, Point, TestAppContext, VisualTestContext,
    point, px,
};

use super::HOLD;
use crate::shell::{Folders, HistoryPage, NAV_SIZE, NAV_TOP, Page, Shell, recorder::Recorder};

/// Back, forward and reload in the 228 pt sidebar a window starts with (Arc 1.170).
const BACK_X: f32 = 122.;
const FORWARD_X: f32 = 156.;
const RELOAD_X: f32 = 190.;

/// A drawn shell showing tab 10 of 10 and 11, whose page can go back but not forward.
fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
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
        shell.set_selected(10, cx);
        let page = Page {
            url: "https://example.org/c".into(),
            can_go_back: true,
            ..Page::default()
        };
        shell.set_page(10, page, cx);
    });
    cx.run_until_parked();
    host.take();
    (shell, cx, host)
}

fn button(x: f32) -> Point<Pixels> {
    point(px(x + NAV_SIZE / 2.), px(NAV_TOP + NAV_SIZE / 2.))
}

fn pages() -> Vec<HistoryPage> {
    let page = |offset, url: &str, title: &str| HistoryPage {
        offset,
        url: url.to_string().into(),
        title: title.to_string().into(),
        favicon: None,
    };
    vec![
        page(-1, "https://example.org/b", "B"),
        page(-2, "https://example.org/a", ""),
    ]
}

/// What the shell asked of the core, but the tooltips hovering the buttons asks for.
fn calls(host: &Recorder) -> Vec<String> {
    host.take()
        .into_iter()
        .filter(|call| !call.starts_with("tooltip"))
        .collect()
}

fn titles(host: &Recorder) -> Vec<String> {
    let menus = host.menus.borrow();
    let last = menus.last().expect("a menu");
    last.iter().map(|(title, _)| title.to_string()).collect()
}

/// A right-click on back asks for the pages back, lists them nearest first, an untitled one by
/// its URL, each as a page, and picking one goes there.
#[gpui::test]
fn right_click_on_back_lists_the_pages_back(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    cx.simulate_mouse_down(button(BACK_X), MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(calls(&host), ["history 10 back"]);
    shell.update(cx, |shell, cx| shell.show_history(10, false, pages(), cx));
    assert_eq!(titles(&host), ["B", "https://example.org/a"]);
    assert!(host.entries.borrow().iter().all(|entry| entry.page));
    cx.update(|_, cx| host.choose("https://example.org/a", cx));
    assert_eq!(calls(&host), ["go to -2 in 10"]);
}

/// Forward can't go anywhere, so it lists nothing.
#[gpui::test]
fn a_dimmed_button_lists_nothing(cx: &mut TestAppContext) {
    let (_shell, cx, host) = shell(cx);
    cx.simulate_mouse_down(button(FORWARD_X), MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_down(button(FORWARD_X), MouseButton::Left, Modifiers::none());
    cx.executor().advance_clock(HOLD);
    cx.run_until_parked();
    assert!(calls(&host).is_empty());
}

/// Held for HOLD, back lists the pages back, and letting go doesn't go back too. A shorter press
/// is a click.
#[gpui::test]
fn holding_back_lists_the_pages_back(cx: &mut TestAppContext) {
    let (_shell, cx, host) = shell(cx);
    cx.simulate_mouse_down(button(BACK_X), MouseButton::Left, Modifiers::none());
    cx.executor().advance_clock(HOLD);
    cx.run_until_parked();
    cx.simulate_mouse_up(button(BACK_X), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(calls(&host), ["history 10 back"]);

    cx.simulate_mouse_down(button(BACK_X), MouseButton::Left, Modifiers::none());
    cx.executor().advance_clock(HOLD / 2);
    cx.simulate_mouse_up(button(BACK_X), MouseButton::Left, Modifiers::none());
    cx.executor().advance_clock(HOLD);
    cx.run_until_parked();
    assert_eq!(calls(&host), ["back 10"]);
}

/// The pages come for the tab they were asked for while it's shown, or no menu opens.
#[gpui::test]
fn pages_for_a_tab_no_longer_shown_open_no_menu(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    cx.simulate_mouse_down(button(BACK_X), MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    shell.update(cx, |shell, cx| {
        shell.set_selected(11, cx);
        shell.show_history(10, false, pages(), cx);
    });
    assert!(host.menus.borrow().is_empty());
}

/// ⌘-click on back, or a middle click on reload, opens where it goes in a background tab, and
/// doesn't go there in the tab shown.
#[gpui::test]
fn cmd_and_middle_clicks_open_in_the_background(cx: &mut TestAppContext) {
    let (_shell, cx, host) = shell(cx);
    let cmd = cx.update(|_, cx| Modifiers::secondary_key(cx));
    cx.simulate_click(button(BACK_X), cmd);
    cx.run_until_parked();
    assert_eq!(calls(&host), ["open -1 of 10 in the background"]);
    cx.simulate_mouse_down(button(RELOAD_X), MouseButton::Middle, Modifiers::none());
    cx.simulate_mouse_up(button(RELOAD_X), MouseButton::Middle, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(calls(&host), ["open 0 of 10 in the background"]);
    cx.simulate_click(button(FORWARD_X), cmd);
    cx.run_until_parked();
    assert!(calls(&host).is_empty());
}

/// The titles of the reload button's menu, as this platform writes them.
fn reload_titles() -> Vec<String> {
    use crate::shell::MenuEntry;
    ["Reload", "Hard Reload", "", "Clear Site Data and Reload"]
        .map(|title| MenuEntry::label(title).to_string())
        .into()
}

/// A right-click on reload lists Reload and Hard Reload with their keys, then the site's data
/// cleared before a reload, and each runs its core call on the tab shown (#1959).
#[gpui::test]
fn right_click_on_reload_offers_its_kinds_of_reload(cx: &mut TestAppContext) {
    let (_shell, cx, host) = shell(cx);
    let calls_for = |title: &'static str, cx: &mut VisualTestContext| {
        cx.simulate_mouse_down(button(RELOAD_X), MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(titles(&host), reload_titles());
        let enabled: Vec<bool> = host
            .menus
            .borrow()
            .last()
            .unwrap()
            .iter()
            .map(|item| item.1)
            .collect();
        assert_eq!(enabled, [true, true, false, true]);
        let keys: Vec<bool> = host
            .entries
            .borrow()
            .iter()
            .map(|entry| entry.shortcut.is_some())
            .collect();
        assert_eq!(keys, [true, true, false, false]);
        let title = crate::shell::MenuEntry::label(title);
        cx.update(|_, cx| host.choose(&title, cx));
        cx.run_until_parked();
        calls(&host)
    };
    assert_eq!(calls_for("Reload", cx), ["reload 10"]);
    assert_eq!(calls_for("Hard Reload", cx), ["reload 10 bypassing cache"]);
    assert_eq!(
        calls_for("Clear Site Data and Reload", cx),
        ["clear site data 10"]
    );
}

/// A page that isn't http(s) has no site whose data to clear: the menu still reloads it.
#[gpui::test]
fn a_page_with_no_site_dims_the_site_data(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    shell.update(cx, |shell, cx| {
        let page = Page {
            url: "file:///Users/someone/notes.html".into(),
            ..Page::default()
        };
        shell.set_page(10, page, cx);
    });
    cx.run_until_parked();
    host.take();
    cx.simulate_mouse_down(button(RELOAD_X), MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    let enabled: Vec<bool> = host
        .menus
        .borrow()
        .last()
        .unwrap()
        .iter()
        .map(|item| item.1)
        .collect();
    assert_eq!(enabled, [true, true, false, false]);
}

/// While the page loads, the button is stop, and its menu still reloads.
#[gpui::test]
fn the_stop_button_has_the_reload_menu_too(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    shell.update(cx, |shell, cx| {
        let page = Page {
            url: "https://example.org/c".into(),
            loading: true,
            ..Page::default()
        };
        shell.set_page(10, page, cx);
    });
    cx.run_until_parked();
    host.take();
    cx.simulate_mouse_down(button(RELOAD_X), MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(titles(&host), reload_titles());
    let title = crate::shell::MenuEntry::label("Hard Reload");
    cx.update(|_, cx| host.choose(&title, cx));
    assert_eq!(calls(&host), ["reload 10 bypassing cache"]);
}

/// With no tab shown, as on a Space's empty card, the menu's items are dimmed, as the View menu's
/// reloads are.
#[gpui::test]
fn with_no_tab_shown_the_menu_is_dimmed(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    shell.update(cx, |shell, cx| shell.set_selected(0, cx));
    cx.run_until_parked();
    host.take();
    cx.simulate_mouse_down(button(RELOAD_X), MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    let menus = host.menus.borrow();
    let enabled: Vec<bool> = menus
        .last()
        .expect("a menu")
        .iter()
        .map(|item| item.1)
        .collect();
    assert_eq!(enabled, [false, false, false, false]);
}
