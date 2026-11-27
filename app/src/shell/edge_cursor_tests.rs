//! The sidebar's edge is what's under the pointer on it, so a press there starts dragging the
//! width: GPUI picks a press's target and the cursor by the same topmost-hitbox rule (#4455).

use std::rc::Rc;

use gpui::{
    Modifiers, MouseButton, MouseDownEvent, TestAppContext, VisualTestContext, point, px,
};

use crate::shell::{Folders, Shell, recorder::Recorder};

fn window(cx: &mut TestAppContext, right: bool) -> (&mut VisualTestContext, gpui::Entity<Shell>) {
    let host = Rc::new(Recorder::default());
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(host, cx));
    shell.update(cx, |shell, cx| {
        let space = (1, "Space 1".into(), None, 1, "".into(), "".into(), "".into());
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
        if right {
            shell.set_sidebar_right(true, cx);
        }
    });
    cx.run_until_parked();
    (cx, shell)
}

fn press_takes_edge(cx: &mut VisualTestContext, shell: &gpui::Entity<Shell>, y: f32) -> bool {
    let (width, right) = shell.read_with(cx, |shell, _| (shell.width(), shell.right));
    let window = cx.update(|window, _| f32::from(window.viewport_size().width));
    let x = if right { window - width + 5. } else { width - 5. };
    cx.simulate_mouse_move(point(px(x), px(y)), None, Modifiers::none());
    cx.simulate_event(MouseDownEvent {
        position: point(px(x), px(y)),
        button: MouseButton::Left,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| shell.resize.is_some())
}

#[gpui::test]
fn the_edge_is_under_the_pointer_on_the_left(cx: &mut TestAppContext) {
    let (cx, shell) = window(cx, false);
    assert!(press_takes_edge(cx, &shell, 300.));
}

#[gpui::test]
fn the_edge_is_under_the_pointer_on_the_right(cx: &mut TestAppContext) {
    let (cx, shell) = window(cx, true);
    assert!(press_takes_edge(cx, &shell, 300.));
}
