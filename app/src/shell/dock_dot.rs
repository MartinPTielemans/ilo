//! Which Space's color the Dock icon's dot takes, with two windows, on a host that records what
//! the dot is asked to show.

use std::{cell::RefCell, rc::Rc};

use gpui::{AppContext, Entity, TestAppContext};

use crate::dock_dot::{self, QUIET};
use crate::shell::{Shell, recorder::Recorder};

const PURPLE: u32 = 0x8655BF;
const RED: u32 = 0xE0475B;

#[derive(Default)]
struct Dot(RefCell<Vec<Option<u32>>>);

impl dock_dot::Host for Dot {
    fn show(&self, color: Option<u32>) {
        self.0.borrow_mut().push(color);
    }
}

/// A window, the key one as it's the newest, with Space 1 in `color` and Space 2 in none,
/// showing `space`.
fn window(cx: &mut TestAppContext, color: u32, space: u64) -> Entity<Shell> {
    let shell = cx.new(|cx| Shell::new(Rc::new(Recorder::default()), cx));
    shell.update(cx, |shell, cx| {
        let spaces = vec![
            (
                1,
                "Space 1".into(),
                Some(color),
                0,
                "".into(),
                "".into(),
                "".into(),
            ),
            (
                2,
                "Space 2".into(),
                None,
                0,
                "".into(),
                "".into(),
                "".into(),
            ),
        ];
        shell.set_spaces(spaces, cx);
        shell.set_space(space, cx);
    });
    shell
}

fn shown(cx: &mut TestAppContext, dot: &Dot) -> Vec<Option<u32>> {
    cx.executor().advance_clock(QUIET);
    cx.run_until_parked();
    dot.0.take()
}

#[gpui::test]
fn the_dot_follows_the_key_windows_space(cx: &mut TestAppContext) {
    let dot = Rc::new(Dot::default());
    cx.update(|cx| dock_dot::open(dot.clone(), cx));
    let first = window(cx, PURPLE, 1);
    assert_eq!(shown(cx, &dot), [Some(PURPLE)]);
    window(cx, PURPLE, 2);
    assert_eq!(shown(cx, &dot), [None]);

    // A window that isn't key switches without the dot.
    first.update(cx, |shell, cx| shell.set_space(2, cx));
    first.update(cx, |shell, cx| shell.set_space(1, cx));
    assert!(shown(cx, &dot).is_empty());

    first.update(cx, |shell, cx| shell.became_key(cx));
    assert_eq!(shown(cx, &dot), [Some(PURPLE)]);

    // A new color for the Space shown recolors it, and a switch back to a green Space too.
    first.update(cx, |shell, cx| {
        let spaces = vec![
            (
                1,
                "Space 1".into(),
                Some(RED),
                0,
                "".into(),
                "".into(),
                "".into(),
            ),
            (
                2,
                "Space 2".into(),
                None,
                0,
                "".into(),
                "".into(),
                "".into(),
            ),
        ];
        shell.set_spaces(spaces, cx);
    });
    assert_eq!(shown(cx, &dot), [Some(RED)]);
    first.update(cx, |shell, cx| shell.set_space(2, cx));
    assert_eq!(shown(cx, &dot), [None]);
}

/// The launch brings saved windows back behind the key one, which macOS doesn't report as becoming
/// key again: the dot and ⌘N stay with the window in front.
#[gpui::test]
fn a_window_opened_behind_leaves_the_key_window_in_charge(cx: &mut TestAppContext) {
    use crate::shell::{Command, commands::Run};
    use gpui::VisualTestContext;

    let dot = Rc::new(Dot::default());
    cx.update(|cx| {
        dock_dot::open(dot.clone(), cx);
        crate::shell::bind_keys(cx);
    });
    let front_host = Rc::new(Recorder::default());
    let browser = front_host.clone();
    let (front, cx): (Entity<Shell>, &mut VisualTestContext) =
        cx.add_window_view(|_, cx| Shell::new(browser, cx));
    front.update(cx, |shell, cx| {
        let spaces = vec![
            (
                1,
                "Space 1".into(),
                Some(PURPLE),
                0,
                "".into(),
                "".into(),
                "".into(),
            ),
            (
                2,
                "Space 2".into(),
                Some(RED),
                0,
                "".into(),
                "".into(),
                "".into(),
            ),
        ];
        shell.set_spaces(spaces, cx);
        shell.set_space(1, cx);
    });
    let behind_host = Rc::new(Recorder::default());
    let behind = cx.new(|cx| Shell::seeded_behind(&front, behind_host.clone(), cx));
    behind.update(cx, |shell, cx| shell.set_space(2, cx));
    cx.executor().advance_clock(QUIET);
    cx.run_until_parked();
    assert_eq!(dot.0.take().last(), Some(&Some(PURPLE)));

    front_host.take();
    behind_host.take();
    cx.dispatch_action(Run(Command::NewWindow));
    assert_eq!(front_host.take(), ["new window"]);
    assert!(behind_host.take().is_empty());
}
