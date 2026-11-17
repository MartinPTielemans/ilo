//! A Board's name stays out of the log (#3094): a window on a host that records what the shell
//! asks of the core, with the core's reports of a tab's page and title played as the app's
//! callbacks take them (tabs.rs), in each order they can come, and the lines written read back.

use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::recorder::Recorder;
use super::{Folders, Page, Shell};
use crate::title_log::{self, A_BOARD};

const NAME: &str = "kitchen shelves";
const BOARD: &str = "chrome://board/0123456789abcdef0123456789abcdef";
const OTHER_BOARD: &str = "chrome://board/fedcba9876543210fedcba9876543210";

/// A window showing Space 1, with `today` as its Today tabs and none shown.
fn shell<'a>(
    cx: &'a mut TestAppContext,
    today: &[u64],
) -> (Entity<Shell>, &'a mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            today.to_vec(),
            Vec::new(),
            Folders::default(),
            cx,
        );
    });
    cx.run_until_parked();
    title_log::take_written();
    (shell, cx)
}

/// The core reports `tab`'s page at `url`, as tab_state_changed takes it.
fn page(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64, url: &str) {
    title_log::page(tab, url);
    let page = Page {
        url: url.to_string().into(),
        ..Page::default()
    };
    shell.update(cx, |shell, cx| shell.set_page(tab, page, cx));
    cx.run_until_parked();
}

/// The core reports `tab`'s title, as tab_title_changed takes it.
fn title(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64, title: &str) {
    title_log::title(tab, title);
    let title = title.to_string().into();
    shell.update(cx, |shell, cx| shell.set_title(tab, title, cx));
    cx.run_until_parked();
}

fn show(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64) {
    shell.update(cx, |shell, cx| shell.set_selected(tab, cx));
    cx.run_until_parked();
}

/// The lines written since the last read, none of them with the Board's name.
fn written() -> Vec<String> {
    let written = title_log::take_written();
    for line in &written {
        assert!(!line.contains(NAME), "a line says the Board's name: {line}");
    }
    written
}

fn titled(tab: u64, title: &str) -> String {
    format!("browser: title {tab} {title}")
}

fn window(title: &str) -> String {
    format!("browser: window title {title}")
}

#[gpui::test]
fn a_new_board_named_while_open_is_logged_as_a_board(cx: &mut TestAppContext) {
    let (shell, cx) = shell(cx, &[5]);
    page(&shell, cx, 5, BOARD);
    show(&shell, cx, 5);
    title(&shell, cx, 5, "Board");
    title(&shell, cx, 5, NAME);
    let written = written();
    assert!(written.contains(&titled(5, A_BOARD)), "{written:?}");
    assert!(
        !written.iter().any(|line| line.ends_with(" Board")),
        "{written:?}"
    );
    assert_eq!(written.last(), Some(&window(A_BOARD)), "{written:?}");
}

#[gpui::test]
fn a_title_before_its_page_waits_for_the_page(cx: &mut TestAppContext) {
    // A Board restored at launch, or reopened from the archive or Recently Closed: a new tab whose
    // name comes before its page, shown at once.
    let (shell, cx) = shell(cx, &[6]);
    title(&shell, cx, 6, NAME);
    show(&shell, cx, 6);
    assert_eq!(
        written(),
        Vec::<String>::new(),
        "a line came before the tab's page"
    );
    // An empty page tells nothing yet.
    page(&shell, cx, 6, "");
    assert_eq!(
        written(),
        Vec::<String>::new(),
        "a line came before the tab's page"
    );
    page(&shell, cx, 6, BOARD);
    assert_eq!(written().first(), Some(&titled(6, A_BOARD)));
    show(&shell, cx, 0);
    show(&shell, cx, 6);
    assert_eq!(written().last(), Some(&window(A_BOARD)));
}

#[gpui::test]
fn a_tab_that_leaves_a_board_logs_its_pages_but_not_the_name(cx: &mut TestAppContext) {
    let (shell, cx) = shell(cx, &[7]);
    page(&shell, cx, 7, BOARD);
    title(&shell, cx, 7, NAME);
    show(&shell, cx, 7);
    written();
    page(&shell, cx, 7, "https://example.com/");
    title(&shell, cx, 7, "Example Domain");
    assert!(written().contains(&titled(7, "Example Domain")));
    // Its name, which it keeps on a page, is still the Board's.
    title(&shell, cx, 7, NAME);
    assert!(written().contains(&titled(7, A_BOARD)));
    page(&shell, cx, 7, OTHER_BOARD);
    title(&shell, cx, 7, "Board");
    assert!(written().contains(&titled(7, A_BOARD)));
}

#[gpui::test]
fn a_page_renamed_keeps_its_line(cx: &mut TestAppContext) {
    let (shell, cx) = shell(cx, &[8]);
    page(&shell, cx, 8, "https://example.com/");
    title(&shell, cx, 8, "Example Domain");
    show(&shell, cx, 8);
    title(&shell, cx, 8, "my reading");
    let written = written();
    assert!(
        written.contains(&titled(8, "Example Domain")),
        "{written:?}"
    );
    assert!(written.contains(&titled(8, "my reading")), "{written:?}");
    assert_eq!(written.last(), Some(&window("my reading")), "{written:?}");
}

#[gpui::test]
fn a_closed_tab_is_known_again_only_by_its_page(cx: &mut TestAppContext) {
    let (shell, cx) = shell(cx, &[9]);
    page(&shell, cx, 9, BOARD);
    title(&shell, cx, 9, NAME);
    written();
    title_log::closed(9);
    title(&shell, cx, 9, NAME);
    assert_eq!(
        written(),
        Vec::<String>::new(),
        "a line came before the tab's page"
    );
    page(&shell, cx, 9, BOARD);
    assert_eq!(written(), vec![titled(9, A_BOARD)]);
}
