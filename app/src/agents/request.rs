//! The request tool (#1439): an agent's HTTP requests from a tab's page to that page's own site,
//! as the page's own script could send them, with its cookies, which the browser adds and never
//! shows. A read goes as asked. Every change asks the user, every time, with no Always: an API
//! call does whatever its site makes it do, and no name tells what, as a button's does. What comes
//! back is the body alone, without its headers, and with values that look like sign-in tokens or
//! secrets hidden, so an agent gets the user's session, never a credential to carry off.

use serde_json::Value;

use super::page::clip;

/// The requests one call sends at most.
pub const MAX: usize = 20;
/// The most a request's body may be, in bytes.
pub const MAX_BODY: usize = 1 << 20;
/// The changes a question lists; it counts the rest.
pub const LISTED: usize = 3;
/// The characters of an answer's body the agent reads at most, and of all a call's together.
const SHOWN: usize = 20_000;
pub const ALL_SHOWN: usize = 100_000;
/// What stands for a value that's hidden.
const HIDDEN: &str = "(hidden)";

/// A request, as checked against the page and ready for the core (bc_agent_request).
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    /// As sent, in capitals.
    pub method: String,
    /// What it does, as the question and the log say it: the method, or the one an override
    /// header names, which a server takes in its place.
    pub does: String,
    /// The whole address, on the page's origin.
    pub url: String,
    /// Its path and query.
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Request {
    /// Whether it may change anything: anything but a GET or HEAD that's just that.
    pub fn changes(&self) -> bool {
        !matches!(self.does.as_str(), "GET" | "HEAD")
    }

    /// What it is, for the log and the answer: "POST /api/items".
    pub fn target(&self) -> String {
        format!("{} {}", self.does, clip(&self.path, 60))
    }

    /// What it is, short enough for a line of the question: "MERGE /_api/…/items(1)".
    pub fn line(&self) -> String {
        format!("{} {}", self.does, short(&self.path))
    }

    /// What its body sends, in a line for the question, or "".
    pub fn summary(&self) -> String {
        summary(&self.body)
    }
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
    let malformed = |why: &str| Wrong::Malformed(why.into());
    // Never what the page's address is: an agent may name a tab that isn't its own.
    let origin = origin(page).ok_or_else(|| {
        malformed("The tab has no site's page loaded. Look at it with snapshot first.")
    })?;
    let asked = arguments.get("requests").and_then(Value::as_array);
    let asked = asked.filter(|asked| (1..=MAX).contains(&asked.len()));
    let asked = asked.ok_or_else(|| {
        Wrong::Malformed(format!("requests is 1 to {MAX} requests, each with a method and a path."))
    })?;
    asked.iter().map(|asked| one(asked, &origin)).collect()
}

fn one(asked: &Value, origin: &str) -> Result<Request, Wrong> {
    let malformed = |why: &str| Err(Wrong::Malformed(why.into()));
    let text = |name: &str| asked.get(name).and_then(Value::as_str).map(str::trim);
    let method = text("method").unwrap_or_default().to_ascii_uppercase();
    if !is_token(&method) {
        return malformed("method is an HTTP method: GET, POST, PUT, PATCH, DELETE or the like.");
    }
    if forbidden_method(&method) {
        return Err(Wrong::Refused(format!("Agents can't send {method} requests, as pages can't.")));
    }
    let Some(path) = text("path") else {
        return malformed("path is a path on the tab's site, such as /api/items.");
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
                    return Err(Wrong::Malformed(format!("The header \"{name}\" isn't a valid one.")));
                }
                if let Some(why) = forbidden_header(name, value) {
                    return Err(Wrong::Refused(why));
                }
                headers.push((name.to_string(), value.to_string()));
            }
        }
        Some(_) => return malformed("headers is an object of header names and their values."),
    }
    let has = |name: &str| headers.iter().any(|(each, _)| each.eq_ignore_ascii_case(name));
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
    let overrides = ["x-http-method-override", "x-http-method", "x-method-override"];
    let does = (overrides.iter())
        .find_map(|name| headers.iter().find(|(each, _)| each.eq_ignore_ascii_case(name)))
        .map(|(_, value)| value.to_ascii_uppercase())
        .filter(|does| is_token(does))
        .unwrap_or_else(|| method.clone());
    Ok(Request { method, does, url, path, headers, body })
}

/// The origin of an http(s) address, as "https://host[:port]" in lower case without a default
/// port, or None for any other.
fn origin(url: &str) -> Option<String> {
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

/// The whole address and the path of `path` on `origin`: a path from its root, or a whole address
/// on that origin and no other.
fn resolve(origin: &str, path: &str) -> Result<(String, String), Wrong> {
    let malformed =
        || Wrong::Malformed("path is a path on the tab's site, such as /api/items.".into());
    if path.is_empty() || path.chars().any(|char| char.is_whitespace() || char.is_control()) {
        return Err(malformed());
    }
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
    if !path.starts_with('/') || path.starts_with("//") || path.starts_with("/\\") {
        return Err(malformed());
    }
    Ok((format!("{origin}{path}"), path.to_string()))
}

/// A token, as HTTP's methods and header names are.
fn is_token(text: &str) -> bool {
    !text.is_empty()
        && text.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

fn forbidden_method(method: &str) -> bool {
    matches!(method, "CONNECT" | "TRACE" | "TRACK")
}

/// Why an agent can't set the header, if it can't: one the browser sets, as a page's fetch can't
/// set it either (Chromium's net::HttpUtil::IsSafeHeader, which the core checks too), or a
/// sign-in of the agent's own.
fn forbidden_header(name: &str, value: &str) -> Option<String> {
    const SET_BY_THE_BROWSER: [&str; 22] = [
        "accept-charset", "accept-encoding", "access-control-request-headers",
        "access-control-request-method", "connection", "content-length", "cookie", "cookie2",
        "date", "dnt", "expect", "host", "keep-alive", "origin", "referer", "set-cookie", "te",
        "trailer", "transfer-encoding", "upgrade", "user-agent", "via",
    ];
    let lower = name.to_ascii_lowercase();
    if lower == "authorization" {
        return Some(
            "Agents can't set the Authorization header: they act in the page's own session, and \
             never send credentials of their own."
                .into(),
        );
    }
    let overrides = ["x-http-method", "x-http-method-override", "x-method-override"];
    let forbidden = SET_BY_THE_BROWSER.contains(&lower.as_str())
        || lower.starts_with("proxy-")
        || lower.starts_with("sec-")
        || overrides.contains(&lower.as_str())
            && value.split(',').any(|method| forbidden_method(&method.trim().to_ascii_uppercase()));
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

/// A body in a line: a JSON object's fields as "name: value", without the metadata a service
/// adds, and anything else's first line.
fn summary(body: &str) -> String {
    const LINE: usize = 60;
    let body = body.trim();
    if body.is_empty() {
        return String::new();
    }
    match serde_json::from_str::<Value>(body) {
        Ok(Value::Object(fields)) => {
            let metadata = |name: &str| {
                name.starts_with("__") || name.starts_with('@') || name.starts_with("odata.")
            };
            let fields = fields.iter().filter(|(name, _)| !metadata(name));
            let fields = fields.map(|(name, value)| match value {
                Value::String(text) => format!("{name}: {}", clip(text, 24)),
                Value::Array(_) | Value::Object(_) => format!("{name}: …"),
                value => format!("{name}: {value}"),
            });
            clip(&fields.collect::<Vec<_>>().join(", "), LINE)
        }
        Ok(Value::Array(items)) => format!("a list of {}", items.len()),
        _ => {
            let first = body.lines().next().unwrap_or_default();
            clip(&first.split_whitespace().collect::<Vec<_>>().join(" "), LINE)
        }
    }
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
        let kind = if response.kind.is_empty() { "data" } else { &response.kind };
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
    let (text, hid) = hide_secrets(&text, response.kind.contains("json"));
    *hidden += hid;
    let chars = text.chars().count();
    let shown = chars.min(SHOWN).min(*left);
    *left -= shown;
    let end = text.char_indices().nth(shown).map_or(text.len(), |(end, _)| end);
    answer.push_str("\n\n");
    answer.push_str(&text[..end]);
    if shown < chars {
        answer.push_str(&format!("\n… (the rest isn't shown: it's {chars} characters in all)"));
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
        || ["json", "xml", "javascript", "x-www-form-urlencoded"].iter().any(|each| kind.contains(each))
}

/// `text` without the values that look like secrets, and how many there were: JSON's fields named
/// as a token, key, password or session's id is, and anywhere a JWT or a bearer credential.
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

/// Whether a field by this name holds a secret. A CSRF token is the page's own, no use without
/// its cookie, and what a change may need, so it shows.
fn secret_name(name: &str) -> bool {
    let name: String = (name.chars()).filter(char::is_ascii_alphanumeric).collect();
    let name = name.to_ascii_lowercase();
    if ["csrf", "xsrf", "authenticity"].iter().any(|word| name.contains(word)) {
        return false;
    }
    let words = [
        "token", "secret", "password", "passwd", "apikey", "accesskey", "privatekey",
        "credential", "authorization", "cookie", "sessionid", "sessionkey", "jwt", "bearer",
    ];
    matches!(name.as_str(), "sid" | "pwd" | "auth") || words.iter().any(|word| name.contains(word))
}

fn hide_json(value: &mut Value, secret: bool) -> usize {
    match value {
        Value::String(text) if !text.is_empty() && (secret || token_like(text)) => {
            *value = Value::String(HIDDEN.into());
            1
        }
        Value::Array(items) => items.iter_mut().map(|item| hide_json(item, secret)).sum(),
        Value::Object(fields) => (fields.iter_mut())
            .map(|(name, item)| hide_json(item, secret || secret_name(name)))
            .sum(),
        _ => 0,
    }
}

/// Whether a value is a credential, whatever its field's name: a JWT, or a bearer one.
fn token_like(text: &str) -> bool {
    let text = text.trim();
    match text.strip_prefix("Bearer ").or_else(|| text.strip_prefix("bearer ")) {
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

/// As `hide_secrets`, in text that isn't JSON whole, such as a page's script or a body cut short:
/// each quoted field's value, and each JWT and bearer credential.
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
    while at < bytes.len() {
        let starts = at == 0 || !base64url(bytes[at - 1]);
        if bytes[at] == b'"' {
            if let Some((name, value)) = field(bytes, at) {
                if secret_name(&text[name.0..name.1]) && value.1 > value.0 {
                    hide(&mut kept, &mut copied, value.0, value.1);
                }
                at = value.1 + 1;
                continue;
            }
        } else if starts && bytes[at] == b'e' {
            let length = jwt(&bytes[at..]);
            if length > 0 {
                hide(&mut kept, &mut copied, at, at + length);
                at += length;
                continue;
            }
        } else if starts && (bytes[at..].starts_with(b"Bearer ") || bytes[at..].starts_with(b"bearer "))
        {
            let from = at + 7;
            let mut to = from;
            while to < bytes.len() && (base64url(bytes[to]) || b".~+/=".contains(&bytes[to])) {
                to += 1;
            }
            if to - from >= 8 {
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

/// The quoted name at `at`, then a colon, then a quoted value: the bounds of the name's text and
/// the value's, inside their quotes.
fn field(bytes: &[u8], at: usize) -> Option<((usize, usize), (usize, usize))> {
    // A name is short; a longer string is a value, which is looked through as text.
    let name_end = quoted(bytes, at, 64)?;
    let mut next = name_end + 1;
    while bytes.get(next).is_some_and(u8::is_ascii_whitespace) {
        next += 1;
    }
    if bytes.get(next) != Some(&b':') {
        return None;
    }
    next += 1;
    while bytes.get(next).is_some_and(u8::is_ascii_whitespace) {
        next += 1;
    }
    if bytes.get(next) != Some(&b'"') {
        return None;
    }
    let value_end = quoted(bytes, next, usize::MAX)?;
    Some(((at + 1, name_end), (next + 1, value_end)))
}

/// Where the string quoted at `at` closes, if within `most` bytes and on its line.
fn quoted(bytes: &[u8], at: usize, most: usize) -> Option<usize> {
    let mut next = at + 1;
    while next < bytes.len() && next - at <= most {
        match bytes[next] {
            b'\\' => next += 2,
            b'"' => return Some(next),
            b'\n' => return None,
            _ => next += 1,
        }
    }
    None
}

/// What a call's requests are, for a task's trace: one's method and path, or how many.
pub fn described(arguments: &Value) -> String {
    let requests = arguments.get("requests").and_then(Value::as_array);
    match requests.map(Vec::as_slice) {
        Some([one]) => {
            let text = |name: &str| one.get(name).and_then(Value::as_str).unwrap_or("").trim();
            format!("{} {}", text("method").to_ascii_uppercase(), clip(text("path"), 40))
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

    fn parsed(requests: Value) -> Result<Vec<Request>, Wrong> {
        parse(&json!({ "requests": requests }), PAGE)
    }

    /// A path goes to the page's own origin, and a whole address only if it's on it.
    #[test]
    fn requests_go_to_the_pages_own_site() {
        let sent = parsed(json!([{ "method": "get", "path": "/api/items?page=2#x" }])).unwrap();
        assert_eq!(sent[0].url, "https://team.example.com/api/items?page=2");
        assert_eq!((sent[0].method.as_str(), sent[0].path.as_str()), ("GET", "/api/items?page=2"));
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
            assert!(matches!(parsed(asked), Err(Wrong::Refused(_))), "{elsewhere} went");
        }
        for wrong in ["//evil.example/a", "/\\evil.example/a", "api/items", "/a b", ""] {
            let asked = json!([{ "method": "GET", "path": wrong }]);
            assert!(matches!(parsed(asked), Err(Wrong::Malformed(_))), "{wrong:?} went");
        }
        let blank = parse(&json!({ "requests": [{ "method": "GET", "path": "/" }] }), "about:blank");
        assert!(matches!(blank, Err(Wrong::Malformed(_))));
    }

    /// The headers the browser sets are the browser's, Authorization is a credential, and a
    /// method override to what pages can't send is refused, as Chromium refuses it.
    #[test]
    fn the_browsers_headers_and_credentials_are_refused() {
        let with = |name: &str, value: &str| {
            parsed(json!([{ "method": "POST", "path": "/a", "headers": { name: value } }]))
        };
        let refused = [
            "Cookie", "cookie", "Host", "Origin", "Referer", "Sec-Fetch-Site",
            "Proxy-Authorization", "Authorization",
        ];
        for name in refused {
            assert!(matches!(with(name, "x"), Err(Wrong::Refused(_))), "{name} was set");
        }
        assert!(matches!(with("X-HTTP-Method-Override", "TRACE"), Err(Wrong::Refused(_))));
        assert!(matches!(with("Bad Name", "x"), Err(Wrong::Malformed(_))));
        let sent = with("X-CSRF-Token", "abc").unwrap();
        assert_eq!(sent[0].headers, [("X-CSRF-Token".to_string(), "abc".to_string())]);
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
              "headers": { "X-HTTP-Method": "merge" }, "body": item },
            { "method": "GET", "path": "/a", "headers": { "X-HTTP-Method-Override": "DELETE" } },
            { "method": "HEAD", "path": "/a" },
        ]);
        let sent = parsed(asked).unwrap();
        assert_eq!(sent[0].line(), "MERGE /_api/…/items(1)");
        assert_eq!(sent[0].summary(), "Status: Closed");
        assert_eq!(sent[0].headers.last().unwrap().1, "application/json");
        assert!(sent[1].changes() && sent[1].does == "DELETE" && sent[1].method == "GET");
        assert!(!sent[2].changes());
    }

    /// A body is JSON, typed so, or text, typed as text unless it says; a GET has none.
    #[test]
    fn bodies_are_typed_as_a_pages_fetch_types_them() {
        let form = json!({ "Content-Type": "application/x-www-form-urlencoded" });
        let asked = json!([{ "method": "PUT", "path": "/a", "body": "a=1&b=2", "headers": form }]);
        let sent = parsed(asked).unwrap();
        assert_eq!(sent[0].body, "a=1&b=2");
        assert_eq!(sent[0].headers.len(), 1);
        let text = parsed(json!([{ "method": "POST", "path": "/a", "body": "hi" }])).unwrap();
        assert_eq!(text[0].headers[0].1, "text/plain;charset=UTF-8");
        let get = parsed(json!([{ "method": "GET", "path": "/a", "body": "hi" }]));
        assert!(matches!(get, Err(Wrong::Malformed(_))));
        let many: Vec<Value> = (0..=MAX).map(|_| json!({ "method": "GET", "path": "/" })).collect();
        assert!(matches!(parsed(Value::Array(many)), Err(Wrong::Malformed(_))));
    }

    /// The answer has the status, the type and a text body, never a binary one, and none of
    /// what looks like a credential.
    #[test]
    fn answers_show_text_without_secrets() {
        let sent = &parsed(json!([{ "method": "GET", "path": "/api/auth/session" }])).unwrap()[0];
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9P";
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
        assert!(!kept.contains("sk-live") && !kept.contains("abcdefghijkl") && !kept.contains("eyJ"));

        let image = Response {
            status: 200,
            kind: "image/png".into(),
            body: vec![0x89, 0x50],
            ..Response::default()
        };
        let read = answer(sent, &image, &mut left, &mut hidden);
        assert!(read.ends_with("(2 bytes of image/png, not shown)"), "{read}");
        let error = Some("No answer came: ERR_TIMED_OUT".to_string());
        let none = Response { error, ..Response::default() };
        assert_eq!(
            answer(sent, &none, &mut left, &mut hidden),
            "GET /api/auth/session: No answer came: ERR_TIMED_OUT"
        );
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
        assert!(first.ends_with(&format!("(the rest isn't shown: it's {all} characters in all)")));
        assert_eq!(left, 10);
        let second = answer(sent, &long, &mut left, &mut hidden);
        assert!(second.contains(&format!("\n\n{}\n…", "é".repeat(10))));
        let third = answer(sent, &long, &mut left, &mut hidden);
        assert!(third.contains("Its body isn't shown"));
    }
}
