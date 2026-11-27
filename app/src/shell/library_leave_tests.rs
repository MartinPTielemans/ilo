//! The Library is left by going to a Space, by any way, and ⌘S hides the sidebar whatever it
//! shows, the Library going with it (#4414).

use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext};

use crate::shell::{Command, Section, Shell, SpaceEntry, recorder::Recorder};

fn entry(id: u64) -> SpaceEntry {
    (
        id,
        format!("Space {id}").into(),
        None,
        1,
        "".into(),
        "".into(),
        "".into(),
    )
}

/// A window with Spaces 1, 2 and 3, the first shown, and the Library open on its Spaces, at rest.
fn library(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    cx.update(|window, _| window.activate_window());
    shell.update(cx, |shell, cx| {
        shell.set_spaces([1, 2, 3].map(entry).to_vec(), cx);
        shell.set_space(1, cx);
        shell.section = Section::Spaces;
        shell.set_library(true, cx);
        shell.library_motion = None;
        shell.sidebar_fade = None;
    });
    cx.run_until_parked();
    host.shown.take();
    (shell, cx, host)
}

/// ⌃2 with the Library open goes to Space 2 and the Library goes back in with its motion; ⌃1,
/// the Space shown already, leaves it open, as a dot on the Space shown does nothing.
#[gpui::test]
fn going_to_a_space_leaves_the_library(cx: &mut TestAppContext) {
    let (shell, cx, host) = library(cx);
    shell.update(cx, |shell, cx| shell.run(Command::SelectSpace(0), "ui", cx));
    assert!(
        shell.read_with(cx, |shell, _| shell.library_open),
        "going to the Space shown closed the Library"
    );
    shell.update(cx, |shell, cx| shell.run(Command::SelectSpace(1), "ui", cx));
    assert_eq!(host.shown.take(), [2], "⌃2 didn't go to Space 2");
    shell.read_with(cx, |shell, _| {
        assert!(!shell.library_open, "⌃2 left the Library open");
        assert!(
            shell.library_motion.is_some(),
            "the Library went without its motion"
        );
    });
}

/// ⌘S with the Library open hides the sidebar, and the Library goes with it; ⌘S again shows the
/// sidebar without it.
#[gpui::test]
fn command_s_hides_the_sidebar_over_the_library(cx: &mut TestAppContext) {
    let (shell, cx, _) = library(cx);
    shell.update(cx, |shell, cx| shell.toggle_sidebar(cx));
    shell.read_with(cx, |shell, _| {
        assert!(
            shell.sidebar_hidden,
            "⌘S over the Library didn't hide the sidebar"
        );
        assert!(
            !shell.library_open,
            "the Library stayed open in the hidden sidebar"
        );
    });
    shell.update(cx, |shell, cx| shell.toggle_sidebar(cx));
    shell.read_with(cx, |shell, _| {
        assert!(!shell.sidebar_hidden, "⌘S again didn't show the sidebar");
        assert!(!shell.library_open, "⌘S again brought the Library back");
    });
}
