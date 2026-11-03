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
        shell.set_tabs(1, Vec::new(), vec![10, 11], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        let page = Page { url: "https://example.org/c".into(), can_go_back: true, ..Page::default() };
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
    vec![page(-1, "https://example.org/b", "B"), page(-2, "https://example.org/a", "")]
}

/// What the shell asked of the core, but the tooltips hovering the buttons asks for.
fn calls(host: &Recorder) -> Vec<String> {
    host.take().into_iter().filter(|call| !call.starts_with("tooltip")).collect()
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
