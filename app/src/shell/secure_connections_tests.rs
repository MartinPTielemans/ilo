//! Always use secure connections, on a host that records what the shell asks: the core's value is
//! the row's, a click sets it once, and a refused set leaves the core's.

use std::rc::Rc;

use gpui::{AppContext, Entity, TestAppContext};

use crate::shell::{Folders, Shell, recorder::Recorder};

/// A window showing Space 1, in profile 7, on a core whose value is `core`.
fn window(core: Option<bool>, cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    host.secure_connections.set(core);
    let shell = cx.new(|cx| Shell::new(host.clone(), cx));
    shell.update(cx, |shell, cx| {
        let space = (
            1,
            "Space 1".into(),
            None,
            7,
            "".into(),
            "".into(),
            "".into(),
        );
        shell.set_spaces(vec![space], cx);
        shell.set_tabs(1, Vec::new(), vec![11], Vec::new(), Folders::default(), cx);
        shell.set_space(1, cx);
    });
    host.take();
    (shell, host)
}

fn shown(shell: &Entity<Shell>, cx: &mut TestAppContext) -> Option<bool> {
    shell.read_with(cx, |shell, _| shell.secure_connections())
}

fn sets(host: &Recorder) -> Vec<String> {
    let calls = host.take();
    calls
        .into_iter()
        .filter(|call| call.starts_with("secure connections"))
        .collect()
}

/// The row says what the core says, on or off, and nothing where the core can't say.
#[gpui::test]
fn the_row_shows_the_cores_value(cx: &mut TestAppContext) {
    for core in [Some(true), Some(false), None] {
        let (shell, host) = window(core, cx);
        assert_eq!(shown(&shell, cx), core);
        assert_eq!(sets(&host), Vec::<String>::new(), "reading sets nothing");
    }
}

/// A click asks the core once, and the row then shows the core's new value.
#[gpui::test]
fn a_click_sets_it_once(cx: &mut TestAppContext) {
    let (shell, host) = window(Some(true), cx);
    let after = shell.update(cx, |shell, cx| shell.set_secure_connections(false, cx));
    assert_eq!(after, Some(false));
    assert_eq!(sets(&host), ["secure connections false"]);
    assert_eq!(shown(&shell, cx), Some(false));
}

/// A set the core refuses, as for a policy, leaves the row showing what the core still has.
#[gpui::test]
fn a_refused_set_shows_the_cores_value_again(cx: &mut TestAppContext) {
    let (shell, host) = window(Some(true), cx);
    host.refuses_secure_connections.set(true);
    let after = shell.update(cx, |shell, cx| shell.set_secure_connections(false, cx));
    assert_eq!(after, Some(true));
    assert_eq!(sets(&host), ["secure connections false"]);
    assert_eq!(shown(&shell, cx), Some(true));
}
