//! Snoozing Today tabs (#2350), on a host that records what the shell asks of the core.

use std::rc::Rc;

use chrono::{NaiveDate, NaiveDateTime};
use gpui::{AppContext, Entity, TestAppContext, point, px};

use crate::{
    shell::{Command, Folders, Shell, recorder::Recorder},
    snooze::Choice,
};

/// Tuesday 10 November 2026 at 14:25.
fn tuesday() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 10)
        .and_then(|day| day.and_hms_opt(14, 25, 0))
        .unwrap()
}

fn at(day: u32, hour: u32) -> f64 {
    let time = NaiveDate::from_ymd_opt(2026, 11, day)
        .and_then(|day| day.and_hms_opt(hour, 0, 0))
        .unwrap();
    crate::snooze::millis(time)
}

/// Home, the user's, and Research, an agent's, with `pinned` and `today` in Home, shown.
fn window(
    cx: &mut TestAppContext,
    pinned: Vec<u64>,
    today: Vec<u64>,
) -> (Entity<Shell>, Rc<Recorder>) {
    crate::snooze::stop_clock(tuesday());
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        let home = (1, "Home".into(), None, 1, "".into(), "".into(), "".into());
        let agent = (
            2,
            "Research".into(),
            None,
            1,
            "".into(),
            "Codex".into(),
            "".into(),
        );
        shell.set_spaces(vec![home, agent], cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, pinned, today, Vec::new(), Folders::default(), cx);
    });
    (shell, host)
}

fn report(shell: &Entity<Shell>, cx: &mut TestAppContext, today: Vec<u64>) {
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), today, Vec::new(), Folders::default(), cx)
    });
}

fn menu(shell: &Entity<Shell>, host: &Recorder, tab: u64, cx: &mut TestAppContext) -> Vec<String> {
    shell.update(cx, |shell, cx| {
        shell.tab_menu(tab, point(px(100.), px(200.)), cx)
    });
    let items = host.menus.borrow().last().cloned().unwrap_or_default();
    items
        .into_iter()
        .map(|(title, _)| title.to_string())
        .collect()
}

/// Snooze's Tomorrow Morning closes the tab into the archive until Wednesday 08:00, says so in a
/// toast with Undo, and ⌘Z brings it back where it was; done again, it's the tab that came back
/// that snoozes, until the same time.
#[gpui::test]
fn a_snoozed_tab_leaves_says_when_it_returns_and_undo_brings_it_back(cx: &mut TestAppContext) {
    let (shell, host) = window(cx, Vec::new(), vec![10, 11, 12]);
    menu(&shell, &host, 11, cx);
    cx.update(|cx| host.choose("Tomorrow Morning", cx));
    let wednesday = at(11, 8);
    assert_eq!(host.take_edits(), [format!("snooze 11 until {wednesday}")]);
    assert_eq!(
        host.undo_toasts.borrow().as_slice(),
        ["Snoozed until tomorrow 08:00"]
    );
    report(&shell, cx, vec![10, 12]);

    assert!(shell.update(cx, |shell, cx| shell.undo(cx)));
    assert_eq!(host.take_edits(), ["reopen 11 as 1001"]);
    report(&shell, cx, vec![10, 1001, 12]);

    assert!(shell.update(cx, |shell, cx| shell.redo(cx)));
    assert_eq!(
        host.take_edits(),
        [format!("snooze 1001 until {wednesday}")]
    );
}

/// The chosen rows snooze together, as one step, and the choice goes with them.
#[gpui::test]
fn the_chosen_rows_snooze_as_one_step(cx: &mut TestAppContext) {
    let (shell, host) = window(cx, Vec::new(), vec![10, 11, 12]);
    shell.update(cx, |shell, cx| {
        let evening = crate::snooze::Choice::ThisEvening.at(tuesday()).unwrap();
        shell.snooze_tabs(vec![12, 11], evening, cx);
        assert!(shell.chosen.is_empty());
    });
    let six = at(10, 18);
    assert_eq!(
        host.take_edits(),
        [
            format!("snooze 11 until {six}"),
            format!("snooze 12 until {six}")
        ]
    );
    assert_eq!(
        host.undo_toasts.borrow().as_slice(),
        ["Snoozed until today 18:00"]
    );
    report(&shell, cx, vec![10]);
    assert!(shell.update(cx, |shell, cx| shell.undo(cx)));
    assert_eq!(
        host.take_edits(),
        ["reopen 12 as 1001", "reopen 11 as 1002"]
    );
}

/// A pinned tab stays, and an agent's Space or a private window's tab has no Space of the user's
/// to come back to, so their menus have no Snooze.
#[gpui::test]
fn only_the_users_today_tabs_snooze(cx: &mut TestAppContext) {
    let (shell, host) = window(cx, vec![5], vec![10]);
    assert!(
        menu(&shell, &host, 10, cx)
            .iter()
            .any(|item| item == "Snooze")
    );
    assert!(
        !menu(&shell, &host, 5, cx)
            .iter()
            .any(|item| item == "Snooze")
    );

    shell.update(cx, |shell, cx| {
        shell.set_space(2, cx);
        shell.set_tabs(2, Vec::new(), vec![20], Vec::new(), Folders::default(), cx);
    });
    assert!(
        !menu(&shell, &host, 20, cx)
            .iter()
            .any(|item| item == "Snooze")
    );
    shell.update(cx, |shell, cx| shell.snooze_tabs(vec![20], tuesday(), cx));
    assert!(host.take_edits().is_empty());

    let private = cx.new(|cx| Shell::private_window(host.clone(), 9, "Private".into(), 1, cx));
    private.update(cx, |shell, cx| {
        shell.set_tabs(9, Vec::new(), vec![30], Vec::new(), Folders::default(), cx);
        assert!(!shell.snoozable(30));
    });
}

/// Pick a Date and Time… starts with Tomorrow Morning's time, snoozes until what's typed when
/// it reads as a time ahead, and does nothing for text that doesn't.
#[gpui::test]
fn the_picker_snoozes_until_the_time_it_reads(cx: &mut TestAppContext) {
    let (shell, host) = window(cx, Vec::new(), vec![10, 11]);
    menu(&shell, &host, 11, cx);
    cx.update(|cx| host.choose("Pick a Date and Time…", cx));
    assert_eq!(
        host.told.borrow().as_slice(),
        ["Snooze Until: Snooze / Cancel / "]
    );
    let answer = host.told_action.borrow_mut().take().unwrap();
    cx.update(|cx| answer(vec!["whenever".into()], cx));
    assert!(host.take_edits().is_empty());

    menu(&shell, &host, 11, cx);
    cx.update(|cx| host.choose("Pick a Date and Time…", cx));
    let answer = host.told_action.borrow_mut().take().unwrap();
    cx.update(|cx| answer(vec!["fri 5pm".into()], cx));
    assert_eq!(
        host.take_edits(),
        [format!("snooze 11 until {}", at(13, 17))]
    );
}

/// A tab back by itself is marked until it's shown; one the user is looking at isn't.
#[gpui::test]
fn a_tab_back_from_snooze_is_marked_until_its_shown(cx: &mut TestAppContext) {
    let (shell, _host) = window(cx, Vec::new(), vec![10, 11]);
    shell.update(cx, |shell, cx| {
        shell.set_selected(10, cx);
        shell.tab_unsnoozed(10, cx);
        shell.tab_unsnoozed(11, cx);
        assert!(!shell.unsnoozed.contains(&10));
        assert!(shell.unsnoozed.contains(&11));
        shell.set_selected(11, cx);
        assert!(shell.unsnoozed.is_empty());
    });
}

/// The command bar finds Snooze Tab, one row for each time, each ending with when it means, and
/// ↩ on one snoozes the tab shown; a pinned tab has none.
#[gpui::test]
fn the_bar_snoozes_the_tab_shown(cx: &mut TestAppContext) {
    let (shell, host) = window(cx, vec![5], vec![10, 11]);
    let found = |shell: &Entity<Shell>, cx: &mut TestAppContext| {
        shell.read_with(cx, |shell, _| {
            let keys = crate::shell::UserKeys::default();
            let rows =
                crate::shell::with_named_rows(Vec::new(), "snooze", &shell.named_bar_rows(), &keys);
            (rows.into_iter())
                .map(|row| format!("{} {}", row.name, row.shortcut.unwrap_or_default()))
                .collect::<Vec<_>>()
        })
    };
    shell.update(cx, |shell, cx| shell.set_selected(11, cx));
    assert_eq!(
        found(&shell, cx),
        [
            "Snooze Tab — Later Today 17:25",
            "Snooze Tab — This Evening 18:00",
            "Snooze Tab — Tomorrow Morning Wed 08:00",
            "Snooze Tab — Next Week Mon 08:00",
            "Snooze Tab — Pick a Date and Time… ",
        ]
    );
    shell.update(cx, |shell, cx| {
        shell.run(Command::Snooze(Choice::NextWeek), "command-bar", cx)
    });
    assert_eq!(
        host.take_edits(),
        [format!("snooze 11 until {}", at(16, 8))]
    );

    shell.update(cx, |shell, cx| shell.set_selected(5, cx));
    assert!(found(&shell, cx).is_empty());
}

/// A snoozed tab in the archive says when it comes back and to which Space, the Filter button
/// offers Snoozed while one is, and its menu brings it back now or snoozes it until another time.
#[gpui::test]
fn a_snoozed_tab_in_the_archive_says_when_and_where_it_comes_back(cx: &mut TestAppContext) {
    let (shell, host) = window(cx, Vec::new(), vec![10]);
    let tab = |id, snoozed_until, space| crate::shell::Archived {
        id,
        url: "https://zed.dev/blog".into(),
        title: "Zed".into(),
        archived: 0.,
        little: false,
        snoozed_until,
        space,
    };
    shell.update(cx, |shell, cx| {
        shell.set_archive(vec![tab(30, 0., 1)], cx);
        assert!(
            !shell
                .archive_filters()
                .contains(&Some(super::library::Only::Snoozed))
        );
        let archive = vec![tab(30, 0., 1), tab(31, at(11, 8), 1), tab(32, at(16, 8), 0)];
        shell.set_archive(archive, cx);
        assert!(
            shell
                .archive_filters()
                .contains(&Some(super::library::Only::Snoozed))
        );
        let details: Vec<String> = (shell.archive.iter())
            .map(|tab| shell.archived_detail(tab))
            .collect();
        assert_eq!(
            details,
            [
                "zed.dev",
                "Snoozed until Wed 08:00 in Home · zed.dev",
                // Its Space is gone, so it stays in the archive as a tab closed then.
                "Snoozed until Mon 08:00 · zed.dev",
            ]
        );
        shell.archived_menu(31, point(px(100.), px(200.)), cx);
    });
    let titles: Vec<String> = (host.menus.borrow().last().unwrap().iter())
        .map(|(title, _)| title.to_string())
        .collect();
    assert_eq!(
        titles,
        [
            "Bring Back Now",
            "Change Time",
            "Later Today",
            "This Evening",
            "Tomorrow Morning",
            "Next Week",
            "",
            "Pick a Date and Time…",
            "Copy URL",
            "",
            "Delete from Archive",
        ]
    );
    host.take();
    cx.update(|cx| host.choose("Next Week", cx));
    assert_eq!(
        host.take(),
        [format!("snooze archived 31 until {}", at(16, 8))]
    );
    shell.update(cx, |shell, cx| {
        shell.archived_menu(31, point(px(100.), px(200.)), cx)
    });
    cx.update(|cx| host.choose("Bring Back Now", cx));
    assert_eq!(host.take(), ["restore archived 31"]);
}
