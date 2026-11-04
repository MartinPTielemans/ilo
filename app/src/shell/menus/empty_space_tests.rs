//! A right-click on the sidebar's empty space shows New Tab and the menu of the Space whose page
//! is under the pointer, in a window, on a host that records what the shell asks of the core;
//! rows keep their own menus (#433).

use std::{rc::Rc, time::Duration};

use gpui::{
    Entity, Modifiers, MouseButton, ScrollDelta, ScrollEvent, TestAppContext, TouchPhase,
    VisualTestContext, point, px,
};

use crate::shell::{
    DIVIDER_HEIGHT, Folders, NAME_HEIGHT, ROW_PITCH, Shell, Slot, recorder::Recorder,
};

/// A window on Spaces 1 and 2, showing 1. Space 1 has pinned tab 10 and Today tab 11, and Space 2
/// has `second`'s Today tabs.
fn window(
    cx: &mut TestAppContext,
    second: Vec<u64>,
) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = |id: u64| {
            (
                id,
                format!("Space {id}").into(),
                None,
                1,
                "".into(),
                "".into(),
                "".into(),
            )
        };
        shell.set_spaces(vec![space(1), space(2)], cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, vec![10], vec![11], Vec::new(), Folders::default(), cx);
        shell.set_tabs(2, Vec::new(), second, Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    host.take();
    (shell, cx, host)
}

/// Where in the window `slot` of the Space shown starts, `x` in from the sidebar's edge.
fn at_slot(shell: &Entity<Shell>, cx: &mut VisualTestContext, slot: Slot, x: f32) -> (f32, f32) {
    shell.read_with(cx, |shell, _| {
        (x, shell.drawn_list_top() + shell.layout()[&slot])
    })
}

/// Where in the window the sidebar's empty space is, under the Space's last row.
fn under_rows(shell: &Entity<Shell>, cx: &mut VisualTestContext, x: f32) -> (f32, f32) {
    shell.read_with(cx, |shell, _| {
        let last = shell.layout().into_values().fold(0., f32::max);
        (x, shell.drawn_list_top() + last + ROW_PITCH + 40.)
    })
}

/// Right-clicks at `(x, y)` and returns the titles of each menu it showed, lines as "—".
fn right_click(
    cx: &mut VisualTestContext,
    host: &Recorder,
    (x, y): (f32, f32),
) -> Vec<Vec<String>> {
    host.menus.borrow_mut().clear();
    cx.simulate_mouse_down(point(px(x), px(y)), MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(point(px(x), px(y)), MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    let menus = host.menus.borrow();
    let title = |title: &str| {
        if title.is_empty() {
            "—".to_string()
        } else {
            title.to_string()
        }
    };
    menus
        .iter()
        .map(|menu| menu.iter().map(|(each, _)| title(each)).collect())
        .collect()
}

/// Space 1's menu with New Tab before it: the menu a right-click on its empty space shows.
fn space_one_menu(menu: &[String]) -> bool {
    let head = [
        "New Tab",
        "New Folder…",
        "—",
        "Change Space Icon…",
        "Rename Space…",
        "Edit Theme Color…",
        "Set Profile",
    ];
    menu.starts_with(&head.map(String::from))
        && menu.last().is_some_and(|last| last == "Delete Space")
}

/// Chooses New Tab in the last menu, and returns the Spaces the shell asked the core to show.
fn new_tab(cx: &mut VisualTestContext, host: &Recorder) -> Vec<u64> {
    host.shown.borrow_mut().clear();
    host.command_bars.borrow_mut().clear();
    cx.update(|_, cx| host.choose("New Tab", cx));
    cx.run_until_parked();
    assert_eq!(
        host.command_bars.borrow().len(),
        1,
        "New Tab opened no command bar"
    );
    host.shown.borrow().clone()
}

/// Under the last row and in the gap between pinned and Today, a right-click shows New Tab, New
/// Folder…, a line and the Space's menu as its name's shows it, its Profile submenu under its own
/// item, and New Tab opens the command bar as ⌘T does, in the Space shown.
#[gpui::test]
fn a_right_click_on_empty_space_shows_new_tab_and_the_spaces_menu(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, vec![20]);
    let under = under_rows(&shell, cx, 100.);
    let menus = right_click(cx, &host, under);
    assert_eq!(menus.len(), 1, "{menus:?}");
    assert!(space_one_menu(&menus[0]), "{menus:?}");
    let entries = host.entries.borrow().clone();
    assert!(entries[0].shortcut.is_some(), "New Tab shows ⌘T");
    let profile = entries
        .iter()
        .position(|entry| entry.title == "Set Profile")
        .unwrap();
    let mut parents = entries.iter().filter_map(|entry| entry.parent);
    assert!(
        parents.all(|parent| parent == profile),
        "the Set Profile submenu moved"
    );
    assert!(
        new_tab(cx, &host).is_empty(),
        "New Tab showed another Space"
    );

    let (x, top) = at_slot(&shell, cx, Slot::Divider, 30.);
    let menus = right_click(cx, &host, (x, top + DIVIDER_HEIGHT / 2.));
    assert_eq!(menus.len(), 1, "{menus:?}");
    assert!(
        space_one_menu(&menus[0]),
        "the gap between pinned and Today: {menus:?}"
    );
}

/// A Space with no tabs is all empty space under its New Tab row, and it's that Space's menu.
#[gpui::test]
fn an_empty_spaces_empty_space_shows_its_menu(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, Vec::new());
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    shell.update(cx, |shell, _| {
        if let Some(settle) = shell.settle.as_mut() {
            settle.start -= Duration::from_secs(1);
        }
    });
    cx.run_until_parked();
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let (_, new_tab_row) = at_slot(&shell, cx, Slot::NewTab, 100.);
    let menus = right_click(cx, &host, (100., new_tab_row + ROW_PITCH + 20.));
    assert_eq!(menus.len(), 1, "{menus:?}");
    assert_eq!(
        menus[0][..4],
        ["New Tab", "New Folder…", "—", "Change Space Icon…"]
    );
    // Delete Space is on: Space 1 is left.
    assert!(
        host.menus.borrow()[0]
            .last()
            .is_some_and(|(title, on)| title == "Delete Space" && *on)
    );
    assert!(
        new_tab(cx, &host).is_empty(),
        "New Tab showed another Space"
    );
}

/// Mid-swipe, with the second Space's page coming in beside the first's, a right-click is for the
/// Space whose page is under the pointer, and its New Tab opens in that Space. Mid-slide after a
/// switch, where the incoming page isn't yet, it's for the Space switched to (#240).
#[gpui::test]
fn a_right_click_mid_swipe_is_for_the_space_under_the_pointer(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, vec![20]);
    let fingers = |touch_phase, dx: f32| ScrollEvent {
        position: point(px(110.), px(500.)),
        delta: ScrollDelta::Pixels(point(px(dx), px(0.))),
        touch_phase,
        ..Default::default()
    };
    cx.simulate_event(fingers(TouchPhase::Started, 0.));
    for _ in 0..10 {
        cx.simulate_event(fingers(TouchPhase::Moved, -10.));
    }
    cx.run_until_parked();
    let pages = shell.read_with(cx, |shell, _| shell.pages());
    let width = shell.read_with(cx, |shell, _| shell.width());
    assert!(
        pages.len() == 2 && pages[0].1 < 0.,
        "the second Space isn't coming in: {pages:?}"
    );

    let beside = under_rows(&shell, cx, width - 20.);
    let menus = right_click(cx, &host, beside);
    assert_eq!(menus.len(), 1, "{menus:?}");
    assert_eq!(
        menus[0][..4],
        ["New Tab", "New Folder…", "—", "Change Space Icon…"]
    );
    assert_eq!(new_tab(cx, &host), [2], "New Tab went to the Space beside");

    let first = under_rows(&shell, cx, 20.);
    let menus = right_click(cx, &host, first);
    assert_eq!(menus.len(), 1, "{menus:?}");
    assert!(
        new_tab(cx, &host).is_empty(),
        "New Tab left the first Space"
    );
    cx.simulate_event(fingers(TouchPhase::Cancelled, 0.));
    cx.run_until_parked();

    // ⌃2: Space 2's page slides in from the right over Space 1's.
    host.shown.borrow_mut().clear();
    shell.update(cx, |shell, cx| shell.set_space(2, cx));
    cx.run_until_parked();
    assert!(
        shell.read_with(cx, |shell, _| shell.settle.is_some()),
        "the Space didn't slide"
    );
    let first = under_rows(&shell, cx, 20.);
    let menus = right_click(cx, &host, first);
    assert_eq!(menus.len(), 1, "a right-click mid-slide: {menus:?}");
    assert_eq!(
        menus[0][..4],
        ["New Tab", "New Folder…", "—", "Change Space Icon…"]
    );
    assert!(
        new_tab(cx, &host).is_empty(),
        "New Tab mid-slide went to Space 1"
    );
}

/// A row, the Space's name and the New Tab row keep what a right-click on them did: a tab's menu,
/// the Space's menu without New Tab, and nothing.
#[gpui::test]
fn rows_keep_their_own_menus(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx, vec![20]);
    let (x, top) = at_slot(&shell, cx, Slot::Tab(11), 100.);
    let menus = right_click(cx, &host, (x, top + ROW_PITCH / 2.));
    assert_eq!(menus.len(), 1, "{menus:?}");
    assert_eq!(menus[0][0], "Copy Link", "{menus:?}");

    let name = shell.read_with(cx, |shell, _| shell.drawn_list_top() + NAME_HEIGHT / 2.);
    let menus = right_click(cx, &host, (60., name));
    assert_eq!(menus.len(), 1, "{menus:?}");
    assert_eq!(menus[0][0], "Change Space Icon…", "{menus:?}");

    let (x, top) = at_slot(&shell, cx, Slot::NewTab, 100.);
    assert!(right_click(cx, &host, (x, top + ROW_PITCH / 2.)).is_empty());
}
