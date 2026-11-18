//! What a card shows of the answers kept, and what goes when.

use gpui::TestAppContext;

use super::*;

fn row(list: List, number: u64) -> Row {
    Row {
        list,
        repo: "acme/web".into(),
        number,
        title: format!("Change {number}").into(),
        draft: false,
    }
}

fn rows(answered: f64, rows: Vec<Row>) -> Answer {
    Answer::Rows { rows, answered }
}

fn shows(profile: u64, cx: &mut TestAppContext) -> Recent {
    cx.update(|cx| shown(profile, |at| format!("{} min ago", at as i64), cx))
}

fn numbers(recent: &Recent) -> Vec<(u64, bool)> {
    match recent {
        Recent::Rows { rows, .. } => rows.iter().map(|(row, new)| (row.number, *new)).collect(),
        _ => Vec::new(),
    }
}

fn on(cx: &mut TestAppContext) {
    cx.update(|cx| set_switches(true, HashSet::new(), cx));
}

#[test]
fn only_github_pages_have_rows() {
    assert_eq!(source("https://github.com"), Some("github.com"));
    assert_eq!(source("https://github.com/pulls"), Some("github.com"));
    assert_eq!(source("https://github.com?q=1"), Some("github.com"));
    assert_eq!(source("https://github.community/"), None);
    assert_eq!(source("https://github.com.example.org/"), None);
    assert_eq!(source("https://gist.github.com/"), None);
    assert_eq!(source("http://github.com/"), None);
}

#[gpui::test]
fn nothing_kept_is_loading_then_the_rows_review_requests_first(cx: &mut TestAppContext) {
    on(cx);
    assert_eq!(shows(1, cx), Recent::Loading);
    let answer = rows(
        1.,
        vec![
            row(List::Created, 7),
            row(List::Review, 9),
            row(List::Created, 8),
            row(List::Review, 3),
        ],
    );
    cx.update(|cx| keep(1, answer, cx));
    // Three at most, none new on the first answer, and another profile still has none.
    assert_eq!(numbers(&shows(1, cx)), [(9, false), (3, false), (7, false)]);
    assert_eq!(shows(2, cx), Recent::Loading);
}

#[gpui::test]
fn a_row_that_came_since_is_new_until_opened(cx: &mut TestAppContext) {
    on(cx);
    cx.update(|cx| keep(1, rows(1., vec![row(List::Review, 1)]), cx));
    let later = vec![row(List::Review, 2), row(List::Review, 1)];
    cx.update(|cx| keep(1, rows(2., later.clone()), cx));
    assert_eq!(numbers(&shows(1, cx)), [(2, true), (1, false)]);
    // Still new on the answer after, until it's opened.
    cx.update(|cx| keep(1, rows(3., later.clone()), cx));
    assert_eq!(numbers(&shows(1, cx)), [(2, true), (1, false)]);
    cx.update(|cx| opened(1, &later[0], cx));
    assert_eq!(numbers(&shows(1, cx)), [(2, false), (1, false)]);
}

#[gpui::test]
fn a_failed_ask_keeps_the_rows_and_says_their_age(cx: &mut TestAppContext) {
    on(cx);
    cx.update(|cx| keep(1, Answer::Failed, cx));
    assert_eq!(shows(1, cx), Recent::Unreachable);
    cx.update(|cx| keep(1, rows(12., vec![row(List::Created, 4)]), cx));
    cx.update(|cx| keep(1, Answer::Failed, cx));
    let Recent::Rows { rows, stale } = shows(1, cx) else {
        panic!("no rows");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(
        stale.as_deref(),
        Some("Couldn't refresh · Updated 12 min ago")
    );
    // The next answer is fresh again.
    cx.update(|cx| {
        keep(
            1,
            Answer::Rows {
                rows: Vec::new(),
                answered: 13.,
            },
            cx,
        )
    });
    assert_eq!(
        shows(1, cx),
        Recent::Rows {
            rows: Vec::new(),
            stale: None
        }
    );
}

#[gpui::test]
fn signed_out_drops_the_rows(cx: &mut TestAppContext) {
    on(cx);
    cx.update(|cx| keep(1, rows(1., vec![row(List::Review, 1)]), cx));
    cx.update(|cx| keep(1, Answer::SignedOut, cx));
    assert_eq!(shows(1, cx), Recent::SignedOut);
    // A failure after keeps nothing to show.
    cx.update(|cx| keep(1, Answer::Failed, cx));
    assert_eq!(shows(1, cx), Recent::SignedOut);
}

#[gpui::test]
fn either_switch_off_forgets_every_profiles_rows(cx: &mut TestAppContext) {
    on(cx);
    assert!(cx.update(|cx| super::shows("github.com", cx)));
    cx.update(|cx| keep(1, rows(1., vec![row(List::Review, 1)]), cx));
    cx.update(|cx| set_site_on("github.com", false, cx));
    assert!(!cx.update(|cx| super::shows("github.com", cx)));
    assert_eq!(shows(1, cx), Recent::Loading);
    cx.update(|cx| set_site_on("github.com", true, cx));
    cx.update(|cx| keep(2, rows(1., vec![row(List::Review, 1)]), cx));
    cx.update(|cx| set_on(false, cx));
    assert!(!cx.update(|cx| super::shows("github.com", cx)));
    // Off, an answer still on its way is dropped.
    cx.update(|cx| keep(2, rows(2., vec![row(List::Review, 1)]), cx));
    assert_eq!(shows(2, cx), Recent::Loading);
}
