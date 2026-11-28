//! Recent's made-up answers.

use crate::*;

pub(crate) fn recent_answers(state: &str) -> Vec<browser::recent::Answer> {
    use browser::recent::{Answer, List, Row};
    let row = |list, repo: &str, number, title: &str, draft| Row {
        list,
        repo: repo.to_string().into(),
        number,
        title: title.to_string().into(),
        draft,
    };
    let rows = vec![
        row(
            List::Review,
            "acme/web",
            482,
            "Cache the search index per branch",
            false,
        ),
        row(
            List::Review,
            "acme/api",
            1093,
            "Retry uploads after a dropped connection",
            false,
        ),
        row(
            List::Created,
            "acme/web",
            488,
            "Draw the sidebar's rows from one list",
            state == "draft",
        ),
    ];
    let answered = now() - 25. * 60. * 1000.;
    match state {
        "rows" | "draft" => vec![Answer::Rows { rows, answered }],
        "none" => vec![Answer::Rows {
            rows: Vec::new(),
            answered,
        }],
        "signedout" => vec![Answer::SignedOut],
        "failed" => vec![Answer::Failed],
        "stale" => vec![Answer::Rows { rows, answered }, Answer::Failed],
        _ => Vec::new(),
    }
}
