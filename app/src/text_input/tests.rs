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

/// ⌘⌫ deletes what's before the caret and ⌥⌫ the word before it, as in any Mac field, or the
/// selection where there is one, and an input method's composition keeps them (#1076).
#[gpui::test]
fn cmd_and_option_backspace_delete_back_to_the_start_and_a_word(cx: &mut TestAppContext) {
    let (input, mut cx) = shown("example.com/some/path", cx);
    let keys = |keys: &str, cx: &mut VisualTestContext| {
        cx.simulate_keystrokes(keys);
        cx.run_until_parked();
        input.read_with(cx, |input, _| (input.text().to_string(), input.selection()))
    };
    let cx = &mut cx;
    assert_eq!(
        keys("alt-backspace", cx),
        ("example.com/some/".into(), 17..17)
    );
    assert_eq!(keys("alt-backspace", cx), ("example.com/".into(), 12..12));
    assert_eq!(keys("left left cmd-backspace", cx), ("m/".into(), 0..0));
    // At the start there's nothing before the caret to delete.
    assert_eq!(keys("cmd-backspace", cx), ("m/".into(), 0..0));
    assert_eq!(keys("alt-backspace", cx), ("m/".into(), 0..0));
    assert_eq!(keys("cmd-a cmd-backspace", cx), (String::new(), 0..0));
    keys("x", cx);
    input.update_in(cx, |input, window, cx| {
        input.replace_and_mark_text_in_range(None, "か", None, window, cx)
    });
    let composing = ("xか".to_string(), 4..4);
    assert_eq!(keys("cmd-backspace", cx), composing);
    assert_eq!(keys("alt-backspace", cx), composing);
    assert!(input.read_with(cx, |input, _| input.composing()));
}

thread_local! {
    static ASKED: std::cell::RefCell<Option<crate::clipboard::Done>> =
        const { std::cell::RefCell::new(None) };
}

/// With the browser's reader, as under Wayland, a field pastes what the reader hands it once it
/// has it, not what GPUI's clipboard holds: GPUI there can't read a copy Chromium holds (#2463).
#[gpui::test]
fn a_paste_lands_once_the_browsers_reader_answers(cx: &mut TestAppContext) {
    crate::clipboard::read_through(|done| ASKED.set(Some(done)));
    let (input, mut cx) = shown("go ", cx);
    let text =
        |cx: &mut VisualTestContext| input.read_with(cx, |input, _| input.text().to_string());
    cx.write_to_clipboard(ClipboardItem::new_string("gpui's".into()));
    cx.simulate_keystrokes("cmd-v");
    cx.run_until_parked();
    assert_eq!(text(&mut cx), "go ");
    let done = ASKED.take().expect("the field asked the reader");
    // From the app, outside any window's update, as the core hands it from a task of its own.
    cx.cx.update(|cx| done(Some("chromium's".into()), cx));
    cx.run_until_parked();
    assert_eq!(text(&mut cx), "go chromium's");
    assert!(input.read_with(&cx, |input, _| input.pasted()));
}

/// A window showing a field 120 wide that fades its edges, focused.
struct Narrow(Entity<TextInput>);

impl Render for Narrow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(120.)).child(self.0.clone())
    }
}

/// A long ask in ⌘T scrolls to keep the caret in view, clear of the fade over an edge with more
/// text past it, and Home brings its start back.
#[gpui::test]
fn a_long_text_scrolls_to_keep_the_caret_in_view(cx: &mut TestAppContext) {
    cx.update(bind_keys);
    let (shows, cx) = cx.add_window_view(|window, cx| {
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Ask Claude…", cx);
            input.set_fade(px(24.), hsla(0., 0., 1., 1.));
            input
        });
        window.focus(&input.focus_handle(cx), cx);
        Narrow(input)
    });
    let input = cx.read(|cx| shows.read(cx).0.clone());
    let typed = "find the museum in Lisbon with the most azulejos and buy two tickets";
    cx.simulate_input(typed);
    cx.run_until_parked();
    // Where the caret is, against the field.
    let caret = |cx: &mut VisualTestContext| {
        input.update_in(cx, |input, window, cx| {
            let field = input.last_bounds.expect("drawn");
            let at = input.cursor_offset();
            let caret = input
                .bounds_for_range(at..at, field, window, cx)
                .expect("a caret");
            (caret.left() - field.left(), field.size.width, input.scroll)
        })
    };
    let (x, width, scroll) = caret(cx);
    assert!(scroll > px(0.), "the line didn't scroll");
    assert!(
        x > px(0.) && x + CARET <= width,
        "the caret is at {x:?} of {width:?}"
    );

    cx.simulate_keystrokes("home");
    cx.run_until_parked();
    assert_eq!(caret(cx), (px(0.), width, px(0.)), "the start is back");
    // Moving within what shows leaves the line where it is, and past the fade at the end scrolls.
    cx.simulate_keystrokes("right right");
    cx.run_until_parked();
    assert_eq!(caret(cx).2, px(0.));
    for _ in 0..typed.len() / 2 {
        cx.simulate_keystrokes("right");
    }
    cx.run_until_parked();
    let (x, width, scroll) = caret(cx);
    assert!(scroll > px(0.), "the line didn't follow the caret");
    assert!(
        x <= width - px(24.) - CARET + px(0.5),
        "the caret is under the fade, at {x:?}"
    );
}

/// The line moves only as far as it must: a caret already in view leaves it, one past an edge
/// brings it just clear of that edge's fade, and it never scrolls past the text's end.
#[test]
fn a_line_scrolls_no_further_than_it_must() {
    let (width, fade) = (px(100.), px(24.));
    assert_eq!(
        scrolled(px(0.), px(50.), px(80.), width, fade),
        px(0.),
        "it fits"
    );
    assert_eq!(
        scrolled(px(0.), px(50.), px(300.), width, fade),
        px(0.),
        "in view"
    );
    assert_eq!(scrolled(px(0.), px(90.), px(300.), width, fade), px(16.));
    assert_eq!(
        scrolled(px(16.), px(300.), px(300.), width, fade),
        px(202.),
        "at the end"
    );
    assert_eq!(
        scrolled(px(202.), px(150.), px(300.), width, fade),
        px(126.)
    );
    assert_eq!(
        scrolled(px(202.), px(150.), px(120.), width, fade),
        px(22.),
        "text deleted"
    );
}

/// The text the field shows after each of `calls`: what's committed, then what's composing.
fn composed(calls: &[korean::Ime]) -> Vec<String> {
    let mut committed = String::new();
    (calls.iter())
        .map(|call| match call {
            korean::Ime::Mark(marked) => format!("{committed}{marked}"),
            korean::Ime::Commit(syllable) => {
                committed.push_str(syllable);
                committed.clone()
            }
        })
        .collect()
}

/// Korean from a 2-set keyboard, where one key can commit a syllable and start the next (#2457).
/// Each syllable stays whole, the caret follows what's composing, and Find searches each committed
/// syllable as it comes, never a lone consonant or vowel.
#[gpui::test]
fn korean_composes_syllable_by_syllable(cx: &mut TestAppContext) {
    for (word, calls) in [("한글", korean::HANGEUL), ("안녕하세요", korean::ANNYEONG)] {
        let (input, mut cx) = shown("", cx);
        let mut searched = String::new();
        let mut finds = Vec::new();
        for (call, shows) in calls.iter().zip(composed(calls)) {
            input.update_in(&mut cx, |input, window, cx| call.apply(input, window, cx));
            let (text, selection, composing) = input.read_with(&cx, |input, _| {
                if let Some(text) = input.committed_change(&searched) {
                    searched = text.to_string();
                    finds.push(searched.clone());
                }
                (
                    input.text().to_string(),
                    input.selection(),
                    input.composing(),
                )
            });
            assert_eq!(text, shows, "after {call:?}");
            assert_eq!(selection, text.len()..text.len(), "after {call:?}");
            assert_eq!(
                composing,
                matches!(call, korean::Ime::Mark(_)),
                "after {call:?}"
            );
        }
        let last = word.chars().last().unwrap().to_string();
        input.update_in(&mut cx, |input, window, cx| {
            input.replace_text_in_range(None, &last, window, cx)
        });
        input.read_with(&cx, |input, _| {
            assert_eq!(input.text(), word);
            assert!(!input.composing());
            finds.push(input.committed_change(&searched).unwrap().to_string());
        });
        let mut syllables = String::new();
        let expected: Vec<String> = (word.chars())
            .map(|syllable| {
                syllables.push(syllable);
                syllables.clone()
            })
            .collect();
        assert_eq!(finds, expected);
    }
}

/// Backspace while a syllable composes takes its last letter off, as the Korean input method
/// rebuilds it, down to nothing; then it deletes committed text as usual. Korean composes in the
/// middle of text too.
#[gpui::test]
fn korean_backspace_takes_a_syllable_apart(cx: &mut TestAppContext) {
    let (input, mut cx) = shown("", cx);
    let compose = |input: &Entity<TextInput>, calls: &[korean::Ime], cx: &mut VisualTestContext| {
        for call in calls {
            input.update_in(cx, |input, window, cx| call.apply(input, window, cx));
        }
    };
    compose(&input, korean::HANGEUL, &mut cx);
    use korean::Ime::Mark;
    compose(&input, &[Mark("그"), Mark("ㄱ"), Mark("")], &mut cx);
    let shows = |input: &Entity<TextInput>, cx: &mut VisualTestContext| {
        input.read_with(cx, |input, _| {
            (
                input.text().to_string(),
                input.selection(),
                input.composing(),
            )
        })
    };
    assert_eq!(shows(&input, &mut cx), ("한".into(), 3..3, false));
    cx.simulate_keystrokes("backspace");
    assert_eq!(shows(&input, &mut cx), (String::new(), 0..0, false));

    cx.simulate_input("ab");
    cx.simulate_keystrokes("left");
    compose(&input, korean::HANGEUL, &mut cx);
    assert_eq!(shows(&input, &mut cx), ("a한글b".into(), 7..7, true));
}
