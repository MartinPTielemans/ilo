//! A press on the window's background moves the window once the pointer goes somewhere, and two
//! in a row zoom it, on a host that records what the shell asks of the core. GPUI's test window
//! can't start a window move, so the drag is tested on the rule alone.

use std::rc::Rc;

use gpui::{
    Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point, TestAppContext,
    VisualTestContext, point, px,
};

use crate::shell::{Folders, Shell, moves_window, recorder::Recorder};

/// The header's empty space beside the window's buttons, where the Windows check double-clicks.
const HEADER: (f32, f32) = (115., 24.);

fn window(cx: &mut TestAppContext) -> (&mut VisualTestContext, Rc<Recorder>) {
    let host = Rc::new(Recorder::default());
    let browser = host.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| Shell::new(browser, cx));
    shell.update(cx, |shell, cx| {
        let space = (
            1,
            "Space 1".into(),
            None,
            1,
            "".into(),
            "".into(),
            "".into(),
        );
        shell.set_spaces(vec![space], cx);
        shell.set_space(1, cx);
        shell.set_tabs(1, Vec::new(), vec![10], Vec::new(), Folders::default(), cx);
        shell.set_selected(10, cx);
    });
    cx.run_until_parked();
    host.take();
    (cx, host)
}

/// What the shell asked of the window since the last time.
fn asked(host: &Recorder) -> Vec<String> {
    host.take()
        .into_iter()
        .filter(|call| call == "window move" || call == "window button zoom")
        .collect()
}

/// The `clicks`th press and release in a row at `at`.
fn press(cx: &mut VisualTestContext, at: Point<Pixels>, clicks: usize) {
    let (button, modifiers) = (MouseButton::Left, Modifiers::none());
    cx.simulate_event(MouseDownEvent {
        position: at,
        button,
        modifiers,
        click_count: clicks,
        first_mouse: false,
    });
    // Windows' move to where the pointer rests, which comes with the button down.
    cx.simulate_mouse_move(at, button, modifiers);
    cx.simulate_event(MouseUpEvent {
        position: at,
        button,
        modifiers,
        click_count: clicks,
    });
    cx.run_until_parked();
}

/// Windows sends a resting pointer a move to where it is as windows show or hide. One that came
/// between a double-click's presses moved the window, and the second press didn't zoom it back.
#[gpui::test]
fn a_move_that_goes_nowhere_keeps_a_double_click(cx: &mut TestAppContext) {
    let (cx, host) = window(cx);
    let at = point(px(HEADER.0), px(HEADER.1));
    press(cx, at, 1);
    press(cx, at, 2);
    // On the Mac the title bar's double-click is AppKit's, which the core never hears of.
    let zoom: &[&str] = if cfg!(target_os = "macos") {
        &[]
    } else {
        &["window button zoom"]
    };
    assert_eq!(asked(&host), zoom);
}

#[test]
fn a_press_moves_the_window_once_the_pointer_goes_past_drag_start() {
    let press = point(px(HEADER.0), px(HEADER.1));
    let to = |x: f32, y: f32| press + point(px(x), px(y));
    assert!(
        !moves_window(Some(press), to(0., 0.)),
        "a move that goes nowhere"
    );
    assert!(!moves_window(Some(press), to(2., -3.)), "a hand's wobble");
    assert!(moves_window(Some(press), to(6., 0.)));
    assert!(moves_window(Some(press), to(0., -4.)));
    assert!(!moves_window(None, to(20., 0.)), "nothing pressed");
}
