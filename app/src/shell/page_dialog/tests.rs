//! A page's dialog goes over its tab's card while the tab shows, and its answer reaches the core
//! without what was typed in any log, on a host that records what the shell asks of the core.

use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::{DialogKind, PageDialog, PageReply};
use crate::shell::{Folders, Page, Shell, Split, recorder::Recorder};

/// A window on one Space of tabs 10 and 20, showing 10, drawn.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![(1, "Space".into(), None, 1, "".into(), "".into(), "".into())],
            cx,
        );
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 20],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    host.take();
    (shell, cx, host)
}

fn prompt(id: u64) -> PageDialog {
    PageDialog {
        id,
        kind: DialogKind::Prompt,
        title: "example.org says".into(),
        message: "Your name?".into(),
        text: "".into(),
        accept: "OK".into(),
        cancel: "Cancel".into(),
        check: "".into(),
        username: "".into(),
        password: "".into(),
    }
}

/// What the shell asked about page dialogs since the last time.
fn dialogs(host: &Recorder) -> Vec<String> {
    host.take()
        .into_iter()
        .filter(|call| call.contains("dialog"))
        .collect()
}

fn ask(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64, dialog: Option<PageDialog>) {
    shell.update(cx, |shell, cx| shell.set_page_dialog(tab, dialog, cx));
    cx.run_until_parked();
}

fn select(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64) {
    shell.update(cx, |shell, cx| shell.set_selected(tab, cx));
    cx.run_until_parked();
}

#[gpui::test]
fn the_dialog_follows_its_tab(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    ask(&shell, cx, 10, Some(prompt(1)));
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
    // Another tab's waits while its tab is hidden.
    ask(&shell, cx, 20, Some(prompt(2)));
    assert!(dialogs(&host).is_empty());
    select(&shell, cx, 20);
    assert_eq!(dialogs(&host), ["page dialog 20 2"]);
    select(&shell, cx, 10);
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
    // The page took it back, such as when it went to another page.
    ask(&shell, cx, 10, None);
    assert_eq!(dialogs(&host), ["page dialog none"]);
    select(&shell, cx, 20);
    assert_eq!(dialogs(&host), ["page dialog 20 2"]);
}

#[gpui::test]
fn a_window_coming_to_the_front_asks_again(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    ask(&shell, cx, 10, Some(prompt(1)));
    dialogs(&host);
    shell.update(cx, |shell, cx| shell.became_key(cx));
    cx.run_until_parked();
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
}

#[gpui::test]
fn the_answer_reaches_the_core_once(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    ask(&shell, cx, 10, Some(prompt(1)));
    dialogs(&host);
    let reply = || PageReply {
        accept: true,
        text: "Ada".into(),
        password: "".into(),
        checked: false,
    };
    shell.update(cx, |shell, cx| shell.answer_page_dialog(10, 1, reply(), cx));
    cx.run_until_parked();
    // The dialog has left already, so there's nothing to take back.
    assert_eq!(dialogs(&host), ["answer dialog 10 1 accept"]);
    assert_eq!(host.replies.borrow()[0].text, "Ada");
    // An answer to a dialog no longer asked, such as one the page took back, goes nowhere.
    shell.update(cx, |shell, cx| shell.answer_page_dialog(10, 1, reply(), cx));
    cx.run_until_parked();
    assert!(dialogs(&host).is_empty());
}

#[gpui::test]
fn the_password_never_reaches_a_log(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    ask(&shell, cx, 10, Some(prompt(1)));
    let reply = PageReply {
        accept: true,
        text: "ada".into(),
        password: "hunter2".into(),
        checked: true,
    };
    assert_eq!(format!("{reply:?}"), "accept, checked");
    shell.update(cx, |shell, cx| shell.answer_page_dialog(10, 1, reply, cx));
    cx.run_until_parked();
    let calls = host.take();
    assert!(
        calls
            .iter()
            .all(|call| !call.contains("hunter2") && !call.contains("ada")),
        "{calls:?}"
    );
    // The core still gets it.
    assert_eq!(host.replies.borrow()[0].password, "hunter2");
}

#[gpui::test]
fn a_tab_in_safaris_engine_keeps_chromiums_dialog_back(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    ask(&shell, cx, 10, Some(prompt(1)));
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
    let engine = |shell: &Entity<Shell>, cx: &mut VisualTestContext, system_engine: bool| {
        shell.update(cx, |shell, cx| {
            let page = Page {
                system_engine,
                ..shell.page(10)
            };
            shell.set_page(10, page, cx);
        });
        cx.run_until_parked();
    };
    engine(&shell, cx, true);
    assert_eq!(dialogs(&host), ["page dialog none"]);
    engine(&shell, cx, false);
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
}

#[gpui::test]
fn a_window_coming_to_the_front_takes_back_anothers_dialog(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    // Another window's page asked, and that window closed with its tab before it drew again, so
    // the dialog over it stayed and took every shortcut after from this one (#3108).
    shell.update(cx, |shell, cx| shell.became_key(cx));
    cx.run_until_parked();
    assert_eq!(dialogs(&host), ["page dialog none"]);
}

#[gpui::test]
fn in_a_split_the_dialog_covers_its_own_pane(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    shell.update(cx, |shell, cx| {
        let split = Split {
            tabs: vec![10, 20],
            ..Default::default()
        };
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 20],
            vec![split],
            Folders::default(),
            cx,
        );
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    host.take();
    ask(&shell, cx, 10, Some(prompt(1)));
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
    let (laid, over) = shell.read_with(cx, |shell, _| {
        let over = shell
            .page_dialog_shown
            .flatten()
            .expect("no dialog shown")
            .2;
        (shell.page_laid.expect("no page laid out"), over)
    });
    // The left pane alone, so the page beside, on the right, stays usable (#3231).
    assert_eq!(over.0.left(), laid.0.left());
    assert_eq!(
        (over.0.top(), over.0.bottom()),
        (laid.0.top(), laid.0.bottom())
    );
    assert!(over.0.right() < laid.0.center().x, "{over:?} in {laid:?}");
    // A click on the page beside focuses its pane, and the dialog stays over its own, at rest,
    // where the page beside keeps the keyboard (#3461).
    select(&shell, cx, 20);
    assert_eq!(dialogs(&host), ["page dialog 10 1 at rest"]);
    let rests_over = shell.read_with(cx, |shell, _| shell.page_dialog_shown.flatten().unwrap().2);
    assert_eq!(rests_over, over);
    // A click on it at rest focuses its pane, and once the core shows that, it's live again.
    shell.update(cx, |shell, cx| shell.focus_dialog_pane(10, cx));
    cx.run_until_parked();
    assert!(host.take().contains(&"select 10".to_string()));
    select(&shell, cx, 10);
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
    // Live, its pane is focused already.
    shell.update(cx, |shell, cx| shell.focus_dialog_pane(10, cx));
    cx.run_until_parked();
    assert!(host.take().is_empty());
}

#[gpui::test]
fn in_a_split_the_focused_panes_dialog_asks_first(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    shell.update(cx, |shell, cx| {
        let split = Split {
            tabs: vec![10, 20],
            ..Default::default()
        };
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 20, 30],
            vec![split],
            Folders::default(),
            cx,
        );
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    host.take();
    // A tab outside the split waits for its tab to show, as anywhere.
    ask(&shell, cx, 30, Some(prompt(3)));
    assert_eq!(dialogs(&host), Vec::<String>::new());
    // The pane beside asks at rest while the one the user is in asks nothing.
    ask(&shell, cx, 20, Some(prompt(2)));
    assert_eq!(dialogs(&host), ["page dialog 20 2 at rest"]);
    // The user's pane asking comes first, live; the other's waits.
    ask(&shell, cx, 10, Some(prompt(1)));
    assert_eq!(dialogs(&host), ["page dialog 10 1"]);
    ask(&shell, cx, 10, None);
    assert_eq!(dialogs(&host), ["page dialog 20 2 at rest"]);
    // Its pane focused by keys, it's live.
    shell.update(cx, |shell, cx| shell.focus_pane(true, cx));
    cx.run_until_parked();
    assert!(host.take().contains(&"select 20".to_string()));
    select(&shell, cx, 20);
    assert_eq!(dialogs(&host), ["page dialog 20 2"]);
}
