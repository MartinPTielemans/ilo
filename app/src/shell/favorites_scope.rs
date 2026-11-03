//! The favorites' scope in the UI: Settings' choices, and the tiles a Space shows under each scope,
//! on a host that records what the shell asks.

use std::rc::Rc;

use gpui::{AppContext, Entity, TestAppContext};

use crate::shell::{Command, FavoritesScope, Folders, Shell, change_keys, recorder::Recorder};

/// The user's Spaces 1 and 2 in profile 7 and Space 3 in profile 8, and Claude Code's Space 4 in
/// profile 7, each with a Today tab. Space 1 shows.
fn shell_with_spaces(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "Space 1".into(), None, 7, "".into(), "".into(), "".into()),
                (2, "Space 2".into(), None, 7, "".into(), "".into(), "".into()),
                (3, "Space 3".into(), None, 8, "".into(), "".into(), "".into()),
                (4, "Research".into(), None, 7, "".into(), "Claude Code".into(), "".into()),
            ],
            cx,
        );
        for (space, tab) in [(1, 11), (2, 12), (3, 13), (4, 14)] {
            shell.set_tabs(space, Vec::new(), vec![tab], Vec::new(), Folders::default(), cx);
        }
        shell.set_space(1, cx);
    });
    (shell, host)
}

fn favorites(shell: &Entity<Shell>, cx: &mut TestAppContext) -> Vec<u64> {
    shell.read_with(cx, |shell, _| shell.favorites().to_vec())
}

/// What ⌘1 asks the core to show in `space`, the first tile if it has any.
fn first_tab(shell: &Entity<Shell>, host: &Recorder, cx: &mut TestAppContext, space: u64) -> String {
    shell.update(cx, |shell, cx| shell.set_space(space, cx));
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::SelectTab(0), "ui", cx));
    host.take().join(", ")
}

/// Settings lists the three choices in the order the core numbers them, titled as the owner asked,
/// and a number the core doesn't know is the default, Arc's.
#[test]
fn the_choices_are_listed_as_the_core_numbers_them() {
    let titles: Vec<_> = FavoritesScope::ALL.iter().map(|scope| scope.title()).collect();
    assert_eq!(titles, ["In Every Space", "In Each Profile's Spaces", "In Each Space"]);
    for (number, scope) in FavoritesScope::ALL.into_iter().enumerate() {
        assert_eq!(scope.to_core(), number as i32);
        assert_eq!(FavoritesScope::from_core(number as i32), scope);
    }
    assert_eq!(FavoritesScope::default(), FavoritesScope::Profile);
    assert_eq!(FavoritesScope::from_core(9), FavoritesScope::Profile);
    assert_eq!(FavoritesScope::from_core(-1), FavoritesScope::Profile);
}

/// Where the core keeps one list per profile, the Spaces of a profile show the same tiles, another
/// profile's Space shows its own, and an agent's Space shows its profile's, as today.
#[gpui::test]
fn a_profiles_spaces_share_its_favorites(cx: &mut TestAppContext) {
    let (shell, host) = shell_with_spaces(cx);
    shell.update(cx, |shell, cx| shell.set_favorites(vec![(40, 7), (41, 7), (42, 8)], cx));
    assert_eq!(favorites(&shell, cx), [40, 41]);
    assert_eq!(first_tab(&shell, &host, cx, 2), "select 40");
    assert_eq!(favorites(&shell, cx), [40, 41]);
    assert_eq!(first_tab(&shell, &host, cx, 3), "select 42");
    assert_eq!(favorites(&shell, cx), [42]);
    shell.update(cx, |shell, cx| shell.set_space(4, cx));
    assert_eq!(favorites(&shell, cx), [40, 41]);
}

/// Where each Space has its own, a Space shows its own tiles alone, even beside another Space of its
/// profile, ⌘1 asks for the first of the Space shown, and an agent's Space, which owns none, shows
/// none.
#[gpui::test]
fn each_space_shows_its_own_favorites(cx: &mut TestAppContext) {
    let (shell, host) = shell_with_spaces(cx);
    shell.update(cx, |shell, cx| {
        shell.set_favorites(vec![(50, 1), (51, 1), (52, 2), (53, 3)], cx)
    });
    assert_eq!(favorites(&shell, cx), [50, 51]);
    assert_eq!(first_tab(&shell, &host, cx, 1), "select 50");
    assert_eq!(first_tab(&shell, &host, cx, 2), "select 52");
    assert_eq!(favorites(&shell, cx), [52]);
    assert_eq!(first_tab(&shell, &host, cx, 3), "select 53");
    shell.update(cx, |shell, cx| shell.set_space(4, cx));
    assert!(favorites(&shell, cx).is_empty());
}

/// Where every profile has a tab for each site, a Space shows its profile's, so Spaces of two
/// profiles show the same sites in tabs of their own.
#[gpui::test]
fn every_space_shows_the_same_sites_in_each_profiles_tabs(cx: &mut TestAppContext) {
    let (shell, host) = shell_with_spaces(cx);
    shell.update(cx, |shell, cx| {
        shell.set_favorites(vec![(60, 7), (61, 8), (62, 7), (63, 8)], cx)
    });
    assert_eq!(favorites(&shell, cx), [60, 62]);
    assert_eq!(first_tab(&shell, &host, cx, 3), "select 61");
    assert_eq!(favorites(&shell, cx), [61, 63]);
}

/// The core reports the favorites again when the scope changes, and the Space shown swaps its tiles
/// to what the new scope gives it, with ⌘1 asking for the first of those.
#[gpui::test]
fn a_change_of_scope_swaps_the_tiles_of_the_space_shown(cx: &mut TestAppContext) {
    let (shell, host) = shell_with_spaces(cx);
    shell.update(cx, |shell, cx| shell.set_favorites(vec![(40, 7), (41, 7)], cx));
    assert_eq!(favorites(&shell, cx), [40, 41]);
    // Each Space: Space 1 and Space 2 each get a copy, in tabs of their own.
    shell.update(cx, |shell, cx| {
        shell.set_favorites(vec![(40, 1), (41, 1), (70, 2), (71, 2)], cx)
    });
    assert_eq!(favorites(&shell, cx), [40, 41]);
    assert_eq!(first_tab(&shell, &host, cx, 2), "select 70");
    // And back to a profile's: the lists merged, the other tabs closed.
    shell.update(cx, |shell, cx| shell.set_favorites(vec![(40, 7), (41, 7)], cx));
    assert_eq!(favorites(&shell, cx), [40, 41]);
    assert_eq!(first_tab(&shell, &host, cx, 2), "select 40");
}

/// With Include Favorites in ⌘1–⌘8 turned off (Settings › Shortcuts), ⌘1 is the Space's first tab
/// after the tiles, and a number past its tabs asks for nothing. Turned on again, ⌘1 is the first
/// tile.
#[gpui::test]
fn the_numbers_can_leave_the_favorites_out(cx: &mut TestAppContext) {
    let (shell, host) = shell_with_spaces(cx);
    shell.update(cx, |shell, cx| shell.set_favorites(vec![(40, 7), (41, 7)], cx));
    assert_eq!(first_tab(&shell, &host, cx, 1), "select 40");
    cx.update(|cx| change_keys(cx, |keys| keys.set_favorites_numbered(false)));
    assert_eq!(first_tab(&shell, &host, cx, 1), "select 11");
    shell.update(cx, |shell, cx| shell.run(Command::SelectTab(1), "ui", cx));
    assert!(host.take().is_empty());
    cx.update(|cx| change_keys(cx, |keys| keys.set_favorites_numbered(true)));
    assert_eq!(first_tab(&shell, &host, cx, 1), "select 40");
}

/// With Last Tab Goes to the Ninth Tab turned on (Settings › Shortcuts), ⌘9 is the ninth tab,
/// counted as ⌘1–⌘8 count, with the favorites or past them, and asks for nothing in a Space with
/// fewer. Turned off, the default, it's the last tab.
#[gpui::test]
fn the_last_tab_can_be_the_ninth(cx: &mut TestAppContext) {
    let (shell, host) = shell_with_spaces(cx);
    let today = |count: u64| (100..100 + count).collect::<Vec<u64>>();
    let ninth = |shell: &Entity<Shell>, cx: &mut TestAppContext| {
        host.take();
        shell.update(cx, |shell, cx| shell.run(Command::SelectLastTab, "page", cx));
        host.take().join(", ")
    };
    shell.update(cx, |shell, cx| {
        shell.set_favorites(vec![(40, 7), (41, 7)], cx);
        shell.set_tabs(1, Vec::new(), today(10), Vec::new(), Folders::default(), cx);
    });
    assert_eq!(ninth(&shell, cx), "select 109");
    cx.update(|cx| change_keys(cx, |keys| keys.set_last_tab_ninth(true)));
    // The two favorites, then the Space's first seven tabs.
    assert_eq!(ninth(&shell, cx), "select 106");
    cx.update(|cx| change_keys(cx, |keys| keys.set_favorites_numbered(false)));
    assert_eq!(ninth(&shell, cx), "select 108");
    shell.update(cx, |shell, cx| {
        shell.set_tabs(1, Vec::new(), today(5), Vec::new(), Folders::default(), cx);
    });
    assert_eq!(ninth(&shell, cx), "");
    cx.update(|cx| change_keys(cx, |keys| keys.reset_all()));
    assert_eq!(ninth(&shell, cx), "select 104");
}
