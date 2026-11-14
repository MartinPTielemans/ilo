//! The tasks Archived Tabs lists among the tabs: by when they left the foot, back with a click,
//! out with Delete from Archive or Clear Archive, alone under the Filter's Tasks, found by search,
//! and never in a private window.

use std::rc::Rc;

use gpui::{
    AppContext as _, Entity, Modifiers, MouseButton, TestAppContext, VisualTestContext, point, px,
};

use super::{
    ARCHIVE_TOP, ARCHIVED_LEFT, ARCHIVED_PITCH, Archived, COLUMN_WIDTH, FILTER_WIDTH, FILTER_X,
    HEADING_BOTTOM, HEADING_HEIGHT, HEADING_TOP, Item, SEARCH_HEIGHT, SEARCH_TOP,
};
use crate::agents::task::{ArchivedTask, Card, Dot};
use crate::shell::recorder::Recorder;
use crate::shell::{Folders, Section, Shell};

fn tab(id: u64, archived: f64, little: bool) -> Archived {
    Archived {
        id,
        url: format!("https://{id}.example/").into(),
        title: format!("Tab {id}").into(),
        archived,
        little,
    }
}

fn task(n: u64, left: f64) -> ArchivedTask {
    ArchivedTask {
        agent: format!("task-{n}").into(),
        title: "Lisbon museum hours".into(),
        asked: "Which museums in Lisbon open on Monday?".into(),
        dot: Dot::Done,
        about: "Claude · Done 13:31 · 1 page".into(),
        left,
    }
}

/// Task 1's card, back at the foot, in its own Space 101.
fn card() -> Card {
    Card {
        agent: "task-1".into(),
        home: 1,
        space: 101,
        name: "Claude".into(),
        replies: true,
        title: "Lisbon museum hours".into(),
        status: "Most open at 10".into(),
        asks: false,
        dot: Dot::Done,
        answers: Vec::new(),
        question: None,
        open: Vec::new(),
        retry: false,
        command: None,
    }
}

/// A window on Space 1, with task 1's Space 101, and the Library open on Archived Tabs at rest,
/// with tabs 7 and 8 and task 1 left between them.
fn archive(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, &mut VisualTestContext) {
    let host = Rc::new(Recorder::default());
    host.space_agents.borrow_mut().insert(101, "task-1".into());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| {
        let space = |id: u64, name: &str, agent: &str| {
            (
                id,
                name.to_string().into(),
                None,
                0,
                "".into(),
                agent.to_string().into(),
                "".into(),
            )
        };
        shell.set_spaces(
            vec![
                space(1, "Space 1", ""),
                space(101, "Lisbon museum hours", "Claude"),
            ],
            cx,
        );
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_tabs(
            101,
            Vec::new(),
            vec![1010],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
        shell.section = Section::Archive;
        shell.set_library(true, cx);
        shell.library_motion = None;
        shell.sidebar_fade = None;
        shell.set_archive(vec![tab(7, 3000., false), tab(8, 1000., false)], cx);
        shell.set_archived_tasks(vec![task(1, 2000.)], cx);
    });
    cx.run_until_parked();
    host.take();
    (shell, host, cx)
}

fn items(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<String> {
    shell.read_with(cx, |shell, _| {
        let items = &shell.library.as_ref().unwrap().items;
        let named = items.iter().map(|item| match item {
            Item::Heading(heading) => heading.to_string(),
            Item::Tab(index) => format!("tab {}", shell.archive[*index].id),
            Item::Task(index) => shell.archived_tasks[*index].agent.to_string(),
            _ => "other".to_string(),
        });
        named.collect()
    })
}

/// The middle of the list's `row`th row, under the first heading.
fn row(row: usize) -> gpui::Point<gpui::Pixels> {
    let top = ARCHIVE_TOP + HEADING_TOP + HEADING_HEIGHT + HEADING_BOTTOM;
    point(
        px(COLUMN_WIDTH + ARCHIVED_LEFT + 60.),
        px(top + ARCHIVED_PITCH * (row as f32 + 0.5)),
    )
}

fn filter(cx: &mut VisualTestContext) {
    let filter = point(
        px(FILTER_X + FILTER_WIDTH / 2.),
        px(SEARCH_TOP + SEARCH_HEIGHT / 2.),
    );
    cx.simulate_click(filter, Modifiers::none());
    cx.run_until_parked();
}

fn search(shell: &Entity<Shell>, text: &str, cx: &mut VisualTestContext) {
    let field = shell.read_with(cx, |shell, _| {
        shell.library.as_ref().unwrap().search.clone()
    });
    let text = text.to_string();
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.set_text(text.clone().into(), text.len(), window, cx)
        })
    });
    cx.run_until_parked();
}

/// A task the user dismissed lists among the archived tabs, by when it left the foot, under the
/// same headings, and the Filter button comes with it.
#[gpui::test]
fn a_dismissed_task_lists_among_the_tabs(cx: &mut TestAppContext) {
    let (shell, _, cx) = archive(cx);
    assert_eq!(items(&shell, cx), ["Today", "tab 7", "task-1", "tab 8"]);
    assert!(
        shell.read_with(cx, |shell, _| shell.archive_filtered()),
        "no Filter button"
    );
    // Gone from the Agents' list, as when it came back to the foot, it leaves the archive's.
    shell.update(cx, |shell, cx| shell.set_archived_tasks(Vec::new(), cx));
    assert_eq!(items(&shell, cx), ["Today", "tab 7", "tab 8"]);
    assert!(
        !shell.read_with(cx, |shell, _| shell.archive_filtered()),
        "a Filter with nothing to pick"
    );
}

/// A click on a task's row closes the Library and brings the task back, as ⇧⌘T does, and its
/// conversation opens as its card comes back to the foot. ⌘Z then has no dismissal to undo.
#[gpui::test]
fn a_click_brings_a_task_back_with_its_conversation(cx: &mut TestAppContext) {
    let (shell, host, cx) = archive(cx);
    shell.update(cx, |shell, cx| {
        shell.keep(
            super::What::DISMISS_TASK,
            Some(super::Edit::RestoreTask("task-1".into())),
            cx,
        )
    });
    cx.simulate_click(row(1), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(host.take(), ["task task-1 Restore"]);
    assert!(
        !shell.read_with(cx, |shell, _| shell.library_open),
        "the Library stayed open"
    );
    // The Agents send its card back.
    shell.update(cx, |shell, cx| shell.set_task_cards(vec![card()], cx));
    cx.run_until_parked();
    assert!(
        host.take().contains(&"show task task-1".to_string()),
        "its conversation didn't open"
    );
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.talk_agent()),
        Some("task-1".into())
    );
    assert!(
        !shell.update(cx, |shell, cx| shell.undo(cx)),
        "⌘Z dismissed it again"
    );
}

/// Delete from Archive takes the task out of the Archive, as a step ⌘Z undoes. Its menu's Open
/// Conversation does what a click does.
#[gpui::test]
fn delete_from_archive_takes_a_task_out(cx: &mut TestAppContext) {
    let (shell, host, cx) = archive(cx);
    let menu = |cx: &mut VisualTestContext| {
        cx.simulate_mouse_down(row(1), MouseButton::Right, Modifiers::none());
        cx.simulate_mouse_up(row(1), MouseButton::Right, Modifiers::none());
        cx.run_until_parked();
        let items = host.menus.borrow().last().cloned().unwrap_or_default();
        items
            .into_iter()
            .filter(|(title, _)| !title.is_empty())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        menu(cx),
        [
            ("Open Conversation".into(), true),
            ("Delete from Archive".into(), true)
        ]
    );
    cx.update(|_, cx| host.choose("Delete from Archive", cx));
    assert_eq!(host.take(), ["task task-1 Delete"]);
    // The Agents' list leaves it out.
    shell.update(cx, |shell, cx| shell.set_archived_tasks(Vec::new(), cx));
    assert_eq!(items(&shell, cx), ["Today", "tab 7", "tab 8"]);
    assert!(
        shell.update(cx, |shell, cx| shell.undo(cx)),
        "⌘Z has nothing to undo"
    );
    assert_eq!(host.take(), ["task task-1 Undelete"]);

    shell.update(cx, |shell, cx| {
        shell.set_archived_tasks(vec![task(1, 2000.)], cx)
    });
    menu(cx);
    cx.update(|_, cx| host.choose("Open Conversation", cx));
    assert_eq!(host.take(), ["task task-1 Restore"]);
    assert!(
        !shell.read_with(cx, |shell, _| shell.library_open),
        "the Library stayed open"
    );
}

/// The Filter's menu offers All, Closed Tabs, Tasks and Visited, without Little ilo while it has
/// none of its tabs. Tasks lists the tasks alone, and with none, says how long they stay.
#[gpui::test]
fn the_filter_lists_only_the_tasks(cx: &mut TestAppContext) {
    let (shell, host, cx) = archive(cx);
    filter(cx);
    let menu = host.menus.borrow().last().cloned().unwrap_or_default();
    assert_eq!(
        menu,
        [
            ("All".into(), true),
            ("".into(), false),
            ("Closed Tabs".into(), true),
            ("Tasks".into(), true),
            ("".into(), false),
            ("Visited".into(), true)
        ]
    );
    assert_eq!(
        host.entries
            .borrow()
            .iter()
            .map(|e| e.checked)
            .collect::<Vec<_>>(),
        [true, false, false, false, false, false]
    );
    cx.update(|_, cx| host.choose("Tasks", cx));
    cx.run_until_parked();
    assert_eq!(items(&shell, cx), ["Today", "task-1"]);
    filter(cx);
    assert_eq!(
        host.entries
            .borrow()
            .iter()
            .map(|e| e.checked)
            .collect::<Vec<_>>(),
        [false, false, false, true, false, false]
    );
    cx.update(|_, cx| host.dismiss(cx));

    // A Little ilo tab keeps the button once the task has gone, and Tasks then lists nothing.
    shell.update(cx, |shell, cx| {
        shell.set_archive(vec![tab(7, 3000., false), tab(9, 500., true)], cx);
        shell.set_archived_tasks(Vec::new(), cx);
    });
    assert!(items(&shell, cx).is_empty());
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.empty_line(Section::Archive, true)),
        "Finished tasks stay here until ilo quits."
    );
    filter(cx);
    cx.update(|_, cx| host.choose("Little ilo", cx));
    cx.run_until_parked();
    assert_eq!(items(&shell, cx), ["Today", "tab 9"]);
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.empty_line(Section::Archive, true)),
        "Closed Today tabs stay here for 30 days."
    );
}

/// Search finds a task by its title or by what was asked, as it finds a tab by its title or URL.
#[gpui::test]
fn search_finds_a_task_by_its_title_or_what_was_asked(cx: &mut TestAppContext) {
    let (shell, _, cx) = archive(cx);
    search(&shell, "museum hours", cx);
    assert_eq!(items(&shell, cx), ["Today", "task-1"]);
    search(&shell, "monday", cx);
    assert_eq!(items(&shell, cx), ["Today", "task-1"]);
    search(&shell, "tab 8", cx);
    assert_eq!(items(&shell, cx), ["Today", "tab 8"]);
    search(&shell, "gulbenkian", cx);
    assert!(items(&shell, cx).is_empty());
}

/// A private window, which offers no tasks, keeps none of the Archive's, so nothing there, such
/// as Clear Archive, reaches the regular windows' tasks.
#[gpui::test]
fn a_private_window_keeps_no_archived_task(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let private = cx.new(|cx| Shell::private_window(host.clone(), 9, "Private".into(), 1, cx));
    private.update(cx, |shell, cx| {
        shell.set_archived_tasks(vec![task(1, 2000.)], cx)
    });
    assert!(private.read_with(cx, |shell, _| shell.archived_tasks.is_empty()));
}

/// Clear Archive empties the tasks with the tabs, saying both, and one ⌘Z brings both back. The
/// tasks stay in memory for it, as Delete from Archive leaves one.
#[gpui::test]
fn clear_archive_takes_the_tasks_with_the_tabs(cx: &mut TestAppContext) {
    let (shell, host, cx) = archive(cx);
    shell.update(cx, |shell, cx| {
        shell.set_archived_tasks(vec![task(2, 2500.), task(1, 2000.)], cx)
    });
    shell.update(cx, |shell, cx| shell.clear_archive(cx));
    let (message, detail, confirmed) = host.confirming.borrow_mut().take().expect("it didn't ask");
    assert_eq!(message, "Clear the archive?");
    assert!(
        detail.starts_with("Its 2 tabs and 2 tasks leave the archive. "),
        "{detail}"
    );
    cx.update(|_, cx| confirmed(cx));
    cx.run_until_parked();
    assert_eq!(host.take_edits(), ["archive clear as 9001"]);
    let tasks = |host: &Recorder| -> Vec<String> {
        host.take()
            .into_iter()
            .filter(|call| call.starts_with("task "))
            .collect()
    };
    assert_eq!(tasks(&host), ["task task-2 Delete", "task task-1 Delete"]);
    shell.update(cx, |shell, cx| {
        shell.set_archive(Vec::new(), cx);
        shell.set_archived_tasks(Vec::new(), cx);
    });
    assert!(items(&shell, cx).is_empty());
    assert!(
        shell.update(cx, |shell, cx| shell.undo(cx)),
        "⌘Z has nothing to undo"
    );
    assert_eq!(host.take_edits(), ["undo archive remove 9001"]);
    assert_eq!(
        tasks(&host),
        ["task task-2 Undelete", "task task-1 Undelete"]
    );
}
