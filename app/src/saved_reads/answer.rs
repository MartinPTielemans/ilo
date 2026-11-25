//! What a read's answer says: rows of text under the definition's columns, or that it's broken,
//! with a sentence a person can read, or that the site wants a sign-in. The core reads the
//! address and answers with its status, its final address, whether it stopped at a redirect off
//! the site, its MIME type and its body (#3820); deciding what that means is here. A body is
//! parsed as data, never run, and an answer that doesn't fit the definition is broken, never
//! read as other rows.

use serde_json::Value;
use url::Url;

use super::definition::{Column, Definition, Kind, Type};
use super::table::{Cell, tables};
use crate::product::PRODUCT;

/// The most of a body the core reads (#3820), and so the most a read takes.
pub const BODY_MOST: usize = 4 << 20;

/// A row's values, a column each, in the definition's order: text, clipped, never markup.
#[derive(Clone, Debug, PartialEq)]
pub struct Row(pub Vec<String>);

/// Why an answer doesn't fit its read, as a sentence for the user and the agent. Settings shows
/// it and the log keeps it, which never keep a row's content, so it holds only what the
/// definition names, row numbers and counts, never a site's text.
#[derive(Clone, Debug, PartialEq)]
pub struct Broken(pub String);

/// What a read's answer came to.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Rows(Vec<Row>),
    /// The site wants a sign-in first. Where it sent the read never goes to an agent.
    NeedsSignIn,
    Broken(Broken),
}

/// What the core answered a read with.
#[derive(Clone, Debug, Default)]
pub struct Answered<'a> {
    pub status: u16,
    /// The address it read last, after the redirects on the site it followed.
    pub url: &'a str,
    /// It stopped at a redirect to another origin.
    pub left: bool,
    pub mime: &'a str,
    pub body: &'a [u8],
    /// The body was longer than the core reads, so it's cut.
    pub cut: bool,
}

/// What `answered` means for `read`: a 401 or a 403, a redirect off the site or one to its
/// sign-in path wants a sign-in; another status, an address off the read's, a cut body or one
/// that doesn't fit is broken; else its rows.
pub fn outcome(read: &Definition, answered: &Answered<'_>) -> Outcome {
    if answered.left || matches!(answered.status, 401 | 403) {
        return Outcome::NeedsSignIn;
    }
    let landed = Url::parse(answered.url).ok();
    if let (Some(landed), Some(sign_in)) = (&landed, &read.sign_in) {
        let on_site = landed.origin().ascii_serialization() == read.site;
        let path = landed.path();
        let under = path.strip_prefix(sign_in.as_str()).is_some_and(|rest| {
            rest.is_empty() || rest.starts_with(['/', '?']) || sign_in.ends_with('/')
        });
        if on_site && under {
            return Outcome::NeedsSignIn;
        }
    }
    if !(200..300).contains(&answered.status) {
        return broken(format!("The site returned error {}.", answered.status));
    }
    let Some(landed) = landed.filter(|landed| super::address::fits(read, landed)) else {
        return broken("The site redirected to another address.");
    };
    if answered.cut || answered.body.len() > BODY_MOST {
        return broken(format!("The site sent more than {} MB.", BODY_MOST >> 20));
    }
    match read_body(read, answered.body, answered.mime, &landed) {
        Ok(rows) => Outcome::Rows(rows),
        Err(why) => Outcome::Broken(why),
    }
}

/// A field path as a reason names it, with the title treatment (#3690).
fn named(path: &str) -> String {
    super::text(path, super::definition::NAME_MOST)
}

fn broken(why: impl Into<String>) -> Outcome {
    Outcome::Broken(Broken(why.into()))
}

/// The rows `body` holds for `read`, read at `url`, shape checked.
pub fn read(read: &Definition, body: &[u8], mime: &str, url: &Url) -> Result<Vec<Row>, Broken> {
    read_body(read, body, mime, url)
}

fn read_body(read: &Definition, body: &[u8], mime: &str, url: &Url) -> Result<Vec<Row>, Broken> {
    let rows = match read.kind {
        Kind::Data => data_rows(read, body)?,
        Kind::Table => table_rows(read, body, mime)?,
    };
    shaped(read, rows, url)
}

/// A value as read, before its shape is checked: its text, and for a table's cell its link.
type Read = (String, Option<String>);

fn data_rows(read: &Definition, body: &[u8]) -> Result<Vec<Vec<Read>>, Broken> {
    let body = body.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(body);
    let Ok(answer) = serde_json::from_slice::<Value>(body) else {
        return Err(Broken(format!(
            "The site didn't send data {PRODUCT} can read."
        )));
    };
    let found = at_path(&answer, &read.rows)
        .ok_or_else(|| Broken(format!("The site's data is missing {}.", named(&read.rows))))?;
    let rows: Vec<&Value> = match found {
        Value::Array(rows) => rows.iter().take(super::ROWS_KEPT).collect(),
        Value::Object(_) => vec![found],
        _ if read.rows.is_empty() => {
            return Err(Broken("The site's data isn't formatted as rows.".into()));
        }
        _ => {
            return Err(Broken(format!(
                "{} isn't formatted as rows.",
                named(&read.rows)
            )));
        }
    };
    let mut out = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let number = index + 1;
        let mut values = Vec::with_capacity(read.columns.len());
        for column in &read.columns {
            let value = match at_path(row, &column.from) {
                None => return Err(Broken(format!("Row {number}: {} is missing.", column.name))),
                Some(Value::Null) => String::new(),
                Some(Value::String(text)) => text.clone(),
                Some(Value::Bool(yes)) => yes.to_string(),
                Some(Value::Number(number)) => number.to_string(),
                Some(Value::Array(_) | Value::Object(_)) => {
                    return Err(Broken(format!(
                        "Row {number}: {} isn't a single value.",
                        column.name
                    )));
                }
            };
            values.push((value, None));
        }
        out.push(values);
    }
    Ok(out)
}

/// The value at a dot path, each key a field's name or a list's index; "" is the value itself.
fn at_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }
    path.split('.').try_fold(value, |value, key| match value {
        Value::Object(fields) => fields.get(key),
        Value::Array(items) => key.parse::<usize>().ok().and_then(|index| items.get(index)),
        _ => None,
    })
}

fn table_rows(read: &Definition, body: &[u8], mime: &str) -> Result<Vec<Vec<Read>>, Broken> {
    let html = crate::link_card::page::decode(body, mime);
    // The read's row in Settings lists its columns, so a reason names none of them.
    let these = if read.columns.len() == 1 {
        "this column"
    } else {
        "these columns"
    };
    let mut found = Vec::new();
    for table in tables(&html) {
        let places: Option<Vec<usize>> = read
            .columns
            .iter()
            .map(|column| place(&table.headers, &column.from))
            .collect();
        if let Some(places) = places {
            found.push((table, places));
        }
    }
    // Two tables with the read's columns could each be the one it was made from, and reading
    // the wrong one would be wrong rows, so neither is read.
    let (table, places) = match found.len() {
        0 => return Err(Broken(format!("No table on the page has {these}."))),
        1 => found.pop().unwrap(),
        many => {
            return Err(Broken(format!(
                "{many} tables have {these}; {PRODUCT} can't pick one."
            )));
        }
    };
    Ok((table.rows.into_iter().take(super::ROWS_KEPT))
        .map(|row| {
            (places.iter())
                .map(|&place| {
                    let Cell { text, link } = row[place].clone();
                    (text, link)
                })
                .collect()
        })
        .collect())
}

/// Which of `headers` a column read from `from` is: the one with those words, else the one
/// alone whose words start with them, as "Population" names "Population (2020)".
fn place(headers: &[String], from: &str) -> Option<usize> {
    let from = super::table::cell_text(from).to_lowercase();
    let lower: Vec<String> = headers.iter().map(|header| header.to_lowercase()).collect();
    let exact: Vec<usize> = (0..lower.len()).filter(|&at| lower[at] == from).collect();
    if let [one] = exact[..] {
        return Some(one);
    }
    if !exact.is_empty() {
        return None;
    }
    let starts: Vec<usize> = (0..lower.len())
        .filter(|&at| {
            lower[at]
                .strip_prefix(&from)
                .is_some_and(|rest| rest.starts_with(|c: char| !c.is_alphanumeric()))
        })
        .collect();
    match starts[..] {
        [one] => Some(one),
        _ => None,
    }
}

/// The rows, each value of its column's type, and at least as many as the read needs.
fn shaped(read: &Definition, rows: Vec<Vec<Read>>, url: &Url) -> Result<Vec<Row>, Broken> {
    let mut out = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let number = index + 1;
        let mut values = Vec::with_capacity(row.len());
        for (column, (value, link)) in read.columns.iter().zip(row) {
            values.push(typed(column, &value, link.as_deref(), url, number)?);
        }
        out.push(Row(values));
    }
    if out.len() < read.least_rows {
        let rows = if out.len() == 1 { "row" } else { "rows" };
        return Err(Broken(format!(
            "Found {} {rows}; expected at least {}.",
            out.len(),
            read.least_rows
        )));
    }
    Ok(out)
}

fn typed(
    column: &Column,
    value: &str,
    link: Option<&str>,
    url: &Url,
    number: usize,
) -> Result<String, Broken> {
    let text = super::text(value, super::VALUE_KEPT);
    let name = &column.name;
    match column.kind {
        Type::Text => Ok(text),
        Type::Number if text.is_empty() => Err(Broken(format!("Row {number}: {name} is missing."))),
        Type::Number => {
            let digits: String = text
                .chars()
                .filter(|c| !matches!(c, ',' | ' ' | '\u{A0}' | '\u{202F}' | '_'))
                .collect();
            let digits = digits.replace('\u{2212}', "-");
            let plain = !digits.is_empty()
                && digits
                    .chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'));
            match digits.parse::<f64>() {
                Ok(parsed) if plain && parsed.is_finite() => Ok(text),
                _ => Err(Broken(format!("Row {number}: {name} isn't a number."))),
            }
        }
        Type::Address => {
            // A table's link is the cell's address; a data field's value is.
            let written = link.unwrap_or(&text).trim();
            if written.is_empty() {
                return Err(Broken(format!("Row {number}: {name} is missing.")));
            }
            match url.join(written) {
                // An address is kept whole or not at all: one cut short would lead somewhere else.
                Ok(address)
                    if matches!(address.scheme(), "http" | "https")
                        && address.username().is_empty()
                        && address.password().is_none()
                        && address.as_str().len() <= super::VALUE_KEPT =>
                {
                    Ok(address.to_string())
                }
                _ => Err(Broken(format!("Row {number}: {name} isn't a web address."))),
            }
        }
    }
}
