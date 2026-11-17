//! What's recent on a site, in its pinned tab's or favorite's hover card (#3074): today the open
//! pull requests of the user signed in to GitHub, review requests first, in the picture's place.
//!
//! The core asks the site, on the tab's profile's own session, at most once each two minutes per
//! profile, and answers parsed rows; what the app keeps of them is here, per profile and in memory
//! alone, so a card shows them at once. It goes when ilo quits, on a signed-out answer, and when
//! either switch turns it off. No agent path reaches any of it.

use std::collections::{HashMap, HashSet};

use gpui::{App, Global, SharedString};

/// Which of the site's lists a row is from, in the card's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum List {
    /// The pull requests the user is asked to review.
    Review,
    /// The user's own.
    Created,
}

impl List {
    pub fn heading(self) -> &'static str {
        match self {
            List::Review => "Review requests",
            List::Created => "Created by me",
        }
    }
}

/// A pull request, as the core parsed it: plain text, never markup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub list: List,
    /// "owner/name".
    pub repo: SharedString,
    pub number: u64,
    pub title: SharedString,
    pub draft: bool,
}

impl Row {
    /// Its page, made here from its parts, which the core checked are a repo's and a number.
    pub fn url(&self) -> String {
        format!("https://github.com/{}/pull/{}", self.repo, self.number)
    }

    /// Its line under the title.
    pub fn detail(&self) -> String {
        format!("{} #{}", self.repo, self.number)
    }

    fn key(&self) -> (SharedString, u64) {
        (self.repo.clone(), self.number)
    }
}

/// The core's answer to an ask.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    /// The site's latest, perhaps from inside the two minutes, and when it gave them, in
    /// milliseconds since the Unix epoch.
    Rows { rows: Vec<Row>, answered: f64 },
    /// It answered that no one is signed in.
    SignedOut,
    /// It couldn't be asked, or answered what isn't a list: offline, a redirect, too big, another
    /// shape.
    Failed,
}

/// Whether this build's core can ask a site, which the preview, without one, answers for itself.
pub const REACHED: bool = true;

/// How many rows the card shows under the lists' headings, in the picture's box at its tallest with
/// the stale line under the third.
pub const SHOWN: usize = 3;

/// What a card shows in its picture's place.
#[derive(Clone, Debug, PartialEq)]
pub enum Recent {
    /// Asked, with nothing kept yet: the rows' places.
    Loading,
    /// Up to SHOWN rows, review requests first, each with whether it's new since the last answer
    /// and not yet opened, none if the user has none open; and, where the last ask failed, how old
    /// they are.
    Rows {
        rows: Vec<(Row, bool)>,
        stale: Option<SharedString>,
    },
    /// The one thing to do.
    SignedOut,
    /// The last ask failed with nothing kept.
    Unreachable,
}

/// The sites that have rows, by their origin: GitHub alone so far.
pub fn source(url: &str) -> Option<&'static str> {
    let rest = url.strip_prefix("https://github.com")?;
    (rest.is_empty() || rest.starts_with(['/', '?', '#'])).then_some("github.com")
}

/// What's kept of a profile's last answers.
#[derive(Clone, Debug, Default, PartialEq)]
struct Kept {
    rows: Vec<Row>,
    answered: f64,
    signed_out: bool,
    /// The last ask failed, which the rows' age then says.
    failed: bool,
    /// Rows that came since the answer before, until opened.
    new: HashSet<(SharedString, u64)>,
}

/// The answers kept, by profile, and the two switches: Settings › General's, and Site Controls'
/// for each site turned off. The switches are this device's, read at launch, never synced.
#[derive(Default)]
pub struct Recents {
    kept: HashMap<u64, Kept>,
    pub on: bool,
    pub sites_off: HashSet<SharedString>,
}

impl Global for Recents {}

/// Whether a card for a page on `site` shows its rows: both switches on.
pub fn shows(site: &str, cx: &App) -> bool {
    cx.try_global::<Recents>()
        .is_some_and(|recents| recents.on && !recents.sites_off.contains(site))
}

/// Settings › General's switch.
pub fn on(cx: &App) -> bool {
    cx.try_global::<Recents>().is_some_and(|recents| recents.on)
}

/// The switches as the launch read them.
pub fn set_switches(on: bool, sites_off: HashSet<SharedString>, cx: &mut App) {
    let recents = cx.default_global::<Recents>();
    recents.on = on;
    recents.sites_off = sites_off;
}

/// Settings › General's switch. Off, every profile's rows go.
pub fn set_on(on: bool, cx: &mut App) {
    let recents = cx.default_global::<Recents>();
    recents.on = on;
    if !on {
        recents.kept.clear();
    }
}

/// Site Controls' switch for `site`. Off, its rows go, which is GitHub's alone so far.
pub fn set_site_on(site: &str, on: bool, cx: &mut App) {
    let recents = cx.default_global::<Recents>();
    match on {
        true => recents.sites_off.remove(site),
        false => {
            recents.kept.clear();
            recents.sites_off.insert(site.to_string().into())
        }
    };
}

/// Keeps `answer` for `profile`.
pub fn keep(profile: u64, answer: Answer, cx: &mut App) {
    let recents = cx.default_global::<Recents>();
    if !recents.on {
        return;
    }
    match answer {
        Answer::Rows { mut rows, answered } => {
            rows.sort_by_key(|row| row.list);
            let before = recents.kept.remove(&profile);
            let new = match &before {
                // The first answer is all there is, so nothing in it is new.
                None => HashSet::new(),
                Some(before) => {
                    let had: HashSet<_> = before.rows.iter().map(Row::key).collect();
                    let now: HashSet<_> = rows.iter().map(Row::key).collect();
                    (before.new.iter().filter(|key| now.contains(*key)).cloned())
                        .chain(now.iter().filter(|key| !had.contains(*key)).cloned())
                        .collect()
                }
            };
            let kept = Kept {
                rows,
                answered,
                signed_out: false,
                failed: false,
                new,
            };
            recents.kept.insert(profile, kept);
        }
        Answer::SignedOut => {
            recents.kept.insert(
                profile,
                Kept {
                    signed_out: true,
                    ..Kept::default()
                },
            );
        }
        // The rows stay, said to be as old as they are.
        Answer::Failed => recents.kept.entry(profile).or_default().failed = true,
    }
}

/// `row` was opened from a card, so it's no longer new.
pub fn opened(profile: u64, row: &Row, cx: &mut App) {
    if let Some(kept) = cx.default_global::<Recents>().kept.get_mut(&profile) {
        kept.new.remove(&row.key());
    }
}

/// What a card of `profile`'s shows, with `age` saying how long ago a time in milliseconds since
/// the Unix epoch was.
pub fn shown(profile: u64, age: impl Fn(f64) -> String, cx: &App) -> Recent {
    let Some(kept) = cx
        .try_global::<Recents>()
        .and_then(|r| r.kept.get(&profile))
    else {
        return Recent::Loading;
    };
    if kept.signed_out {
        return Recent::SignedOut;
    }
    if kept.rows.is_empty() && kept.failed {
        return Recent::Unreachable;
    }
    let rows = (kept.rows.iter().take(SHOWN))
        .map(|row| (row.clone(), kept.new.contains(&row.key())))
        .collect();
    let stale = kept
        .failed
        .then(|| format!("Couldn't refresh · Updated {}", age(kept.answered)).into());
    Recent::Rows { rows, stale }
}

#[cfg(test)]
mod tests;
