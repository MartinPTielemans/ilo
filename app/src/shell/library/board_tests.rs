//! Library › Boards (#3073), on a profile folder of the test's own.

use std::rc::Rc;

use gpui::{AppContext as _, Entity, Modifiers, MouseButton, TestAppContext, point, px};

use super::{
    ARCHIVE_TOP, ARCHIVED_LEFT, ARCHIVED_PITCH, COLUMN_WIDTH, HEADING_BOTTOM, HEADING_HEIGHT,
    HEADING_TOP, Section,
};
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

/// A Board's row's menu with the keyboard: ↓ from Boards' field rings the row, ⇧F10 opens its
/// menu, and Duplicate in it makes a copy, as from a right-click (#3526).
#[gpui::test]
fn a_boards_row_menu_opens_from_the_keyboard(cx: &mut TestAppContext) {
    let profile = std::env::temp_dir().join(format!("ilo-board-keys-{}", boards::new_id()));
    let id = "d".repeat(32);
    boards::make(&profile, &id).unwrap();
    std::fs::write(boards::dir(&profile).join(&id).join("board.json"), "{}").unwrap();
    let host = Rc::new(Recorder::default());
    (host.profile_folders.borrow_mut()).insert(1, profile.clone());
    let browser = host.clone();
    cx.update(|cx| {
        crate::shell::commands::bind_keys(cx);
        crate::text_input::bind_keys(cx);
    });
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Default::default(), cx);
        shell.set_space(1, cx);
        shell.run(Command::ViewBoards, "test", cx);
        shell.library_motion = None;
        shell.sidebar_fade = None;
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("down");
    cx.run_until_parked();
    cx.simulate_keystrokes("shift-f10");
    cx.run_until_parked();
    let menu = host.menus.borrow().last().cloned().unwrap_or_default();
    let titles: Vec<String> = menu.iter().map(|(title, _)| title.to_string()).collect();
    assert_eq!(titles, ["Open", "Duplicate", "", "Delete"]);
    cx.update(|_, cx| host.choose("Duplicate", cx));
    cx.run_until_parked();
    let rows = listed(&shell, cx);
    assert!(
        rows.iter()
            .any(|(each, name)| *each != id && name == "Board copy"),
        "no Board copy listed: {rows:?}"
    );
    std::fs::remove_dir_all(&profile).unwrap();
}

/// A right-click on a Board's row offers Open, Duplicate and Delete. Duplicate opens a copy named
/// as one in a new tab and closes the Library, and Delete takes the Board out of the list. The
/// point is the first row's, under its day's heading, where tools/parity.sh right-clicks.
#[gpui::test]
fn a_boards_row_offers_open_duplicate_and_delete(cx: &mut TestAppContext) {
    let profile = std::env::temp_dir().join(format!("ilo-board-menu-{}", boards::new_id()));
    let id = "c".repeat(32);
    boards::make(&profile, &id).unwrap();
    std::fs::write(boards::dir(&profile).join(&id).join("board.json"), "{}").unwrap();
    let host = Rc::new(Recorder::default());
    (host.profile_folders.borrow_mut()).insert(1, profile.clone());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Default::default(), cx);
        shell.set_space(1, cx);
        shell.run(Command::ViewBoards, "test", cx);
        shell.library_motion = None;
        shell.sidebar_fade = None;
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();

    let top = ARCHIVE_TOP + HEADING_TOP + HEADING_HEIGHT + HEADING_BOTTOM + ARCHIVED_PITCH / 2.;
    let at = point(px(COLUMN_WIDTH + ARCHIVED_LEFT + 60.), px(top));
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    let menu = host.menus.borrow().last().cloned().unwrap_or_default();
    let titles: Vec<String> = menu.iter().map(|(title, _)| title.to_string()).collect();
    assert_eq!(titles, ["Open", "Duplicate", "", "Delete"], "at {at:?}");
    cx.update(|_, cx| host.choose("Duplicate", cx));
    cx.run_until_parked();
    let rows = listed(&shell, cx);
    let copy = (rows.iter())
        .find(|(each, _)| *each != id)
        .map(|(copy, name)| (copy.clone(), name.as_str()));
    let Some((copy, "Board copy")) = copy else {
        panic!("no Board copy listed: {rows:?}");
    };
    let open = format!("open {} false 0 0", boards::url(&copy));
    let calls = host.take();
    assert!(
        calls.contains(&open),
        "the copy didn't open in a new tab: {calls:?}"
    );
    assert!(
        !shell.read_with(cx, |shell, _| shell.library_open),
        "the Library stayed"
    );
    assert!(host.take_edits().is_empty(), "the copy is a ⌘Z step");

    shell.update(cx, |shell, cx| shell.run(Command::ViewBoards, "test", cx));
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    shell.update(cx, |shell, cx| shell.delete_board(id.clone(), cx));
    cx.run_until_parked();
    assert!(!boards::dir(&profile).join(&id).exists());
    assert_eq!(listed(&shell, cx), [(copy, "Board copy".into())]);
    std::fs::remove_dir_all(&profile).unwrap();
}

/// Over a Board, the bar offers Duplicate Board, first for "dup", and not Duplicate Current Tab,
/// whose tab would only say the Board is open in another tab; over a page it's the other way
/// round. Duplicate Board copies the Board shown. A private window offers no Board command.
#[gpui::test]
fn the_bar_duplicates_the_board_shown(cx: &mut TestAppContext) {
    let profile = std::env::temp_dir().join(format!("ilo-board-copy-{}", boards::new_id()));
    let id = "e".repeat(32);
    boards::make(&profile, &id).unwrap();
    boards::write_name(&profile, &id, "Trip").unwrap();
    let host = Rc::new(Recorder::default());
    (host.profile_folders.borrow_mut()).insert(1, profile.clone());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, _| window.activate_window());
    let over = |url: &str, cx: &mut gpui::VisualTestContext| {
        shell.update(cx, |shell, cx| {
            let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
            shell.set_spaces(vec![space], cx);
            shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Default::default(), cx);
            shell.set_space(1, cx);
            let page = Page {
                url: url.into(),
                ..Page::default()
            };
            shell.set_page(10, page, cx);
            shell.set_selected(10, cx);
        });
        let found = cx.update(|_, cx| crate::shell::bar_commands("dup", cx));
        (found.iter().map(|row| row.command))
            .filter(|each| matches!(each, Command::DuplicateBoard | Command::DuplicateTab))
            .collect::<Vec<_>>()
    };
    assert_eq!(over("https://example.com/", cx), [Command::DuplicateTab]);
    assert_eq!(over(&boards::url(&id), cx), [Command::DuplicateBoard]);
    let first = cx.update(|_, cx| crate::shell::bar_commands("dup", cx));
    assert_eq!(
        first.first().map(|row| row.command),
        Some(Command::DuplicateBoard)
    );
    shell.update(cx, |shell, cx| {
        shell.run(Command::DuplicateBoard, "keys", cx)
    });
    cx.run_until_parked();
    let mut names: Vec<String> = (boards::list(&profile).into_iter())
        .map(|board| board.name)
        .collect();
    names.sort();
    assert_eq!(names, ["Trip", "Trip copy"]);

    let browser: Rc<dyn Browser> = Rc::new(Recorder::default());
    let private = cx.new(|cx| Shell::private_window(browser, 1, "Incognito".into(), 1, cx));
    private.update(cx, |shell, cx| {
        let page = Page {
            url: boards::url(&id).into(),
            ..Page::default()
        };
        shell.set_page(10, page, cx);
        assert!(!shell.offers(Command::DuplicateBoard));
    });
    std::fs::remove_dir_all(&profile).unwrap();
}

/// Boards takes its place in the column's drawn order, after Spaces, once the profile has
/// boards to keep.
#[gpui::test]
fn boards_follow_spaces_in_the_column(cx: &mut TestAppContext) {
    let profile = std::env::temp_dir().join(format!("ilo-board-order-{}", boards::new_id()));
    let (shell, _) = window(cx, &profile);
    let shown = shell.read_with(cx, |shell, _| shell.shown_sections());
    let order: Vec<_> = (Section::ORDER.into_iter())
        .filter(|section| shown.contains(section))
        .collect();
    assert_eq!(shown, order);
    assert!(shown.contains(&Section::Boards), "{shown:?}");
    if let Some(spaces) = shown.iter().position(|&each| each == Section::Spaces) {
        let boards = shown.iter().position(|&each| each == Section::Boards);
        assert_eq!(boards, Some(spaces + 1), "{shown:?}");
    }
}
