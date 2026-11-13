//! A split view's row folds to the pane with the keyboard (#2340): a press on a folded segment
//! shows its pane and gives it the row, a change made elsewhere waits for the pointer to leave the
//! row, and while the split isn't shown the row keeps the pane shown last.

use std::rc::Rc;

use gpui::{
    Entity, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Point, TestAppContext,
    VisualTestContext, point, px,
};

use crate::shell::{
    Command, Folders, ROW_HEIGHT, Row, Shell, Slot, Split, commands::bind_keys, recorder::Recorder,
};

/// An active window under Reduce Motion, so each fold shows where it ends, showing the Today tabs
/// 3, 4 and 5, 3 and 4 a split, 3 shown.
fn window(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext, Rc<Recorder>) {
    cx.update(|cx| {
        bind_keys(cx);
        cx.set_reduce_motion(true);
    });
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_selected(3, cx);
        let split = Split {
            tabs: vec![3, 4],
            ..Split::default()
        };
        shell.set_tabs(
            1,
            Vec::new(),
            vec![3, 4, 5],
            vec![split],
            Folders::default(),
            cx,
        );
    });
    cx.run_until_parked();
    cx.update(|window, _| window.activate_window());
    host.take();
    (shell, cx, host)
}

/// The middle of `tab`'s segment in the split's row.
fn segment(shell: &Entity<Shell>, cx: &mut VisualTestContext, tab: u64) -> Point<gpui::Pixels> {
    shell.read_with(cx, |shell, _| {
        let top = shell.drawn_list_top() + shell.layout()[&Slot::Tab(3)];
        point(px(shell.segment_middle(tab)), px(top + ROW_HEIGHT / 2.))
    })
}

fn weights(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Vec<f32> {
    shell.read_with(cx, |shell, _| shell.fold_weights(3, &[3, 4]))
}

fn press(cx: &mut VisualTestContext, position: Point<gpui::Pixels>) {
    let (button, modifiers) = (MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(position, None, modifiers);
    cx.simulate_event(MouseDownEvent {
        position,
        button,
        modifiers,
        click_count: 1,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        position,
        button,
        modifiers,
        click_count: 1,
    });
    cx.run_until_parked();
}

/// The pane shown has the row; the other is its favicon, at the row's end. A press there shows
/// that pane and gives it the row at once, the pointer still on the row.
#[gpui::test]
fn a_press_on_a_folded_segment_shows_its_pane_and_unfolds_it(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    assert_eq!(weights(&shell, cx), [1., 0.]);
    let folded = segment(&shell, cx, 4);
    let width = shell.read_with(cx, |shell, _| shell.width());
    assert!(
        folded.x > px(width * 0.8),
        "the folded pane is its favicon at the row's end: {folded:?} of {width}"
    );
    shell.update(cx, |shell, cx| shell.hover(Row::Tab(3), true, cx));
    press(cx, folded);
    assert!(host.take().contains(&"select 4".to_string()));
    assert_eq!(weights(&shell, cx), [0., 1.], "the press is the aim");
    shell.update(cx, |shell, cx| shell.set_selected(4, cx));
    cx.run_until_parked();
    assert_eq!(weights(&shell, cx), [0., 1.]);
}

/// With the pointer on the row, a pane shown from elsewhere waits to take the row until the pointer
/// leaves, so the segment aimed at doesn't move from under it.
#[gpui::test]
fn the_row_waits_while_the_pointer_is_on_it(cx: &mut TestAppContext) {
    let (shell, cx, _host) = window(cx);
    shell.update(cx, |shell, cx| shell.hover(Row::Tab(3), true, cx));
    cx.run_until_parked();
    shell.update(cx, |shell, cx| shell.set_selected(4, cx));
    cx.run_until_parked();
    assert_eq!(weights(&shell, cx), [1., 0.], "it moved under the pointer");
    shell.update(cx, |shell, cx| shell.hover(Row::Tab(3), false, cx));
    cx.run_until_parked();
    assert_eq!(weights(&shell, cx), [0., 1.]);
}

/// Another tab shown, the row keeps the pane shown last, which ⌘1 shows again.
#[gpui::test]
fn the_row_keeps_the_pane_shown_last(cx: &mut TestAppContext) {
    let (shell, cx, host) = window(cx);
    shell.update(cx, |shell, cx| shell.set_selected(4, cx));
    cx.run_until_parked();
    shell.update(cx, |shell, cx| shell.set_selected(5, cx));
    cx.run_until_parked();
    assert_eq!(weights(&shell, cx), [0., 1.]);
    shell.update(cx, |shell, cx| shell.run(Command::SelectTab(0), "ui", cx));
    cx.run_until_parked();
    assert!(
        host.take().contains(&"select 4".to_string()),
        "⌘1 shows the pane the row shows"
    );
}
