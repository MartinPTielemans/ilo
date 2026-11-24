//! A tab's page background colour, which the open tab's line takes: the shell keeps what the core
//! reports for each tab and draws only for a change to a tab it draws.

use std::{cell::Cell, rc::Rc};

use gpui::{AppContext, Entity, Subscription, TestAppContext};

use crate::shell::{Folders, Shell, recorder::Recorder};

/// A window whose Space has the Today tabs 10 and 11, showing 10, and how often it asked to be
/// drawn since.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, Rc<Cell<usize>>, Subscription) {
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
            vec![10, 11],
            Vec::new(),
            Folders::default(),
            cx,
        );
        shell.set_selected(10, cx);
    });
    let drawn = Rc::new(Cell::new(0));
    let counted = drawn.clone();
    let watch = cx.update(|cx| cx.observe(&shell, move |_, _| counted.set(counted.get() + 1)));
    (shell, drawn, watch)
}

fn set(shell: &Entity<Shell>, cx: &mut TestAppContext, tab: u64, color: Option<u32>) {
    shell.update(cx, |shell, cx| shell.set_tab_background(tab, color, cx));
}

fn kept(shell: &Entity<Shell>, cx: &mut TestAppContext, tab: u64) -> Option<u32> {
    shell.read_with(cx, |shell, _| shell.tab_background(tab))
}

#[gpui::test]
fn a_tab_keeps_its_pages_colour_and_draws_once_for_a_change(cx: &mut TestAppContext) {
    let (shell, drawn, _watch) = window(cx);
    assert_eq!(kept(&shell, cx, 10), None);
    set(&shell, cx, 10, Some(0x1d1d21));
    assert_eq!(kept(&shell, cx, 10), Some(0x1d1d21));
    assert_eq!(drawn.get(), 1);
    // The colour kept already, as the read at a tab switch gives it again: nothing to draw.
    set(&shell, cx, 10, Some(0x1d1d21));
    assert_eq!(drawn.get(), 1);
    // A page with no single opaque colour: the line goes back to its plain plate.
    set(&shell, cx, 10, None);
    assert_eq!(kept(&shell, cx, 10), None);
    assert_eq!(drawn.get(), 2);
}

#[gpui::test]
fn each_tab_has_its_own_colour(cx: &mut TestAppContext) {
    let (shell, _, _watch) = window(cx);
    set(&shell, cx, 10, Some(0xffffff));
    set(&shell, cx, 11, Some(0x000000));
    assert_eq!(kept(&shell, cx, 10), Some(0xffffff));
    assert_eq!(kept(&shell, cx, 11), Some(0x000000));
}

#[gpui::test]
fn a_tab_the_window_doesnt_draw_keeps_its_colour_and_draws_nothing(cx: &mut TestAppContext) {
    let (shell, drawn, _watch) = window(cx);
    // Tab 30 is in no Space this window shows.
    set(&shell, cx, 30, Some(0x336699));
    assert_eq!(kept(&shell, cx, 30), Some(0x336699));
    assert_eq!(drawn.get(), 0);
    assert_eq!(kept(&shell, cx, 31), None);
}
