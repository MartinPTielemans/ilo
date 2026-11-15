//! What a Live Folder asks for and when, and what each answer leaves of its rows (#2979).

use std::time::Duration;

use web_time::Instant;

use super::*;

fn item(list: LiveList, repo: &str, number: u32, state: PullState) -> LiveItem {
    LiveItem {
        list,
        repo: repo.to_string().into(),
        number,
        title: format!("Change {number}").into(),
        state,
    }
}

fn mine(number: u32) -> LiveItem {
    item(LiveList::Created, "harbor/web", number, PullState::Open)
}

fn review(number: u32) -> LiveItem {
    item(
        LiveList::ReviewRequests,
        "quill/editor",
        number,
        PullState::Open,
    )
}

/// The numbers of its item rows, and its headings by their list's first letter.
fn shown(rows: &[LiveRow]) -> Vec<String> {
    (rows.iter())
        .map(|row| match row {
            LiveRow::Heading(LiveList::ReviewRequests) => "R".into(),
            LiveRow::Heading(LiveList::Created) => "C".into(),
            LiveRow::Item { item, new: true } => format!("{}*", item.number),
            LiveRow::Item { item, new: false } => item.number.to_string(),
            LiveRow::SignIn => "sign in".into(),
        })
        .collect()
}

#[test]
fn the_floor_holds_back_all_but_refresh() {
    let start = Instant::now();
    let mut folder = LiveFolder::default();
    assert!(folder.due(LiveEvent::SpaceShown, start));
    folder.ask(start);
    // Under way, nothing asks again, Refresh neither.
    assert!(!folder.due(LiveEvent::Refresh, start));
    folder.answer(LiveAnswer::Items(vec![mine(1)]), start);
    let soon = start + FLOOR - Duration::from_secs(1);
    for event in [
        LiveEvent::WindowActive,
        LiveEvent::SpaceShown,
        LiveEvent::Opened,
        LiveEvent::SiteCommitted,
    ] {
        assert!(!folder.due(event, soon), "{event:?} under the floor");
        assert!(folder.due(event, start + FLOOR), "{event:?} past the floor");
    }
    assert!(folder.due(LiveEvent::Refresh, soon));
}

#[test]
fn the_floor_counts_from_a_failed_try_too() {
    let start = Instant::now();
    let mut folder = LiveFolder::default();
    folder.ask(start);
    folder.answer(LiveAnswer::Failed, start);
    assert!(!folder.due(LiveEvent::WindowActive, start + Duration::from_secs(30)));
    assert!(folder.due(LiveEvent::Refresh, start + Duration::from_secs(30)));
}

#[test]
fn review_requests_come_first_each_under_its_heading() {
    let now = Instant::now();
    let mut folder = LiveFolder::default();
    folder.answer(
        LiveAnswer::Items(vec![mine(7), review(3), mine(8), review(4)]),
        now,
    );
    assert_eq!(
        shown(&folder.rows(|_| false)),
        ["R", "3", "4", "C", "7", "8"]
    );
    assert_eq!(folder.waiting(), 2);
}

#[test]
fn a_list_with_no_rows_has_no_heading() {
    let mut folder = LiveFolder::default();
    folder.answer(LiveAnswer::Items(vec![mine(7)]), Instant::now());
    assert_eq!(shown(&folder.rows(|_| false)), ["C", "7"]);
    // Its one item open as a tab, the tab's row stands for it.
    let open = mine(7).url();
    assert!(folder.rows(|url| url == open).is_empty());
    assert_eq!(folder.item_at(&open), Some(&mine(7)));
}

#[test]
fn a_gone_item_stays_only_while_its_tab_is_open() {
    let mut folder = LiveFolder::default();
    let merged = item(LiveList::Created, "harbor/web", 9, PullState::Merged);
    folder.answer(
        LiveAnswer::Items(vec![mine(7), merged.clone()]),
        Instant::now(),
    );
    // Not open, it has no row.
    assert_eq!(shown(&folder.rows(|_| false)), ["C", "7"]);
    // Open, its tab's row shows it merged.
    assert_eq!(
        folder.item_at(&merged.url()).map(|item| item.state),
        Some(PullState::Merged)
    );
    assert_eq!(folder.waiting(), 0);
}

#[test]
fn items_after_the_first_answer_are_new_until_opened() {
    let start = Instant::now();
    let mut folder = LiveFolder::default();
    folder.answer(LiveAnswer::Items(vec![review(3)]), start);
    assert_eq!(shown(&folder.rows(|_| false)), ["R", "3"]);
    folder.answer(LiveAnswer::Items(vec![review(4), review(3)]), start);
    assert_eq!(shown(&folder.rows(|_| false)), ["R", "4*", "3"]);
    // A failed refresh keeps the rows and what's new.
    folder.answer(LiveAnswer::Failed, start);
    assert_eq!(shown(&folder.rows(|_| false)), ["R", "4*", "3"]);
    folder.opened(&review(4).url());
    assert_eq!(shown(&folder.rows(|_| false)), ["R", "4", "3"]);
    // One that leaves and comes back is new again.
    folder.answer(LiveAnswer::Items(vec![review(3)]), start);
    folder.answer(LiveAnswer::Items(vec![review(3), review(4)]), start);
    assert_eq!(shown(&folder.rows(|_| false)), ["R", "3", "4*"]);
}

#[test]
fn signed_out_it_has_the_sign_in_row_alone() {
    let now = Instant::now();
    let mut folder = LiveFolder::default();
    folder.answer(LiveAnswer::Items(vec![review(3)]), now);
    folder.answer(LiveAnswer::SignedOut, now);
    assert_eq!(shown(&folder.rows(|_| false)), ["sign in"]);
    assert_eq!(folder.waiting(), 0);
    // Signing in fills it, with nothing new: the lists before aren't known.
    folder.answer(LiveAnswer::Items(vec![review(3), mine(7)]), now);
    assert_eq!(shown(&folder.rows(|_| false)), ["R", "3", "C", "7"]);
}

#[test]
fn the_tooltip_gives_the_rows_age_and_a_failed_try() {
    let start = Instant::now();
    let mut folder = LiveFolder::default();
    assert_eq!(folder.age(start), None);
    folder.answer(LiveAnswer::Failed, start);
    assert_eq!(folder.age(start).as_deref(), Some("Couldn't refresh"));
    folder.answer(LiveAnswer::Items(vec![mine(1)]), start);
    assert_eq!(folder.age(start).as_deref(), Some("Updated just now"));
    folder.answer(LiveAnswer::Failed, start + Duration::from_secs(60));
    let later = start + Duration::from_secs(12 * 60 + 30);
    assert_eq!(
        folder.age(later).as_deref(),
        Some("Couldn't refresh · Updated 12 min ago")
    );
    assert!(folder.failed());
    folder.answer(LiveAnswer::Items(vec![mine(1)]), later);
    assert_eq!(folder.age(later).as_deref(), Some("Updated just now"));
    assert!(!folder.failed());
}

#[test]
fn only_the_sites_pages_change_its_lists() {
    let source = LiveSource::PullRequests;
    assert!(source.changed_by("https://github.com/harbor/web/pull/7"));
    assert!(!source.changed_by("https://gist.github.com/x"));
    assert!(!source.changed_by("https://github.com.example/"));
    assert_eq!(mine(7).url(), "https://github.com/harbor/web/pull/7");
    assert_eq!(mine(7).place(), "harbor/web #7");
}
