//! ⌘W closes the selected tab, or the window when no tab is selected, on a host that records what
//! the shell asks.

use std::rc::Rc;

use gpui::{AppContext, TestAppContext};

use crate::shell::{Command, Folders, Shell, recorder::Recorder};

fn window_with_a_tab(cx: &mut TestAppContext) -> (gpui::Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        shell.set_spaces(
            vec![(
                1,
                "Space 1".into(),
                None,
                0,
                "".into(),
                "".into(),
                "".into(),
            )],
            cx,
        );
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
    });
    (shell, host)
}

#[gpui::test]
fn close_tab_closes_the_window_when_no_tab_is_selected(cx: &mut TestAppContext) {
    let (shell, host) = window_with_a_tab(cx);
    shell.update(cx, |shell, cx| shell.set_selected(0, cx));
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::CloseTab, "ui", cx));
    assert_eq!(host.take(), ["close window"]);
}

#[gpui::test]
fn close_tab_closes_the_tab_and_not_the_window_when_one_is_selected(cx: &mut TestAppContext) {
    let (shell, host) = window_with_a_tab(cx);
    shell.update(cx, |shell, cx| shell.set_selected(10, cx));
    host.take();
    shell.update(cx, |shell, cx| shell.run(Command::CloseTab, "ui", cx));
    assert_eq!(host.take(), ["close 10"]);
}
