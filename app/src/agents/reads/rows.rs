//! A saved read as Settings › Agents lists it (#3822): its name, its site and its state on the row,
//! the line its state needs under them, and what the row shows opened. Only the definition and its
//! state are kept, never a row a read gave, so nothing here holds a site's text but its host.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::saved_reads::definition::Kind;
use crate::saved_reads::{Saved, State};

/// Settings' heading over the saved reads, and its group's foot.
pub const HEADING: &str = "Saved reads";
pub const FOOT: &str = concat!(
    "Agents ask for one while they work, and you save it from their card. Only agents connected \
     to ",
    env!("APP_NAME"),
    " can use them."
);
/// The opened row's button, which removes the read with no dialog: ⌘Z brings it back.
pub const REMOVE: &str = "Remove Read";

/// The ink a state reads in: the detail's, the warning token's, the destructive one's, or the
/// quiet one's of a read the user's list stopped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ink {
    Ready,
    Warning,
    Broken,
    Paused,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    pub site: String,
    /// "Read 4 min ago", "Needs a sign-in", "Broken since Nov 19" or "Paused", in `ink`.
    pub state: String,
    pub ink: Ink,
    /// What the user can do about a state that stops the read, under the row's line.
    pub says: Option<String>,
    /// What the row shows opened, each a label and its value.
    pub facts: Vec<(&'static str, String)>,
    /// The opened row's foot, beside Remove Read: "Saved Nov 14 · Asked for by Claude Code".
    pub foot: String,
}

/// `saved` as its row shows it at `now`: `fenced` if its site is on the user's Always ask first
/// list, which pauses it whatever it last came to.
pub fn row(saved: &Saved, fenced: bool, now: SystemTime) -> Row {
    let read = &saved.read;
    let site = super::shown(&read.site);
    let at = |seconds: u64| UNIX_EPOCH + Duration::from_secs(seconds);
    let (state, ink, says) = match (&saved.state, fenced) {
        (_, true) => (
            "Paused".to_string(),
            Ink::Paused,
            Some(format!(
                "{site} is on Always ask first. Remove it from that list to resume this read."
            )),
        ),
        (State::Ready, false) => (read_ago(at(saved.last_read), now), Ink::Ready, None),
        (State::NeedsSignIn, false) => (
            "Needs a sign-in".to_string(),
            Ink::Warning,
            Some(format!(
                "Sign in to {site} in any tab so agents can read it again."
            )),
        ),
        (State::Broken(why), false) => (
            format!("Broken since {}", day(at(saved.since), now)),
            Ink::Broken,
            Some(format!("{why} Ask an agent to fix it, or remove it.")),
        ),
    };
    let address = format!("{site}{}", read.url);
    let columns: Vec<&str> = read
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect();
    let kind = match read.kind {
        Kind::Data => "The site's own data",
        Kind::Table => "A table on the page",
    };
    let mut facts = vec![("Reads", address), ("Kind", kind.to_string())];
    if !read.params.is_empty() {
        let names: Vec<&str> = read.params.keys().map(String::as_str).collect();
        facts.push(("Fills in", names.join(", ")));
    }
    facts.push(("Columns", columns.join(", ")));
    facts.push(("Last read", read_ago(at(saved.last_read), now)));
    Row {
        name: read.name.clone(),
        site,
        state,
        ink,
        says,
        facts,
        foot: format!(
            "Saved {} · Asked for by {}",
            day(at(saved.saved), now),
            saved.asked_by
        ),
    }
}

/// "Read 4 min ago", or "Read just now".
fn read_ago(at: SystemTime, now: SystemTime) -> String {
    match crate::agents::ago(at, now).as_str() {
        "Just now" => "Read just now".to_string(),
        ago => format!("Read {ago}"),
    }
}

/// The day `at` falls on in the user's calendar, as "Nov 19", with its year if it isn't `now`'s.
fn day(at: SystemTime, now: SystemTime) -> String {
    let local = |time: SystemTime| chrono::DateTime::<chrono::Local>::from(time);
    let (at, now) = (local(at), local(now));
    match chrono::Datelike::year(&at) == chrono::Datelike::year(&now) {
        true => at.format("%b %-d").to_string(),
        false => at.format("%b %-d, %Y").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const DAY: u64 = 86_400;
    /// 2026-11-21, noon UTC.
    const NOW: u64 = 1_795_262_400;

    fn saved(state: State, params: bool) -> Saved {
        let mut arguments = json!({
            "name": "city buildings", "kind": "table", "url": "/reads/table",
            "columns": [
                { "name": "Name", "type": "text", "from": "Name" },
                { "name": "City", "type": "text", "from": "City" },
            ],
        });
        if params {
            arguments["url"] = json!("/reads/items/{id}.json");
            arguments["kind"] = json!("json");
            arguments["params"] = json!({ "id": { "type": "text", "pattern": "A-[0-9]{1,6}" } });
            arguments["columns"] = json!([{ "name": "Name", "type": "text", "from": "name" }]);
        }
        Saved {
            read: super::super::definition(&arguments, "https://reads.test").expect("not a read"),
            saved: NOW - 7 * DAY,
            asked_by: "Claude Code".into(),
            last_read: NOW - 240,
            state,
            since: NOW - 2 * DAY,
        }
    }

    fn now() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(NOW)
    }

    /// Lib test 6: each state draws its line in its ink, with what the user can do about one that
    /// stops the read; the user's list pauses a read whatever it came to.
    #[test]
    fn each_state_says_itself_and_what_to_do() {
        let ready = row(&saved(State::Ready, false), false, now());
        assert_eq!(
            (ready.state.as_str(), ready.ink, ready.says),
            ("Read 4 min ago", Ink::Ready, None)
        );
        let sign_in = row(&saved(State::NeedsSignIn, false), false, now());
        assert_eq!(
            (sign_in.state.as_str(), sign_in.ink, sign_in.says.as_deref()),
            (
                "Needs a sign-in",
                Ink::Warning,
                Some("Sign in to reads.test in any tab so agents can read it again.")
            )
        );
        let why = "The page changed: no table has these columns.";
        let broken = row(&saved(State::Broken(why.into()), false), false, now());
        assert!(
            broken.state.starts_with("Broken since Nov 1"),
            "{}",
            broken.state
        );
        assert_eq!(broken.ink, Ink::Broken);
        assert_eq!(
            broken.says.as_deref(),
            Some(
                "The page changed: no table has these columns. Ask an agent to fix it, or remove \
                 it."
            )
        );
        let paused = row(&saved(State::NeedsSignIn, false), true, now());
        assert_eq!(
            (paused.state.as_str(), paused.ink, paused.says.as_deref()),
            (
                "Paused",
                Ink::Paused,
                Some(
                    "reads.test is on Always ask first. Remove it from that list to resume this \
                     read."
                )
            )
        );
        let mut just = saved(State::Ready, false);
        just.last_read = NOW;
        assert_eq!(row(&just, false, now()).state, "Read just now");
    }

    /// Lib test 6: opened, a row says what it reads, its kind, what it fills in if anything, its
    /// columns and when it was last read, over when it was saved and who asked for it.
    #[test]
    fn an_opened_row_says_what_the_read_is() {
        let table = row(&saved(State::Ready, false), false, now());
        assert_eq!(
            table.facts,
            [
                ("Reads", "reads.test/reads/table".to_string()),
                ("Kind", "A table on the page".to_string()),
                ("Columns", "Name, City".to_string()),
                ("Last read", "Read 4 min ago".to_string()),
            ]
        );
        assert!(table.foot.starts_with("Saved Nov 1"), "{}", table.foot);
        assert!(
            table.foot.ends_with(" · Asked for by Claude Code"),
            "{}",
            table.foot
        );
        let data = row(&saved(State::Ready, true), false, now());
        assert_eq!(data.facts[1], ("Kind", "The site's own data".to_string()));
        assert_eq!(data.facts[2], ("Fills in", "id".to_string()));
    }
}
