//! Sites the user puts off limits (#3724): on one of these, and on its subdomains, an agent can't
//! open a page, use one it came to any other way, or click, type or send anything, whatever its
//! task's access: no tier lifts a fence, and the way on is the user's taking the site off. The list
//! is the user's alone: no agent tool lists, reads or changes it.
//!
//! Every string a person reads for it is here, so a wording pass changes this file alone.

use std::net::Ipv4Addr;

/// Settings › Agents' heading over the list.
pub const HEADING: &str = "Off limits";
/// The one row while the list is empty, and its line.
pub const NONE: &str = "No sites";
pub const NONE_LINE: &str = "Add a site agents can't open or use, such as your bank.";
/// The button under the list that opens the field, and the field's own words.
pub const ADD_SITE: &str = "Add Site…";
pub const PLACEHOLDER: &str = "Site or address";
pub const SPOKEN: &str = "Site agents can't open or use";
/// The sentence under the list.
pub const FOOT: &str = "Agents can't open or use these sites, whatever a task's access.";

/// What an agent's call on a tab it came to without an open asks: the tab is there already, so
/// it asks to use the site where an open asks to open it, with the same answers.
pub fn arrival(site: &str) -> String {
    format!("use {site}")
}

/// What the request tool asks on a fenced site, where its reads ask too.
pub fn requests(count: usize, site: &str) -> String {
    match count {
        1 => format!("send a request to {site}"),
        count => format!("send {count} requests to {site}"),
    }
}

/// What an answer says in place of the page, once the tab came to a fenced site by itself.
pub fn withheld(tab: u64, site: &str) -> String {
    format!(
        "Tab {tab} is now on {site}, which is off limits to agents, so nothing of its page is \
         shown and calls on it are refused."
    )
}

/// What a call on such a tab answers while its page shows a dialog: the call did nothing.
pub fn refused(tab: u64, site: &str) -> String {
    format!("Nothing was done. {}", withheld(tab, site))
}

/// The activity log's words for that refusal, which the user reads: by the entry of their list
/// the site matched, so a subdomain's names what they wrote.
pub fn logged(entry: &str) -> String {
    format!("{entry} is on Off limits.")
}

/// The title list_tabs gives such a tab, with no address.
pub const LISTED: &str = "Off limits";

/// Why close_tab leaves such a tab open, which says neither that its page shows a dialog nor that
/// it asks before it's left.
pub const STAYS_OPEN: &str = "its page may have unsaved changes";

/// What a download from a fenced site answers the agent at once, while the user is asked: neither
/// the file's name nor how it ends.
pub fn download(site: &str) -> String {
    format!(
        "A download from {site}, which is off limits to agents, is refused: its name isn't \
         shown."
    )
}

/// A run's answer when its step's tab came to a fenced site.
pub fn run_stopped(step: usize, tab: u64, site: &str) -> String {
    format!(
        "Step {step} is on {site}, which is off limits to agents, so the run stopped there. \
         Carry on in another tab, without tab {tab}."
    )
}

/// The activity log's words for the question, and for the answer at a page the agent came to:
/// each as it's doing it, done, and refused.
pub const ASKED: (&str, &str, &str) = (
    "Asking you first on",
    "Asked you first on",
    "Couldn't ask you first on",
);
pub const CAME: (&str, &str, &str) = ("Asking to use", "Allowed to use", "Not allowed to use");
/// The same, as the agent bar says what it did last.
pub const ASKED_DONE: &str = "Asked you first";
pub const CAME_DONE: &str = "Allowed to use a page";

/// The log's actions for them.
pub const ASKED_ACTION: &str = "asked_first";
pub const CAME_ACTION: &str = "came_to";

/// The host `typed` names, as the list keeps it: a site or a pasted address, lowercased, in the
/// address's own spelling (punycode for a name in another script, as Chromium's addresses and the
/// agents' card have it), without www., a trailing dot, scheme, path or port. None if it names no
/// web host.
pub fn host_of(typed: &str) -> Option<String> {
    let typed = typed.trim();
    if typed.is_empty() || typed.contains(char::is_whitespace) {
        return None;
    }
    let address = match typed.contains("://") {
        true => typed.to_string(),
        false => format!("https://{typed}"),
    };
    let url = url::Url::parse(&address).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = match url.host()? {
        url::Host::Domain(domain) => {
            let domain = domain.trim_end_matches('.');
            domain.strip_prefix("www.").unwrap_or(domain).to_string()
        }
        url::Host::Ipv4(address) => address.to_string(),
        url::Host::Ipv6(address) => format!("[{address}]"),
    };
    (!host.is_empty() && !host.starts_with('.')).then_some(host)
}

/// Whether `entry` covers its subdomains: a name of two labels or more does, and an IP address or
/// a single label such as localhost matches only itself.
fn takes_subdomains(entry: &str) -> bool {
    entry.contains('.') && !entry.starts_with('[') && entry.parse::<Ipv4Addr>().is_err()
}

/// The entry of `fenced` that `host` is under, as `site` gives a tab's host: the most specific,
/// so an entry and one of its subdomains both listed give the subdomain's.
pub fn entry_for<'a>(fenced: &'a [String], host: &str) -> Option<&'a str> {
    if host.is_empty() || fenced.is_empty() {
        return None;
    }
    // A tab's host is already the address's own spelling; anything else is made so.
    let host = match host.is_ascii() && !host.ends_with('.') {
        true => host.to_ascii_lowercase(),
        false => host_of(host)?,
    };
    (fenced.iter())
        .filter(|entry| {
            host == **entry
                || (takes_subdomains(entry)
                    && host.len() > entry.len()
                    && host.ends_with(entry.as_str())
                    && host.as_bytes()[host.len() - entry.len() - 1] == b'.')
        })
        .max_by_key(|entry| entry.len())
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|entry| entry.to_string()).collect()
    }

    /// A site or a pasted address is kept as its host alone, and what names no web host isn't
    /// kept (#3724).
    #[test]
    fn the_list_keeps_a_host() {
        for (typed, kept) in [
            ("mybank.example", "mybank.example"),
            ("  MyBank.Example  ", "mybank.example"),
            ("www.mybank.example", "mybank.example"),
            (
                "https://www.mybank.example:8443/login?next=/#top",
                "mybank.example",
            ),
            ("http://app.mybank.example/", "app.mybank.example"),
            ("mybank.example.", "mybank.example"),
            ("https://user:secret@mybank.example/", "mybank.example"),
            ("localhost:3000", "localhost"),
            ("192.168.1.10/admin", "192.168.1.10"),
            ("http://[::1]:8080/", "[::1]"),
            ("bücher.example", "xn--bcher-kva.example"),
            ("xn--bcher-kva.example", "xn--bcher-kva.example"),
        ] {
            assert_eq!(host_of(typed).as_deref(), Some(kept), "{typed}");
        }
        for typed in [
            "",
            "   ",
            "my bank",
            "file:///etc",
            "chrome://settings",
            "ftp://x.example",
        ] {
            assert_eq!(host_of(typed), None, "{typed}");
        }
    }

    /// An entry covers its subdomains, the most specific entry is the one a host is under, and a
    /// lookalike or a parent isn't under it (#3724).
    #[test]
    fn an_entry_covers_its_subdomains() {
        let fenced = list(&["mybank.example", "app.pension.example"]);
        assert_eq!(entry_for(&fenced, "mybank.example"), Some("mybank.example"));
        assert_eq!(
            entry_for(&fenced, "app.mybank.example"),
            Some("mybank.example")
        );
        assert_eq!(
            entry_for(&fenced, "a.b.mybank.example"),
            Some("mybank.example")
        );
        assert_eq!(entry_for(&fenced, "notmybank.example"), None);
        assert_eq!(entry_for(&fenced, "mybank.example.evil.example"), None);
        assert_eq!(entry_for(&fenced, "example"), None);
        assert_eq!(entry_for(&fenced, "pension.example"), None);
        assert_eq!(
            entry_for(&fenced, "x.app.pension.example"),
            Some("app.pension.example")
        );
        let both = list(&["mybank.example", "app.mybank.example"]);
        assert_eq!(
            entry_for(&both, "x.app.mybank.example"),
            Some("app.mybank.example")
        );
        assert_eq!(
            entry_for(&both, "www2.mybank.example"),
            Some("mybank.example")
        );
        assert_eq!(entry_for(&[], "mybank.example"), None);
        assert_eq!(entry_for(&fenced, ""), None);
    }

    /// An IP address and a single label match only themselves, a host in another script matches
    /// in either spelling, and a trailing dot is the same host (#3724).
    #[test]
    fn the_edges_of_a_host() {
        let fenced = list(&[
            "localhost",
            "192.168.1.10",
            "[::1]",
            "xn--bcher-kva.example",
        ]);
        assert_eq!(entry_for(&fenced, "localhost"), Some("localhost"));
        assert_eq!(entry_for(&fenced, "app.localhost"), None);
        assert_eq!(entry_for(&fenced, "192.168.1.10"), Some("192.168.1.10"));
        assert_eq!(entry_for(&fenced, "1.192.168.1.10"), None);
        assert_eq!(entry_for(&fenced, "[::1]"), Some("[::1]"));
        assert_eq!(
            entry_for(&fenced, "xn--bcher-kva.example"),
            Some("xn--bcher-kva.example")
        );
        assert_eq!(
            entry_for(&fenced, "bücher.example"),
            Some("xn--bcher-kva.example")
        );
        assert_eq!(
            entry_for(&fenced, "shop.bücher.example"),
            Some("xn--bcher-kva.example")
        );
        let fenced = list(&["mybank.example"]);
        assert_eq!(
            entry_for(&fenced, "mybank.example."),
            Some("mybank.example")
        );
        assert_eq!(
            entry_for(&fenced, "app.mybank.example."),
            Some("mybank.example")
        );
        assert_eq!(entry_for(&fenced, "MyBank.Example"), Some("mybank.example"));
    }
}
