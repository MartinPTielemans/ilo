//! `typing_in` says a window's field has its keyboard only while GPUI's focus is on it and the
//! window's last frame drew it, and only for that window: a field drawn in another window, as a
//! page dialog's are in the dialog's own, doesn't count.

use gpui::{TestAppContext, VisualTestContext};

use super::*;

/// A window's root, which may draw a field.
struct Root {
    field: Entity<TextInput>,
    shows_field: bool,
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        drawing(window);
        div().size_full().when(self.shows_field, |root| {
            root.child(div().h(px(30.)).w(px(200.)).child(self.field.clone()))
        })
    }
}

fn root(cx: &mut TestAppContext) -> (Entity<Root>, &mut VisualTestContext) {
    let (root, cx) = cx.add_window_view(|_, cx| Root {
        field: cx.new(|cx| TextInput::new("Name", cx)),
        shows_field: true,
    });
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    (root, cx)
}

fn typing(root: &Entity<Root>, cx: &mut VisualTestContext) -> bool {
    let _ = root;
    cx.update(|window, cx| typing_in(window, cx))
}

fn focus_field(root: &Entity<Root>, cx: &mut VisualTestContext) {
    root.update_in(cx, |root, window, cx| {
        window.focus(&root.field.focus_handle(cx), cx)
    });
    cx.run_until_parked();
}

#[gpui::test]
fn a_field_drawn_with_the_keyboard_is_typed_in(cx: &mut TestAppContext) {
    let (root, cx) = root(cx);
    assert!(!typing(&root, cx), "no field has the keyboard yet");
    focus_field(&root, cx);
    assert!(typing(&root, cx));
    // An input method composing is the same case: the field keeps the keyboard.
    root.update_in(cx, |root, window, cx| {
        root.field.update(cx, |field, cx| {
            field.replace_and_mark_text_in_range(None, "に", None, window, cx)
        })
    });
    cx.run_until_parked();
    assert!(typing(&root, cx), "a composing field let the keyboard go");
    root.update_in(cx, |_, window, cx| window.blur(cx));
    cx.run_until_parked();
    assert!(!typing(&root, cx), "a field left still counted");
}

#[gpui::test]
fn a_field_no_longer_drawn_isnt_typed_in(cx: &mut TestAppContext) {
    let (root, cx) = root(cx);
    focus_field(&root, cx);
    root.update(cx, |root, cx| {
        root.shows_field = false;
        cx.notify();
    });
    cx.run_until_parked();
    // GPUI's focus stays on the field, which the frame no longer drew.
    let focused = root.update_in(cx, |root, window, cx| {
        root.field.focus_handle(cx).is_focused(window)
    });
    assert!(focused, "the test lost the case it's for");
    assert!(!typing(&root, cx), "a field not drawn still counted");
}

#[gpui::test]
fn a_field_gone_isnt_typed_in(cx: &mut TestAppContext) {
    let (root, cx) = root(cx);
    focus_field(&root, cx);
    root.update(cx, |root, cx| {
        root.field = cx.new(|cx| TextInput::new("Name", cx));
        cx.notify();
    });
    cx.run_until_parked();
    assert!(!typing(&root, cx), "a field gone still counted");
}

#[gpui::test]
fn another_windows_field_doesnt_count(cx: &mut TestAppContext) {
    let open = |shows_field: bool, cx: &mut TestAppContext| {
        cx.add_window(|_, cx| Root {
            field: cx.new(|cx| TextInput::new("Name", cx)),
            shows_field,
        })
    };
    let (other, this) = (open(true, cx), open(false, cx));
    let field = other
        .update(cx, |root, window, cx| {
            let field = root.field.focus_handle(cx);
            window.focus(&field, cx);
            field
        })
        .unwrap();
    // This window's focus on the very field the other one draws, which it doesn't draw itself.
    this.update(cx, |_, window, cx| window.focus(&field, cx))
        .unwrap();
    cx.run_until_parked();
    let typing = |handle: gpui::WindowHandle<Root>, cx: &mut TestAppContext| {
        handle
            .update(cx, |_, window, cx| typing_in(window, cx))
            .unwrap()
    };
    assert!(typing(other, cx));
    assert!(
        !typing(this, cx),
        "another window's field counted for this one"
    );
}
