//! Sort Today Tabs by Site (#2344) in a window, on a host that records what the shell asks of the
//! core; the test plays the core's moves back as its reports, as the core does a moment after each.

use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::recorder::Recorder;
use super::{Command, Folders, Page, Shell, Split};

/// Today's tabs and their pages: two sites with three tabs, one with two, one with one, a split
/// view (14 and 15) and a page off the web. 20's page is a subdomain of 10's site.
const TABS: &[(u64, &str)] = &[
    (10, "https://github.com/ilo/ilo"),
    (11, "https://en.wikipedia.org/wiki/Arc"),
    (12, "https://news.ycombinator.com/"),
    (13, "https://github.com/ilo/ilo/pulls"),
    (14, "https://example.com/a"),
    (15, "https://github.com/x"),
    (16, "https://en.wikipedia.org/wiki/Chromium"),
    (17, "about:blank"),
    (18, "https://www.rust-lang.org/"),
    (19, "https://news.ycombinator.com/item?id=1"),
    (20, "https://docs.github.com/en"),
];

const SPLIT: [u64; 2] = [14, 15];

fn split() -> Vec<Split> {
    vec![Split {
        tabs: SPLIT.to_vec(),
        ..Default::default()
    }]
}

/// A window showing Space 1, with `TABS` as its Today tabs in order and 16 shown.
fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        let today = TABS.iter().map(|&(tab, _)| tab).collect();
        shell.set_tabs(1, Vec::new(), today, split(), Folders::default(), cx);
        for &(tab, url) in TABS {
            let page = Page {
                url: url.into(),
                ..Page::default()
            };
            shell.set_page(tab, page, cx);
        }
        shell.set_selected(16, cx);
    });
    cx.run_until_parked();
    host.take();
    (shell, cx, host)
}

fn today(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<u64> {
    shell.read_with(cx, |shell, _| shell.today().to_vec())
}

/// Plays the moves the shell asked of the core since the last, a tab with its split as the core
/// moves it, and reports Today's tabs as they are then. Returns how many there were; the shell
/// asked nothing else.
fn play(shell: &Entity<Shell>, cx: &mut VisualTestContext, host: &Recorder) -> usize {
    let mut tabs = today(shell, cx);
    let asked = host.take_edits();
    for edit in &asked {
        let words: Vec<&str> = edit.split(' ').collect();
        let ["move", tab, "false", "0", index] = words[..] else {
            panic!("asked the core for more than moves: {asked:?}");
        };
        let (tab, index): (u64, usize) = (tab.parse().unwrap(), index.parse().unwrap());
        let panes = if SPLIT.contains(&tab) {
            SPLIT.to_vec()
        } else {
            vec![tab]
        };
        tabs.retain(|tab| !panes.contains(tab));
        tabs.splice(index..index, panes);
    }
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), tabs, split(), Folders::default(), cx)
    });
    cx.run_until_parked();
    asked.len()
}

const BEFORE: [u64; 11] = [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20];
/// github.com's three at 10's place, wikipedia.org's two at 11's, ycombinator.com's at 12's; the
/// split, the blank page and rust-lang.org's one tab keep their order among them.
const AFTER: [u64; 11] = [10, 13, 20, 11, 16, 12, 19, 14, 15, 17, 18];

#[gpui::test]
fn each_site_gathers_at_its_first_tab_and_the_tab_shown_stays(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    shell.update(cx, |shell, cx| shell.run(Command::SortBySite, "ui", cx));
    assert!(play(&shell, cx, &host) > 0);
    assert_eq!(today(&shell, cx), AFTER);
    assert_eq!(shell.read_with(cx, |shell, _| shell.selected), 16);
    // Nothing else: no tab loaded, unloaded, closed or shown.
    assert!(host.take().iter().all(|call| !call.starts_with("select")));
}

#[gpui::test]
fn undo_puts_every_row_back_and_redo_sorts_again(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    shell.update(cx, |shell, cx| shell.run(Command::SortBySite, "ui", cx));
    play(&shell, cx, &host);
    let titles = shell.read_with(cx, |shell, _| shell.history.titles());
    assert_eq!(titles, (Some("Sort by Site"), None), "Undo Sort by Site");

    assert!(shell.update(cx, |shell, cx| shell.undo(cx)));
    play(&shell, cx, &host);
    assert_eq!(today(&shell, cx), BEFORE);
    assert!(shell.update(cx, |shell, cx| shell.redo(cx)));
    play(&shell, cx, &host);
    assert_eq!(today(&shell, cx), AFTER);
}

/// A tab opened after the sort keeps its place when it's undone, and one closed since is left out.
#[gpui::test]
fn undo_leaves_a_tab_opened_since_where_it_is(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    shell.update(cx, |shell, cx| shell.run(Command::SortBySite, "ui", cx));
    play(&shell, cx, &host);
    let mut now = vec![99];
    now.extend(AFTER.iter().filter(|&&tab| tab != 19));
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), now, split(), Folders::default(), cx)
    });
    shell.update(cx, |shell, cx| shell.undo(cx));
    play(&shell, cx, &host);
    let mut back = vec![99];
    back.extend(BEFORE.iter().filter(|&&tab| tab != 19));
    assert_eq!(today(&shell, cx), back);
}

/// With every site's tabs together already, it's dimmed, and run anyway it does nothing and takes
/// no ⌘Z step.
#[gpui::test]
fn it_is_dimmed_when_it_would_change_nothing(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    let offers = |shell: &Entity<Shell>, cx: &mut VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.offers(Command::SortBySite))
    };
    assert!(offers(&shell, cx));
    shell.update(cx, |shell, cx| shell.run(Command::SortBySite, "ui", cx));
    play(&shell, cx, &host);
    assert!(!offers(&shell, cx));
    shell.update(cx, |shell, cx| shell.run(Command::SortBySite, "ui", cx));
    assert_eq!(play(&shell, cx, &host), 0);
    shell.update(cx, |shell, cx| shell.undo(cx));
    play(&shell, cx, &host);
    assert_eq!(today(&shell, cx), BEFORE, "one ⌘Z undid the one sort");
}

/// It's in the Tabs menu, the command bar and the divider's menu, which lead with Today's own two
/// commands.
#[gpui::test]
fn the_divider_menu_leads_with_todays_commands(cx: &mut TestAppContext) {
    let (shell, cx, host) = shell(cx);
    let names: Vec<&str> = super::commands::BAR_COMMANDS
        .iter()
        .map(|&(name, _, _)| name)
        .collect();
    assert!(names.contains(&"Sort Today Tabs by Site"));
    shell.update(cx, |shell, cx| {
        shell.divider_menu(gpui::point(gpui::px(100.), gpui::px(200.)), cx)
    });
    let entries = host.entries.borrow().clone();
    let titles: Vec<&str> = entries.iter().map(|entry| entry.title.as_ref()).collect();
    assert_eq!(
        titles[..4],
        ["Sort Today Tabs by Site", "Clear Today", "", "New Tab"]
    );
    cx.update(|_, cx| host.choose("Sort Today Tabs by Site", cx));
    play(&shell, cx, &host);
    assert_eq!(today(&shell, cx), AFTER);
}
