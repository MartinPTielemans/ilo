//! Copy links without tracking (#2351): on, every copy path and the share sheet leave out a link's
//! trackers and keep its content, a video's time too; off, each gives the address as it is.

use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext, point, px};

use super::recorder::Recorder;
use super::{Archived, Command, Folders, Page, Shell};

const TRACKED: &str = "https://www.youtube.com/watch?v=ID&si=X&t=42&utm_source=y";
const CLEAN: &str = "https://www.youtube.com/watch?v=ID&t=42";

/// A window with the Today tab 10 selected and the pinned tab 12 in the folder 20, both on
/// `TRACKED`, and an archived tab 30 on it.
fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    host.share_sheet.set(true);
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        let folders = Folders {
            folders: [(20, ("Videos".into(), true))].into(),
            parents: [(12, 20)].into(),
            ..Default::default()
        };
        shell.set_tabs(1, vec![20, 12], vec![10], Vec::new(), folders, cx);
        for tab in [10, 12] {
            let page = Page {
                url: TRACKED.into(),
                ..Page::default()
            };
            shell.set_page(tab, page, cx);
            shell.set_title(tab, "Video".into(), cx);
        }
        shell.set_selected(10, cx);
        let archived = Archived {
            id: 30,
            url: TRACKED.into(),
            title: "Video".into(),
            archived: 0.,
            little: false,
        };
        shell.set_archive(vec![archived], cx);
    });
    cx.run_until_parked();
    (shell, cx, host)
}

/// What each copy path gave, and the share sheet, in order.
fn every_copy(shell: &Entity<Shell>, cx: &mut VisualTestContext, host: &Recorder) -> Vec<String> {
    host.take();
    shell.update(cx, |shell, cx| {
        shell.run(Command::CopyUrl, "ui", cx);
        shell.run(Command::CopyMarkdownLink, "ui", cx);
        shell.copy_url_from_button(cx);
        shell.tab_menu(10, point(px(100.), px(200.)), cx);
    });
    cx.update(|_, cx| host.choose("Copy Link", cx));
    shell.update(cx, |shell, cx| {
        shell.chosen = vec![shell.row_of(10), shell.row_of(12)];
        shell.run(Command::CopyUrl, "ui", cx);
        shell.chosen.clear();
        shell.copy_folder_links(20, false, cx);
        shell.archived_menu(30, point(px(100.), px(200.)), cx);
    });
    cx.update(|_, cx| host.choose("Copy URL", cx));
    shell.update(cx, |shell, cx| shell.run(Command::Share, "ui", cx));
    let gave = host.take().into_iter();
    gave.filter(|call| call.starts_with("copy ") || call.starts_with("share sheet "))
        .collect()
}

fn every_path(url: &str) -> Vec<String> {
    vec![
        format!("copy {url}"),
        format!("copy [Video]({url})"),
        format!("copy {url}"),
        format!("copy {url}"),
        format!("copy {url}\n{url}"),
        format!("copy {url}"),
        format!("copy {url}"),
        format!("share sheet 10 {url} Video"),
    ]
}

#[gpui::test]
fn every_copy_path_follows_the_switch(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    assert_eq!(every_copy(&shell, cx, &host), every_path(CLEAN));
    shell.update(cx, |shell, cx| shell.set_clean_links(false, cx));
    assert_eq!(every_copy(&shell, cx, &host), every_path(TRACKED));
    shell.update(cx, |shell, cx| shell.set_clean_links(true, cx));
    assert_eq!(every_copy(&shell, cx, &host), every_path(CLEAN));
}

/// A window opened while the switch is off gives the address as it is from the start.
#[gpui::test]
fn a_window_opened_with_the_switch_off_follows_it(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_global(super::CleanLinksOn(false)));
    let (shell, cx, host) = shell(cx);
    assert_eq!(every_copy(&shell, cx, &host), every_path(TRACKED));
}
