//! A Blank Window (⌃⌘N, Arc's): one Space of its own, unsaved and in no other window, named for
//! what it is with the square glyph before the name, with no favorites, Space switcher or
//! Library, and its tabs' way out to one of the user's Spaces of its profile (#669).

use std::rc::Rc;

use gpui::{AppContext, Entity, TestAppContext, point, px};

use crate::shell::{BLANK_ICON, BLANK_WINDOW, Command, Folders, Shell, recorder::Recorder};
use crate::space_icon::SpaceIcon;

fn blank(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let shell = cx.new(|cx| Shell::blank_window(browser, 9, 1, None, cx));
    shell.update(cx, |shell, cx| {
        let work = (1, "Work".into(), None, 1, "".into(), "".into(), "".into());
        let other = (
            2,
            "Other profile".into(),
            None,
            2,
            "".into(),
            "".into(),
            "".into(),
        );
        let agent = (
            3,
            "Agent".into(),
            None,
            1,
            "".into(),
            "".into(),
            "claude".into(),
        );
        shell.set_spaces(vec![work, other, agent], cx);
        shell.set_tabs(
            9,
            Vec::new(),
            vec![90, 91],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_selected(90, cx);
    });
    (shell, host)
}

/// Its one Space is its own, named Blank Window with the square glyph before the name, and the
/// user's Spaces don't come into it.
#[gpui::test]
fn a_blank_window_shows_its_own_space_alone(cx: &mut TestAppContext) {
    let (shell, host) = blank(cx);
    shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.spaces.len(),
            1,
            "the user's Spaces came into the Blank Window"
        );
        let space = &shell.spaces[0];
        assert_eq!((space.id, space.name.as_ref()), (9, BLANK_WINDOW));
        assert_eq!(space.icon.as_ref(), BLANK_ICON);
        assert!(matches!(SpaceIcon::parse(&space.icon), SpaceIcon::Glyph(_)));
        assert!(shell.one_space() && !shell.private());
    });
    assert!(
        host.menu_bar.borrow().spaces.is_empty(),
        "the Spaces menu lists the user's Spaces"
    );
    // No Library, no new Space and no favorites from it.
    shell.update(cx, |shell, cx| shell.run(Command::ViewLibrary, "ui", cx));
    assert!(
        !shell.read_with(cx, |shell, _| shell.library_open),
        "the Library opened"
    );
    shell.update(cx, |shell, cx| shell.set_favorites(vec![(90, 1)], cx));
    assert!(
        shell.read_with(cx, |shell, _| shell.favorites.is_empty()),
        "it took favorites"
    );
}

/// A tab's menu offers Move to › the user's Spaces of its profile, not an agent's or another
/// profile's, and choosing one moves the tab out.
#[gpui::test]
fn a_blank_windows_tab_moves_out_to_a_space(cx: &mut TestAppContext) {
    let (shell, host) = blank(cx);
    shell.update(cx, |shell, cx| {
        shell.tab_menu(90, point(px(100.), px(200.)), cx)
    });
    let shown = crate::menu::described(&host.entries.borrow());
    assert!(shown.contains("Move to › Work"), "{shown}");
    assert!(
        !shown.contains("Other profile") && !shown.contains("Agent"),
        "{shown}"
    );
    assert!(!shown.contains("New Space"), "{shown}");
    host.take();
    cx.update(|cx| host.choose("Work", cx));
    assert!(
        host.take()
            .contains(&"move 90 out of blank to 1".to_string())
    );
}

/// ⌃⌘N runs New Blank Window on the Mac; off the Mac it has no key, as Ctrl+Alt+N there is ⌥⌘N's
/// Little ilo Window.
#[gpui::test]
fn ctrl_cmd_n_is_the_macs_blank_window(cx: &mut TestAppContext) {
    let keys = crate::shell::UserKeys::default();
    let command = keys.command_for("ctrl-cmd-n", true);
    match crate::shell::OFF_THE_MAC {
        false => assert_eq!(command, Some(Command::NewBlankWindow)),
        true => assert_ne!(command, Some(Command::NewBlankWindow)),
    }
    let (shell, host) = blank(cx);
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::NewBlankWindow, "ui", cx));
    assert!(host.take().contains(&"new blank window".to_string()));
}

/// Made from a window that heard the Spaces, before they change again, a Blank Window's tab
/// offers them as Move to all the same.
#[gpui::test]
fn a_blank_window_takes_the_spaces_from_the_window_it_came_from(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let from = cx.new(|cx| Shell::new(browser.clone(), cx));
    from.update(cx, |shell, cx| {
        let work = (1, "Work".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![work], cx);
    });
    let shell = cx.new(|cx| Shell::blank_window(browser, 9, 1, Some(&from), cx));
    shell.update(cx, |shell, cx| {
        shell.set_tabs(9, Vec::new(), vec![90], Vec::new(), Folders::default(), cx);
        shell.tab_menu(90, point(px(100.), px(200.)), cx);
    });
    let shown = crate::menu::described(&host.entries.borrow());
    assert!(shown.contains("Move to › Work"), "{shown}");
}
