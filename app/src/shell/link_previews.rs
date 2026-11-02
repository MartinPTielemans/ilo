//! Which tabs' links to other sites open in a link preview, on a host that records what the shell
//! asks.

use std::rc::Rc;

use gpui::{AppContext, Entity, Pixels, Point, TestAppContext, VisualTestContext, point, px};

use crate::link_preview::Closed;
use crate::shell::{Command, Folders, Shell, page_menu::made_up, recorder::Recorder};

/// A pinned tab and a favorite keep their own site, so their links to others preview. A Today
/// tab's go where they lead, and nothing previews in an agent's Space.
#[gpui::test]
fn pinned_tabs_and_favorites_preview_links(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "Space 1".into(), None, 0, "".into(), "".into(), "".into()),
                (2, "Research".into(), None, 0, "".into(), "Claude Code".into(), "".into()),
            ],
            cx,
        );
        shell.set_favorites(vec![(30, 0)], cx);
        shell.set_tabs(1, vec![10], vec![11], Vec::new(), Folders::default(), cx);
        shell.set_tabs(2, vec![20], vec![21], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
    });
    shell.read_with(cx, |shell, _| {
        assert!(shell.previews_links_of(10));
        assert!(shell.previews_links_of(30));
        assert!(!shell.previews_links_of(11));
        // A tab of a Space not shown isn't in this window.
        assert!(!shell.previews_links_of(20));
    });
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    shell.read_with(cx, |shell, _| {
        assert!(!shell.previews_links_of(20));
        assert!(!shell.previews_links_of(30));
    });
}

/// A closed preview comes back over its tab with ⌘Z, and ⇧⌘Z closes it again. ⇧⌘T opens it while
/// no tab has closed since, which leaves ⌘Z nothing of it, and a tab closed after it comes first.
#[gpui::test]
fn a_closed_preview_comes_back_with_undo_and_reopen(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(vec![(1, "Space 1".into(), None, 0, "".into(), "".into(), "".into())], cx);
        shell.set_tabs(1, vec![10], vec![11, 12], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.page_laid().is_some()), "no page laid out");
    let link = point(px(400.), px(200.));
    let closed = Closed { over: 10, url: "https://example.org/a".into(), link };
    let close = |cx: &mut VisualTestContext| {
        let closed = closed.clone();
        shell.update(cx, |shell, cx| shell.preview_closed(closed, false, cx));
    };
    let run = |command, cx: &mut VisualTestContext| {
        shell.update(cx, |shell, cx| shell.run(command, "ui", cx));
        host.take()
    };
    host.take();
    close(cx);
    assert_eq!(run(Command::Undo, cx), ["open preview over 10 https://example.org/a from 400,200"]);
    assert_eq!(run(Command::Redo, cx), ["close preview over 10"]);
    // Redo's close tells the shell too, which the history has already.
    shell.update(cx, |shell, cx| shell.preview_closed(closed.clone(), true, cx));
    let reopened = run(Command::ReopenClosedTab, cx);
    assert_eq!(reopened, ["open preview over 10 https://example.org/a from 400,200"]);
    assert_eq!(run(Command::Undo, cx), Vec::<String>::new());
    close(cx);
    shell.update(cx, |shell, cx| shell.close_tab(12, false, cx));
    host.take();
    assert_eq!(run(Command::ReopenClosedTab, cx), ["reopen closed tab"]);
}

/// A window showing the user's Space 1, with a pinned tab 10 and Today tabs 11 and 12, 11 shown,
/// beside an agent's Space 2 with a Today tab 20.
fn window_showing_today_tab(
    cx: &mut TestAppContext,
) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "Space 1".into(), None, 0, "".into(), "".into(), "".into()),
                (2, "Research".into(), None, 0, "".into(), "Claude Code".into(), "".into()),
            ],
            cx,
        );
        shell.set_tabs(1, vec![10], vec![11, 12], Vec::new(), Folders::default(), cx);
        shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
        shell.set_selected(11, cx);
    });
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.page_laid().is_some()), "no page laid out");
    host.take();
    (shell, host, cx)
}

const CALL: &str = "https://meet.google.com/abc-defg-hij";

/// The core asks, as for a ⇧-click, for the call's link at `at` in `tab`'s page.
fn ask(shell: &Entity<Shell>, tab: u64, at: Point<Pixels>, cx: &mut VisualTestContext) -> bool {
    shell.update(cx, |shell, cx| shell.preview_link(tab, CALL.into(), at, cx))
}

/// The titles of the last menu shown.
fn menu(host: &Recorder) -> Vec<String> {
    let menus = host.menus.borrow();
    menus.last().expect("a menu").iter().map(|(title, _)| title.to_string()).collect()
}

/// A link ⇧-clicked in any tab's page, here a Today tab's to a call, opens in a link preview over
/// the window, growing from the link: link rules and meeting links route only a plain click's. A
/// page the window doesn't show, or an agent's, gets none, and the core opens the link as it would.
#[gpui::test]
fn a_link_asked_for_opens_in_a_preview_from_any_tab(cx: &mut TestAppContext) {
    let (shell, host, cx) = window_showing_today_tab(cx);
    let at = point(px(40.), px(60.));
    let (page, _) = shell.read_with(cx, |shell, _| shell.page_laid()).unwrap();
    let link = page.origin + at;
    assert!(ask(&shell, 11, at, cx));
    let (x, y) = (f32::from(link.x), f32::from(link.y));
    assert_eq!(host.take(), [format!("open preview over 11 {CALL} from {x},{y}")]);
    assert!(!ask(&shell, 12, at, cx), "a tab the window doesn't show");
    shell.update(cx, |shell, cx| {
        shell.set_space(2, cx);
        shell.set_selected(20, cx);
    });
    assert!(shell.read_with(cx, |shell, _| shell.shows(20)));
    host.take();
    assert!(!ask(&shell, 20, at, cx), "an agent's tab");
    assert_eq!(host.take(), Vec::<String>::new());
}

/// A link's menu has Open Link in Peek after Open Link in New Tab, which runs the core's row; the
/// core then asks for the preview as for a ⇧-click. An agent's page's menu has none.
#[gpui::test]
fn a_links_menu_opens_it_in_a_preview(cx: &mut TestAppContext) {
    let (shell, host, cx) = window_showing_today_tab(cx);
    let at = point(px(300.), px(200.));
    shell.update(cx, |shell, cx| shell.show_page_menu(11, made_up("link", true), at, cx));
    let titles = menu(&host);
    let first = ["Open Link in New Tab", "Open Link in Peek", "Open Link in New Window"];
    assert_eq!(titles[..3], first);
    cx.update(|_, cx| host.choose("Open Link in Peek", cx));
    assert_eq!(host.take(), ["page menu 11 runs -1"]);
    shell.update(cx, |shell, cx| {
        shell.set_space(2, cx);
        shell.set_selected(20, cx);
        shell.show_page_menu(20, made_up("link", true), at, cx);
    });
    assert!(!menu(&host).contains(&"Open Link in Peek".to_string()));
}

/// With link previews turned off in Settings, a ⇧-click gets none and goes as Chrome's would, and
/// a link's menu has no Open Link in Peek, its other rows still running their own.
#[gpui::test]
fn with_previews_off_no_link_is_asked_into_one(cx: &mut TestAppContext) {
    let (shell, host, cx) = window_showing_today_tab(cx);
    host.previews_off.set(true);
    let at = point(px(40.), px(60.));
    assert!(!ask(&shell, 11, at, cx));
    assert_eq!(host.take(), Vec::<String>::new());
    shell.update(cx, |shell, cx| shell.show_page_menu(11, made_up("link", true), at, cx));
    let titles = menu(&host);
    assert_eq!(titles[..2], ["Open Link in New Tab", "Open Link in New Window"]);
    cx.update(|_, cx| host.choose("Open Link in New Window", cx));
    assert_eq!(host.take(), ["page menu 11 runs 50101"]);
}
