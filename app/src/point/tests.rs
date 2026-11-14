use std::sync::Arc;

use gpui::{Bounds, point, px, size};

use super::*;

/// Trending, a section of the made-up Fieldnotes, as the core would report it.
fn trending() -> Element {
    Element {
        kind: "section".into(),
        name: "Trending".into(),
        address: "https://fieldnotes.example/recipes".into(),
        title: "Fieldnotes".into(),
        tag: "section".into(),
        text: "Trending\nLemon tart".into(),
        selector: "main > section:nth-of-type(2)".into(),
        color: "rgb(22, 20, 15)".into(),
        background: "rgb(255, 255, 255)".into(),
        font: "Inter".into(),
        font_size: "15px".into(),
        padding: "16px".into(),
        frame: Bounds::new(point(px(880.), px(120.)), size(px(300.), px(420.))),
        withheld: false,
        png: Some(Arc::from(&b"png"[..])),
        width: 632,
        height: 872,
    }
}

/// A sign-in form holding a filled email and password, as autofill leaves them: the core leaves
/// both values out of its text and says so.
fn sign_in() -> Element {
    Element {
        kind: "form".into(),
        name: "Sign in".into(),
        tag: "form".into(),
        text: "Sign in\nForgot your password?".into(),
        withheld: true,
        png: Some(Arc::from(&b"hatched"[..])),
        ..trending()
    }
}

#[test]
fn a_marks_label_is_arranges_kind_and_name() {
    assert_eq!(label("section", "Trending"), "Section · Trending");
    assert_eq!(label("region", ""), "Section");
    assert_eq!(label("input", "Password"), "Field · Password");
    assert_eq!(label("x-widget", "Map"), "Part of the page · Map");
}

#[test]
fn a_notes_label_is_its_number_and_words_once_added() {
    let note = Note {
        element: trending(),
        words: "This is the box on the right. Hide it.".into(),
    };
    assert_eq!(note.label(1), "1 · This is the box on the right. Hide it.");
    let empty = Note {
        words: "  ".into(),
        ..note
    };
    assert_eq!(empty.label(2), "2 · Section · Trending");
}

#[test]
fn the_panel_lists_each_row_that_goes_and_says_what_is_withheld() {
    let names: Vec<_> = trending().rows().iter().map(|row| row.name).collect();
    assert_eq!(
        names,
        [
            "Page", "Title", "Element", "Name", "Text", "Finds it", "Box", "Styles", "Picture"
        ]
    );
    assert_eq!(
        trending().folded(),
        "Sends the section, its text and this picture."
    );

    let rows = sign_in().rows();
    let value = rows.iter().find(|row| row.name == "Value").unwrap();
    assert!(value.withheld);
    assert!(value.value.starts_with("Not sent."));
    assert!(sign_in().folded().ends_with("No field's value is sent."));
}

#[test]
fn the_answer_is_the_notes_and_their_rows_and_nothing_else() {
    let notes = [
        Note {
            element: trending(),
            words: "Hide this.".into(),
        },
        Note {
            element: Element {
                png: None,
                ..sign_in()
            },
            words: String::new(),
        },
    ];
    let (text, pictures) = answer(&notes);
    assert!(text.starts_with("The user pointed at 2 elements."));
    assert!(
        text.contains("\n\n1. Their note: Hide this.\nPage: https://fieldnotes.example/recipes")
    );
    assert!(text.contains("\n\n2. They wrote no note.\n"));
    // Each row the panel shows, in its order, and no other.
    for note in &notes {
        for row in note.element.rows() {
            assert!(text.contains(&format!("\n{}: {}", row.name, row.value)));
        }
    }
    // A picture for each element that has one, in the notes' order.
    assert_eq!(pictures, [Arc::from(&b"png"[..])]);
    assert_eq!(logged(notes.len()), "You pointed at 2 elements");
    assert_eq!(logged(1), "You pointed at 1 element");
}

#[test]
fn the_tooltip_says_why_point_cant_start_and_the_agent_never_hears_it() {
    assert_eq!(why_not(0), None);
    assert!(why_not(1).unwrap().contains("web pages"));
    assert!(why_not(2).unwrap().contains("private window"));
    // Whatever stopped it, the agent's answer is the one that says nothing of the tab.
    assert!(!NOTHING.contains("tab") && !NOTHING.contains("page"));
}

/// The log's row opens to what was sent: the page's title, then each element by its number with
/// the user's note and the panel's rows, its kind alone, its value as "Not sent" and its picture as
/// its size, in the order the panel shows them.
#[test]
fn the_logs_row_holds_each_note_and_the_rows_sent() {
    let notes = [
        Note {
            element: trending(),
            words: " Hide this. ".into(),
        },
        Note {
            element: Element {
                png: None,
                ..sign_in()
            },
            words: String::new(),
        },
    ];
    let lines: Vec<_> = (sent(&notes).iter())
        .map(|line| format!("{} {}: {}", line.depth, line.name, line.value))
        .collect();
    assert_eq!(
        lines,
        [
            "0 title: Fieldnotes",
            "0 1: ",
            "1 kind: Section",
            "1 note: Hide this.",
            "1 name: Trending",
            "1 text: Trending\nLemon tart",
            "1 found by: main > section:nth-of-type(2)",
            "1 box: 300 × 420 at 880, 120",
            "1 styles: color rgb(22, 20, 15); background rgb(255, 255, 255); font Inter; size 15px; \
             padding 16px",
            "1 picture: 632 × 872 pixels",
            "0 2: ",
            "1 kind: Form",
            "1 name: Sign in",
            "1 text: Sign in\nForgot your password?",
            "1 found by: main > section:nth-of-type(2)",
            "1 box: 300 × 420 at 880, 120",
            "1 styles: color rgb(22, 20, 15); background rgb(255, 255, 255); font Inter; size 15px; \
             padding 16px",
            "1 value: Not sent",
            "1 picture: None: the page didn't give one.",
        ]
    );
}
