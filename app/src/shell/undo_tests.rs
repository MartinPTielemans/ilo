//! ⌘Z and ⇧⌘Z on a host that records what the shell asks of the core: each action the user can take
//! that loses or moves something is undone, and done again, at the place it was, and the core's
//! reports are played by the test, as the core plays them a moment after each call.

use std::{collections::HashMap, rc::Rc};

use gpui::{App, AppContext, Entity, TestAppContext};

use crate::shell::{
    Archived, Command, Folders, Shell, Split,
    recorder::Recorder,
    undo::{Prop, Value},
};

/// Two Spaces in one profile, the first shown, with the given tabs in the first.
fn window(
    cx: &mut TestAppContext,
    pinned: Vec<u64>,
    today: Vec<u64>,
    folders: Folders,
    splits: Vec<Split>,
) -> (Entity<Shell>, Rc<Recorder>) {
    on(cx, Rc::new(Recorder::default()), pinned, today, folders, splits)
}

/// As `window`, on `host`.
fn on(
    cx: &mut TestAppContext,
    host: Rc<Recorder>,
    pinned: Vec<u64>,
    today: Vec<u64>,
    folders: Folders,
    splits: Vec<Split>,
) -> (Entity<Shell>, Rc<Recorder>) {
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "One".into(), None, 1, "".into(), "".into(), "".into()),
                (2, "Two".into(), None, 1, "".into(), "".into(), "".into()),
                (3, "Three".into(), None, 2, "".into(), "".into(), "".into()),
            ],
            cx,
        );
        shell.set_space(1, cx);
        shell.set_tabs(1, pinned, today, splits, folders, cx);
    });
    (shell, host)
}

fn plain(cx: &mut TestAppContext, today: Vec<u64>) -> (Entity<Shell>, Rc<Recorder>) {
    window(cx, Vec::new(), today, Folders::default(), Vec::new())
}

/// A second window over the same core as `first`, with its own Today tabs.
fn beside(
    cx: &mut TestAppContext,
    first: &Rc<Recorder>,
    today: Vec<u64>,
) -> (Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::beside(first));
    on(cx, host, Vec::new(), today, Folders::default(), Vec::new())
}

/// The core reports the first Space's tabs.
fn report(shell: &Entity<Shell>, cx: &mut TestAppContext, pinned: Vec<u64>, today: Vec<u64>) {
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, pinned, today, Vec::new(), Folders::default(), cx)
    });
}

fn undo(shell: &Entity<Shell>, cx: &mut TestAppContext) -> bool {
    shell.update(cx, |shell, cx| shell.undo(cx))
}

fn redo(shell: &Entity<Shell>, cx: &mut TestAppContext) -> bool {
    shell.update(cx, |shell, cx| shell.redo(cx))
}

fn titles(
    shell: &Entity<Shell>,
    cx: &mut TestAppContext,
) -> (Option<&'static str>, Option<&'static str>) {
    shell.read_with(cx, |shell, _| shell.history.titles())
}

#[gpui::test]
fn a_closed_tab_reopens_and_closes_again_under_its_new_id(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![10, 11, 12]);
    shell.update(cx, |shell, cx| shell.close_tab(11, false, cx));
    assert_eq!(host.take_edits(), ["close 11"]);
    report(&shell, cx, Vec::new(), vec![10, 12]);

    assert!(undo(&shell, cx));
    assert_eq!(host.take_edits(), ["reopen 11 as 1001"]);
    report(&shell, cx, Vec::new(), vec![10, 1001, 12]);

    // Done again, it's the tab that came back it closes.
    assert!(redo(&shell, cx));
    assert_eq!(host.take_edits(), ["close 1001"]);
    report(&shell, cx, Vec::new(), vec![10, 12]);
    assert!(undo(&shell, cx));
    assert_eq!(host.take_edits(), ["reopen 1001 as 1002"]);
}

/// The core reopens a tab behind the one shown, so ⌘Z brings the tabs back without showing them.
#[gpui::test]
fn tabs_closed_below_the_tab_shown_come_back_behind_it(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![1, 2, 3, 4]);
    shell.update(cx, |shell, cx| shell.set_selected(2, cx));
    shell.update(cx, |shell, cx| shell.close_tabs_below(2, cx));
    report(&shell, cx, Vec::new(), vec![1, 2]);
    host.take();
    assert!(undo(&shell, cx));
    assert!(!host.take().iter().any(|call| call.starts_with("select")));
    report(&shell, cx, Vec::new(), vec![1, 2, 1001, 1002]);
    assert!(redo(&shell, cx));
    assert!(undo(&shell, cx));
    assert!(!host.take().iter().any(|call| call.starts_with("select")));
}

#[gpui::test]
fn the_tab_shown_when_it_closed_is_shown_again_when_it_comes_back(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![10, 11, 12]);
    shell.update(cx, |shell, cx| shell.set_selected(11, cx));
    shell.update(cx, |shell, cx| shell.close_tab(11, false, cx));
    report(&shell, cx, Vec::new(), vec![10, 12]);
    shell.update(cx, |shell, cx| shell.set_selected(12, cx));
    host.take();
    assert!(undo(&shell, cx));
    assert!(host.take().contains(&"select 1001".to_string()));
    // Closed again while it's shown, it's shown again the next time too.
    report(&shell, cx, Vec::new(), vec![10, 1001, 12]);
    shell.update(cx, |shell, cx| shell.set_selected(1001, cx));
    assert!(redo(&shell, cx));
    report(&shell, cx, Vec::new(), vec![10, 12]);
    shell.update(cx, |shell, cx| shell.set_selected(12, cx));
    host.take();
    assert!(undo(&shell, cx));
    assert!(host.take().contains(&"select 1002".to_string()));
}

#[gpui::test]
fn the_edit_menu_names_what_it_would_undo_and_redo(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![10, 11]);
    assert_eq!(*host.edit_titles.borrow(), (None, None));
    shell.update(cx, |shell, cx| shell.close_tab(11, false, cx));
    assert_eq!(*host.edit_titles.borrow(), (Some("Close Tab".into()), None));
    report(&shell, cx, Vec::new(), vec![10]);
    undo(&shell, cx);
    assert_eq!(*host.edit_titles.borrow(), (None, Some("Close Tab".into())));
    report(&shell, cx, Vec::new(), vec![10, 1001]);
    redo(&shell, cx);
    assert_eq!(*host.edit_titles.borrow(), (Some("Close Tab".into()), None));
}

#[gpui::test]
fn closing_the_tabs_below_brings_back_the_top_one_first(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![1, 2, 3, 4]);
    shell.update(cx, |shell, cx| shell.close_tabs_below(2, cx));
    assert_eq!(host.take_edits(), ["close below 2"]);
    assert_eq!(host.undo_toasts.borrow().as_slice(), ["Tabs Closed"]);
    report(&shell, cx, Vec::new(), vec![1, 2]);
    undo(&shell, cx);
    assert_eq!(host.take_edits(), ["reopen 3 as 1001", "reopen 4 as 1002"]);
    report(&shell, cx, Vec::new(), vec![1, 2, 1001, 1002]);
    redo(&shell, cx);
    assert_eq!(host.take_edits(), ["close 1001", "close 1002"]);
}

#[gpui::test]
fn a_pinned_tab_keeps_its_row_so_closing_it_has_nothing_to_undo(cx: &mut TestAppContext) {
    let (shell, host) = window(cx, vec![5], vec![6], Folders::default(), Vec::new());
    shell.update(cx, |shell, cx| shell.close_tab(5, false, cx));
    host.take_edits();
    assert!(!undo(&shell, cx));
    assert!(host.take_edits().is_empty());
}

#[gpui::test]
fn closing_a_split_brings_all_its_tabs_back(cx: &mut TestAppContext) {
    let split = Split {
        tabs: vec![10, 11],
        ..Default::default()
    };
    let (shell, host) = window(
        cx,
        Vec::new(),
        vec![10, 11, 12],
        Folders::default(),
        vec![split],
    );
    shell.update(cx, |shell, cx| shell.close_tab(10, true, cx));
    // Both panes closed: the core keeps each with its partner, so each comes back.
    assert_eq!(host.take_edits(), ["close 10"]);
    report(&shell, cx, Vec::new(), vec![12]);
    undo(&shell, cx);
    assert_eq!(
        host.take_edits(),
        ["reopen 10 as 1001", "reopen 11 as 1002"]
    );
}

#[gpui::test]
fn removing_a_favorite_is_said_in_the_toast_and_undone_to_its_place_and_url(
    cx: &mut TestAppContext,
) {
    let (shell, host) = plain(cx, vec![30]);
    shell.update(cx, |shell, cx| {
        shell.set_favorites(vec![(20, 1), (21, 1), (22, 1)], cx)
    });
    host.pinned_urls
        .borrow_mut()
        .insert(21, "https://a.example/".into());
    shell.update(cx, |shell, cx| shell.move_tab_to(21, false, 0, 0, cx));
    assert_eq!(host.take_edits(), ["move 21 false 0 0"]);
    assert_eq!(
        host.undo_toasts.borrow().as_slice(),
        ["Removed from Favorites"]
    );
    assert_eq!(titles(&shell, cx).0, Some("Remove from Favorites"));

    // The core has made it the first Today tab, which forgot the URL.
    shell.update(cx, |shell, cx| {
        shell.set_favorites(vec![(20, 1), (22, 1)], cx)
    });
    report(&shell, cx, Vec::new(), vec![21, 30]);
    host.pinned_urls.borrow_mut().clear();
    assert!(undo(&shell, cx));
    assert_eq!(
        host.take_edits(),
        ["favorite 21 1", "pinned url 21 https://a.example/"]
    );

    shell.update(cx, |shell, cx| {
        shell.set_favorites(vec![(20, 1), (21, 1), (22, 1)], cx)
    });
    report(&shell, cx, Vec::new(), vec![30]);
    assert!(redo(&shell, cx));
    assert_eq!(host.take_edits(), ["move 21 false 0 0"]);
}

/// Under Every Space the core keeps a twin of each favorite in the other profiles, which sit
/// between the tiles the Space shown has; the core's Add makes them again, at the same place, as it
/// moves and removes them with the favorite.
#[gpui::test]
fn an_every_space_favorite_is_put_back_by_its_place_among_the_tiles_its_space_shows(
    cx: &mut TestAppContext,
) {
    let (shell, host) = plain(cx, vec![30]);
    // Spaces 1 and 2 are in profile 1; the twins 40 to 42 are profile 2's.
    let both = vec![(20, 1), (40, 2), (21, 1), (41, 2), (22, 1), (42, 2)];
    shell.update(cx, |shell, cx| shell.set_favorites(both.clone(), cx));
    assert_eq!(shell.read_with(cx, |shell, _| shell.favorites().to_vec()), [20, 21, 22]);
    host.pinned_urls
        .borrow_mut()
        .insert(21, "https://a.example/".into());
    shell.update(cx, |shell, cx| shell.move_tab_to(21, false, 0, 0, cx));
    assert_eq!(host.take_edits(), ["move 21 false 0 0"]);

    // The core closed 21's twin with it.
    shell.update(cx, |shell, cx| {
        shell.set_favorites(vec![(20, 1), (40, 2), (22, 1), (42, 2)], cx)
    });
    report(&shell, cx, Vec::new(), vec![21, 30]);
    host.pinned_urls.borrow_mut().clear();
    assert!(undo(&shell, cx));
    // Index 1 among 20, 22 and 21's own, not 2 among the twins too.
    assert_eq!(
        host.take_edits(),
        ["favorite 21 1", "pinned url 21 https://a.example/"]
    );

    // The core made the twin again, under a new id, in the same place.
    shell.update(cx, |shell, cx| {
        shell.set_favorites(vec![(20, 1), (40, 2), (21, 1), (43, 2), (22, 1), (42, 2)], cx)
    });
    report(&shell, cx, Vec::new(), vec![30]);
    assert_eq!(shell.read_with(cx, |shell, _| shell.favorites().to_vec()), [20, 21, 22]);
    assert!(redo(&shell, cx));
    assert_eq!(host.take_edits(), ["move 21 false 0 0"]);
}

#[gpui::test]
fn the_toasts_undo_button_does_what_command_z_does_and_goes(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![30]);
    shell.update(cx, |shell, cx| {
        shell.set_favorites(vec![(20, 1), (21, 1)], cx)
    });
    shell.update(cx, |shell, cx| shell.move_tab_to(21, false, 0, 0, cx));
    host.take_edits();
    shell.update(cx, |shell, cx| shell.set_favorites(vec![(20, 1)], cx));
    report(&shell, cx, Vec::new(), vec![21, 30]);

    let button = host
        .undo_toast
        .borrow()
        .clone()
        .expect("the toast offers Undo");
    cx.update(|cx: &mut App| button(cx));
    assert_eq!(host.take_edits(), ["favorite 21 1"]);
    assert!(
        host.undo_toast.borrow().is_none(),
        "the offer goes once it's taken"
    );
}

#[gpui::test]
fn the_next_thing_the_user_does_takes_the_offer_away(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![30, 31]);
    shell.update(cx, |shell, cx| {
        shell.set_favorites(vec![(20, 1), (21, 1)], cx)
    });
    shell.update(cx, |shell, cx| shell.move_tab_to(21, false, 0, 0, cx));
    assert!(host.undo_toast.borrow().is_some());
    // Its button would undo this instead, under words that name the other.
    shell.update(cx, |shell, cx| shell.close_tab(31, false, cx));
    assert!(host.undo_toast.borrow().is_none());
}

#[gpui::test]
fn pinning_and_unpinning_are_undone_to_the_same_row(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![9, 10, 11]);
    shell.update(cx, |shell, cx| shell.pin_tab(10, true, cx));
    assert_eq!(host.take_edits(), ["pinned 10 true"]);
    report(&shell, cx, vec![10], vec![9, 11]);
    undo(&shell, cx);
    assert_eq!(host.take_edits(), ["move 10 false 0 1"]);
    report(&shell, cx, Vec::new(), vec![9, 10, 11]);

    // Unpinned, it forgets the URL it went back to, which the undo gives it again.
    report(&shell, cx, vec![10], vec![9, 11]);
    host.pinned_urls
        .borrow_mut()
        .insert(10, "https://p.example/".into());
    shell.update(cx, |shell, cx| shell.pin_tab(10, false, cx));
    assert_eq!(host.take_edits(), ["pinned 10 false"]);
    report(&shell, cx, Vec::new(), vec![10, 9, 11]);
    host.pinned_urls.borrow_mut().clear();
    undo(&shell, cx);
    assert_eq!(
        host.take_edits(),
        ["move 10 true 0 0", "pinned url 10 https://p.example/"]
    );
}

#[gpui::test]
fn a_drag_is_undone_to_where_the_tab_was_in_its_folder(cx: &mut TestAppContext) {
    let folders = Folders {
        folders: HashMap::from([(5, ("Reading".into(), true))]),
        parents: HashMap::from([(6, 5), (7, 5), (8, 5)]),
        ..Default::default()
    };
    let (shell, host) = window(cx, vec![4, 5, 6, 7, 8], vec![9], folders, Vec::new());
    // 8 goes out of the folder to the top of Today.
    shell.update(cx, |shell, cx| shell.move_tab_to(8, false, 0, 0, cx));
    host.take_edits();
    report(&shell, cx, vec![4, 5, 6, 7], vec![8, 9]);
    undo(&shell, cx);
    // Third of the folder's, counted without the folder's own row.
    assert_eq!(host.take_edits(), ["move 8 true 5 2"]);
}

#[gpui::test]
fn moving_a_tab_to_a_space_of_the_same_profile_is_undone_but_not_to_another_profile(
    cx: &mut TestAppContext,
) {
    let (shell, host) = plain(cx, vec![10, 11]);
    shell.update(cx, |shell, cx| shell.send_to_space(11, 2, false, cx));
    assert_eq!(host.take_edits(), ["space 11 2"]);
    assert_eq!(host.undo_toasts.borrow().as_slice(), ["Tab Moved to Space"]);
    report(&shell, cx, Vec::new(), vec![10]);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(2, Vec::new(), vec![11], Vec::new(), Folders::default(), cx)
    });
    undo(&shell, cx);
    assert_eq!(host.take_edits(), ["space 11 1", "move 11 false 0 1"]);

    // Another profile's Space opens it there as a new tab, which has nothing to give back.
    let (shell, host) = plain(cx, vec![10, 11]);
    shell.update(cx, |shell, cx| shell.send_to_space(11, 3, false, cx));
    host.take_edits();
    assert!(!undo(&shell, cx));
    assert!(host.take_edits().is_empty());
}

#[gpui::test]
fn pin_to_space_pins_the_tab_before_it_moves_and_one_undo_puts_it_back_in_today(
    cx: &mut TestAppContext,
) {
    let (shell, host) = plain(cx, vec![10, 11]);
    shell.update(cx, |shell, cx| shell.send_to_space(11, 2, true, cx));
    assert_eq!(host.take_edits(), ["pinned 11 true", "space 11 2"]);
    report(&shell, cx, Vec::new(), vec![10]);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(2, vec![11], Vec::new(), Vec::new(), Folders::default(), cx)
    });
    undo(&shell, cx);
    assert_eq!(host.take_edits(), ["space 11 1", "move 11 false 0 1"]);

    // A pinned tab moves pinned already.
    let (shell, host) = window(cx, vec![11], vec![10], Folders::default(), Vec::new());
    shell.update(cx, |shell, cx| shell.send_to_space(11, 2, true, cx));
    assert_eq!(host.take_edits(), ["space 11 2"]);
}

/// New Space… makes the Space right after the one shown, as Arc's does, not after the others.
#[gpui::test]
fn a_new_space_goes_right_after_the_space_shown(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![10]);
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::NewSpace, "ui", cx));
    assert!(host.take().contains(&"create space Space 4 after 2".to_string()));
}

/// The command bar finds each other Space by its name, to focus it, pin the tab in it or move the
/// tab to it, Focus first as in Arc's, and the Space shown's closed folders, to open one with the
/// folders it's in.
#[gpui::test]
fn the_command_bar_finds_spaces_and_closed_folders_by_name(cx: &mut TestAppContext) {
    let folders = Folders {
        folders: HashMap::from([(5, ("Reading".into(), false)), (6, ("Recipes".into(), false))]),
        parents: HashMap::from([(6, 5), (7, 6)]),
        ..Default::default()
    };
    let (shell, host) = window(cx, vec![5, 6, 7], vec![10], folders, Vec::new());
    shell.update(cx, |shell, cx| shell.set_selected(10, cx));
    let found = |typed: &str, cx: &mut TestAppContext| {
        shell.read_with(cx, |shell, _| {
            let found = Vec::new();
            let keys = crate::shell::UserKeys::default();
            let rows = crate::shell::with_named_rows(found, typed, &shell.named_bar_rows(), &keys);
            rows.into_iter().map(|row| (row.name.to_string(), row.command)).collect::<Vec<_>>()
        })
    };
    assert_eq!(
        found("two", cx),
        [
            ("Focus Two".to_string(), Command::SelectSpace(1)),
            ("Pin to Two".to_string(), Command::PinToSpace(2)),
            ("Move to Two".to_string(), Command::MoveToSpace(2)),
        ]
    );
    // Nothing typed lists none, and the Space shown has none.
    assert!(found("", cx).is_empty());
    assert!(found("one", cx).iter().all(|(name, _)| !name.ends_with("One")));
    let recipes = found("recipes", cx);
    assert_eq!(recipes, [("Open Recipes".to_string(), Command::OpenFolder(6))]);
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::OpenFolder(6), "command-bar", cx));
    assert_eq!(host.take(), ["folder open 6 true", "folder open 5 true"]);
}

#[gpui::test]
fn a_deleted_folder_is_made_again_with_what_it_held_and_deleted_again_by_its_new_id(
    cx: &mut TestAppContext,
) {
    let folders = Folders {
        folders: HashMap::from([(5, ("Reading".into(), false))]),
        parents: HashMap::from([(6, 5), (7, 5)]),
        ..Default::default()
    };
    let (shell, host) = window(cx, vec![4, 5, 6, 7], Vec::new(), folders, Vec::new());
    shell.update(cx, |shell, cx| shell.delete_folder_now(5, cx));
    assert_eq!(host.take_edits(), ["delete folder 5"]);
    assert_eq!(host.undo_toasts.borrow().as_slice(), ["Folder Deleted"]);
    report(&shell, cx, vec![4, 6, 7], Vec::new());

    undo(&shell, cx);
    assert_eq!(
        host.take_edits(),
        [
            "folder 1001 1 Reading 0 1",
            "move 6 true 1001 0",
            "move 7 true 1001 1"
        ]
    );
    let folders = Folders {
        folders: HashMap::from([(1001, ("Reading".into(), true))]),
        parents: HashMap::from([(6, 1001), (7, 1001)]),
        ..Default::default()
    };
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, vec![4, 1001, 6, 7], Vec::new(), Vec::new(), folders, cx)
    });
    redo(&shell, cx);
    assert_eq!(host.take_edits(), ["delete folder 1001"]);
}

#[gpui::test]
fn leaving_a_split_is_undone_by_joining_the_tab_beside_it(cx: &mut TestAppContext) {
    let split = Split {
        tabs: vec![10, 11, 12],
        ..Default::default()
    };
    let (shell, host) = window(
        cx,
        Vec::new(),
        vec![10, 11, 12],
        Folders::default(),
        vec![split],
    );
    shell.update(cx, |shell, cx| shell.separate_tab(12, cx));
    assert_eq!(host.take_edits(), ["separate 12"]);
    report(&shell, cx, Vec::new(), vec![10, 11, 12]);
    undo(&shell, cx);
    assert_eq!(host.take_edits(), ["split 12 11 false"]);

    // The first one rejoins at the left of the one after it.
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11, 12],
            vec![Split {
                tabs: vec![10, 11],
                ..Default::default()
            }],
            Folders::default(),
            cx,
        )
    });
    shell.update(cx, |shell, cx| shell.separate_tab(10, cx));
    host.take_edits();
    report(&shell, cx, Vec::new(), vec![10, 11, 12]);
    undo(&shell, cx);
    assert_eq!(host.take_edits(), ["split 10 11 true"]);
}

/// The core reports the first Space's Today tabs with one split view.
fn report_split(
    shell: &Entity<Shell>,
    cx: &mut TestAppContext,
    today: Vec<u64>,
    tabs: Vec<u64>,
    stacked: bool,
) {
    let split = Split {
        tabs,
        stacked,
        ..Default::default()
    };
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), today, vec![split], Folders::default(), cx)
    });
}

#[gpui::test]
fn joining_a_split_is_undone_to_the_tabs_row_with_the_page_shown_and_done_again(
    cx: &mut TestAppContext,
) {
    let (shell, host) = plain(cx, vec![10, 11, 12]);
    shell.update(cx, |shell, cx| shell.join_split(12, 10, false, false, cx));
    assert_eq!(host.take_edits(), ["split 12 10 false"]);
    assert_eq!(titles(&shell, cx), (Some("Split View"), None));
    // The core moved 12 into 10's place, and the user clicked 10's page.
    report_split(&shell, cx, vec![10, 12, 11], vec![10, 12], false);
    shell.update(cx, |shell, cx| shell.set_selected(10, cx));
    host.take();

    assert!(undo(&shell, cx));
    assert_eq!(host.take_edits(), ["separate 12", "move 12 false 0 2"]);
    assert!(host.take().contains(&"select 10".to_string()));
    assert_eq!(titles(&shell, cx), (None, Some("Split View")));

    report(&shell, cx, Vec::new(), vec![10, 11, 12]);
    assert!(redo(&shell, cx));
    assert_eq!(host.take_edits(), ["split 12 10 false"]);
}

#[gpui::test]
fn a_tab_that_left_one_split_for_another_goes_back_to_the_first(cx: &mut TestAppContext) {
    let splits = vec![
        Split {
            tabs: vec![10, 11],
            ..Default::default()
        },
        Split {
            tabs: vec![12, 13],
            ..Default::default()
        },
    ];
    let (shell, host) = window(cx, Vec::new(), vec![10, 11, 12, 13], Folders::default(), splits);
    shell.update(cx, |shell, cx| shell.join_split(11, 12, true, false, cx));
    assert_eq!(host.take_edits(), ["split 11 12 true"]);
    report_split(&shell, cx, vec![10, 11, 12, 13], vec![11, 12, 13], false);
    assert!(undo(&shell, cx));
    // Joining 10 again takes it out of the other split: no separate first.
    assert_eq!(host.take_edits(), ["split 11 10 false"]);
}

#[gpui::test]
fn a_drop_that_stacks_a_new_split_is_one_step(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![10, 11]);
    shell.update(cx, |shell, cx| shell.join_split(11, 10, true, true, cx));
    assert_eq!(host.take_edits(), ["split 11 10 true"]);
    assert!(host.take().contains(&"split stacked 10 true".to_string()));
    report_split(&shell, cx, vec![11, 10], vec![11, 10], true);

    assert!(undo(&shell, cx));
    assert_eq!(host.take_edits(), ["separate 11", "move 11 false 0 1"]);
    report(&shell, cx, Vec::new(), vec![10, 11]);
    host.take();
    assert!(redo(&shell, cx));
    assert_eq!(host.take_edits(), ["split 11 10 true"]);
    assert!(host.take().contains(&"split stacked 10 true".to_string()));
}

#[gpui::test]
fn a_split_that_is_full_or_has_the_tab_keeps_nothing(cx: &mut TestAppContext) {
    let split = Split {
        tabs: vec![10, 11, 12],
        ..Default::default()
    };
    let today = vec![10, 11, 12, 13];
    let (shell, host) = window(cx, Vec::new(), today, Folders::default(), vec![split]);
    shell.update(cx, |shell, cx| {
        shell.join_split(13, 10, false, false, cx);
        shell.join_split(12, 10, false, false, cx);
    });
    host.take_edits();
    assert_eq!(titles(&shell, cx), (None, None));
}

#[gpui::test]
fn a_tab_opened_for_a_split_is_closed_by_undo(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![10]);
    shell.update(cx, |shell, cx| shell.split_opened(20, cx));
    assert_eq!(titles(&shell, cx).0, Some("Split View"));
    report_split(&shell, cx, vec![10, 20], vec![10, 20], false);
    assert!(undo(&shell, cx));
    assert_eq!(host.take_edits(), ["close 20"]);
    report(&shell, cx, Vec::new(), vec![10]);
    assert!(redo(&shell, cx));
    assert_eq!(host.take_edits(), ["reopen 20 as 1001"]);
}

#[gpui::test]
fn a_change_of_orientation_is_its_own_step(cx: &mut TestAppContext) {
    let split = Split {
        tabs: vec![10, 11],
        ..Default::default()
    };
    let (shell, host) = window(cx, Vec::new(), vec![10, 11], Folders::default(), vec![split]);
    shell.update(cx, |shell, cx| shell.stack_split(10, true, cx));
    assert!(host.take().contains(&"split stacked 10 true".to_string()));
    assert_eq!(titles(&shell, cx).0, Some("Change Orientation"));
    report_split(&shell, cx, vec![10, 11], vec![10, 11], true);
    assert!(undo(&shell, cx));
    assert!(host.take().contains(&"split stacked 10 false".to_string()));
}

#[gpui::test]
fn a_private_window_keeps_no_split_to_undo(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::private_window(host.clone(), 9, "Private".into(), 1, cx));
    shell.update(cx, |shell, cx| {
        shell.set_tabs(9, Vec::new(), vec![10, 11], Vec::new(), Folders::default(), cx);
        shell.join_split(11, 10, false, false, cx);
        shell.split_opened(12, cx);
    });
    assert_eq!(host.take_edits(), ["split 11 10 false"]);
    assert_eq!(titles(&shell, cx), (None, None));
}

#[gpui::test]
fn names_icons_and_colours_go_back_to_what_they_were(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![10]);
    host.names.borrow_mut().insert(10, "Old".into());
    shell.update(cx, |shell, cx| {
        shell.change(Prop::TabName(10), Value::Text("New".into()), cx);
        shell.change(Prop::SpaceName(2), Value::Text("Work".into()), cx);
        let red = Value::Color { color: Some(0xff0000), theme: "red".into() };
        shell.change(Prop::SpaceColor(2), red, cx);
        shell.change(Prop::SpaceIcon(2), Value::Text("🌱".into()), cx);
    });
    assert_eq!(
        host.take_edits(),
        [
            "rename tab 10 New",
            "rename space 2 Work",
            "theme 2 red",
            "color 2 Some(16711680)",
            "space icon 2 🌱"
        ]
    );
    host.names.borrow_mut().insert(10, "New".into());
    for _ in 0..4 {
        undo(&shell, cx);
    }
    assert_eq!(
        host.take_edits(),
        [
            "space icon 2 ",
            "theme 2 none",
            "color 2 None",
            "rename space 2 Two",
            "rename tab 10 Old"
        ]
    );
    redo(&shell, cx);
    assert_eq!(host.take_edits(), ["rename tab 10 New"]);
}

#[gpui::test]
fn naming_something_what_it_is_already_called_is_not_a_step(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![10]);
    shell.update(cx, |shell, cx| {
        shell.change(Prop::SpaceName(1), Value::Text("One".into()), cx)
    });
    assert!(host.take_edits().is_empty());
    assert!(!undo(&shell, cx));
}

#[gpui::test]
fn a_renamed_tab_that_closed_and_came_back_is_renamed_back_under_its_new_id(
    cx: &mut TestAppContext,
) {
    let (shell, host) = plain(cx, vec![10, 11]);
    host.names.borrow_mut().insert(10, "Old".into());
    shell.update(cx, |shell, cx| {
        shell.change(Prop::TabName(10), Value::Text("New".into()), cx);
        shell.close_tab(10, false, cx);
    });
    host.take_edits();
    report(&shell, cx, Vec::new(), vec![11]);
    undo(&shell, cx);
    report(&shell, cx, Vec::new(), vec![1001, 11]);
    host.names.borrow_mut().insert(1001, "New".into());
    undo(&shell, cx);
    assert_eq!(
        host.take_edits(),
        ["reopen 10 as 1001", "rename tab 1001 Old"]
    );
}

#[gpui::test]
fn a_step_that_cant_be_done_any_more_is_dropped_and_the_one_before_it_undone(
    cx: &mut TestAppContext,
) {
    let (shell, host) = plain(cx, vec![10, 11]);
    shell.update(cx, |shell, cx| {
        shell.change(Prop::SpaceName(1), Value::Text("Work".into()), cx);
        shell.pin_tab(11, true, cx);
    });
    host.take_edits();
    // The pinned tab is gone, closed by a page or an agent.
    report(&shell, cx, Vec::new(), vec![10]);
    assert!(undo(&shell, cx));
    assert_eq!(host.take_edits(), ["rename space 1 One"]);
}

#[gpui::test]
fn a_new_step_ends_what_could_be_done_again(cx: &mut TestAppContext) {
    let (shell, _) = plain(cx, vec![10, 11]);
    shell.update(cx, |shell, cx| {
        shell.change(Prop::SpaceName(1), Value::Text("Work".into()), cx)
    });
    undo(&shell, cx);
    assert_eq!(titles(&shell, cx), (None, Some("Rename Space")));
    shell.update(cx, |shell, cx| {
        shell.change(Prop::SpaceName(1), Value::Text("Play".into()), cx)
    });
    assert!(!redo(&shell, cx));
}

#[gpui::test]
fn clearing_and_deleting_a_space_are_undone_and_done_again(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![10, 11, 12]);
    shell.update(cx, |shell, cx| {
        shell.set_selected(10, cx);
        shell.run(Command::Clear, "ui", cx);
    });
    assert_eq!(host.take_edits(), ["clear as 9001"]);
    assert!(
        host.undo_toasts.borrow().is_empty(),
        "Clear says nothing (Arc)"
    );
    report(&shell, cx, Vec::new(), vec![10]);
    undo(&shell, cx);
    assert_eq!(host.take_edits(), ["undo clear 9001"]);
    report(&shell, cx, Vec::new(), vec![10, 1001, 1002]);
    assert!(redo(&shell, cx));
    // The redo is a new Clear, which the next undo asks for by its new id.
    assert_eq!(host.take_edits(), ["clear as 9002"]);
    undo(&shell, cx);
    assert_eq!(host.take_edits(), ["undo clear 9002"]);

    shell.update(cx, |shell, cx| shell.delete_space_now(2, cx));
    assert_eq!(host.take_edits(), ["delete space 2"]);
    undo(&shell, cx);
    assert_eq!(host.take_edits(), ["undo delete space 2"]);
    redo(&shell, cx);
    assert_eq!(host.take_edits(), ["delete space 2"]);
}

#[gpui::test]
fn the_archives_delete_and_clear_are_undone_and_done_again(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![10]);
    shell.update(cx, |shell, cx| {
        shell.set_archive(vec![archived(7), archived(8)], cx)
    });
    shell.update(cx, |shell, cx| shell.delete_archived(7, cx));
    assert_eq!(host.take_edits(), ["archive remove 7 as 9001"]);
    assert_eq!(
        host.undo_toasts.borrow().as_slice(),
        ["Deleted from Archive"]
    );
    undo(&shell, cx);
    assert_eq!(host.take_edits(), ["undo archive remove 9001"]);
    redo(&shell, cx);
    assert_eq!(host.take_edits(), ["archive remove 7 as 9002"]);

    shell.update(cx, |shell, cx| shell.empty_archive(cx));
    assert_eq!(host.take_edits(), ["archive clear as 9003"]);
    undo(&shell, cx);
    assert_eq!(host.take_edits(), ["undo archive remove 9003"]);
    redo(&shell, cx);
    assert_eq!(host.take_edits(), ["archive clear as 9004"]);
}

fn archived(id: u64) -> Archived {
    Archived {
        id,
        url: "https://a.example/".into(),
        title: "A".into(),
        archived: 0.,
    }
}

#[gpui::test]
fn two_windows_over_one_core_each_undo_only_their_own_archive_removals(cx: &mut TestAppContext) {
    let (first, one) = plain(cx, vec![10]);
    let (second, two) = beside(cx, &one, vec![20]);
    for shell in [&first, &second] {
        shell.update(cx, |shell, cx| {
            shell.set_archive(vec![archived(7), archived(8)], cx)
        });
    }
    first.update(cx, |shell, cx| shell.delete_archived(7, cx));
    second.update(cx, |shell, cx| shell.delete_archived(8, cx));
    assert_eq!(one.take_edits(), ["archive remove 7 as 9001"]);
    assert_eq!(two.take_edits(), ["archive remove 8 as 9002"]);

    // The newer removal is the second window's, so a pop of the core's stack would take that.
    assert!(undo(&first, cx));
    assert_eq!(one.take_edits(), ["undo archive remove 9001"]);
    assert_eq!(*one.core.removals.borrow(), [(9002, 1)]);
    assert!(two.take_edits().is_empty());

    assert!(undo(&second, cx));
    assert_eq!(two.take_edits(), ["undo archive remove 9002"]);
    assert!(one.core.removals.borrow().is_empty());
}

#[gpui::test]
fn two_windows_over_one_core_each_undo_only_the_space_they_deleted(cx: &mut TestAppContext) {
    let (first, one) = plain(cx, vec![10]);
    let (second, two) = beside(cx, &one, vec![20]);
    first.update(cx, |shell, cx| shell.delete_space_now(2, cx));
    second.update(cx, |shell, cx| shell.delete_space_now(3, cx));
    one.take_edits();
    two.take_edits();

    assert!(undo(&first, cx));
    assert_eq!(one.take_edits(), ["undo delete space 2"]);
    assert_eq!(*one.core.deleted.borrow(), [3]);
    assert!(two.take_edits().is_empty());
    assert!(undo(&second, cx));
    assert_eq!(two.take_edits(), ["undo delete space 3"]);
}

#[gpui::test]
fn two_windows_clear_and_undo_on_their_own(cx: &mut TestAppContext) {
    let (first, one) = plain(cx, vec![10, 11, 12]);
    let (second, two) = beside(cx, &one, vec![20, 21, 22]);
    first.update(cx, |shell, cx| {
        shell.set_selected(10, cx);
        shell.run(Command::Clear, "ui", cx);
    });
    second.update(cx, |shell, cx| {
        shell.set_selected(20, cx);
        shell.run(Command::Clear, "ui", cx);
    });
    one.take_edits();
    two.take_edits();
    assert!(undo(&first, cx));
    assert_eq!(one.take_edits(), ["undo clear 9001"]);
    assert!(two.take_edits().is_empty());
    assert!(undo(&second, cx));
    assert_eq!(two.take_edits(), ["undo clear 9002"]);
}

#[gpui::test]
fn a_step_the_core_has_dropped_is_dropped_and_the_one_before_it_undone(cx: &mut TestAppContext) {
    // The window closes a tab, then deletes an archived one; the other window then fills the
    // core's 25 removals, which pushes the first window's out.
    let (first, one) = plain(cx, vec![10, 11]);
    let (second, _) = beside(cx, &one, vec![20]);
    let others: Vec<Archived> = (100..126).map(archived).collect();
    for shell in [&first, &second] {
        let mut all = others.clone();
        all.push(archived(7));
        shell.update(cx, |shell, cx| shell.set_archive(all, cx));
    }
    first.update(cx, |shell, cx| shell.close_tab(11, false, cx));
    first.update(cx, |shell, cx| shell.delete_archived(7, cx));
    for id in 100..125 {
        second.update(cx, |shell, cx| shell.delete_archived(id, cx));
    }
    assert_eq!(one.core.removals.borrow().len(), 25);
    one.take_edits();

    // Gone from the core, so the window's Undo does the close's instead.
    assert!(undo(&first, cx));
    assert_eq!(one.take_edits(), ["reopen 11 as 1001"]);
}

#[gpui::test]
fn the_tenth_clear_back_is_the_last_the_core_keeps(cx: &mut TestAppContext) {
    let (shell, host) = plain(cx, vec![10, 11, 12]);
    shell.update(cx, |shell, cx| shell.set_selected(10, cx));
    for _ in 0..11 {
        shell.update(cx, |shell, cx| shell.run(Command::Clear, "ui", cx));
    }
    host.take_edits();
    for _ in 0..10 {
        assert!(undo(&shell, cx));
    }
    host.take_edits();
    // The eleventh is gone from the core, so it drops out of the history and nothing is left.
    assert!(!undo(&shell, cx));
    assert!(host.take_edits().is_empty());
}

#[gpui::test]
fn a_page_without_text_focus_gets_command_z_only_while_there_is_something_to_undo(
    cx: &mut TestAppContext,
) {
    let (shell, host) = plain(cx, vec![10, 11]);
    // Nothing to undo: the key goes on to the Edit menu, for the page's own undo.
    assert!(!shell.update(cx, |shell, cx| shell.run_for_page(Command::Undo, cx)));
    assert!(!shell.update(cx, |shell, cx| shell.run_for_page(Command::Redo, cx)));
    shell.update(cx, |shell, cx| shell.close_tab(11, false, cx));
    host.take_edits();
    assert!(shell.update(cx, |shell, cx| shell.run_for_page(Command::Undo, cx)));
    assert_eq!(host.take_edits(), ["reopen 11 as 1001"]);
    report(&shell, cx, Vec::new(), vec![10, 1001]);
    assert!(shell.update(cx, |shell, cx| shell.run_for_page(Command::Redo, cx)));
}

#[gpui::test]
fn a_private_window_keeps_nothing_to_undo(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::private_window(host.clone(), 9, "Private".into(), 1, cx));
    shell.update(cx, |shell, cx| {
        shell.set_tabs(9, Vec::new(), vec![10], Vec::new(), Folders::default(), cx)
    });
    shell.update(cx, |shell, cx| shell.close_tab(10, false, cx));
    assert_eq!(titles(&shell, cx), (None, None));
    assert!(!undo(&shell, cx));
}

/// With the UI focused, ⌘Z and ⇧⌘Z are the shell's own, and the Edit menu's Undo and Redo reach it
/// too, since the menu sends them to whatever is focused.
#[gpui::test]
fn command_z_and_the_edit_menu_undo_and_redo_with_the_ui_focused(cx: &mut TestAppContext) {
    use gpui::VisualTestContext;

    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    cx.update(crate::shell::bind_keys);
    let (shell, cx): (Entity<Shell>, &mut VisualTestContext) =
        cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(vec![(1, "One".into(), None, 1, "".into(), "".into(), "".into())], cx);
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_selected(10, cx);
    });
    cx.update(|window, cx| window.focus(&shell.read(cx).focus_handle.clone(), cx));

    cx.simulate_keystrokes("cmd-d");
    assert_eq!(host.take_edits(), ["pinned 10 true"]);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, vec![10], vec![11], Vec::new(), Folders::default(), cx)
    });
    cx.simulate_keystrokes("cmd-z");
    assert_eq!(host.take_edits(), ["move 10 false 0 0"]);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        )
    });
    cx.simulate_keystrokes("shift-cmd-z");
    assert_eq!(host.take_edits(), ["move 10 true 0 0"]);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, vec![10], vec![11], Vec::new(), Folders::default(), cx)
    });

    // The menu's items are the actions a field would take for itself.
    cx.dispatch_action(crate::text_input::Undo);
    assert_eq!(host.take_edits(), ["move 10 false 0 0"]);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        )
    });
    cx.dispatch_action(crate::text_input::Redo);
    assert_eq!(host.take_edits(), ["move 10 true 0 0"]);
}

#[gpui::test]
fn a_folders_icon_goes_back_to_what_it_was(cx: &mut TestAppContext) {
    let folders = Folders {
        folders: HashMap::from([(5, ("Reading".into(), true))]),
        parents: HashMap::from([(6, 5)]),
        icons: HashMap::from([(5, "📚".into())]),
        ..Default::default()
    };
    let (shell, host) = window(cx, vec![4, 5, 6], Vec::new(), folders, Vec::new());
    shell.update(cx, |shell, cx| {
        shell.change(Prop::FolderIcon(5), Value::Text("🌱".into()), cx)
    });
    assert_eq!(host.take_edits(), ["folder icon 5 🌱"]);
    assert_eq!(titles(&shell, cx).0, Some("Change Icon"));
    undo(&shell, cx);
    assert_eq!(host.take_edits(), ["folder icon 5 📚"]);
}

/// The core reports the first Space's lists with one split view.
fn report_lists(
    shell: &Entity<Shell>,
    cx: &mut TestAppContext,
    pinned: Vec<u64>,
    today: Vec<u64>,
    split: Vec<u64>,
) {
    let splits = vec![Split {
        tabs: split,
        ..Default::default()
    }];
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, pinned, today, splits, Folders::default(), cx)
    });
}

/// ⌃⇧- on a pinned tab's pane asks the core to close it, which only takes it out of the split
/// unloaded, as ⌘W keeps a pinned tab's row; ⇧⌘T then puts it back in its pane, pinned, and
/// doesn't reopen an archived tab instead (#1289).
#[gpui::test]
fn closing_a_pinned_tabs_pane_keeps_it_pinned_and_reopening_puts_it_back(
    cx: &mut TestAppContext,
) {
    let split = Split {
        tabs: vec![20, 21],
        ..Default::default()
    };
    let (shell, host) = window(cx, vec![20, 21], vec![10], Folders::default(), vec![split]);
    shell.update(cx, |shell, cx| shell.set_selected(20, cx));
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::CloseSplitPane, "ui", cx));
    assert_eq!(host.take_edits(), ["close 20"]);
    assert_eq!(titles(&shell, cx).0, Some("Close Tab"));
    // The core separated it after the split's last tab, unloaded, and showed the one beside it.
    report(&shell, cx, vec![21, 20], vec![10]);
    shell.update(cx, |shell, cx| shell.set_selected(21, cx));
    host.take();

    shell.update(cx, |shell, cx| shell.run(Command::ReopenClosedTab, "ui", cx));
    assert_eq!(host.take_edits(), ["split 20 21 true"]);
    assert!(!host.take().contains(&"reopen closed tab".to_string()));
    // Reopened, it's no longer a close for ⌘Z to undo, and the next ⇧⌘T is the core's.
    assert_eq!(titles(&shell, cx).0, None);
    report_lists(&shell, cx, vec![20, 21], vec![10], vec![20, 21]);
    shell.update(cx, |shell, cx| shell.run(Command::ReopenClosedTab, "ui", cx));
    assert!(host.take_edits().is_empty());
    assert!(host.take().contains(&"reopen closed tab".to_string()));
}

/// ⌘Z after closing a pinned tab's pane puts it back in its pane too, and then ⇧⌘T has no pane
/// left to put back, so it asks the core for the tab closed last.
#[gpui::test]
fn undoing_a_pinned_panes_close_rejoins_it_once(cx: &mut TestAppContext) {
    let split = Split {
        tabs: vec![20, 21],
        ..Default::default()
    };
    let (shell, host) = window(cx, vec![20, 21], vec![10], Folders::default(), vec![split]);
    shell.update(cx, |shell, cx| shell.set_selected(21, cx));
    shell.update(cx, |shell, cx| shell.run(Command::CloseSplitPane, "ui", cx));
    assert_eq!(host.take_edits(), ["close 21"]);
    report(&shell, cx, vec![20, 21], vec![10]);
    assert!(undo(&shell, cx));
    assert_eq!(host.take_edits(), ["split 21 20 false"]);
    report_lists(&shell, cx, vec![20, 21], vec![10], vec![20, 21]);
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::ReopenClosedTab, "ui", cx));
    assert!(host.take_edits().is_empty());
    assert!(host.take().contains(&"reopen closed tab".to_string()));
}

/// ⌃⇧- on a Today tab's pane closes the tab, into the archive, which ⌘Z reopens at its place, and
/// ⇧⌘T asks the core for, which puts it back in its split.
#[gpui::test]
fn closing_a_today_tabs_pane_closes_the_tab(cx: &mut TestAppContext) {
    let split = Split {
        tabs: vec![10, 11],
        ..Default::default()
    };
    let (shell, host) = window(cx, Vec::new(), vec![10, 11, 12], Folders::default(), vec![split]);
    shell.update(cx, |shell, cx| shell.set_selected(11, cx));
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::CloseSplitPane, "ui", cx));
    assert_eq!(host.take_edits(), ["close 11"]);
    report(&shell, cx, Vec::new(), vec![10, 12]);
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::ReopenClosedTab, "ui", cx));
    assert!(host.take().contains(&"reopen closed tab".to_string()));
    assert!(host.take_edits().is_empty());
}

/// A pinned tab ⌥-clicked onto a Today page takes that page into its place in the pinned tabs,
/// pinned, before joining it, so the split is pinned and the tab keeps its pin and place, where
/// the core alone would move it into Today. One ⌘Z puts both back, and ⇧⌘Z does both again.
#[gpui::test]
fn a_pinned_tab_joining_a_today_tab_keeps_its_pin(cx: &mut TestAppContext) {
    let (shell, host) = window(cx, vec![20, 21], vec![10], Folders::default(), Vec::new());
    shell.update(cx, |shell, cx| shell.set_selected(10, cx));
    host.take();
    assert!(shell.update(cx, |shell, cx| shell.alt_press_row(21, cx)));
    assert_eq!(host.take_edits(), ["move 10 true 0 1", "split 21 10 false"]);
    assert_eq!(titles(&shell, cx).0, Some("Split View"));
    report_lists(&shell, cx, vec![20, 10, 21], Vec::new(), vec![10, 21]);

    assert!(undo(&shell, cx));
    assert_eq!(
        host.take_edits(),
        ["separate 21", "move 21 true 0 1", "move 10 false 0 0"]
    );
    report(&shell, cx, vec![20, 21], vec![10]);
    assert!(redo(&shell, cx));
    assert_eq!(host.take_edits(), ["move 10 true 0 1", "split 21 10 false"]);
}

/// A Today tab joining a pinned tab's split is the core's to pin, and a Today tab joining another
/// moves nothing first.
#[gpui::test]
fn only_a_pinned_tab_joining_today_moves_a_row_first(cx: &mut TestAppContext) {
    let (shell, host) = window(cx, vec![20], vec![10, 11], Folders::default(), Vec::new());
    shell.update(cx, |shell, cx| shell.join_split(10, 20, false, false, cx));
    assert_eq!(host.take_edits(), ["split 10 20 false"]);
    report_lists(&shell, cx, vec![20, 10], vec![11], vec![20, 10]);
    shell.update(cx, |shell, cx| shell.join_split(11, 10, false, false, cx));
    assert_eq!(host.take_edits(), ["split 11 10 false"]);
}
