//! Library › Agents: the profile's past tasks, kept 30 days, each one deleted with its ✕, all with
//! Clear or with the profile's history of their time, and never in a private window (#1080).

use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::Item;
use crate::agent_history::{Past, read, write};
use crate::shell::commands::ShellHandle;
use crate::shell::recorder::Recorder;
use crate::shell::{Command, Folders, HistoryDeleted, Section, Shell, history_deleted};

const DAY: u64 = 24 * 60 * 60;

fn past(id: &str, ended: u64) -> Past {
    Past {
        id: id.into(),
        title: format!("Task {id}"),
        agent: "Claude Code".into(),
        started: ended - 60,
        ended,
        pages: vec![("Booking".into(), "https://taberna.example/booking".into())],
        orders: 1,
        messages: 0,
    }
}

/// A window on Space 1 of profile 0, whose file keeps task 2, a minute old, task 1, a day old,
/// and task 0, 30 days old, with the Library open on Agents.
fn agents(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    let now = crate::usage::now();
    let kept = [
        past("2", now - 60),
        past("1", now - DAY),
        past("0", now - 30 * DAY),
    ];
    host.agent_history.borrow_mut().insert(0, write(&kept));
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
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
        shell.section = Section::Agents;
        shell.set_library(true, cx);
        shell.library_motion = None;
        shell.sidebar_fade = None;
    });
    cx.run_until_parked();
    (shell, host, cx)
}

fn listed(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    shell.read_with(cx, |shell, _| {
        let items = &shell.library.as_ref().unwrap().items;
        (items.iter())
            .filter_map(|item| match item {
                Item::Past(index) => Some(shell.agent_history[*index].id.clone()),
                Item::Clear => Some("clear".into()),
                _ => None,
            })
            .collect()
    })
}

fn file(host: &Recorder) -> Vec<String> {
    let text = host
        .agent_history
        .borrow()
        .get(&0)
        .cloned()
        .unwrap_or_default();
    read(&text).into_iter().map(|past| past.id).collect()
}

/// It lists what the file keeps, newest first, under Clear, and a task that ended 30 days ago goes,
/// from the list and from the file.
#[gpui::test]
fn agents_lists_the_profiles_past_tasks_for_30_days(cx: &mut TestAppContext) {
    let (shell, host, cx) = agents(cx);
    assert_eq!(listed(&shell, cx), ["clear", "2", "1"]);
    assert_eq!(
        file(&host),
        ["2", "1"],
        "the 30-day-old task stayed in the file"
    );
}

/// A row's ✕ deletes it alone, and Clear the rest.
#[gpui::test]
fn one_deletes_alone_and_clear_takes_them_all(cx: &mut TestAppContext) {
    let (shell, host, cx) = agents(cx);
    shell.update(cx, |shell, cx| shell.delete_past_task("2", cx));
    assert_eq!(listed(&shell, cx), ["clear", "1"]);
    assert_eq!(file(&host), ["1"]);
    shell.update(cx, |shell, cx| shell.clear_agent_history(cx));
    assert!(listed(&shell, cx).is_empty());
    assert!(file(&host).is_empty());
}

/// Deleting the profile's history takes the past tasks that ran in the time it deleted, and all
/// of them with all of it. Opening Clear Browsing Data deletes nothing: the user may cancel it.
#[gpui::test]
fn deleting_history_deletes_them(cx: &mut TestAppContext) {
    let (shell, host, cx) = agents(cx);
    cx.update(|_, cx| cx.set_global(ShellHandle(shell.downgrade())));
    shell.update(cx, |shell, cx| {
        shell.run(Command::ClearBrowsingData, "ui", cx)
    });
    assert_eq!(file(&host), ["2", "1"]);
    let now = crate::usage::now();
    let hour = HistoryDeleted::Some {
        begin: now - 3600,
        end: u64::MAX,
        urls: Vec::new(),
    };
    cx.update(|_, cx| history_deleted(0, hour, cx));
    assert_eq!(file(&host), ["1"]);
    // Another profile's history going leaves this one's.
    cx.update(|_, cx| history_deleted(1, HistoryDeleted::All, cx));
    assert_eq!(file(&host), ["1"]);
    cx.update(|_, cx| history_deleted(0, HistoryDeleted::All, cx));
    assert!(file(&host).is_empty());
}

/// A private window lists none, and reads none.
#[gpui::test]
fn a_private_window_lists_no_past_tasks(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    host.agent_history
        .borrow_mut()
        .insert(0, write(&[past("1", crate::usage::now())]));
    let browser = host.clone();
    let (shell, cx) =
        cx.add_window_view(|_, cx| Shell::private_window(browser, 1, "Private".into(), 0, cx));
    shell.update(cx, |shell, cx| {
        shell.section = Section::Agents;
        shell.set_library(true, cx);
    });
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.agent_history.is_empty()));
}
