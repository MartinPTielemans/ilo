//! Site notes (#4122), which a person reads as skills: what ilo knows about a site, handed to an
//! agent the first time in a task that one of its own tabs is on that site, ahead of the answer
//! that shows it the page. The code keeps `notes` until the strings move, apart from `skill.rs`,
//! which writes the SKILL.md for outside apps.
//!
//! A note is the app's own text, compiled in from `notes/`. Nothing else makes or changes one: no
//! page, agent, file or server, and nothing here reads anything at run time. A note grants
//! nothing and passes through no check, so one that names what a task's access refuses is a bug
//! in the note. It's a field of its answer, never part of the answer's text, and leaves for the
//! agent as a part of its own, ahead of the part that holds the page's words, so what a page
//! writes can't become one. The user's line and the log's words come from the field.
//!
//! A note's file starts with its record, which no agent and no person is shown: each public page
//! its facts came from (`source:`), the day they were last checked (`checked:`), what it holds
//! (`holds:`, which Settings names), and the public addresses `check/notes.sh` opens signed out
//! with the words each must show (`landmark:`). A note with no landmark has facts only an account shows, so it stays unproven until a thread is run
//! with and without it. A line `---` ends the record.
//!
//! Every string a person or an agent reads for it is here or in `notes/`, but for the sentence
//! the server's instructions say of them (`tools::INSTRUCTIONS`).

use std::collections::HashMap;

/// One site's note.
#[derive(Debug, PartialEq)]
pub struct Note {
    /// The site as a person names it, which the frame, the user's line and Settings say, and
    /// which the log and a task's given notes know it by.
    pub name: &'static str,
    /// Its hosts, without www., one of which a page's address has to be on, over https.
    pub hosts: &'static [&'static str],
    /// A domain whose every name one label under it is the site's too, where each customer has
    /// a name of its own there, such as a company's Jira. None where no subdomain is the site's.
    pub tenants: Option<&'static str>,
    /// The path the page's has to be or be under, "" for any: several of Google's apps share a
    /// host. Where two notes match, the one with the longer path is the page's.
    pub path: &'static str,
    /// Its file: the record, then what the agent is handed.
    file: &'static str,
}

/// The bytes a note's text holds at most.
pub const MAX_BYTES: usize = 1200;

/// A note's parts, in their order: a line that is one of these names the sentences under it.
pub const PARTS: [&str; 3] = [
    "Go straight there",
    "Keys beat clicks",
    "Know before you start",
];

/// A note of `name`'s on `hosts`, from `notes/<file>.txt`.
macro_rules! note {
    ($name:literal, $file:literal, [$($host:literal),*] $(, tenants $tenants:literal)? $(, path $path:literal)?) => {
        Note {
            name: $name,
            hosts: &[$($host),*],
            tenants: note!(@tenants $($tenants)?),
            path: note!(@path $($path)?),
            file: include_str!(concat!("notes/", $file, ".txt")),
        }
    };
    (@tenants) => { None };
    (@tenants $tenants:literal) => { Some($tenants) };
    (@path) => { "" };
    (@path $path:literal) => { $path };
}

/// Every note, as Settings lists them: by name, as a person looks one up.
pub static NOTES: [Note; 30] = [
    note!("Airtable", "airtable", ["airtable.com"]),
    note!(
        "Amazon",
        "amazon",
        ["amazon.com", "amazon.co.uk", "amazon.de", "amazon.se"]
    ),
    note!("Asana", "asana", ["app.asana.com"]),
    note!("borger.dk", "borger-dk", ["borger.dk"]),
    note!("ClickUp", "clickup", ["app.clickup.com"]),
    note!("Confluence", "confluence", [], tenants "atlassian.net", path "/wiki"),
    note!("Discord", "discord", ["discord.com"]),
    note!("e-Boks", "e-boks", ["e-boks.dk", "private.e-boks.com"]),
    note!("GitHub", "github", ["github.com"]),
    note!("Gmail", "gmail", ["mail.google.com"]),
    note!(
        "Google Calendar",
        "google-calendar",
        ["calendar.google.com"]
    ),
    note!("Google Docs", "google-docs", ["docs.google.com"], path "/document"),
    note!("Google Drive", "google-drive", ["drive.google.com"]),
    note!("Google Forms", "google-forms", ["docs.google.com"], path "/forms"),
    note!("Google Maps", "google-maps", ["google.com"], path "/maps"),
    note!("Google Sheets", "google-sheets", ["docs.google.com"], path "/spreadsheets"),
    note!("Google Slides", "google-slides", ["docs.google.com"], path "/presentation"),
    note!("Jira", "jira", [], tenants "atlassian.net"),
    note!("Linear", "linear", ["linear.app"]),
    note!("LinkedIn", "linkedin", ["linkedin.com"]),
    note!("Notion", "notion", ["notion.so", "notion.com"]),
    note!(
        "Outlook",
        "outlook",
        [
            "outlook.live.com",
            "outlook.office.com",
            "outlook.office365.com"
        ]
    ),
    note!("Reddit", "reddit", ["reddit.com"]),
    note!("skat.dk", "skat-dk", ["skat.dk"]),
    note!("Slack", "slack", ["app.slack.com"], tenants "slack.com"),
    note!("Stack Overflow", "stack-overflow", ["stackoverflow.com"]),
    note!("Trello", "trello", ["trello.com"]),
    note!("Wikipedia", "wikipedia", ["wikipedia.org"], tenants "wikipedia.org"),
    note!("X", "x", ["x.com"]),
    note!("YouTube", "youtube", ["youtube.com"]),
];

/// What the frame begins with, before the site's name. The model reads it, so it keeps the
/// product's name until the brand sweep.
const FRAME: &str = concat!(env!("PRODUCT_NAME"), "'s notes for ");

/// Settings › Agents' heading over the list, its switch and the sentence under it: the words
/// table's SKS1 to SKS3, a person's word for notes being skills.
pub const HEADING: &str = "Skills";
pub const SWITCH_LINE: &str = concat!(
    "A thread gets what ",
    env!("PRODUCT_NAME"),
    " knows about a site when its tab lands there."
);
pub const SWITCH_OFF_LINE: &str = "Off: threads find their way on every site by reading it.";
pub const FOOT: &str = concat!(
    "Skills are part of ",
    env!("PRODUCT_NAME"),
    ": written by us, shipped in the app, never fetched. Each is plain text you can read here."
);
/// What a note may hold, as its record's `holds:` names them and a row in Settings says them, in
/// this order: the words table's SKS6, its rule for a skill's line under its name.
pub const HOLDS: [&str; 4] = ["addresses", "search words", "keys", "where things are"];
/// A row's button, closed and open.
pub const READ: &str = "Read";
pub const CLOSE: &str = "Close";

/// The note for the page at `url`: of those whose host the address is on, over https at its own
/// port, and under whose path it is, the one with the longest path. A page outside https has
/// none, and nor has a subdomain, which may be another product, but where a note's tenants are.
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
    let path = url.path();
    (NOTES.iter())
        .filter(|note| note.on(host) && under(path, note.path))
        .max_by_key(|note| note.path.len())
}

/// Whether `path` is `prefix` or under it, a whole segment at a time: /forms isn't /formsx.
fn under(path: &str, prefix: &str) -> bool {
    match path.strip_prefix(prefix) {
        Some(rest) => prefix.is_empty() || rest.is_empty() || rest.starts_with('/'),
        None => false,
    }
}

impl Note {
    /// Whether `host`, lowercase and without www., is the note's site.
    fn on(&self, host: &str) -> bool {
        let tenant = |domain: &str| {
            (host.strip_suffix(domain))
                .and_then(|name| name.strip_suffix('.'))
                .is_some_and(|name| !name.is_empty() && !name.contains('.'))
        };
        self.hosts.contains(&host) || self.tenants.is_some_and(tenant)
    }

    /// What the agent is handed: the file after its record.
    pub fn text(&self) -> &'static str {
        (self.file.split_once("\n---\n")).map_or(self.file, |(_, text)| text)
    }

    /// The record's values for `key`, in its order.
    pub fn record(&self, key: &str) -> impl Iterator<Item = &'static str> {
        let record = self
            .file
            .split_once("\n---\n")
            .map_or("", |(record, _)| record);
        (record.lines()).filter_map(move |line| {
            let (at, value) = line.split_once(": ")?;
            (at == key).then_some(value)
        })
    }

    /// The note as the agent gets it, an answer's first part: the frame's line, then its text.
    pub fn framed(&self) -> String {
        format!("{FRAME}{}:\n{}", self.name, self.text().trim_end())
    }

    /// What it holds, under its name in Settings: the record's `holds:` in `HOLDS`' order, the
    /// first word capitalised, or where things are for a note that names none.
    pub fn about(&self) -> String {
        let held: Vec<&str> = self
            .record("holds")
            .flat_map(|holds| holds.split(", "))
            .collect();
        let named: Vec<&str> = (HOLDS.iter().copied())
            .filter(|each| held.contains(each))
            .collect();
        let about = match named.is_empty() {
            true => HOLDS[3].to_string(),
            false => named.join(", "),
        };
        let mut chars = about.chars();
        chars.next().map_or(String::new(), |first| {
            first.to_uppercase().chain(chars).collect()
        })
    }

    /// The line under a thread's step whose answer carried it, the words table's SKS4.
    pub fn with(&self) -> String {
        format!("Used the {} skill", self.name)
    }
}

/// An activity entry's title, for a call whose answer carried a note, by the words table's SKS5.
pub fn logged(title: &str) -> String {
    format!("{title}, with a skill")
}

/// The notes each task has been given, by the task's own key: kept while the task is, in memory
/// alone.
#[derive(Default)]
pub struct Given(HashMap<String, Vec<&'static str>>);

impl Given {
    /// Whether `task` hasn't had `note` yet, which it has from now on. Notes are told apart by
    /// name, as two may share a host.
    pub fn first(&mut self, task: &str, note: &Note) -> bool {
        let had = self.0.entry(task.into()).or_default();
        if had.contains(&note.name) {
            return false;
        }
        had.push(note.name);
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

    fn named(url: &str) -> Option<&'static str> {
        note_for(url).map(|note| note.name)
    }

    /// The line under a step whose answer carried a note names the site's skill.
    #[test]
    fn a_notes_line_under_its_step_names_the_site() {
        let github = note_for("https://github.com/").expect("GitHub's note");
        assert_eq!(github.with(), "Used the GitHub skill");
    }

    /// A note's line in Settings names what it holds, in the rule's order, the first word
    /// capitalised, and keys only where it has its keys part.
    #[test]
    fn a_notes_line_in_settings_names_what_it_holds() {
        let about = |url| note_for(url).unwrap().about();
        assert_eq!(
            about("https://github.com/"),
            "Addresses, search words, keys"
        );
        assert_eq!(about("https://www.youtube.com/"), "Keys");
        assert_eq!(about("https://docs.google.com/forms"), "Where things are");
        for note in &NOTES {
            for held in note.record("holds").flat_map(|holds| holds.split(", ")) {
                assert!(HOLDS.contains(&held), "{}: {held}", note.name);
            }
            let keys = note.text().lines().any(|line| line == PARTS[1]);
            assert_eq!(
                note.about().to_lowercase().contains("keys"),
                keys,
                "{}",
                note.name
            );
        }
    }

    /// A note is for its own hosts over https alone: not a subdomain, a lookalike, another port,
    /// http or a page off the web.
    #[test]
    fn a_note_is_for_its_own_host_over_https() {
        for url in [
            "https://github.com/",
            "https://www.github.com/pulls",
            "https://GitHub.com/notifications?query=is%3Aunread#top",
            "https://github.com./",
        ] {
            assert_eq!(named(url), Some("GitHub"), "{url}");
        }
        assert_eq!(named("https://mail.google.com/mail/u/0/"), Some("Gmail"));
        assert_eq!(named("https://www.youtube.com/watch?v=x"), Some("YouTube"));
        assert_eq!(
            named("https://www.amazon.de/gp/cart/view.html"),
            Some("Amazon")
        );
        assert_eq!(named("https://www.notion.com/help"), Some("Notion"));
        assert_eq!(named("https://notion.so/a-page-0123"), Some("Notion"));
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
            "https://amazon.example/",
            "https://smile.amazon.com.evil.example/",
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

    /// Of the notes on a host, a page has the one with the longest path it's under, a whole
    /// segment at a time, and none where it's under none of them.
    #[test]
    fn the_longest_path_a_page_is_under_wins() {
        for (url, name) in [
            (
                "https://docs.google.com/document/d/1abc/edit",
                "Google Docs",
            ),
            ("https://docs.google.com/document/u/0/", "Google Docs"),
            (
                "https://docs.google.com/spreadsheets/d/1abc/edit#gid=0",
                "Google Sheets",
            ),
            (
                "https://docs.google.com/presentation/d/1abc/edit",
                "Google Slides",
            ),
            ("https://docs.google.com/forms/d/1abc/edit", "Google Forms"),
            ("https://docs.google.com/forms", "Google Forms"),
            (
                "https://www.google.com/maps/search/?api=1&query=x",
                "Google Maps",
            ),
            (
                "https://acme.atlassian.net/wiki/spaces/ENG/overview",
                "Confluence",
            ),
            ("https://acme.atlassian.net/wiki", "Confluence"),
            ("https://acme.atlassian.net/browse/ENG-12", "Jira"),
            ("https://acme.atlassian.net/wikipedia", "Jira"),
            ("https://acme.atlassian.net/", "Jira"),
        ] {
            assert_eq!(named(url), Some(name), "{url}");
        }
        for url in [
            "https://docs.google.com/",
            "https://docs.google.com/formsx/d/1abc",
            "https://docs.google.com/documents",
            "https://www.google.com/search?q=maps",
            "https://www.google.com/mapsearch",
        ] {
            assert_eq!(note_for(url), None, "{url}");
        }
    }

    /// A name one label under a note's tenants' domain is the note's site, and only for a note
    /// that has tenants: not the domain alone, two labels under it or a lookalike of it.
    #[test]
    fn a_tenant_is_the_site_only_where_its_note_says() {
        for (url, name) in [
            ("https://acme.atlassian.net/jira/your-work", "Jira"),
            ("https://en.wikipedia.org/wiki/Main_Page", "Wikipedia"),
            ("https://da.wikipedia.org/wiki/Forside", "Wikipedia"),
            ("https://acme.slack.com/archives/C1", "Slack"),
            ("https://app.slack.com/client/T1/C1", "Slack"),
        ] {
            assert_eq!(named(url), Some(name), "{url}");
        }
        for url in [
            "https://atlassian.net/",
            "https://a.b.atlassian.net/browse/X-1",
            "https://evilatlassian.net/",
            "https://acme.atlassian.net.evil.example/",
            "https://slack.com.evil.example/",
            "https://evil-slack.com/",
            "https://en.m.wikipedia.org/wiki/Main_Page",
            "https://gist.github.com/",
            "https://x.mail.google.com/",
            "https://api.linear.app/",
            "https://evil.trello.com/",
        ] {
            assert_eq!(note_for(url), None, "{url}");
        }
        let tenanted = NOTES.iter().filter(|note| note.tenants.is_some());
        let names: Vec<&str> = tenanted.map(|note| note.name).collect();
        assert_eq!(names, ["Confluence", "Jira", "Slack", "Wikipedia"]);
    }

    /// Every note is under the size limit, plain text in the three parts' order, and names no
    /// address outside its own site: its addresses start at the site's root. Its record names
    /// the public pages its facts came from, the day they were checked, and landmarks on its
    /// own site, none of which reaches the agent.
    #[test]
    fn every_note_is_short_and_stays_on_its_site() {
        for note in &NOTES {
            let (name, text) = (note.name, note.text());
            assert!(!text.trim().is_empty(), "{name}");
            assert!(text.len() <= MAX_BYTES, "{name}: {}", text.len());
            assert!(note.framed().starts_with(&format!("{FRAME}{name}:\n")));
            // No address with a scheme or a host of its own, and no markup a client would draw.
            for outside in [
                "://", "www.", "http", ".com", ".org", ".io", ".app", ".net", ".dk", ".so", "</",
                "](", "`",
            ] {
                assert!(!text.contains(outside), "{name} has {outside}");
            }
            // The frame's own words, which only the app writes, and what no note is about.
            let lower = text.to_lowercase();
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
                "log på",
                "mitid",
                "2fa",
                " api",
                "refus",
                "allow",
                "permission",
            ] {
                assert!(!lower.contains(never), "{name} says {never}");
            }
            // None of the record reaches the agent.
            for line in text.lines() {
                let record = ["source: ", "checked: ", "holds: ", "landmark: ", "---"];
                assert!(
                    !record.iter().any(|key| line.starts_with(key)),
                    "{name}: {line}"
                );
            }
            let heads: Vec<usize> = (PARTS.iter())
                .filter_map(|part| text.lines().position(|line| line == *part))
                .collect();
            assert!(!heads.is_empty(), "{name} has no part");
            assert!(heads.is_sorted(), "{name}'s parts are out of order");
            let named = |line: &str| PARTS.contains(&line);
            assert!(text.lines().next().is_some_and(named), "{name}");

            let sources: Vec<&str> = note.record("source").collect();
            assert!(!sources.is_empty(), "{name} names no source");
            for source in sources {
                assert!(source.starts_with("https://"), "{name}: {source}");
            }
            let checked: Vec<&str> = note.record("checked").collect();
            assert_eq!(checked.len(), 1, "{name}");
            let day = checked[0].as_bytes();
            assert!(
                day.len() == 10 && day[4] == b'-' && day[7] == b'-',
                "{name}: {}",
                checked[0]
            );
            for landmark in note.record("landmark") {
                let (url, words) = landmark.split_once(" | ").expect(landmark);
                assert!(!words.trim().is_empty(), "{name}: {landmark}");
                assert_eq!(
                    note_for(url).map(|found| found.name),
                    Some(name),
                    "{landmark}"
                );
            }
        }
        // One note a name and a host as `note_for` looks it up, unless their paths differ.
        for (at, note) in NOTES.iter().enumerate() {
            assert!(NOTES[..at].iter().all(|each| each.name != note.name));
            for host in note.hosts {
                let found = note_for(&format!("https://{host}{}", note.path));
                assert_eq!(found.map(|found| found.name), Some(note.name));
            }
        }
        assert!(NOTES.is_sorted_by_key(|note| note.name.to_lowercase()));
    }

    /// A task has a note once, each task its own, for as long as the task is there, and two
    /// notes on one host are two.
    #[test]
    fn a_task_is_given_a_note_once() {
        let found = |url| note_for(url).unwrap();
        let (github, gmail) = (
            found("https://github.com/"),
            found("https://mail.google.com/"),
        );
        let docs = found("https://docs.google.com/document/d/1");
        let sheets = found("https://docs.google.com/spreadsheets/d/1");
        let mut given = Given::default();
        assert!(given.first("session-1", github));
        assert!(!given.first("session-1", github));
        assert!(given.first("session-1", gmail));
        assert!(given.first("session-1", docs));
        assert!(given.first("session-1", sheets));
        assert!(!given.first("session-1", docs));
        assert!(given.first("session-2", github));
        given.keep(|task| task == "session-2");
        assert!(given.first("session-1", github));
        assert!(!given.first("session-2", github));
    }
}
