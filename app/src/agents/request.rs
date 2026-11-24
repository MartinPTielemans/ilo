//! The request tool (#1439): an agent's HTTP requests from a tab's page to that page's own site,
//! as the page's own script could send them, with its cookies, which the browser adds and never
//! shows. A read goes as asked. Every change asks the user, every time, with no Always: an API
//! call does whatever its site makes it do, and no name tells what, as a button's does. What comes
//! back is the body alone, without its headers, a page as its outline rather than its source, and
//! with values that look like sign-in tokens or secrets hidden, by their names or by their look,
//! so an agent gets the user's session, never a credential to carry off.

use serde_json::Value;

use super::page::clip;

/// The requests one call sends at most.
pub const MAX: usize = 20;
/// The most a request's body may be, in bytes.
pub const MAX_BODY: usize = 1 << 20;
/// The most of a `what` the tool takes, once it's plain text: a sentence, not a page.
pub const WHAT_MOST: usize = 500;
/// The most of a `what` the question's card shows, ended at a word: one plain sentence naming the
/// thing and the change fits, and at the sidebar's least width Deny and Allow stay in view.
pub const WHAT_SHOWN: usize = 120;
/// The characters of an answer's body the agent reads at most, and of all a call's together, which
/// leaves room under the answer's cap (`tools::MAX_ANSWER`) for each request's line.
const SHOWN: usize = 10_000;
pub const ALL_SHOWN: usize = 11_000;
/// The headers that name a method for a server to take in the sent one's place.
const OVERRIDES: [&str; 3] = [
    "x-http-method-override",
    "x-http-method",
    "x-method-override",
];
/// What stands for a value that's hidden.
const HIDDEN: &str = "(hidden)";
/// What stands for an empty value in what a request sent, as an empty one is an object's name.
const EMPTY: &str = "\"\"";

/// A request, as checked against the page and ready for the core (bc_agent_request).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// As sent, in capitals.
    pub method: String,
    /// The whole address, on the page's origin.
    pub url: String,
    /// Its path and query.
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    /// The agent's own sentence for what it does, as plain text (`plain`), or "" for a read.
    pub what: String,
}

/// What a change does, as the browser knows it from the method and its overrides, word for word:
/// it never reads a meaning into the path or the body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Does {
    Sends,
    Changes,
    Deletes,
}

impl Does {
    /// The card's facts line for one change: "Changes something on tracker.example".
    pub fn facts(self, site: &str) -> String {
        match self {
            Does::Sends => format!("Sends data to {site}"),
            Does::Changes => format!("Changes something on {site}"),
            Does::Deletes => format!("Deletes something on {site}"),
        }
    }

    /// The facts of one of several changes, whose site the card names once.
    pub fn short_facts(self) -> &'static str {
        match self {
            Does::Sends => "Sends data",
            Does::Changes => "Changes something",
            Does::Deletes => "Deletes something",
        }
    }

    /// What a task's card says it waits on: "a change".
    pub fn noun(self) -> &'static str {
        match self {
            Does::Sends => "data to send",
            Does::Changes => "a change",
            Does::Deletes => "a delete",
        }
    }
}

impl Request {
    /// The methods a server may take it as, which the card, its head and whether it asks all read:
    /// the one sent, then each piece of each override header's value, split at its commas, as the
    /// browser splits one it refuses (`forbidden_header`), trimmed and in capitals.
    fn says(&self) -> Vec<String> {
        let overrides = (self.headers.iter())
            .filter(|(name, _)| OVERRIDES.iter().any(|each| name.eq_ignore_ascii_case(each)))
            .flat_map(|(_, value)| value.split(','))
            .map(|piece| piece.trim().to_ascii_uppercase());
        std::iter::once(self.method.clone())
            .chain(overrides)
            .collect()
    }

    /// What it does, the strongest of what it says: it deletes if any says DELETE, sends only
    /// when all say POST, and changes something for anything else, a method the browser doesn't
    /// know or an empty override included.
    pub fn kind(&self) -> Does {
        let says = self.says();
        match () {
            _ if says.iter().any(|each| each == "DELETE") => Does::Deletes,
            _ if says.iter().all(|each| each == "POST") => Does::Sends,
            _ => Does::Changes,
        }
    }

    /// Whether it may change anything: anything but a GET or HEAD whose overrides, if it sets any,
    /// all say GET or HEAD too.
    pub fn changes(&self) -> bool {
        !(self.says().iter()).all(|each| matches!(each.as_str(), "GET" | "HEAD"))
    }

    /// What it is, for the log and the answer: "POST /api/items".
    pub fn target(&self) -> String {
        format!("{} {}", self.methods(), clip(&self.path, 60))
    }

    /// The method sent, then every other an override names, once each: "POST as GET, DELETE". A
    /// piece that isn't a method's name isn't named, though `kind` and `changes` count it.
    pub fn methods(&self) -> String {
        let mut named: Vec<String> = Vec::new();
        for each in self.says().into_iter().skip(1) {
            if each != self.method && is_token(&each) && !named.contains(&each) {
                named.push(each);
            }
        }
        match named.is_empty() {
            true => self.method.clone(),
            false => format!("{} as {}", self.method, named.join(", ")),
        }
    }

    /// What its body sends, a line a field, for the activity log (#2326).
    pub fn sent(&self) -> Vec<Sent> {
        let kind = (self.headers.iter())
            .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .map_or("", |(_, value)| value.as_str());
        sent(&self.body, kind)
    }
}

/// A line of what a request sent, as the activity log shows it: how deep it sits, a field's name,
/// an object's or an item's place, and its value, empty under an object's name. A secret's value is
/// "(hidden)", by the rule that hides them from an agent's answers, so the log never keeps one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sent {
    pub depth: u8,
    pub name: String,
    pub value: String,
}

impl Sent {
    pub fn hidden(&self) -> bool {
        self.value == HIDDEN
    }
}

/// The most lines of what a request sent the log keeps, and characters of a value: a body may be a
/// megabyte, and the log holds a thousand entries.
const SENT_LINES: usize = 200;
const SENT_VALUE: usize = 2000;

/// `body` a line a field: a form's fields, a JSON object's, nested ones under their object's name,
/// or the lines of any other text.
pub fn sent(body: &str, kind: &str) -> Vec<Sent> {
    let body = body.trim();
    let mut lines = Vec::new();
    if body.is_empty() {
        return lines;
    }
    let line = |depth: u8, name: &str, value: &str| Sent {
        depth,
        name: clip(name, 64),
        value: clip(value, SENT_VALUE),
    };
    if kind.to_ascii_lowercase().contains("x-www-form-urlencoded") {
        for pair in body.split('&').filter(|pair| !pair.is_empty()) {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            let (name, value) = (form_decoded(name), form_decoded(value));
            let secret = !value.is_empty() && (secret_name(&name) || token_like(&value));
            let value = match (secret, value.is_empty()) {
                (true, _) => HIDDEN,
                (false, true) => EMPTY,
                (false, false) => &value,
            };
            lines.push(line(0, &name, value));
        }
    } else if let Ok(value @ (Value::Object(_) | Value::Array(_))) =
        serde_json::from_str::<Value>(body)
    {
        json_lines(&mut lines, None, &value, 0, false);
    } else {
        let (text, _) = hide_text(body);
        let shown = text
            .lines()
            .map(str::trim_end)
            .filter(|each| !each.is_empty());
        lines.extend(shown.map(|each| line(0, "", each)));
    }
    if lines.len() > SENT_LINES {
        lines.truncate(SENT_LINES - 1);
        lines.push(line(0, "", "…"));
    }
    lines
}

/// `value`'s lines, under `name` at `depth`: a scalar or a list of them on its line, an object's
/// name over its fields a step deeper, and a list's items by their place, one whose fields are all
/// scalars on a line of its own.
fn json_lines(lines: &mut Vec<Sent>, name: Option<&str>, value: &Value, depth: u8, secret: bool) {
    if lines.len() > SENT_LINES {
        return;
    }
    let secret = secret || name.is_some_and(secret_name);
    let push = |lines: &mut Vec<Sent>, value: &str| {
        lines.push(Sent {
            depth,
            name: clip(name.unwrap_or(""), 64),
            value: clip(value, SENT_VALUE),
        })
    };
    let deeper = depth.saturating_add(1).min(8);
    match value {
        Value::Object(fields) if fields.is_empty() => push(lines, "{}"),
        Value::Array(items) if items.is_empty() => push(lines, "[]"),
        Value::Array(items) if items.iter().all(|item| scalar(item).is_some()) => {
            let shown: Vec<String> = (items.iter())
                .map(|item| shown_scalar(item, secret))
                .collect();
            push(lines, &shown.join(", "));
        }
        Value::Object(fields) => {
            // At the top, an object's fields are the body's own lines.
            let depth = match name {
                Some(_) => {
                    push(lines, "");
                    deeper
                }
                None => depth,
            };
            for (field, item) in fields {
                json_lines(lines, Some(field), item, depth, secret);
            }
        }
        Value::Array(items) => {
            let depth = match name {
                Some(_) => {
                    push(lines, "");
                    deeper
                }
                None => depth,
            };
            for (at, item) in items.iter().enumerate() {
                let place = (at + 1).to_string();
                let flat = item.as_object().filter(|fields| {
                    !fields.is_empty() && fields.values().all(|each| scalar(each).is_some())
                });
                match flat {
                    Some(fields) => {
                        let shown: Vec<String> = (fields.iter())
                            .map(|(field, each)| {
                                format!(
                                    "{field}: {}",
                                    shown_scalar(each, secret || secret_name(field))
                                )
                            })
                            .collect();
                        lines.push(Sent {
                            depth,
                            name: place,
                            value: clip(&shown.join(" · "), SENT_VALUE),
                        });
                    }
                    None => json_lines(lines, Some(&place), item, depth, secret),
                }
            }
        }
        scalar => push(lines, &shown_scalar(scalar, secret)),
    }
}

/// A JSON scalar as its line shows it, a string without its quotes, or None for a list or object.
fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(_) | Value::Object(_) => None,
        value => Some(value.to_string()),
    }
}

/// A scalar as shown, "(hidden)" if it's a secret's or looks like a credential.
fn shown_scalar(value: &Value, secret: bool) -> String {
    let text = scalar(value).unwrap_or_default();
    let blank = value.is_null() || text.is_empty();
    match (blank, secret || token_like(&text)) {
        (false, true) => HIDDEN.into(),
        (true, _) if !value.is_null() => EMPTY.into(),
        _ => text,
    }
}

/// A form's name or value as typed: "+" a space, and "%xx" its byte.
fn form_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = |at: usize| bytes.get(at).and_then(|byte| (*byte as char).to_digit(16));
        match (bytes[at], hex(at + 1), hex(at + 2)) {
            (b'+', _, _) => decoded.push(b' '),
            (b'%', Some(high), Some(low)) => {
                decoded.push((high * 16 + low) as u8);
                at += 2;
            }
            (byte, _, _) => decoded.push(byte),
        }
        at += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// Why a call's requests aren't sent: a rule's refusal, which the activity log has, or arguments
/// that aren't right, which only the agent reads.
#[derive(Debug, PartialEq)]
pub enum Wrong {
    Refused(String),
    Malformed(String),
}

/// The call's requests, checked against the page at `page`, the tab's address.
pub fn parse(arguments: &Value, page: &str) -> Result<Vec<Request>, Wrong> {
    // Never what the page's address is: an agent may name a tab that isn't its own.
    let origin = origin(page).ok_or_else(|| {
        let why = match page.is_empty() || page == "about:blank" {
            true => {
                "The tab has no page loaded yet, so no site to send requests to. Go to the \
                     site's page with navigate first."
            }
            false => {
                "The tab shows no site's page, so no site to send requests to. Go to the \
                      site's page with navigate first."
            }
        };
        Wrong::Malformed(why.into())
    })?;
    let asked = arguments.get("requests").and_then(Value::as_array);
    let asked = asked.filter(|asked| (1..=MAX).contains(&asked.len()));
    let asked = asked.ok_or_else(|| {
        Wrong::Malformed(format!(
            "requests is 1 to {MAX} requests, each with a method and a path."
        ))
    })?;
    (asked.iter().enumerate())
        .map(|(at, asked)| one(asked, &origin, at + 1))
        .collect()
}

fn one(asked: &Value, origin: &str, number: usize) -> Result<Request, Wrong> {
    let malformed = |why: &str| Err(Wrong::Malformed(why.into()));
    let text = |name: &str| asked.get(name).and_then(Value::as_str).map(str::trim);
    let method = text("method").unwrap_or_default().to_ascii_uppercase();
    if !is_token(&method) {
        return malformed("method is an HTTP method: GET, POST, PUT, PATCH, DELETE or the like.");
    }
    if forbidden_method(&method) {
        return Err(Wrong::Refused(format!(
            "Agents can't send {method} requests, as pages can't."
        )));
    }
    let Some(path) = text("path") else {
        return malformed(PATH);
    };
    let (url, path) = resolve(origin, path)?;
    let mut headers = Vec::new();
    match asked.get("headers") {
        None | Some(Value::Null) => {}
        Some(Value::Object(asked)) => {
            for (name, value) in asked {
                let Some(value) = value.as_str() else {
                    return malformed("Each header's value is a string.");
                };
                let (name, value) = (name.trim(), value.trim());
                if !is_token(name) || value.contains(['\r', '\n', '\0']) {
                    return Err(Wrong::Malformed(format!(
                        "The header \"{name}\" isn't a valid one."
                    )));
                }
                if let Some(why) = forbidden_header(name, value) {
                    return Err(Wrong::Refused(why));
                }
                headers.push((name.to_string(), value.to_string()));
            }
        }
        Some(_) => return malformed("headers is an object of header names and their values."),
    }
    let has = |name: &str| {
        headers
            .iter()
            .any(|(each, _)| each.eq_ignore_ascii_case(name))
    };
    let (body, kind) = match asked.get("body") {
        None | Some(Value::Null) => (String::new(), ""),
        Some(Value::String(body)) => (body.clone(), "text/plain;charset=UTF-8"),
        Some(json) => (json.to_string(), "application/json"),
    };
    if !body.is_empty() && matches!(method.as_str(), "GET" | "HEAD") {
        return Err(Wrong::Malformed(format!("A {method} request has no body.")));
    }
    if body.len() > MAX_BODY {
        return malformed("A body is 1 MB at most.");
    }
    if body.contains('\0') {
        return malformed("A body is text, without NUL characters.");
    }
    // As the page's fetch types its body: JSON as JSON, a string as text, unless it says.
    if !body.is_empty() && !has("content-type") {
        headers.push(("Content-Type".into(), kind.into()));
    }
    let mut request = Request {
        method,
        url,
        path,
        headers,
        body,
        what: String::new(),
    };
    // The user reads the agent's own words for a change before they allow it, so a change has them;
    // a read never asks, and keeps none. A change is what `changes` says, the method and every
    // override together, so one whose override says GET has them too.
    let what = match asked.get("what") {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(what)) => plain(what),
        Some(_) => return malformed("`what` is a string: one plain sentence."),
    };
    match request.changes() {
        false => {}
        true if what.is_empty() => {
            return Err(Wrong::Malformed(format!(
                "Request {number} ({} {}) may change something, so it needs a `what`: one plain \
                 sentence saying what it will do, for the user to read before they allow it. \
                 Nothing was sent.",
                request.methods(),
                short(&request.path)
            )));
        }
        true if what.chars().count() > WHAT_MOST => {
            return Err(Wrong::Malformed(format!(
                "Request {number}'s `what` is {} characters; give one plain sentence of {WHAT_MOST} \
                 at most. Nothing was sent.",
                what.chars().count()
            )));
        }
        true => request.what = what,
    }
    Ok(request)
}

/// Characters drawn as nothing: direction overrides and marks, joiners, variation selectors, tags
/// and other format characters.
pub(crate) fn invisible(c: char) -> bool {
    matches!(c,
        '\u{AD}' | '\u{34F}' | '\u{61C}' | '\u{115F}' | '\u{1160}' | '\u{17B4}' | '\u{17B5}'
        | '\u{180B}'..='\u{180F}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{206F}' | '\u{3164}' | '\u{FE00}'..='\u{FE0F}' | '\u{FEFF}'
        | '\u{FFA0}' | '\u{FFF0}'..='\u{FFFB}' | '\u{1BCA0}'..='\u{1BCA3}'
        | '\u{1D173}'..='\u{1D17A}' | '\u{E0000}'..='\u{E0FFF}')
}

/// Glyphs drawn blank that aren't spaces, such as the Braille blank.
pub(crate) fn blank(c: char) -> bool {
    matches!(c, '\u{2800}' | '\u{1D159}' | '\u{16FE4}')
}

/// What joins or styles the characters beside it, as in an emoji family or a Persian word: the
/// joiners, and the variation selector that asks for an emoji's colour.
fn joins(c: char) -> bool {
    matches!(c, '\u{200C}' | '\u{200D}' | '\u{FE0F}')
}

/// A single quotation mark or apostrophe, of which a run reads as a double one.
fn single_quote(c: char) -> bool {
    matches!(
        c,
        '\'' | '`'
            | '\u{2BC}'
            | '\u{2018}'
            | '\u{2019}'
            | '\u{201A}'
            | '\u{201B}'
            | '\u{2032}'
            | '\u{FF07}'
    )
}

/// An agent's sentence as plain text in one paragraph, wherever it's shown, so it can't lay out
/// lines that read as the browser's own under its quote, or close the quote: line breaks, tabs,
/// other control characters and blank glyphs become a space, double quotation marks and runs of
/// single ones become one single mark, direction overrides and invisible format characters go,
/// and runs of spaces become one. A joiner stays between two characters that show, and an emoji's
/// colour selector after one, as they make what's drawn there and hide nothing.
pub fn plain(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let shows = |c: char| !(invisible(c) || blank(c) || c.is_control() || c.is_whitespace());
    let mut spaced = String::with_capacity(text.len());
    let mut last = None;
    for (at, &c) in chars.iter().enumerate() {
        if invisible(c) {
            // A joiner after an emoji's colour selector joins the emoji it's on: the rainbow flag.
            let before = last.is_some_and(|last| match c {
                '\u{FE0F}' => shows(last),
                _ => shows(last) || last == '\u{FE0F}',
            });
            let after = chars.get(at + 1).is_some_and(|next| shows(*next));
            let kept = match c {
                '\u{FE0F}' => before,
                _ => joins(c) && before && after,
            };
            if kept {
                spaced.push(c);
                last = Some(c);
            }
            continue;
        }
        let c = match c {
            _ if c.is_control() || c.is_whitespace() || blank(c) => ' ',
            '"'
            | '\u{2BA}'
            | '\u{2DD}'
            | '\u{2EE}'
            | '\u{5F4}'
            | '\u{201C}'
            | '\u{201D}'
            | '\u{201F}'
            | '\u{2033}'
            | '\u{2036}'
            | '\u{275D}'
            | '\u{275E}'
            | '\u{3003}'
            | '\u{301D}'..='\u{301F}'
            | '\u{FF02}' => '\'',
            '\u{201E}' => '\u{201A}',
            '\u{AB}' => '\u{2039}',
            '\u{BB}' => '\u{203A}',
            '\u{300E}' => '\u{300C}',
            '\u{300F}' => '\u{300D}',
            _ => c,
        };
        if single_quote(c) && last.is_some_and(single_quote) {
            continue;
        }
        spaced.push(c);
        last = Some(c);
    }
    spaced
        .split(' ')
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Text an agent wrote as the whole request shows it, exactly, with what wouldn't show or would
/// move what's around it written out instead, as `\u{202E}`: control characters, invisible and
/// format characters, blank glyphs and every space but the plain one. A line break stays one
/// where `lines` is set, as a body's are its own. A backslash before "u{" is written out too, as
/// `\u{005C}`, so every `\u{…}` that isn't one is the browser's, and marked. Gives the text and
/// where each escape is in it.
pub fn escaped(text: &str, lines: bool) -> (String, Vec<std::ops::Range<usize>>) {
    let mut shown = String::with_capacity(text.len());
    let mut marks = Vec::new();
    for (at, c) in text.char_indices() {
        let written = (c.is_control() && !(lines && c == '\n'))
            || invisible(c)
            || blank(c)
            || (c.is_whitespace() && c != ' ' && c != '\n')
            || (c == '\\' && text[at + 1..].starts_with("u{"));
        if written {
            let start = shown.len();
            shown.push_str(&format!("\\u{{{:04X}}}", u32::from(c)));
            marks.push(start..shown.len());
        } else {
            shown.push(c);
        }
    }
    (shown, marks)
}

/// A sentence as the question's card shows it: whole up to WHAT_SHOWN, else cut with "…", at a
/// word if one ends in the last quarter, else at the limit, so a sentence in a script without
/// spaces, or with one near its start, keeps the card's width of it.
pub fn shown(what: &str) -> String {
    if what.chars().count() <= WHAT_SHOWN {
        return what.to_string();
    }
    let cut: Vec<char> = what.chars().take(WHAT_SHOWN - 1).collect();
    let end = (cut.iter().rposition(|c| *c == ' '))
        .filter(|end| *end >= WHAT_SHOWN * 3 / 4)
        .unwrap_or(cut.len());
    let kept: String = cut[..end].iter().collect();
    format!("{}…", kept.trim_end_matches([',', ';', ':', '、', ' ']))
}

/// The origin of an http(s) address, as "https://host[:port]" in lower case without a default
/// port, or None for any other.
pub(crate) fn origin(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let host = &rest[..rest.find(['/', '?', '#', '\\']).unwrap_or(rest.len())];
    if host.is_empty() || host.contains('@') {
        return None;
    }
    let host = host.to_ascii_lowercase();
    let default = if scheme == "https" { ":443" } else { ":80" };
    let host = host.strip_suffix(default).unwrap_or(&host);
    Some(format!("{scheme}://{host}"))
}

/// What a path is, for an agent that gave none.
const PATH: &str = "path is a path on the tab's site, such as /api/items?q=red, or a whole address \
                    on it.";

/// The whole address and the path of `path` on `origin`: a path from its root, or a whole address
/// on that origin and no other. A space in it is %20, as the address bar makes it; any other
/// whitespace or control character is refused by name and place, as a server may read it either
/// way (#3661).
fn resolve(origin: &str, path: &str) -> Result<(String, String), Wrong> {
    let malformed = |why: String| Err(Wrong::Malformed(why));
    if path.is_empty() {
        return malformed(PATH.into());
    }
    let odd = (path.chars().enumerate())
        .find(|(_, char)| *char != ' ' && (char.is_whitespace() || char.is_control()));
    if let Some((at, char)) = odd {
        let name = match char {
            '\t' => "a tab".to_string(),
            '\n' => "a line break".to_string(),
            '\r' => "a carriage return".to_string(),
            char => format!("the character U+{:04X}", char as u32),
        };
        let mut bytes = [0; 4];
        let encoded: String = (char.encode_utf8(&mut bytes).bytes())
            .map(|byte| format!("%{byte:02X}"))
            .collect();
        let before: String = path.chars().take(at).collect();
        return malformed(format!(
            "path is a path on the tab's site, without {name}: there's one at character {}, \
             after “{before}”. Encode it as {encoded} to send it.",
            at + 1
        ));
    }
    let path = path.replace(' ', "%20");
    let path = path.split('#').next().unwrap_or_default();
    let lower = path.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        if self::origin(path).as_deref() != Some(origin) {
            return Err(Wrong::Refused(
                "Agents send requests only to the tab's own site, as its page could.".into(),
            ));
        }
        let rest = &path[path.find("://").map_or(0, |at| at + 3)..];
        let at = rest.find(['/', '?', '\\']).unwrap_or(rest.len());
        let shown = match &rest[at..] {
            "" => "/".to_string(),
            query if query.starts_with('?') => format!("/{query}"),
            shown => shown.to_string(),
        };
        return Ok((format!("{origin}{shown}"), shown));
    }
    // "//host" and "/\host" name another host, as a browser reads them.
    if path.starts_with("//") || path.starts_with("/\\") {
        return malformed(format!(
            "path is a path on the tab's site, from its root: one starting with {} is read as \
             another site's host. Start it with a single /.",
            &path[..2]
        ));
    }
    if !path.starts_with('/') {
        return malformed(format!(
            "path is a path on the tab's site, from its root, as /{path} is, or a whole address \
             on it."
        ));
    }
    Ok((format!("{origin}{path}"), path.to_string()))
}

/// A token, as HTTP's methods and header names are.
fn is_token(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

fn forbidden_method(method: &str) -> bool {
    matches!(method, "CONNECT" | "TRACE" | "TRACK")
}

/// Why an agent can't set the header, if it can't: one the browser sets, as a page's fetch can't
/// set it either (Chromium's net::HttpUtil::IsSafeHeader, which the core checks too), or a
/// sign-in of the agent's own.
fn forbidden_header(name: &str, value: &str) -> Option<String> {
    const SET_BY_THE_BROWSER: [&str; 22] = [
        "accept-charset",
        "accept-encoding",
        "access-control-request-headers",
        "access-control-request-method",
        "connection",
        "content-length",
        "cookie",
        "cookie2",
        "date",
        "dnt",
        "expect",
        "host",
        "keep-alive",
        "origin",
        "referer",
        "set-cookie",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
        "user-agent",
        "via",
    ];
    let lower = name.to_ascii_lowercase();
    if lower == "authorization" {
        return Some(
            "Agents can't set the Authorization header: they act in the page's own session, and \
             never send credentials of their own."
                .into(),
        );
    }
    let overrides = [
        "x-http-method",
        "x-http-method-override",
        "x-method-override",
    ];
    let forbidden = SET_BY_THE_BROWSER.contains(&lower.as_str())
        || lower.starts_with("proxy-")
        || lower.starts_with("sec-")
        || overrides.contains(&lower.as_str())
            && value
                .split(',')
                .any(|method| forbidden_method(&method.trim().to_ascii_uppercase()));
    forbidden.then(|| {
        format!(
            "Agents can't set the {name} header: the browser sets it, as it does for the page's own \
             requests."
        )
    })
}

/// A path short enough for a line of the question: its first part and its last, around "…".
fn short(path: &str) -> String {
    const SHORT: usize = 32;
    if path.chars().count() <= SHORT {
        return path.into();
    }
    let (route, query) = path.split_once('?').unwrap_or((path, ""));
    let parts: Vec<&str> = route.split('/').filter(|part| !part.is_empty()).collect();
    let mut short = match parts.as_slice() {
        [first, _, .., last] => format!("/{first}/…/{last}"),
        _ => route.to_string(),
    };
    if !query.is_empty() {
        short.push_str("?…");
    }
    clip(&short, SHORT + 8)
}

/// The core's answer to a request (bc_agent_response).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Response {
    /// The HTTP status, or 0 if none came.
    pub status: u16,
    /// The body's MIME type, or "".
    pub kind: String,
    pub body: Vec<u8>,
    /// Whether the body is its first megabyte only.
    pub cut: bool,
    /// Why it couldn't be sent, or broke off.
    pub error: Option<String>,
}

/// What the agent reads of `sent`'s answer: its status and type, then its body if it's text, up
/// to what's `left` of the call's, without what looks like a secret, which counts in `hidden`.
pub fn answer(sent: &Request, response: &Response, left: &mut usize, hidden: &mut usize) -> String {
    let target = sent.target();
    if response.status == 0 {
        let why = response.error.as_deref().unwrap_or("No answer came.");
        return format!("{target}: {why}");
    }
    let mut answer = match response.kind.is_empty() {
        true => format!("{target} → {}", response.status),
        false => format!("{target} → {} {}", response.status, response.kind),
    };
    if let Some(why) = &response.error {
        answer.push_str(&format!("\n{why}"));
    }
    if response.body.is_empty() {
        return answer;
    }
    let size = response.body.len();
    let text = match is_text(&response.kind) {
        true => String::from_utf8(response.body.clone()).ok(),
        false => None,
    };
    let Some(text) = text else {
        let kind = if response.kind.is_empty() {
            "data"
        } else {
            &response.kind
        };
        answer.push_str(&format!("\n({size} bytes of {kind}, not shown)"));
        return answer;
    };
    if *left == 0 {
        answer.push_str(
            "\n(Its body isn't shown: this call's answers already showed as much as one may. Send \
             fewer requests at a time.)",
        );
        return answer;
    }
    // A page comes as its outline, as the agent reads a tab's, so nothing of its source that the
    // user doesn't see, such as a form's hidden fields or a meta tag, reaches the agent.
    let page = is_html(&response.kind, &text);
    let (text, withheld) = match page {
        true => outline(&text),
        false => (text, 0),
    };
    let (text, hid) = hide_secrets(&text, response.kind.contains("json"));
    *hidden += hid;
    if page {
        answer.push_str(&format!("\n{}", outlined(withheld + hid)));
    }
    let chars = text.chars().count();
    let shown = chars.min(SHOWN).min(*left);
    *left -= shown;
    let end = text
        .char_indices()
        .nth(shown)
        .map_or(text.len(), |(end, _)| end);
    answer.push_str("\n\n");
    answer.push_str(&text[..end]);
    if shown < chars {
        answer.push_str(&format!(
            "\n… (the rest isn't shown: it's {chars} characters in all)"
        ));
    } else if response.cut {
        answer.push_str("\n… (the answer goes on past its first megabyte, which is all that came)");
    }
    answer
}

/// Whether a body of this type is text an agent can read: text, JSON, XML, script or a form, or
/// of no type said.
fn is_text(kind: &str) -> bool {
    let kind = kind.to_ascii_lowercase();
    kind.is_empty()
        || kind.starts_with("text/")
        || ["json", "xml", "javascript", "x-www-form-urlencoded"]
            .iter()
            .any(|each| kind.contains(each))
}

/// Whether an answer is a page: of an HTML type, or of none with markup that says it's one.
fn is_html(kind: &str, text: &str) -> bool {
    let kind = kind.to_ascii_lowercase();
    if !kind.is_empty() {
        return kind.contains("html");
    }
    let start = text.trim_start_matches(['\u{feff}', ' ', '\t', '\r', '\n']);
    let head = start.get(..14).unwrap_or(start).to_ascii_lowercase();
    head.starts_with("<!doctype html") || head.starts_with("<html")
}

/// The elements whose content isn't a page's text: what runs, styles or draws it, what shows when
/// one doesn't, and a text field's value.
const UNREAD: [&str; 8] = [
    "script", "style", "template", "noscript", "svg", "iframe", "object", "textarea",
];

/// The elements a page's text breaks a line at.
const BLOCKS: &str = "address article aside blockquote br caption dd details div dl dt fieldset \
    figcaption figure footer form h1 h2 h3 h4 h5 h6 header hr li main nav ol p section table tr";

fn is_block(name: &str) -> bool {
    BLOCKS.split_whitespace().any(|block| block == name)
}

#[cfg(test)]
thread_local! {
    /// The bytes the reader's scans have read on this thread, and how many its test allows.
    static SCANNED: std::cell::Cell<(usize, usize)> =
        const { std::cell::Cell::new((0, usize::MAX)) };
}

/// Counts `bytes` read by one of the reader's scans, in tests, which stop it once it's past what
/// they allow: each scan stops at its own end, so a body is read a few times over in all rather
/// than again from each byte.
fn scanned(bytes: usize) {
    #[cfg(test)]
    SCANNED.with(|scanned| {
        let (read, most) = scanned.get();
        let read = read.saturating_add(bytes);
        scanned.set((read, most));
        assert!(
            read <= most,
            "the reader read {read} bytes, past the {most} allowed"
        );
    });
    #[cfg(not(test))]
    let _ = bytes;
}

/// A page as an agent reads it in its outline rather than its source: its text, a link's address,
/// an image's description, a form's method and address, and its fields' names and buttons, but
/// no script, style, comment or meta tag, and no field's value. The values it leaves out, a meta
/// tag's and a field's, are counted.
fn outline(html: &str) -> (String, usize) {
    let bytes = html.as_bytes();
    let (mut out, mut withheld) = (String::new(), 0);
    let mut link: Option<String> = None;
    let mut at = 0;
    let close_link = |out: &mut String, link: &mut Option<String>| {
        if let Some(href) = link.take() {
            out.push_str(&format!(" ({href})"));
        }
    };
    while at < bytes.len() {
        if bytes[at] != b'<' {
            let end = (at..bytes.len()).find(|&next| bytes[next] == b'<');
            let end = end.unwrap_or(bytes.len());
            scanned(end - at);
            out.push_str(&decoded(&html[at..end]));
            at = end;
            continue;
        }
        if html[at..].starts_with("<!--") {
            let from = at;
            at = html[at..]
                .find("-->")
                .map_or(bytes.len(), |end| at + end + 3);
            scanned(at - from);
            continue;
        }
        if matches!(bytes.get(at + 1), Some(b'!' | b'?')) {
            let from = at;
            at = html[at..].find('>').map_or(bytes.len(), |end| at + end + 1);
            scanned(at - from);
            continue;
        }
        let Some(tag) = tag(html, at) else {
            out.push('<');
            at += 1;
            continue;
        };
        // A value left open is the rest of the page, which is withheld as a field's is.
        if tag.open {
            withheld += 1;
            break;
        }
        at = tag.end;
        let value = |name: &str| {
            (tag.attributes.iter())
                .find(|(attribute, _)| attribute == name)
                .map(|(_, bounds)| decoded(&html[bounds.0..bounds.1]).trim().to_string())
                .filter(|value| !value.is_empty())
        };
        let field = value("name").or_else(|| value("id")).unwrap_or_default();
        let name = tag.name.as_str();
        if tag.closing {
            if name == "a" {
                close_link(&mut out, &mut link);
            }
            if is_block(name) {
                out.push('\n');
            }
            continue;
        }
        if UNREAD.contains(&name) {
            let end = (at..bytes.len()).find(|&next| {
                bytes[next..].starts_with(b"</")
                    && (bytes.get(next + 2..next + 2 + name.len()))
                        .is_some_and(|word| word.eq_ignore_ascii_case(name.as_bytes()))
            });
            let end = end.unwrap_or(bytes.len());
            scanned(end - at);
            if name == "textarea" {
                out.push_str(&format!(" [text field: {field}] "));
                withheld += usize::from(!html[at..end].trim().is_empty());
            }
            at = html[end..]
                .find('>')
                .map_or(bytes.len(), |close| end + close + 1);
            scanned(at - end);
            continue;
        }
        match name {
            "a" => {
                close_link(&mut out, &mut link);
                link = value("href");
            }
            "img" => {
                if let Some(alt) = value("alt") {
                    out.push_str(&format!(" [image: {alt}] "));
                }
            }
            "input" => {
                let kind = value("type").unwrap_or_else(|| "text".into());
                let kind = kind.to_ascii_lowercase();
                match kind.as_str() {
                    "submit" | "button" | "reset" => {
                        let label = value("value").unwrap_or(kind.clone());
                        out.push_str(&format!(" [button: {label}] "));
                    }
                    "image" => {
                        let label = value("alt").unwrap_or_default();
                        out.push_str(&format!(" [button: {label}] "));
                    }
                    _ => {
                        if kind != "hidden" {
                            out.push_str(&format!(" [{kind} field: {field}] "));
                        }
                        withheld += usize::from(value("value").is_some());
                    }
                }
            }
            "select" => out.push_str(&format!(" [choice: {field}] ")),
            "form" => {
                let method = value("method").unwrap_or_else(|| "get".into());
                let action = value("action").unwrap_or_default();
                out.push_str(&format!("\n[form: {} {action}]\n", method.to_uppercase()));
            }
            "meta" => withheld += usize::from(value("content").is_some()),
            _ if is_block(name) => out.push('\n'),
            _ => {}
        }
    }
    close_link(&mut out, &mut link);
    // Spaces as a page draws them: one between words, and no empty lines.
    let lines = (out.lines()).map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "));
    let lines: Vec<String> = lines.filter(|line| !line.is_empty()).collect();
    (lines.join("\n"), withheld)
}

/// Markup's text with its character references read: the named ones text has most, and numbers.
fn decoded(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        scanned(amp);
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        // An entity's name is short, so its `;` is looked for only that far, not to the end.
        let end = (rest.bytes().take(11)).position(|byte| byte == b';');
        let read = end.and_then(|end| {
            let name = &rest[1..end];
            let read = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => match name.strip_prefix('#') {
                    Some(hex) if hex.starts_with(['x', 'X']) => u32::from_str_radix(&hex[1..], 16)
                        .ok()
                        .and_then(char::from_u32),
                    Some(number) => number.parse().ok().and_then(char::from_u32),
                    None => None,
                },
            };
            read.map(|read| (read, end))
        });
        match read {
            Some((read, end)) => {
                out.push(read);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The line an answer that's a page starts with: that it's the page's outline, and how many of
/// its values were withheld.
fn outlined(withheld: usize) -> String {
    let said = "This is the page's outline, not its source";
    match withheld {
        0 => format!("{said}."),
        1 => format!("{said}: 1 value was withheld."),
        many => format!("{said}: {many} values were withheld."),
    }
}

/// `text` without the values that look like secrets, and how many there were: JSON's fields named
/// as a token, password, session's id or one-time code is, or a key or code whose value reads as
/// a secret, and the same in an address's query, markup and script; and anywhere a JWT or a
/// bearer credential.
pub fn hide_secrets(text: &str, json: bool) -> (String, usize) {
    if json && let Ok(mut value) = serde_json::from_str::<Value>(text) {
        let hidden = hide_json(&mut value, false);
        return match hidden {
            0 => (text.to_string(), 0),
            hidden => (value.to_string(), hidden),
        };
    }
    hide_text(text)
}

/// An agent's sentence as the log and Settings › Agents' activity keep it, in memory, without
/// what reads as a secret: what `hide_secrets` hides in text, and the word after a secret's name,
/// such as "API key" or "password:", and the first "to", "is", "=" or ":" within NEAR words of it
/// ("the password for bob to hunter2"), or right after the name with a digit in it. It's a best
/// effort, never whole: a sentence can say a secret in more ways than words can be told apart,
/// which is why no file keeps the sentence. The card shows it whole.
pub fn hide_said(what: &str) -> (String, usize) {
    let (text, mut hidden) = hide_text(what);
    let core = |word: &str| -> String {
        (word.chars())
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
    };
    // What may come between a name and its value: "to", "is", "=", or a dash. Not "as", which
    // more often starts the words after ("the token, as asked").
    let filler = |word: &str| {
        let lower = word.to_lowercase();
        let bare = lower.trim_matches(|c: char| !c.is_alphanumeric() && c != '=' && c != ':');
        matches!(bare, "to" | "is" | "was" | "=" | ":" | "")
    };
    let mut words: Vec<String> = text.split(' ').map(str::to_string).collect();
    let mut at = 0;
    while at < words.len() {
        // A name and its value in one word: "token=abc123", "password:hunter2".
        if let Some((name, value)) = words[at].split_once(['=', ':'])
            && !value.is_empty()
            && secret_name(name)
            && value != HIDDEN
        {
            let sep = &words[at][name.len()..name.len() + 1];
            words[at] = format!("{name}{sep}{HIDDEN}");
            hidden += 1;
            at += 1;
            continue;
        }
        let one = secret_name(&core(&words[at]));
        let two = at + 1 < words.len()
            && !one
            && secret_name(&format!("{}{}", core(&words[at]), core(&words[at + 1])))
            && !secret_name(&core(&words[at + 1]));
        if !(one || two) {
            at += 1;
            continue;
        }
        let mut next = at + if two { 2 } else { 1 };
        let named = &words[next - 1];
        let mut past = named.ends_with(':') || named.ends_with('=');
        // Past a few words between: "the password for bob to hunter2", "the secret key to …".
        let mut near = next..(next + NEAR).min(words.len());
        if !past && let Some(filler_at) = near.find(|&word| filler(&words[word])) {
            next = filler_at;
        }
        while next < words.len() && filler(&words[next]) {
            past = true;
            next += 1;
        }
        let Some(value) = words.get(next) else {
            break;
        };
        let end = value.trim_end_matches(['.', ',', ';', '!', '?', ')', '\'', '\u{2019}']);
        if !end.is_empty()
            && !value.starts_with(HIDDEN)
            && (past || end.chars().any(|c| c.is_ascii_digit()))
        {
            let rest = value[end.len()..].to_string();
            words[next] = format!("{HIDDEN}{rest}");
            hidden += 1;
        }
        at = next + 1;
    }
    (words.join(" "), hidden)
}

/// How many words after a secret's name `hide_said` looks for the "to" or "is" before its value.
const NEAR: usize = 3;

/// Whether a field by this name holds a secret whatever its value. A CSRF token is the page's own,
/// no use without its cookie, and what a change may need, so it shows.
fn secret_name(name: &str) -> bool {
    if kept_name(name) {
        return false;
    }
    let name = plain_name(name);
    let words = [
        "token",
        "secret",
        "password",
        "passwd",
        "passcode",
        "apikey",
        "accesskey",
        "privatekey",
        "credential",
        "authorization",
        "cookie",
        "sessionid",
        "sessionkey",
        "sessid",
        "jwt",
        "bearer",
    ];
    // A session's id, a refresh token and a one-time code go by these names alone.
    matches!(
        name.as_str(),
        "sid" | "pwd" | "auth" | "session" | "refresh"
    ) || name.ends_with("otp")
        || words.iter().any(|word| name.contains(word))
}

/// Whether a field by this name shows whatever its value looks like: a CSRF token's, which a
/// site's API needs for a change, and every change asks the user first.
fn kept_name(name: &str) -> bool {
    let name = plain_name(name);
    ["csrf", "xsrf", "authenticity"]
        .iter()
        .any(|word| name.contains(word))
}

/// A field's name as `secret_name` reads it: its letters and digits, in lower case.
fn plain_name(name: &str) -> String {
    let name: String = (name.chars()).filter(char::is_ascii_alphanumeric).collect();
    name.to_ascii_lowercase()
}

/// Whether a field by this name holds a secret only when its value reads as one: a `key` or a
/// `code` is as often a sort's key or a country's code.
fn shaped_secret(name: &str, value: &str) -> bool {
    let name = plain_name(name);
    // These codes are a place's, a phone's or an answer's number, however many digits it has.
    let numbers = [
        "zipcode",
        "postalcode",
        "postcode",
        "areacode",
        "countrycode",
        "dialcode",
        "statuscode",
        "errorcode",
    ];
    let digits = !numbers.iter().any(|number| name.ends_with(number));
    (name.ends_with("key") || name.ends_with("code")) && secret_shaped(value, digits)
}

/// Whether a value reads as a secret: a one-time code's six to eight digits, if `digits`, or 16
/// or more of a credential's letters, digits and marks, with both letters and digits, as an API
/// key or an authorization code is.
fn secret_shaped(value: &str, digits_may_be: bool) -> bool {
    let value = value.trim().as_bytes();
    let digits = value.iter().filter(|byte| byte.is_ascii_digit()).count();
    let letters = value
        .iter()
        .filter(|byte| byte.is_ascii_alphabetic())
        .count();
    (digits_may_be && digits == value.len() && (6..=8).contains(&value.len()))
        || (value.len() >= 16
            && digits > 0
            && letters > 0
            && (value.iter()).all(|&byte| base64url(byte) || b".~+/=".contains(&byte)))
}

/// Whether the value a field by this name holds is hidden.
fn secret_field(name: &str, value: &str) -> bool {
    !value.trim().is_empty() && (secret_name(name) || shaped_secret(name, value))
}

/// Whether a byte is in a run `random_run` reads: a token's letters, digits and joins.
fn run_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"_-+=".contains(&byte)
}

/// Whether a run reads as a random secret, whatever names it: a piece of it between `-`, `_`, `+`,
/// `=` and `.` that's 16 or more of mixed case and digits, or 20 or more of one case and digits,
/// spread as a random draw is. Hex is never one, nor digits alone, since commits, digests and ids
/// are written so, nor a `sha512-` digest; and a piece that reads as words isn't, such as a file
/// named for what it shows.
fn random_run(run: &str) -> bool {
    if ["sha256-", "sha384-", "sha512-"]
        .iter()
        .any(|digest| run.starts_with(digest))
    {
        return false;
    }
    (run.trim_end_matches('='))
        .split(['-', '_', '+', '=', '.'])
        .any(random_piece)
}

fn random_piece(piece: &str) -> bool {
    let bytes = piece.as_bytes();
    let upper = bytes.iter().any(u8::is_ascii_uppercase);
    let lower = bytes.iter().any(u8::is_ascii_lowercase);
    let digit = bytes.iter().any(u8::is_ascii_digit);
    let hex = piece
        .strip_prefix("0x")
        .or_else(|| piece.strip_prefix("0X"))
        .unwrap_or(piece);
    if !(upper || lower) || hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return false;
    }
    let spread = entropy(bytes);
    (bytes.len() >= 16 && upper && lower && digit && spread >= 3.5 && !words(piece))
        || (bytes.len() >= 20 && digit && upper != lower && spread >= 3.6)
}

/// Bits a byte of `bytes` carries, by how often each comes.
fn entropy(bytes: &[u8]) -> f64 {
    let mut counts = [0usize; 256];
    for &byte in bytes {
        counts[byte as usize] += 1;
    }
    let length = bytes.len() as f64;
    (counts.iter().filter(|&&count| count > 0))
        .map(|&count| {
            let share = count as f64 / length;
            -share * share.log2()
        })
        .sum()
}

/// Whether mixed case reads as words: at least 30% of its letters vowels, as a word's are and a
/// random draw's (about 19%) aren't, and words of three letters or more on average, in the
/// `CamelCase` or `WORDWord` way, where a random draw changes case every letter or two.
fn words(piece: &str) -> bool {
    let bytes = piece.as_bytes();
    let letters = bytes
        .iter()
        .filter(|byte| byte.is_ascii_alphabetic())
        .count();
    let vowels = (bytes.iter())
        .filter(|byte| b"aeiouAEIOU".contains(byte))
        .count();
    let starts = (0..bytes.len())
        .filter(|&at| {
            let letter = |at: usize| bytes.get(at).is_some_and(u8::is_ascii_alphabetic);
            let lower = |at: usize| bytes.get(at).is_some_and(u8::is_ascii_lowercase);
            letter(at)
                && (at == 0
                    || !letter(at - 1)
                    || bytes[at].is_ascii_uppercase() && (lower(at - 1) || lower(at + 1)))
        })
        .count();
    letters > 0 && vowels * 10 >= letters * 3 && letters >= starts * 3
}

/// In the run `from..end` of `text`, the bounds of what `random_run` hides, if any: its value if
/// it's a `name=value`, nothing under a name `kept_name` keeps, and not an escape's two digits
/// before it ("%2F"). A hidden value runs on over the runs a `/` joins it to, of eight or more,
/// as base64 has them.
fn random_span(text: &str, from: usize, end: usize) -> Option<Bounds> {
    let bytes = text.as_bytes();
    let mut start = from;
    if from > 0 && bytes[from - 1] == b'%' {
        start = (from + 2).min(end);
    }
    if let Some(equals) = text[start..end].find('=') {
        let (name, value) = (&text[start..start + equals], &text[start + equals + 1..end]);
        let named = name
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic());
        if named && !value.trim_start_matches('=').is_empty() {
            if kept_name(name) {
                return None;
            }
            start += equals + 1;
        }
    }
    if !random_run(&text[start..end]) {
        return None;
    }
    let joined = |at: usize, step: isize| {
        let mut length = 0;
        let mut next = at as isize + step;
        while next >= 0 && (next as usize) < bytes.len() && run_byte(bytes[next as usize]) {
            length += 1;
            next += step;
        }
        scanned(length);
        length
    };
    let (mut start, mut end) = (start, end);
    while end < bytes.len() && bytes[end] == b'/' && joined(end, 1) >= 8 {
        end += 1 + joined(end, 1);
    }
    while start > 0 && bytes[start - 1] == b'/' && joined(start - 1, -1) >= 8 {
        start -= 1 + joined(start - 1, -1);
    }
    Some((start, end))
}

fn hide_json(value: &mut Value, secret: bool) -> usize {
    match value {
        Value::String(text) if !text.is_empty() && (secret || token_like(text)) => {
            *value = Value::String(HIDDEN.into());
            1
        }
        // A string can hold an address, markup or script, whose secrets go as the text's do.
        Value::String(text) => {
            let (kept, hidden) = hide_text(text);
            if hidden > 0 {
                *text = kept;
            }
            hidden
        }
        Value::Array(items) => items.iter_mut().map(|item| hide_json(item, secret)).sum(),
        Value::Object(fields) => (fields.iter_mut())
            .map(|(name, item)| match item {
                Value::String(text) if kept_name(name) => match token_like(text) {
                    true => {
                        *item = Value::String(HIDDEN.into());
                        1
                    }
                    false => 0,
                },
                Value::String(text) if shaped_secret(name, text) => {
                    *item = Value::String(HIDDEN.into());
                    1
                }
                _ => hide_json(item, secret || secret_name(name)),
            })
            .sum(),
        _ => 0,
    }
}

/// Whether a value is a credential, whatever its field's name: a JWT, or a bearer one.
fn token_like(text: &str) -> bool {
    let text = text.trim();
    match text
        .strip_prefix("Bearer ")
        .or_else(|| text.strip_prefix("bearer "))
    {
        Some(token) => !token.trim().is_empty(),
        None => jwt(text.as_bytes()) == text.len(),
    }
}

fn base64url(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

/// The length of the JWT `text` starts with, or 0: a header and claims, each base64url of a JSON
/// object, so "eyJ", then a signature, all joined by dots.
fn jwt(text: &[u8]) -> usize {
    let mut at = 0;
    for part in 0..3 {
        let start = at;
        while at < text.len() && base64url(text[at]) {
            at += 1;
        }
        scanned(at - start);
        if part == 2 {
            break;
        }
        let read = &text[start..at];
        if read.len() < 10 || !read.starts_with(b"eyJ") || text.get(at) != Some(&b'.') {
            return 0;
        }
        at += 1;
    }
    at
}

/// As `hide_secrets`, in text that isn't JSON whole, such as a page, its script or a form's body,
/// by the same names as JSON's fields: a field's value, its name quoted once, twice or not at all,
/// as a script's variables are too; an input's value and a meta tag's content; and the values in
/// an address's query or a form's. And each JWT and bearer credential, whatever its name.
fn hide_text(text: &str) -> (String, usize) {
    let bytes = text.as_bytes();
    let (mut kept, mut hidden) = (String::with_capacity(text.len()), 0);
    let (mut at, mut copied) = (0, 0);
    let mut hide = |kept: &mut String, copied: &mut usize, from: usize, to: usize| {
        kept.push_str(&text[*copied..from]);
        kept.push_str(HIDDEN);
        *copied = to;
        hidden += 1;
    };
    let said = |(from, to): (usize, usize)| &text[from..to];
    while at < bytes.len() {
        let starts = at == 0 || !base64url(bytes[at - 1]);
        if bytes[at] == b'<'
            && let Some((name, value, password)) = tag_value(text, at)
        {
            if password && value.1 > value.0 || secret_field(&name, said(value)) {
                hide(&mut kept, &mut copied, value.0, value.1);
                at = value.1;
                continue;
            }
            if kept_name(&name) {
                at = value.1;
                continue;
            }
        }
        if matches!(bytes[at], b'"' | b'\'') {
            if let Some((name, value)) = field(bytes, at) {
                if secret_field(said(name), said(value)) {
                    hide(&mut kept, &mut copied, value.0, value.1);
                    at = value.1 + 1;
                } else if kept_name(said(name)) {
                    at = value.1 + 1;
                } else {
                    // Its value is looked through as text, as it may be an address.
                    at = name.1 + 1;
                }
                continue;
            }
        }
        let query = match bytes[at] {
            b'?' | b'&' | b'#' | b';' => Some(at + 1),
            _ if at == 0 => Some(0),
            _ => None,
        };
        if let Some((name, value)) = query.and_then(|from| query_value(bytes, from)) {
            if secret_field(said(name), said(value)) {
                hide(&mut kept, &mut copied, value.0, value.1);
                at = value.1;
                continue;
            }
            if kept_name(said(name)) {
                at = value.1;
                continue;
            }
        }
        if let Some((name, value)) = bare_field(bytes, at) {
            if secret_field(said(name), said(value)) {
                hide(&mut kept, &mut copied, value.0, value.1);
                at = value.1 + 1;
            } else if kept_name(said(name)) {
                at = value.1 + 1;
            } else {
                at = value.0;
            }
            continue;
        }
        if starts && bytes[at] == b'e' {
            let length = jwt(&bytes[at..]);
            if length > 0 {
                hide(&mut kept, &mut copied, at, at + length);
                at += length;
                continue;
            }
        } else if starts
            && (bytes[at..].starts_with(b"Bearer ") || bytes[at..].starts_with(b"bearer "))
        {
            let from = at + 7;
            let mut to = from;
            while to < bytes.len() && (base64url(bytes[to]) || b".~+/=".contains(&bytes[to])) {
                to += 1;
            }
            scanned(to - from);
            if to - from >= 8 {
                hide(&mut kept, &mut copied, from, to);
                at = to;
                continue;
            }
        }
        if run_byte(bytes[at]) && (at == 0 || !run_byte(bytes[at - 1])) {
            let end = (at..bytes.len())
                .find(|&next| !run_byte(bytes[next]))
                .unwrap_or(bytes.len());
            scanned(end - at);
            if let Some((from, to)) = random_span(text, at, end) {
                let from = from.max(copied);
                hide(&mut kept, &mut copied, from, to);
                at = to;
                continue;
            }
        }
        at += 1;
    }
    kept.push_str(&text[copied..]);
    (kept, hidden)
}

/// The bounds of a byte range in `hide_text`'s text.
type Bounds = (usize, usize);

/// The name quoted at `at`, then a colon, then a quoted value: the bounds of the name's text and
/// the value's, inside their quotes. Either may be in double or single quotes, and the value in
/// a script's backquotes.
fn field(bytes: &[u8], at: usize) -> Option<(Bounds, Bounds)> {
    // A name is short; a longer string is a value, which is looked through as text.
    let name_end = quoted(bytes, at, 64)?;
    let next = past_spaces(bytes, name_end + 1);
    if bytes.get(next) != Some(&b':') {
        return None;
    }
    let next = past_spaces(bytes, next + 1);
    if !matches!(bytes.get(next), Some(b'"' | b'\'' | b'`')) {
        return None;
    }
    let value_end = quoted(bytes, next, usize::MAX)?;
    Some(((at + 1, name_end), (next + 1, value_end)))
}

/// A script's name, unquoted, at `at`, then a colon or `=`, then a quoted value, as `field` has
/// them: `token: "…"` in an object or `window.apiKey = "…"`.
fn bare_field(bytes: &[u8], at: usize) -> Option<(Bounds, Bounds)> {
    let word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$';
    let first = bytes[at];
    if !(first.is_ascii_alphabetic() || first == b'_' || first == b'$')
        || (at > 0 && word(bytes[at - 1]))
    {
        return None;
    }
    let mut end = at;
    while end < bytes.len() && end - at <= 64 && word(bytes[end]) {
        end += 1;
    }
    if end - at > 64 {
        return None;
    }
    let next = past_spaces(bytes, end);
    match bytes.get(next) {
        Some(b':') => {}
        // Not a comparison or an arrow.
        Some(b'=') if !matches!(bytes.get(next + 1), Some(b'=' | b'>')) => {}
        _ => return None,
    }
    let next = past_spaces(bytes, next + 1);
    if !matches!(bytes.get(next), Some(b'"' | b'\'' | b'`')) {
        return None;
    }
    let value_end = quoted(bytes, next, usize::MAX)?;
    Some(((at, end), (next + 1, value_end)))
}

/// A query's or form's `name=value` from `from`, as an address or a form's body has it: the
/// bounds of its name and of its value, which isn't empty.
fn query_value(bytes: &[u8], from: usize) -> Option<(Bounds, Bounds)> {
    let named = |byte: u8| byte.is_ascii_alphanumeric() || b"_.-%[]".contains(&byte);
    // A name is short, so its run stops past 64 rather than reading a long word to its end.
    let mut end = from;
    while end < bytes.len() && end - from <= 64 && named(bytes[end]) {
        end += 1;
    }
    if end == from || end - from > 64 || bytes.get(end) != Some(&b'=') {
        return None;
    }
    let start = end + 1;
    let mut stop = start;
    while stop < bytes.len()
        && !(bytes[stop].is_ascii_whitespace() || b"&#?;,\"'`<>\\()".contains(&bytes[stop]))
    {
        stop += 1;
    }
    scanned(stop - start);
    (stop > start).then_some(((from, end), (start, stop)))
}

/// At an `<input` or `<meta` tag, the name its value goes by, the bounds of that value, and
/// whether it's a password's: an input's `value` by its `name` or else its `id`, and a meta tag's
/// `content` by its `name` or else its `property`, `itemprop` or `http-equiv`.
fn tag_value(text: &str, at: usize) -> Option<(String, Bounds, bool)> {
    let tag = tag(text, at).filter(|tag| !tag.closing && !tag.open)?;
    let input = match tag.name.as_str() {
        "input" => true,
        "meta" => false,
        _ => return None,
    };
    let (mut name, mut value, mut password) = (None::<String>, None, false);
    for (attribute, bounds) in tag.attributes {
        let said = &text[bounds.0..bounds.1];
        match attribute.as_str() {
            "name" => name = Some(said.to_string()),
            "id" | "property" | "itemprop" | "http-equiv" if name.is_none() => {
                name = Some(said.to_string())
            }
            "value" if input => value = Some(bounds),
            "content" if !input => value = Some(bounds),
            "type" if input => password = said.trim().eq_ignore_ascii_case("password"),
            _ => {}
        }
    }
    Some((name.unwrap_or_default(), value?, password))
}

/// A markup tag: its name in lower case, whether it closes, its attributes' names in lower case
/// and their values' bounds, and where it ends, past its `>`. One with a quoted value left open is
/// `open`, its attributes those before that value, and ends at the end of the text, as a browser
/// reads the rest as that value.
struct Tag {
    name: String,
    closing: bool,
    attributes: Vec<(String, Bounds)>,
    end: usize,
    open: bool,
}

/// The tag at the `<` at `at`, if one starts there.
fn tag(text: &str, at: usize) -> Option<Tag> {
    let bytes = text.as_bytes();
    let mut next = at + 1;
    let closing = bytes.get(next) == Some(&b'/');
    if closing {
        next += 1;
    }
    let start = next;
    if !bytes.get(start).is_some_and(u8::is_ascii_alphabetic) {
        return None;
    }
    while next < bytes.len() && (bytes[next].is_ascii_alphanumeric() || bytes[next] == b'-') {
        next += 1;
    }
    scanned(next - start);
    let name = text[start..next].to_ascii_lowercase();
    let mut attributes = Vec::new();
    loop {
        next = past_spaces(bytes, next);
        // A tag ends at the next `<` outside a quoted value as at its `>`, so one left open isn't
        // read on to the end of the body, and again from each tag after it.
        match bytes.get(next) {
            None | Some(b'<') => break,
            Some(b'>') => {
                next += 1;
                break;
            }
            Some(b'/') => {
                next += 1;
                continue;
            }
            _ => {}
        }
        let start = next;
        while next < bytes.len()
            && !(bytes[next].is_ascii_whitespace() || b"=<>/".contains(&bytes[next]))
        {
            next += 1;
        }
        scanned(next - start);
        let attribute = text[start..next].to_ascii_lowercase();
        next = past_spaces(bytes, next);
        let mut bounds = (next, next);
        if bytes.get(next) == Some(&b'=') {
            next = past_spaces(bytes, next + 1);
            match bytes.get(next) {
                Some(&quote @ (b'"' | b'\'')) => {
                    let close = (next + 1..bytes.len()).find(|&at| bytes[at] == quote);
                    scanned(close.unwrap_or(bytes.len()) - next);
                    let Some(close) = close else {
                        let end = bytes.len();
                        return Some(Tag {
                            name,
                            closing,
                            attributes,
                            end,
                            open: true,
                        });
                    };
                    bounds = (next + 1, close);
                    next = close + 1;
                }
                _ => {
                    let from = next;
                    while next < bytes.len()
                        && !(bytes[next].is_ascii_whitespace() || b"<>".contains(&bytes[next]))
                    {
                        next += 1;
                    }
                    scanned(next - from);
                    bounds = (from, next);
                }
            }
        }
        if next == start {
            next += 1;
        } else {
            attributes.push((attribute, bounds));
        }
    }
    Some(Tag {
        name,
        closing,
        attributes,
        end: next,
        open: false,
    })
}

/// The first byte from `at` that isn't a space.
fn past_spaces(bytes: &[u8], from: usize) -> usize {
    let mut at = from;
    while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    }
    scanned(at - from);
    at
}

/// Where the string quoted at `at`, by the quote there, closes, if within `most` bytes and on its
/// line.
fn quoted(bytes: &[u8], at: usize, most: usize) -> Option<usize> {
    let quote = bytes[at];
    let mut next = at + 1;
    while next < bytes.len() && next - at <= most {
        match bytes[next] {
            b'\\' => next += 2,
            b'\n' => break,
            byte if byte == quote => {
                scanned(next - at);
                return Some(next);
            }
            _ => next += 1,
        }
    }
    scanned(next - at);
    None
}

/// What a call's requests are, for a task's trace: one's method and path, or how many.
pub fn described(arguments: &Value) -> String {
    let requests = arguments.get("requests").and_then(Value::as_array);
    match requests.map(Vec::as_slice) {
        Some([one]) => {
            let text = |name: &str| one.get(name).and_then(Value::as_str).unwrap_or("").trim();
            format!(
                "{} {}",
                text("method").to_ascii_uppercase(),
                clip(text("path"), 40)
            )
        }
        Some(many) if !many.is_empty() => format!("{} requests", many.len()),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const PAGE: &str = "https://team.example.com/board?view=1#top";

    fn lines(sent: &[Sent]) -> Vec<String> {
        (sent.iter())
            .map(|each| {
                let indent = "  ".repeat(each.depth.into());
                match each.name.is_empty() {
                    true => format!("{indent}{}", each.value),
                    false => format!("{indent}{}: {}", each.name, each.value),
                }
            })
            .collect()
    }

    /// What a request sent is a line a field for the log: a form's decoded, a JSON object's with
    /// nested ones under their names and a list's items by place, any other text's lines; and a
    /// secret's value never, by name or by its look.
    #[test]
    fn what_was_sent_is_a_line_a_field_without_secrets() {
        let form = "name=Ada+Lovelace&city=London%2C+UK&password=hunter2&csrf_token=abc&empty=";
        assert_eq!(
            lines(&sent(form, "application/x-www-form-urlencoded")),
            [
                "name: Ada Lovelace",
                "city: London, UK",
                "password: (hidden)",
                "csrf_token: abc",
                "empty: \"\""
            ]
        );
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTYifQ.c2lnbmF0dXJl";
        let nested = json!({
            "board": { "name": "Home", "columns": [
                { "title": "To do", "cards": 2 }, { "title": "Done", "cards": 5 }
            ]},
            "tags": ["home", "weekly"],
            "api_token": "s3cret",
            "auth": { "user": "ada", "key": 7 },
            "note": jwt,
            "notify": true,
            "steps": [{ "do": ["a"] }],
        });
        assert_eq!(
            lines(&sent(&nested.to_string(), "application/json")),
            [
                "board: ",
                "  name: Home",
                "  columns: ",
                "    1: title: To do · cards: 2",
                "    2: title: Done · cards: 5",
                "tags: home, weekly",
                "api_token: (hidden)",
                "auth: ",
                "  user: (hidden)",
                "  key: (hidden)",
                "note: (hidden)",
                "notify: true",
                "steps: ",
                "  1: ",
                "    do: a",
            ]
        );
        let text = format!("first line\n\nAuthorization: Bearer {jwt}\n");
        assert_eq!(
            lines(&sent(&text, "text/plain")),
            ["first line", "Authorization: Bearer (hidden)"]
        );
        assert!(sent("", "application/json").is_empty());
        let long =
            json!({ "items": (0..500).map(|n| json!({ "n": { "deep": n } })).collect::<Vec<_>>() });
        let kept = sent(&long.to_string(), "application/json");
        assert_eq!(kept.len(), SENT_LINES);
        assert_eq!(kept.last().map(|each| each.value.as_str()), Some("…"));
    }

    fn parsed(requests: Value) -> Result<Vec<Request>, Wrong> {
        parse(&json!({ "requests": requests }), PAGE)
    }

    /// A path goes to the page's own origin, and a whole address only if it's on it.
    #[test]
    fn requests_go_to_the_pages_own_site() {
        let sent = parsed(json!([{ "method": "get", "path": "/api/items?page=2#x" }])).unwrap();
        assert_eq!(sent[0].url, "https://team.example.com/api/items?page=2");
        assert_eq!(
            (sent[0].method.as_str(), sent[0].path.as_str()),
            ("GET", "/api/items?page=2")
        );
        assert!(!sent[0].changes());
        let whole = json!([{ "method": "GET", "path": "HTTPS://Team.Example.com:443/a" }]);
        assert_eq!(parsed(whole).unwrap()[0].url, "https://team.example.com/a");
        for elsewhere in [
            "https://example.com/a",
            "http://team.example.com/a",
            "https://team.example.com:8443/a",
            "https://team.example.com@evil.example/a",
        ] {
            let asked = json!([{ "method": "GET", "path": elsewhere }]);
            assert!(
                matches!(parsed(asked), Err(Wrong::Refused(_))),
                "{elsewhere} went"
            );
        }
        for wrong in [
            "//evil.example/a",
            "/\\evil.example/a",
            "api/items",
            "/a\tb",
            "",
        ] {
            let asked = json!([{ "method": "GET", "path": wrong }]);
            assert!(
                matches!(parsed(asked), Err(Wrong::Malformed(_))),
                "{wrong:?} went"
            );
        }
        for page in ["about:blank", ""] {
            let blank = parse(
                &json!({ "requests": [{ "method": "GET", "path": "/" }] }),
                page,
            );
            assert!(matches!(blank, Err(Wrong::Malformed(_))));
        }
    }

    /// A path an agent wrote as it would type it in the address bar goes as the address bar sends
    /// it: a space as %20, a quote as it is, for the page's fetch to encode. Any other whitespace
    /// or control character is refused by name and place, with how to encode it, and a path not
    /// from the root says how it would be (#3661).
    #[test]
    fn a_path_says_what_it_wont_take_and_where() {
        let path = |path: &str| parsed(json!([{ "method": "GET", "path": path }]));
        let sent = path("/search?q=\"red shoes\" size 9").unwrap();
        assert_eq!(
            sent[0].url,
            "https://team.example.com/search?q=\"red%20shoes\"%20size%209"
        );
        assert_eq!(sent[0].path, "/search?q=\"red%20shoes\"%20size%209");
        let whole = path("https://team.example.com/a b").unwrap();
        assert_eq!(whole[0].url, "https://team.example.com/a%20b");
        let wrong = |given: &str| match path(given) {
            Err(Wrong::Malformed(why)) => why,
            other => panic!("{given:?} answered {other:?}"),
        };
        assert_eq!(
            wrong("/search?q=red\nshoes"),
            "path is a path on the tab's site, without a line break: there's one at character \
             14, after “/search?q=red”. Encode it as %0A to send it."
        );
        assert_eq!(
            wrong("/a\u{a0}b"),
            "path is a path on the tab's site, without the character U+00A0: there's one at \
             character 3, after “/a”. Encode it as %C2%A0 to send it."
        );
        assert_eq!(
            wrong("api/items"),
            "path is a path on the tab's site, from its root, as /api/items is, or a whole \
             address on it."
        );
        assert!(wrong("//evil.example/a").starts_with(
            "path is a path on the tab's site, from its root: one starting with // is read"
        ));
        assert_eq!(
            wrong(""),
            "path is a path on the tab's site, such as /api/items?q=red, or a whole address on \
             it."
        );
    }

    /// The headers the browser sets are the browser's, Authorization is a credential, and a
    /// method override to what pages can't send is refused, as Chromium refuses it.
    #[test]
    fn the_browsers_headers_and_credentials_are_refused() {
        let with = |name: &str, value: &str| {
            let asked = json!([{ "method": "POST", "path": "/a", "headers": { name: value },
                                 "what": "Add an item." }]);
            parsed(asked)
        };
        let refused = [
            "Cookie",
            "cookie",
            "Host",
            "Origin",
            "Referer",
            "Sec-Fetch-Site",
            "Proxy-Authorization",
            "Authorization",
        ];
        for name in refused {
            assert!(
                matches!(with(name, "x"), Err(Wrong::Refused(_))),
                "{name} was set"
            );
        }
        assert!(matches!(
            with("X-HTTP-Method-Override", "TRACE"),
            Err(Wrong::Refused(_))
        ));
        assert!(matches!(with("Bad Name", "x"), Err(Wrong::Malformed(_))));
        let sent = with("X-CSRF-Token", "abc").unwrap();
        assert_eq!(
            sent[0].headers,
            [("X-CSRF-Token".to_string(), "abc".to_string())]
        );
        let trace = parsed(json!([{ "method": "TRACE", "path": "/" }]));
        assert!(matches!(trace, Err(Wrong::Refused(_))));
        let spaced = parsed(json!([{ "method": "GE T", "path": "/" }]));
        assert!(matches!(spaced, Err(Wrong::Malformed(_))));
    }

    /// A GET with a method override is a change, as the server takes it, and says what it does.
    #[test]
    fn what_changes_is_what_the_server_does() {
        let item = json!({ "__metadata": { "type": "x" }, "Status": "Closed" });
        let asked = json!([
            { "method": "POST", "path": "/_api/web/lists/getbytitle('Tasks')/items(1)",
              "headers": { "X-HTTP-Method": "merge" }, "body": item, "what": "Close task 1." },
            { "method": "GET", "path": "/a", "headers": { "X-HTTP-Method-Override": "DELETE" },
              "what": "Delete a." },
            { "method": "HEAD", "path": "/a" },
        ]);
        let sent = parsed(asked).unwrap();
        assert!(sent[0].target().starts_with("POST as MERGE /_api/"));
        assert_eq!(sent[0].headers.last().unwrap().1, "application/json");
        assert!(sent[1].changes() && sent[1].kind() == Does::Deletes && sent[1].method == "GET");
        assert_eq!(sent[1].methods(), "GET as DELETE");
        assert!(!sent[2].changes());
    }

    /// A request is a read only when the method sent and every override header it sets both say
    /// GET or HEAD: a server may take either, so a change by either asks (#1439). What it's called
    /// names the method sent first, and the override beside it where it differs.
    #[test]
    fn a_read_is_one_only_by_its_method_and_its_overrides() {
        let overrides = [
            "X-HTTP-Method-Override",
            "X-HTTP-Method",
            "X-Method-Override",
        ];
        let one = |method: &str, headers: Value| {
            let asked = json!([{ "method": method, "path": "/a", "headers": headers,
                                 "what": "Do it." }]);
            parsed(asked).unwrap().remove(0)
        };
        for method in ["POST", "PUT", "PATCH", "DELETE"] {
            for name in overrides {
                for read in ["GET", "HEAD", "get"] {
                    let sent = one(method, json!({ name: read }));
                    assert!(sent.changes(), "{method} with {name}: {read} went unasked");
                    assert_eq!(sent.method, method);
                }
            }
        }
        for name in overrides {
            for change in ["POST", "DELETE", "MERGE", "GET, DELETE", ""] {
                let sent = one("GET", json!({ name: change }));
                assert!(sent.changes(), "GET with {name}: {change:?} went unasked");
            }
        }
        let both = one(
            "GET",
            json!({ "X-HTTP-Method": "GET", "X-Method-Override": "DELETE" }),
        );
        assert!(
            both.changes(),
            "a second override naming a change went unasked"
        );
        for method in ["GET", "HEAD"] {
            assert!(!one(method, json!({})).changes(), "a plain {method} asked");
            for name in overrides {
                for read in ["GET", "HEAD"] {
                    let sent = one(method, json!({ name: read }));
                    assert!(!sent.changes(), "{method} with {name}: {read} asked");
                }
            }
        }

        let masked = one("POST", json!({ "X-HTTP-Method-Override": "GET" }));
        assert_eq!(masked.target(), "POST as GET /a");
        assert_eq!(masked.what, "Do it.");
        let plain = one("POST", json!({}));
        assert_eq!(plain.target(), "POST /a");
        let same = one("DELETE", json!({ "X-HTTP-Method": "delete" }));
        assert_eq!(same.target(), "DELETE /a");
    }

    /// What the card says a change does is the stronger of the method sent and every override, as
    /// a server may take either: a delete if any says DELETE, data to send only when all say POST,
    /// and a change for anything else, a method the browser doesn't know included.
    #[test]
    fn a_change_is_the_strongest_its_method_and_overrides_say() {
        let one = |method: &str, headers: Value| {
            let asked = json!([{ "method": method, "path": "/a", "headers": headers,
                                 "what": "Do it." }]);
            parsed(asked).unwrap().remove(0).kind()
        };
        let none = json!({});
        assert_eq!(one("POST", none.clone()), Does::Sends);
        assert_eq!(one("DELETE", none.clone()), Does::Deletes);
        for method in [
            "PUT", "PATCH", "MERGE", "PURGE", "MKCOL", "COPY", "MOVE", "FROB",
        ] {
            assert_eq!(one(method, none.clone()), Does::Changes, "{method}");
        }
        for name in [
            "X-HTTP-Method-Override",
            "X-HTTP-Method",
            "X-Method-Override",
        ] {
            assert_eq!(one("POST", json!({ name: "post" })), Does::Sends, "{name}");
            for method in ["GET", "POST", "PUT", "DELETE", "MERGE"] {
                assert_eq!(
                    one(method, json!({ name: "DELETE" })),
                    Does::Deletes,
                    "{method} {name}"
                );
            }
            assert_eq!(
                one("DELETE", json!({ name: "GET" })),
                Does::Deletes,
                "{name}"
            );
            for (method, says) in [
                ("POST", "GET"),
                ("POST", "PUT"),
                ("POST", "MERGE"),
                ("POST", "FROB"),
                ("POST", ""),
                ("PUT", "POST"),
                ("GET", "POST"),
                ("GET", "PATCH"),
            ] {
                assert_eq!(
                    one(method, json!({ name: says })),
                    Does::Changes,
                    "{method} {name}: {says}"
                );
            }
        }
        let both = json!({ "X-HTTP-Method": "POST", "X-Method-Override": "DELETE" });
        assert_eq!(one("POST", both), Does::Deletes);
        // A value with commas is read piece by piece, so a DELETE anywhere in it deletes.
        for says in [
            "POST, DELETE",
            "DELETE, POST",
            " get ,delete",
            "GET,DELETE,",
        ] {
            assert_eq!(
                one("POST", json!({ "X-HTTP-Method-Override": says })),
                Does::Deletes,
                "{says}"
            );
        }
        assert_eq!(
            one("POST", json!({ "X-HTTP-Method": "POST, POST" })),
            Does::Sends
        );
        assert_eq!(
            one("POST", json!({ "X-HTTP-Method": "POST, PUT" })),
            Does::Changes
        );
        let both = json!({ "X-HTTP-Method": "POST", "X-Method-Override": "PUT" });
        assert_eq!(one("POST", both), Does::Changes);
    }

    /// The head names the method sent, then every other method its overrides name, once each and
    /// in their order, from the same pieces the facts line reads.
    #[test]
    fn the_head_names_every_override_that_differs() {
        let head = |method: &str, headers: Value| {
            let asked = json!([{ "method": method, "path": "/a", "headers": headers,
                                 "what": "Do it." }]);
            parsed(asked).unwrap().remove(0).methods()
        };
        assert_eq!(head("POST", json!({})), "POST");
        assert_eq!(head("POST", json!({ "X-HTTP-Method": "post" })), "POST");
        let two = json!({ "X-HTTP-Method-Override": "GET, DELETE" });
        assert_eq!(head("POST", two), "POST as GET, DELETE");
        let three = json!({ "X-HTTP-Method": "GET", "X-Method-Override": "DELETE, GET, POST" });
        assert_eq!(head("POST", three), "POST as GET, DELETE");
        assert_eq!(head("GET", json!({ "X-HTTP-Method": "GET,  " })), "GET");
    }

    /// A write whose override says it's a read asks, so it needs the agent's sentence as any change
    /// does, and its name leads with the method sent.
    #[test]
    fn a_write_that_says_its_a_read_needs_its_sentence() {
        let masked = json!([{ "method": "POST", "path": "/api/items",
                              "headers": { "X-HTTP-Method-Override": "GET" } }]);
        let Err(Wrong::Malformed(why)) = parsed(masked) else {
            panic!("a POST that said it was a GET went without a `what`");
        };
        assert!(
            why.starts_with("Request 1 (POST as GET /api/items) may change something"),
            "{why}"
        );
        let read = json!([{ "method": "GET", "path": "/a", "headers": { "X-HTTP-Method": "HEAD" },
                            "what": "Read it." }]);
        assert_eq!(parsed(read).unwrap()[0].what, "", "a read kept a sentence");
    }

    /// A body is JSON, typed so, or text, typed as text unless it says; a GET has none.
    #[test]
    fn bodies_are_typed_as_a_pages_fetch_types_them() {
        let form = json!({ "Content-Type": "application/x-www-form-urlencoded" });
        let asked = json!([{ "method": "PUT", "path": "/a", "body": "a=1&b=2", "headers": form,
                             "what": "Set a and b." }]);
        let sent = parsed(asked).unwrap();
        assert_eq!(sent[0].body, "a=1&b=2");
        assert_eq!(sent[0].headers.len(), 1);
        let text = json!([{ "method": "POST", "path": "/a", "body": "hi", "what": "Say hi." }]);
        let text = parsed(text).unwrap();
        assert_eq!(text[0].headers[0].1, "text/plain;charset=UTF-8");
        let get = parsed(json!([{ "method": "GET", "path": "/a", "body": "hi" }]));
        assert!(matches!(get, Err(Wrong::Malformed(_))));
        let many: Vec<Value> = (0..=MAX)
            .map(|_| json!({ "method": "GET", "path": "/" }))
            .collect();
        assert!(matches!(
            parsed(Value::Array(many)),
            Err(Wrong::Malformed(_))
        ));
    }

    /// The answer has the status, the type and a text body, never a binary one, and none of
    /// what looks like a credential.
    #[test]
    fn answers_show_text_without_secrets() {
        let sent = &parsed(json!([{ "method": "GET", "path": "/api/auth/session" }])).unwrap()[0];
        let jwt =
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9P";
        let body = json!({
            "user": { "name": "Ada", "email": "ada@example.com" },
            "accessToken": "ya29.a0AfH6SM",
            "csrfToken": "keep-me",
            "tokens_used": 12,
            "note": jwt,
        });
        let response = Response {
            status: 200,
            kind: "application/json".into(),
            body: body.to_string().into_bytes(),
            ..Response::default()
        };
        let (mut left, mut hidden) = (ALL_SHOWN, 0);
        let read = answer(sent, &response, &mut left, &mut hidden);
        assert!(read.starts_with("GET /api/auth/session → 200 application/json\n\n"));
        assert!(read.contains("Ada") && read.contains("keep-me") && read.contains("12"));
        assert!(!read.contains("ya29") && !read.contains("eyJ"), "{read}");
        assert_eq!(hidden, 2);

        let page = format!(
            "<script>window.__DATA__ = {{\"apiKey\": \"sk-live-1\", \"title\": \"Board\"}}; \
             fetch('/x', {{headers: {{Authorization: 'Bearer abcdefghijkl'}}}}); \
             var t = '{jwt}';</script>"
        );
        let (kept, count) = hide_secrets(&page, false);
        assert_eq!(count, 3, "{kept}");
        assert!(kept.contains("\"title\": \"Board\""), "{kept}");
        assert!(
            !kept.contains("sk-live") && !kept.contains("abcdefghijkl") && !kept.contains("eyJ")
        );

        let image = Response {
            status: 200,
            kind: "image/png".into(),
            body: vec![0x89, 0x50],
            ..Response::default()
        };
        let read = answer(sent, &image, &mut left, &mut hidden);
        assert!(
            read.ends_with("(2 bytes of image/png, not shown)"),
            "{read}"
        );
        let error = Some("No answer came: ERR_TIMED_OUT".to_string());
        let none = Response {
            error,
            ..Response::default()
        };
        assert_eq!(
            answer(sent, &none, &mut left, &mut hidden),
            "GET /api/auth/session: No answer came: ERR_TIMED_OUT"
        );
    }

    /// What a hidden value was, for the tests' messages: none of `values` is in `kept`.
    fn none_of(kept: &str, values: &[&str]) {
        for value in values {
            assert!(!kept.contains(value), "{value} shows in {kept}");
        }
    }

    /// Fields that only ever hold a secret are hidden whatever their value: a session's id, a
    /// one-time code, a passcode and a refresh token.
    #[test]
    fn a_session_id_one_time_code_and_refresh_token_are_hidden() {
        let body = json!({
            "session": "made-up-session-1",
            "PHPSESSID": "made-up-session-2",
            "JSESSIONID": "made-up-session-3",
            "otp": "made-up-otp-1",
            "totp": "made-up-otp-2",
            "passcode": "made-up-pass-1",
            "refresh": "made-up-refresh-1",
            "user": "Ada",
        });
        let (kept, hidden) = hide_secrets(&body.to_string(), true);
        none_of(
            &kept,
            &[
                "made-up-session",
                "made-up-otp",
                "made-up-pass",
                "made-up-refresh",
            ],
        );
        assert!(kept.contains("Ada"), "{kept}");
        assert_eq!(hidden, 7, "{kept}");
    }

    /// `key` and `code` are common names for harmless values, so only a value shaped as a secret
    /// is hidden under them: a long one of letters and digits, or a one-time code's digits.
    #[test]
    fn a_key_or_code_is_hidden_when_it_reads_as_a_secret() {
        let body = json!({
            "key": "Mk7Qw2Lp9Zx4Vb8Nc3Rt",
            "code": "481920",
            "grant": { "code": "Zq8LmN3pR7sT1vX5yB9c" },
            "items": [
                { "key": "sidebar-width", "code": "US" },
                { "key": "title", "code": "ERR_TIMEOUT", "zip_code": "10115" },
            ],
        });
        let (kept, hidden) = hide_secrets(&body.to_string(), true);
        none_of(&kept, &["Mk7Qw2", "481920", "Zq8LmN"]);
        for shown in ["sidebar-width", "\"US\"", "title", "ERR_TIMEOUT", "10115"] {
            assert!(kept.contains(shown), "{shown} is hidden in {kept}");
        }
        assert_eq!(hidden, 3, "{kept}");
    }

    /// A code that's a place's, a phone's or an answer's number shows however many digits it
    /// has, and a one-time code's digits under any other name ending in code are hidden.
    #[test]
    fn a_postal_or_status_code_shows_and_a_one_time_code_is_hidden() {
        let shown = json!({
            "zip_code": "94105123",
            "postalCode": "1234567",
            "postcode": "123456",
            "area_code": "1234567",
            "countryCode": "123456",
            "dial-code": "1234567",
            "status_code": "200000",
            "errorCode": "40400401",
        });
        let (kept, hidden) = hide_secrets(&shown.to_string(), true);
        assert_eq!(hidden, 0, "{kept}");
        let page = "<input name=\"postal_code\" value=\"1234567\"> ?zipcode=94105123";
        assert_eq!(hide_secrets(page, false), (page.to_string(), 0));

        let one_time = json!({
            "code": "481920",
            "verificationCode": "48192017",
            "smscode": "4819201",
            "promo_code": "481920",
        });
        let (kept, hidden) = hide_secrets(&one_time.to_string(), true);
        none_of(&kept, &["481920"]);
        assert_eq!(hidden, 4, "{kept}");
    }

    /// A body of a megabyte of tags, fields or quotes that never close is read a few times over at
    /// most, as each scan stops at its own end rather than reading on through the rest of the body
    /// again from the next tag. The scans count what they read, so a slow machine can't fail it
    /// and a fast one can't hide a read that goes on (#3831).
    #[test]
    fn an_unclosed_tag_or_quote_is_read_once() {
        // The reads a body's bytes may take in all: about three at most today. One read again from
        // each tag or quote would take tens of thousands.
        const READS: usize = 8;
        let repeated = |unit: &str, end: &str| {
            let mut body = unit.repeat(MAX_BODY / unit.len());
            body.push_str(end);
            body
        };
        let bodies = [
            repeated("<input ", ""),
            repeated("<meta ", ""),
            repeated("<input value='", "'"),
            repeated("<input name=token value=", ""),
            repeated("\"token\": '", "'"),
            repeated("token = `", "`"),
            repeated("&", "a"),
            repeated("a", "="),
            repeated("?token_", "=x"),
            repeated("<script>", ""),
            repeated("<textarea>", ""),
            repeated("<!-- ", ""),
            repeated("<!x ", ""),
            repeated("<a href='", "'"),
            repeated("&#x", ""),
            repeated("Mk7Qw2Lp/", ""),
        ];
        let sent = &parsed(json!([{ "method": "GET", "path": "/" }])).unwrap()[0];
        for body in bodies {
            let page = Response {
                status: 200,
                kind: "text/html".into(),
                body: body.clone().into_bytes(),
                ..Response::default()
            };
            let json = serde_json::to_string(&body).unwrap();
            // Each is one answer's work: as text, as JSON, and as a page through its outline.
            let passes: [(&str, &dyn Fn()); 3] = [
                ("text", &|| _ = hide_secrets(&body, false)),
                ("JSON", &|| _ = hide_secrets(&json, true)),
                ("a page", &|| {
                    let (mut left, mut hidden) = (ALL_SHOWN, 0);
                    answer(sent, &page, &mut left, &mut hidden);
                }),
            ];
            for (read_as, pass) in passes {
                let length = match read_as {
                    "JSON" => json.len(),
                    _ => body.len(),
                };
                // The count stops a reader that reads on, rather than waiting out its minutes.
                SCANNED.with(|scanned| scanned.set((0, READS * length)));
                let read = std::panic::catch_unwind(std::panic::AssertUnwindSafe(pass));
                let (count, _) = SCANNED.with(|scanned| scanned.replace((0, usize::MAX)));
                assert!(
                    read.is_ok(),
                    "{count} bytes read as {read_as} for a body of {length} bytes, {:?}",
                    &body[..20]
                );
            }
        }
    }

    /// A page's hidden inputs and meta tags are hidden by the same names as JSON's fields, and a
    /// form's CSRF token and a search's words show.
    #[test]
    fn a_pages_hidden_inputs_and_meta_tags_are_hidden_by_their_names() {
        let page = r#"<form><input type="hidden" name="session_id" value="made-up-session-4">
            <input type='hidden' name='otp' value='made-up-otp-3'>
            <input value="Mk7Qw2Lp9Zx4Vb8Nc3Rt" type="hidden" name="key">
            <input type="hidden" name="authenticity_token" value="keep-csrf">
            <input name="q" value="shoes"></form>
            <meta name="api-key" content="made-up-key-1"><meta name="description" content="A shop">"#;
        let (kept, hidden) = hide_secrets(page, false);
        none_of(
            &kept,
            &["made-up-session", "made-up-otp", "Mk7Qw2", "made-up-key"],
        );
        for shown in ["keep-csrf", "shoes", "A shop"] {
            assert!(kept.contains(shown), "{shown} is hidden in {kept}");
        }
        assert_eq!(hidden, 4, "{kept}");
    }

    /// A script's fields are hidden by the same names as JSON's, whether their names are quoted
    /// once, twice or not at all, and so are its variables'.
    #[test]
    fn a_scripts_fields_are_hidden_by_their_names_however_quoted() {
        let script = "var config = {token: \"made-up-token-1\", 'secret': 'made-up-secret-1', \
             title: 'Board', key: `Mk7Qw2Lp9Zx4Vb8Nc3Rt`, code: 'ok'};\n\
             window.apiKey = \"made-up-key-2\";";
        let (kept, hidden) = hide_secrets(script, false);
        none_of(
            &kept,
            &["made-up-token", "made-up-secret", "Mk7Qw2", "made-up-key"],
        );
        assert!(kept.contains("'Board'") && kept.contains("'ok'"), "{kept}");
        assert_eq!(hidden, 4, "{kept}");
    }

    /// An address's query, in JSON, a page or a form's body, is hidden by the same names, and
    /// its other values show.
    #[test]
    fn an_addresss_query_is_hidden_by_the_same_names() {
        let body = json!({
            "next": "https://example.com/done?state=s1&access_token=made-up-token-2&page=2",
            "reset": "/reset?code=Zq8LmN3pR7sT1vX5yB9c",
        });
        let (kept, hidden) = hide_secrets(&body.to_string(), true);
        none_of(&kept, &["made-up-token", "Zq8LmN"]);
        assert!(
            kept.contains("state=s1") && kept.contains("page=2"),
            "{kept}"
        );
        assert_eq!(hidden, 2, "{kept}");

        let page = r#"<a href="/login;jsessionid=made-up-session-5?lang=en">In</a>
            <a href="/feed?a=1&amp;session=made-up-session-6">Feed</a>"#;
        let (kept, hidden) = hide_secrets(page, false);
        none_of(&kept, &["made-up-session"]);
        assert!(kept.contains("lang=en") && kept.contains("a=1"), "{kept}");
        assert_eq!(hidden, 2, "{kept}");

        let (kept, hidden) = hide_secrets("user=ada&refresh=made-up-refresh-2&remember=1", false);
        none_of(&kept, &["made-up-refresh"]);
        assert!(
            kept.contains("user=ada") && kept.contains("remember=1"),
            "{kept}"
        );
        assert_eq!(hidden, 1, "{kept}");
    }

    /// A value that reads as a random draw is hidden under any name or none, in JSON, a query
    /// and text: a token of mixed case and digits, one of one case and digits, one after a
    /// prefix, and base64 split by a `/`, which goes whole.
    #[test]
    fn a_value_that_reads_as_random_is_hidden_wherever_it_stands() {
        let body = json!({
            "note": "Mk7Qw2Lp9Zx4Vb8Nc3Rt",
            "seed": "KZ4Q7MXW2RT6PL3NB5VY",
            "pair": "q7x2m9w4k8z3v6n1p5r0t",
            "item": "mu_Zq8LmN3pR7sT1vX5yB9cWd",
            "link": "/files?item=Hv3Kp8Rw2Nz6Qm4Tx9Lc",
            "pem": "Tf6Yh2Wq9Lz4Kd8Rm3Xp/Nb7Vc1Gs5Ju0Ea8Ho",
            "title": "Board",
        });
        let (kept, hidden) = hide_secrets(&body.to_string(), true);
        none_of(
            &kept,
            &[
                "Mk7Qw2", "KZ4Q7M", "q7x2m9", "Zq8LmN", "Hv3Kp8", "Tf6Yh2", "Nb7Vc1",
            ],
        );
        assert!(kept.contains("/files?item=(hidden)"), "{kept}");
        assert!(kept.contains("Board"), "{kept}");
        assert_eq!(hidden, 6, "{kept}");

        let (kept, hidden) = hide_secrets("Your key is Mk7Qw2Lp9Zx4Vb8Nc3Rt, keep it.", false);
        assert_eq!(kept, "Your key is (hidden), keep it.");
        assert_eq!(hidden, 1);
    }

    /// What an agent passes back to a site reads as random but shows: a commit's or a digest's
    /// hex, a UUID, digits, an integrity digest, and names and file names made of words.
    #[test]
    fn hex_uuids_digests_and_words_show() {
        let shown = [
            "3f786850e387550fdab836ed7e6dc881de23001b",
            "9b74c9897bac770ffc029102a200c5de1a7f4b1e3d5c9a8b7e6f5d4c3b2a1908",
            "0x9E3779B97F4A7C15F39CC0605CEDC834",
            "f47ac10b-58cc-4372-a567-0e02b2c3d479",
            "123456789012345678901234",
            "sha512-Mk7Qw2Lp9Zx4Vb8Nc3RtZq8LmN3pR7sT1vX5yB9c==",
            "userNameDisplayConfiguration",
            "RunnerLowAutumnSunriseBlack8Gold",
            "NoteRefLovelace1843",
        ];
        for value in shown {
            let (kept, hidden) = hide_secrets(&json!({ "value": value }).to_string(), true);
            assert_eq!(hidden, 0, "{value} is hidden: {kept}");
        }
        let escaped = "/commit%2F3f786850e387550fdab836ed7e6dc881de23001b?x=1";
        assert_eq!(hide_secrets(escaped, false), (escaped.to_string(), 0));
    }

    /// A name kept today, a CSRF token's, keeps its value however random it reads, in JSON, a
    /// form's body and script, since a change may need it; the same value under another name is
    /// hidden.
    #[test]
    fn a_csrf_token_shows_and_the_same_value_under_another_name_is_hidden() {
        let token = "Mk7Qw2Lp9Zx4Vb8Nc3Rt";
        let kept_as = [
            json!({ "csrf_token": token }).to_string(),
            format!("csrf_token={token}&page=2"),
            format!("var config = {{xsrfToken: '{token}'}};"),
            format!("<meta name=\"csrf-token\" content=\"{token}\">"),
        ];
        for text in kept_as {
            let (kept, hidden) = hide_secrets(&text, text.starts_with('{'));
            assert!(kept.contains(token) && hidden == 0, "{kept}");
        }
        let hidden_as = [
            json!({ "state": token }).to_string(),
            format!("state={token}&page=2"),
            format!("var config = {{state: '{token}'}};"),
        ];
        for text in hidden_as {
            let (kept, hidden) = hide_secrets(&text, text.starts_with('{'));
            assert!(!kept.contains(token) && hidden == 1, "{kept}");
        }
    }

    /// A page comes as its outline: its text, links, form and fields' names, never its script,
    /// meta tags or fields' values, and says so, with how many values that withheld.
    #[test]
    fn a_page_comes_as_its_outline() {
        let sent = &parsed(json!([{ "method": "GET", "path": "/login" }])).unwrap()[0];
        let page = r#"<!DOCTYPE html><html><head><title>Sign in &amp; more</title>
            <meta name="request-id" content="made-up-meta-1">
            <script>window.boot = {"post": "made-up-script-1"};</script>
            <style>.a { color: red }</style></head>
            <body><!-- made-up-comment-1 --><h1>Sign in</h1>
            <form method="post" action="/session">
            <input type="hidden" name="csrf_token" value="made-up-csrf-1">
            <input name="login" value="made-up-login-1"><input type="password" name="password">
            <textarea name="note">made-up-note-1</textarea>
            <input type="submit" value="Sign in"></form>
            <p>New here? <a href="/join?ref=home">Make an account</a></p>
            <img src="/a.png" alt="A logo"></body></html>"#;
        let response = Response {
            status: 200,
            kind: "text/html; charset=utf-8".into(),
            body: page.as_bytes().to_vec(),
            ..Response::default()
        };
        let (mut left, mut hidden) = (ALL_SHOWN, 0);
        let read = answer(sent, &response, &mut left, &mut hidden);
        assert!(
            read.starts_with(
                "GET /login → 200 text/html; charset=utf-8\nThis is the page's outline, not \
                 its source: 4 values were withheld.\n\n"
            ),
            "{read}"
        );
        none_of(&read, &["made-up", "color", "boot"]);
        for shown in [
            "Sign in & more",
            "[form: POST /session]",
            "[text field: login]",
            "[password field: password]",
            "[text field: note]",
            "[button: Sign in]",
            "New here? Make an account (/join?ref=home)",
            "[image: A logo]",
        ] {
            assert!(read.contains(shown), "{shown} isn't in {read}");
        }
        assert_eq!(hidden, 0);

        // A page of no type said is read as one, and text isn't.
        let untyped = Response {
            kind: String::new(),
            ..response.clone()
        };
        let read = answer(sent, &untyped, &mut left, &mut hidden);
        assert!(
            read.contains("outline") && !read.contains("made-up"),
            "{read}"
        );
        let text = Response {
            kind: "text/plain".into(),
            body: b"<b>plain</b>".to_vec(),
            ..response
        };
        let read = answer(sent, &text, &mut left, &mut hidden);
        assert!(
            !read.contains("outline") && read.contains("<b>plain</b>"),
            "{read}"
        );
    }

    /// A quoted value left open runs to the end of the page, as a browser reads it, so the outline
    /// ends there: a page cut inside a field's value, or one whose quote never closes, shows its
    /// text before that tag and nothing of the value or after it.
    #[test]
    fn a_page_left_inside_a_quoted_value_ends_its_outline_there() {
        let sent = &parsed(json!([{ "method": "GET", "path": "/a" }])).unwrap()[0];
        let cut = r#"<!DOCTYPE html><html><body><p>Before the cut</p>
            <form method="post" action="/session">
            <input type="hidden" name="csrf_token" value="made-up-cut-1"#;
        let unclosed = r#"<!DOCTYPE html><html><body><p>Shown first</p>
            <input name="login" value='made-up-open-2><p>made-up-after-2</p>
            <input type="hidden" value="made-up-after-3"><p>made-up-after-4</p></body></html>"#;
        for (page, shown) in [(cut, "Before the cut"), (unclosed, "Shown first")] {
            let response = Response {
                status: 200,
                kind: "text/html".into(),
                body: page.as_bytes().to_vec(),
                ..Response::default()
            };
            let (mut left, mut hidden) = (ALL_SHOWN, 0);
            let read = answer(sent, &response, &mut left, &mut hidden);
            none_of(&read, &["made-up"]);
            assert!(read.contains(shown), "{shown} isn't in {read}");
            assert!(read.contains("1 value was withheld"), "{read}");
        }
    }

    /// A call's answers together show what one may, and each says where it's cut.
    #[test]
    fn long_answers_are_cut() {
        let sent = &parsed(json!([{ "method": "GET", "path": "/a" }])).unwrap()[0];
        let long = Response {
            status: 200,
            kind: "text/plain".into(),
            body: "é".repeat(SHOWN + 5).into_bytes(),
            ..Response::default()
        };
        let (mut left, mut hidden) = (SHOWN + 10, 0);
        let first = answer(sent, &long, &mut left, &mut hidden);
        let all = SHOWN + 5;
        assert!(first.ends_with(&format!(
            "(the rest isn't shown: it's {all} characters in all)"
        )));
        assert_eq!(left, 10);
        let second = answer(sent, &long, &mut left, &mut hidden);
        assert!(second.contains(&format!("\n\n{}\n…", "é".repeat(10))));
        let third = answer(sent, &long, &mut left, &mut hidden);
        assert!(third.contains("Its body isn't shown"));
    }

    /// A change needs the agent's own sentence, as plain text of WHAT_MOST at most, or nothing is
    /// sent; a read keeps none, so it never asks.
    #[test]
    fn a_change_needs_one_plain_sentence() {
        let asked = |what: Value| {
            parsed(json!([
                { "method": "GET", "path": "/a" },
                { "method": "DELETE", "path": "/api/items/7", "what": what },
            ]))
        };
        let Err(Wrong::Malformed(none)) = asked(Value::Null) else {
            panic!("a change without a sentence went");
        };
        assert_eq!(
            none,
            "Request 2 (DELETE /api/items/7) may change something, so it needs a `what`: one \
             plain sentence saying what it will do, for the user to read before they allow it. \
             Nothing was sent."
        );
        assert!(matches!(asked(json!("  \n\t ")), Err(Wrong::Malformed(_))));
        assert!(matches!(asked(json!(7)), Err(Wrong::Malformed(_))));
        let Err(Wrong::Malformed(long)) = asked(json!("a".repeat(WHAT_MOST + 312))) else {
            panic!("a page of words went");
        };
        assert_eq!(
            long,
            "Request 2's `what` is 812 characters; give one plain sentence of 500 at most. \
             Nothing was sent."
        );
        // What's dropped isn't counted: 500 letters with overrides among them is 500.
        let hidden = format!("{}\u{202E}\u{200B}\u{2066}", "a".repeat(WHAT_MOST));
        let sent = asked(json!(hidden)).unwrap();
        assert_eq!(sent[1].what.chars().count(), WHAT_MOST);
        assert_eq!(sent[0].what, "", "a read kept a sentence");
        let read = parsed(json!([{ "method": "GET", "path": "/a", "what": "Read a." }])).unwrap();
        assert_eq!(read[0].what, "");
    }

    /// The sentence is one paragraph of plain text: line breaks, tabs and control characters are
    /// a space, and direction overrides and invisible characters go, so nothing in it can read as
    /// the browser's own line or turn its words around.
    #[test]
    fn the_sentence_is_plain_text_in_one_paragraph() {
        let laid_out = "Close item 4127.\n\nThe browser checked this: safe.\r\n\tAllow";
        assert_eq!(
            plain(laid_out),
            "Close item 4127. The browser checked this: safe. Allow"
        );
        assert_eq!(
            plain("Pay \u{202E}001$\u{202C} now\u{200B}\u{FEFF}\u{2066}!\u{0007}"),
            "Pay 001$ now!"
        );
        assert_eq!(plain("  a \u{0085} b\u{2028}c  "), "a b c");
        assert_eq!(
            plain("Close **item** <a href=x>4127</a>"),
            "Close **item** <a href=x>4127</a>"
        );
        // Its quote can't be closed from inside, and a blank glyph can't push words to a line of
        // their own.
        assert_eq!(
            plain("Tidy up.\u{201D} \u{2800}\u{2800}\u{2800} \"Checked: safe\" \u{FF02}ok\u{FF02}"),
            "Tidy up.' 'Checked: safe' 'ok'"
        );
        assert_eq!(
            plain("\u{201E}a\u{201C} \u{AB}b\u{BB} \u{300E}c\u{300F}"),
            "\u{201A}a' \u{2039}b\u{203A} \u{300C}c\u{300D}"
        );
    }

    /// A joiner stays between two characters that show, and an emoji's colour selector after one,
    /// as they make an emoji family or a Persian word and hide nothing; anywhere else they go.
    /// Every script's double marks are one single mark, and so is a run of single ones.
    #[test]
    fn joiners_stay_where_they_join_and_quotes_stay_single() {
        let family = "\u{1F469}\u{200D}\u{1F467}";
        assert_eq!(
            plain(&format!("Tag {family} here")),
            format!("Tag {family} here")
        );
        let persian = "\u{0645}\u{06CC}\u{200C}\u{062E}\u{0648}\u{0627}\u{0647}\u{0645}";
        assert_eq!(plain(persian), persian);
        assert_eq!(
            plain("Love it \u{2764}\u{FE0F}"),
            "Love it \u{2764}\u{FE0F}"
        );
        assert_eq!(plain("a \u{200D}b c\u{200D} \u{FE0F}d \u{200C}"), "a b c d");
        assert_eq!(
            plain("x\u{2BA}y\u{275D}z\u{275E} \u{5F4}w\u{5F4}"),
            "x'y'z' 'w'"
        );
        assert_eq!(
            plain("Done.'' ''Checked'\u{2019} it's"),
            "Done.' 'Checked' it's"
        );
        let rainbow = "\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}";
        assert_eq!(
            plain(&format!("Fly {rainbow} today")),
            format!("Fly {rainbow} today")
        );
        assert_eq!(plain("a \u{FE0F}\u{200D}b"), "a b");
        assert_eq!(plain("x\u{2EE}y\u{2DD} z`\u{2BC}\u{201A}'w"), "x'y' z`w");
    }

    /// The whole request writes out what wouldn't show, so an override can't turn its text
    /// around and a blank can't hide a word: every control, format and invisible character, blank
    /// glyph and space but the plain one, with where each is, and a body's own line breaks kept.
    #[test]
    fn the_whole_request_writes_out_what_wouldnt_show() {
        let (shown, marks) = escaped("/pay/\u{202E}gro.elpmaxe\u{202C}", false);
        assert_eq!(shown, "/pay/\\u{202E}gro.elpmaxe\\u{202C}");
        assert_eq!(
            marks
                .iter()
                .map(|at| &shown[at.clone()])
                .collect::<Vec<_>>(),
            ["\\u{202E}", "\\u{202C}"]
        );
        let (shown, _) = escaped("a\u{200B}b\u{2800}c\u{00A0}d\te\u{2028}f g", false);
        assert_eq!(
            shown,
            "a\\u{200B}b\\u{2800}c\\u{00A0}d\\u{0009}e\\u{2028}f g"
        );
        assert_eq!(escaped("a\nb", false).0, "a\\u{000A}b");
        assert_eq!(escaped("a\n\nb\r\n", true).0, "a\n\nb\\u{000D}\n");
        assert_eq!(
            escaped("Close 4127 \u{1F469}", true).0,
            "Close 4127 \u{1F469}"
        );
    }

    /// A `\u{…}` the agent typed reads as one the browser wrote but for its mark, so the
    /// backslash before it is written out itself, marked: every unmarked `\u{` is the browser's
    /// own. A backslash before anything else stays.
    #[test]
    fn a_typed_escape_has_its_backslash_written_out() {
        let (shown, marks) = escaped("a\\u{202E}b \\u202E C:\\dir \\u", true);
        assert_eq!(shown, "a\\u{005C}u{202E}b \\u202E C:\\dir \\u");
        assert_eq!(
            marks
                .iter()
                .map(|at| &shown[at.clone()])
                .collect::<Vec<_>>(),
            ["\\u{005C}"]
        );
        let (shown, marks) = escaped("\\u{202E}\u{202E}", false);
        assert_eq!(shown, "\\u{005C}u{202E}\\u{202E}");
        assert_eq!(marks, [0..8, 15..23]);
    }

    /// The log and Settings › Agents' activity keep the sentence without what reads as a secret:
    /// the word after a secret's name and a filler or a colon, the filler a few words on, or a
    /// word with a digit right after the name, and every JWT and bearer credential. A word after
    /// the name with neither, and the words around, stay.
    #[test]
    fn a_sentence_is_kept_without_its_secrets() {
        let hid = |what: &str| hide_said(what).0;
        assert_eq!(
            hid("Set the API key to sk-live-123"),
            "Set the API key to (hidden)"
        );
        assert_eq!(
            hid("Set the API key sk-live-123."),
            "Set the API key (hidden)."
        );
        assert_eq!(
            hid("Change the password: hunter two"),
            "Change the password: (hidden) two"
        );
        assert_eq!(hid("Use token=abc123 for it"), "Use token=(hidden) for it");
        assert_eq!(
            hid("Rotate the access token, as asked"),
            "Rotate the access token, as asked"
        );
        assert_eq!(hid("Reset the password field"), "Reset the password field");
        assert_eq!(
            hid("Save the secret is opensesame now"),
            "Save the secret is (hidden) now"
        );
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N";
        assert_eq!(hid(&format!("Send {jwt} on")), "Send (hidden) on");
        assert_eq!(
            hid("Send Bearer abcdefgh12345 on"),
            "Send Bearer (hidden) on"
        );
        assert_eq!(hid("Close item 4127."), "Close item 4127.");
        assert_eq!(
            hid("Change the password for bob to hunter2"),
            "Change the password for bob to (hidden)"
        );
        assert_eq!(
            hid("Set the secret key to sk-live-123"),
            "Set the secret key to (hidden)"
        );
        // Past NEAR words, a "to" is the sentence's own.
        assert_eq!(
            hid("Reset the password for the whole team to Friday"),
            "Reset the password for the whole team to Friday"
        );
    }

    /// The card shows a sentence whole up to WHAT_SHOWN, and a longer one up to a word, with "…".
    #[test]
    fn the_card_clips_a_long_sentence_at_a_word() {
        let whole = format!("{} end.", "x".repeat(WHAT_SHOWN - 5));
        assert_eq!(whole.chars().count(), WHAT_SHOWN);
        assert_eq!(shown(&whole), whole);
        let long = "Merge the two duplicate items, keeping the older one's history and labels, then \
                    close the newer one with a note pointing at the older one so watchers follow.";
        let clipped = shown(long);
        assert!(clipped.chars().count() <= WHAT_SHOWN, "{clipped}");
        assert!(clipped.ends_with("…") && long.starts_with(clipped.trim_end_matches('…')));
        assert!(
            long[clipped.len() - "…".len()..].starts_with(' '),
            "cut inside a word"
        );
    }

    /// A sentence without a space in its last quarter keeps the card's width of it, cut at the
    /// limit: Japanese, which has none, and one whose only space is near its start.
    #[test]
    fn the_card_clips_a_sentence_without_late_spaces_at_its_limit() {
        let japanese = "重複している二件の項目をまとめ、古い方の履歴とラベルを残してから、新しい方を\
            閉じて古い方へのメモを付けます。見ている人が古い方を追えるように、両方のリンクも更新し、\
            関係するチームの全員に変更を伝えて、来週の会議で結果を確認します。\
            終わったら、作業の記録を共有の場所に保存して、次の担当者に引き継ぎます。";
        assert!(japanese.chars().count() > WHAT_SHOWN);
        let clipped = shown(japanese);
        assert_eq!(clipped.chars().count(), WHAT_SHOWN, "{clipped}");
        assert!(japanese.starts_with(clipped.trim_end_matches('…')));

        let early = format!("Merge {}", "x".repeat(200));
        assert_eq!(shown(&early).chars().count(), WHAT_SHOWN);
        assert!(shown(&early).starts_with("Merge x"));
    }
}
