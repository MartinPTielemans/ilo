//! Where a web address in a line that wraps may break: after a scheme's `//`, after each dot of
//! its host and before its `:port`, so a long host fills each line with whole parts of itself
//! rather than "http:/" over "/sign-in…localhos" over "t:49217" (#3748). The address is never
//! shortened, elided or reordered, and what is copied, sent or logged stays the text as it is.
//!
//! A break hint in the string can't do it: GPUI finds a line's breaks from the shaped glyphs, and
//! CoreText and DirectWrite give a zero-width space no glyph of its own, so only Linux's shaper
//! would break there. So a line holding an address is drawn as a row of its parts that wraps
//! between them, each part a text of its own, and a part wider than the line breaks inside, as
//! GPUI breaks any word with no place to break before. A line with no address is one text, as
//! before, so prose and a page's long alert cost nothing more. The parts have no id, so they add
//! nothing to what a screen reader reads: the dialog and the cards name their own text.

use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, div};

/// Where a part ends, in the marked text, drawn as nothing.
const BREAK: char = '\u{200B}';

/// `text` drawn so each web address in it wraps between its parts.
pub fn text(text: &str) -> AnyElement {
    let Some(lines) = lines(text) else {
        return SharedString::from(text.to_owned()).into_any_element();
    };
    div()
        .flex()
        .flex_col()
        .children(lines.into_iter().map(|parts| {
            match <[String; 1]>::try_from(parts) {
                Ok([line]) => SharedString::from(line).into_any_element(),
                Err(parts) => div()
                    .flex()
                    .flex_wrap()
                    .children(parts.into_iter().map(|part| div().min_w_0().child(part)))
                    .into_any_element(),
            }
        }))
        .into_any_element()
}

/// The lines of `text`, each as the parts it wraps between, or None if it holds no web address.
/// A line with no address is one part, so it wraps as text does; one with an address breaks after
/// each space and at the address's places. An empty line is a space, so it keeps its height.
fn lines(text: &str) -> Option<Vec<Vec<String>>> {
    let marked = marked(text);
    if !marked.contains(BREAK) {
        return None;
    }
    let lines = marked.split('\n').map(|line| {
        if !line.contains(BREAK) {
            return vec![if line.is_empty() {
                " ".into()
            } else {
                line.into()
            }];
        }
        let mut parts = Vec::new();
        let mut part = String::new();
        for c in line.chars() {
            if c != BREAK {
                part.push(c);
            }
            // A no-break space joins, as "wants\u{a0}to" does.
            if (c == BREAK || c == ' ') && !part.is_empty() {
                parts.push(std::mem::take(&mut part));
            }
        }
        if !part.is_empty() {
            parts.push(part);
        }
        parts
    });
    Some(lines.collect())
}

/// `text` with a BREAK at each of its web addresses' places to break, and the rest as it was.
fn marked(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    for piece in text.split_inclusive(char::is_whitespace) {
        let word = piece.trim_end_matches(char::is_whitespace);
        let space = &piece[word.len()..];
        let core = word.trim_start_matches(OPENING);
        let lead = &word[..word.len() - core.len()];
        let core = core.trim_end_matches(CLOSING);
        let trail = &word[lead.len() + core.len()..];
        match broken(core) {
            Some(core) => {
                out.push_str(lead);
                out.push_str(&core);
                out.push_str(trail);
            }
            None => out.push_str(word),
        }
        out.push_str(space);
    }
    out
}

/// What a sentence puts around an address: quotes and brackets before it, and those and its
/// punctuation after. A `[` stays, as it opens an IPv6 host.
const OPENING: &[char] = &['(', '<', '"', '\'', '“', '‘'];
const CLOSING: &[char] = &['.', ',', ';', ':', '!', '?', ')', '>', '"', '\'', '”', '’'];

/// `word` with its places to break, if it's a web address: a scheme's, or a host with a dot, a
/// bracketed IPv6 host or a host with a port, with or without a path after it.
fn broken(word: &str) -> Option<String> {
    let (scheme, rest) = match word.split_once("://") {
        Some((scheme, rest)) if is_scheme(scheme) => (&word[..scheme.len() + 3], rest),
        _ => ("", word),
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, path) = rest.split_at(end);
    // A user name before the host is drawn as it is; only the host and port break.
    let user = authority.rfind('@').map_or(0, |at| at + 1);
    let (user, host_port) = authority.split_at(user);
    let (host, port) = match host_port.strip_prefix('[') {
        Some(inside) => {
            let close = inside.find(']')? + 2;
            host_port.split_at(close)
        }
        None => match host_port.rfind(':') {
            Some(colon) => host_port.split_at(colon),
            None => (host_port, ""),
        },
    };
    let port_ok =
        port.is_empty() || (port.len() > 1 && port[1..].bytes().all(|b| b.is_ascii_digit()));
    if host.is_empty() || !port_ok {
        return None;
    }
    let bracketed = host.starts_with('[');
    if !bracketed && !is_host(host, !scheme.is_empty(), !port.is_empty()) {
        return None;
    }
    let mut out = String::with_capacity(word.len() + 8);
    out.push_str(scheme);
    if !scheme.is_empty() {
        out.push(BREAK);
    }
    out.push_str(user);
    match bracketed {
        // An IPv6 host's colons aren't a port's, and it has no dots.
        true => out.push_str(host),
        false => {
            let mut labels = host.split('.').peekable();
            while let Some(label) = labels.next() {
                out.push_str(label);
                if labels.peek().is_some() {
                    out.push('.');
                    out.push(BREAK);
                }
            }
        }
    }
    if !port.is_empty() {
        out.push(BREAK);
        out.push_str(port);
    }
    out.push_str(path);
    Some(out)
}

fn is_scheme(scheme: &str) -> bool {
    let mut chars = scheme.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// Whether `host` is a host's name: labels of letters, digits and hyphens, in any script, between
/// dots. Without a scheme, one with a port needs a dot or a letter, so a time such as "12:30"
/// isn't taken for one, and one without a port a dot and a last label of two or more characters
/// with a letter, so prose such as "e.g" or "2.5" isn't.
fn is_host(host: &str, scheme: bool, port: bool) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    let named = labels
        .iter()
        .all(|label| !label.is_empty() && label.chars().all(|c| c.is_alphanumeric() || c == '-'));
    if !named {
        return false;
    }
    let last = labels[labels.len() - 1];
    let dotted = labels.len() > 1;
    match (scheme, port) {
        (true, _) => true,
        (false, true) => dotted || host.chars().any(char::is_alphabetic),
        (false, false) => {
            dotted && last.chars().count() > 1 && last.chars().any(char::is_alphabetic)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `marked` with each break shown as `|`, for reading.
    fn shown(text: &str) -> String {
        marked(text).replace(BREAK, "|")
    }

    /// The marked text is the text with breaks added and nothing else.
    fn same_text(text: &str) {
        assert_eq!(marked(text).replace(BREAK, ""), text, "{text}");
    }

    #[test]
    fn an_address_breaks_after_its_slashes_its_dots_and_before_its_port() {
        let address = "http://sign-in.accounts.a-long-example-host-for-the-dialog.localhost:49217";
        assert_eq!(
            shown(address),
            "http://|sign-in.|accounts.|a-long-example-host-for-the-dialog.|localhost|:49217"
        );
        same_text(address);
    }

    #[test]
    fn a_host_in_a_sentence_breaks_and_the_sentence_does_not() {
        let card = "sign-in.accounts.a-long-example-host-for-the-card.localhost asks you to sign \
                    in. Hand the tab back when you’re done.";
        assert_eq!(
            shown(card),
            "sign-in.|accounts.|a-long-example-host-for-the-card.|localhost asks you to sign in. \
             Hand the tab back when you’re done."
        );
        same_text(card);
    }

    #[test]
    fn a_path_and_a_closing_mark_stay_as_they_are() {
        assert_eq!(
            shown("(see https://docs.example.org/a.b/c?d=e.f)."),
            "(see https://|docs.|example.|org/a.b/c?d=e.f)."
        );
        assert_eq!(shown("“shop.example.com”,"), "“shop.|example.|com”,");
    }

    #[test]
    fn an_international_host_breaks_at_its_dots_in_either_form() {
        assert_eq!(shown("bücher.beispiel.de"), "bücher.|beispiel.|de");
        assert_eq!(
            shown("xn--bcher-kva.xn--beispiel-q4a.de"),
            "xn--bcher-kva.|xn--beispiel-q4a.|de"
        );
        assert_eq!(shown("https://例え.テスト/"), "https://|例え.|テスト/");
    }

    #[test]
    fn an_ipv6_host_keeps_its_colons_and_breaks_before_its_port() {
        assert_eq!(
            shown("http://[2001:db8::1]:8443/"),
            "http://|[2001:db8::1]|:8443/"
        );
        assert_eq!(shown("[::1]"), "[::1]");
        same_text("http://[2001:db8::1]:8443/");
    }

    #[test]
    fn one_long_label_has_no_place_to_break_inside_it() {
        let label = "a".repeat(80);
        let host = format!("{label}.example");
        assert_eq!(shown(&host), format!("{label}.|example"));
    }

    #[test]
    fn a_host_with_a_port_and_no_dot_breaks_before_the_port() {
        assert_eq!(shown("localhost:8080"), "localhost|:8080");
        assert_eq!(shown("http://localhost/"), "http://|localhost/");
        assert_eq!(shown("10.0.0.1:80"), "10.|0.|0.|1|:80");
    }

    #[test]
    fn prose_is_not_taken_for_an_address() {
        for text in [
            "Sign in. Hand the tab back when you’re done.",
            "e.g. 2.5 MB of 3.14 at 12:30, 3:2",
            "Settings › Agents: on.",
            "note: done",
            "a: b",
            "",
        ] {
            assert_eq!(marked(text), text);
            assert_eq!(lines(text), None, "{text}");
        }
    }

    #[test]
    fn whitespace_and_line_ends_are_kept() {
        let message = "http://a.example:81\n\nYour connection to this site isn’t private.";
        assert_eq!(
            shown(message),
            "http://|a.|example|:81\n\nYour connection to this site isn’t private."
        );
        same_text(message);
    }

    #[test]
    fn a_line_with_an_address_wraps_between_its_parts_and_its_words() {
        let card = "sign-in.accounts.long-host.localhost asks you to sign in.";
        assert_eq!(
            lines(card).unwrap(),
            [vec![
                "sign-in.",
                "accounts.",
                "long-host.",
                "localhost ",
                "asks ",
                "you ",
                "to ",
                "sign ",
                "in."
            ]]
        );
        let title = "shop.example.com wants\u{a0}to";
        assert_eq!(
            lines(title).unwrap(),
            [vec!["shop.", "example.", "com ", "wants\u{a0}to"]]
        );
    }

    #[test]
    fn a_line_without_an_address_stays_one_text() {
        let message = "http://a.example:81\n\nYour connection to this site isn’t private.";
        assert_eq!(
            lines(message).unwrap(),
            [
                vec!["http://", "a.", "example", ":81"],
                vec![" "],
                vec!["Your connection to this site isn’t private."]
            ]
        );
    }

    #[test]
    fn the_parts_together_are_the_text() {
        for text in [
            "http://sign-in.accounts.a-long-example-host-for-the-dialog.localhost:49217",
            "(see https://docs.example.org/a.b/c?d=e.f).",
            "http://[2001:db8::1]:8443/ and bücher.beispiel.de",
        ] {
            let parts: String = lines(text).unwrap().concat().concat();
            assert_eq!(parts, text);
        }
    }
}
