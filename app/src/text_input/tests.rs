//! A field as an input method asks it where its caret is, and as an assistive app reads it.

use gpui::{
    Bounds, TestAppContext, VisualTestContext,
    accesskit::{Node, NodeId, TextSelection, TreeUpdate},
};

use super::*;

/// A window showing a field alone, focused.
struct Shows(Entity<TextInput>);

impl Render for Shows {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.0.clone())
    }
}

fn shown(text: &str, cx: &mut TestAppContext) -> (Entity<TextInput>, VisualTestContext) {
    shown_as(text, false, cx)
}

fn shown_as(
    text: &str,
    secure: bool,
    cx: &mut TestAppContext,
) -> (Entity<TextInput>, VisualTestContext) {
    cx.update(bind_keys);
    let (shows, cx) = cx.add_window_view(|window, cx| {
        let input = cx.new(|cx| TextInput::new("Search or Enter URL…", cx).secure(secure));
        window.focus(&input.focus_handle(cx), cx);
        Shows(input)
    });
    let input = cx.read(|cx| shows.read(cx).0.clone());
    if !text.is_empty() {
        cx.simulate_input(text);
    }
    cx.run_until_parked();
    (input, cx.clone())
}

fn caret(
    input: &Entity<TextInput>,
    at: usize,
    cx: &mut VisualTestContext,
) -> Option<Bounds<Pixels>> {
    let field = Bounds::new(point(px(10.), px(20.)), size(px(200.), px(30.)));
    input.update_in(cx, |input, window, cx| {
        input.bounds_for_range(at..at, field, window, cx)
    })
}

// Under Wayland the browser's input method composes for a field only once it knows where its
// caret is, and the command bar opens empty, showing its placeholder.
#[gpui::test]
fn an_empty_field_has_its_caret_at_its_start(cx: &mut TestAppContext) {
    let (input, mut cx) = shown("", cx);
    let caret = caret(&input, 0, &mut cx).expect("no caret");
    assert_eq!(
        (caret.origin, caret.size.width),
        (point(px(10.), px(20.)), px(0.))
    );
}

#[gpui::test]
fn a_field_has_its_caret_after_what_it_holds(cx: &mut TestAppContext) {
    let (input, mut cx) = shown("abc", cx);
    let caret = caret(&input, 3, &mut cx).expect("no caret");
    assert!(
        caret.origin.x > px(10.),
        "the caret is at {:?}",
        caret.origin
    );
}

/// The field and its text run, as an assistive app reads them.
fn read(cx: &mut VisualTestContext) -> (Node, Node, NodeId) {
    let tree: TreeUpdate = cx.a11y_tree().expect("GPUI built a tree");
    let node = |id: NodeId| {
        tree.nodes
            .iter()
            .find(|(each, _)| *each == id)
            .unwrap()
            .1
            .clone()
    };
    let field = node(tree.focus);
    let run = field
        .children()
        .iter()
        .copied()
        .find(|&id| node(id).role() == Role::TextRun);
    let run = run.expect("the field has no text run");
    (field, node(run), run)
}

// Orca reads a field's text and caret through AT-SPI's Text, which AccessKit gives only a field
// with text runs, in characters as the user sees them: a thumbs-up with its skin tone is one.
#[gpui::test]
fn an_assistive_app_reads_the_text_and_the_selection_in_it(cx: &mut TestAppContext) {
    let (_, mut cx) = shown("née 👍🏽", cx);
    cx.simulate_a11y_activation();
    cx.simulate_keystrokes("shift-left shift-left");
    let (field, run, id) = read(&mut cx);
    assert_eq!(
        (field.role(), field.value()),
        (Role::TextInput, Some("née 👍🏽"))
    );
    assert_eq!(run.value(), Some("née 👍🏽"));
    assert_eq!(run.character_lengths(), [1, 2, 1, 1, 8]);
    let at = |character_index| accesskit::TextPosition {
        node: id,
        character_index,
    };
    // Selected leftwards from the end, so the caret, the focus, is before the space.
    assert_eq!(
        field.text_selection(),
        Some(&TextSelection {
            anchor: at(5),
            focus: at(3)
        })
    );
}

// A password field reads as one, and gives the dots it shows, never what was typed.
#[gpui::test]
fn an_assistive_app_reads_a_secure_fields_dots(cx: &mut TestAppContext) {
    let (_, mut cx) = shown_as("hunter2", true, cx);
    cx.simulate_a11y_activation();
    let (field, run, _) = read(&mut cx);
    assert_eq!(
        (field.role(), field.value()),
        (Role::PasswordInput, Some("•••••••"))
    );
    assert_eq!(run.value(), Some("•••••••"));
    let tree = cx.a11y_tree().unwrap();
    assert!(
        !tree
            .nodes
            .iter()
            .any(|(_, node)| format!("{node:?}").contains("hunter2"))
    );
}
