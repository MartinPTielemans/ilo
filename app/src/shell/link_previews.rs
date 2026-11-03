//! Which tabs' links to other sites open in a link preview, on a host that records what the shell
//! asks.

use std::rc::Rc;

use gpui::{AppContext, TestAppContext, VisualTestContext, point, px};

use crate::link_preview::Closed;
use crate::shell::{Command, Folders, Shell, recorder::Recorder};

/// A pinned tab and a favorite keep their own site, so their links to others preview. A Today
/// tab's go where they lead, and nothing previews in an agent's Space.
#[gpui::test]
fn pinned_tabs_and_favorites_preview_links(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (
                    1,
                    "Space 1".into(),
                    None,
                    0,
                    "".into(),
                    "".into(),
                    "".into(),
                ),
                (
                    2,
                    "Research".into(),
                    None,
                    0,
                    "".into(),
                    "Claude Code".into(),
                    "".into(),
                ),
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
        shell.set_spaces(
            vec![(
                1,
                "Space 1".into(),
                None,
                0,
                "".into(),
                "".into(),
                "".into(),
            )],
            cx,
        );
        shell.set_tabs(
            1,
            vec![10],
            vec![11, 12],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    assert!(
        shell.read_with(cx, |shell, _| shell.page_laid().is_some()),
        "no page laid out"
    );
    let link = point(px(400.), px(200.));
    let closed = Closed {
        over: 10,
        url: "https://example.org/a".into(),
        link,
    };
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
    assert_eq!(
        run(Command::Undo, cx),
        ["reopen preview over 10 https://example.org/a"]
    );
    assert_eq!(run(Command::Redo, cx), ["close preview over 10"]);
    // Redo's close tells the shell too, which the history has already.
    shell.update(cx, |shell, cx| {
        shell.preview_closed(closed.clone(), true, cx)
    });
    assert_eq!(
        run(Command::ReopenClosedTab, cx),
        ["reopen preview over 10 https://example.org/a"]
    );
    assert_eq!(run(Command::Undo, cx), Vec::<String>::new());
    close(cx);
    shell.update(cx, |shell, cx| shell.close_tab(12, false, cx));
    host.take();
    assert_eq!(run(Command::ReopenClosedTab, cx), ["reopen closed tab"]);
}
