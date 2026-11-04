//! What the command bar learns of the user (#1395, #1396), on a host that records what the shell
//! asks of it: a pick ranks a command above its equals and no further, per profile, a switch ranks
//! a tab, and neither is kept from a private window or an agent's Space, past a tab's close or past
//! the history it was learned in being deleted.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use gpui::{AppContext, Entity, TestAppContext};

use super::bar_usage::{picks_file, picks_of};
use super::commands::{ShellHandle, bar_commands};
use crate::shell::{
    Command, Folders, HistoryDeleted, PICKS_FILE, Page, Shell, history_deleted, move_old_picks,
    picked_in_bar, recorder::Recorder,
};
use crate::usage;

const DAY: u64 = 24 * 60 * 60;

/// Folders the test made, removed when it ends.
struct TestFolders(PathBuf);

impl TestFolders {
    fn new() -> Self {
        static MADE: AtomicUsize = AtomicUsize::new(0);
        let made = MADE.fetch_add(1, Ordering::Relaxed);
        let name = format!("ilo-bar-usage-{}-{made}", std::process::id());
        let root = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    /// A profile's folder, which the browser would give.
    fn folder(&self, name: &str) -> PathBuf {
        let folder = self.0.join(name);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }
}

impl Drop for TestFolders {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What the file in `profile`'s folder keeps, None for no file.
fn file_of(host: &Recorder, profile: u64) -> Option<String> {
    std::fs::read_to_string(picks_file(host, profile)?).ok()
}

fn put_file(host: &Recorder, profile: u64, text: &str) {
    std::fs::write(picks_file(host, profile).unwrap(), text).unwrap();
}

/// Space 1 in profile 7 with tabs 10, 11 and 12, Space 2 in profile 8 with tab 20, and Claude
/// Code's Space 3, in profile 7, with tab 30. Space 1 shows. Each profile has its folder.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>, TestFolders) {
    let host = Rc::new(Recorder::default());
    let folders = TestFolders::new();
    for (profile, name) in [(7, "Default"), (8, "Profile 1")] {
        (host.profile_folders.borrow_mut()).insert(profile, folders.folder(name));
    }
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
    (shell, host, folders)
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
    let (shell, host, _folders) = window(cx);
    open_bar(&shell, cx);
    // The table's order, before any pick.
    let found = names("new", cx);
    assert!(before(&found, "New Folder…", "New Window"), "{found:?}");
    pick("New Window", Command::NewWindow, cx);
    let found = names("new", cx);
    assert!(before(&found, "New Window", "New Folder…"), "{found:?}");
    // Only the times are kept, in a file in the profile's own folder, so it goes with the profile.
    let path = picks_file(&*host, 7).unwrap();
    assert_eq!(
        path.parent(),
        host.profile_folders.borrow().get(&7).map(PathBuf::as_path)
    );
    let kept = file_of(&host, 7).unwrap_or_default();
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
    let (shell, host, _folders) = window(cx);
    let now = usage::now();
    let month_ago = now - 30 * DAY;
    let file = format!("New Folder…\t{month_ago},{month_ago},{month_ago}\nNew Window\t{now}\n");
    put_file(&host, 7, &file);
    open_bar(&shell, cx);
    let found = names("new", cx);
    assert!(before(&found, "New Window", "New Folder…"), "{found:?}");
}

#[gpui::test]
fn each_profile_learns_its_own_picks(cx: &mut TestAppContext) {
    let (shell, host, _folders) = window(cx);
    open_bar(&shell, cx);
    pick("New Window", Command::NewWindow, cx);
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    open_bar(&shell, cx);
    let found = names("new", cx);
    assert!(before(&found, "New Folder…", "New Window"), "{found:?}");
    assert_eq!(file_of(&host, 8), None);
}

/// The number shown for tab 11's switches.
fn switched(shell: &Entity<Shell>, tab: u64, cx: &mut TestAppContext) -> Option<f32> {
    let tabs = shell.read_with(cx, |shell, cx| shell.open_tabs(cx));
    tabs.iter()
        .find(|each| each.tab == tab)
        .map(|each| each.switched)
}

/// Picks New Window and switches to tabs 11 and 12 before showing 10.
fn learn(shell: &Entity<Shell>, cx: &mut TestAppContext) {
    open_bar(shell, cx);
    pick("New Window", Command::NewWindow, cx);
    shell.update(cx, |shell, cx| {
        for tab in [11, 12, 10] {
            shell.set_selected(tab, cx);
        }
    });
}

#[gpui::test]
fn deleting_all_history_forgets_what_the_bar_learned(cx: &mut TestAppContext) {
    let (shell, host, _folders) = window(cx);
    learn(&shell, cx);
    // Opening Clear Browsing Data forgets nothing: the user may cancel it.
    shell.update(cx, |shell, cx| {
        shell.run(Command::ClearBrowsingData, "menu", cx)
    });
    assert_eq!(host.clear_browsing_data.get(), 1);
    assert!(before(&names("new", cx), "New Window", "New Folder…"));
    assert!(switched(&shell, 11, cx).is_some_and(|score| score > 0.0));
    // Another profile's history going leaves this one's.
    cx.update(|cx| history_deleted(8, HistoryDeleted::All, cx));
    assert!(before(&names("new", cx), "New Window", "New Folder…"));
    cx.update(|cx| history_deleted(7, HistoryDeleted::All, cx));
    // None picked leaves no file.
    assert_eq!(file_of(&host, 7), None);
    cx.update(|cx| assert_eq!(picks_of(7, cx).as_deref(), Some("")));
    let found = names("new", cx);
    assert!(before(&found, "New Folder…", "New Window"), "{found:?}");
    assert_eq!(switched(&shell, 11, cx), Some(0.0));
}

/// A page deleted from the History page or the command bar takes the switches to the tabs at it,
/// and leaves the rest.
#[gpui::test]
fn deleting_a_page_forgets_the_switches_to_its_tabs(cx: &mut TestAppContext) {
    let (shell, host, _folders) = window(cx);
    learn(&shell, cx);
    let urls = vec!["https://11.test/".to_string()];
    let deleted = HistoryDeleted::Some {
        begin: 0,
        end: 0,
        urls,
    };
    cx.update(|cx| history_deleted(7, deleted, cx));
    assert_eq!(switched(&shell, 11, cx), Some(0.0));
    assert!(switched(&shell, 12, cx).is_some_and(|score| score > 0.0));
    // A pick names no page, so it stays.
    assert!(before(&names("new", cx), "New Window", "New Folder…"));
    let kept = file_of(&host, 7).unwrap_or_default();
    assert!(kept.starts_with("New Window\t"), "{kept}");
}

/// Clear Browsing Data's Last hour takes what was learned in that hour, and keeps what came before.
#[gpui::test]
fn deleting_a_span_of_history_forgets_the_uses_in_it(cx: &mut TestAppContext) {
    let (shell, host, _folders) = window(cx);
    let now = usage::now();
    let week_ago = now - 7 * DAY;
    put_file(&host, 7, &format!("New Folder…\t{week_ago}\n"));
    learn(&shell, cx);
    let deleted = HistoryDeleted::Some {
        begin: now - 3600,
        end: u64::MAX,
        urls: Vec::new(),
    };
    cx.update(|cx| history_deleted(7, deleted, cx));
    let kept = file_of(&host, 7).unwrap_or_default();
    assert_eq!(kept, format!("New Folder…\t{week_ago}\n"));
    assert!(before(&names("new", cx), "New Folder…", "New Window"));
    assert_eq!(switched(&shell, 11, cx), Some(0.0));
}

#[gpui::test]
fn switches_rank_a_tab_until_it_closes(cx: &mut TestAppContext) {
    let (shell, _, _folders) = window(cx);
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
    let (shell, host, _folders) = window(cx);
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
    assert_eq!(file_of(&host, 7), None);
    cx.update(|cx| assert_eq!(picks_of(7, cx), None));
    let after = shell.read_with(cx, |shell, cx| shell.open_tabs(cx));
    let scores = |tabs: &[crate::shell::BarTab]| -> Vec<(u64, u32)> {
        tabs.iter()
            .map(|tab| (tab.tab, tab.switched.round() as u32))
            .collect()
    };
    assert_eq!(scores(&before), scores(&after));
}

/// The picks all profiles shared in one file move into each profile's folder once, by name,
/// and the old file goes. A profile that has its own file keeps it.
#[test]
fn the_old_shared_file_moves_into_each_profiles_folder() {
    let folders = TestFolders::new();
    let old = folders.0.join(PICKS_FILE);
    let text = "Work\tNew Window\t5\nPersonal\tNew Folder…\t6\nWork\tNew Tab\t7\nGone\tX\t8\n";
    std::fs::write(&old, text).unwrap();
    let (work, personal) = (folders.folder("Default"), folders.folder("Profile 1"));
    std::fs::write(personal.join(PICKS_FILE), "Mine\t9\n").unwrap();
    let profiles = [
        ("Work".to_string(), work.clone()),
        ("Personal".to_string(), personal.clone()),
    ];
    move_old_picks(&old, &profiles);
    let read = |folder: &PathBuf| std::fs::read_to_string(folder.join(PICKS_FILE)).unwrap();
    assert_eq!(read(&work), "New Window\t5\nNew Tab\t7\n");
    assert_eq!(read(&personal), "Mine\t9\n");
    assert!(!old.exists());
}
