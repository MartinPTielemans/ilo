//! The stops the keyboard goes through Settings by, on a panel of three buttons and a field.

use gpui::{
    Entity, FocusHandle, Focusable, KeyUpEvent, Keystroke, Modifiers, PlatformInput,
    TestAppContext, VisualTestContext, bounds, size,
};

use super::*;
use crate::text_input::TextInput;

/// Tab and ⇧Tab go round, the arrows stop at the ends, and with none ringed the first key goes to
/// the first stop, or the last going back.
#[test]
fn the_keys_go_round_or_to_an_end() {
    assert_eq!(step(None, 3, true, true), Some(0));
    assert_eq!(step(None, 3, false, true), Some(2));
    assert_eq!(step(Some(0), 3, true, true), Some(1));
    assert_eq!(step(Some(2), 3, true, true), Some(0));
    assert_eq!(step(Some(0), 3, false, true), Some(2));
    assert_eq!(step(Some(2), 3, true, false), Some(2));
    assert_eq!(step(Some(0), 3, false, false), Some(0));
    assert_eq!(step(Some(1), 3, false, false), Some(0));
    assert_eq!(step(None, 0, true, true), None);
}

/// A stop under the list's bottom scrolls up into it with a padding's room, one over its top
/// down, one in it stays, and the list never scrolls past its ends.
#[test]
fn the_list_scrolls_the_least_that_shows_the_stop() {
    let view = bounds(point(px(0.), px(100.)), size(px(600.), px(400.)));
    let at = |top: f32, height: f32| bounds(point(px(0.), px(top)), size(px(100.), px(height)));
    let max = px(1000.);
    // 24 tall, its bottom 34 under the list's: up by that and the room.
    assert_eq!(revealed(px(-50.), at(510., 24.), view, max), px(-50. - 34. - PADDING));
    // Its top 20 over the list's: down by that and the room.
    assert_eq!(revealed(px(-50.), at(80., 24.), view, max), px(-50. + 20. + PADDING));
    assert_eq!(revealed(px(-50.), at(200., 24.), view, max), px(-50.));
    assert_eq!(revealed(px(-5.), at(80., 24.), view, max), px(0.));
    assert_eq!(revealed(px(-990.), at(510., 24.), view, max), px(-1000.));
    // Taller than the list: its top shows.
    assert_eq!(revealed(px(0.), at(200., 600.), view, max), px(-90.));
}

/// Three buttons and a field in a panel that moves its ring as Settings does.
struct Panel {
    focus: FocusHandle,
    stops: Stops,
    field: Entity<TextInput>,
    pressed: Rc<RefCell<Vec<&'static str>>>,
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.stops.begin();
        let button = |name: &'static str| {
            let pressed = self.pressed.clone();
            button(&Look::settings(), name, name)
                .button_stop(&self.stops, cx)
                .on_click(move |_, _, _| pressed.borrow_mut().push(name))
        };
        let handle = self.field.focus_handle(cx);
        let field = div()
            .w(px(200.))
            .relative()
            .child(self.field.clone())
            .child(self.stops.mark("field", handle, Ring::Inside(CONTROL_RADIUS)));
        div()
            .key_context("Panel")
            .track_focus(&self.focus)
            .size_full()
            .on_action(cx.listener(|panel, _: &NextStop, window, cx| {
                panel.stops.go(true, true, window, cx);
            }))
            .on_action(cx.listener(|panel, _: &PreviousStop, window, cx| {
                panel.stops.go(false, true, window, cx);
            }))
            .on_action(cx.listener(|panel, _: &StopDown, window, cx| {
                panel.stops.go(true, false, window, cx);
            }))
            .on_action(cx.listener(|panel, _: &StopUp, window, cx| {
                panel.stops.go(false, false, window, cx);
            }))
            .capture_any_mouse_down(cx.listener(|panel, _, _, _| panel.stops.set_keyed(false)))
            .flex()
            .flex_col()
            .items_start()
            .child(button("Back"))
            .child(field)
            .child(button("Next"))
            .child(button("Done"))
    }
}

fn panel(cx: &mut TestAppContext) -> (Entity<Panel>, &mut VisualTestContext) {
    cx.update(|cx| {
        crate::text_input::bind_keys(cx);
        bind_stop_keys("Panel", cx);
    });
    let (panel, cx) = cx.add_window_view(|_, cx| Panel {
        focus: cx.focus_handle(),
        stops: Stops::default(),
        field: cx.new(|cx| TextInput::new("Search", cx)),
        pressed: Rc::default(),
    });
    // As Settings' window has the keyboard as it opens.
    cx.update(|window, cx| window.focus(&panel.read(cx).focus.clone(), cx));
    cx.run_until_parked();
    (panel, cx)
}

/// Presses `key` and lets it go, as GPUI's click from the keyboard is on the release.
fn press(cx: &mut VisualTestContext, key: &'static str) {
    cx.simulate_keystrokes(key);
    let keystroke = Keystroke::parse(key).unwrap();
    cx.update(|window, cx| {
        window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke }), cx);
    });
    cx.run_until_parked();
}

/// What has the keyboard: a stop's id, the field, the panel, or nothing.
fn keyed(panel: &Entity<Panel>, cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|window, cx| {
        let focused = window.focused(cx)?;
        let panel = panel.read(cx);
        if panel.field.focus_handle(cx) == focused {
            return Some("field".into());
        }
        if panel.focus == focused {
            return Some("panel".into());
        }
        let handles = panel.stops.handles.borrow();
        handles.iter().find(|(_, handle)| **handle == focused).map(|(id, _)| id.to_string())
    })
}

/// Tab rings the stops in the order drawn, the field among them, and round from the last to the
/// first; ⇧Tab goes back round. Space or return on a button is its click, and a press takes the
/// ring away.
#[gpui::test]
fn tab_rings_each_stop_and_space_presses_it(cx: &mut TestAppContext) {
    let (panel, cx) = panel(cx);
    assert_eq!(keyed(&panel, cx).as_deref(), Some("panel"));
    press(cx, "tab");
    assert_eq!(keyed(&panel, cx).as_deref(), Some("Back"));
    assert!(panel.read_with(cx, |panel, _| panel.stops.keyed()));
    press(cx, "space");
    assert_eq!(*panel.read_with(cx, |panel, _| panel.pressed.clone()).borrow(), ["Back"]);
    press(cx, "tab");
    assert_eq!(keyed(&panel, cx).as_deref(), Some("field"));
    // Typed into the field, a space is the field's.
    cx.simulate_input("a b");
    assert_eq!(panel.read_with(cx, |panel, cx| panel.field.read(cx).text().to_string()), "a b");
    press(cx, "tab");
    press(cx, "tab");
    assert_eq!(keyed(&panel, cx).as_deref(), Some("Done"));
    press(cx, "enter");
    assert_eq!(*panel.read_with(cx, |panel, _| panel.pressed.clone()).borrow(), ["Back", "Done"]);
    press(cx, "tab");
    assert_eq!(keyed(&panel, cx).as_deref(), Some("Back"));
    press(cx, "shift-tab");
    assert_eq!(keyed(&panel, cx).as_deref(), Some("Done"));
    // A press on Next takes the ring away, and gives it the keyboard.
    let next = panel.read_with(cx, |panel, _| {
        let drawn = panel.stops.shared.drawn.borrow();
        drawn.iter().find(|each| each.id.to_string() == "Next").unwrap().frame.center()
    });
    cx.simulate_click(next, Modifiers::none());
    cx.run_until_parked();
    assert!(!panel.read_with(cx, |panel, _| panel.stops.keyed()));
    let pressed = panel.read_with(cx, |panel, _| panel.pressed.clone());
    assert_eq!(*pressed.borrow(), ["Back", "Done", "Next"]);
    assert_eq!(keyed(&panel, cx).as_deref(), Some("Next"));
}

/// ↓ and ↑ go along the stops and stay at the ends, but in the field they're the field's.
#[gpui::test]
fn the_arrows_stop_at_the_ends(cx: &mut TestAppContext) {
    let (panel, cx) = panel(cx);
    press(cx, "up");
    assert_eq!(keyed(&panel, cx).as_deref(), Some("Done"));
    press(cx, "down");
    assert_eq!(keyed(&panel, cx).as_deref(), Some("Done"));
    press(cx, "up");
    assert_eq!(keyed(&panel, cx).as_deref(), Some("Next"));
    press(cx, "up");
    assert_eq!(keyed(&panel, cx).as_deref(), Some("field"));
    press(cx, "up");
    assert_eq!(keyed(&panel, cx).as_deref(), Some("field"));
    press(cx, "shift-tab");
    assert_eq!(keyed(&panel, cx).as_deref(), Some("Back"));
    press(cx, "up");
    assert_eq!(keyed(&panel, cx).as_deref(), Some("Back"));
}
