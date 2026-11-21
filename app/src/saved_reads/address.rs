//! The address a use of a saved read reads. Two checks guard it, and each holds alone, so a slip
//! in either still reads nothing off the definition's site or path:
//! - the values: each is of its parameter's kind (an integer in its range, one of its list, or a
//!   string matching its narrow pattern), none is missing and none is unknown, and each is
//!   percent-encoded but for the characters every part of an address holds as they are;
//! - the address: the finished one is on the site, with no user, password or fragment, on the
//!   definition's path, a parameter filling one whole segment that holds no separator even once
//!   decoded, and with exactly the definition's query keys and literal values.

use serde_json::{Map, Value};
use url::Url;

use super::definition::{Definition, Param, Piece, dots, pieces, unreserved};

/// The longest value a use fills a parameter with, in characters.
pub const VALUE_MOST: usize = 200;

/// Why a use's address can't be read, as a sentence for the agent that asked.
#[derive(Clone, Debug, PartialEq)]
pub struct Refused(pub String);

fn refused<T>(why: impl Into<String>) -> Result<T, Refused> {
    Err(Refused(why.into()))
}

/// The address a use of `read` with `params` reads, both checks on.
pub fn address(read: &Definition, params: &Map<String, Value>) -> Result<Url, Refused> {
    build(
        read,
        params,
        Checks {
            values: true,
            address: true,
        },
    )
}

/// Which checks build runs. Only the tests turn one off, to show the other holds alone.
#[derive(Clone, Copy, Debug)]
pub(super) struct Checks {
    pub values: bool,
    pub address: bool,
}

pub(super) fn build(
    read: &Definition,
    params: &Map<String, Value>,
    checks: Checks,
) -> Result<Url, Refused> {
    if checks.values {
        if let Some(unknown) = params.keys().find(|name| !read.params.contains_key(*name)) {
            return refused(format!(
                "\"{}\" has no parameter \"{}\".",
                read.name,
                super::text(unknown, 40)
            ));
        }
    }
    let mut filled = String::with_capacity(read.url.len() + 64);
    let (path, query) = read
        .url
        .split_once('?')
        .map_or((read.url.as_str(), None), |(p, q)| (p, Some(q)));
    let fill = |part: &str, out: &mut String| -> Result<(), Refused> {
        for piece in pieces(part).map_err(Refused)? {
            match piece {
                Piece::Literal(text) => out.push_str(text),
                Piece::Param(name) => {
                    let Some(value) = params.get(name) else {
                        return refused(format!(
                            "\"{}\" requires parameter \"{name}\".",
                            read.name
                        ));
                    };
                    let value = match value {
                        Value::String(text) => text.clone(),
                        Value::Number(number) => number.to_string(),
                        _ if checks.values => {
                            return refused(format!("\"{name}\" must be a string or number."));
                        }
                        other => other.to_string(),
                    };
                    if checks.values {
                        value_fits(name, &read.params[name], &value)?;
                        out.push_str(&encoded(&value));
                    } else {
                        out.push_str(&value);
                    }
                }
            }
        }
        Ok(())
    };
    for (index, segment) in path.split('/').enumerate() {
        if index > 0 {
            filled.push('/');
        }
        fill(segment, &mut filled)?;
    }
    if let Some(query) = query {
        filled.push('?');
        for (index, pair) in query.split('&').enumerate() {
            if index > 0 {
                filled.push('&');
            }
            fill(pair, &mut filled)?;
        }
    }
    let url = match Url::parse(&format!("{}{filled}", read.site)) {
        Ok(url) => url,
        Err(_) => {
            return refused(format!(
                "Can't build a valid URL for \"{}\" with these values.",
                read.name
            ));
        }
    };
    if checks.address {
        on_path(read, &url)?;
    }
    Ok(url)
}

/// Checks a value against its parameter's kind.
fn value_fits(name: &str, param: &Param, value: &str) -> Result<(), Refused> {
    if value.chars().count() > VALUE_MOST {
        return refused(format!(
            "\"{name}\" is longer than {VALUE_MOST} characters."
        ));
    }
    match param {
        Param::Integer { min, max } => {
            let whole = value.strip_prefix('-').unwrap_or(value);
            let number = (!whole.is_empty() && whole.bytes().all(|b| b.is_ascii_digit()))
                .then(|| value.parse::<i64>().ok())
                .flatten();
            match number {
                Some(number) if (*min..=*max).contains(&number) => Ok(()),
                Some(_) => refused(format!("\"{name}\" must be a number from {min} to {max}.")),
                None => refused(format!(
                    "\"{name}\" must be an integer from {min} to {max}."
                )),
            }
        }
        Param::OneOf(values) if values.iter().any(|one| one == value) => Ok(()),
        Param::OneOf(values) => {
            refused(format!("\"{name}\" must be one of: {}.", values.join(", ")))
        }
        Param::Text { pattern } => {
            let whole = fancy_regex::Regex::new(&format!("^(?:{pattern})$"));
            // The pattern's kind can't match a separator, but the value is held to it too, so
            // that a pattern kept before a rule changed still can't let one through.
            let fits = whole.is_ok_and(|whole| whole.is_match(value).unwrap_or(false))
                && value.chars().all(unreserved)
                && !dots(value);
            if fits {
                Ok(())
            } else {
                refused(format!("\"{name}\" doesn't match pattern `{pattern}`."))
            }
        }
    }
}

/// `value` percent-encoded but for RFC 3986's unreserved characters.
fn encoded(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if unreserved(byte as char) && byte.is_ascii() {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Whether `url` is an address of `read`'s: on its site and path, with only its query.
pub(super) fn fits(read: &Definition, url: &Url) -> bool {
    on_path(read, url).is_ok()
}

/// Checks the finished address is on the read's site and path, with only its query.
fn on_path(read: &Definition, url: &Url) -> Result<(), Refused> {
    const SITE: &str = "The filled URL resolves to a different site than the read allows.";
    const PARTS: &str =
        "The filled URL adds or changes URL components outside the read's definition.";
    const QUERY: &str =
        "The filled URL adds or changes query parameters outside the read's definition.";
    let left = |why: &str| refused(why);
    let (path, query) = read.url.split_once('?').unwrap_or((&read.url, ""));
    let off_path = || refused(format!("The filled URL's path falls outside `{path}`."));
    if url.origin().ascii_serialization() != read.site {
        return left(SITE);
    }
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return left(PARTS);
    }
    let mut pattern = String::from("^");
    for (index, segment) in path.split('/').enumerate() {
        if index > 0 {
            pattern.push('/');
        }
        for piece in pieces(segment).map_err(Refused)? {
            match piece {
                Piece::Literal(text) => pattern.push_str(&fancy_regex::escape(text)),
                Piece::Param(_) => pattern.push_str("([^/?#]+)"),
            }
        }
    }
    pattern.push('$');
    let Ok(pattern) = fancy_regex::Regex::new(&pattern) else {
        return off_path();
    };
    let Ok(Some(found)) = pattern.captures(url.path()) else {
        return off_path();
    };
    // A server may decode a value before it routes, so a segment that decodes to a separator,
    // a dot segment or another encoding is as far off the path as one that is one (#3814).
    for value in found.iter().skip(1).flatten() {
        let decoded = percent_decoded(value.as_str());
        if decoded.contains(['/', '\\', '%', '?', '#']) || dots(&decoded) {
            return off_path();
        }
    }
    let mut want: Vec<(String, Option<String>)> = Vec::new();
    if !query.is_empty() {
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            let literal = !value.contains('{');
            want.push((
                percent_decoded(key),
                literal.then(|| percent_decoded(value)),
            ));
        }
    }
    let mut have: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if url.query() == Some("") || (url.query().is_some() && want.is_empty()) {
        return left(QUERY);
    }
    want.sort();
    have.sort();
    let same = want.len() == have.len()
        && want.iter().zip(&have).all(|((key, literal), (k, v))| {
            key == k && literal.as_ref().is_none_or(|literal| literal == v)
        });
    if !same {
        return left(QUERY);
    }
    Ok(())
}

/// `text` with its %XX escapes decoded once, as text; a byte that isn't UTF-8 becomes U+FFFD.
fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[at] == b'%' && at + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (hex(bytes[at + 1]), hex(bytes[at + 2])) {
                out.push((high * 16 + low) as u8);
                at += 3;
                continue;
            }
        }
        out.push(bytes[at]);
        at += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
