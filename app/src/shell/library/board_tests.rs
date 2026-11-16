//! Library › Boards (#3073), on a profile folder of the test's own.

use std::rc::Rc;

use gpui::{AppContext as _, Entity, TestAppContext};

use super::Section;
use crate::boards;
use crate::shell::recorder::Recorder;
use crate::shell::undo::{Prop, Value};
use crate::shell::{Browser, Command, Page, Shell};

fn window(cx: &mut TestAppContext, profile: &std::path::Path) -> (Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    (host.profile_folders.borrow_mut()).insert(1, profile.to_owned());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Default::default(), cx);
        shell.set_space(1, cx);
    });
    (shell, host)
}

fn listed(shell: &Entity<Shell>, cx: &mut TestAppContext) -> Vec<(String, String)> {
    shell.read_with(cx, |shell, _| {
        (shell.board_list.iter())
            .map(|board| (board.id.clone(), board.name.clone()))
            .collect()
    })
}

/// View Boards… lists the profile's Boards, the last changed first. Renaming a Board's tab names
/// its row. Delete closes the window's tab of it and takes it out of the list, into the folder ⌘Z
/// brings it back from, and ⌘Z brings it back, and then its tab.
#[gpui::test]
fn boards_list_rename_and_delete_with_undo(cx: &mut TestAppContext) {
    let profile = std::env::temp_dir().join(format!("ilo-board-list-{}", boards::new_id()));
    let (old, new) = ("a".repeat(32), "b".repeat(32));
    boards::make(&profile, &old).unwrap();
    std::fs::write(boards::dir(&profile).join(&old).join("board.json"), "{}").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    boards::make(&profile, &new).unwrap();
    std::fs::write(boards::dir(&profile).join(&new).join("board.json"), "{}").unwrap();

    let (shell, host) = window(cx, &profile);
    shell.update(cx, |shell, cx| {
        let url = boards::url(&new).into();
        let page = Page {
            url,
            ..Page::default()
        };
        shell.set_page(10, page, cx);
        assert!(shell.offers(Command::ViewBoards));
        shell.run(Command::ViewBoards, "ui", cx);
    });
    cx.run_until_parked();
    let shown = shell.read_with(cx, |shell, _| shell.library_open.then_some(shell.section));
    assert_eq!(shown, Some(Section::Boards));
    assert_eq!(
        listed(&shell, cx),
        [(new.clone(), "Board".into()), (old.clone(), "Board".into())]
    );

    shell.update(cx, |shell, cx| {
        shell.change(Prop::TabName(10), Value::Text("Sketches".into()), cx)
    });
    cx.run_until_parked();
    assert_eq!(listed(&shell, cx)[0], (new.clone(), "Sketches".into()));

    host.take_edits();
    shell.update(cx, |shell, cx| shell.delete_board(new.clone(), cx));
    cx.run_until_parked();
    assert_eq!(host.take_edits(), ["close 10"]);
    assert!(!boards::dir(&profile).join(&new).exists());
    assert_eq!(listed(&shell, cx), [(old.clone(), "Board".into())]);

    shell.update(cx, |shell, cx| assert!(shell.undo(cx)));
    cx.run_until_parked();
    assert!(
        boards::dir(&profile).join(&new).is_dir(),
        "⌘Z didn't bring the Board back"
    );
    assert_eq!(listed(&shell, cx)[0], (new.clone(), "Sketches".into()));
    let reopened = host.take_edits();
    assert!(
        reopened
            .iter()
            .any(|edit| edit.starts_with("reopen 10 as ")),
        "⌘Z didn't reopen the Board's tab: {reopened:?}"
    );
    std::fs::remove_dir_all(&profile).unwrap();
}

/// A private window keeps no Boards: no Boards section, and View Boards… does nothing.
#[gpui::test]
fn a_private_window_has_no_boards(cx: &mut TestAppContext) {
    let browser: Rc<dyn Browser> = Rc::new(Recorder::default());
    let private = cx.new(|cx| Shell::private_window(browser, 1, "Incognito".into(), 1, cx));
    private.update(cx, |shell, cx| {
        assert!(!shell.has_section(Section::Boards));
        assert!(!shell.offers(Command::ViewBoards));
        shell.run(Command::ViewBoards, "ui", cx);
        assert!(!shell.library_open);
    });
}
