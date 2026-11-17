//! ⌃Tab's walk and its switcher (#2954) on a host that records what the shell asks: a quick ⌃Tab
//! is today's one switch with nothing shown, and a held one shows the Space's tabs most recently
//! shown first and switches at most once more.

use std::rc::Rc;
use std::time::Duration;

use gpui::{AppContext, Entity, TestAppContext};

use crate::shell::{Command, Folders, Page, Shell, recorder::Recorder};
use crate::tab_switcher::{DELAY, Heard};

/// A window whose Space has the Today tabs 10 to 13, shown in that order, so 13 shows and the walk
/// goes 12, 11, 10.
fn walked_window(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![(1, "One".into(), None, 1, "".into(), "".into(), "".into())],
            cx,
        );
        shell.set_space(1, cx);
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 11, 12, 13],
            Vec::new(),
            Folders::default(),
            cx,
        );
        for tab in [10, 11, 12, 13] {
            let url = format!("https://site{tab}.example/").into();
            shell.set_page(
                tab,
                Page {
                    url,
                    ..Page::default()
                },
                cx,
            );
            shell.set_selected(tab, cx);
        }
    });
    host.take();
    (shell, host)
}

fn run(shell: &Entity<Shell>, cx: &mut TestAppContext, command: Command) {
    shell.update(cx, |shell, cx| shell.run(command, "page", cx));
}

/// The core shows `tab`, as it does after a select.
fn shows(shell: &Entity<Shell>, cx: &mut TestAppContext, tab: u64) {
    shell.update(cx, |shell, cx| shell.set_selected(tab, cx));
}

fn release(shell: &Entity<Shell>, cx: &mut TestAppContext) {
    shell.update(cx, |shell, cx| shell.control_released(cx));
    cx.run_until_parked();
}

fn wait(cx: &mut TestAppContext, time: Duration) {
    cx.executor().advance_clock(time);
    cx.run_until_parked();
}

/// What the switcher's user does: a key, a press or leaving it.
fn hear(host: &Rc<Recorder>, cx: &mut TestAppContext, heard: Heard) {
    let show = host
        .tab_switcher
        .borrow()
        .clone()
        .expect("no switcher shown");
    cx.update(|cx| (show.heard)(heard, cx));
    cx.run_until_parked();
}

/// What the shell asked of the walk: switches, the switcher and the watch on ⌃, without what
/// else it asks meanwhile, such as the hover cards' pictures.
fn asked(host: &Rc<Recorder>) -> Vec<String> {
    let walk = ["watch control", "select ", "tab switcher "];
    (host.take().into_iter())
        .filter(|line| walk.iter().any(|each| line.starts_with(each)))
        .collect()
}

fn recent(shell: &Entity<Shell>, cx: &mut TestAppContext) -> Vec<u64> {
    shell.read_with(cx, |shell, _| shell.recent.clone())
}

#[gpui::test]
fn a_quick_ctrl_tab_switches_once_and_shows_nothing(cx: &mut TestAppContext) {
    let (shell, host) = walked_window(cx);
    run(&shell, cx, Command::RecentTab);
    shows(&shell, cx, 12);
    release(&shell, cx);
    wait(cx, DELAY * 3);
    assert_eq!(asked(&host), ["watch control", "select 12"]);
    // The tab it went to counts as shown, so the next ⌃Tab goes back.
    assert_eq!(recent(&shell, cx)[..2], [12, 13]);
    run(&shell, cx, Command::RecentTab);
    assert_eq!(asked(&host), ["watch control", "select 13"]);
}

#[gpui::test]
fn held_past_the_delay_it_lists_the_tabs_with_the_one_shown_selected(cx: &mut TestAppContext) {
    let (shell, host) = walked_window(cx);
    run(&shell, cx, Command::RecentTab);
    shows(&shell, cx, 12);
    wait(cx, DELAY / 2);
    assert_eq!(asked(&host), ["watch control", "select 12"]);
    wait(cx, DELAY);
    assert_eq!(asked(&host), ["tab switcher show 13 12 11 10 at 1"]);
    // A walk's tabs don't count as shown while ⌃ is held.
    assert_eq!(recent(&shell, cx), [13, 12, 11, 10]);
}

#[gpui::test]
fn a_second_ctrl_tab_before_the_delay_shows_it_at_once_and_the_page_stays(cx: &mut TestAppContext) {
    let (shell, host) = walked_window(cx);
    run(&shell, cx, Command::RecentTab);
    shows(&shell, cx, 12);
    run(&shell, cx, Command::RecentTab);
    assert_eq!(
        asked(&host),
        [
            "watch control",
            "select 12",
            "tab switcher show 13 12 11 10 at 2"
        ]
    );
    // Its delay, when it comes, shows nothing more.
    wait(cx, DELAY * 2);
    assert_eq!(asked(&host), Vec::<String>::new());
}

#[gpui::test]
fn its_keys_move_the_selection_wrapping_and_letting_go_switches_once(cx: &mut TestAppContext) {
    let (shell, host) = walked_window(cx);
    run(&shell, cx, Command::RecentTab);
    shows(&shell, cx, 12);
    wait(cx, DELAY);
    host.take();
    hear(&host, cx, Heard::Step(true));
    hear(&host, cx, Heard::Step(true));
    hear(&host, cx, Heard::Step(true));
    hear(&host, cx, Heard::Step(false));
    // ⌃Tab from the page, before the switcher has the keyboard, moves it as its own keys do.
    run(&shell, cx, Command::RecentTabBack);
    assert_eq!(
        asked(&host),
        [
            "tab switcher select 2",
            "tab switcher select 3",
            "tab switcher select 0",
            "tab switcher select 3",
            "tab switcher select 2",
        ]
    );
    release(&shell, cx);
    assert_eq!(asked(&host), ["tab switcher close", "select 11"]);
    shows(&shell, cx, 11);
    assert_eq!(recent(&shell, cx)[..2], [11, 13]);
}

#[gpui::test]
fn letting_go_on_the_tab_shown_switches_no_more(cx: &mut TestAppContext) {
    let (shell, host) = walked_window(cx);
    run(&shell, cx, Command::RecentTab);
    shows(&shell, cx, 12);
    wait(cx, DELAY);
    host.take();
    release(&shell, cx);
    assert_eq!(asked(&host), ["tab switcher close"]);
    assert_eq!(recent(&shell, cx)[..2], [12, 13]);
}

#[gpui::test]
fn escape_and_a_click_outside_go_back_to_where_the_walk_started(cx: &mut TestAppContext) {
    let (shell, host) = walked_window(cx);
    run(&shell, cx, Command::RecentTab);
    shows(&shell, cx, 12);
    wait(cx, DELAY);
    hear(&host, cx, Heard::Step(true));
    host.take();
    hear(&host, cx, Heard::Stay);
    assert_eq!(asked(&host), ["tab switcher close", "select 13"]);
    shows(&shell, cx, 13);
    // ⌃ going up after shows nothing more.
    release(&shell, cx);
    assert_eq!(asked(&host), Vec::<String>::new());
    assert_eq!(recent(&shell, cx)[..2], [13, 12]);
}

#[gpui::test]
fn a_click_outside_that_also_ends_the_walk_still_stays(cx: &mut TestAppContext) {
    // On the Mac the press ends the walk and dismisses the switcher, each heard after it, in
    // either order.
    for release_first in [true, false] {
        let (shell, host) = walked_window(cx);
        run(&shell, cx, Command::RecentTab);
        shows(&shell, cx, 12);
        wait(cx, DELAY);
        hear(&host, cx, Heard::Step(true));
        host.take();
        let show = host.tab_switcher.borrow().clone().unwrap();
        cx.update(|cx| {
            if release_first {
                shell.update(cx, |shell, cx| shell.control_released(cx));
                (show.heard)(Heard::Stay, cx);
            } else {
                (show.heard)(Heard::Stay, cx);
                shell.update(cx, |shell, cx| shell.control_released(cx));
            }
        });
        cx.run_until_parked();
        assert_eq!(
            asked(&host),
            ["tab switcher close", "select 13"],
            "{release_first}"
        );
    }
}

/// A press on a sidebar row while the switcher shows selects the row's tab during the press. On the
/// Mac the press also reports ⌃ up, and the switcher's dismissal comes after it, so three things
/// arrive in an order the host doesn't fix (#3114). Neither ⌃ up nor the dismissal undoes the
/// row's tab: heard after the row's select, in either order, the walk ends on the row's tab; a
/// dismissal heard before it goes back to where the walk started, and the row's tab shows after.
#[gpui::test]
fn a_click_outside_on_a_sidebar_row_keeps_the_tab_it_showed(cx: &mut TestAppContext) {
    for release_first in [true, false] {
        let (shell, host) = walked_window(cx);
        run(&shell, cx, Command::RecentTab);
        shows(&shell, cx, 12);
        wait(cx, DELAY);
        host.take();
        let show = host.tab_switcher.borrow().clone().unwrap();
        // The press's ⌃ up, then its select of the row's tab, during its event.
        cx.update(|cx| {
            shell.update(cx, |shell, cx| {
                shell.control_released(cx);
                shell.select(10, cx);
            });
            if !release_first {
                (show.heard)(Heard::Stay, cx);
            }
        });
        cx.run_until_parked();
        if release_first {
            hear(&host, cx, Heard::Stay);
        }
        assert_eq!(
            asked(&host),
            ["select 10", "tab switcher close"],
            "release first {release_first}"
        );
        shows(&shell, cx, 10);
        assert_eq!(recent(&shell, cx)[..2], [10, 13], "{release_first}");
    }

    // The dismissal before the row's select.
    let (shell, host) = walked_window(cx);
    run(&shell, cx, Command::RecentTab);
    shows(&shell, cx, 12);
    wait(cx, DELAY);
    host.take();
    hear(&host, cx, Heard::Stay);
    shell.update(cx, |shell, cx| {
        shell.control_released(cx);
        shell.select(10, cx);
    });
    cx.run_until_parked();
    assert_eq!(
        asked(&host),
        ["tab switcher close", "select 13", "select 10"]
    );
    shows(&shell, cx, 13);
    shows(&shell, cx, 10);
    assert_eq!(recent(&shell, cx)[..2], [10, 13]);
}

#[gpui::test]
fn a_press_on_a_row_shows_its_tab_and_leaving_the_app_lets_go(cx: &mut TestAppContext) {
    let (shell, host) = walked_window(cx);
    run(&shell, cx, Command::RecentTab);
    shows(&shell, cx, 12);
    wait(cx, DELAY);
    host.take();
    hear(&host, cx, Heard::Chose(3));
    assert_eq!(asked(&host), ["tab switcher close", "select 10"]);
    shows(&shell, cx, 10);
    release(&shell, cx);
    assert_eq!(asked(&host), Vec::<String>::new());

    run(&shell, cx, Command::RecentTab);
    shows(&shell, cx, 13);
    wait(cx, DELAY);
    hear(&host, cx, Heard::Step(true));
    host.take();
    hear(&host, cx, Heard::LetGo);
    assert_eq!(asked(&host), ["tab switcher close", "select 12"]);
}

#[gpui::test]
fn ctrl_shift_tab_starts_from_the_tab_shown_longest_ago(cx: &mut TestAppContext) {
    let (shell, host) = walked_window(cx);
    run(&shell, cx, Command::RecentTabBack);
    shows(&shell, cx, 10);
    wait(cx, DELAY);
    assert_eq!(
        asked(&host),
        [
            "watch control",
            "select 10",
            "tab switcher show 13 12 11 10 at 3"
        ]
    );
}

#[gpui::test]
fn a_tab_closed_during_the_walk_is_not_listed(cx: &mut TestAppContext) {
    let (shell, host) = walked_window(cx);
    run(&shell, cx, Command::RecentTab);
    shows(&shell, cx, 12);
    shell.update(cx, |shell, cx| {
        shell.set_tabs(
            1,
            Vec::new(),
            vec![10, 12, 13],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.tabs.remove(&11);
    });
    host.take();
    wait(cx, DELAY);
    assert_eq!(asked(&host), ["tab switcher show 13 12 10 at 1"]);
}

#[gpui::test]
fn a_press_on_a_row_counts_only_its_tab_as_shown(cx: &mut TestAppContext) {
    let (shell, host) = walked_window(cx);
    run(&shell, cx, Command::RecentTab);
    shows(&shell, cx, 12);
    wait(cx, DELAY);
    hear(&host, cx, Heard::Chose(3));
    // ⌃ goes up before the core has shown the tab chosen.
    release(&shell, cx);
    shows(&shell, cx, 10);
    // The walk's 12 was passed through, never chosen.
    assert_eq!(recent(&shell, cx), [10, 13, 12, 11]);
    host.take();
    run(&shell, cx, Command::RecentTabBack);
    assert_eq!(asked(&host), ["watch control", "select 11"]);
}
