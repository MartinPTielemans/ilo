//! What Library › Agents keeps of a past task, and for how long.

use super::*;

const DAY: u64 = 24 * 60 * 60;

fn past(id: &str, ended: u64) -> Past {
    Past {
        id: id.into(),
        title: "Book a table for four".into(),
        agent: "Claude Code".into(),
        started: ended - 60,
        ended,
        pages: vec![(
            "Booking · Taberna".into(),
            "https://taberna.example/booking".into(),
        )],
        orders: 1,
        messages: 0,
        steps: vec![
            step(ended - 50, "open_tab", "taberna.example", ""),
            step(ended - 40, "click", "taberna.example", ""),
            step(
                ended - 30,
                "navigate",
                "",
                "Pages outside http(s) are the user's.",
            ),
            Step {
                target: "POST /api/notes".into(),
                url: "https://taberna.example/api/notes".into(),
                sent: vec![
                    Sent {
                        depth: 0,
                        name: "note".into(),
                        value: String::new(),
                    },
                    Sent {
                        depth: 1,
                        name: "title".into(),
                        value: "Table for four".into(),
                    },
                ],
                ..step(ended - 20, "request", "taberna.example", "")
            },
        ],
        answers: "claude-code".into(),
    }
}

fn step(at: u64, action: &str, site: &str, refused: &str) -> Step {
    Step {
        at,
        action: action.into(),
        site: site.into(),
        refused: refused.into(),
        ..Default::default()
    }
}

/// An entry is read back as it was written, its pages and counts with it, and a line that isn't
/// one is left out.
#[test]
fn an_entry_reads_back_as_it_was_kept() {
    let entries = vec![past("task-2", 2_000_000), past("task-1", 1_000_000)];
    let text = write(&entries) + "not json\n";
    assert_eq!(read(&text), entries);
}

/// An entry kept before its steps and answers were has neither, and reads all the same.
#[test]
fn an_entry_from_before_steps_were_kept_reads_without_them() {
    let line = r#"{"id":"task-1","title":"Book","agent":"Claude Code","started":1,"ended":61,"pages":[],"orders":0,"messages":0}"#;
    let read = read(line);
    assert_eq!(read.len(), 1);
    assert!(read[0].steps.is_empty() && read[0].answers.is_empty());
}

/// Its page says how long it ran, and the sites it went to, from its log, then its pages, each
/// once.
#[test]
fn its_page_says_how_long_it_ran_and_where() {
    let mut past = past("task-1", 1_000_000);
    assert_eq!(past.length(), "1 min");
    past.started = past.ended - 30;
    assert_eq!(past.length(), "Under a minute");
    past.started = past.ended - 3900;
    assert_eq!(past.length(), "1 h 5 min");
    past.pages
        .push(("Menu".into(), "https://www.menu.example/today".into()));
    assert_eq!(past.sites(), ["taberna.example", "menu.example"]);
}

/// Kept, a task goes first; a later turn of the same task takes its place, and one that ended
/// 30 days ago or more goes as another is kept or as they're shown.
#[test]
fn entries_live_thirty_days_and_a_tasks_later_turn_replaces_it() {
    let now = 100 * DAY;
    let old = past("task-1", now - 30 * DAY);
    let young = past("task-2", now - 30 * DAY + 1);
    let entries = kept(vec![young.clone(), old], past("task-3", now), now);
    assert_eq!(
        entries
            .iter()
            .map(|each| each.id.as_str())
            .collect::<Vec<_>>(),
        ["task-3", "task-2"],
        "the 30-day-old entry stayed"
    );
    let again = Past {
        orders: 2,
        ..past("task-2", now)
    };
    let entries = kept(entries, again.clone(), now);
    assert_eq!(entries, vec![again, past("task-3", now)]);
    assert!(
        living(entries, now + KEEP).is_empty(),
        "entries lived past 30 days"
    );
}

#[test]
fn one_entry_deletes_alone() {
    let entries = vec![past("task-2", 2 * DAY), past("task-1", DAY)];
    assert_eq!(deleted(entries, "task-2"), vec![past("task-1", DAY)]);
}

/// Its row says its agent, when it ended, what it did that had consequences, and its pages.
#[test]
fn a_row_says_what_its_receipt_did() {
    let about = past("task-1", 1_000_000).about();
    assert!(about.starts_with("Claude Code · "), "{about}");
    assert!(about.ends_with(" · 1 order placed · 1 page"), "{about}");
}

/// A Point step (#2352) in the file keeps its page, its title and, per element, its kind alone,
/// its box, "Not sent" for its value and its picture's size: never the page's title or text, the
/// element's name, the user's note or the selector that found it, which Settings › Agents alone
/// shows, from memory (#2326's rule).
#[test]
fn a_saved_point_step_keeps_no_words_of_the_page_or_the_user() {
    use crate::point::{Element, Note};
    let element = Element {
        kind: "region".into(),
        name: "Ada Lovelace's orders".into(),
        address: "https://shop.example/orders".into(),
        title: "Orders for Ada".into(),
        tag: "section".into(),
        text: "Order 1815 for Ada Lovelace".into(),
        selector: "#ada-lovelace-orders".into(),
        frame: gpui::Bounds::new(
            gpui::point(gpui::px(10.), gpui::px(20.)),
            gpui::size(gpui::px(300.), gpui::px(200.)),
        ),
        withheld: true,
        png: Some(std::sync::Arc::from(&b"png"[..])),
        width: 316,
        height: 216,
        ..Element::default()
    };
    let note = Note {
        element,
        words: "the total is wrong".into(),
    };
    let sent = crate::point::sent(&[note]);
    let mut entry = past("point", 40 * DAY);
    entry.steps = vec![Step {
        target: "1 element".into(),
        url: "https://shop.example/orders".into(),
        sent,
        ..step(40 * DAY - 10, "pointed", "shop.example", "")
    }];
    let text = write(&[entry]);
    for said in [
        "Orders for Ada",
        "Ada Lovelace",
        "1815",
        "the total is wrong",
        "ada-lovelace",
    ] {
        assert!(!text.contains(said), "the file kept \"{said}\": {text}");
    }
    let kept = &read(&text)[0].steps[0];
    let lines: Vec<_> = (kept.sent.iter())
        .map(|line| format!("{} {}: {}", line.depth, line.name, line.value))
        .collect();
    assert_eq!(
        lines,
        [
            "0 1: ",
            "1 kind: Section",
            "1 box: 300 × 200 at 10, 20",
            "1 value: Not sent",
            "1 picture: 316 × 216 pixels",
        ]
    );
    assert_eq!(kept.url, "https://shop.example/orders");
    assert_eq!(kept.target, "1 element");
}
