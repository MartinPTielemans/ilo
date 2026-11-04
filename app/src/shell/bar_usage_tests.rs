//! What the command bar learns of the user (#1395, #1396), on a host that records what the shell
//! asks of it: a pick ranks a command above its equals and no further, per profile, a switch ranks
//! a tab, and neither is kept from a private window or an agent's Space, past a tab's close or past
//! Clear Browsing Data.

use std::rc::Rc;

use gpui::{AppContext, Entity, TestAppContext};

use super::bar_usage::picks_of;
use super::commands::{ShellHandle, bar_commands};
use crate::shell::{Command, Folders, Page, Shell, picked_in_bar, recorder::Recorder};
use crate::usage;

const DAY: u64 = 24 * 60 * 60;

/// Space 1 in profile 7 with tabs 10, 11 and 12, Space 2 in profile 8 with tab 20, and Claude
/// Code's Space 3, in profile 7, with tab 30. Space 1 shows.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        let space = |id, profile, agent: &str| {
            let name = format!("Space {id}").into();
            (
                id,
                name,
                None,
                profile,
                "".into(),
                agent.to_string().into(),
                "".into(),
            )
        };
        let spaces = vec![space(1, 7, ""), space(2, 8, ""), space(3, 7, "Claude Code")];
        shell.set_spaces(spaces, cx);
        let today = |space, tabs: Vec<u64>| (space, tabs);
        for (space, tabs) in [
            today(1, vec![10, 11, 12]),
            today(2, vec![20]),
            today(3, vec![30]),
        ] {
            shell.set_tabs(space, Vec::new(), tabs, Vec::new(), Folders::default(), cx);
        }
        shell.set_space(1, cx);
        for tab in [10, 11, 12, 20, 30] {
            let url = format!("https://{tab}.test/").into();
            shell.set_page(
                tab,
                Page {
                    url,
                    ..Page::default()
                },
                cx,
            );
        }
    });
    cx.update(|cx| cx.set_global(ShellHandle(shell.downgrade())));
    (shell, host)
}

/// The names of the commands the bar lists for `typed`.
fn names(typed: &str, cx: &mut TestAppContext) -> Vec<String> {
    let found = cx.update(|cx| bar_commands(typed, cx));
    found.iter().map(|row| row.name.to_string()).collect()
}

fn open_bar(shell: &Entity<Shell>, cx: &mut TestAppContext) {
    shell.update(cx, |shell, cx| shell.run(Command::NewTab, "ui", cx));
}

fn pick(name: &str, command: Command, cx: &mut TestAppContext) {
    cx.update(|cx| picked_in_bar(name, command, cx));
}

fn before(names: &[String], first: &str, second: &str) -> bool {
    let at = |name| names.iter().position(|each| each == name);
    at(first)
        .zip(at(second))
        .is_some_and(|(first, second)| first < second)
}

#[gpui::test]
fn a_pick_ranks_a_command_above_its_equals_and_no_further(cx: &mut TestAppContext) {
    let (shell, host) = window(cx);
    open_bar(&shell, cx);
    // The table's order, before any pick.
    let found = names("new", cx);
    assert!(before(&found, "New Folder…", "New Window"), "{found:?}");
    pick("New Window", Command::NewWindow, cx);
    let found = names("new", cx);
    assert!(before(&found, "New Window", "New Folder…"), "{found:?}");
    // Only the times are kept, in the profile's file.
    let kept = host.bar_picks.borrow().get(&7).cloned().unwrap_or_default();
    assert!(kept.starts_with("New Window\t"), "{kept}");
    assert_eq!(kept.lines().count(), 1);
    // A pick breaks ties only: a better match still comes first, however often it's picked.
    for _ in 0..5 {
        pick("New Window", Command::NewWindow, cx);
    }
    for typed in ["win", "w", "dup"] {
        let found = cx.update(|cx| bar_commands(typed, cx));
        let matched: Vec<_> = found.iter().map(|row| row.matched).collect();
        assert!(matched.is_sorted_by(|a, b| a >= b), "{typed}: {matched:?}");
    }
    // With nothing typed, the commands-only list keeps the table's order, learned by place.
    let all = names("", cx);
    assert!(before(&all, "New Folder…", "New Window"), "{all:?}");
}

#[gpui::test]
fn picks_are_read_from_the_profiles_file_and_old_ones_count_less(cx: &mut TestAppContext) {
    let (shell, host) = window(cx);
    let now = usage::now();
    let month_ago = now - 30 * DAY;
    let file = format!("New Folder…\t{month_ago},{month_ago},{month_ago}\nNew Window\t{now}\n");
    host.bar_picks.borrow_mut().insert(7, file);
    open_bar(&shell, cx);
    let found = names("new", cx);
    assert!(before(&found, "New Window", "New Folder…"), "{found:?}");
}

#[gpui::test]
fn each_profile_learns_its_own_picks(cx: &mut TestAppContext) {
    let (shell, host) = window(cx);
    open_bar(&shell, cx);
    pick("New Window", Command::NewWindow, cx);
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    open_bar(&shell, cx);
    let found = names("new", cx);
    assert!(before(&found, "New Folder…", "New Window"), "{found:?}");
    assert_eq!(host.bar_picks.borrow().get(&8), None);
}

#[gpui::test]
fn clear_browsing_data_forgets_what_the_bar_learned(cx: &mut TestAppContext) {
    let (shell, host) = window(cx);
    open_bar(&shell, cx);
    pick("New Window", Command::NewWindow, cx);
    shell.update(cx, |shell, cx| {
        shell.set_selected(11, cx);
        shell.set_selected(10, cx);
    });
    let switched = |shell: &Entity<Shell>, cx: &mut TestAppContext| {
        let tabs = shell.read_with(cx, |shell, cx| shell.open_tabs(cx));
        tabs.iter()
            .find(|tab| tab.tab == 11)
            .map(|tab| tab.switched)
    };
    assert!(switched(&shell, cx).is_some_and(|score| score > 0.0));
    shell.update(cx, |shell, cx| {
        shell.run(Command::ClearBrowsingData, "menu", cx)
    });
    assert_eq!(host.clear_browsing_data.get(), 1);
    assert_eq!(
        host.bar_picks.borrow().get(&7).map(String::as_str),
        Some("")
    );
    cx.update(|cx| assert_eq!(picks_of(7, cx).as_deref(), Some("")));
    let found = names("new", cx);
    assert!(before(&found, "New Folder…", "New Window"), "{found:?}");
    assert_eq!(switched(&shell, cx), Some(0.0));
    // Clear Browsing Data itself isn't counted.
    pick("Clear Browsing Data…", Command::ClearBrowsingData, cx);
    cx.update(|cx| assert_eq!(picks_of(7, cx).as_deref(), Some("")));
}

#[gpui::test]
fn switches_rank_a_tab_until_it_closes(cx: &mut TestAppContext) {
    let (shell, _) = window(cx);
    shell.update(cx, |shell, cx| {
        for tab in [11, 12, 11, 12, 11, 10] {
            shell.set_selected(tab, cx);
        }
    });
    let scores = |shell: &Entity<Shell>, cx: &mut TestAppContext| {
        let tabs = shell.read_with(cx, |shell, cx| shell.bar_tabs(cx));
        tabs.iter()
            .map(|tab| (tab.tab, tab.switched.round() as u32))
            .collect::<Vec<_>>()
    };
    // The list before anything is typed stays most recently shown first.
    assert_eq!(scores(&shell, cx), [(11, 3), (12, 2)]);
    // Tab 11 closes, and its switches go with it.
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 12],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11, 12],
            Vec::new(),
            Folders::default(),
            cx,
        );
    });
    assert_eq!(scores(&shell, cx), [(12, 2)]);
    shell.update(cx, |shell, cx| {
        let url = "https://11.test/".into();
        shell.set_page(
            11,
            Page {
                url,
                ..Page::default()
            },
            cx,
        );
    });
    assert_eq!(scores(&shell, cx), [(12, 2), (11, 0)]);
}

#[gpui::test]
fn an_agents_space_and_a_private_window_teach_the_bar_nothing(cx: &mut TestAppContext) {
    let (shell, host) = window(cx);
    shell.update(cx, |shell, cx| {
        shell.set_space(3, cx);
        shell.set_selected(30, cx);
        shell.set_space(1, cx);
        shell.set_selected(10, cx);
    });
    // The window doesn't list an agent's tabs, and the bar learns nothing of them either.
    let agents = shell.read_with(cx, |shell, cx| shell.switch_score(30, cx));
    assert_eq!(agents, 0.0);

    let before = shell.read_with(cx, |shell, cx| shell.open_tabs(cx));
    let private = cx.new(|cx| Shell::private_window(host.clone(), 9, "Private".into(), 7, cx));
    cx.update(|cx| cx.set_global(ShellHandle(private.downgrade())));
    private.update(cx, |shell, cx| {
        shell.run(Command::NewTab, "ui", cx);
        shell.set_tabs(
            9,
            Vec::new(),
            vec![40, 41],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_selected(40, cx);
        shell.set_selected(41, cx);
    });
    pick("New Window", Command::NewWindow, cx);
    assert_eq!(host.bar_picks.borrow().get(&7), None);
    cx.update(|cx| assert_eq!(picks_of(7, cx), None));
    let after = shell.read_with(cx, |shell, cx| shell.open_tabs(cx));
    let scores = |tabs: &[crate::shell::BarTab]| -> Vec<(u64, u32)> {
        tabs.iter()
            .map(|tab| (tab.tab, tab.switched.round() as u32))
            .collect()
    };
    assert_eq!(scores(&before), scores(&after));
}
