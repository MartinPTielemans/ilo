//! What a saved read is: its name, its site, its kind, the address it reads with the parameters
//! that fill it, the columns it reads with their types, and the least rows a fitting answer has.
//! A definition holds no code: an address on its own site, field paths or column headers, and
//! patterns of a narrow kind. Everything is checked when it's made, so a definition that could
//! read off its site or path is never kept, and a use only has values left to check.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

/// The most characters a read's or a column's name has: a task title's (#3690).
pub const NAME_MOST: usize = crate::agents::task::TITLE_KEPT;
/// The most columns a read has.
pub const COLUMNS_MOST: usize = 20;
/// The most parameters a read has.
pub const PARAMS_MOST: usize = 8;
/// The longest address template, in bytes.
pub const URL_MOST: usize = 1000;
/// The longest pattern, field path or header, in characters.
pub const PATTERN_MOST: usize = 200;
/// The most values a one-of parameter lists.
pub const VALUES_MOST: usize = 100;

#[derive(Clone, Debug, PartialEq)]
pub struct Definition {
    pub name: String,
    /// The site's origin, "https://host[:port]", in lower case without its default port.
    pub site: String,
    pub kind: Kind,
    /// A path on the site from its root, with an optional query, and a {name} for each
    /// parameter in a path segment or a query value: "/api/v1/crates/{crate}/downloads".
    pub url: String,
    pub params: BTreeMap<String, Param>,
    /// For the data kind, the path in the answer to its list of rows, "" for the answer itself.
    pub rows: String,
    pub columns: Vec<Column>,
    pub least_rows: usize,
    /// The path on the site its sign-in page is at, such as /login, if it has one: a read the
    /// site sends there needs a sign-in rather than being broken.
    pub sign_in: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// The site's own data: a JSON answer, read by field paths.
    Data,
    /// A table on a page, found by its column headers. The page's scripts never run.
    Table,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Param {
    Integer {
        min: i64,
        max: i64,
    },
    OneOf(Vec<String>),
    /// A string matching the whole of `pattern`, whose kind (safe_pattern) can't let a path
    /// separator, an encoded one or a query through.
    Text {
        pattern: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub name: String,
    pub kind: Type,
    /// For the data kind, the field's path in a row; for the table kind, its header's words.
    pub from: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Type {
    Text,
    Number,
    /// An http(s) address.
    Address,
}

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::Data => "json",
            Kind::Table => "table",
        }
    }
}

impl Type {
    pub fn id(self) -> &'static str {
        match self {
            Type::Text => "text",
            Type::Number => "number",
            Type::Address => "address",
        }
    }
}

/// Why a definition can't be made, as a sentence for the agent that proposed it.
#[derive(Clone, Debug, PartialEq)]
pub struct Wrong(pub String);

fn wrong<T>(why: impl Into<String>) -> Result<T, Wrong> {
    Err(Wrong(why.into()))
}

impl Definition {
    /// The definition `value` describes, checked whole, or why it can't be one.
    pub fn parse(value: &Value) -> Result<Definition, Wrong> {
        let field = |key: &str| value.get(key);
        let string = |key: &str| -> Result<&str, Wrong> {
            match field(key) {
                Some(Value::String(text)) => Ok(text),
                _ => wrong(format!(
                    "{key} is missing: a definition has a name, a site, a kind, a url and its columns."
                )),
            }
        };
        let name = named("name", string("name")?)?;
        let site = site(string("site")?)?;
        let kind = match string("kind")? {
            "json" => Kind::Data,
            "table" => Kind::Table,
            other => return wrong(format!("kind is json or table, not “{}”.", shown(other))),
        };
        let params = match field("params") {
            None | Some(Value::Null) => BTreeMap::new(),
            Some(Value::Object(params)) => self::params(params)?,
            Some(_) => return wrong("params is an object of each parameter's name and kind."),
        };
        let url = string("url")?.to_string();
        template(&url, &params)?;
        let rows = match (kind, field("rows")) {
            (Kind::Data, Some(Value::String(rows))) => path("rows", rows)?,
            (Kind::Data, None | Some(Value::Null)) => String::new(),
            (Kind::Data, Some(_)) => {
                return wrong("rows is the path to the answer's list of rows, such as data.items.");
            }
            (Kind::Table, None | Some(Value::Null)) => String::new(),
            (Kind::Table, Some(_)) => {
                return wrong("A table read has no rows path: its rows are the table's.");
            }
        };
        let columns = match field("columns") {
            Some(Value::Array(columns)) => self::columns(columns, kind)?,
            _ => {
                return wrong(
                    "columns is a list of the columns it reads, each with a name, a type and where it's read from.",
                );
            }
        };
        let least_rows = match field("least_rows") {
            None | Some(Value::Null) => 1,
            Some(least) => match least.as_u64() {
                Some(least) if (1..=super::ROWS_KEPT as u64).contains(&least) => least as usize,
                _ => {
                    return wrong(format!(
                        "least_rows is a whole number from 1 to {}.",
                        super::ROWS_KEPT
                    ));
                }
            },
        };
        let sign_in = match field("sign_in") {
            None | Some(Value::Null) => None,
            Some(Value::String(path)) if sign_in_path(path) => Some(path.clone()),
            Some(_) => {
                return wrong("sign_in is the path of the site's sign-in page, such as /login.");
            }
        };
        Ok(Definition {
            name,
            site,
            kind,
            url,
            params,
            rows,
            columns,
            least_rows,
            sign_in,
        })
    }

    /// The definition as it's kept, which parse reads back.
    pub fn to_json(&self) -> Value {
        let params: Map<String, Value> = (self.params.iter())
            .map(|(name, param)| {
                let param = match param {
                    Param::Integer { min, max } => {
                        json!({ "type": "integer", "min": min, "max": max })
                    }
                    Param::OneOf(values) => json!({ "type": "one_of", "values": values }),
                    Param::Text { pattern } => json!({ "type": "text", "pattern": pattern }),
                };
                (name.clone(), param)
            })
            .collect();
        let columns: Vec<Value> = (self.columns.iter())
            .map(|column| json!({ "name": column.name, "type": column.kind.id(), "from": column.from }))
            .collect();
        let mut value = json!({
            "name": self.name,
            "site": self.site,
            "kind": self.kind.id(),
            "url": self.url,
            "params": params,
            "columns": columns,
            "least_rows": self.least_rows,
        });
        if self.kind == Kind::Data {
            value["rows"] = json!(self.rows);
        }
        if let Some(sign_in) = &self.sign_in {
            value["sign_in"] = json!(sign_in);
        }
        value
    }
}

/// `text`, cut for a sentence that quotes it.
fn shown(text: &str) -> String {
    super::text(text, NAME_MOST)
}

/// A read's or a column's name: one line of text that shows, at most NAME_MOST characters. It's
/// what agents call it by and what the user sees, so it's refused rather than changed: nothing in
/// it is hidden, and the name kept is the one asked.
fn named(what: &str, name: &str) -> Result<String, Wrong> {
    use crate::agents::request::{blank, invisible};
    let count = name.chars().count();
    if name.trim().is_empty() {
        return wrong(format!("{what} is empty."));
    }
    if count > NAME_MOST {
        return wrong(format!(
            "{what} is {count} characters; it's at most {NAME_MOST}, as a tab's title reads."
        ));
    }
    let odd = name
        .chars()
        .find(|&c| invisible(c) || blank(c) || c.is_control() || (c.is_whitespace() && c != ' '));
    if let Some(c) = odd {
        return wrong(format!(
            "{what} has the character U+{:04X}, which doesn't show; use plain words.",
            c as u32
        ));
    }
    if name != name.trim() || name.contains("  ") {
        return wrong(format!("{what} has spaces at its ends or two together."));
    }
    Ok(name.to_string())
}

/// The site's origin, from an http(s) origin with nothing after it but an optional "/". A read's
/// site is one origin (scheme, host and port), as request.rs's and the core's reads are, never a
/// registrable domain; it's kept as the url crate writes an origin, which every check compares to.
fn site(site: &str) -> Result<String, Wrong> {
    let refused = || {
        wrong(format!(
            "site is an http(s) origin, such as https://example.com, not “{}”.",
            shown(site)
        ))
    };
    // request.rs's rule first: http(s) only, with a host and no user or password in it.
    if crate::agents::request::origin(site).is_none() {
        return refused();
    }
    let Ok(parsed) = url::Url::parse(site) else {
        return refused();
    };
    if parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !parsed.username().is_empty()
    {
        return refused();
    }
    Ok(parsed.origin().ascii_serialization())
}

/// A sign-in page's path: from the site's root, with no query, fragment, parameter, encoding or
/// dot segment, so it names one path on the read's own origin. It only tells a redirect there
/// from a broken read; ilo never reads it.
fn sign_in_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && path.len() <= URL_MOST
        && path.chars().all(|c| {
            c.is_ascii_graphic()
                && !matches!(
                    c,
                    '?' | '#' | '\\' | '%' | '{' | '}' | '"' | '<' | '>' | '`'
                )
        })
        && !path.split('/').any(|segment| matches!(segment, "." | ".."))
}

/// A parameter's name: a lower-case letter, then letters, digits or "_".
fn param_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && name.len() <= 32
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn params(params: &Map<String, Value>) -> Result<BTreeMap<String, Param>, Wrong> {
    if params.len() > PARAMS_MOST {
        return wrong(format!("A read has at most {PARAMS_MOST} parameters."));
    }
    let mut out = BTreeMap::new();
    for (name, param) in params {
        if !param_name(name) {
            return wrong(format!(
                "“{}” can't name a parameter: use a lower-case letter, then letters, digits or _.",
                shown(name)
            ));
        }
        let kind = param
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let param = match kind {
            "integer" => {
                let (Some(min), Some(max)) = (
                    param.get("min").and_then(Value::as_i64),
                    param.get("max").and_then(Value::as_i64),
                ) else {
                    return wrong(format!("{name} is an integer, so it has a min and a max."));
                };
                if min > max {
                    return wrong(format!("{name}'s min is more than its max."));
                }
                Param::Integer { min, max }
            }
            "one_of" => {
                let values: Vec<String> = (param.get("values").and_then(Value::as_array))
                    .into_iter()
                    .flatten()
                    .filter_map(|value| value.as_str().map(str::to_string))
                    .collect();
                if values.is_empty() || values.len() > VALUES_MOST {
                    return wrong(format!(
                        "{name} is one of a list, so it has values: from 1 to {VALUES_MOST} strings."
                    ));
                }
                for value in &values {
                    // A value is checked as a pattern's literal would be: it fills a path too.
                    if value.is_empty()
                        || value.len() > PATTERN_MOST
                        || !value.chars().all(unreserved)
                        || dots(value)
                    {
                        return wrong(format!(
                            "{name}'s value “{}” has a character a path can't hold as it is; use letters, digits, - _ ~ and .",
                            shown(value)
                        ));
                    }
                }
                Param::OneOf(values)
            }
            "text" => {
                let pattern = param
                    .get("pattern")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                safe_pattern(pattern).map_err(|why| Wrong(format!("{name}'s pattern: {why}")))?;
                Param::Text {
                    pattern: pattern.to_string(),
                }
            }
            _ => return wrong(format!("{name}'s type is integer, one_of or text.")),
        };
        out.insert(name.clone(), param);
    }
    Ok(out)
}

/// A character every part of an address holds as it is: RFC 3986's unreserved ones.
pub(super) fn unreserved(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~')
}

/// A value of nothing but dots, which an address reads as "this folder" or "the one above".
pub(super) fn dots(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c == '.')
}

/// Whether `pattern` is of the narrow kind a text parameter may have, or why not. It can name
/// only letters, digits and "- _ ~ .", in classes such as [a-z0-9-] and with counts such as
/// {1,40}, so nothing it matches can hold a path separator, a "%" that encodes one, a query or a
/// fragment, whatever the pattern says: "." for any character and a class starting "^" are
/// refused, as are ranges across kinds, such as "+-0", that would take "/" in.
pub fn safe_pattern(pattern: &str) -> Result<(), String> {
    if pattern.is_empty() {
        return Err("a text parameter needs a pattern, such as [a-z0-9-]{1,40}.".into());
    }
    if pattern.chars().count() > PATTERN_MOST {
        return Err(format!("it's longer than {PATTERN_MOST} characters."));
    }
    let chars: Vec<char> = pattern.chars().collect();
    let refused = |c: char| {
        Err(format!(
            "it may use letters, digits, - _ ~ and \\., classes such as [a-z0-9-], \\d, groups and counts such as {{1,40}}; “{c}” isn't one of them."
        ))
    };
    let mut at = 0;
    let mut atom = false;
    let mut depth = 0usize;
    while at < chars.len() {
        let c = chars[at];
        match c {
            c if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '~') => atom = true,
            '\\' => match chars.get(at + 1) {
                Some('.' | '-' | 'd') => {
                    at += 1;
                    atom = true;
                }
                _ => return refused('\\'),
            },
            '[' => {
                at = class(&chars, at + 1)?;
                atom = true;
            }
            '(' => {
                if chars.get(at + 1) == Some(&'?') {
                    return refused('?');
                }
                depth += 1;
                atom = false;
            }
            ')' => {
                if depth == 0 {
                    return Err("a group closes that never opened.".into());
                }
                depth -= 1;
                atom = true;
            }
            '|' => atom = false,
            '+' | '*' | '?' if atom => atom = false,
            '{' if atom => {
                let end = (chars[at..].iter().position(|&c| c == '}')).map(|end| at + end);
                let Some(end) = end else { return refused('{') };
                let count: String = chars[at + 1..end].iter().collect();
                let fits = |part: &str| !part.is_empty() && part.len() <= 3 && part.chars().all(|c| c.is_ascii_digit());
                let good = match count.split_once(',') {
                    None => fits(&count),
                    Some((least, most)) => fits(least) && (most.is_empty() || fits(most)),
                };
                if !good {
                    return Err(format!("“{{{count}}}” isn't a count such as {{1,40}}."));
                }
                at = end;
                atom = false;
            }
            '.' => return Err("“.” matches any character, “/” too; write \\. for a dot, or a class such as [a-z0-9.].".into()),
            c => return refused(c),
        }
        at += 1;
    }
    if depth != 0 {
        return Err("a group opens that never closes.".into());
    }
    let compiled = fancy_regex::RegexBuilder::new(&format!("^(?:{pattern})$"))
        .delegate_size_limit(1 << 16)
        .build();
    if compiled.is_err() {
        return Err("it isn't a pattern a read can use.".into());
    }
    Ok(())
}

/// Checks a class's members from `at`, past its "[", and gives where its "]" is.
fn class(chars: &[char], mut at: usize) -> Result<usize, String> {
    let start = at;
    if chars.get(at) == Some(&'^') {
        return Err("a class starting ^ takes every character it doesn't name, “/” too; name the ones it takes.".into());
    }
    let member = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '~' | '.');
    let kind = |c: char| match c {
        'a'..='z' => 1,
        'A'..='Z' => 2,
        '0'..='9' => 3,
        _ => 0,
    };
    while let Some(&c) = chars.get(at) {
        match c {
            ']' if at > start => return Ok(at),
            '\\' => match chars.get(at + 1) {
                Some('.' | '-' | 'd') => at += 1,
                _ => return Err("in a class, only \\., \\- and \\d are allowed after a \\.".into()),
            },
            '-' => {
                // A range between the members either side, both of one kind: a-z, A-Z or 0-9.
                let before = chars
                    .get(at.wrapping_sub(1))
                    .copied()
                    .filter(|_| at > start);
                let after = chars.get(at + 1).copied().filter(|&c| c != ']');
                if let (Some(from), Some(to)) = (before, after) {
                    if kind(from) == 0 || kind(from) != kind(to) || from > to {
                        return Err(format!(
                            "the range {from}-{to} isn't one of a-z, A-Z or 0-9, or a part of one."
                        ));
                    }
                    at += 1;
                }
            }
            c if member(c) => {}
            c => {
                return Err(format!(
                    "a class may hold letters, digits, - _ ~ and .; “{c}” isn't one of them."
                ));
            }
        }
        at += 1;
    }
    Err("a class opens that never closes.".into())
}

/// Checks the address template against its parameters: a path from the site's root, with a
/// query of its own keys, a {name} for each parameter, each used once, in a path segment or a
/// query value.
fn template(url: &str, params: &BTreeMap<String, Param>) -> Result<(), Wrong> {
    if url.len() > URL_MOST {
        return wrong(format!("url is longer than {URL_MOST} bytes."));
    }
    if !url.starts_with('/') || url.starts_with("//") {
        return wrong(
            "url is a path on the site from its root, such as /api/items/{id}, starting with a single /.",
        );
    }
    if let Some(c) = url.chars().find(|&c| {
        !c.is_ascii_graphic() || matches!(c, '#' | '\\' | '"' | '<' | '>' | '`' | '^' | '|')
    }) {
        return wrong(format!(
            "url has “{}”, which an address can't hold as it is.",
            c.escape_default()
        ));
    }
    let (path, query) = url.split_once('?').unwrap_or((url, ""));
    if query.contains('?') {
        return wrong("url has more than one ?.");
    }
    let mut used = Vec::new();
    let mut names = |part: &str, what: &str| -> Result<(), Wrong> {
        for piece in pieces(part).map_err(|why| Wrong(format!("url: {why}")))? {
            if let Piece::Param(name) = piece {
                if !params.contains_key(name) {
                    return wrong(format!(
                        "url names {{{name}}}, which isn't one of its params."
                    ));
                }
                if used.contains(&name.to_string()) {
                    return wrong(format!("url names {{{name}}} twice."));
                }
                used.push(name.to_string());
            } else if let Piece::Literal(text) = piece {
                if what == "key" && text.is_empty() {
                    return wrong("url's query has a key that's empty.");
                }
            }
        }
        Ok(())
    };
    for segment in path.split('/').skip(1) {
        if matches!(segment, "." | "..") {
            return wrong("url's path has a . or .. segment.");
        }
        names(segment, "segment")?;
    }
    if !query.is_empty() {
        let mut keys = Vec::new();
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            if key.contains('{') || key.contains('}') {
                return wrong("url's query takes a parameter only as a value, as in ?page={page}.");
            }
            names(key, "key")?;
            if keys.contains(&key) {
                return wrong(format!("url's query has the key {key} twice."));
            }
            keys.push(key);
            names(value, "value")?;
        }
    }
    if let Some(unused) = params.keys().find(|name| !used.contains(name)) {
        return wrong(format!(
            "{unused} isn't in url: each parameter fills a {{{unused}}} in it."
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Piece<'a> {
    Literal(&'a str),
    Param(&'a str),
}

/// A part of the template, as its literals and its {name}s.
pub(super) fn pieces(part: &str) -> Result<Vec<Piece<'_>>, String> {
    let mut out = Vec::new();
    let mut rest = part;
    while let Some(open) = rest.find(['{', '}']) {
        if rest.as_bytes()[open] == b'}' {
            return Err("a } closes that never opened.".into());
        }
        let Some(close) = rest[open..].find('}').map(|close| open + close) else {
            return Err("a { opens that never closes.".into());
        };
        let name = &rest[open + 1..close];
        if !param_name(name) {
            return Err(format!("{{{name}}} isn't a parameter's name."));
        }
        if open > 0 {
            out.push(Piece::Literal(&rest[..open]));
        }
        out.push(Piece::Param(name));
        rest = &rest[close + 1..];
    }
    if !rest.is_empty() || out.is_empty() {
        out.push(Piece::Literal(rest));
    }
    Ok(out)
}

/// A field path: keys joined by ".", each a name or a list's index.
fn path(what: &str, path: &str) -> Result<String, Wrong> {
    if path.chars().count() > PATTERN_MOST {
        return wrong(format!("{what} is longer than {PATTERN_MOST} characters."));
    }
    if !path.is_empty() && path.split('.').any(str::is_empty) {
        return wrong(format!(
            "{what} is keys joined by a ., such as data.items, with none empty."
        ));
    }
    Ok(path.to_string())
}

fn columns(columns: &[Value], kind: Kind) -> Result<Vec<Column>, Wrong> {
    if columns.is_empty() || columns.len() > COLUMNS_MOST {
        return wrong(format!("A read has from 1 to {COLUMNS_MOST} columns."));
    }
    let mut out: Vec<Column> = Vec::new();
    for column in columns {
        let text = |key: &str| column.get(key).and_then(Value::as_str);
        let Some(name) = text("name") else {
            return wrong("Each column has a name.");
        };
        let name = named("A column's name", name)?;
        if out
            .iter()
            .any(|other| other.name.eq_ignore_ascii_case(&name))
        {
            return wrong(format!("Two columns are named {name}."));
        }
        let kind_of = match text("type").unwrap_or("text") {
            "text" => Type::Text,
            "number" => Type::Number,
            "address" => Type::Address,
            other => {
                return wrong(format!(
                    "{name}'s type is text, number or address, not “{}”.",
                    shown(other)
                ));
            }
        };
        let from = text("from").unwrap_or(&name).to_string();
        let from = match kind {
            Kind::Data => path(&format!("{name}'s from"), &from)?,
            Kind::Table if from.trim().is_empty() || from.chars().count() > PATTERN_MOST => {
                return wrong(format!(
                    "{name}'s from is its header's words, at most {PATTERN_MOST} characters."
                ));
            }
            Kind::Table => from,
        };
        out.push(Column {
            name,
            kind: kind_of,
            from,
        });
    }
    Ok(out)
}
