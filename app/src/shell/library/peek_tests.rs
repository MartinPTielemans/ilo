//! A click on an archived tab opens it in a link preview beside the Library, as Arc's archive
//! previews its tabs (#696): kept, it's restored and the Library closes; ⌘-click, or a page no
//! preview can show, restores it at once.

use std::rc::Rc;

use gpui::{AppContext as _, Entity, Modifiers, TestAppContext, VisualTestContext, point, px};

use super::{
    ARCHIVE_TOP, ARCHIVED_LEFT, ARCHIVED_PITCH, Archived, COLUMN_WIDTH, HEADING_BOTTOM,
    HEADING_HEIGHT, HEADING_TOP,
};
use crate::shell::recorder::Recorder;
use crate::shell::{Folders, Section, Shell};

fn tab(id: u64, url: &str) -> Archived {
    Archived {
        id,
        url: url.to_string().into(),
        title: format!("Tab {id}").into(),
        archived: 3000. - id as f64,
        little: false,
    }
}

/// A window on Space 1 showing tab 10, and the Library open on Archived Tabs at rest, with tab 7,
/// a web page, over tab 8, a file.
fn archive(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| {
        let space = (
            1,
            "Space 1".into(),
            None,
            0,
            "".into(),
            "".into(),
            "".into(),
        );
        shell.set_spaces(vec![space], cx);
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
        shell.section = Section::Archive;
        shell.set_library(true, cx);
        shell.library_motion = None;
        shell.sidebar_fade = None;
        let tabs = vec![
            tab(7, "https://7.example/a"),
            tab(8, "file:///Users/someone/notes.html"),
        ];
        shell.set_archive(tabs, cx);
    });
    cx.run_until_parked();
    host.take();
    (shell, host, cx)
}

/// The middle of the list's `row`th row, under its one heading.
fn row(row: usize) -> gpui::Point<gpui::Pixels> {
    let top = ARCHIVE_TOP + HEADING_TOP + HEADING_HEIGHT + HEADING_BOTTOM;
    point(
        px(COLUMN_WIDTH + ARCHIVED_LEFT + 60.),
        px(top + ARCHIVED_PITCH * (row as f32 + 0.5)),
    )
}

fn calls(host: &Recorder) -> Vec<String> {
    let calls = host.take().into_iter();
    calls.filter(|call| call.contains("archived")).collect()
}

fn library_open(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> bool {
    shell.read_with(cx, |shell, _| shell.library_open)
}

/// A click opens the archived tab in a preview over the tab shown, the Library staying, and
/// restores nothing: the preview's own keys and buttons close or keep it (link_preview's tests).
#[gpui::test]
fn a_click_on_an_archived_tab_opens_it_in_a_preview(cx: &mut TestAppContext) {
    let (shell, host, cx) = archive(cx);
    cx.simulate_click(row(0), Modifiers::none());
    assert_eq!(
        calls(&host),
        ["open archived 7 in preview over 10 https://7.example/a"]
    );
    assert!(library_open(&shell, cx));
    // Kept, it's the window's tab, and the Library closes as for a tab restored from its row.
    shell.update(cx, |shell, cx| shell.kept_from_preview(cx));
    cx.run_until_parked();
    assert!(!library_open(&shell, cx));
}

/// ⌘-click, Ctrl off the Mac, restores it at once, as a click did before, and closes the Library.
#[gpui::test]
fn a_secondary_click_restores_at_once(cx: &mut TestAppContext) {
    let (shell, host, cx) = archive(cx);
    let command = cx.update(|_, cx| Modifiers::secondary_key(cx));
    cx.simulate_click(row(0), command);
    assert_eq!(calls(&host), ["restore archived 7"]);
    assert!(!library_open(&shell, cx));
}

/// A page that isn't a web page has nothing to preview, so its tab comes back at once.
#[gpui::test]
fn a_page_no_preview_shows_restores_at_once(cx: &mut TestAppContext) {
    let (_shell, host, cx) = archive(cx);
    cx.simulate_click(row(1), Modifiers::none());
    assert_eq!(calls(&host), ["restore archived 8"]);
}

/// With no tab shown, there's no page for the preview to go over, so it comes back at once.
#[gpui::test]
fn with_no_tab_shown_it_restores_at_once(cx: &mut TestAppContext) {
    let (shell, host, cx) = archive(cx);
    shell.update(cx, |shell, cx| shell.set_selected(0, cx));
    cx.run_until_parked();
    host.take();
    cx.simulate_click(row(0), Modifiers::none());
    assert_eq!(calls(&host), ["restore archived 7"]);
}
