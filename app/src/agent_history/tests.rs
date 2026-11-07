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
