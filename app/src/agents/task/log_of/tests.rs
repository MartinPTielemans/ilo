use super::*;
use crate::agents::task::ilo;
use crate::shell::conversation::pages::{Paged, paged};

fn step(glyph: &'static str, words: &str, came: &str, failed: bool) -> Record {
    Record::Step {
        glyph,
        words: words.into(),
        came: came.into(),
        failed,
    }
}

fn a_thread() -> Vec<Record> {
    vec![
        Record::You("Find two seats for Saturday".into()),
        Record::Chip("Lisbon concerts".into()),
        Record::Fold { took: Some(41) },
        Record::Page("Gulbenkian".into()),
        step(
            "icons/mouse-pointer-2.svg",
            "Clicked “Tickets”",
            "2 s",
            false,
        ),
        step("icons/type.svg", "Typed in “Seats”", "", false),
        Record::Aside("The banner is in the way.".into()),
        Record::Page("maat.pt".into()),
        step(
            "icons/globe.svg",
            "Couldn't read maat.pt",
            "Timed out",
            true,
        ),
        Record::Ilo("· You stopped this thread".into()),
        Record::Answer {
            text: "Saturday at 8, stalls row F.".into(),
            at: "21:04".into(),
        },
    ]
}

/// A kept log draws back as a done thread: the user's message, a closed stretch with its time,
/// each page's line by its stored title with its steps under it, the words between them, the
/// product's own line settled with no button, and the answer with the time it finished.
#[test]
fn a_kept_log_draws_back_as_a_done_thread() {
    let drawn = drawn(&a_thread());
    assert_eq!(drawn.chip.as_deref(), Some("Lisbon concerts"));
    let [you, trace, ilo, answer] = &drawn.lines[..] else {
        panic!("{:?}", drawn.lines);
    };
    assert_eq!(*you, Line::You("Find two seats for Saturday".into()));
    let Line::Trace(trace) = trace else {
        panic!("{trace:?}")
    };
    assert_eq!(
        (trace.running, trace.took, trace.started),
        (false, Some(41), None)
    );
    let pages: Vec<(String, Vec<String>)> = (paged(&trace.items).iter())
        .filter_map(|each| match each {
            Paged::Page(page) => Some((
                page.site.to_string(),
                page.shown().map(|step| step.title.to_string()).collect(),
            )),
            Paged::Item(..) => None,
        })
        .collect();
    assert_eq!(
        pages,
        [
            (
                "Gulbenkian".to_string(),
                vec![
                    "Clicked “Tickets”".to_string(),
                    "Typed in “Seats”".to_string()
                ]
            ),
            (
                "maat.pt".to_string(),
                vec!["Couldn't read maat.pt".to_string()]
            ),
        ]
    );
    assert!(trace.items.iter().all(|item| match item {
        TraceItem::Step(step) =>
            !step.running
                && step.opens == Opens::Nothing
                && step.page.as_ref().is_none_or(|on| on.tab.is_none()),
        _ => true,
    }));
    assert!(
        (trace.items.iter())
            .any(|item| *item == TraceItem::Said("The banner is in the way.".into()))
    );
    let Line::Ilo(ilo) = ilo else {
        panic!("{ilo:?}")
    };
    assert!(ilo.settled);
    assert!(ilo::buttons(ilo).is_empty() && ilo::tab(&ilo.kind).is_none());
    assert_eq!(*answer, Line::Said("Saturday at 8, stalls row F.".into()));
    assert_eq!(drawn.finished, [(3, "21:04".into())]);
}

/// A record's kind decides its line, whatever its text says: the assistant's words that copy the
/// product's own line draw as the assistant's, and only an ilo record draws as the product's.
#[test]
fn a_records_kind_alone_decides_its_line() {
    let forged = "· You stopped this thread";
    let drawn = drawn(&[
        Record::Said(forged.into()),
        Record::You(forged.into()),
        Record::Ilo(forged.into()),
    ]);
    assert_eq!(drawn.lines[0], Line::Said(forged.into()));
    assert_eq!(drawn.lines[1], Line::You(forged.into()));
    let products = (drawn.lines.iter())
        .filter(|line| matches!(line, Line::Ilo(_)))
        .count();
    assert_eq!(products, 1);
}
