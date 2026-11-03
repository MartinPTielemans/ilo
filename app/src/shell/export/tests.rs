use std::collections::HashMap;
use std::rc::Rc;

use gpui::{AppContext, TestAppContext, point, px};

use super::file_name;
use crate::shell::{Command, Folders, Page, Shell, Split, recorder::Recorder};

/// The shell with Work (1), its pinned tabs, a folder and Today tabs, one a split view, and Home
/// (2), empty, Work shown.
fn work(cx: &mut TestAppContext) -> (Rc<Recorder>, gpui::Entity<Shell>) {
    let host = Rc::new(Recorder::default());
    host.pinned_urls.borrow_mut().extend([
        (10, "https://docs.example/a (draft)".to_string()),
        (12, "https://plan.example/".to_string()),
        (13, "https://notes.example/".to_string()),
    ]);
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "Work".into(), None, 0, "".into(), "".into(), "".into()),
                (2, "Home".into(), None, 0, "".into(), "".into(), "".into()),
            ],
            cx,
        );
        let folders = Folders {
            folders: HashMap::from([(50, ("Q3 [plans]".into(), false))]),
            parents: HashMap::from([(12, 50), (13, 50)]),
            ..Default::default()
        };
        let split = Split {
            tabs: vec![21, 22],
            ..Default::default()
        };
        shell.set_tabs(
            1,
            vec![10, 50, 12, 13],
            vec![20, 21, 22],
            vec![split],
            folders,
            cx,
        );
        shell.set_space(1, cx);
        for (tab, url) in [
            (20, "https://news.example/"),
            (21, "https://a.example/"),
            (22, "https://b.example/"),
        ] {
            shell.set_page(
                tab,
                Page {
                    url: url.into(),
                    ..Page::default()
                },
                cx,
            );
        }
        for (tab, title) in [
            (10, "Draft"),
            (12, "Plan"),
            (13, "Notes"),
            (20, "*News*"),
            (21, "A"),
            (22, "B"),
        ] {
            shell.set_title(tab, title.into(), cx);
        }
    });
    (host, shell)
}

/// The Space's menu saves it as Markdown named for it: its pinned rows, a folder holding its own
/// nested under it, then its Today tabs, each tab of a split view a link.
#[gpui::test]
fn the_space_menu_exports_its_tabs_as_markdown(cx: &mut TestAppContext) {
    let (host, shell) = work(cx);
    shell.update(cx, |shell, cx| {
        shell.space_menu(1, point(px(100.), px(112.)), cx)
    });
    cx.update(|cx| host.choose("Export as Markdown…", cx));
    let saved = host.saved_markdown.take();
    let markdown = "# Work\n\n## Pinned\n\n- [Draft](https://docs.example/a%20%28draft%29)\n\
                    - Q3 \\[plans\\]\n  - [Plan](https://plan.example/)\n  \
                    - [Notes](https://notes.example/)\n\n## Today\n\n\
                    - [\\*News\\*](https://news.example/)\n- [A](https://a.example/)\n\
                    - [B](https://b.example/)\n";
    assert_eq!(saved, [("Work.md".to_string(), markdown.to_string(), 6)]);
}

/// The command bar's Export Space as Markdown… saves the Space shown, and an empty one is its
/// name alone.
#[gpui::test]
fn the_command_exports_the_space_shown(cx: &mut TestAppContext) {
    let (host, shell) = work(cx);
    shell.update(cx, |shell, cx| {
        shell.set_space(2, cx);
        shell.run(Command::ExportSpace, "command bar", cx);
    });
    assert_eq!(
        host.saved_markdown.take(),
        [("Home.md".into(), "# Home\n".into(), 0)]
    );
}

/// A private window's Space is kept nowhere, so it has nothing to export.
#[gpui::test]
fn a_private_window_doesnt_export(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let shell = cx.new(|cx| Shell::private_window(browser, 7, "Private".into(), 0, cx));
    shell.update(cx, |shell, cx| {
        shell.run(Command::ExportSpace, "command bar", cx)
    });
    assert!(host.saved_markdown.take().is_empty());
}

/// A Space's file takes its name, without what a file's name can't hold anywhere.
#[test]
fn a_spaces_file_is_named_for_it() {
    assert_eq!(file_name("Work"), "Work.md");
    assert_eq!(file_name("Q3: a/b"), "Q3- a-b.md");
    assert_eq!(file_name(" .hidden "), "hidden.md");
    assert_eq!(file_name(""), "Space.md");
}

/// Settings' Export… saves each of the user's Spaces with its favorites and its pinned tabs,
/// folders holding theirs nested under them, as the Space's menu writes them, and leaves an
/// agent's Space and Today tabs out.
#[gpui::test]
fn settings_exports_every_space(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    host.pinned_urls.borrow_mut().extend([
        (30, "https://mail.example/".to_string()),
        (10, "https://docs.example/a (draft)".to_string()),
        (12, "https://plan.example/".to_string()),
        (13, "https://notes.example/".to_string()),
        (20, "https://agent.example/".to_string()),
    ]);
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "Work".into(), None, 0, "".into(), "".into(), "".into()),
                (
                    2,
                    "Research".into(),
                    None,
                    0,
                    "".into(),
                    "Claude Code".into(),
                    "".into(),
                ),
                (3, "Home".into(), None, 2, "".into(), "".into(), "".into()),
            ],
            cx,
        );
        shell.set_favorites(vec![(30, 1)], cx);
        let folders = Folders {
            folders: HashMap::from([(50, ("Q3 [plans]".into(), false))]),
            parents: HashMap::from([(12, 50), (13, 50)]),
            ..Default::default()
        };
        shell.set_tabs(1, vec![10, 50, 12, 13], vec![11], Vec::new(), folders, cx);
        shell.set_tabs(2, vec![20], Vec::new(), Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
        for (tab, title) in [(30, "Mail"), (10, "Draft"), (12, "Plan"), (13, "Notes")] {
            shell.set_title(tab, title.into(), cx);
        }
        shell.export_spaces(cx);
    });
    let markdown = "# Spaces\n\n## Work\n\n### Favorites\n\n- [Mail](https://mail.example/)\n\n\
                    ### Pinned\n\n- [Draft](https://docs.example/a%20%28draft%29)\n- Q3 \\[plans\\]\n  \
                    - [Plan](https://plan.example/)\n  - [Notes](https://notes.example/)\n\n## Home\n";
    assert_eq!(
        host.saved_markdown.take(),
        [("Spaces.md".into(), markdown.into(), 4)]
    );
}
