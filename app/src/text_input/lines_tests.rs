//! A composer's field of several lines: ⇧↩ starts a line, a paste keeps its lines, and the field
//! grows to its rows, then scrolls, while ↩ stays its owner's.

use gpui::{TestAppContext, VisualTestContext};

use super::*;

/// A window showing a field 240 wide, focused, which says when ↩ reaches it.
struct Composes {
    input: Entity<TextInput>,
    sent: usize,
}

actions!(lines_tests, [Send]);

impl Render for Composes {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("Composes")
            .on_action(cx.listener(|this, _: &Send, _, _| this.sent += 1))
            .w(px(240.))
            .child(self.input.clone())
    }
}

fn composer(rows: Option<usize>, cx: &mut TestAppContext) -> (Entity<Composes>, VisualTestContext) {
    cx.update(|cx| {
        bind_keys(cx);
        cx.bind_keys([KeyBinding::new("enter", Send, Some("Composes"))]);
    });
    let (shows, cx) = cx.add_window_view(|window, cx| {
        let input = cx.new(|cx| {
            let input = TextInput::new("Reply", cx);
            match rows {
                Some(rows) => input.multi_line(rows),
                None => input,
            }
        });
        window.focus(&input.focus_handle(cx), cx);
        Composes { input, sent: 0 }
    });
    cx.run_until_parked();
    (shows, cx.clone())
}

fn text(shows: &Entity<Composes>, cx: &mut VisualTestContext) -> String {
    let input = cx.read(|cx| shows.read(cx).input.clone());
    input.read_with(cx, |input, _| input.text().to_string())
}

fn sent(shows: &Entity<Composes>, cx: &mut VisualTestContext) -> usize {
    cx.read(|cx| shows.read(cx).sent)
}

#[gpui::test]
fn shift_enter_starts_a_line_without_sending(cx: &mut TestAppContext) {
    let (shows, mut cx) = composer(Some(8), cx);
    cx.simulate_input("first");
    cx.simulate_keystrokes("shift-enter");
    cx.simulate_input("second");
    cx.run_until_parked();
    assert_eq!(text(&shows, &mut cx), "first\nsecond");
    assert_eq!(sent(&shows, &mut cx), 0);
    cx.simulate_keystrokes("enter");
    assert_eq!(sent(&shows, &mut cx), 1, "↩ stays the owner's");
}

#[gpui::test]
fn a_field_of_one_line_leaves_shift_enter_to_its_owner(cx: &mut TestAppContext) {
    let (shows, mut cx) = composer(None, cx);
    cx.simulate_input("one");
    cx.simulate_keystrokes("shift-enter");
    assert_eq!(text(&shows, &mut cx), "one");
}

/// Japanese IME: while it composes, ↩ commits what it holds and sends nothing, and ⇧↩ adds no line.
#[gpui::test]
fn enter_while_composing_commits_without_sending(cx: &mut TestAppContext) {
    let (shows, mut cx) = composer(Some(8), cx);
    let input = cx.read(|cx| shows.read(cx).input.clone());
    input.update_in(&mut cx, |input, window, cx| {
        input.replace_and_mark_text_in_range(None, "にほん", None, window, cx)
    });
    cx.simulate_keystrokes("shift-enter");
    assert_eq!(text(&shows, &mut cx), "にほん");
    // The input method takes ↩ first, as the system's does, and commits.
    input.update_in(&mut cx, |input, window, cx| {
        input.replace_text_in_range(None, "日本", window, cx)
    });
    assert!(!input.read_with(&cx, |input, _| input.composing()));
    assert_eq!(text(&shows, &mut cx), "日本");
    assert_eq!(sent(&shows, &mut cx), 0);
}

#[gpui::test]
fn a_paste_keeps_its_lines(cx: &mut TestAppContext) {
    let (shows, mut cx) = composer(Some(8), cx);
    cx.write_to_clipboard(ClipboardItem::new_string("a\r\nb\rc\nd".into()));
    cx.simulate_keystrokes("cmd-v");
    cx.run_until_parked();
    assert_eq!(text(&shows, &mut cx), "a\nb\nc\nd");

    let (shows, mut cx) = composer(None, &mut cx.cx);
    cx.write_to_clipboard(ClipboardItem::new_string("a\nb".into()));
    cx.simulate_keystrokes("cmd-v");
    cx.run_until_parked();
    assert_eq!(text(&shows, &mut cx), "a b");
}

#[gpui::test]
fn the_field_grows_to_its_rows_then_scrolls(cx: &mut TestAppContext) {
    let (shows, mut cx) = composer(Some(3), cx);
    let input = cx.read(|cx| shows.read(cx).input.clone());
    let drawn = |cx: &mut VisualTestContext| {
        input.read_with(cx, |input, _| (input.rows_drawn(), input.grown()))
    };
    assert_eq!(drawn(&mut cx), (1, 0.));
    cx.simulate_input("one");
    cx.simulate_keystrokes("shift-enter");
    cx.simulate_input("two");
    cx.run_until_parked();
    let (rows, grown) = drawn(&mut cx);
    assert_eq!(rows, 2);
    assert!(grown > 0.);
    let row = grown;
    for line in ["three", "four", "five"] {
        cx.simulate_keystrokes("shift-enter");
        cx.simulate_input(line);
    }
    cx.run_until_parked();
    assert_eq!(drawn(&mut cx), (3, row * 2.), "no taller than its rows");
}

#[gpui::test]
fn up_and_down_go_a_line_and_past_the_ends_are_the_owners(cx: &mut TestAppContext) {
    let (shows, mut cx) = composer(Some(8), cx);
    let input = cx.read(|cx| shows.read(cx).input.clone());
    cx.simulate_input("abc");
    cx.simulate_keystrokes("shift-enter");
    cx.simulate_input("def");
    cx.run_until_parked();
    let caret = |cx: &mut VisualTestContext| input.read_with(cx, |input, _| input.cursor_offset());
    assert_eq!(caret(&mut cx), 7);
    cx.simulate_keystrokes("up");
    assert_eq!(caret(&mut cx), 3, "the same place a line up");
    cx.simulate_keystrokes("up");
    assert_eq!(caret(&mut cx), 3, "the first line's up isn't the field's");
    cx.simulate_keystrokes("shift-down");
    let selected = input.read_with(&cx, |input, _| input.selected_range.clone());
    assert_eq!(selected, 3..7);
}

/// A field that's a composer only some of the time joins its lines when it goes back to one.
#[gpui::test]
fn back_to_one_line_its_lines_join(cx: &mut TestAppContext) {
    let (shows, mut cx) = composer(Some(8), cx);
    let input = cx.read(|cx| shows.read(cx).input.clone());
    cx.simulate_input("a");
    cx.simulate_keystrokes("shift-enter");
    cx.simulate_input("b");
    input.update(&mut cx, |input, cx| input.set_multi_line(None, cx));
    cx.run_until_parked();
    assert_eq!(text(&shows, &mut cx), "a b");
    assert_eq!(input.read_with(&cx, |input, _| input.rows_drawn()), 1);
}
