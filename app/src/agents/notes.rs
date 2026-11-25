//! Site notes (#4122): what ilo knows about a site, handed to an agent the first time in a task
//! that one of its own tabs is on that site, ahead of the answer that shows it the page.
//!
//! A note is the app's own text, compiled in from `notes/`. Nothing else makes or changes one: no
//! page, agent, file or server, and nothing here reads anything at run time. A note grants
//! nothing and passes through no check, so one that names what a task's access refuses is a bug
//! in the note. It's a field of its answer, never part of the answer's text, and leaves for the
//! agent as a part of its own, ahead of the part that holds the page's words, so what a page
//! writes can't become one. The user's line and the log's words come from the field.
//!
//! Every string a person or an agent reads for it is here or in `notes/`, but for the sentence
//! the server's instructions say of them (`tools::INSTRUCTIONS`).

use std::collections::HashMap;

/// One site's note.
#[derive(Debug, PartialEq)]
pub struct Note {
    /// The site as a person names it, which the frame, the user's line and Settings say.
    pub name: &'static str,
    /// Its host, without www., which a page's address has to be on, over https.
    pub host: &'static str,
    /// What it holds, in a few words, under its name in Settings.
    pub about: &'static str,
    /// What the agent is handed.
    pub text: &'static str,
}

/// The bytes a note holds at most.
pub const MAX_BYTES: usize = 1200;

/// A note's parts, in their order: a line that is one of these names the sentences under it.
pub const PARTS: [&str; 3] = [
    "Go straight there",
    "Keys beat clicks",
    "Know before you start",
];

/// Every note, as Settings lists them.
pub static NOTES: [Note; 3] = [
    Note {
        name: "GitHub",
        host: "github.com",
        about: "Addresses, search words, keys",
        text: include_str!("notes/github.txt"),
    },
    Note {
        name: "Gmail",
        host: "mail.google.com",
        about: "Search words, keys",
        text: include_str!("notes/gmail.txt"),
    },
    Note {
        name: "YouTube",
        host: "youtube.com",
        about: "The player's keys",
        text: include_str!("notes/youtube.txt"),
    },
];

/// What the frame begins with, before the site's name.
const FRAME: &str = concat!(env!("APP_NAME"), "'s notes for ");

/// Settings › Agents' heading over the list, its switch and the sentence under it.
pub const HEADING: &str = "Site notes";
pub const SWITCH_LINE: &str = concat!(
    "A thread gets what ",
    env!("PRODUCT_NAME"),
    " knows about a site when its tab lands there."
);
pub const SWITCH_OFF_LINE: &str = "Off: threads find their way on every site by reading it.";
pub const FOOT: &str = concat!(
    "Notes are part of ",
    env!("PRODUCT_NAME"),
    ": written by us, shipped in the app, never fetched. Each is plain text you can read here."
);
/// A row's button, closed and open.
pub const READ: &str = "Read";
pub const CLOSE: &str = "Close";

/// The note for the page at `url`: the one whose host the address is on, over https at its own
/// port. A page outside https has none, and nor has a subdomain, which may be another product.
pub fn note_for(url: &str) -> Option<&'static Note> {
    let url = url::Url::parse(url).ok()?;
    if url.scheme() != "https" || url.port().is_some() {
        return None;
    }
    let Some(url::Host::Domain(host)) = url.host() else {
        return None;
    };
    let host = host.trim_end_matches('.');
    let host = host.strip_prefix("www.").unwrap_or(host);
    NOTES.iter().find(|note| note.host == host)
}

impl Note {
    /// The note as the agent gets it, an answer's first part: the frame's line, then its text.
    pub fn framed(&self) -> String {
        format!("{FRAME}{}:\n{}", self.name, self.text.trim_end())
    }

    /// The line under a thread's step whose answer carried it, the words table's D5, the product
    /// by its name.
    pub fn with(&self) -> String {
        format!(
            "Included {}\u{2019}s notes on {}",
            env!("PRODUCT_NAME"),
            self.name
        )
    }
}

/// An activity entry's title, for a call whose answer carried a note.
pub fn logged(title: &str) -> String {
    format!("{title}, with notes")
}

/// The notes each task has been given, by the task's own key: kept while the task is, in memory
/// alone.
#[derive(Default)]
pub struct Given(HashMap<String, Vec<&'static str>>);

impl Given {
    /// Whether `task` hasn't had `note` yet, which it has from now on.
    pub fn first(&mut self, task: &str, note: &Note) -> bool {
        let had = self.0.entry(task.into()).or_default();
        if had.contains(&note.host) {
            return false;
        }
        had.push(note.host);
        true
    }

    /// Keeps the sets of the tasks that `live` says are still there.
    pub fn keep(&mut self, live: impl Fn(&str) -> bool) {
        self.0.retain(|task, _| live(task));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The line under a step whose answer carried a note names the product and the site.
    #[test]
    fn a_notes_line_under_its_step_names_the_product_and_the_site() {
        let github = note_for("https://github.com/").expect("GitHub's note");
        assert_eq!(
            github.with(),
            format!("Included {}\u{2019}s notes on GitHub", env!("PRODUCT_NAME"))
        );
    }

    /// A note is for its own host over https alone: not a subdomain, a lookalike, another port,
    /// http or a page off the web.
    #[test]
    fn a_note_is_for_its_own_host_over_https() {
        for url in [
            "https://github.com/",
            "https://www.github.com/pulls",
            "https://GitHub.com/notifications?query=is%3Aunread#top",
            "https://github.com./",
        ] {
            assert_eq!(note_for(url).map(|note| note.name), Some("GitHub"), "{url}");
        }
        assert_eq!(
            note_for("https://mail.google.com/mail/u/0/").map(|note| note.name),
            Some("Gmail")
        );
        assert_eq!(
            note_for("https://www.youtube.com/watch?v=x").map(|note| note.name),
            Some("YouTube")
        );
        for url in [
            "http://github.com/",
            "https://github.com:8443/",
            "https://gist.github.com/",
            "https://docs.github.com/",
            "https://github.com.evil.example/",
            "https://notgithub.com/",
            "https://evil.example/github.com",
            "https://github.com@evil.example/",
            "https://google.com/",
            "https://accounts.google.com/",
            "https://m.youtube.com/",
            "file:///github.com/",
            "chrome://settings/",
            "about:blank",
            "view-source:https://github.com/",
            "data:text/html,https://github.com/",
            "github.com",
            "",
        ] {
            assert_eq!(note_for(url), None, "{url}");
        }
    }

    /// Every note is under the size limit, plain text in the three parts' order, and names no
    /// address outside its own site: its addresses start at the site's root.
    #[test]
    fn every_note_is_short_and_stays_on_its_site() {
        for note in &NOTES {
            let name = note.name;
            assert!(!note.text.trim().is_empty(), "{name}");
            assert!(note.text.len() <= MAX_BYTES, "{name}: {}", note.text.len());
            assert!(note.framed().starts_with(&format!("{FRAME}{name}:\n")));
            // No address with a scheme or a host of its own, and no markup a client would draw.
            for outside in [
                "://", "www.", "http", ".com", ".org", ".io", ".app", "</", "](", "`",
            ] {
                assert!(!note.text.contains(outside), "{name} has {outside}");
            }
            // The frame's own words, which only the app writes, and what no note is about.
            let lower = note.text.to_lowercase();
            for never in [
                "notes for",
                "password",
                "passkey",
                "token",
                "cookie",
                "captcha",
                "sign in",
                "sign-in",
                "log in",
                "login",
                "2fa",
                " api",
                "refus",
                "allow",
                "permission",
            ] {
                assert!(!lower.contains(never), "{name} says {never}");
            }
            let heads: Vec<usize> = (PARTS.iter())
                .filter_map(|part| note.text.lines().position(|line| line == *part))
                .collect();
            assert!(!heads.is_empty(), "{name} has no part");
            assert!(heads.is_sorted(), "{name}'s parts are out of order");
            // A line is a part's name or a sentence of it, and never empty of both.
            let named = |line: &str| PARTS.contains(&line);
            assert!(note.text.lines().next().is_some_and(named), "{name}");
        }
        // One note a host, and a host as `note_for` looks it up.
        for (at, note) in NOTES.iter().enumerate() {
            assert!(NOTES[..at].iter().all(|each| each.host != note.host));
            let found = note_for(&format!("https://{}/", note.host));
            assert_eq!(found.map(|found| found.name), Some(note.name));
        }
    }

    /// A task has a note once, each task its own, for as long as the task is there.
    #[test]
    fn a_task_is_given_a_note_once() {
        let (github, gmail) = (&NOTES[0], &NOTES[1]);
        let mut given = Given::default();
        assert!(given.first("session-1", github));
        assert!(!given.first("session-1", github));
        assert!(given.first("session-1", gmail));
        assert!(given.first("session-2", github));
        given.keep(|task| task == "session-2");
        assert!(given.first("session-1", github));
        assert!(!given.first("session-2", github));
    }
}
