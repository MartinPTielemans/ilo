//! A surface closing as the user meets it: a panel with a field, opened from a button, over a
//! window whose own focus is the fallback.

use gpui::{
    Context, Entity, EntityInputHandler as _, FocusHandle, Focusable, KeyBinding, Modifiers,
    Render, TestAppContext, VisualTestContext, actions, div, point, prelude::*, px, size,
};

use super::*;
use crate::text_input::TextInput;

actions!(dismissal_tests, [Escape, Submit]);

struct Host {
    window: FocusHandle,
    opener: Option<FocusHandle>,
    other: FocusHandle,
    field: Entity<TextInput>,
    open: Option<FocusReturn>,
    closes: Vec<Close>,
    returned: Vec<Returned>,
    submitted: usize,
}

impl Host {
    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = Some(FocusReturn::remember(false, window, cx));
        window.focus(&self.field.focus_handle(cx), cx);
        cx.notify();
    }

    fn close(&mut self, close: Close, window: &mut Window, cx: &mut Context<Self>) {
        let Some(back) = self.open.take() else {
            return;
        };
        let field = self.field.focus_handle(cx);
        self.returned
            .push(back.give_back(close, &[&field], &self.window, window, cx));
        self.closes.push(close);
        cx.notify();
    }
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let row = |id: &'static str| div().id(id).h(px(20.)).w_full();
        div()
            .id("window")
            .track_focus(&self.window)
            .size_full()
            .flex()
            .flex_col()
            .when_some(self.opener.as_ref(), |root, opener| {
                root.child(row("opener").track_focus(opener))
            })
            .child(row("other").track_focus(&self.other))
            .when(self.open.is_some(), |root| {
                root.child(
                    div()
                        .id("panel")
                        .key_context("Panel")
                        .h(px(40.))
                        .w_full()
                        .child(self.field.clone())
                        .on_action(cx.listener(|host, _: &Escape, window, cx| {
                            let field = host.field.clone();
                            if let Some(close) = Close::escape_from([&field], cx) {
                                host.close(close, window, cx);
                            }
                        }))
                        .on_action(cx.listener(|host, _: &Submit, _, cx| {
                            let field = host.field.clone();
                            if submits([&field], cx) {
                                host.submitted += 1;
                            }
                        }))
                        .on_mouse_down_out(cx.listener(|host, _, window, cx| {
                            host.close(Close::new(Why::PressOutside, By::Pointer), window, cx)
                        })),
                )
            })
    }
}

/// A window with the opener focused and the panel opened from it.
fn opened(cx: &mut TestAppContext) -> (Entity<Host>, &mut VisualTestContext) {
    cx.update(|cx| {
        cx.bind_keys([
            KeyBinding::new("escape", Escape, Some("Panel")),
            KeyBinding::new("enter", Submit, Some("Panel")),
        ])
    });
    let (host, cx) = cx.add_window_view(|_, cx| Host {
        window: cx.focus_handle(),
        opener: Some(cx.focus_handle()),
        other: cx.focus_handle(),
        field: cx.new(|cx| TextInput::new("Find", cx)),
        open: None,
        closes: Vec::new(),
        returned: Vec::new(),
        submitted: 0,
    });
    cx.simulate_resize(size(px(400.), px(300.)));
    cx.update(|window, cx| {
        window.activate_window();
        let opener = host.read(cx).opener.clone().unwrap();
        window.focus(&opener, cx);
        host.update(cx, |host, cx| host.open(window, cx));
    });
    cx.run_until_parked();
    (host, cx)
}

fn focused(
    handle: impl Fn(&Host, &App) -> FocusHandle,
    host: &Entity<Host>,
    cx: &mut VisualTestContext,
) -> bool {
    cx.update(|window, cx| handle(host.read(cx), cx).is_focused(window))
}

fn closes(host: &Entity<Host>, cx: &mut VisualTestContext) -> (Vec<Close>, Vec<Returned>) {
    host.read_with(cx, |host, _| (host.closes.clone(), host.returned.clone()))
}

#[gpui::test]
fn escape_closes_and_gives_the_keyboard_back_to_the_opener(cx: &mut TestAppContext) {
    let (host, cx) = opened(cx);
    assert!(focused(|host, cx| host.field.focus_handle(cx), &host, cx));
    cx.simulate_keystrokes("escape");
    assert_eq!(
        closes(&host, cx),
        (
            vec![Close::new(Why::Escape, By::Keyboard)],
            vec![Returned::Opener]
        )
    );
    assert!(focused(|host, _| host.opener.clone().unwrap(), &host, cx));
}

#[gpui::test]
fn a_press_outside_closes_with_its_reason_and_leaves_the_opener(cx: &mut TestAppContext) {
    let (host, cx) = opened(cx);
    cx.simulate_click(point(px(200.), px(250.)), Modifiers::none());
    cx.run_until_parked();
    let (closed, _) = closes(&host, cx);
    assert_eq!(closed, [Close::new(Why::PressOutside, By::Pointer)]);
    assert!(
        focused(|host, _| host.window.clone(), &host, cx),
        "a press outside gave the opener the keyboard"
    );
}

#[gpui::test]
fn an_opener_gone_while_open_sends_the_keyboard_to_the_fallback(cx: &mut TestAppContext) {
    let (host, cx) = opened(cx);
    host.update(cx, |host, cx| {
        host.opener = None;
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    assert_eq!(closes(&host, cx).1, [Returned::Fallback]);
    assert!(focused(|host, _| host.window.clone(), &host, cx));
}

#[gpui::test]
fn what_the_user_moved_the_keyboard_to_keeps_it(cx: &mut TestAppContext) {
    let (host, cx) = opened(cx);
    cx.update(|window, cx| {
        let other = host.read(cx).other.clone();
        window.focus(&other, cx);
        host.update(cx, |host, cx| {
            host.close(Close::new(Why::Toggled, By::Keyboard), window, cx)
        });
    });
    cx.run_until_parked();
    assert_eq!(closes(&host, cx).1, [Returned::Left]);
    assert!(focused(|host, _| host.other.clone(), &host, cx));
}

/// An input method composing "ぴ" in the panel's field, as Japanese input does before it's chosen.
fn compose(host: &Entity<Host>, cx: &mut VisualTestContext) -> Entity<TextInput> {
    let field = host.read_with(cx, |host, _| host.field.clone());
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.replace_and_mark_text_in_range(None, "ぴ", None, window, cx)
        })
    });
    cx.run_until_parked();
    field
}

/// The input method ends its composition as it does when it takes the key itself: committing
/// `text` in its place, or with `text` empty dropping it.
fn end_composition(field: &Entity<TextInput>, text: &str, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        field.update(cx, |field, cx| {
            field.replace_text_in_range(None, text, window, cx)
        })
    });
    cx.run_until_parked();
}

/// What the field shows, and whether it's still marked.
fn marked(field: &Entity<TextInput>, cx: &mut VisualTestContext) -> (String, bool) {
    field.read_with(cx, |field, _| (field.text().to_string(), field.composing()))
}

/// Escape while an input method composes is the composition's: the panel stays and the marked
/// text is left for the method. Once the method has dropped it, the next Escape closes the panel.
#[gpui::test]
fn escape_while_an_input_method_composes_closes_nothing(cx: &mut TestAppContext) {
    let (host, cx) = opened(cx);
    let field = compose(&host, cx);
    cx.simulate_keystrokes("escape");
    assert_eq!(closes(&host, cx).0, []);
    assert!(
        host.read_with(cx, |host, _| host.open.is_some()),
        "the panel closed"
    );
    assert_eq!(
        marked(&field, cx),
        ("ぴ".into(), true),
        "Escape ended the composition"
    );
    end_composition(&field, "", cx);
    cx.simulate_keystrokes("escape");
    assert_eq!(closes(&host, cx).0, [Close::new(Why::Escape, By::Keyboard)]);
}

/// Return while an input method composes is the composition's: nothing is submitted and the
/// marked text is left for the method. Once the method has committed it, the next Return submits.
#[gpui::test]
fn return_while_an_input_method_composes_submits_nothing(cx: &mut TestAppContext) {
    let (host, cx) = opened(cx);
    let field = compose(&host, cx);
    cx.simulate_keystrokes("enter");
    assert_eq!(host.read_with(cx, |host, _| host.submitted), 0);
    assert_eq!(
        marked(&field, cx),
        ("ぴ".into(), true),
        "Return ended the composition"
    );
    end_composition(&field, "ぴ", cx);
    assert_eq!(marked(&field, cx), ("ぴ".into(), false));
    cx.simulate_keystrokes("enter");
    assert_eq!(host.read_with(cx, |host, _| host.submitted), 1);
    assert!(
        host.read_with(cx, |host, _| host.open.is_some()),
        "Return closed the panel"
    );
}

/// With nothing composing, Return submits at once.
#[gpui::test]
fn return_submits_with_nothing_composing(cx: &mut TestAppContext) {
    let (host, cx) = opened(cx);
    cx.simulate_input("ab");
    cx.simulate_keystrokes("enter");
    assert_eq!(host.read_with(cx, |host, _| host.submitted), 1);
    let field = host.read_with(cx, |host, _| host.field.clone());
    assert_eq!(
        field.read_with(cx, |field, _| field.text().to_string()),
        "ab"
    );
}

/// Opened from the page, the panel gives the page the keyboard back on Escape, but not while
/// another window of ours, as an overlay is, has the keyboard: the window's own focus takes it.
#[gpui::test]
fn the_page_gets_the_keyboard_back_only_while_no_other_overlay_has_it(cx: &mut TestAppContext) {
    let (host, cx) = opened(cx);
    let from_page = |host: &Entity<Host>, cx: &mut VisualTestContext| {
        cx.update(|window, cx| {
            host.update(cx, |host, cx| {
                host.open = Some(FocusReturn {
                    opener: Some(Opener::Page),
                });
                window.focus(&host.field.focus_handle(cx), cx);
                cx.notify();
            })
        });
        cx.run_until_parked();
    };
    from_page(&host, cx);
    cx.simulate_keystrokes("escape");
    assert_eq!(closes(&host, cx).1, [Returned::Page]);

    from_page(&host, cx);
    let overlay = cx.update(|_, cx| {
        cx.open_window(Default::default(), |_, cx| cx.new(|_| gpui::Empty))
            .unwrap()
    });
    cx.update(|_, cx| {
        overlay
            .update(cx, |_, window, _| window.activate_window())
            .unwrap()
    });
    cx.run_until_parked();
    host.update_in(cx, |host, window, cx| {
        host.close(Close::new(Why::Escape, By::Keyboard), window, cx)
    });
    assert_eq!(closes(&host, cx).1, [Returned::Page, Returned::Fallback]);
    assert!(focused(|host, _| host.window.clone(), &host, cx));
}

/// The rule for every reason a surface closes: Escape, its own key and a choice in it give the
/// keyboard back to the opener, leaving it goes to the fallback, and whatever took the keyboard
/// in the meantime, the user or the choice's own result, keeps it.
#[gpui::test]
fn where_the_keyboard_goes_for_each_reason_a_surface_closes(cx: &mut TestAppContext) {
    let reasons = [
        (Why::Escape, Returned::Opener),
        (Why::Toggled, Returned::Opener),
        (Why::Chosen, Returned::Opener),
        (Why::PressOutside, Returned::Fallback),
        (Why::FocusLeft, Returned::Fallback),
        (Why::WindowLeft, Returned::Fallback),
    ];
    let (host, cx) = opened(cx);
    for (why, back) in reasons {
        for moved in [false, true] {
            cx.update(|window, cx| {
                let opener = host.read(cx).opener.clone().unwrap();
                window.focus(&opener, cx);
                host.update(cx, |host, cx| host.open(window, cx));
                if moved {
                    window.focus(&host.read(cx).other.clone(), cx);
                }
                host.update(cx, |host, cx| {
                    host.close(Close::new(why, By::Keyboard), window, cx)
                });
            });
            cx.run_until_parked();
            let returned = host.read_with(cx, |host, _| *host.returned.last().unwrap());
            let (expected, has) = match moved {
                true => (
                    Returned::Left,
                    focused(|host, _| host.other.clone(), &host, cx),
                ),
                false => (
                    back,
                    match back {
                        Returned::Opener => {
                            focused(|host, _| host.opener.clone().unwrap(), &host, cx)
                        }
                        _ => focused(|host, _| host.window.clone(), &host, cx),
                    },
                ),
            };
            assert_eq!(returned, expected, "{why:?}, moved in between: {moved}");
            assert!(
                has,
                "{why:?}, moved in between: {moved}: the keyboard isn't where it went"
            );
        }
    }
}
