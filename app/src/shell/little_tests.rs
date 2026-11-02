//! A Little ilo window's shell on a host that records what it asks of the core: what closes it,
//! what it leaves out, and where ⌥⌘N and ⌘O send their page.

use std::rc::Rc;

use gpui::{AppContext, Entity, TestAppContext};

use crate::shell::{Command, Folders, Page, Shell, recorder::Recorder};

/// A regular window's shell with the user's two Spaces, the first shown.
fn main_window(cx: &mut TestAppContext, host: &Rc<Recorder>) -> Entity<Shell> {
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "One".into(), Some(7), 1, "".into(), "".into(), "".into()),
                (2, "Two".into(), None, 1, "".into(), "".into(), "".into()),
            ],
            cx,
        );
        shell.set_space(1, cx);
    });
    shell
}

/// A Little ilo window for the Space One, its own Space 9 holding the page 10 at `url`.
fn little(cx: &mut TestAppContext, url: &str) -> (Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let main = main_window(cx, &host);
    let shell = cx.new(|cx| Shell::little_window(host.clone(), 9, 1, 1, Some(&main), cx));
    shell.update(cx, |shell, cx| {
        shell.set_tabs(9, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        shell.set_page(10, Page { url: url.to_string().into(), ..Page::default() }, cx);
    });
    host.take();
    (shell, host)
}

fn run(shell: &Entity<Shell>, cx: &mut TestAppContext, command: Command, from: &str) {
    shell.update(cx, |shell, cx| shell.run(command, from, cx));
}

#[gpui::test]
fn it_takes_its_spaces_look_and_has_no_sidebar(cx: &mut TestAppContext) {
    let (shell, _) = little(cx, "https://example.com/");
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.little(), Some(1));
        assert!(shell.sidebar_hidden);
        let own: Vec<_> = (shell.spaces.iter())
            .map(|space| (space.id, space.name.to_string(), space.color))
            .collect();
        assert_eq!(own, [(9, "One".to_string(), Some(7))]);
    });
    // The Space renamed and recoloured, the window follows.
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![
                (1, "Uno".into(), Some(3), 1, "".into(), "".into(), "".into()),
                (2, "Two".into(), None, 1, "".into(), "".into(), "".into()),
            ],
            cx,
        )
    });
    shell.read_with(cx, |shell, _| {
        let own = &shell.spaces[0];
        let look = (shell.spaces.len(), own.id, own.name.as_ref(), own.color);
        assert_eq!(look, (1, 9, "Uno", Some(3)));
    });
}

#[gpui::test]
fn command_w_closes_the_window_not_the_tab(cx: &mut TestAppContext) {
    let (shell, host) = little(cx, "https://example.com/");
    run(&shell, cx, Command::CloseTab, "ui");
    assert_eq!(host.take(), ["close window"]);
}

#[gpui::test]
fn an_unused_escape_closes_it_unless_the_page_is_loading(cx: &mut TestAppContext) {
    let (shell, host) = little(cx, "https://example.com/");
    shell.update(cx, |shell, cx| {
        let page = Page { url: "https://example.com/".into(), loading: true, ..Page::default() };
        shell.set_page(10, page, cx);
    });
    // Escape stops the load first.
    assert!(!shell.update(cx, |shell, cx| shell.escape_unused(cx)));
    shell.update(cx, |shell, cx| {
        shell.set_page(10, Page { url: "https://example.com/".into(), ..Page::default() }, cx)
    });
    run(&shell, cx, Command::Stop, "page");
    assert_eq!(host.take(), ["close window"]);
}

#[gpui::test]
fn an_escape_in_another_window_is_the_pages(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let main = main_window(cx, &host);
    host.take();
    assert!(!main.update(cx, |shell, cx| shell.escape_unused(cx)));
    assert!(host.take().is_empty());
}

#[gpui::test]
fn the_sidebar_spaces_and_split_view_are_left_out(cx: &mut TestAppContext) {
    let (shell, host) = little(cx, "https://example.com/");
    for command in [
        Command::ToggleSidebar,
        Command::NewSpace,
        Command::NextSpace,
        Command::AddSplit,
        Command::ViewLibrary,
        Command::NewTab,
        Command::ReopenClosedTab,
    ] {
        shell.read_with(cx, |shell, _| assert!(!shell.offers(command), "{command:?}"));
        run(&shell, cx, command, "ui");
    }
    assert!(host.take().is_empty());
    shell.read_with(cx, |shell, _| assert!(shell.sidebar_hidden));
}

#[gpui::test]
fn command_o_moves_the_page_to_its_space_in_the_main_window(cx: &mut TestAppContext) {
    let (shell, host) = little(cx, "https://example.com/");
    shell.read_with(cx, |shell, _| assert!(shell.offers(Command::OpenInMainWindow)));
    run(&shell, cx, Command::OpenInMainWindow, "ui");
    assert_eq!(host.take(), ["open in main window 10 1"]);
}

#[gpui::test]
fn open_in_main_window_is_little_ilos_alone(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let main = main_window(cx, &host);
    host.take();
    main.read_with(cx, |shell, _| assert!(!shell.offers(Command::OpenInMainWindow)));
    run(&main, cx, Command::OpenInMainWindow, "ui");
    assert!(host.take().is_empty());
}

#[gpui::test]
fn a_new_little_window_is_for_the_space_it_came_from(cx: &mut TestAppContext) {
    let host = Rc::new(Recorder::default());
    let main = main_window(cx, &host);
    main.update(cx, |shell, cx| shell.set_space(2, cx));
    host.take();
    run(&main, cx, Command::NewLittleWindow, "ui");
    assert_eq!(host.take(), ["new little window 2"]);
    let (shell, host) = little(cx, "https://example.com/");
    run(&shell, cx, Command::NewLittleWindow, "ui");
    assert_eq!(host.take(), ["new little window 1"]);
}

/// ⌥⌘N reaches the browser before the page, as ⌘N does, and ⌘O only if the page doesn't keep it.
#[test]
fn its_keys_are_option_command_n_and_command_o() {
    let keys = crate::shell::UserKeys::default();
    assert_eq!(keys.command_for("alt-cmd-n", true), Some(Command::NewLittleWindow));
    assert_eq!(keys.command_for("cmd-o", true), None);
    assert_eq!(keys.command_for("cmd-o", false), Some(Command::OpenInMainWindow));
}
