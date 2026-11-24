//! A live folder's made-up answer and tabs.

use crate::*;

/// What the preview's Live Folder fetches, as PREVIEW_LIVE_FOLDER asks: made-up pull requests,
/// the mock's (#2979), or with `signed-out` that answer. With `failed`, the next fetch fails.
pub(crate) fn live_stand_in() -> LiveAnswer {
    if std::env::var("PREVIEW_LIVE_FOLDER").as_deref() == Ok("signed-out") {
        return LiveAnswer::SignedOut;
    }
    let item = |list, repo: &str, number, title: &str, state| LiveItem {
        list,
        repo: repo.to_string().into(),
        number,
        title: title.to_string().into(),
        state,
    };
    use LiveList::{Created, ReviewRequests};
    use PullState::{Draft, Open};
    LiveAnswer::Items(vec![
        item(
            ReviewRequests,
            "quill/editor",
            412,
            "Keep ⌘L in the split's own field",
            Open,
        ),
        item(
            ReviewRequests,
            "harbor/web",
            88,
            "Cache favicons per profile",
            Open,
        ),
        item(
            Created,
            "lantern/browser",
            2961,
            "Recent tabs switcher on a held ⌃Tab",
            Open,
        ),
        item(
            Created,
            "lantern/browser",
            2990,
            "Idle check for Live Folders",
            Draft,
        ),
        item(
            Created,
            "harbor/timetables",
            57,
            "Parse the ferry timetable's winter page",
            Open,
        ),
    ])
}

/// The tabs PREVIEW_LIVE_FOLDER=states opens in the Live Folder, at their items' URLs: #2961,
/// selected, and #57, merged since, so it has left the lists and its tab is a plain one.
pub(crate) const LIVE_TABS: [(u64, &str, &str); 2] = [
    (
        9001,
        "Recent tabs switcher on a held ⌃Tab · lantern/browser#2961",
        "https://github.com/lantern/browser/pull/2961",
    ),
    (
        9002,
        "Parse the ferry timetable's winter page · harbor/timetables#57",
        "https://github.com/harbor/timetables/pull/57",
    ),
];

/// The preview's Live Folder's id, past every tab and folder it makes up.
pub(crate) const LIVE_FOLDER: u64 = 9000;
